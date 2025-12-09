use super::gpu_renderer::{
    AtlasUpdate, GlyphAtlasConfig, GlyphInstance, GpuRenderer, StandaloneGlyph,
};
use crate::font_storage::FontStorage;
use crate::text::TextLayout;
use bytemuck::{Pod, Zeroable};
use std::sync::Arc;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct InstanceData {
    pub screen_rect: [f32; 4], // x, y, w, h
    pub uv_rect: [f32; 4],     // u, v, w, h
    pub color: [f32; 4],
    pub layer: u32,
    pub _padding: [u32; 3],
}

pub trait ToInstance {
    fn to_color(&self) -> [f32; 4];
}

impl ToInstance for [f32; 4] {
    fn to_color(&self) -> [f32; 4] {
        *self
    }
}

impl ToInstance for () {
    fn to_color(&self) -> [f32; 4] {
        [1.0, 1.0, 1.0, 1.0]
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Globals {
    screen_size: [f32; 2],
    _padding: [f32; 2],
}

pub struct WgpuRenderer {
    pub gpu_renderer: GpuRenderer,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    pipeline: wgpu::RenderPipeline,
    standalone_pipeline: wgpu::RenderPipeline,
    atlas_texture: wgpu::Texture,
    atlas_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    instance_buffer: std::cell::RefCell<wgpu::Buffer>,
    instance_capacity: std::cell::Cell<usize>,
    bind_group_layout: wgpu::BindGroupLayout,
    standalone_bind_group_layout: wgpu::BindGroupLayout,
    globals_buffer: wgpu::Buffer,
    globals_bind_group: wgpu::BindGroup,
}

const SHADER: &str = include_str!("wgpu_renderer_shader.wgsl");

const STANDALONE_SHADER: &str = include_str!("wgpu_renderer_standalone.wgsl");

impl WgpuRenderer {
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        configs: Vec<GlyphAtlasConfig>,
        format: wgpu::TextureFormat,
    ) -> Self {
        let gpu_renderer = GpuRenderer::new(configs.clone());

        // Calculate max dimensions and layers
        let max_width = configs
            .iter()
            .map(|c| c.texture_size.get())
            .max()
            .unwrap_or(512) as u32;
        let max_height = configs
            .iter()
            .map(|c| c.texture_size.get())
            .max()
            .unwrap_or(512) as u32;
        let layers = configs.len() as u32;

        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Glyph Atlas Array"),
            size: wgpu::Extent3d {
                width: max_width,
                height: max_height,
                depth_or_array_layers: layers,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("WgpuRenderer Bind Group Layout"),
            entries: &[
                // Globals
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Texture Array
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        // Standalone layout (Texture 2D instead of Array)
        let standalone_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("WgpuRenderer Standalone Bind Group Layout"),
                entries: &[
                    // Globals
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // Texture 2D
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("WgpuRenderer Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let standalone_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("WgpuRenderer Standalone Pipeline Layout"),
                bind_group_layouts: &[&standalone_bind_group_layout],
                push_constant_ranges: &[],
            });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("WgpuRenderer Shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });

        let standalone_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("WgpuRenderer Standalone Shader"),
            source: wgpu::ShaderSource::Wgsl(STANDALONE_SHADER.into()),
        });

        let instance_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<InstanceData>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // screen_rect
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // uv_rect
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // color
                wgpu::VertexAttribute {
                    offset: 32,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // layer
                wgpu::VertexAttribute {
                    offset: 48,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Uint32,
                },
            ],
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("WgpuRenderer Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[instance_buffer_layout.clone()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let standalone_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("WgpuRenderer Standalone Pipeline"),
            layout: Some(&standalone_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &standalone_shader,
                entry_point: Some("vs_main"),
                buffers: &[instance_buffer_layout], // Same layout
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &standalone_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let instance_capacity = 1024;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Instance Buffer"),
            size: (instance_capacity * std::mem::size_of::<InstanceData>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Globals Buffer"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Globals Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: globals_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
            ],
        });

        Self {
            gpu_renderer,
            device,
            queue,
            pipeline,
            standalone_pipeline,
            atlas_texture,
            atlas_view,
            sampler,
            instance_buffer: std::cell::RefCell::new(instance_buffer),
            instance_capacity: std::cell::Cell::new(instance_capacity),
            bind_group_layout,
            standalone_bind_group_layout,
            globals_buffer,
            globals_bind_group,
        }
    }

    pub fn render<T: ToInstance + Copy>(
        &mut self,
        layout: &TextLayout<T>,
        font_storage: &mut FontStorage,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        screen_size: [f32; 2],
    ) {
        // Update globals
        let globals = Globals {
            screen_size,
            _padding: [0.0; 2],
        };
        self.queue
            .write_buffer(&self.globals_buffer, 0, bytemuck::bytes_of(&globals));

        let device = &self.device;
        let queue = &self.queue;
        let atlas_texture = &self.atlas_texture;

        let encoder_cell = std::cell::RefCell::new(encoder);

        self.gpu_renderer.render(
            layout,
            font_storage,
            &mut |updates: Vec<AtlasUpdate>| {
                for update in updates {
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: atlas_texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d {
                                x: update.x as u32,
                                y: update.y as u32,
                                z: update.texture_index as u32,
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
                        &update.pixels,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(update.width as u32),
                            rows_per_image: Some(update.height as u32),
                        },
                        wgpu::Extent3d {
                            width: update.width as u32,
                            height: update.height as u32,
                            depth_or_array_layers: 1,
                        },
                    );
                }
            },
            &mut |instances: Vec<GlyphInstance<T>>| {
                if instances.is_empty() {
                    return;
                }

                // Resize buffer if needed
                if instances.len() > self.instance_capacity.get() {
                    self.instance_capacity
                        .set(instances.len().next_power_of_two());
                    *self.instance_buffer.borrow_mut() =
                        device.create_buffer(&wgpu::BufferDescriptor {
                            label: Some("Instance Buffer"),
                            size: (self.instance_capacity.get()
                                * std::mem::size_of::<InstanceData>())
                                as u64,
                            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                            mapped_at_creation: false,
                        });
                }

                let instance_data: Vec<InstanceData> = instances
                    .iter()
                    .map(|inst| InstanceData {
                        screen_rect: [
                            inst.screen_rect.min.x,
                            inst.screen_rect.min.y,
                            inst.screen_rect.width(),
                            inst.screen_rect.height(),
                        ],
                        uv_rect: [
                            inst.uv_rect.min.x,
                            inst.uv_rect.min.y,
                            inst.uv_rect.width(),
                            inst.uv_rect.height(),
                        ],
                        color: inst.user_data.to_color(),
                        layer: inst.texture_index as u32,
                        _padding: [0; 3],
                    })
                    .collect();

                queue.write_buffer(
                    &self.instance_buffer.borrow(),
                    0,
                    bytemuck::cast_slice(&instance_data),
                );

                let mut encoder = encoder_cell.borrow_mut();
                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Text Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                rpass.set_pipeline(&self.pipeline);
                rpass.set_bind_group(0, &self.globals_bind_group, &[]);
                rpass.set_vertex_buffer(
                    0,
                    self.instance_buffer.borrow().slice(
                        0..(instance_data.len() * std::mem::size_of::<InstanceData>()) as u64,
                    ),
                );
                rpass.draw(0..4, 0..instance_data.len() as u32);
            },
            &mut |standalone: StandaloneGlyph<T>| {
                // Create temporary texture
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("Standalone Glyph Texture"),
                    size: wgpu::Extent3d {
                        width: standalone.width as u32,
                        height: standalone.height as u32,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::R8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });

                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &standalone.pixels,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(standalone.width as u32),
                        rows_per_image: Some(standalone.height as u32),
                    },
                    wgpu::Extent3d {
                        width: standalone.width as u32,
                        height: standalone.height as u32,
                        depth_or_array_layers: 1,
                    },
                );

                let view_resource = texture.create_view(&wgpu::TextureViewDescriptor::default());

                // Create bind group for standalone
                let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Standalone Bind Group"),
                    layout: &self.standalone_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: self.globals_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(&view_resource),
                        },
                    ],
                });

                // Instance data for standalone
                let instance_data = InstanceData {
                    screen_rect: [
                        standalone.screen_rect.min.x,
                        standalone.screen_rect.min.y,
                        standalone.screen_rect.width(),
                        standalone.screen_rect.height(),
                    ],
                    uv_rect: [0.0, 0.0, 1.0, 1.0],
                    color: standalone.user_data.to_color(),
                    layer: 0,
                    _padding: [0; 3],
                };

                // Use the main instance buffer? Or a temp one?
                // Just write to the start of the instance buffer.
                queue.write_buffer(
                    &self.instance_buffer.borrow(),
                    0,
                    bytemuck::bytes_of(&instance_data),
                );

                let mut encoder = encoder_cell.borrow_mut();
                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Standalone Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                rpass.set_pipeline(&self.standalone_pipeline);
                rpass.set_bind_group(0, &bind_group, &[]);
                rpass.set_vertex_buffer(
                    0,
                    self.instance_buffer
                        .borrow()
                        .slice(0..std::mem::size_of::<InstanceData>() as u64),
                );
                rpass.draw(0..4, 0..1);
            },
        );
    }
}

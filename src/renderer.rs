pub mod cpu_renderer;
pub mod gpu_renderer;
pub use cpu_renderer::CpuRenderer;
pub use gpu_renderer::GpuRenderer;

#[cfg(feature = "wgpu")]
pub mod wgpu_renderer;
#[cfg(feature = "wgpu")]
pub use wgpu_renderer::WgpuRenderer;

// debug uses
#[cfg(debug_assertions)]
pub mod debug_renderer;
#[cfg(all(debug_assertions, feature = "wgpu"))]
pub mod cpu_debug_renderer;

use crate::font_storage::FontStorage;
use crate::text::{GlyphPosition, TextLayout};

use super::{GlyphCache, LayoutRenderer};

/// Simple L8 bitmap produced by the CPU renderer.
///
/// Pixels are arranged in row-major order with the origin at the top-left.
/// Each pixel stores a single 8-bit coverage value where `0` represents
/// transparent/empty and `255` is fully opaque.
pub struct CpuBitmap {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl CpuBitmap {
    pub fn new(width: usize, height: usize) -> Self {
        let len = width.saturating_mul(height);
        Self {
            width,
            height,
            pixels: vec![0; len],
        }
    }
}

/// Default CPU implementation of [`LayoutRenderer`].
///
/// This type is stateless and can be freely shared.
pub struct DefaultLayoutRenderer;

impl DefaultLayoutRenderer {
    pub fn new() -> Self {
        Self
    }

    fn render_glyph_into_bitmap<C: GlyphCache>(
        &self,
        cache: &C,
        bitmap: &mut CpuBitmap,
        glyph_pos: &GlyphPosition,
        font_storage: &mut FontStorage,
    ) {
        let Some(cached) = cache.get(glyph_pos.glyph_id, font_storage) else {
            return;
        };

        if cached.width == 0 || cached.height == 0 {
            return;
        }

        let glyph_width = cached.width;
        let glyph_height = cached.height;
        let origin_x = glyph_pos.x;
        let origin_y = glyph_pos.y;

        for row in 0..glyph_height {
            let y = origin_y + row as f32;
            if y < 0.0 {
                continue;
            }
            let iy = y.floor() as isize;
            if iy < 0 || iy as usize >= bitmap.height {
                continue;
            }

            for col in 0..glyph_width {
                let src_alpha = cached.data[row * glyph_width + col];
                if src_alpha == 0 {
                    continue;
                }

                let x = origin_x + col as f32;
                if x < 0.0 {
                    continue;
                }

                let ix = x.floor() as isize;
                if ix < 0 || ix as usize >= bitmap.width {
                    continue;
                }

                let idx = iy as usize * bitmap.width + ix as usize;
                let existing = bitmap.pixels[idx] as u16;
                let combined = existing.saturating_add(src_alpha as u16).min(255);
                bitmap.pixels[idx] = combined as u8;
            }
        }
    }
}

impl<C> LayoutRenderer<C> for DefaultLayoutRenderer
where
    C: GlyphCache,
{
    fn render_layout(
        &self,
        cache: &C,
        layout: &TextLayout,
        image_size: [usize; 2],
        font_storage: &mut FontStorage,
    ) -> CpuBitmap {
        let width = image_size[0];
        let height = image_size[1];

        if width == 0 || height == 0 {
            return CpuBitmap::new(0, 0);
        }

        let mut bitmap = CpuBitmap::new(width, height);
        for line in &layout.lines {
            for glyph in &line.glyphs {
                self.render_glyph_into_bitmap(cache, &mut bitmap, glyph, font_storage);
            }
        }

        bitmap
    }
}

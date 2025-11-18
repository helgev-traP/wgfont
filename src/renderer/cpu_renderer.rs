mod render;
mod vec_cache;

use crate::font_storage::FontStorage;
use crate::glyph_id::GlyphId;
use crate::text::TextLayout;

pub use render::{CpuBitmap, DefaultLayoutRenderer};
pub use vec_cache::MokaGlyphCache;

/// Cached glyph bitmap used by CPU rendering.
pub struct CachedGlyph<'a> {
    pub width: usize,
    pub height: usize,
    pub data: &'a [u8],
}

/// Abstraction over a glyph cache.
pub trait GlyphCache: Send + Sync {
    fn get<'a>(
        &'a self,
        glyph_id: GlyphId,
        font_storage: &mut FontStorage,
    ) -> Option<CachedGlyph<'a>>;
}

/// Abstraction over a layout renderer which can draw glyphs into a bitmap
/// using a glyph cache.
pub trait LayoutRenderer<C: GlyphCache> {
    fn render_layout(
        &self,
        cache: &C,
        layout: &TextLayout,
        image_size: [usize; 2],
        font_storage: &mut FontStorage,
    ) -> CpuBitmap;
}

/// CPU-based renderer that rasterizes glyphs using a cache and a rendering
/// backend implementation.
pub struct CpuRenderer<C, R>
where
    C: GlyphCache,
    R: LayoutRenderer<C>,
{
    cache: C,
    renderer: R,
}

impl<C, R> CpuRenderer<C, R>
where
    C: GlyphCache,
    R: LayoutRenderer<C>,
{
    /// Creates a renderer from the provided cache and renderer implementation.
    pub fn with_parts(cache: C, renderer: R) -> Self {
        Self { cache, renderer }
    }

    /// Renders the provided [`TextLayout`] into an [`CpuBitmap`].
    pub fn render_layout(
        &self,
        layout: &TextLayout,
        image_size: [usize; 2],
        font_storage: &mut FontStorage,
    ) -> CpuBitmap {
        self.renderer
            .render_layout(&self.cache, layout, image_size, font_storage)
    }

    /// Returns a reference to the underlying glyph cache.
    pub fn cache(&self) -> &C {
        &self.cache
    }

    /// Returns a reference to the underlying layout renderer implementation.
    pub fn renderer(&self) -> &R {
        &self.renderer
    }
}

impl CpuRenderer<MokaGlyphCache, DefaultLayoutRenderer> {
    /// Creates a new renderer with a glyph cache limited by `cache_capacity_bytes`.
    pub fn new(cache_capacity_bytes: u64) -> Self {
        let cache = MokaGlyphCache::new(cache_capacity_bytes);
        let renderer = DefaultLayoutRenderer::new();
        Self { cache, renderer }
    }
}

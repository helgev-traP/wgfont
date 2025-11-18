use crate::font_storage::FontStorage;
use crate::glyph_id::GlyphId;
use mini_moka::sync::Cache;
use std::sync::Arc;

use super::{CachedGlyph, GlyphCache};

struct CachedGlyphData {
    width: usize,
    height: usize,
    data: Vec<u8>,
}

impl CachedGlyphData {
    fn weight(&self) -> u32 {
        self.data.len().min(u32::MAX as usize) as u32
    }
}

/// Glyph cache implementation backed by `mini_moka`.
pub struct MokaGlyphCache {
    glyph_cache: Cache<GlyphId, Arc<CachedGlyphData>>,
}

impl MokaGlyphCache {
    /// Creates a new cache limited by `cache_capacity_bytes`.
    pub fn new(cache_capacity_bytes: u64) -> Self {
        let capacity = cache_capacity_bytes.max(1);
        let glyph_cache = Cache::builder()
            .max_capacity(capacity)
            .weigher(|_, glyph: &Arc<CachedGlyphData>| glyph.weight())
            .build();

        Self { glyph_cache }
    }
}

impl GlyphCache for MokaGlyphCache {
    fn get<'a>(
        &'a self,
        glyph_id: GlyphId,
        font_storage: &mut FontStorage,
    ) -> Option<CachedGlyph<'a>> {
        if let Some(glyph) = self.glyph_cache.get(&glyph_id) {
            return Some(CachedGlyph {
                width: glyph.width,
                height: glyph.height,
                data: &glyph.data,
            });
        }

        let font = font_storage.font(glyph_id.font_id())?;
        let (metrics, coverage) =
            font.rasterize_indexed(glyph_id.glyph_index(), glyph_id.font_size());

        if metrics.width == 0 || metrics.height == 0 {
            return None;
        }

        let cached = Arc::new(CachedGlyphData {
            width: metrics.width,
            height: metrics.height,
            data: coverage,
        });

        let result = CachedGlyph {
            width: cached.width,
            height: cached.height,
            data: &cached.data,
        };

        self.glyph_cache.insert(glyph_id, cached.clone());
        Some(result)
    }
}

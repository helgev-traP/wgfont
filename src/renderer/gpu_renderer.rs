use std::collections::HashSet;

use crate::{
    font_storage::FontStorage,
    text::{GlyphPosition, TextLayout},
};

mod glyph_cache;
pub use glyph_cache::{CacheAtlas, GlyphAtlasConfig, GlyphCache, GlyphCacheItem};

pub struct GpuRenderer {
    cache: GlyphCache,
}

impl GpuRenderer {
    pub fn new(configs: Vec<GlyphAtlasConfig>) -> Self {
        Self {
            cache: GlyphCache::new(configs),
        }
    }

    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    pub fn render(
        &mut self,
        layout: &TextLayout,
        font_storage: &mut FontStorage,
        mut f: impl FnMut(&GlyphPosition, &GlyphCacheItem),
    ) {
        todo!()
    }

    pub fn render_true_order(
        &mut self,
        layout: &TextLayout,
        font_storage: &mut FontStorage,
        mut f: impl FnMut(&GlyphPosition, &GlyphCacheItem),
    ) {
        let mut not_yet_rendered = layout
            .lines
            .iter()
            .flat_map(|line| line.glyphs.iter().cloned())
            .collect::<HashSet<_, fxhash::FxBuildHasher>>();

        let mut pingpong_set = HashSet::<_, fxhash::FxBuildHasher>::default();

        while !not_yet_rendered.is_empty() {
            let mut render_in_this_batch = Vec::new();

            // 1
            // protect entry
            for glyph in not_yet_rendered.drain() {
                if let Some(cached_glyph) = self
                    .cache
                    .get_and_protect_entry(&glyph.glyph_id, font_storage)
                {
                    render_in_this_batch.push((glyph, cached_glyph));
                    pingpong_set.insert(glyph);
                }
            }
            std::mem::swap(&mut not_yet_rendered, &mut pingpong_set);

            // 2
            // push with evicting unprotected
            for glyph in not_yet_rendered.drain() {
                if let Some(cached_glyph) = self
                    .cache
                    .get_and_push_with_evicting_unprotected(&glyph.glyph_id, font_storage)
                {
                    render_in_this_batch.push((glyph, cached_glyph));
                    pingpong_set.insert(glyph);
                }
            }
            std::mem::swap(&mut not_yet_rendered, &mut pingpong_set);

            // 3
            // render
            for (glyph, cached_glyph) in render_in_this_batch {
                todo!()
            }
        }
    }
}

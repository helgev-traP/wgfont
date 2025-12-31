use std::collections::HashSet;

use crate::{glyph_id::GlyphId, text::TextData};

/// Configuration knobs used by the text layout pipeline.
///
/// All parameters are honored during a single `TextData::layout` call so the
/// caller can measure or place text inside arbitrary rectangles.
#[derive(Clone, Debug, PartialEq)]
pub struct TextLayoutConfig {
    /// Maximum width of the layout box. If text exceeds this, it may wrap or overflow.
    pub max_width: Option<f32>,
    /// Maximum height of the layout box.
    pub max_height: Option<f32>,
    /// Horizontal alignment of the text within the layout box.
    pub horizontal_align: HorizontalAlign,
    /// Vertical alignment of the text within the layout box.
    pub vertical_align: VerticalAlign,
    /// Scaling factor for the line height.
    pub line_height_scale: f32,
    /// Strategy for wrapping text.
    pub wrap_style: WrapStyle,
    /// Whether to force a hard break when text exceeds width, even in the middle of a word (if word wrapping fails).
    pub wrap_hard_break: bool,
    /// Characters that are considered word separators for wrapping.
    pub word_separators: HashSet<char, fxhash::FxBuildHasher>,
    /// Characters that trigger a hard line break.
    pub linebreak_char: HashSet<char, fxhash::FxBuildHasher>,
}

impl Default for TextLayoutConfig {
    fn default() -> Self {
        Self {
            max_width: None,
            max_height: None,
            horizontal_align: HorizontalAlign::Left,
            vertical_align: VerticalAlign::Top,
            line_height_scale: 1.0,
            wrap_style: WrapStyle::NoWrap,
            wrap_hard_break: false,
            // TODO: implement tab handling.
            word_separators: [' ', '\t', '\n', '\r'].iter().cloned().collect(),
            linebreak_char: ['\n', '\r'].iter().cloned().collect(),
        }
    }
}

/// Horizontal justification applied after each line is assembled.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HorizontalAlign {
    /// Align text to the left.
    #[default]
    Left,
    /// Center text horizontally.
    Center,
    /// Align text to the right.
    Right,
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// Vertical alignment strategy for the entire block of text.
pub enum VerticalAlign {
    /// Align text to the top.
    #[default]
    Top,
    /// Center text vertically.
    Middle,
    /// Align text to the bottom.
    Bottom,
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// Wrapping rules that define where line breaks may occur.
pub enum WrapStyle {
    /// Wrap text at word boundaries.
    #[default]
    WordWrap,
    /// Wrap text at any character.
    CharWrap,
    /// Do not wrap text.
    NoWrap,
}

/// Final layout output produced by [`TextData::layout`].
#[derive(Clone, Debug, PartialEq)]
pub struct TextLayout<T> {
    /// The configuration used for this layout.
    pub config: TextLayoutConfig,
    /// The total height of the laid out text.
    pub total_height: f32,
    /// The total width of the laid out text.
    pub total_width: f32,
    /// The lines of text in the layout.
    pub lines: Vec<TextLayoutLine<T>>,
}

impl<T> TextLayout<T> {
    /// Returns the number of lines in the layout.
    pub fn len_lines(&self) -> usize {
        self.lines.len()
    }

    /// Returns the total number of glyphs in the layout (sum of glyphs in all lines).
    pub fn len_glyphs(&self) -> usize {
        self.lines.iter().map(|line| line.glyphs.len()).sum()
    }
}

/// A single row of positioned glyphs in the final layout.
#[derive(Clone, Debug, PartialEq)]
pub struct TextLayoutLine<T> {
    /// The height of this line.
    pub line_height: f32,
    /// The width of this line.
    pub line_width: f32,
    /// The Y coordinate of the top of this line.
    pub top: f32,
    /// The Y coordinate of the bottom of this line.
    pub bottom: f32,
    /// The glyphs contained in this line.
    pub glyphs: Vec<GlyphPosition<T>>,
}

/// **Y-axis goes down**
///
/// Each glyph uses the global coordinates generated during layout so renderers
/// can draw them directly without additional transformations.
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphPosition<T> {
    /// The unique identifier for the glyph.
    pub glyph_id: GlyphId,
    /// The absolute X coordinate of the glyph.
    pub x: f32,
    /// The absolute Y coordinate of the glyph.
    pub y: f32,
    /// Custom user data associated with this glyph.
    pub user_data: T,
}
// place holder for eq and hash
// todo: consider another way
impl<T: Eq> Eq for GlyphPosition<T> {}
impl<T: std::hash::Hash> std::hash::Hash for GlyphPosition<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.glyph_id.hash(state);
        self.x.to_bits().hash(state);
        self.y.to_bits().hash(state);
        self.user_data.hash(state);
    }
}

/// Intermediate storage used while collecting glyphs for a single line.
struct LineRecord<T> {
    buffer: Option<layout_utl::LayoutBuffer<T>>,
    metrics: Option<fontdue::LineMetrics>,
}

impl<T: Clone> TextData<T> {
    /// Computes the bounding box that would be produced by [`Self::layout`].
    ///
    /// This helper simply forwards to `layout` because the layout stage must
    /// still run to honor wrapping, alignment, and kerning rules. The resulting
    /// size is returned as `[width, height]` for convenience.
    pub fn measure(
        &self,
        config: &TextLayoutConfig,
        font_storage: &mut crate::font_storage::FontStorage,
    ) -> [f32; 2] {
        let layout = self.layout(config, font_storage);
        [layout.total_width, layout.total_height]
    }

    /// Performs glyph layout according to the provided configuration.
    ///
    /// The implementation follows a two-stage pipeline:
    /// 1. Each input character is translated into glyph fragments that are
    ///    buffered into line records while respecting wrap style and width
    ///    constraints.
    /// 2. The buffered lines are converted into final glyph positions with
    ///    alignment offsets applied.
    ///
    /// Breaking the work into stages keeps the code readable and allows future
    /// extensions such as hyphenation without rewriting the core placement
    /// logic.
    pub fn layout(
        &self,
        config: &TextLayoutConfig,
        font_storage: &mut crate::font_storage::FontStorage,
    ) -> TextLayout<T> {
        LayoutEngine::new(config, font_storage).layout(&self.texts)
    }
}

struct LayoutEngine<'a, T> {
    config: &'a TextLayoutConfig,
    font_storage: &'a mut crate::font_storage::FontStorage,

    // State
    lines: Vec<LineRecord<T>>,
    line_buf: Option<layout_utl::LayoutBuffer<T>>,
    word_buf: Option<Vec<layout_utl::GlyphFragment<T>>>,
    last_line_metrics: Option<fontdue::LineMetrics>,
}

impl<'a, T: Clone> LayoutEngine<'a, T> {
    fn new(
        config: &'a TextLayoutConfig,
        font_storage: &'a mut crate::font_storage::FontStorage,
    ) -> Self {
        Self {
            config,
            font_storage,
            lines: Vec::new(),
            // Buffer for the line currently being built.
            line_buf: None,
            // Buffer for the word currently being built.
            word_buf: None,
            // Metrics of the last processed line, used for handling empty lines/newlines.
            last_line_metrics: None,
        }
    }

    fn layout(mut self, texts: &[crate::text::TextElement<T>]) -> TextLayout<T> {
        for text in texts {
            self.process_text_run(text);
        }

        // Flush remaining word buffer
        if let Some(word) = self.word_buf.take() {
            self.append_fragments_with_rules(&word, true);
        }

        // Ensure the last line is finalized, even if empty (to preserve vertical spacing).
        self.finalize_line(self.last_line_metrics);

        self.build_result()
    }

    fn process_text_run(&mut self, text: &crate::text::TextElement<T>) {
        use std::sync::Arc;

        let Some(font) = self.font_storage.font(text.font_id) else {
            return;
        };
        let Some(line_metric) = font.horizontal_line_metrics(text.font_size) else {
            return;
        };
        if text.content.is_empty() {
            return;
        }

        self.last_line_metrics = Some(line_metric);

        let create_fragment = |ch: char| {
            let glyph_idx = font.lookup_glyph_index(ch);
            let metrics = font.metrics_indexed(glyph_idx, text.font_size);
            layout_utl::GlyphFragment {
                ch,
                glyph_idx,
                metrics,
                line_metrics: line_metric,
                font_id: text.font_id,
                font_size: text.font_size,
                font: Arc::clone(&font),
                user_data: text.user_data.clone(),
            }
        };

        for ch in text.content.chars() {
            match layout_utl::classify_char(
                ch,
                &self.config.word_separators,
                &self.config.linebreak_char,
            ) {
                layout_utl::CharBehavior::LineBreak => {
                    // Newline characters always terminate the current line.
                    // If there is a pending word, append it to the current line first.
                    if let Some(word) = self.word_buf.take() {
                        self.append_fragments_with_rules(&word, true);
                    }

                    // We explicitly do not append the newline glyph to the layout.
                    // Instead, we just finalize the line with the current metrics.
                    self.finalize_line(Some(line_metric));
                }
                layout_utl::CharBehavior::WordBreak { render_glyph } => {
                    // A separator (e.g., space) marks the end of a word.
                    if let Some(word) = self.word_buf.take() {
                        self.append_fragments_with_rules(&word, true);
                    }

                    if render_glyph {
                        let fragment = create_fragment(ch);
                        // Append the separator itself (not part of the `word_buf`).
                        self.append_fragments_with_rules(std::slice::from_ref(&fragment), false);
                    }
                }
                layout_utl::CharBehavior::Regular => {
                    let fragment = create_fragment(ch);
                    if matches!(self.config.wrap_style, WrapStyle::CharWrap) {
                        // In CharWrap mode, we treat every character as an independent unit,
                        // bypassing the word buffer.
                        self.append_fragments_with_rules(std::slice::from_ref(&fragment), true);
                    } else {
                        // Accumulate characters into the word buffer until a break occurs.
                        match &mut self.word_buf {
                            Some(buffer) => buffer.push(fragment),
                            None => self.word_buf = Some(vec![fragment]),
                        }
                    }
                }
                layout_utl::CharBehavior::Ignore => {
                    // Skip control characters or invalid inputs.
                }
            }
        }
    }

    fn append_fragments_with_rules(
        &mut self,
        fragments: &[layout_utl::GlyphFragment<T>],
        allow_leading_space: bool,
    ) {
        if fragments.is_empty() {
            return;
        }

        // Rule: Drop leading spaces if they start a new line.
        // This prevents lines from looking indented due to a wrapped space.
        if !allow_leading_space
            && let Some(first) = fragments.first()
            && first.ch.is_whitespace()
            && self
                .line_buf
                .as_ref()
                .map(|line| line.glyphs.is_empty())
                .unwrap_or(true)
        {
            return;
        }

        self.append_fragments_to_line(fragments);
    }

    fn append_fragments_to_line(&mut self, fragments: &[layout_utl::GlyphFragment<T>]) {
        if fragments.is_empty() {
            return;
        }

        let limit = if self.config.wrap_style == WrapStyle::NoWrap {
            None
        } else {
            self.config.max_width
        };

        let Some(buffer) = layout_utl::LayoutBuffer::from_fragments(fragments, self.font_storage)
        else {
            return;
        };

        if let Some(limit_width) = limit {
            // Case 1: Try to append the entire fragment sequence to the current line.
            if let Some(current) = self.line_buf.as_mut() {
                let projected = current.projected_concat_length(&buffer, self.font_storage);
                if projected <= limit_width {
                    // It fits!
                    current.concat(buffer, self.font_storage);
                    return;
                }
            }

            // Case 2: It doesn't fit on the current line, so push the current line to `lines`.
            if self.line_buf.is_some() {
                self.push_line_buffer();
            }

            // Case 3: Try to put the entire fragment sequence on the new empty line.
            if buffer.width() <= limit_width {
                self.line_buf = Some(buffer);
                return;
            }

            // Case 4: It doesn't fit even on a new line (e.g., a very long word).
            if !self.config.wrap_hard_break {
                // If hard break is disabled, we just let it overflow.
                self.line_buf = Some(buffer);
                return;
            }

            // Case 5: Hard break is enabled. We must split the fragment sequence.
            let mut start = 0usize;
            while start < fragments.len() {
                let mut end = start + 1;
                // Start with the smallest possible chunk (1 char).
                let mut best = layout_utl::LayoutBuffer::from_fragments(
                    &fragments[start..end],
                    self.font_storage,
                )
                .expect("fragment slice must not be empty");

                // Even a single character might be too wide (edge case).
                if best.width() > limit_width {
                    self.push_line_buffer();
                    self.line_buf = Some(best);
                    start = end;
                    continue;
                }

                // Greedily extend the chunk as long as it fits.
                while end < fragments.len() {
                    let next_buf = layout_utl::LayoutBuffer::from_fragments(
                        &fragments[end..end + 1],
                        self.font_storage,
                    )
                    .expect("fragment slice must not be empty");

                    let projected = best.projected_concat_length(&next_buf, self.font_storage);
                    if projected > limit_width {
                        // Adding next char would exceed limit, so stop here.
                        break;
                    }

                    best.concat(next_buf, self.font_storage);
                    end += 1;
                }

                // Commit the chunk to a new line.
                self.push_line_buffer();
                self.line_buf = Some(best);
                start = end;

                // If there are more fragments, force a break for the next iteration.
                if start < fragments.len() {
                    self.push_line_buffer();
                }
            }
        } else {
            // No max width limit (NoWrap mode or unconfigured).
            if let Some(current) = self.line_buf.as_mut() {
                current.concat(buffer, self.font_storage);
            } else {
                self.line_buf = Some(buffer);
            }
        }
    }

    fn finalize_line(&mut self, metrics: Option<fontdue::LineMetrics>) {
        if self.line_buf.is_some() || metrics.is_some() {
            self.lines.push(LineRecord {
                buffer: self.line_buf.take(),
                metrics,
            });
        }
    }

    fn push_line_buffer(&mut self) {
        if self.line_buf.is_some() {
            self.lines.push(LineRecord {
                buffer: self.line_buf.take(),
                metrics: None,
            });
        }
    }

    fn build_result(self) -> TextLayout<T> {
        /// Final measurements for a single laid-out line before alignment.
        struct LineData<T> {
            width: f32,
            height: f32,
            y: f32,
            glyphs: Vec<GlyphPosition<T>>,
        }

        let mut layout_lines: Vec<LineData<T>> = Vec::new();
        let mut cursor_y = 0.0;
        let mut max_line_width: f32 = 0.0;
        let line_height_scale = self.config.line_height_scale;

        // Convert the abstract "lines" (buffers) into physical "LineData" (coordinates).
        for record in self.lines {
            let (width, ascent, descent, line_gap, glyphs) = if let Some(buffer) = record.buffer {
                let (ascent, descent, line_gap) = buffer.line_metrics();
                let width_value = buffer.width();
                let glyphs = buffer.glyphs;
                (width_value, ascent, descent, line_gap, glyphs)
            } else if let Some(metrics) = record.metrics {
                // Empty line but with valid metrics (e.g., from newline char).
                (
                    0.0,
                    metrics.ascent,
                    metrics.descent,
                    metrics.line_gap,
                    Vec::new(),
                )
            } else {
                // Fallback for completely empty state (should happen rarely).
                (0.0, 0.0, 0.0, 0.0, Vec::new())
            };

            max_line_width = max_line_width.max(width);
            let raw_line_height = ascent - descent + line_gap;
            let scaled_line_height = (raw_line_height * line_height_scale).max(0.0);

            // Baseline is relative to the *top* of the line box.
            let baseline = cursor_y + ascent;

            let mut glyph_positions = Vec::with_capacity(glyphs.len());
            for mut glyph in glyphs {
                glyph.y += baseline;
                glyph_positions.push(glyph);
            }

            cursor_y += scaled_line_height;

            layout_lines.push(LineData {
                width,
                height: scaled_line_height,
                y: cursor_y - scaled_line_height,
                glyphs: glyph_positions,
            });
        }

        let total_height = cursor_y;
        let total_width = max_line_width;

        let target_width = self.config.max_width.unwrap_or(total_width);
        let target_height = self.config.max_height.unwrap_or(total_height);

        let vertical_offset = match self.config.vertical_align {
            VerticalAlign::Top => 0.0,
            VerticalAlign::Middle => (target_height - total_height) / 2.0,
            VerticalAlign::Bottom => target_height - total_height,
        };

        let mut lines_out = Vec::with_capacity(layout_lines.len());

        for mut line in layout_lines {
            let horizontal_offset = match self.config.horizontal_align {
                HorizontalAlign::Left => 0.0,
                HorizontalAlign::Center => (target_width - line.width) / 2.0,
                HorizontalAlign::Right => target_width - line.width,
            };

            if horizontal_offset != 0.0 {
                for glyph in &mut line.glyphs {
                    glyph.x += horizontal_offset;
                }
            }

            if vertical_offset != 0.0 {
                for glyph in &mut line.glyphs {
                    glyph.y += vertical_offset;
                }
            }

            lines_out.push(TextLayoutLine {
                line_height: line.height,
                line_width: line.width,
                top: line.y + vertical_offset,
                bottom: line.y + vertical_offset + line.height,
                glyphs: line.glyphs,
            });
        }

        TextLayout {
            config: self.config.clone(),
            total_height,
            total_width,
            lines: lines_out,
        }
    }
}

mod layout_utl {
    use crate::font_storage::FontStorage;

    use super::*;
    use std::sync::Arc;

    /// Defines how a character should be handled during layout.
    pub enum CharBehavior {
        /// Always triggers a hard line break (e.g., newline).
        LineBreak,
        /// Breaks a word but may or may not be rendered (e.g., space, tab).
        WordBreak { render_glyph: bool },
        /// Standard character content.
        Regular,
        /// Character should be completely ignored (e.g., non-printable control chars).
        Ignore,
    }

    /// Classifies a character to determine its layout behavior.
    pub fn classify_char(
        ch: char,
        word_separators: &HashSet<char, fxhash::FxBuildHasher>,
        linebreak_char: &HashSet<char, fxhash::FxBuildHasher>,
    ) -> CharBehavior {
        if linebreak_char.contains(&ch) {
            return CharBehavior::LineBreak;
        }

        if word_separators.contains(&ch) {
            // Render the separator only if it is NOT a control character.
            // Spaces are not control chars, but tabs are.
            // This prevents "tofu" rendering for tabs until proper tab support is added.
            return CharBehavior::WordBreak {
                render_glyph: !ch.is_control(),
            };
        }

        if ch.is_control() {
            return CharBehavior::Ignore;
        }

        CharBehavior::Regular
    }

    #[derive(Clone)]
    /// Precomputed glyph data used to build layout buffers.
    ///
    /// Storing the font handle allows kerning to be applied without repeatedly
    /// fetching the same font from storage.
    pub struct GlyphFragment<T> {
        pub ch: char,
        pub glyph_idx: u16,
        pub metrics: fontdue::Metrics,
        pub line_metrics: fontdue::LineMetrics,
        pub font_id: fontdb::ID,
        pub font_size: f32,
        pub font: Arc<fontdue::Font>,
        pub user_data: T,
    }

    /// Buffer of glyph positions with origin located on the baseline.
    ///
    /// Layout buffers are concatenated as new fragments are processed, letting
    /// us calculate kerning-aware widths before the final glyph positions are
    /// produced.
    pub struct LayoutBuffer<T> {
        pub instance_length: f32,

        pub max_accent: f32,
        pub max_descent: f32,
        pub max_line_gap: f32,

        pub first_glyph: u16,
        pub first_font_id: fontdb::ID,
        pub first_font_size: f32,
        pub last_glyph: u16,
        pub last_font_id: fontdb::ID,
        pub last_font_size: f32,
        pub last_metrics: fontdue::Metrics,
        pub last_origin_x: f32,

        pub glyphs: Vec<GlyphPosition<T>>,
    }

    impl<T: Clone> LayoutBuffer<T> {
        /// Creates a buffer containing a single glyph fragment.
        ///
        /// The glyph is stored relative to the baseline so it can be shifted
        /// after all fragments for the line are known.
        pub fn new(
            glyph_idx: u16,
            metrics: &fontdue::Metrics,
            line_metrics: &fontdue::LineMetrics,
            font_id: fontdb::ID,
            font_size: f32,
            user_data: T,
        ) -> Self {
            let mut buffer = Self {
                instance_length: metrics.width as f32 + metrics.xmin as f32,
                max_accent: line_metrics.ascent,
                max_descent: line_metrics.descent,
                max_line_gap: line_metrics.line_gap,
                first_glyph: glyph_idx,
                first_font_id: font_id,
                first_font_size: font_size,
                last_glyph: glyph_idx,
                last_font_id: font_id,
                last_font_size: font_size,
                last_metrics: *metrics,
                last_origin_x: 0.0,
                glyphs: vec![],
            };

            buffer.glyphs.push(GlyphPosition {
                glyph_id: GlyphId::new(font_id, glyph_idx, font_size),
                x: metrics.xmin as f32,
                y: -(metrics.ymin as f32 + metrics.height as f32),
                user_data,
            });

            buffer
        }

        /// Appends another glyph to the buffer, updating metrics and kerning.
        ///
        /// The kerning calculation uses the provided font handle when the
        /// previous and new glyph share the same font and size. This keeps the
        /// layout accurate while avoiding redundant lookups.
        pub fn push(
            &mut self,
            glyph_idx: u16,
            metrics: &fontdue::Metrics,
            line_metrics: &fontdue::LineMetrics,
            font: &fontdue::Font,
            font_id: fontdb::ID,
            font_size: f32,
            user_data: T,
            _font_storage: &mut FontStorage,
        ) {
            let advance_kerned = if self.last_font_id == font_id
                && (self.last_font_size - font_size).abs() < f32::EPSILON
            {
                let kerning = font
                    .horizontal_kern_indexed(self.last_glyph, glyph_idx, font_size)
                    .unwrap_or(0.0);
                self.last_metrics.advance_width + kerning
            } else {
                // for simplicity, just ignore kerning for different font or size
                /*
                // use average kerning for different font or size

                let kerning_of_curr_font = font
                    .horizontal_kern_indexed(self.last_glyph, glyph_idx, font_size)
                    .unwrap_or(0.0);
                let kerning_of_prev_font = font_storage
                    .font(self.last_font_id)
                    .and_then(|f| {
                        f.horizontal_kern_indexed(self.last_glyph, glyph_idx, self.last_font_size)
                    })
                    .unwrap_or(0.0);

                let average_kerning = (kerning_of_curr_font + kerning_of_prev_font) / 2.0;

                self.last_metrics.advance_width + average_kerning
                */

                self.last_metrics.advance_width
            };

            let new_origin_x = self.last_origin_x + advance_kerned;

            self.instance_length = new_origin_x + metrics.width as f32 + metrics.xmin as f32;
            self.max_accent = self.max_accent.max(line_metrics.ascent);
            self.max_descent = self.max_descent.max(line_metrics.descent);
            self.max_line_gap = self.max_line_gap.max(line_metrics.line_gap);
            self.last_glyph = glyph_idx;
            self.last_font_id = font_id;
            self.last_font_size = font_size;
            self.last_metrics = *metrics;
            self.last_origin_x = new_origin_x;
            self.glyphs.push(GlyphPosition {
                glyph_id: GlyphId::new(font_id, glyph_idx, font_size),
                x: new_origin_x + metrics.xmin as f32,
                y: -(metrics.ymin as f32 + metrics.height as f32),
                user_data,
            });
        }

        /// Concatenates another layout buffer, adjusting positions in-place.
        ///
        /// When the buffers originate from the same font and size we apply
        /// kerning between the boundary glyphs; otherwise the buffers are joined
        /// using the recorded advance of the current buffer.
        pub fn concat(&mut self, other: LayoutBuffer<T>, font_storage: &mut FontStorage) {
            let advance_kerned = if self.last_font_id == other.first_font_id
                && (self.last_font_size - other.first_font_size).abs() < f32::EPSILON
            {
                let font = font_storage
                    .font(self.last_font_id)
                    .expect("font must exist in font storage");
                let kerning = font
                    .horizontal_kern_indexed(
                        self.last_glyph,
                        other.first_glyph,
                        self.last_font_size,
                    )
                    .unwrap_or(0.0);
                self.last_metrics.advance_width + kerning
            } else {
                // for simplicity, just ignore kerning for different font or size
                self.last_metrics.advance_width
            };

            let x_offset = self.last_origin_x + advance_kerned;

            let new_instance_length = x_offset + other.instance_length;
            let new_origin_x = x_offset + other.last_origin_x;

            self.instance_length = new_instance_length;
            self.max_accent = self.max_accent.max(other.max_accent);
            self.max_descent = self.max_descent.max(other.max_descent);
            self.max_line_gap = self.max_line_gap.max(other.max_line_gap);
            self.last_glyph = other.last_glyph;
            self.last_font_id = other.last_font_id;
            self.last_font_size = other.last_font_size;
            self.last_metrics = other.last_metrics;
            self.last_origin_x = new_origin_x;
            for mut glyph_pos in other.glyphs {
                glyph_pos.x += x_offset;
                self.glyphs.push(glyph_pos);
            }
        }

        /// Returns the current width of the buffer.
        pub fn width(&self) -> f32 {
            self.instance_length.max(0.0)
        }

        /// Estimates the width after concatenating `other` without modifying `self`.
        ///
        /// This prediction is used during wrapping decisions to avoid expensive
        /// cloning or re-layout work.
        pub fn projected_concat_length(
            &self,
            other: &LayoutBuffer<T>,
            font_storage: &mut FontStorage,
        ) -> f32 {
            let advance_kerned = if self.last_font_id == other.first_font_id
                && (self.last_font_size - other.first_font_size).abs() < f32::EPSILON
            {
                font_storage
                    .font(self.last_font_id)
                    .and_then(|font| {
                        font.horizontal_kern_indexed(
                            self.last_glyph,
                            other.first_glyph,
                            self.last_font_size,
                        )
                    })
                    .unwrap_or(0.0)
                    + self.last_metrics.advance_width
            } else {
                self.last_metrics.advance_width
            };

            let x_offset = self.last_origin_x + advance_kerned;
            x_offset + other.instance_length
        }

        /// Returns line metrics derived from the buffered glyph fragments.
        pub fn line_metrics(&self) -> (f32, f32, f32) {
            (self.max_accent, self.max_descent, self.max_line_gap)
        }

        /// Builds a layout buffer from a slice of glyph fragments.
        ///
        /// `None` is returned when the slice is empty because there are no
        /// glyphs to measure or position.
        pub fn from_fragments(
            fragments: &[GlyphFragment<T>],
            font_storage: &mut FontStorage,
        ) -> Option<LayoutBuffer<T>> {
            let first = fragments.first()?;
            let mut buffer = LayoutBuffer::new(
                first.glyph_idx,
                &first.metrics,
                &first.line_metrics,
                first.font_id,
                first.font_size,
                first.user_data.clone(),
            );

            for fragment in fragments.iter().skip(1) {
                buffer.push(
                    fragment.glyph_idx,
                    &fragment.metrics,
                    &fragment.line_metrics,
                    fragment.font.as_ref(),
                    fragment.font_id,
                    fragment.font_size,
                    fragment.user_data.clone(),
                    font_storage,
                );
            }

            Some(buffer)
        }
    }
}

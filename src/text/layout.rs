use std::collections::HashSet;

use crate::{glyph_id::GlyphId, text::TextData};

/// Configuration knobs used by the text layout pipeline.
///
/// All parameters are honored during a single `TextData::layout` call so the
/// caller can measure or place text inside arbitrary rectangles.
#[derive(Clone)]
pub struct TextLayoutConfig {
    pub max_width: Option<f32>,
    pub max_height: Option<f32>,
    pub horizontal_align: HorizontalAlign,
    pub vertical_align: VerticalAlign,
    pub line_height_scale: f32,
    pub wrap_style: WrapStyle,
    pub wrap_hard_break: bool,
    pub word_separators: HashSet<char, fxhash::FxBuildHasher>,
    pub linebreak_char: HashSet<char, fxhash::FxBuildHasher>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Horizontal justification applied after each line is assembled.
pub enum HorizontalAlign {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Vertical alignment strategy for the entire block of text.
pub enum VerticalAlign {
    Top,
    Middle,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Wrapping rules that define where line breaks may occur.
pub enum WrapStyle {
    NoWrap,
    WordWrap,
    CharWrap,
}

/// Final layout output produced by [`TextData::layout`].
pub struct TextLayout {
    pub config: TextLayoutConfig,
    pub total_height: f32,
    pub total_width: f32,
    pub lines: Vec<TextLayoutLine>,
}

/// A single row of positioned glyphs in the final layout.
pub struct TextLayoutLine {
    pub line_height: f32,
    pub line_width: f32,
    pub glyphs: Vec<GlyphPosition>,
}

/// **Y-axis goes down**
///
/// Each glyph uses the global coordinates generated during layout so renderers
/// can draw them directly without additional transformations.
pub struct GlyphPosition {
    pub glyph_id: GlyphId,
    pub x: f32,
    pub y: f32,
}

impl TextData {
    pub fn measure(
        &self,
        config: &TextLayoutConfig,
        font_storage: &mut crate::font_storage::FontStorage,
    ) -> [f32; 2] {
        let TextLayoutConfig {
            max_width,
            max_height,
            horizontal_align,
            vertical_align,
            line_height_scale,
            wrap_style,
            wrap_hard_break,
            word_separators,
            linebreak_char: newline_char,
        } = config;

        let mut total_width = 0.0;
        let mut total_height = 0.0;

        for text in &self.texts {
            let Some(font) = font_storage.font(text.font_id) else {
                continue;
            };
            let Some(line_metric) = font.horizontal_line_metrics(text.font_size) else {
                continue;
            };
            if text.content.is_empty() {
                continue;
            }

            todo!()
        }
        [total_width, total_height]
    }

    pub fn layout(
        &self,
        config: &TextLayoutConfig,
        font_storage: &mut crate::font_storage::FontStorage,
    ) -> TextLayout {
        let TextLayoutConfig {
            max_width,
            max_height,
            horizontal_align,
            vertical_align,
            line_height_scale,
            wrap_style,
            wrap_hard_break,
            word_separators,
            linebreak_char,
        } = config;

        let mut word_buf: Option<layout_utl::LayoutBuffer> = None;
        let mut line_buf: Option<layout_utl::LayoutBuffer> = None;
        let mut lines: Vec<Option<layout_utl::LayoutBuffer>> = vec![];

        for text in &self.texts {
            let Some(font) = font_storage.font(text.font_id) else {
                continue;
            };
            let Some(line_metric) = font.horizontal_line_metrics(text.font_size) else {
                continue;
            };
            if text.content.is_empty() {
                continue;
            }

            for ch in text.content.chars() {
                let glyph_idx = font.lookup_glyph_index(ch);
                let metrics = font.metrics_indexed(glyph_idx, text.font_size);

                match ch {
                    c if linebreak_char.contains(&c) => {
                        // 改行
                        // 現在のバッファを確定してさらに改行を入れる
                        // もしフォントに改行グリフが登録されている場合、
                        // 横幅オーバーランは気にせず行末にPush

                        // コードの少なさ、簡潔さを優先する案

                        // 1. 単語の確定処理
                        if let Some(w_buf) = &word_buf {
                            if
                            /* 行バッファがあって、単語を行に入れられるか？ */
                            true {
                                // 単語バッファを行バッファにpush
                            } else {
                                // 行があればlinesにpushして、
                                // 単語バッファを行バッファに移動
                            }
                        }

                        // 2. 行バッファをlinesにpushして終了
                        // pushする前に行末に
                    }
                    c if word_separators.contains(&c) => {
                        // スペース
                        // 単語の確定処理
                        if let Some(w_buf) = &word_buf {
                            if
                            /* 行バッファがあって、単語は行に入れられるか */
                            true {
                                // 単語バッファを行にPush
                            } else {
                                // 行バッファがあれば、それを確定してlinesにPush
                                // 単語バッファを行バッファに移動する
                            }
                        }

                        // 空白の追加処理
                        if
                        /* 行があって、空白がその行に入るか */
                        true {
                            // そのままpush
                        } else {
                            // 行を確定し、linesにPushして
                            // 新しい行バッファに空白をpush
                        }
                    }
                    c => {
                        // 通常の文字追加
                        // カーニングの関係によっては行バッファも見ないといけない

                        match &word_buf {
                            Some(w_buf) => {
                                // 現在行と単語は未確定
                                // 以下、順番に確認
                                // 1.  (行バッファがあれば)行バッファ含めて改行せずに横幅内に入るか
                                //   - 入るなら単語バッファにそのままpush
                                // 2.  行バッファ含めずに単語バッファは横幅内に入るか
                                //   - 単語内で改行しないで済むならその方が良い
                                //   i)  行バッファを確定し、linesにpush
                                //   ii) この時点で行バッファはNone -> 次の単語確定は次の行に。
                                //  iii) 単語バッファに文字をpush
                                // 3.  行バッファ含めないでも単語は横幅に入らない
                                //   - 単語内で無理やり改行。
                                //   i)  現在の単語バッファを行にpush(これが通ることは前回のイテレートで確認済み)
                                //   ii) 新しい行に今の文字から新しく単語バッファを作る
                                //       この時点でline_buf == None, word_buf == Some
                            }
                            None => {
                                // 行バッファだけある
                                // 1. 無条件で新しい単語バッファにPushして終了
                            }
                        }
                    }
                }
            }
        }

        todo!()
    }
}

mod measure_utl {
    pub struct LineBuffer {
        max_ascent: f32,
        max_descent: f32,
        max_line_gap: f32,

        last_char: char,
        instance_length: f32,
        advance_length: f32,
    }

    impl LineBuffer {
        pub fn from_word_buffer(word: &WordBuffer) -> Self {
            Self {
                max_ascent: 0.0,
                max_descent: 0.0,
                max_line_gap: 0.0,
                last_char: word.last_char,
                instance_length: word.instance_length,
                advance_length: word.advance_length,
            }
        }
    }

    //
    pub struct WordBuffer {
        first_char: char,
        last_char: char,
        instance_length: f32,
        advance_length: f32,
    }

    impl WordBuffer {
        pub fn new(first: char, metrics: &fontdue::Metrics) -> Self {
            Self {
                first_char: first,
                last_char: first,
                instance_length: metrics.width as f32 + metrics.xmin as f32,
                advance_length: metrics.advance_width,
            }
        }

        pub fn push(&mut self, char: char, metrics: &fontdue::Metrics) {
            self.last_char = char;
            self.instance_length = self.advance_length + metrics.width as f32 + metrics.xmin as f32;
            self.advance_length += metrics.advance_width;
        }

        pub fn length(&self) -> f32 {
            self.instance_length
        }

        pub fn length_if_pushed(&self, metrics: &fontdue::Metrics) -> f32 {
            self.advance_length + metrics.width as f32 + metrics.xmin as f32
        }
    }
}

mod layout_utl {
    use crate::font_storage::FontStorage;

    use super::*;

    /// Y-Origin at the baseline.
    pub struct LayoutBuffer {
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

        pub glyphs: Vec<GlyphPosition>,
    }

    impl LayoutBuffer {
        pub fn new(
            glyph_idx: u16,
            metrics: &fontdue::Metrics,
            line_metrics: &fontdue::LineMetrics,
            font_id: fontdb::ID,
            font_size: f32,
        ) -> Self {
            Self {
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
                glyphs: vec![GlyphPosition {
                    glyph_id: GlyphId::new(font_id, glyph_idx, font_size),
                    x: metrics.xmin as f32,
                    y: -(metrics.ymin as f32 + metrics.height as f32),
                }],
            }
        }

        pub fn push(
            &mut self,
            glyph_idx: u16,
            metrics: &fontdue::Metrics,
            line_metrics: &fontdue::LineMetrics,
            font: &fontdue::Font,
            font_id: fontdb::ID,
            font_size: f32,
            font_storage: &mut FontStorage,
        ) {
            let advance_kerned =
                self.advance_with_glyph(glyph_idx, font, font_id, font_size, font_storage);

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
            });
        }

        pub fn concat(&mut self, other: LayoutBuffer, font_storage: &mut FontStorage) {
            let advance_kerned = self.advance_with_buffer(&other, font_storage);

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

        pub fn len(&self) -> f32 {
            self.instance_length
        }

        pub fn len_if_pushed(
            &self,
            glyph_idx: u16,
            metrics: &fontdue::Metrics,
            font: &fontdue::Font,
            font_id: fontdb::ID,
            font_size: f32,
            font_storage: &mut FontStorage,
        ) -> f32 {
            let advance_kerned =
                self.advance_with_glyph(glyph_idx, font, font_id, font_size, font_storage);
            let new_origin_x = self.last_origin_x + advance_kerned;
            new_origin_x + metrics.width as f32 + metrics.xmin as f32
        }

        pub fn len_if_concatenated(
            &self,
            other: &LayoutBuffer,
            font_storage: &mut FontStorage,
        ) -> f32 {
            let advance_kerned = self.advance_with_buffer(other, font_storage);
            let x_offset = self.last_origin_x + advance_kerned;
            x_offset + other.instance_length
        }

        fn advance_with_glyph(
            &self,
            glyph_idx: u16,
            font: &fontdue::Font,
            font_id: fontdb::ID,
            font_size: f32,
            _font_storage: &mut FontStorage,
        ) -> f32 {
            if self.last_font_id == font_id && (self.last_font_size - font_size).abs() < f32::EPSILON
            {
                let kerning = font
                    .horizontal_kern_indexed(self.last_glyph, glyph_idx, font_size)
                    .unwrap_or(0.0);
                self.last_metrics.advance_width + kerning
            } else {
                self.last_metrics.advance_width
            }
        }

        fn advance_with_buffer(
            &self,
            other: &LayoutBuffer,
            font_storage: &mut FontStorage,
        ) -> f32 {
            if self.last_font_id == other.first_font_id
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
                self.last_metrics.advance_width
            }
        }
    }
}

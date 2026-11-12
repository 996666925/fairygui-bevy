//! Plain bitmap text layout following LayaAir 3.3 Text/BitmapFont.
//! Coordinates include FairyGUI's two-pixel label padding; glyph steps round
//! independently, while glyph offsets and dimensions retain fractional scale.
use crate::{
    geom::{Rect, Size},
    package::BitmapFontData,
    widget::TextFieldData,
};

#[path = "bitmap_rich_text.rs"]
mod rich;
pub use rich::{
    BitmapElement, BitmapElementKind, BitmapRichLayout, BitmapRichQuad, BitmapRichRun,
    layout_bitmap_rich_text,
};

#[derive(Debug, Clone, PartialEq)]
pub struct BitmapTextGlyph {
    pub char_code: u32,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BitmapTextLayout {
    pub glyphs: Vec<BitmapTextGlyph>,
    /// Native underline stroke bounds, including alignment and label padding.
    pub underlines: Vec<Rect>,
    /// Measured text including native padding (independent of alignment).
    pub text_size: Size,
    /// Result of Both/Height auto-size; other modes retain the supplied size.
    pub object_size: Size,
    pub font_scale: f64,
}

/// Bounds of a horizontal Text.renderText stroke, before native GPU rasterization.
/// A strikethrough extends four pixels at each end and truncates its center y.
pub fn bitmap_decoration_rect(
    x: f64,
    width: f64,
    line_y: f64,
    line_height: f64,
    font_size: f64,
    strike: bool,
) -> Rect {
    let thickness = (font_size / 16.0).max(1.0);
    let (x, y, width) = if strike {
        (
            x - 4.0,
            (line_y + line_height / 2.0 - thickness).trunc(),
            width + 8.0,
        )
    } else {
        (x, line_y + line_height - thickness, width)
    };
    Rect::new(
        x as f32,
        (y - thickness / 2.0) as f32,
        width as f32,
        thickness as f32,
    )
}

#[derive(Debug)]
struct Line {
    text: Vec<u16>,
    width: f64,
    y: f64,
    height: f64,
}
fn round(v: f64) -> f64 {
    (v + 0.5).floor()
}
fn advance(font: &BitmapFontData, code: u16, size: f64) -> f64 {
    let scale = if font.auto_scale_size {
        size / f64::from(font.font_size)
    } else {
        1.0
    };
    font.glyph(u32::from(code))
        .map_or(0.0, |g| round(f64::from(g.advance) * scale))
}
fn measure(font: &BitmapFontData, text: &[u16], size: f64) -> f64 {
    text.iter().map(|&c| advance(font, c, size)).sum()
}
fn alphanumeric(c: u16) -> bool {
    c <= 127 && (c as u8).is_ascii_alphanumeric()
}
fn is_word(c: u16) -> bool {
    alphanumeric(c)
        || (b'!' as u16..=b'+' as u16).contains(&c)
        || c == b'/' as u16
        || c == b'_' as u16
}
fn is_punctuation(c: u16) -> bool {
    [
        46, 44, 65292, 12290, 12289, 33, 65281, 65307, 59, 8221, 8217, 41, 65289, 93, 12305, 125,
        12299,
    ]
    .contains(&c)
}

fn wrap(
    font: &BitmapFontData,
    text: &[u16],
    size: f64,
    width: f64,
    no_break_word: bool,
) -> Vec<Vec<u16>> {
    if measure(font, text, size) <= width {
        return vec![text.to_vec()];
    }
    let mut lines = Vec::new();
    let mut start = 0;
    let mut j = 0;
    let mut word_width = 0.0;
    while j < text.len() {
        let code = text[j];
        let tw = advance(font, code, size);
        word_width += tw;
        if word_width <= width || j == start {
            j += 1;
            continue;
        }
        let mut end = j;
        if no_break_word && (alphanumeric(code) || is_punctuation(code)) {
            let part = &text[start..end];
            let boundary = if part.is_empty() {
                Some(0)
            } else if part.last().is_some_and(|&c| is_word(c)) {
                Some(part.iter().rposition(|&c| !is_word(c)).map_or(0, |i| i + 1))
            } else {
                None
            };
            if let Some(boundary) = boundary.filter(|&v| v > 0) {
                if boundary as isize > part.len() as isize - 20 {
                    end = start + boundary;
                }
            } else if boundary.is_none() && is_punctuation(code) && end > start + 1 {
                end -= 1;
            }
        }
        if end > start {
            lines.push(text[start..end].to_vec());
        }
        start = end;
        j = end;
        word_width = 0.0;
    }
    lines.push(text[start..].to_vec());
    lines
}

#[allow(clippy::too_many_arguments)]
fn run(
    font: &BitmapFontData,
    text: &[u16],
    size: f64,
    scale: f64,
    width: Option<f64>,
    word_wrap: bool,
    no_break: bool,
    leading: i32,
) -> Vec<Line> {
    let scaled_size = (size * scale).floor().max(1.0);
    let height = if font.auto_scale_size {
        round(f64::from(font.line_height) * scaled_size / f64::from(font.font_size))
    } else {
        f64::from(font.line_height)
    } + 1.0;
    let mut lines = Vec::new();
    let mut y = 0.0;
    for raw in text.split(|&c| c == 10) {
        let parts = if word_wrap && !raw.is_empty() {
            wrap(
                font,
                raw,
                scaled_size,
                width.unwrap_or(f64::MAX).max(0.0),
                no_break,
            )
        } else {
            vec![raw.to_vec()]
        };
        for part in parts {
            lines.push(Line {
                width: measure(font, &part, scaled_size),
                text: part,
                y,
                height,
            });
            y += height + (f64::from(leading) * scale).floor();
        }
    }
    lines
}
fn metrics(lines: &[Line]) -> (f64, f64) {
    let width = lines.iter().map(|l| l.width).fold(0.0, f64::max);
    let height = lines.last().map_or(0.0, |l| l.y + l.height);
    (
        width + if width > 0.0 { 4.0 } else { 0.0 },
        height + if height > 0.0 { 4.0 } else { 0.0 },
    )
}

/// Plain (non-UBB/non-HTML) bitmap text, including native wrapping, alignment,
/// Both/Height, Shrink and Ellipsis. Missing UTF-16 glyphs have zero advance.
pub fn layout_bitmap_text(
    font: &BitmapFontData,
    data: &TextFieldData,
    text: &str,
    size: Size,
) -> BitmapTextLayout {
    let width_set = data.auto_size != 1;
    let height_set = data.auto_size != 1 && data.auto_size != 2;
    let width = if width_set {
        Some(f64::from(size.width) - 4.0)
    } else {
        None
    };
    let height = if height_set {
        f64::from(size.height) - 4.0
    } else {
        f64::MAX
    };
    let font_size = f64::from(data.font_size);
    let mut scale = 1.0;
    let word_wrap = !data.single_line && data.auto_size != 1;
    let text: Vec<_> = text.encode_utf16().collect();
    let mut lines = if text.is_empty() {
        Vec::new()
    } else {
        run(
            font,
            &text,
            font_size,
            scale,
            width,
            word_wrap || data.auto_size == 4,
            word_wrap,
            data.leading,
        )
    };
    let (mut tw, mut th) = metrics(&lines);
    let available = width.unwrap_or(f64::MAX);
    if data.auto_size == 3 && !lines.is_empty() {
        if lines.len() > 1 && th > height {
            let (mut low, mut high) = (0.0, font_size);
            scale = (height / th).max(0.0).sqrt();
            let mut current = (scale * font_size).floor();
            // Native can loop indefinitely for a non-scaling font in a box
            // smaller than its fixed line height. Bound that degenerate case.
            for _ in 0..128 {
                lines = run(
                    font,
                    &text,
                    font_size,
                    scale,
                    width,
                    word_wrap,
                    word_wrap,
                    data.leading,
                );
                (tw, th) = metrics(&lines);
                if tw > available || th > height {
                    high = current;
                } else {
                    low = current;
                }
                if high - low > 1.0 || (high != low && current == high) {
                    current = low + (high - low) / 2.0;
                    scale = if font_size == 0.0 {
                        0.0
                    } else {
                        current / font_size
                    };
                } else {
                    break;
                }
            }
        } else if tw > available {
            scale = (available / tw).max(0.0);
            lines = run(
                font,
                &text,
                font_size,
                scale,
                width,
                word_wrap,
                word_wrap,
                data.leading,
            );
            (tw, th) = metrics(&lines);
            if tw > available {
                scale = if font_size == 0.0 {
                    0.0
                } else {
                    ((font_size * scale).floor() - 1.0) / font_size
                };
                lines = run(
                    font,
                    &text,
                    font_size,
                    scale,
                    width,
                    word_wrap,
                    word_wrap,
                    data.leading,
                );
                (tw, th) = metrics(&lines);
            }
        }
    } else if data.auto_size == 4
        && (tw > available || th > height || (!word_wrap && lines.len() > 1))
    {
        let count = if !word_wrap && lines.len() > 1 {
            Some(1)
        } else {
            lines
                .iter()
                .position(|l| l.y + l.height > height)
                .map(|i| i.max(1))
        };
        let deleted = count.is_some_and(|n| lines.len() > n);
        if deleted {
            lines.truncate(count.unwrap());
        }
        if let Some(last) = lines.last_mut().filter(|l| !l.text.is_empty())
            && (deleted || last.width > available - 10.0)
        {
            let space = last.width - available;
            let remove = if space < 5.0 {
                2
            } else if space < 10.0 {
                1
            } else {
                0
            };
            for _ in 0..remove {
                if last.text.len() > 1 {
                    if matches!(last.text.last(), Some(0xDC00..=0xDFFF)) {
                        last.text.pop();
                    }
                    last.text.pop();
                }
            }
            last.text.push(8230);
            last.width = measure(font, &last.text, (font_size * scale).floor().max(1.0));
        }
        (tw, th) = metrics(&lines);
    }
    let object_size = Size::new(
        if data.auto_size == 1 {
            tw as f32
        } else {
            size.width
        },
        if data.auto_size == 1 || data.auto_size == 2 {
            th as f32
        } else {
            size.height
        },
    );
    let vertical = if height_set && th < f64::from(object_size.height) {
        match data.vertical_align {
            1 => ((f64::from(object_size.height) - th) * 0.5).floor(),
            2 => f64::from(object_size.height) - th,
            _ => 0.0,
        }
    } else {
        0.0
    };
    let draw_scale = ((if font.auto_scale_size {
        font_size
    } else {
        f64::from(font.font_size)
    }) * scale)
        .floor()
        / f64::from(font.font_size);
    let mut glyphs = Vec::new();
    let mut underlines = Vec::new();
    for line in &lines {
        let available = f64::from(object_size.width) - 4.0;
        let align = if width_set {
            match data.align {
                1 => ((available - line.width) * 0.5).floor().max(0.0),
                2 => (available - line.width).max(0.0),
                _ => 0.0,
            }
        } else {
            0.0
        };
        let mut x = 2.0 + align;
        if data.underline && line.width > 0.0 {
            underlines.push(bitmap_decoration_rect(
                x,
                line.width,
                2.0 + vertical + line.y,
                line.height,
                (font_size * scale).floor().max(1.0),
                false,
            ));
        }
        for &code in &line.text {
            if let Some(g) = font.glyph(u32::from(code)) {
                if g.texture.is_some() {
                    glyphs.push(BitmapTextGlyph {
                        char_code: u32::from(code),
                        x: (x + f64::from(g.x) * draw_scale) as f32,
                        y: (2.0 + vertical + line.y + f64::from(g.y) * draw_scale) as f32,
                        width: (f64::from(g.width) * draw_scale) as f32,
                        height: (f64::from(g.height) * draw_scale) as f32,
                    });
                }
                x += round(f64::from(g.advance) * draw_scale);
            }
        }
    }
    BitmapTextLayout {
        glyphs,
        underlines,
        text_size: Size::new(tw as f32, th as f32),
        object_size,
        font_scale: scale,
    }
}

#[cfg(test)]
#[path = "bitmap_text_tests.rs"]
mod tests;

//! 位图字体渲染。
//!
//! Bevy 的文本系统不支持位图字体，这里按 FairyGUI 的做法**逐字形渲染**：
//! 每个字形是一个带子区域图片的子节点，位置由字形自身的偏移与步进决定。
//! 这样既复用了既有的图集切片能力，也与编辑器的排版结果保持一致。

use fairygui_core::package::{BitmapFontData, GlyphTexture};

/// 一个待放置的字形。
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedGlyph {
    /// 是否使用文本颜色；彩色位图字体保留纹理原色。
    pub tint: bool,
    /// 相对文本原点的横向位置。
    pub x: f32,
    /// 相对文本原点的纵向位置。
    pub y: f32,
    /// 字形宽度。
    pub width: f32,
    /// 字形高度。
    pub height: f32,
    /// 纹理来源。
    pub texture: Option<GlyphTexture>,
}

/// 用位图字体排版一行文本。
///
/// 没有图（例如空格）的字形不会被绘制，但它的步进仍然计入，
/// 否则后续字符的位置会错。
pub fn layout_line(font: &BitmapFontData, text: &str) -> Vec<PlacedGlyph> {
    let mut cursor = 0.0f32;
    let mut glyphs = Vec::new();

    for ch in text.chars() {
        let Some(glyph) = font.glyph(ch as u32) else {
            // 字体里没有这个字，按默认步进占位。
            cursor += font.x_advance;
            continue;
        };

        if glyph.texture.is_some() && glyph.width > 0.0 && glyph.height > 0.0 {
            glyphs.push(PlacedGlyph {
                tint: font.tint,
                x: cursor + glyph.x,
                y: glyph.y,
                width: glyph.width,
                height: glyph.height,
                texture: glyph.texture.clone(),
            });
        }

        cursor += glyph.advance;
    }

    glyphs
}

/// 这个字符是否属于东亚文字。
///
/// 东亚文字几乎每个字之间都可以断行，西文则必须断在空格处，两者规则不同。
fn is_cjk(ch: char) -> bool {
    matches!(
        ch as u32,
        // 谚文字母、部首扩展、假名、汉字、谚文音节、兼容汉字、全角形式、扩展区
        0x1100..=0x11FF
            | 0x2E80..=0x9FFF
            | 0xAC00..=0xD7AF
            | 0xF900..=0xFAFF
            | 0xFF00..=0xFF60
            | 0x20000..=0x2FA1F
    )
}

/// 把一行切成若干「可整体挪到下一行」的片段。
///
/// 西文以空格为界，且空格归在前一段末尾——这样断行时空格留在行尾，符合
/// 阅读习惯。东亚文字逐字成段，与中文排版一致。
///
/// 完整的 Unicode 断行算法（连字符、避头尾、禁则处理）没有实现，这里只处理
/// 最常见的两类；混排文本按字符自身性质决定，不会因为整段是西文就漏判。
fn split_breakable(line: &str) -> Vec<String> {
    let mut pieces = Vec::new();
    let mut current = String::new();

    for ch in line.chars() {
        current.push(ch);

        if ch == ' ' || is_cjk(ch) {
            pieces.push(std::mem::take(&mut current));
        }
    }

    if !current.is_empty() {
        pieces.push(current);
    }
    pieces
}

/// 量出一段文本的宽度。字体里缺某个字时按默认步进计。
fn measure(font: &BitmapFontData, text: &str) -> f32 {
    text.chars()
        .map(|ch| font.glyph(ch as u32).map_or(font.x_advance, |g| g.advance))
        .sum()
}

/// 用位图字体排版文本，超过宽度时换行。
///
/// `max_width` 小于等于 0 时不换行。断行先按片段尝试整体挪行：西文以空格为界，
/// 因此不会把单词断成两半；东亚文字逐字成段，与中文排版一致。单个片段自己
/// 就超过整行宽度时（超长单词）只能按字符硬断。
pub fn layout_text(font: &BitmapFontData, text: &str, max_width: f32) -> Vec<PlacedGlyph> {
    // 宽度为 0 表示不自动换行，但显式换行依然要生效。
    let wrap = max_width > 0.0;

    let line_height = font.line_height.max(1) as f32;
    let mut placed = Vec::new();
    let mut line_offset = 0.0f32;

    for line in text.split('\n') {
        let mut cursor = 0.0f32;
        let mut line_index = 0.0f32;

        for piece in split_breakable(line) {
            // 这一片整体放不下时先换行——这是「西文不断词」的关键一步。
            if wrap && cursor > 0.0 && cursor + measure(font, &piece) > max_width {
                line_index += 1.0;
                cursor = 0.0;
            }

            for ch in piece.chars() {
                let Some(glyph) = font.glyph(ch as u32) else {
                    cursor += font.x_advance;
                    continue;
                };

                // 单片超宽时的兜底：按字符硬断，总比溢出行外好。
                if wrap && cursor + glyph.advance > max_width && cursor > 0.0 {
                    line_index += 1.0;
                    cursor = 0.0;
                }

                if glyph.texture.is_some() && glyph.width > 0.0 && glyph.height > 0.0 {
                    placed.push(PlacedGlyph {
                        tint: font.tint,
                        x: cursor + glyph.x,
                        y: glyph.y + line_offset + line_index * line_height,
                        width: glyph.width,
                        height: glyph.height,
                        texture: glyph.texture.clone(),
                    });
                }

                cursor += glyph.advance;
            }
        }

        line_offset += (line_index + 1.0) * line_height;
    }

    placed
}

/// 富文本的一段。
#[derive(Debug, Clone, PartialEq)]
pub enum RichSegment {
    /// 文本段。
    Text {
        text: String,
        color: Option<fairygui_core::color::Color>,
        size: Option<i32>,
        bold: bool,
        italic: bool,
        underline: bool,
        strikethrough: bool,
        stroke: Option<i32>,
        stroke_color: Option<fairygui_core::color::Color>,
        url: Option<String>,
        link_id: Option<usize>,
    },
    /// 内联图片段。`source` 是资源地址或条目名。
    Image {
        source: String,
        dimensions: fairygui_core::text::ImageDimensions,
        url: Option<String>,
        link_id: Option<usize>,
    },
}

/// 把一段 UBB 文本拆成富文本段。
///
/// 保留文本样式、链接地址和图片资源地址供 Bevy 段落使用。
pub fn layout_ubb(text: &str) -> Vec<RichSegment> {
    segments(fairygui_core::text::parse_ubb(text))
}

pub(crate) fn parse_for(
    data: &fairygui_core::widget::TextFieldData,
    text: &str,
) -> fairygui_core::text::UbbDocument {
    use fairygui_core::text::{MarkupOptions, MarkupStyle, parse_rich_text};
    let color = |c: fairygui_core::color::Color| {
        let argb = c.to_argb();
        format!("#{:06X}{:02X}", argb & 0xffffff, argb >> 24)
    };
    let options = MarkupOptions {
        style: MarkupStyle {
            font_size: data.font_size,
            color: color(data.color),
            bold: data.bold,
            italic: data.italic,
            underline: data.underline,
            stroke: data.stroke as i32,
            stroke_color: color(
                data.stroke_color
                    .unwrap_or(fairygui_core::color::Color::new(0.0, 0.0, 0.0, 1.0)),
            ),
            ..Default::default()
        },
        ..Default::default()
    };
    parse_rich_text(text, data.ubb_enabled, &options)
}
pub(crate) fn layout_for(
    data: &fairygui_core::widget::TextFieldData,
    text: &str,
) -> Vec<RichSegment> {
    segments(parse_for(data, text))
}
fn segments(document: fairygui_core::text::UbbDocument) -> Vec<RichSegment> {
    document
        .spans
        .into_iter()
        .map(|span| match span.image {
            Some(source) => RichSegment::Image {
                source,
                dimensions: span.image_dimensions,
                url: span.url,
                link_id: span.link_id,
            },
            None => RichSegment::Text {
                text: span.text,
                color: span.color,
                size: span.size,
                bold: span.bold,
                italic: span.italic,
                underline: span.underline,
                strikethrough: span.strikethrough,
                stroke: span.stroke,
                stroke_color: span.stroke_color,
                url: span.url,
                link_id: span.link_id,
            },
        })
        .collect()
}

/// 从资源地址里取出条目名。
///
/// 支持 `ui://包名/条目名` 与裸条目名两种写法。
pub fn segment_source_name(source: &str) -> &str {
    source.rsplit('/').next().unwrap_or(source)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fairygui_core::geom::Rect;

    fn font_with_glyphs(chars: &[(u32, f32, f32, f32)]) -> BitmapFontData {
        let mut font = BitmapFontData {
            ttf: false,
            tint: false,
            auto_scale_size: false,
            font_size: 20,
            x_advance: 10.0,
            line_height: 20,
            glyphs: Default::default(),
        };

        for (code, x, width, advance) in chars {
            font.glyphs.insert(
                *code,
                fairygui_core::package::BmGlyph {
                    char_code: *code,
                    x: *x,
                    y: 0.0,
                    width: *width,
                    height: 16.0,
                    advance: *advance,
                    texture: Some(GlyphTexture::FontAtlas {
                        atlas_item: 0,
                        rect: Rect::new(0.0, 0.0, *width, 16.0),
                    }),
                },
            );
        }

        font
    }

    #[test]
    fn glyphs_advance_by_their_own_width() {
        let font =
            font_with_glyphs(&[(b'a' as u32, 0.0, 8.0, 10.0), (b'b' as u32, 0.0, 8.0, 12.0)]);
        let glyphs = layout_line(&font, "ab");

        assert_eq!(glyphs.len(), 2);
        assert_eq!(glyphs[0].x, 0.0);
        assert_eq!(glyphs[1].x, 10.0, "第二个字形应当按第一个的步进排布");
    }

    #[test]
    fn glyph_offset_shifts_inside_its_cell() {
        let font = font_with_glyphs(&[(b'a' as u32, 2.0, 8.0, 12.0)]);
        let glyphs = layout_line(&font, "a");
        assert_eq!(glyphs[0].x, 2.0, "字形自身的横向偏移应当计入");
    }

    #[test]
    fn western_text_breaks_at_spaces() {
        let pieces = split_breakable("hello world");
        assert_eq!(
            pieces,
            vec!["hello ".to_string(), "world".to_string()],
            "空格应当归在前一段末尾——断行时空格留在行尾"
        );
    }

    #[test]
    fn cjk_text_breaks_per_character() {
        let pieces = split_breakable("你好世界");
        assert_eq!(pieces.len(), 4, "汉字逐字成段，与中文排版一致");
        assert!(pieces.iter().all(|piece| piece.chars().count() == 1));
    }

    #[test]
    fn mixed_text_handles_both_rules() {
        let pieces = split_breakable("你好 world");
        assert_eq!(pieces.len(), 4, "两个汉字、一个空格、一个单词，各成一段");
        assert_eq!(pieces.last().map(String::as_str), Some("world"));
    }

    #[test]
    fn missing_characters_still_advance() {
        let font =
            font_with_glyphs(&[(b'a' as u32, 0.0, 8.0, 10.0), (b'b' as u32, 0.0, 8.0, 10.0)]);
        let glyphs = layout_line(&font, "a?b");

        // 问号缺字不会被绘制，但它占用的宽度不能丢。
        assert_eq!(glyphs.len(), 2);
        assert_eq!(glyphs[1].x, 20.0);
    }

    #[test]
    fn wrapping_starts_a_new_line() {
        let font = font_with_glyphs(&[(b'a' as u32, 0.0, 8.0, 10.0)]);
        // 宽度只够放两个字符。
        let glyphs = layout_text(&font, "aaaa", 20.0);

        assert_eq!(glyphs.len(), 4);
        assert_eq!(glyphs[0].y, 0.0);
        assert_eq!(glyphs[1].y, 0.0);
        assert_eq!(glyphs[2].y, 20.0, "第三个应当换到第二行");
    }

    #[test]
    fn explicit_newlines_are_respected() {
        let font = font_with_glyphs(&[(b'a' as u32, 0.0, 8.0, 10.0)]);
        let glyphs = layout_text(&font, "a\na", 0.0);

        assert_eq!(glyphs.len(), 2);
        assert_eq!(glyphs[1].y, 20.0);
    }

    #[test]
    fn zero_width_disables_wrapping() {
        let font = font_with_glyphs(&[(b'a' as u32, 0.0, 8.0, 10.0)]);
        let glyphs = layout_text(&font, "aaaa", 0.0);
        assert!(glyphs.iter().all(|glyph| glyph.y == 0.0));
    }

    #[test]
    fn glyphs_without_texture_are_not_drawn() {
        let mut font = font_with_glyphs(&[(b'a' as u32, 0.0, 8.0, 10.0)]);
        font.glyphs.get_mut(&(b'a' as u32)).unwrap().texture = None;

        let glyphs = layout_line(&font, "a");
        assert!(glyphs.is_empty(), "没有纹理的字形不应产生绘制项");
    }

    #[test]
    fn rich_segments_keep_all_ubb_text_styles() {
        let segments = layout_ubb("[size=30][b][i][u]styled[/u][/i][/b][/size]");
        let [
            RichSegment::Text {
                text,
                size,
                bold,
                italic,
                underline,
                ..
            },
        ] = segments.as_slice()
        else {
            panic!("expected one styled text segment");
        };

        assert_eq!(text, "styled");
        assert_eq!(*size, Some(30));
        assert!(*bold && *italic && *underline);
    }
}

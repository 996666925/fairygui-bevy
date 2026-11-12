//! UBB 富文本标记解析。
//!
//! 语法形如 `[color=#ff0000]红字[/color]`，标签成对出现、可以嵌套。
//! 解析结果是一串「文本段」，每段自带样式；渲染层再把它们映射成
//! 引擎的富文本能力。
//!
//! UBB 先转为 HTML，再经过原版样式栈；未知方括号标签保留为正文。

use crate::color::Color;

/// 一个带样式的文本段。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UbbSpan {
    /// 文本内容。
    pub text: String,
    /// 文字颜色。
    pub color: Option<Color>,
    /// 字号。
    pub size: Option<i32>,
    /// 加粗。
    pub bold: bool,
    /// 斜体。
    pub italic: bool,
    /// 下划线。
    pub underline: bool,
    pub strikethrough: bool,
    pub stroke: Option<i32>,
    pub stroke_color: Option<Color>,
    /// 链接地址。
    pub url: Option<String>,
    /// Opening tag offset; distinguishes adjacent links with the same href.
    pub link_id: Option<usize>,
    /// 内联图片地址。非空时该段的 `text` 无意义。
    pub image: Option<String>,
    /// External HtmlImage axes. FairyGUI's ui:// override ignores these.
    pub image_dimensions: super::ImageDimensions,
}

impl UbbSpan {
    /// 是否为一张内联图片。
    pub fn is_image(&self) -> bool {
        self.image.is_some()
    }
}

/// 解析结果。
#[derive(Debug, Clone, Default)]
pub struct UbbDocument {
    /// 按出现顺序排列的文本段。
    pub spans: Vec<UbbSpan>,
}

impl UbbDocument {
    /// 纯文本内容（去掉全部样式）。
    pub fn plain_text(&self) -> String {
        self.spans
            .iter()
            .filter(|span| !span.is_image())
            .map(|span| span.text.as_str())
            .collect()
    }

    /// 是否没有任何内容。
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }
}

/// Parse UBB through the native HTML style stack. Link identities refer to
/// opening elements, preserving adjacent tags even when their hrefs match.
pub fn parse_ubb(input: &str) -> UbbDocument {
    parse_rich_text(input, true, &super::MarkupOptions::default())
}

pub fn parse_rich_text(input: &str, ubb: bool, options: &super::MarkupOptions) -> UbbDocument {
    use super::markup::{MarkupKind, markup_color, parse_markup};
    let document = parse_markup(input, ubb, options);
    let mut spans = Vec::new();
    let mut link = None;
    for (i, element) in document.elements.into_iter().enumerate() {
        match element.kind {
            MarkupKind::Link => {
                link = Some((
                    i,
                    element.attributes.get("href").cloned().unwrap_or_default(),
                ))
            }
            MarkupKind::LinkEnd => link = None,
            MarkupKind::Text | MarkupKind::Image => {
                let image = element.kind == MarkupKind::Image;
                let style = element.style;
                spans.push(UbbSpan {
                    text: element.text,
                    color: (!image && (style.color_changed || style.color != options.style.color))
                        .then(|| markup_color(&style.color)),
                    size: (!image && style.font_size != options.style.font_size)
                        .then_some(style.font_size),
                    bold: style.bold,
                    italic: style.italic,
                    underline: style.underline,
                    strikethrough: style.strikethrough,
                    stroke: (!image && style.stroke != options.style.stroke)
                        .then_some(style.stroke),
                    stroke_color: (!image && style.stroke_color != options.style.stroke_color)
                        .then(|| markup_color(&style.stroke_color)),
                    url: link.as_ref().map(|(_, href)| href.clone()),
                    link_id: link.as_ref().map(|(id, _)| *id),
                    image: image
                        .then(|| element.attributes.get("src").cloned().unwrap_or_default()),
                    image_dimensions: super::ImageDimensions::from_attributes(&element.attributes),
                });
            }
            _ => {}
        }
    }
    UbbDocument { spans }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_becomes_a_single_span() {
        let document = parse_ubb("你好");
        assert_eq!(document.spans.len(), 1);
        assert_eq!(document.plain_text(), "你好");
        assert!(document.spans[0].color.is_none());
    }

    #[test]
    fn color_tag_applies_and_restores() {
        let document = parse_ubb("a[color=#ff0000]b[/color]c");
        assert_eq!(document.spans.len(), 3);

        assert_eq!(document.spans[0].text, "a");
        assert!(document.spans[0].color.is_none());

        assert_eq!(document.spans[1].text, "b");
        assert_eq!(
            document.spans[1].color,
            Some(Color::from_rgba8(255, 0, 0, 255))
        );

        assert_eq!(document.spans[2].text, "c");
        assert!(
            document.spans[2].color.is_none(),
            "闭合标签之后应当恢复原样式"
        );
    }

    #[test]
    fn short_hex_expands() {
        let document = parse_ubb("[color=#f00]x[/color]");
        assert_eq!(
            document.spans[0].color,
            Some(Color::from_rgba8(255, 0, 0, 255))
        );
    }

    #[test]
    fn eight_digit_hex_reads_alpha_last() {
        let document = parse_ubb("[color=#ff000080]x[/color]");
        let color = document.spans[0].color.expect("color");
        assert!((color.a - 128.0 / 255.0).abs() < 1e-3);
        assert_eq!(color.r, 1.0);
    }

    #[test]
    fn nested_tags_stack() {
        let document = parse_ubb("[b][i]both[/i]bold[/b]plain");
        assert_eq!(document.spans.len(), 3);

        assert!(document.spans[0].bold && document.spans[0].italic);
        assert!(document.spans[1].bold && !document.spans[1].italic);
        assert!(!document.spans[2].bold && !document.spans[2].italic);
    }

    #[test]
    fn size_and_style_tags_are_recognised() {
        let document = parse_ubb("[size=24][u][i]x[/i][/u][/size]");
        let span = &document.spans[0];
        assert_eq!(span.size, Some(24));
        assert!(span.underline);
        assert!(span.italic);
    }

    #[test]
    fn url_tag_carries_the_address() {
        let document = parse_ubb("[url=https://example.com]点击[/url]");
        assert_eq!(document.spans[0].text, "点击");
        assert_eq!(
            document.spans[0].url.as_deref(),
            Some("https://example.com")
        );
    }

    #[test]
    fn unknown_tags_are_kept_as_text() {
        let document = parse_ubb("第[1]章");
        assert_eq!(
            document.plain_text(),
            "第[1]章",
            "无法识别的内容应当原样保留，不能被吞掉"
        );
    }

    #[test]
    fn unclosed_bracket_is_kept() {
        let document = parse_ubb("a[b");
        assert_eq!(document.plain_text(), "a[b");
    }

    #[test]
    fn empty_input_yields_empty_document() {
        let document = parse_ubb("");
        assert!(document.is_empty());
        assert_eq!(document.plain_text(), "");
    }

    #[test]
    fn closing_without_opening_is_ignored() {
        let document = parse_ubb("[/color]x");
        assert_eq!(document.plain_text(), "x");
        assert!(document.spans[0].color.is_none());
    }

    #[test]
    fn mismatched_closing_pops_the_native_shared_style_stack() {
        let document = parse_ubb("[b]a[/color]b[/b]");
        assert!(document.spans[0].bold);
        assert!(!document.spans[1].bold);
        assert_eq!(document.plain_text(), "ab");
    }

    #[test]
    fn image_tag_produces_its_own_span() {
        let document = parse_ubb("a[img]icon.png[/img]b");

        assert_eq!(document.spans.len(), 3, "图片应当自成一个段");
        assert_eq!(document.spans[0].text, "a");
        assert_eq!(document.spans[1].image.as_deref(), Some("icon.png"));
        assert!(document.spans[1].is_image());
        assert_eq!(document.spans[2].text, "b");

        // 纯文本会跳过图片段。
        assert_eq!(document.plain_text(), "ab");
    }

    #[test]
    fn unclosed_image_tag_is_removed_without_consuming_its_following_text() {
        let document = parse_ubb("a[img]icon.png");
        assert_eq!(document.plain_text(), "aicon.png");
    }
}

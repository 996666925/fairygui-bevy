//! FairyGUI/Laya's UBB-to-HTML pipeline and HTML style stack.
use std::collections::BTreeMap;

#[cfg(test)]
#[path = "markup_tests.rs"]
mod tests;

#[derive(Debug, Clone, PartialEq)]
pub struct MarkupStyle {
    pub font_size: i32,
    pub color: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub stroke: i32,
    pub stroke_color: String,
    pub align: Option<String>,
    pub color_changed: bool,
}
impl Default for MarkupStyle {
    fn default() -> Self {
        Self {
            font_size: 12,
            color: "#000000".into(),
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            stroke: 0,
            stroke_color: "#000000".into(),
            align: Some("left".into()),
            color_changed: false,
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct MarkupOptions {
    pub style: MarkupStyle,
    pub link_underline: bool,
    pub link_color: Option<String>,
    pub ignore_whitespace: bool,
    pub default_image_width: i32,
    pub default_image_height: i32,
}
impl Default for MarkupOptions {
    fn default() -> Self {
        Self {
            style: MarkupStyle::default(),
            link_underline: true,
            link_color: None,
            ignore_whitespace: false,
            default_image_width: 0,
            default_image_height: 0,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkupKind {
    Text,
    Link,
    Image,
    Input,
    Select,
    LinkEnd,
}
#[derive(Debug, Clone, PartialEq)]
pub struct MarkupElement {
    pub kind: MarkupKind,
    pub text: String,
    pub style: MarkupStyle,
    pub attributes: BTreeMap<String, String>,
    pub items: Vec<String>,
    pub values: Vec<String>,
}
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MarkupDocument {
    pub elements: Vec<MarkupElement>,
}

/// HtmlImage treats each explicitly supplied axis independently. -1 is the
/// native automatic-size sentinel, including the default for missing/invalid attrs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImageDimensions {
    pub width: Option<i32>,
    pub height: Option<i32>,
}
impl ImageDimensions {
    pub fn from_attributes(attrs: &BTreeMap<String, String>) -> Self {
        let axis = |name| match attribute_int(attrs, name, -1) {
            -1 => None,
            v => Some(v),
        };
        Self {
            width: axis("width"),
            height: axis("height"),
        }
    }
    pub fn resolve(self, natural: Option<crate::geom::Size>) -> crate::geom::Size {
        let natural = natural.unwrap_or(crate::geom::Size::ZERO);
        crate::geom::Size::new(
            self.width.map_or(natural.width, |v| v as f32),
            self.height.map_or(natural.height, |v| v as f32),
        )
    }
}

fn tag_text(text: &str, start: usize) -> Option<(String, usize)> {
    let mut from = start;
    let mut result = String::new();
    while let Some(offset) = text[from..].find('[') {
        let at = from + offset;
        if at > 0 && text.as_bytes()[at - 1] == b'\\' {
            result.push_str(&text[from..at - 1]);
            result.push('[');
            from = at + 1;
        } else {
            result.push_str(&text[from..at]);
            return Some((result, at));
        }
    }
    None
}
/// Execute native built-in UBB replacements. Unknown tags remain literal;
/// recognized tags are case-insensitive, but their names are not whitespace-trimmed.
pub fn ubb_to_html(text: &str, options: &MarkupOptions) -> String {
    replace_ubb(text, options, false)
}

/// Native UBBParser.parse(text, true). Recognized tags are removed without
/// executing handlers, so image source text remains and lastColor is unset.
pub fn strip_ubb_tags(text: &str) -> String {
    replace_ubb(text, &MarkupOptions::default(), true)
}

fn replace_ubb(text: &str, options: &MarkupOptions, remove: bool) -> String {
    let mut from = 0;
    let mut result = String::new();
    while let Some(offset) = text[from..].find('[') {
        let at = from + offset;
        if at > 0 && text.as_bytes()[at - 1] == b'\\' {
            result.push_str(&text[from..at - 1]);
            result.push('[');
            from = at + 1;
            continue;
        }
        result.push_str(&text[from..at]);
        from = at;
        let Some(end) = text[at..].find(']').map(|p| p + at) else {
            break;
        };
        let closing = text.as_bytes().get(at + 1) == Some(&b'/');
        let raw = &text[at + if closing { 2 } else { 1 }..end];
        let (name, attr) = raw
            .split_once('=')
            .map_or((raw, None), |(n, a)| (n, Some(a)));
        let name = name.to_lowercase();
        if remove
            && matches!(
                name.as_str(),
                "url"
                    | "img"
                    | "b"
                    | "i"
                    | "u"
                    | "sup"
                    | "sub"
                    | "color"
                    | "font"
                    | "size"
                    | "stroke"
            )
        {
            from = end + 1;
            continue;
        }
        let value = attr.unwrap_or("null");
        let mut next = end + 1;
        let replacement = match name.as_str() {
            "url" => Some(if closing {
                "</a>".into()
            } else {
                let href = attr
                    .map(str::to_owned)
                    .or_else(|| tag_text(text, next).map(|t| t.0))
                    .unwrap_or_else(|| "null".into());
                format!("<a href=\"{href}\">")
            }),
            "img" => {
                if closing {
                    Some(String::new())
                } else if let Some((src, at)) = tag_text(text, next).filter(|(s, _)| !s.is_empty())
                {
                    next = at;
                    Some(if options.default_image_width != 0 {
                        format!(
                            "<img src=\"{src}\" width=\"{}\" height=\"{}\"/>",
                            options.default_image_width, options.default_image_height
                        )
                    } else {
                        format!("<img src=\"{src}\"/>")
                    })
                } else {
                    Some(String::new())
                }
            }
            "b" | "i" | "u" | "sup" | "sub" => {
                Some(format!("<{}{name}>", if closing { "/" } else { "" }))
            }
            "color" | "size" | "font" => Some(if closing {
                "</font>".into()
            } else {
                let attr_name = if name == "font" { "face" } else { &name };
                format!("<font {attr_name}=\"{value}\">")
            }),
            "stroke" => Some(if closing {
                "</font>".into()
            } else if let Some(attr) = attr {
                let mut p = attr.split(',');
                let width = p.next().unwrap();
                let color = p
                    .next()
                    .filter(|s| !s.is_empty())
                    .map(|s| format!(" strokeColor=\"{s}\""))
                    .unwrap_or_default();
                format!("<font stroke=\"{width}\"{color}>")
            } else {
                "<font stroke=\"1\">".into()
            }),
            _ => None,
        };
        if let Some(replacement) = replacement {
            result.push_str(&replacement);
        } else {
            result.push_str(&text[at..next]);
        }
        from = next;
    }
    result.push_str(&text[from..]);
    result
}

fn js_int(value: &str) -> Option<i32> {
    let s = value.trim_start();
    let (sign, s) = if let Some(s) = s.strip_prefix('-') {
        (-1i64, s)
    } else {
        (1, s.strip_prefix('+').unwrap_or(s))
    };
    let (radix, s) = if s.starts_with("0x") || s.starts_with("0X") {
        (16, &s[2..])
    } else {
        (10, s)
    };
    let digits: String = s.chars().take_while(|c| c.is_digit(radix)).collect();
    if digits.is_empty() {
        None
    } else {
        i64::from_str_radix(&digits, radix).ok().map(|v| {
            v.saturating_mul(sign)
                .clamp(i32::MIN as i64, i32::MAX as i64) as i32
        })
    }
}
fn attribute_int(attrs: &BTreeMap<String, String>, name: &str, default: i32) -> i32 {
    attrs
        .get(name)
        .and_then(|s| {
            js_int(s).map(|v| {
                if s.ends_with('%') {
                    (f64::from(v) / 100.0 * f64::from(default)).ceil() as i32
                } else {
                    v
                }
            })
        })
        .unwrap_or(default)
}
/// XMLUtils.decodeString uses UTF-16 fromCharCode for numeric entities, and a
/// ten-code-unit search window. Rust strings replace unpaired units on output.
pub fn decode_entities(text: &str) -> String {
    let src: Vec<u16> = text.encode_utf16().collect();
    let mut result = Vec::new();
    let mut i = 0;
    while i < src.len() {
        if src[i] != 38 {
            result.push(src[i]);
            i += 1;
            continue;
        }
        let end = (i + 11).min(src.len());
        let stop = (i + 1..end).find(|&p| src[p] == 59);
        if let Some(stop) = stop.filter(|&p| p > i + 1) {
            let entity = String::from_utf16_lossy(&src[i + 1..stop]);
            let decoded = if let Some(n) = entity.strip_prefix('#').filter(|s| !s.is_empty()) {
                let number = if let Some(hex) = n.strip_prefix('x') {
                    let digits: String =
                        hex.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
                    i64::from_str_radix(&digits, 16).ok().map(|n| n as i32)
                } else {
                    js_int(n)
                };
                Some(number.unwrap_or(0) as u16)
            } else {
                match entity.as_str() {
                    "amp" => Some(38),
                    "apos" => Some(39),
                    "gt" => Some(62),
                    "lt" => Some(60),
                    "nbsp" => Some(32),
                    "quot" => Some(34),
                    _ => None,
                }
            };
            if let Some(c) = decoded {
                result.push(c);
                i = stop + 1;
                continue;
            }
        }
        result.push(38);
        i += 1;
    }
    String::from_utf16_lossy(&result)
}
fn space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 11)
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum TagKind {
    Start,
    End,
    Void,
    Other,
}
struct Tag<'a> {
    name: String,
    kind: TagKind,
    body: &'a str,
    gap: &'a str,
}
struct Scanner<'a> {
    text: &'a str,
    pos: usize,
}
impl<'a> Scanner<'a> {
    fn next(&mut self) -> Option<Tag<'a>> {
        let start = self.pos;
        let at = self.text[start..].find('<')? + start;
        let bytes = self.text.as_bytes();
        let mut p = at + 1;
        if p == bytes.len() {
            return None;
        }
        for (open, close) in [("<![CDATA[", "]]>"), ("<!--", "-->")] {
            if self.text[at..].starts_with(open) {
                let end = self.text[at + open.len()..]
                    .find(close)
                    .map_or(bytes.len(), |n| at + open.len() + n + close.len());
                self.pos = end;
                return Some(Tag {
                    name: String::new(),
                    kind: TagKind::Other,
                    body: &self.text[at..end],
                    gap: &self.text[start..at],
                });
            }
        }
        let kind = match bytes[p] {
            b'/' => {
                p += 1;
                TagKind::End
            }
            b'!' | b'?' => {
                p += 1;
                TagKind::Other
            }
            _ => TagKind::Start,
        };
        let name_start = at + 1 + usize::from(kind == TagKind::End);
        while p < bytes.len() && !space(bytes[p]) && !matches!(bytes[p], b'>' | b'/') {
            p += 1;
        }
        if p == bytes.len() {
            return None;
        }
        let name = self.text[name_start..p].to_lowercase();
        let mut single = false;
        let mut double = false;
        let mut possible = None;
        while p < bytes.len() {
            let c = bytes[p];
            if c == b'"' && !single {
                double = !double;
            } else if c == b'\'' && !double {
                single = !single;
            }
            if c == b'>' {
                if !single && !double {
                    possible = None;
                    break;
                }
                possible = Some(p);
            } else if c == b'<' {
                break;
            }
            p += 1;
        }
        if let Some(end) = possible {
            p = end;
        }
        if p == bytes.len() {
            return None;
        }
        let kind = if bytes[p.saturating_sub(1)] == b'/' {
            TagKind::Void
        } else {
            kind
        };
        self.pos = p + 1;
        Some(Tag {
            name,
            kind,
            body: &self.text[at..p + 1],
            gap: &self.text[start..at],
        })
    }
}
fn attributes(text: &str) -> BTreeMap<String, String> {
    let b = text.as_bytes();
    let mut p = 0;
    while p < b.len() && !space(b[p]) && !matches!(b[p], b'>' | b'/') {
        p += 1;
    }
    let mut attrs = BTreeMap::new();
    while p < b.len() {
        while p < b.len() && (space(b[p]) || matches!(b[p], b'/' | b'>')) {
            p += 1;
        }
        let start = p;
        while p < b.len() && !space(b[p]) && !matches!(b[p], b'=' | b'/' | b'>') {
            p += 1;
        }
        if start == p {
            break;
        }
        let name = text[start..p].to_lowercase();
        while p < b.len() && space(b[p]) {
            p += 1;
        }
        if b.get(p) != Some(&b'=') {
            attrs.insert(name, String::new());
            continue;
        }
        p += 1;
        while p < b.len() && space(b[p]) {
            p += 1;
        }
        let quote = b.get(p).copied().filter(|c| matches!(c, b'\'' | b'"'));
        if quote.is_some() {
            p += 1;
        }
        let start = p;
        while p < b.len()
            && if let Some(quote) = quote {
                b[p] != quote
            } else {
                !space(b[p]) && b[p] != b'>'
            }
        {
            p += 1;
        }
        if p == b.len() {
            break;
        }
        attrs.insert(name, decode_entities(&text[start..p]));
        if quote.is_some() {
            p += 1;
        }
    }
    attrs
}
fn element(kind: MarkupKind, style: MarkupStyle, attrs: BTreeMap<String, String>) -> MarkupElement {
    MarkupElement {
        kind,
        text: String::new(),
        style,
        attributes: attrs,
        items: Vec::new(),
        values: Vec::new(),
    }
}
fn append(out: &mut Vec<MarkupElement>, text: String, style: &MarkupStyle) {
    if let Some(previous) = out
        .last_mut()
        .filter(|e| e.kind == MarkupKind::Text && e.style == *style)
    {
        previous.text.push_str(&text);
    } else {
        let mut e = element(MarkupKind::Text, style.clone(), BTreeMap::new());
        e.text = text;
        out.push(e);
    }
}
fn newline(out: &[MarkupElement]) -> bool {
    out.last()
        .is_none_or(|e| e.kind == MarkupKind::Text && e.text.ends_with('\n'))
}
fn decoded_gap(gap: &str, trim: bool) -> String {
    if trim {
        decode_entities(gap.trim_start_matches([' ', '\t', '\n', '\r', '\u{b}']))
            .trim_end()
            .to_owned()
    } else {
        decode_entities(gap)
    }
}
/// Parse the native HTML subset consumed by Laya Text. Unknown tags do not draw;
/// form/input/select elements remain explicit metadata for host implementations.
pub fn parse_html(text: &str, options: &MarkupOptions) -> MarkupDocument {
    let mut scan = Scanner { text, pos: 0 };
    let mut out = Vec::new();
    let mut style = options.style.clone();
    style.color_changed = false;
    let mut stack = Vec::new();
    let mut skip_text = 0;
    let mut ignore = options.ignore_whitespace;
    let mut skip_cr = false;
    while let Some(tag) = scan.next() {
        if skip_text == 0 {
            let mut gap = decoded_gap(tag.gap, ignore);
            if !gap.is_empty() {
                if skip_cr && gap.starts_with('\n') {
                    gap.remove(0);
                }
                append(&mut out, gap, &style);
            }
        }
        skip_cr = false;
        match tag.name.as_str() {
            "b" | "i" | "u" | "strike" => {
                if tag.kind == TagKind::Start {
                    stack.push(style.clone());
                    match tag.name.as_str() {
                        "b" => style.bold = true,
                        "i" => style.italic = true,
                        "u" => style.underline = true,
                        _ => style.strikethrough = true,
                    };
                } else if let Some(s) = stack.pop() {
                    style = s;
                }
            }
            "font" => {
                if tag.kind == TagKind::Start {
                    stack.push(style.clone());
                    let attrs = attributes(tag.body);
                    style.font_size = attribute_int(&attrs, "size", style.font_size);
                    if let Some(c) = attrs.get("color") {
                        style.color = c.clone();
                        style.color_changed = true;
                    }
                    if let Some(w) = attrs.get("stroke") {
                        style.stroke = js_int(w).unwrap_or(0);
                    }
                    if let Some(c) = attrs.get("strokecolor") {
                        style.stroke_color = c.clone();
                    }
                } else if tag.kind == TagKind::End
                    && let Some(s) = stack.pop()
                {
                    style = s;
                }
            }
            "br" => append(&mut out, "\n".into(), &style),
            "img" if matches!(tag.kind, TagKind::Start | TagKind::Void) => {
                let s = MarkupStyle {
                    align: style.align.clone(),
                    underline: style.underline,
                    ..Default::default()
                };
                out.push(element(MarkupKind::Image, s, attributes(tag.body)));
            }
            "a" => {
                if tag.kind == TagKind::Start {
                    stack.push(style.clone());
                    style.underline |= options.link_underline;
                    if !style.color_changed
                        && let Some(c) = &options.link_color
                    {
                        style.color = c.clone();
                    }
                    let s = MarkupStyle {
                        align: style.align.clone(),
                        ..Default::default()
                    };
                    out.push(element(MarkupKind::Link, s, attributes(tag.body)));
                } else if tag.kind == TagKind::End {
                    if let Some(s) = stack.pop() {
                        style = s;
                    }
                    out.push(element(
                        MarkupKind::LinkEnd,
                        MarkupStyle::default(),
                        BTreeMap::new(),
                    ));
                }
            }
            "input" => out.push(element(
                MarkupKind::Input,
                style.clone(),
                attributes(tag.body),
            )),
            "select" if matches!(tag.kind, TagKind::Start | TagKind::Void) => {
                let mut e = element(MarkupKind::Select, style.clone(), attributes(tag.body));
                if tag.kind == TagKind::Start {
                    while let Some(option) = scan.next() {
                        if option.name == "select" {
                            break;
                        }
                        if option.name == "option" {
                            if matches!(option.kind, TagKind::Start | TagKind::Void) {
                                e.values.push(
                                    attributes(option.body).remove("value").unwrap_or_default(),
                                );
                            } else {
                                e.items.push(decoded_gap(option.gap, false));
                            }
                        }
                    }
                }
                out.push(e);
            }
            "p" => {
                if tag.kind == TagKind::Start {
                    stack.push(style.clone());
                    style.align = attributes(tag.body).remove("align");
                    if !newline(&out) {
                        append(&mut out, "\n".into(), &style);
                    }
                } else if tag.kind == TagKind::End {
                    append(&mut out, "\n".into(), &style);
                    skip_cr = true;
                    if let Some(s) = stack.pop() {
                        style = s;
                    }
                }
            }
            "ui" | "div" | "li" => {
                if tag.kind == TagKind::Start {
                    if !newline(&out) {
                        append(&mut out, "\n".into(), &style);
                    }
                } else {
                    append(&mut out, "\n".into(), &style);
                    skip_cr = true;
                }
            }
            "html" | "body" => ignore = true,
            "head" | "style" | "script" | "form" => {
                if tag.kind == TagKind::Start {
                    skip_text += 1;
                } else if tag.kind == TagKind::End {
                    skip_text -= 1;
                }
            }
            _ => {}
        }
    }
    if skip_text == 0 {
        let mut tail = decoded_gap(&text[scan.pos..], ignore);
        if !tail.is_empty() {
            if skip_cr && tail.starts_with('\n') {
                tail.remove(0);
            }
            append(&mut out, tail, &style);
        }
    }
    MarkupDocument { elements: out }
}
pub fn parse_markup(text: &str, ubb: bool, options: &MarkupOptions) -> MarkupDocument {
    if ubb {
        parse_html(&ubb_to_html(text, options), options)
    } else {
        parse_html(text, options)
    }
}

/// Laya 3.3 ColorUtils/Color string channels, including native RGBA hex order.
pub fn markup_color(value: &str) -> crate::color::Color {
    use crate::color::Color;
    if value == "none" {
        return Color::TRANSPARENT;
    }
    if value.contains("rgba(") || value.contains("rgb(") {
        let Some((_, rest)) = value.split_once('(') else {
            return Color::new(0.0, 0.0, 0.0, 1.0);
        };
        let Some((body, _)) = rest.split_once(')') else {
            return Color::new(0.0, 0.0, 0.0, 1.0);
        };
        let float = |s: &str| {
            let s = s.trim_start();
            let prefix: String = s
                .chars()
                .take_while(|c| c.is_ascii_digit() || matches!(c, '.' | '+' | '-' | 'e' | 'E'))
                .collect();
            prefix.parse::<f32>().unwrap_or(0.0)
        };
        let channels: Vec<_> = body.split(',').map(float).collect();
        return Color::new(
            channels.first().copied().unwrap_or(f32::NAN) / 255.0,
            channels.get(1).copied().unwrap_or(f32::NAN) / 255.0,
            channels.get(2).copied().unwrap_or(f32::NAN) / 255.0,
            if channels.len() == 4 {
                channels[3]
            } else {
                1.0
            },
        );
    }
    let named = match value {
        "purple" => Some(0x800080),
        "orange" => Some(0xFFA500),
        "white" => Some(0xFFFFFF),
        "red" => Some(0xFF0000),
        "green" => Some(0x00FF00),
        "blue" => Some(0x0000FF),
        "yellow" => Some(0xFFFF00),
        "gray" => Some(0x808080),
        _ => None,
    };
    if let Some(rgb) = named {
        return Color::from_rgba8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, 255);
    }
    let hex = value.strip_prefix('#').unwrap_or(value);
    let len = hex.len();
    let hex = if matches!(len, 3 | 4) {
        hex.chars().flat_map(|c| [c, c]).collect::<String>()
    } else {
        hex.into()
    };
    let parse = |s: &str| {
        let s = s.trim_start();
        let digits: String = s.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
        u64::from_str_radix(&digits, 16).ok()
    };
    let (rgb, alpha) = if len == 8 && hex.is_ascii() {
        (
            parse(&hex[..6]).unwrap_or(0),
            parse(&hex[6..]).map_or(f32::NAN, |a| a as f32 / 255.0),
        )
    } else {
        (parse(&hex).unwrap_or(0), 1.0)
    };
    Color::new(
        ((rgb >> 16) & 255) as f32 / 255.0,
        ((rgb >> 8) & 255) as f32 / 255.0,
        (rgb & 255) as f32 / 255.0,
        alpha,
    )
}

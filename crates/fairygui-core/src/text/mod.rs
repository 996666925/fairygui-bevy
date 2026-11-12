//! 文本处理。
//!
//! 这里放与文本内容相关的解析：UBB 转换、HTML/XML 标记、样式与实体。渲染层再把解析结果映射成
//! 引擎的富文本能力。

pub mod font;
pub mod input;
pub mod markup;
pub use font::is_font_resource;
pub mod ubb;
pub use markup::{
    ImageDimensions, MarkupDocument, MarkupElement, MarkupKind, MarkupOptions, MarkupStyle,
    decode_entities, parse_html, parse_markup, strip_ubb_tags, ubb_to_html,
};

pub use ubb::{UbbDocument, UbbSpan, parse_rich_text, parse_ubb};
#[cfg(test)]
mod password_tests;
#[cfg(test)]
mod prompt_tests;
#[cfg(test)]
mod stroke_tests;

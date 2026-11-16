//! Writable per-field HTML parsing policy for ordinary and bitmap rich text.
use crate::{GuiRuntime, core::object::GObjectId, properties::Property};
use bevy::prelude::*;

/// Attach to a text view, compound title control, or root/window owner.
/// Changes reparse the paragraph in the same frame. Plain text is unaffected.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct GuiHtmlOptions {
    /// Adds an underline to links. Explicit `<u>` or field underlining survives false.
    pub link_underline: bool,
    /// Native color syntax (for example `#FF000080` or `red`). None inherits.
    /// Explicit `<font color>` takes precedence over this link default.
    pub link_color: Option<String>,
    /// Use the native parser's whitespace handling; this is not CSS collapsing.
    pub ignore_whitespace: bool,
}

impl Default for GuiHtmlOptions {
    fn default() -> Self {
        Self::from(crate::core::text::HtmlParseOptions::default())
    }
}

impl From<crate::core::text::HtmlParseOptions> for GuiHtmlOptions {
    fn from(options: crate::core::text::HtmlParseOptions) -> Self {
        Self {
            link_underline: options.link_underline,
            link_color: options.link_color,
            ignore_whitespace: options.ignore_whitespace,
        }
    }
}

impl From<GuiHtmlOptions> for crate::core::text::HtmlParseOptions {
    fn from(options: GuiHtmlOptions) -> Self {
        Self {
            link_underline: options.link_underline,
            link_color: options.link_color,
            ignore_whitespace: options.ignore_whitespace,
        }
    }
}

impl Property for GuiHtmlOptions {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        Some(
            runtime
                .store()
                .object_text_field(object)?
                .html_options
                .clone()
                .into(),
        )
    }

    fn apply(&self, previous: &Self, native: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        let mut next = native.clone();
        if self.link_underline != previous.link_underline {
            next.link_underline = self.link_underline;
        }
        if self.link_color != previous.link_color {
            next.link_color.clone_from(&self.link_color);
        }
        if self.ignore_whitespace != previous.ignore_whitespace {
            next.ignore_whitespace = self.ignore_whitespace;
        }
        runtime
            .store_mut()
            .set_object_html_options(object, next.into());
    }
}

#[cfg(test)]
#[path = "ecs_html_options_tests.rs"]
mod tests;

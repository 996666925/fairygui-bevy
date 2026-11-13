//! Writable text appearance and layout, independent of the committed value.
use bevy::prelude::*;
use fairygui_core::{color::Color as NativeColor, object::GObjectId};

use crate::{properties::Property, runtime::GuiRuntime};

/// Edit directly on a text/input view, a compound control's view, or a root
/// owner. Buttons, labels and combo boxes forward to their resolved title.
/// Unedited fields follow controller/transition changes. Colors use Bevy's
/// color conversion to sRGB; `GuiDisplay::grayed` remains a presentation effect.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct GuiTextFormat {
    /// None selects the configured default font. Supports system families,
    /// external font assets and native package bitmap-font names/URLs.
    pub font: Option<String>,
    pub font_size: i32,
    pub color: Color,
    /// 0 left, 1 center, 2 right.
    pub align: i32,
    /// 0 top, 1 middle, 2 bottom.
    pub vertical_align: i32,
    pub leading: i32,
    pub letter_spacing: i32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub single_line: bool,
    /// 0 fixed, 1 both, 2 height, 3 shrink, 4 ellipsis.
    pub auto_size: i32,
    pub ubb_enabled: bool,
    pub html_enabled: bool,
    pub stroke: f32,
    pub stroke_color: Color,
}

impl Default for GuiTextFormat {
    fn default() -> Self {
        Self {
            font: None,
            font_size: 12,
            color: Color::BLACK,
            align: 0,
            vertical_align: 0,
            leading: 0,
            letter_spacing: 0,
            bold: false,
            italic: false,
            underline: false,
            single_line: false,
            auto_size: 0,
            ubb_enabled: false,
            html_enabled: false,
            stroke: 0.,
            stroke_color: Color::BLACK,
        }
    }
}

fn native_color(color: Color) -> Option<NativeColor> {
    let color = color.to_srgba();
    [color.red, color.green, color.blue, color.alpha]
        .into_iter()
        .all(f32::is_finite)
        .then_some(NativeColor::new(
            color.red,
            color.green,
            color.blue,
            color.alpha,
        ))
}

impl Property for GuiTextFormat {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        let field = runtime.store().object_text_field(object)?;
        let color = |c: NativeColor| Color::srgba(c.r, c.g, c.b, c.a);
        Some(Self {
            font: field.font.clone(),
            font_size: field.font_size,
            color: color(field.color),
            align: field.align,
            vertical_align: field.vertical_align,
            leading: field.leading,
            letter_spacing: field.letter_spacing,
            bold: field.bold,
            italic: field.italic,
            underline: field.underline,
            single_line: field.single_line,
            auto_size: field.auto_size,
            ubb_enabled: field.ubb_enabled,
            html_enabled: field.html_enabled,
            stroke: field.stroke,
            stroke_color: color(
                field
                    .stroke_color
                    .unwrap_or(NativeColor::from_rgba8(0, 0, 0, 255)),
            ),
        })
    }

    fn apply(&self, previous: &Self, native: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        let store = runtime.store_mut();
        macro_rules! apply {
            ($($field:ident => $method:ident),* $(,)?) => {$(
                if self.$field != previous.$field && self.$field != native.$field {
                    store.$method(object, self.$field);
                }
            )*};
        }
        if self.font != previous.font && self.font != native.font {
            store.set_object_font(object, self.font.as_deref());
        }
        apply! {
            font_size => set_object_font_size,
            align => set_object_text_align,
            vertical_align => set_object_text_vertical_align,
            leading => set_object_text_leading,
            letter_spacing => set_object_letter_spacing,
            bold => set_object_bold,
            italic => set_object_italic,
            underline => set_object_underline,
            single_line => set_object_single_line,
            auto_size => set_object_text_auto_size,
            ubb_enabled => set_object_ubb_enabled,
            html_enabled => set_object_html_enabled,
        }
        if self.color != previous.color
            && self.color != native.color
            && let Some(color) = native_color(self.color)
        {
            store.set_control_color(object, color, None);
        }
        if self.stroke_color != previous.stroke_color
            && self.stroke_color != native.stroke_color
            && let Some(color) = native_color(self.stroke_color)
        {
            store.set_control_outline_color(object, color);
        }
        if self.stroke != previous.stroke && self.stroke != native.stroke {
            store.set_object_stroke(object, self.stroke);
        }
    }
}

#[cfg(test)]
#[path = "ecs_text_format_tests.rs"]
mod tests;

//! Writable native color, image masking and loader layout properties.
use bevy::prelude::*;
use fairygui_core::{object::GObjectId, widget::ControlData};

use crate::{
    FillMethod, backend, properties::Property, runtime::GuiRuntime, text_format::native_color,
};

/// Native Color/TitleColor. Images, movies and loaders tint their own pixels;
/// text and compound titles expose the same color as GuiTextFormat.color;
/// graphs expose fill color. Graying, filters and inherited alpha stay separate.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct GuiColor(pub Color);
impl Default for GuiColor {
    fn default() -> Self {
        Self(Color::WHITE)
    }
}
impl Property for GuiColor {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        runtime
            .store()
            .object_color(object)
            .map(|color| Self(backend::to_bevy_color(color)))
    }
    fn apply(&self, _: &Self, _: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        if let Some(color) = native_color(self.0) {
            runtime.store_mut().set_control_color(object, color, None);
        }
    }
}

/// Native Image/Loader fill mask. This changes painted pixels, preserving the
/// object's layout and input rectangle. Amount keeps JS Number precision and
/// the raw requested value; rendering clamps it to 0..=1.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct GuiImageFill {
    pub method: FillMethod,
    pub origin: u8,
    pub clockwise: bool,
    pub amount: f64,
}
impl Property for GuiImageFill {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        let (method, origin, clockwise, amount) = match runtime.store().object(object)?.control() {
            ControlData::Image(data) => (
                data.fill_method,
                data.fill_origin,
                data.fill_clockwise,
                data.fill_amount,
            ),
            ControlData::Loader(data) => (
                data.fill_method,
                data.fill_origin,
                data.fill_clockwise,
                data.fill_amount,
            ),
            _ => return None,
        };
        Some(Self {
            method: FillMethod::from_code(method),
            origin,
            clockwise,
            amount,
        })
    }
    fn apply(&self, previous: &Self, native: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        let mut next = *native;
        macro_rules! merge {
            ($($field:ident),* $(,)?) => {$(
                if self.$field != previous.$field { next.$field = self.$field; }
            )*};
        }
        merge!(method, origin, clockwise);
        if self.amount != previous.amount && self.amount.is_finite() {
            next.amount = self.amount;
        }
        runtime.store_mut().set_image_fill(
            object,
            next.method as u8,
            next.origin,
            next.clockwise,
            next.amount,
        );
    }
}

/// Native GLoader sizing and error-display configuration. Its URL uses GuiIcon;
/// load results remain in GuiLoaderState. Image masking uses GuiImageFill.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuiLoaderSettings {
    /// 0 none, 1 fit, 2 match height, 3 match width, 4 stretch, 5 cover.
    pub fill: i32,
    /// 0 left, 1 center, 2 right.
    pub align: i32,
    /// 0 top, 1 middle, 2 bottom.
    pub vertical_align: i32,
    pub shrink_only: bool,
    pub auto_size: bool,
    /// Resize component content using its relations instead of scaling it.
    pub use_resize: bool,
    /// Affects subsequent failures, without removing an existing error sign.
    pub show_error_sign: bool,
}
impl Default for GuiLoaderSettings {
    fn default() -> Self {
        Self {
            fill: 0,
            align: 0,
            vertical_align: 0,
            shrink_only: false,
            auto_size: false,
            use_resize: false,
            show_error_sign: true,
        }
    }
}
impl Property for GuiLoaderSettings {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        let ControlData::Loader(data) = runtime.store().object(object)?.control() else {
            return None;
        };
        Some(Self {
            fill: data.fill,
            align: data.align,
            vertical_align: data.vertical_align,
            shrink_only: data.shrink_only,
            auto_size: data.auto_size,
            use_resize: data.use_resize,
            show_error_sign: data.show_error_sign,
        })
    }
    fn apply(&self, previous: &Self, native: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        let mut next = *native;
        macro_rules! merge {
            ($($field:ident),* $(,)?) => {$(
                if self.$field != previous.$field { next.$field = self.$field; }
            )*};
        }
        merge!(
            fill,
            align,
            vertical_align,
            shrink_only,
            auto_size,
            use_resize,
            show_error_sign
        );
        let store = runtime.store_mut();
        store.set_loader_layout(object, next.fill, next.align, next.vertical_align);
        store.set_loader_shrink_only(object, next.shrink_only);
        store.set_loader_auto_size(object, next.auto_size);
        store.set_loader_use_resize(object, next.use_resize);
        store.set_loader_show_error_sign(object, next.show_error_sign);
    }
}

#[cfg(test)]
#[path = "ecs_visual_properties_tests.rs"]
mod tests;

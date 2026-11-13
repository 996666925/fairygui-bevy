//! Writable local blend and color-filter properties.
use crate::{BlendMode, ColorFilterParams, GuiRuntime, properties::Property};
use bevy::prelude::*;
use fairygui_core::object::GObjectId;

/// Local Sprite blend override. `None` inherits the parent, while
/// `Some(BlendMode::Normal)` explicitly stops that inheritance.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GuiBlendMode(pub Option<BlendMode>);

impl Property for GuiBlendMode {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        let object = runtime.store().object(object)?;
        Some(Self(
            object.blend_mode_is_set().then(|| object.blend_mode()),
        ))
    }
    fn apply(&self, _: &Self, _: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        if let Some(mode) = self.0 {
            runtime.store_mut().set_blend_mode(object, mode);
        } else {
            runtime.store_mut().reset_blend_mode(object);
        }
    }
}

/// Stored local filter, independent of temporary grayed rendering. `None`
/// removes the filter; `Some(default())` retains an explicit identity filter
/// and its offscreen isolation boundary. Parameters use native -1..1 units;
/// rendering clamps infinities and treats NaN as zero, preserving raw values.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct GuiColorFilter(pub Option<ColorFilterParams>);

fn number_eq(a: f32, b: f32) -> bool {
    a == b || a.is_nan() && b.is_nan()
}
impl PartialEq for GuiColorFilter {
    fn eq(&self, other: &Self) -> bool {
        match (self.0, other.0) {
            (Some(a), Some(b)) => {
                number_eq(a.brightness, b.brightness)
                    && number_eq(a.contrast, b.contrast)
                    && number_eq(a.saturation, b.saturation)
                    && number_eq(a.hue, b.hue)
            }
            (None, None) => true,
            _ => false,
        }
    }
}
impl Property for GuiColorFilter {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        Some(Self(runtime.store().object(object)?.color_filter()))
    }
    fn apply(&self, previous: &Self, native: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        let mut next = self.0;
        if let (Some(requested), Some(previous), Some(native)) = (&mut next, previous.0, native.0) {
            // Keep concurrent animation/native edits to untouched channels.
            macro_rules! merge {
                ($($field:ident),* $(,)?) => {$(
                    if number_eq(requested.$field, previous.$field) {
                        requested.$field = native.$field;
                    }
                )*};
            }
            merge!(brightness, contrast, saturation, hue);
        }
        if Self(next) != *native {
            runtime.store_mut().set_color_filter(object, next);
        }
    }
}

#[cfg(test)]
#[path = "ecs_effect_properties_tests.rs"]
mod tests;

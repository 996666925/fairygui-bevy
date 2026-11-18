//! Writable component/list margins, in local logical pixels.
use crate::{
    GuiRuntime,
    core::{geom::Margin, object::GObjectId},
    properties::Property,
};
use bevy::prelude::*;

/// Container content margins. Finite negative values are supported. Changing
/// margins updates list/pane layout and soft clipping, without changing outer size.
/// As in FairyGUI, visible overflow retains its content origin until list
/// alignment changes; clipped and scrolling containers update it immediately.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct GuiMargin {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}
impl GuiMargin {
    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }
    pub const fn all(value: f32) -> Self {
        Self::new(value, value, value, value)
    }
}
impl Property for GuiMargin {
    fn read(runtime: &GuiRuntime, id: GObjectId) -> Option<Self> {
        let m = runtime.store().component_margin(id)?;
        Some(Self::new(m.left, m.top, m.right, m.bottom))
    }
    fn apply(&self, previous: &Self, native: &Self, runtime: &mut GuiRuntime, id: GObjectId) {
        let merge = |value: f32, old: f32, current: f32| {
            if value != old && value.is_finite() {
                value
            } else {
                current
            }
        };
        runtime.store_mut().set_component_margin(
            id,
            Margin::new(
                merge(self.left, previous.left, native.left),
                merge(self.top, previous.top, native.top),
                merge(self.right, previous.right, native.right),
                merge(self.bottom, previous.bottom, native.bottom),
            ),
        );
    }
}

#[cfg(test)]
#[path = "ecs_margin_tests.rs"]
mod tests;

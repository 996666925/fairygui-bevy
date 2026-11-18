use crate::{GuiRuntime, core::GObjectId, properties::Property};
use bevy::prelude::*;

/// Writable GComponent viewWidth/viewHeight in local logical pixels. Assigning
/// an axis resizes the outer object by adding its margins and nonfloating bar.
/// Published values reflect size constraints and a pane's minimum-one viewport.
/// Edit one axis to preserve the other axis's retained outer-size request.
/// GuiTransform.size wins if both properties explicitly edit an axis in a pass.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct GuiViewSize(pub Vec2);

impl Property for GuiViewSize {
    fn read(runtime: &GuiRuntime, id: GObjectId) -> Option<Self> {
        let size = runtime.store().component_view_size(id)?;
        Some(Self(Vec2::new(size.width, size.height)))
    }
    fn apply(&self, previous: &Self, _: &Self, runtime: &mut GuiRuntime, id: GObjectId) {
        if self.0.x != previous.0.x {
            runtime.store_mut().set_component_view_width(id, self.0.x);
        }
        if self.0.y != previous.0.y {
            runtime.store_mut().set_component_view_height(id, self.0.y);
        }
    }
}

#[cfg(test)]
#[path = "ecs_view_size_tests.rs"]
mod tests;

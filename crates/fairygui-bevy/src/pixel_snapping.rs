use crate::{GuiRuntime, core::GObjectId, properties::Property};
use bevy::prelude::*;

/// Round the display position in parent-local logical pixels, preserving
/// GuiTransform.position, relations and layout. Defaults to false; DPI scaling
/// occurs after native rounding. Use on a view or a root/window owner.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GuiPixelSnapping(pub bool);

impl Property for GuiPixelSnapping {
    fn read(runtime: &GuiRuntime, id: GObjectId) -> Option<Self> {
        Some(Self(runtime.store().object(id)?.pixel_snapping()))
    }
    fn apply(&self, _: &Self, _: &Self, runtime: &mut GuiRuntime, id: GObjectId) {
        runtime.store_mut().set_pixel_snapping(id, self.0);
    }
}

#[cfg(test)]
#[path = "ecs_pixel_snapping_tests.rs"]
mod tests;

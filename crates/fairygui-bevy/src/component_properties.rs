//! Writable native component input properties.
use crate::{GuiRuntime, properties::Property};
use bevy::prelude::*;
use fairygui_core::object::GObjectId;

/// GComponent.opaque controls blank-rectangle picking, independently of alpha.
/// False lets empty space pass through while children still receive input.
/// Custom pixel/child hit areas keep their native priority and geometry.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GuiOpaque(pub bool);
impl Property for GuiOpaque {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        Some(Self(
            runtime
                .store()
                .object(object)?
                .control()
                .component()?
                .opaque,
        ))
    }
    fn apply(&self, _: &Self, _: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        runtime.store_mut().set_component_opaque(object, self.0);
    }
}

#[cfg(test)]
#[path = "ecs_component_input_tests.rs"]
mod tests;

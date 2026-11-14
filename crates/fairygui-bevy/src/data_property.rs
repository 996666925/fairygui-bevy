//! Writable application data attached to any FairyGUI object.
use crate::{GuiRuntime, core::object::GObjectId, properties::Property};
use bevy::prelude::*;

/// Native GObject.data. `None` removes the optional user payload.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct GuiData(pub Option<String>);

impl Property for GuiData {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        Some(Self(
            runtime.store().object(object)?.data().map(str::to_owned),
        ))
    }

    fn apply(&self, _: &Self, _: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        runtime.store_mut().set_data(object, self.0.clone());
    }
}

#[cfg(test)]
#[path = "ecs_data_tests.rs"]
mod tests;

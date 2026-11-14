//! Writable native hover text, independent of tooltip presentation policy.
use crate::{GuiRuntime, core::object::GObjectId, properties::Property};
use bevy::prelude::*;

/// Optional native GObject tooltip text. `None` clears the exported tooltip.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct GuiTooltips(pub Option<String>);

impl Property for GuiTooltips {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        Some(Self(
            runtime
                .store()
                .object(object)?
                .tooltips()
                .map(str::to_owned),
        ))
    }

    fn apply(&self, _: &Self, _: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        runtime.store_mut().set_tooltips(object, self.0.clone());
    }
}

#[cfg(test)]
#[path = "ecs_tooltips_tests.rs"]
mod tests;

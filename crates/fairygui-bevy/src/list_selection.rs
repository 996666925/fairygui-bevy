//! Writable selection; native input changes share the property baseline.
use crate::{GuiListSelection, GuiRuntime, core::GObjectId, properties::Property};
use bevy::prelude::*;

impl Property for GuiListSelection {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        runtime.store().list_selections(object).map(Self)
    }

    fn ready(world: &World, entity: Entity, object: GObjectId) -> bool {
        let view = world
            .non_send::<GuiRuntime>()
            .entity_of(object)
            .unwrap_or(entity);
        !crate::list::content_pending(world, view, object)
            && !crate::tree::content_pending(world, view, object)
    }

    fn apply(&self, _: &Self, _: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        runtime.store_mut().set_list_selections(object, &self.0);
    }
}

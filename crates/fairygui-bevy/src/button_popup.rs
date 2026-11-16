//! Declarative entity references for GButton.linkedPopup.
use crate::{FairyGuiRoot, FairyGuiWindow, GObjectRef, GuiRuntime, runtime::edit};
use bevy::{ecs::query::QueryState, prelude::*};
use fairygui_core::{object::GObjectId, widget::ControlData};
use std::collections::{HashMap, HashSet};

/// Toggle an existing popup or window on button press, using native placement
/// and popup-stack rules. Attach to a button view or its root/window owner.
/// Targets accept views and current root/window owners. Unready targets wait;
/// template changes rebind automatically. None disables the link. Removing this
/// component clears its managed link. A view component overrides its owner's.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GuiButtonPopup(pub Option<Entity>);

#[derive(Resource)]
struct Bindings {
    query: QueryState<(Entity, &'static GuiButtonPopup)>,
    managed: HashSet<GObjectId>,
}
impl FromWorld for Bindings {
    fn from_world(world: &mut World) -> Self {
        Self {
            query: world.query(),
            managed: HashSet::new(),
        }
    }
}

fn object(world: &World, entity: Entity) -> Option<GObjectId> {
    edit::resolve_object(
        world.non_send::<GuiRuntime>(),
        entity,
        world.get::<GObjectRef>(entity),
        world.get::<FairyGuiRoot>(entity),
        world.get::<FairyGuiWindow>(entity),
    )
}

pub(crate) fn sync(world: &mut World) {
    world.init_resource::<Bindings>();
    world.resource_scope(|world, mut bindings: Mut<Bindings>| {
        let mut desired = HashMap::new();
        for (entity, link) in bindings.query.iter(world) {
            let Some(button) = object(world, entity) else {
                continue;
            };
            let runtime = world.non_send::<GuiRuntime>();
            if !matches!(
                runtime.store().object(button).map(|o| o.control()),
                Some(ControlData::Button(_))
            ) {
                continue;
            }
            let target = link
                .0
                .and_then(|target| object(world, target))
                .filter(|&target| target != button && runtime.root_container() != Some(target));
            let view = world.get::<GObjectRef>(entity).is_some();
            let entry = desired.entry(button).or_insert((view, target));
            if view {
                *entry = (true, target);
            }
        }
        let mut runtime = world.non_send_mut::<GuiRuntime>();
        for &button in &bindings.managed {
            if !desired.contains_key(&button) {
                runtime.store_mut().set_button_linked_popup(button, None);
            }
        }
        bindings.managed.clear();
        for (button, (_, target)) in desired {
            runtime.store_mut().set_button_linked_popup(button, target);
            bindings.managed.insert(button);
        }
    });
}

#[cfg(test)]
#[path = "ecs_button_popup_tests.rs"]
mod tests;

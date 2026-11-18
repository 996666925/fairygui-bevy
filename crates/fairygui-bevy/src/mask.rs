//! Entity-based component masks, separate from their native state snapshots.
use crate::{FairyGuiRoot, FairyGuiWindow, GObjectRef, GuiRuntime, runtime::edit};
use bevy::{ecs::query::QueryState, prelude::*};
use fairygui_core::object::GObjectId;
use std::collections::HashMap;

/// Assign a mask using a view entity or a current root/window owner. Pending
/// sources retain the current mask and retry after loading. Invalid sources
/// leave it unchanged. A view declaration overrides its owner's declaration.
/// Removing a successfully applied declaration clears its managed mask.
/// Unchanged declarations do not overwrite subsequent immediate edits.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GuiMask {
    pub source: Option<Entity>,
    pub reversed: bool,
}

impl GuiMask {
    pub fn new(source: Entity) -> Self {
        Self {
            source: Some(source),
            reversed: false,
        }
    }
    pub fn reversed(source: Entity) -> Self {
        Self {
            source: Some(source),
            reversed: true,
        }
    }
}

/// Actual native mask on a component view, including masks loaded from a FUI.
/// This snapshot never creates a writable declaration or takes over that mask.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
#[component(immutable)]
pub struct GuiMaskState {
    /// Native identity remains assigned when its source has been destroyed.
    pub assigned: bool,
    /// Current source view, or None when absent or no longer materialized.
    pub source: Option<Entity>,
    pub reversed: bool,
}

#[derive(Default)]
struct Binding {
    acknowledged: Option<(Entity, GuiMask, Option<GObjectId>)>,
    applied: bool,
}

#[derive(Resource)]
struct Bindings {
    declarations: QueryState<(Entity, &'static GuiMask)>,
    views: QueryState<(Entity, &'static GObjectRef)>,
    managed: HashMap<GObjectId, Binding>,
}

impl FromWorld for Bindings {
    fn from_world(world: &mut World) -> Self {
        Self {
            declarations: world.query(),
            views: world.query(),
            managed: HashMap::new(),
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
        for (entity, request) in bindings.declarations.iter(world) {
            let Some(owner) = object(world, entity) else {
                continue;
            };
            let runtime = world.non_send::<GuiRuntime>();
            let is_view = world.get::<GObjectRef>(entity).is_some();
            if (is_view && runtime.entity_of(owner) != Some(entity))
                || runtime
                    .store()
                    .object(owner)
                    .and_then(|o| o.control().component())
                    .is_none()
            {
                continue;
            }
            let entry = desired.entry(owner).or_insert((entity, *request));
            if is_view {
                *entry = (entity, *request);
            }
        }
        bindings.managed.retain(|owner, binding| {
            if desired.contains_key(owner) {
                return true;
            }
            if binding.applied {
                world
                    .non_send_mut::<GuiRuntime>()
                    .store_mut()
                    .set_component_mask(*owner, None, false);
            }
            false
        });
        for (owner, (entity, request)) in desired {
            let target = match request.source {
                Some(source) => object(world, source).map(Some),
                None => Some(None),
            };
            let binding = bindings.managed.entry(owner).or_default();
            let Some(target) = target else {
                // A pending transition must not forget a previously applied
                // binding: removing the declaration must still release it.
                binding.acknowledged = None;
                continue;
            };
            let key = (entity, request, target);
            if binding.acknowledged != Some(key) {
                binding.acknowledged = None;
                if world
                    .non_send_mut::<GuiRuntime>()
                    .store_mut()
                    .set_component_mask(owner, target, request.reversed)
                {
                    binding.acknowledged = Some(key);
                    binding.applied = true;
                }
            }
        }
        let runtime = world.non_send::<GuiRuntime>();
        let snapshots: Vec<_> = bindings
            .views
            .iter(world)
            .filter_map(|(entity, reference)| {
                if runtime.entity_of(reference.0) != Some(entity) {
                    return None;
                }
                runtime.store().object(reference.0)?.control().component()?;
                let mask = runtime.store().component_mask(reference.0);
                Some((
                    entity,
                    GuiMaskState {
                        assigned: mask.is_some(),
                        source: mask.and_then(|(source, _)| runtime.entity_of(source)),
                        reversed: mask.is_some_and(|(_, reversed)| reversed),
                    },
                ))
            })
            .collect();
        for (entity, state) in snapshots {
            if world.get::<GuiMaskState>(entity) != Some(&state) {
                world.entity_mut(entity).insert(state);
            }
        }
    });
}

#[cfg(test)]
#[path = "ecs_mask_tests.rs"]
mod tests;

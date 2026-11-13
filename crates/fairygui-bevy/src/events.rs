//! Bevy observer propagation over a captured FairyGUI logical path.
use crate::runtime::{GObjectRef, GuiRuntime};
use bevy::{
    ecs::{
        event::{PropagateEntityTrigger, SetEntityEventTarget},
        query::QueryData,
        traversal::Traversal,
    },
    prelude::*,
};
use fairygui_core::object::{GObjectId, UiEvent};
use std::sync::Arc;

#[cfg(test)]
#[path = "ecs_event_tests.rs"]
mod tests;

pub(crate) struct PendingGuiEvent {
    pub event: UiEvent,
    pub route: Vec<GObjectId>,
}
impl PendingGuiEvent {
    pub fn direct(event: UiEvent) -> Self {
        Self {
            route: vec![event.current_target],
            event,
        }
    }
}

/// FairyGUI notification delivered through Bevy entity observers. Each event
/// follows its captured logical path, respecting native grip/arrow boundaries.
/// `On<GuiEvent>::propagate(false)` stops delivery to subsequent ancestors;
/// other observers on the current entity still run, as in Bevy.
#[derive(EntityEvent, Debug, Clone)]
#[entity_event(trigger = PropagateEntityTrigger<true, Self, GuiEventTraversal>)]
pub struct GuiEvent {
    /// Current recipient, updated together with event.current_target.
    pub entity: Entity,
    /// Original control view, independent of the current recipient. None if
    /// the original native object did not have a view when Publish resolved it.
    pub target: Option<Entity>,
    pub event: UiEvent,
    route: Arc<[(Entity, GObjectId)]>,
}
impl GuiEvent {
    /// Construct a direct notification for one entity. To capture the native
    /// logical bubbling path, enqueue through GuiRuntime::dispatch_event.
    pub fn new(entity: Entity, event: UiEvent) -> Self {
        Self {
            entity,
            target: event.is_at_target().then_some(entity),
            route: Arc::from([(entity, event.current_target)]),
            event,
        }
    }
}
impl SetEntityEventTarget for GuiEvent {
    fn set_event_target(&mut self, entity: Entity) {
        self.entity = entity;
        if let Some((_, object)) = self
            .route
            .iter()
            .find(|(recipient, _)| *recipient == entity)
        {
            self.event.current_target = *object;
            self.event.bubbling = *object != self.event.target;
        }
    }
}

/// Query used by Bevy's propagation trigger. Logical paths live in the event,
/// so decorative render hierarchies and intervening reparenting cannot reroute it.
#[doc(hidden)]
#[derive(QueryData, Debug)]
pub struct GuiEventTraversal {
    entity: Entity,
}
impl Traversal<GuiEvent> for GuiEventTraversal {
    fn traverse(item: Self::Item<'_, '_>, event: &GuiEvent) -> Option<Entity> {
        let index = event
            .route
            .iter()
            .position(|(entity, _)| *entity == item.entity)?;
        event.route.get(index + 1).map(|(entity, _)| *entity)
    }
}

/// Publish one Bevy event per native dispatch, with observer-controlled bubbling.
pub fn emit_gui_events(
    mut commands: Commands,
    mut runtime: NonSendMut<GuiRuntime>,
    entities: Query<(Entity, &GObjectRef)>,
) {
    for pending in runtime.take_gui_events() {
        let resolve = |object| {
            runtime
                .entity_of(object)
                .filter(|&entity| {
                    entities
                        .get(entity)
                        .is_ok_and(|(_, reference)| reference.0 == object)
                })
                .or_else(|| {
                    entities
                        .iter()
                        .filter_map(|(entity, reference)| (reference.0 == object).then_some(entity))
                        .min_by_key(|entity| entity.to_bits())
                })
        };
        let target = resolve(pending.event.target);
        let route: Arc<[(Entity, GObjectId)]> = pending
            .route
            .into_iter()
            .filter_map(|object| resolve(object).map(|entity| (entity, object)))
            .collect();
        let Some(&(entity, current_target)) = route.first() else {
            continue;
        };
        let mut event = pending.event;
        event.current_target = current_target;
        event.bubbling = current_target != event.target;
        commands.trigger(GuiEvent {
            entity,
            target,
            event,
            route,
        });
    }
}

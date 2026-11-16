//! ECS configuration for moving a retained control itself.
use super::*;
use crate::GuiDragSource;

#[cfg(test)]
#[path = "ecs_object_drag_tests.rs"]
mod tests;

/// Enable pointer movement of a control view or root/window owner's view.
/// Owner configuration waits for materialization and follows template replacement;
/// a view's configuration takes precedence. Bounds are logical stage pixels,
/// matching native GObject.dragBounds. Removing the component cancels the gesture.
/// A GuiDragSource on the same control takes precedence and moves its proxy.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct GuiDraggable {
    pub bounds: Option<GuiDragBounds>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GuiDragBounds {
    Stage(fairygui_core::geom::Rect),
    /// A control view or current root/window owner's rectangle, converted to
    /// stage coordinates each frame. Missing/unresolvable bounds disable
    /// movement until ready; invalidation ends an active gesture.
    Control(Entity),
}

/// Read-only motion state on configured owners and their views. Refreshed before Publish and
/// after Sync; false while the pointer is still below its movement threshold.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GuiObjectDragState {
    dragging: bool,
    touch_id: Option<u64>,
}
impl GuiObjectDragState {
    pub fn is_dragging(&self) -> bool {
        self.dragging
    }
    pub fn touch_id(&self) -> Option<u64> {
        self.touch_id
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Binding {
    entity: Entity,
    pub bounds: Option<fairygui_core::geom::Rect>,
}
#[derive(Debug, Clone, Copy)]
pub(crate) struct Active {
    pub touch_id: Option<u64>,
    pub position: Vec2,
}

pub(crate) fn prepare_object_drags(
    sources: Query<(Entity, &GObjectRef), With<GuiDragSource>>,
    configurations: Query<(Entity, &GuiDraggable, Has<GObjectRef>)>,
    resolver: edit::GuiObjectResolver,
    mut runtime: NonSendMut<GuiRuntime>,
) {
    runtime.object_drag_bindings.clear();
    runtime.drag_source_bindings.clear();
    for (entity, reference) in &sources {
        if runtime.entity_of(reference.0) == Some(entity) {
            runtime.drag_source_bindings.insert(reference.0);
        }
    }
    let mut effective = HashMap::new();
    for (entity, draggable, view) in &configurations {
        let Some(object) = resolver.object(&runtime, entity) else {
            continue;
        };
        let Some(target) = runtime.entity_of(object) else {
            continue;
        };
        if view && entity != target {
            continue;
        }
        let entry = effective.entry(object).or_insert((view, target, draggable));
        if view {
            *entry = (true, target, draggable);
        }
    }
    for (object, (_, entity, draggable)) in effective {
        let bounds = match draggable.bounds {
            None => Some(None),
            Some(GuiDragBounds::Stage(rect)) => Some(Some(rect)),
            Some(GuiDragBounds::Control(entity)) => {
                resolver.object(&runtime, entity).and_then(|target| {
                    let object = runtime.store().object(target)?;
                    let rect = runtime.store().local_to_global_rect(
                        target,
                        fairygui_core::geom::Rect::new(0., 0., object.width(), object.height()),
                    )?;
                    let rect = if let Some(stage) = runtime.root_container() {
                        runtime.store().global_to_local_rect(stage, rect)?
                    } else {
                        rect
                    };
                    Some(Some(rect))
                })
            }
        };
        let Some(bounds) = bounds else {
            continue;
        };
        let valid = bounds.is_none_or(|rect| {
            [rect.x, rect.y, rect.width, rect.height]
                .iter()
                .all(|v| v.is_finite())
                && rect.width >= 0.
                && rect.height >= 0.
        });
        if valid {
            runtime
                .object_drag_bindings
                .insert(object, Binding { entity, bounds });
        }
    }
    let invalid: Vec<_> = runtime
        .active_object_drags
        .keys()
        .copied()
        .filter(|&owner| !runtime.object_drag_eligible(owner))
        .collect();
    for owner in invalid {
        runtime.end_object_drag(owner);
    }
}

impl GuiRuntime {
    pub(crate) fn object_drag_eligible(&self, owner: GObjectId) -> bool {
        self.object_drag_bindings
            .get(&owner)
            .is_some_and(|binding| {
                self.entity_of(owner) == Some(binding.entity)
                    && !self.drag_source_bindings.contains(&owner)
                    && crate::input::eligible(self, owner)
            })
    }

    pub(crate) fn object_drag_event(
        &mut self,
        owner: GObjectId,
        kind: fairygui_core::object::EventKind,
        active: Active,
    ) {
        if !self.store.contains(owner) {
            return;
        }
        let mut event = fairygui_core::object::UiEvent::new(kind, owner).with_pointer(
            fairygui_core::Point::new(active.position.x, active.position.y),
        );
        event.touch_id = active.touch_id;
        self.dispatch_event(event);
    }

    pub(crate) fn end_object_drag(&mut self, owner: GObjectId) {
        if let Some(active) = self.active_object_drags.remove(&owner) {
            self.object_drag_event(owner, fairygui_core::object::EventKind::DragEnd, active);
        }
    }
}

type DragStateViews<'w, 's> = Query<
    'w,
    's,
    (Entity, Option<&'static GuiObjectDragState>),
    Or<(
        With<GuiDraggable>,
        With<GObjectRef>,
        With<GuiObjectDragState>,
    )>,
>;

pub(crate) fn sync_object_drag_state(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    configurations: Query<Entity, With<GuiDraggable>>,
    controls: DragStateViews,
    resolver: edit::GuiObjectResolver,
) {
    let configured: std::collections::HashSet<_> = configurations
        .iter()
        .filter_map(|entity| resolver.object(&runtime, entity))
        .collect();
    for (entity, current) in &controls {
        let object = resolver.object(&runtime, entity);
        if object.is_none_or(|object| !configured.contains(&object)) {
            if current.is_some() {
                commands.entity(entity).remove::<GuiObjectDragState>();
            }
            continue;
        }
        let active = runtime.active_object_drags.get(&object.unwrap());
        let next = GuiObjectDragState {
            dragging: active.is_some(),
            touch_id: active.and_then(|active| active.touch_id),
        };
        if current != Some(&next) {
            commands.entity(entity).insert(next);
        }
    }
}

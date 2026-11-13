//! ECS configuration for moving a retained control itself.
use super::*;
use crate::GuiDragSource;

#[cfg(test)]
#[path = "ecs_object_drag_tests.rs"]
mod tests;

/// Enable pointer movement of a control view. Bounds are logical stage pixels,
/// matching native GObject.dragBounds. Removing the component cancels the gesture.
/// A GuiDragSource on the same control takes precedence and moves its proxy.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct GuiDraggable {
    pub bounds: Option<GuiDragBounds>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GuiDragBounds {
    Stage(fairygui_core::geom::Rect),
    /// A control view's rectangle, converted to stage coordinates each frame.
    /// Missing/unresolvable bounds disable movement until the target is ready.
    Control(Entity),
}

/// Read-only motion state on configured controls. Refreshed before Publish and
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
    controls: Query<(
        Entity,
        &GObjectRef,
        Option<&GuiDraggable>,
        Has<GuiDragSource>,
    )>,
    mut runtime: NonSendMut<GuiRuntime>,
) {
    runtime.object_drag_bindings.clear();
    runtime.drag_source_bindings.clear();
    for (entity, reference, draggable, drop_source) in &controls {
        if runtime.entity_of(reference.0) != Some(entity) {
            continue;
        }
        if drop_source {
            runtime.drag_source_bindings.insert(reference.0);
        }
        if let Some(draggable) = draggable {
            let bounds = match draggable.bounds {
                None => Some(None),
                Some(GuiDragBounds::Stage(rect)) => Some(Some(rect)),
                Some(GuiDragBounds::Control(entity)) => {
                    controls.get(entity).ok().and_then(|(_, reference, ..)| {
                        let object = runtime.store().object(reference.0)?;
                        let rect = runtime.store().local_to_global_rect(
                            reference.0,
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
                    .insert(reference.0, Binding { entity, bounds });
            }
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

pub(crate) fn sync_object_drag_state(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    controls: Query<(
        Entity,
        &GObjectRef,
        Option<&GuiDraggable>,
        Option<&GuiObjectDragState>,
    )>,
) {
    for (entity, reference, configured, current) in &controls {
        if runtime.entity_of(reference.0) != Some(entity) {
            continue;
        }
        if configured.is_none() {
            if current.is_some() {
                commands.entity(entity).remove::<GuiObjectDragState>();
            }
            continue;
        }
        let active = runtime.active_object_drags.get(&reference.0);
        let next = GuiObjectDragState {
            dragging: active.is_some(),
            touch_id: active.and_then(|active| active.touch_id),
        };
        if current != Some(&next) {
            commands.entity(entity).insert(next);
        }
    }
}

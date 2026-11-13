//! Entity-owned application drag-and-drop, separate from window/scroll gestures.
use super::*;
use crate::{GuiInputSettings, GuiInputState};
use bevy::window::PrimaryWindow;

#[cfg(test)]
#[path = "ecs_drag_tests.rs"]
mod tests;

/// Attach to a control view to start a drag after the configured pointer threshold.
/// The nearest configured ancestor of the pressed control wins. Removing this
/// component disables future starts; Cancel ends an already active drag.
#[derive(Component, Debug, Default, Clone, PartialEq, Eq)]
pub struct GuiDragSource {
    /// Native 100x100 centered Loader proxy; None starts without a visual proxy.
    pub icon: Option<String>,
    pub data: Option<String>,
}

/// A control view that accepts drops. The nearest marked ancestor of the release
/// hit receives one GuiDrop entity event. Attach an observer on this same entity.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GuiDropTarget;

/// Explicit starts accept a materialized root/window owner or control view.
/// The source must contain the captured left mouse/finger target. Missing targets
/// and pointers already released are discarded; an active drag ignores new starts.
#[derive(Message, Debug, Clone)]
pub enum GuiDragCommand {
    Start {
        source: Entity,
        icon: Option<String>,
        data: Option<String>,
        touch_id: Option<u64>,
    },
    Cancel,
}

/// Target-only application event. Generic GuiEvent Drop notifications still
/// follow the logical bubbling path, with this receiver as the original target.
#[derive(EntityEvent, Debug, Clone)]
pub struct GuiDrop {
    pub entity: Entity,
    pub source: Entity,
    pub data: Option<String>,
    /// Pointer position in stage logical pixels.
    pub position: Vec2,
    pub touch_id: Option<u64>,
}

/// Current ECS drag, refreshed before Publish and at Sync end. Unchanged state
/// preserves change ticks. The proxy is disposed on completion/cancellation.
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct GuiDragState {
    source: Option<Entity>,
    agent: Option<Entity>,
    target: Option<Entity>,
    data: Option<String>,
    position: Option<Vec2>,
    touch_id: Option<u64>,
}
impl GuiDragState {
    pub fn is_dragging(&self) -> bool {
        self.source.is_some()
    }
    pub fn source(&self) -> Option<Entity> {
        self.source
    }
    pub fn agent(&self) -> Option<Entity> {
        self.agent
    }
    pub fn target(&self) -> Option<Entity> {
        self.target
    }
    pub fn data(&self) -> Option<&str> {
        self.data.as_deref()
    }
    pub fn position(&self) -> Option<Vec2> {
        self.position
    }
    pub fn touch_id(&self) -> Option<u64> {
        self.touch_id
    }
}

#[derive(Debug)]
pub(super) struct Session {
    pub source: Entity,
}

impl GuiRuntime {
    pub(super) fn ecs_drop_target(&self, mut target: GObjectId) -> Option<(GObjectId, Entity)> {
        loop {
            if let Some(&entity) = self.drop_targets.get(&target)
                && self.entity_of(target) == Some(entity)
                && crate::input::eligible(self, target)
            {
                return Some((target, entity));
            }
            target = self.store.object(target)?.parent()?;
        }
    }

    fn begin_ecs_drag(
        &mut self,
        input: &mut GuiInputState,
        source: GObjectId,
        configuration: &GuiDragSource,
        touch_id: Option<u64>,
    ) {
        if self.is_dragging() || !crate::input::eligible(self, source) {
            return;
        }
        let Some(entity) = self.entity_of(source) else {
            return;
        };
        let Some((_, pressed, position, _, _)) = input
            .drag_pointers()
            .into_iter()
            .find(|(id, ..)| *id == touch_id)
        else {
            return;
        };
        let mut ancestor = Some(pressed);
        while ancestor.is_some_and(|ancestor| ancestor != source) {
            ancestor = ancestor.and_then(|ancestor| self.store.object(ancestor)?.parent());
        }
        if ancestor.is_none() || !position.is_finite() {
            return;
        }
        let icon = configuration
            .icon
            .as_deref()
            .filter(|url| !url.is_empty())
            .map(|url| {
                let icon = self
                    .store
                    .create(fairygui_core::package::ObjectType::Loader);
                self.store.set_size(icon, 100., 100.);
                self.store.set_pivot(icon, 0.5, 0.5, true);
                self.store.set_loader_layout(icon, 0, 1, 1);
                self.store.set_loader_show_error_sign(icon, false);
                self.store.apply_icon_to(icon, url);
                if let Some(handle) = self.asset_handle_for(source) {
                    if let Some(stage) = self.root_container() {
                        self.object_assets
                            .entry(stage)
                            .or_insert_with(|| handle.clone());
                    }
                    self.object_assets.insert(icon, handle);
                }
                icon
            });
        input.claim_drag_pointer(touch_id, self);
        self.start_drag(
            source,
            icon,
            configuration.data.clone(),
            fairygui_core::Point::new(position.x, position.y),
        );
        self.drag_touch_id = touch_id;
        self.ecs_drag = Some(Session { source: entity });
    }
}

/// Revalidate ownership before routing a release; never let a disposed or
/// blocked source deliver a cached drop on that frame.
pub(crate) fn prepare_drag_targets(
    mut commands: Commands,
    targets: Query<(Entity, &GObjectRef), With<GuiDropTarget>>,
    objects: Query<&GObjectRef>,
    mut runtime: NonSendMut<GuiRuntime>,
) {
    runtime.drop_targets = targets
        .iter()
        .filter_map(|(entity, reference)| {
            (runtime.entity_of(reference.0) == Some(entity)).then_some((reference.0, entity))
        })
        .collect();
    if runtime.ecs_drag.as_ref().is_some_and(|session| {
        objects.get(session.source).is_err()
            || runtime.drag_drop.source().is_none_or(|source| {
                runtime.entity_of(source) != Some(session.source)
                    || !crate::input::eligible(&runtime, source)
            })
    }) {
        runtime.cancel_drag(&mut commands);
    }
}

pub(crate) fn begin_configured_drags(
    sources: Query<&GuiDragSource>,
    mut state: ResMut<GuiInputState>,
    windows: Query<(Entity, &Window, Has<PrimaryWindow>)>,
    settings: Res<GuiInputSettings>,
    mut runtime: NonSendMut<GuiRuntime>,
) {
    if runtime.is_dragging() {
        return;
    }
    for (touch_id, target, _, moved, started) in state.drag_pointers() {
        if !moved || started {
            continue;
        }
        let mut source = Some(target);
        while let Some(object) = source {
            if let Some(entity) = runtime.entity_of(object)
                && let Ok(configuration) = sources.get(entity)
            {
                windows::viewport(&mut runtime, &windows, &settings);
                runtime.begin_ecs_drag(&mut state, object, configuration, touch_id);
                if runtime.is_dragging() {
                    return;
                }
                break;
            }
            source = runtime
                .store
                .object(object)
                .and_then(|object| object.parent());
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_drag_commands(
    mut commands: Commands,
    mut messages: MessageReader<GuiDragCommand>,
    objects: Query<&GObjectRef>,
    roots: Query<&GuiRootInstance>,
    mut state: ResMut<GuiInputState>,
    windows: Query<(Entity, &Window, Has<PrimaryWindow>)>,
    settings: Res<GuiInputSettings>,
    mut runtime: NonSendMut<GuiRuntime>,
) {
    for message in messages.read() {
        match message {
            GuiDragCommand::Cancel => runtime.cancel_drag(&mut commands),
            GuiDragCommand::Start {
                source,
                icon,
                data,
                touch_id,
            } => {
                let object = objects
                    .get(*source)
                    .map(|reference| reference.0)
                    .or_else(|_| roots.get(*source).map(|root| root.object))
                    .ok()
                    .or_else(|| runtime.ecs_window_object(*source));
                if let Some(object) = object {
                    windows::viewport(&mut runtime, &windows, &settings);
                    runtime.begin_ecs_drag(
                        &mut state,
                        object,
                        &GuiDragSource {
                            icon: icon.clone(),
                            data: data.clone(),
                        },
                        *touch_id,
                    );
                }
            }
        }
    }
}

pub(crate) fn sync_drag_state(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    mut state: ResMut<GuiDragState>,
    layers: Query<&GlobalZIndex>,
) {
    let next = if let Some(session) = &runtime.ecs_drag {
        let position = runtime.drag_drop.pointer();
        let agent = runtime
            .drag_drop
            .icon()
            .and_then(|icon| runtime.entity_of(icon));
        if let Some(agent) = agent
            && layers
                .get(agent)
                .ok()
                .is_none_or(|layer| layer.0 != 1_000_000)
        {
            commands.entity(agent).insert(GlobalZIndex(1_000_000));
        }
        GuiDragState {
            source: Some(session.source),
            agent,
            target: runtime
                .drag_drop
                .target()
                .and_then(|target| runtime.ecs_drop_target(target))
                .map(|(_, entity)| entity),
            data: runtime.drag_drop.data().map(str::to_owned),
            position: Some(Vec2::new(position.x, position.y)),
            touch_id: runtime.drag_touch_id,
        }
    } else {
        GuiDragState::default()
    };
    if *state != next {
        *state = next;
    }
}

pub(crate) fn emit_drops(mut commands: Commands, mut runtime: NonSendMut<GuiRuntime>) {
    for event in std::mem::take(&mut runtime.drop_events) {
        commands.trigger(event);
    }
}

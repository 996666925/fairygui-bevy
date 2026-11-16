//! Entity-owned application drag-and-drop, separate from window/scroll gestures.
use super::*;
use crate::GuiInputSettings;
use crate::input::InputRouter;
use bevy::ecs::system::SystemParam;
use bevy::window::PrimaryWindow;

#[cfg(test)]
#[path = "ecs_drag_tests.rs"]
mod tests;

/// Attach to a control view or root/window owner to start after the pointer threshold.
/// Owner configuration waits for assets and follows template replacement. A
/// view's configuration takes precedence over its owner's configuration.
/// The nearest configured ancestor of the pressed control wins. Removing this
/// component disables future starts; GuiDrag::cancel ends an already active drag.
#[derive(Component, Debug, Default, Clone, PartialEq, Eq)]
pub struct GuiDragSource {
    /// Native 100x100 centered Loader proxy; None starts without a visual proxy.
    pub icon: Option<String>,
    pub data: Option<String>,
}

/// A control view or root/window owner that accepts drops. The nearest configured
/// ancestor of the release hit receives one GuiDrop on the entity declaring this
/// component. A view declaration takes precedence over its owner's declaration.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GuiDropTarget;

/// Immediate access to application drag-and-drop from systems and observers.
/// Accepts materialized root/window owners or control views. The chosen mouse
/// or finger must still be captured inside the source. Use GuiDragSource for
/// automatic threshold starts; GuiDragState remains the published ECS snapshot.
///
/// Like GuiMut, this parameter mutably borrows GuiRuntime. Use a ParamSet if a
/// system also needs GuiMut, and release each borrow before using the other.
/// Mutable FairyGuiRoot/FairyGuiWindow queries also require ParamSet: owner
/// aliases are validated against their current template before capture is claimed.
#[derive(SystemParam)]
pub struct GuiDrag<'w, 's> {
    runtime: NonSendMut<'w, GuiRuntime>,
    input: ResMut<'w, InputRouter>,
    resolver: edit::GuiObjectResolver<'w, 's>,
    windows: Query<'w, 's, (Entity, &'static Window, Has<PrimaryWindow>)>,
    settings: Res<'w, GuiInputSettings>,
    commands: Commands<'w, 's>,
}
impl std::fmt::Debug for GuiDrag<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuiDrag")
            .field("dragging", &self.is_dragging())
            .finish_non_exhaustive()
    }
}
impl GuiDrag<'_, '_> {
    /// Start immediately with a captured pointer: None selects the left mouse,
    /// Some(id) selects that finger. Returns false for absent/unready/ineligible
    /// sources, unrelated/released pointers, or an already active drag.
    pub fn start(
        &mut self,
        source: Entity,
        configuration: GuiDragSource,
        touch_id: Option<u64>,
    ) -> bool {
        let object = self.resolver.object(&self.runtime, source);
        let Some(object) = object else {
            return false;
        };
        windows::viewport(&mut self.runtime, &self.windows, &self.settings);
        self.runtime
            .begin_ecs_drag(&mut self.input, source, object, &configuration, touch_id)
    }
    /// Cancel immediately without emitting a drop. Native proxy data is removed
    /// immediately; proxy entities are despawned when Bevy applies deferred work.
    /// The claimed pointer cannot trigger another automatic start until pressed again.
    pub fn cancel(&mut self) -> bool {
        if !self.runtime.is_dragging() {
            return false;
        }
        self.runtime.cancel_drag(&mut self.commands);
        true
    }
    pub fn is_dragging(&self) -> bool {
        self.runtime.is_dragging()
    }
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
    origin: Entity,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct DropTargetBinding {
    pub entity: Entity,
    view: Entity,
}

impl GuiRuntime {
    pub(super) fn ecs_drop_target(&self, mut target: GObjectId) -> Option<(GObjectId, Entity)> {
        loop {
            if let Some(binding) = self.drop_targets.get(&target)
                && self.entity_of(target) == Some(binding.view)
                && crate::input::eligible(self, target)
            {
                return Some((target, binding.entity));
            }
            target = self.store.object(target)?.parent()?;
        }
    }

    fn begin_ecs_drag(
        &mut self,
        input: &mut InputRouter,
        origin: Entity,
        source: GObjectId,
        configuration: &GuiDragSource,
        touch_id: Option<u64>,
    ) -> bool {
        if self.is_dragging() || !crate::input::eligible(self, source) {
            return false;
        }
        let Some(entity) = self.entity_of(source) else {
            return false;
        };
        let Some((_, pressed, position, _, _)) = input
            .drag_pointers()
            .into_iter()
            .find(|(id, ..)| *id == touch_id)
        else {
            return false;
        };
        let mut ancestor = Some(pressed);
        while ancestor.is_some_and(|ancestor| ancestor != source) {
            ancestor = ancestor.and_then(|ancestor| self.store.object(ancestor)?.parent());
        }
        if ancestor.is_none() || !position.is_finite() {
            return false;
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
        self.ecs_drag = Some(Session {
            source: entity,
            origin,
        });
        true
    }
}

/// Revalidate ownership before routing a release; never let a disposed or
/// blocked source deliver a cached drop on that frame.
pub(crate) fn prepare_drag_targets(
    mut commands: Commands,
    targets: Query<Entity, With<GuiDropTarget>>,
    resolver: edit::GuiObjectResolver,
    mut runtime: NonSendMut<GuiRuntime>,
) {
    runtime.drop_targets.clear();
    for entity in &targets {
        let Some((object, view)) = resolver.view(&runtime, entity) else {
            continue;
        };
        let binding = runtime
            .drop_targets
            .entry(object)
            .or_insert(DropTargetBinding { entity, view });
        if entity == view {
            *binding = DropTargetBinding { entity, view };
        }
    }
    if runtime.ecs_drag.as_ref().is_some_and(|session| {
        runtime.drag_drop.source().is_none_or(|source| {
            resolver.object(&runtime, session.origin) != Some(source)
                || resolver.object(&runtime, session.source) != Some(source)
                || runtime.entity_of(source) != Some(session.source)
                || !crate::input::eligible(&runtime, source)
        })
    }) {
        runtime.cancel_drag(&mut commands);
    }
}

pub(crate) fn begin_configured_drags(
    sources: Query<(Entity, &GuiDragSource)>,
    resolver: edit::GuiObjectResolver,
    mut state: ResMut<InputRouter>,
    windows: Query<(Entity, &Window, Has<PrimaryWindow>)>,
    settings: Res<GuiInputSettings>,
    mut runtime: NonSendMut<GuiRuntime>,
) {
    if runtime.is_dragging() {
        return;
    }
    let mut effective = HashMap::new();
    for (entity, configuration) in &sources {
        let Some((object, view)) = resolver.view(&runtime, entity) else {
            continue;
        };
        let entry = effective.entry(object).or_insert((entity, configuration));
        if entity == view {
            *entry = (entity, configuration);
        }
    }
    for (touch_id, target, _, moved, started) in state.drag_pointers() {
        if !moved || started {
            continue;
        }
        let mut source = Some(target);
        while let Some(object) = source {
            if let Some(&(entity, configuration)) = effective.get(&object) {
                windows::viewport(&mut runtime, &windows, &settings);
                runtime.begin_ecs_drag(&mut state, entity, object, configuration, touch_id);
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

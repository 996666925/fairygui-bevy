//! Entity-owned native windows and their lifecycle commands.
use super::*;
use crate::GuiInputSettings;
use bevy::window::PrimaryWindow;

#[cfg(test)]
#[path = "ecs_window_tests.rs"]
mod tests;

/// Asset-backed native window. Removing the component or despawning its owner
/// disposes the model and every view. Hiding retains them for reuse. Native
/// close-button input updates visible=false on this component.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct FairyGuiWindow {
    pub asset: Handle<FuiAsset>,
    pub item_name: String,
    pub visible: bool,
    pub modal: bool,
    pub draggable: bool,
    pub center_on_create: bool,
    /// Local template name or registered ui:// URL, created lazily on first wait.
    pub modal_wait: Option<String>,
}
impl FairyGuiWindow {
    pub fn new(asset: Handle<FuiAsset>, item_name: impl Into<String>) -> Self {
        Self {
            asset,
            item_name: item_name.into(),
            visible: true,
            modal: false,
            draggable: true,
            center_on_create: true,
            modal_wait: None,
        }
    }
    fn same_template(&self, other: &Self) -> bool {
        self.asset == other.asset && self.item_name == other.item_name
    }
}

/// Snapshot on the owner and window view, refreshed before Publish and at Sync end.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct GuiWindowState {
    view: Entity,
    showing: bool,
    modal: bool,
    draggable: bool,
    dragging: bool,
    frame: Option<Entity>,
    content: Option<Entity>,
    content_area: Option<Entity>,
    drag_area: Option<Entity>,
    close_button: Option<Entity>,
    modal_waiting: bool,
    modal_wait_request: u32,
    modal_wait_pane: Option<Entity>,
    modal_wait_error: Option<String>,
}
impl GuiWindowState {
    pub fn view(&self) -> Entity {
        self.view
    }
    pub fn is_showing(&self) -> bool {
        self.showing
    }
    pub fn is_modal(&self) -> bool {
        self.modal
    }
    pub fn is_draggable(&self) -> bool {
        self.draggable
    }
    pub fn is_dragging(&self) -> bool {
        self.dragging
    }
    pub fn frame(&self) -> Option<Entity> {
        self.frame
    }
    pub fn content(&self) -> Option<Entity> {
        self.content
    }
    pub fn content_area(&self) -> Option<Entity> {
        self.content_area
    }
    pub fn drag_area(&self) -> Option<Entity> {
        self.drag_area
    }
    pub fn close_button(&self) -> Option<Entity> {
        self.close_button
    }
    pub fn is_modal_waiting(&self) -> bool {
        self.modal_waiting
    }
    pub fn modal_wait_request(&self) -> u32 {
        self.modal_wait_request
    }
    pub fn modal_wait_pane(&self) -> Option<Entity> {
        self.modal_wait_pane
    }
    pub fn modal_wait_error(&self) -> Option<&str> {
        self.modal_wait_error.as_deref()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiWindowAction {
    Show,
    Hide,
    BringToFront,
    Center,
    /// Zero is an unrestricted request token, following native window semantics.
    ShowModalWait {
        request: u32,
    },
    /// A nonzero token only closes the matching request; zero closes any request.
    CloseModalWait {
        request: u32,
    },
}
/// Accepts a window owner or its root view. Waiting asset requests are retained;
/// replacement/removal cancels them. Dispose by removing FairyGuiWindow/despawning.
#[derive(Message, Debug, Clone, Copy)]
pub struct GuiWindowCommand {
    pub target: Entity,
    pub action: GuiWindowAction,
}
impl GuiWindowCommand {
    pub fn new(target: Entity, action: GuiWindowAction) -> Self {
        Self { target, action }
    }
}
#[derive(Debug)]
pub(super) struct Binding {
    source: FairyGuiWindow,
    object: Option<GObjectId>,
    pub(super) failed: bool,
    wait_pane: Option<GObjectId>,
    wait_error: Option<String>,
}

impl GuiRuntime {
    pub(crate) fn ecs_window_pending(&self, owner: Entity) -> bool {
        self.ecs_windows
            .get(&owner)
            .is_some_and(|binding| binding.object.is_none() && !binding.failed)
    }
    pub(crate) fn ecs_window_object(&self, owner: Entity) -> Option<GObjectId> {
        self.ecs_windows
            .get(&owner)?
            .object
            .filter(|&id| self.store.contains(id))
    }
    pub(crate) fn is_ecs_window_object(&self, object: GObjectId) -> bool {
        self.ecs_windows
            .values()
            .any(|binding| binding.object == Some(object))
    }
    pub(crate) fn window_index(&self, object: GObjectId) -> Option<usize> {
        self.windows
            .iter()
            .position(|window| window.owner() == object)
    }
    fn dispose_ecs_window(&mut self, commands: &mut Commands, object: GObjectId) {
        if let Some(index) = self.window_index(object) {
            self.close_window(commands, index);
        }
    }

    fn clear_ecs_wait(&mut self, owner: Entity) {
        let (object, pane) = {
            let Some(binding) = self.ecs_windows.get_mut(&owner) else {
                return;
            };
            binding.wait_error = None;
            (binding.object, binding.wait_pane.take())
        };
        if let Some(index) = object.and_then(|object| self.window_index(object)) {
            self.windows[index].clear_modal_wait_pane(&mut self.store);
        }
        if let Some(pane) = pane {
            self.store.remove(pane);
        }
    }

    fn show_ecs_wait(&mut self, index: usize, request: u32) {
        let object = self.windows[index].owner();
        let owner = self
            .ecs_windows
            .iter()
            .find_map(|(&owner, binding)| (binding.object == Some(object)).then_some(owner));
        if let Some(owner) = owner {
            let reference = self.ecs_windows[&owner].source.modal_wait.clone();
            if let Some(reference) = reference {
                let existing = self.ecs_windows[&owner]
                    .wait_pane
                    .filter(|&pane| self.store.contains(pane));
                let pane = existing.or_else(|| {
                    let template = self
                        .store
                        .object(object)
                        .and_then(|object| object.source_package_id())
                        .and_then(|id| self.registry.get_by_id(id))
                        .cloned()
                        .and_then(|package| self.resolve_template(&package, &reference))?;
                    let pane = self.create_template(&template)?;
                    self.ecs_windows.get_mut(&owner).unwrap().wait_pane = Some(pane);
                    Some(pane)
                });
                self.ecs_windows.get_mut(&owner).unwrap().wait_error =
                    pane.is_none().then_some(reference);
                if let Some(pane) = pane {
                    self.windows[index].set_modal_wait_pane(pane);
                }
            }
        }
        self.show_window_modal_wait(index, request);
    }
}
fn viewport(
    runtime: &mut GuiRuntime,
    windows: &Query<(Entity, &Window, Has<PrimaryWindow>)>,
    settings: &GuiInputSettings,
) {
    if let Some((_, window, _)) = windows
        .iter()
        .find(|(entity, _, primary)| settings.window.map_or(*primary, |chosen| chosen == *entity))
    {
        let size = fairygui_core::Size::new(window.width(), window.height());
        runtime.ensure_root(size);
        runtime.set_root_size(size);
    } else if runtime.root_container().is_none() {
        runtime.ensure_root(fairygui_core::Size::new(1280., 720.));
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_ecs_windows(
    mut commands: Commands,
    sources: Query<(Entity, &FairyGuiWindow)>,
    existing: Query<()>,
    mut removed: RemovedComponents<FairyGuiWindow>,
    assets: Res<Assets<FuiAsset>>,
    windows: Query<(Entity, &Window, Has<PrimaryWindow>)>,
    settings: Res<GuiInputSettings>,
    mut runtime: NonSendMut<GuiRuntime>,
) {
    for entity in removed.read() {
        if let Some(binding) = runtime.ecs_windows.remove(&entity)
            && let Some(object) = binding.object
        {
            runtime.dispose_ecs_window(&mut commands, object);
        }
        if existing.contains(entity) {
            commands
                .entity(entity)
                .remove::<(GuiWindowState, GuiRootView)>();
        }
    }
    for (entity, source) in &sources {
        if runtime
            .ecs_windows
            .get(&entity)
            .is_some_and(|binding| binding.source.modal_wait != source.modal_wait)
        {
            runtime.clear_ecs_wait(entity);
        }
        let mut binding = runtime.ecs_windows.remove(&entity).unwrap_or(Binding {
            source: source.clone(),
            object: None,
            failed: false,
            wait_pane: None,
            wait_error: None,
        });
        if !binding.source.same_template(source) {
            if let Some(object) = binding.object.take() {
                runtime.dispose_ecs_window(&mut commands, object);
            }
            binding.failed = false;
            binding.wait_pane = None;
            binding.wait_error = None;
            commands
                .entity(entity)
                .remove::<(GuiWindowState, GuiRootView)>();
        }
        if binding
            .object
            .is_some_and(|object| !runtime.store.contains(object))
        {
            if let Some(pane) = binding.wait_pane.take() {
                runtime.store.remove(pane);
            }
            binding.object = None;
            binding.failed = true;
            commands
                .entity(entity)
                .remove::<(GuiWindowState, GuiRootView)>();
        }
        if let Some(object) = binding.object {
            if let Some(index) = runtime.window_index(object) {
                if binding.source.visible != source.visible {
                    if source.visible {
                        runtime.show_existing_window(index);
                    } else {
                        runtime.hide_window(index);
                    }
                }
                if binding.source.modal != source.modal {
                    let runtime = &mut *runtime;
                    if let Some(root) = runtime.root.as_mut() {
                        runtime.windows[index].set_modal(root, source.modal);
                    }
                }
                runtime.windows[index].set_draggable(source.draggable);
            }
        } else if !binding.failed
            && let Some(asset) = assets.get(&source.asset)
        {
            viewport(&mut runtime, &windows, &settings);
            runtime.register_package(source.asset.clone(), &asset.package);
            if let Some(index) = runtime.construct_window(
                &asset.package,
                &source.item_name,
                source.modal,
                source.visible,
                source.center_on_create,
            ) {
                runtime.windows[index].set_draggable(source.draggable);
                binding.object = Some(runtime.windows[index].owner());
            } else {
                binding.failed = true;
                tracing::warn!(?entity, item = %source.item_name, "ECS FairyGUI window could not be created");
            }
        }
        binding.source = source.clone();
        runtime.ecs_windows.insert(entity, binding);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_window_commands(
    mut messages: MessageReader<GuiWindowCommand>,
    mut sources: Query<&mut FairyGuiWindow>,
    objects: Query<&GObjectRef>,
    mut removed: RemovedComponents<FairyGuiWindow>,
    mut pending: Local<Vec<(FairyGuiWindow, GuiWindowCommand)>>,
    windows: Query<(Entity, &Window, Has<PrimaryWindow>)>,
    settings: Res<GuiInputSettings>,
    mut runtime: NonSendMut<GuiRuntime>,
) {
    for entity in removed.read() {
        pending.retain(|(_, command)| command.target != entity);
    }
    let waiting = std::mem::take(&mut *pending);
    for (expected, command) in waiting
        .into_iter()
        .map(|(source, command)| (Some(source), command))
        .chain(messages.read().copied().map(|command| (None, command)))
    {
        let source = sources.get(command.target).ok();
        if let Some(expected) = expected
            && source.is_none_or(|source| !expected.same_template(source))
        {
            continue;
        }
        let object = runtime.ecs_window_object(command.target).or_else(|| {
            objects
                .get(command.target)
                .ok()
                .map(|reference| reference.0)
        });
        let index = object.and_then(|object| runtime.window_index(object));
        let Some(index) = index else {
            if let Some(source) = source
                && runtime
                    .ecs_windows
                    .get(&command.target)
                    .is_some_and(|binding| !binding.failed)
            {
                pending.push((source.clone(), command));
            }
            continue;
        };
        match command.action {
            GuiWindowAction::Show => {
                runtime.show_existing_window(index);
            }
            GuiWindowAction::Hide => {
                runtime.hide_window(index);
            }
            GuiWindowAction::BringToFront => {
                runtime.bring_window_to_front(index);
            }
            GuiWindowAction::Center => {
                viewport(&mut runtime, &windows, &settings);
                let runtime = &mut *runtime;
                if let Some(root) = runtime.root.as_ref() {
                    runtime.windows[index].center_on(&mut runtime.store, root);
                }
            }
            GuiWindowAction::ShowModalWait { request } => {
                runtime.show_ecs_wait(index, request);
            }
            GuiWindowAction::CloseModalWait { request } => {
                runtime.close_window_modal_wait(index, request);
            }
        }
        let object = runtime.windows[index].owner();
        let showing = runtime.windows[index].is_showing();
        for (entity, binding) in &mut runtime.ecs_windows {
            if binding.object == Some(object) {
                binding.source.visible = showing;
                if let Ok(mut source) = sources.get_mut(*entity)
                    && source.visible != showing
                {
                    source.visible = showing;
                }
            }
        }
    }
}

pub(crate) fn sync_window_state(
    mut commands: Commands,
    mut runtime: NonSendMut<GuiRuntime>,
    mut sources: Query<(
        Entity,
        &mut FairyGuiWindow,
        Option<&GuiWindowState>,
        Option<&GuiRootView>,
    )>,
    states: Query<&GuiWindowState, Without<FairyGuiWindow>>,
) {
    for (entity, mut source, current, root_view) in &mut sources {
        let Some(object) = runtime.ecs_window_object(entity) else {
            continue;
        };
        let Some(index) = runtime.window_index(object) else {
            continue;
        };
        let Some(view) = runtime.entity_of(object) else {
            continue;
        };
        let window = &runtime.windows[index];
        let state = GuiWindowState {
            view,
            showing: window.is_showing(),
            modal: window.is_modal(),
            draggable: window.is_draggable(),
            dragging: window.is_dragging(),
            frame: window.frame().and_then(|id| runtime.entity_of(id)),
            content: window.content().and_then(|id| runtime.entity_of(id)),
            content_area: window.content_area().and_then(|id| runtime.entity_of(id)),
            drag_area: window.drag_area().and_then(|id| runtime.entity_of(id)),
            close_button: window.close_button().and_then(|id| runtime.entity_of(id)),
            modal_waiting: window.is_modal_waiting(),
            modal_wait_request: window.modal_wait_request(),
            modal_wait_pane: window
                .modal_wait_pane()
                .and_then(|id| runtime.entity_of(id)),
            modal_wait_error: runtime.ecs_windows[&entity].wait_error.clone(),
        };
        if current != Some(&state) {
            commands.entity(entity).insert(state.clone());
        }
        if states.get(view).ok() != Some(&state) {
            commands.entity(view).insert(state.clone());
        }
        if root_view.map(|root| root.0) != Some(view) {
            commands.entity(entity).insert(GuiRootView(view));
        }
        let binding = runtime.ecs_windows.get_mut(&entity).unwrap();
        // Do not overwrite a new application visibility request before reconciliation.
        if source.visible == binding.source.visible && source.visible != state.showing {
            source.visible = state.showing;
            binding.source.visible = state.showing;
        }
    }
}

//! Stage-wide native modal waiting, configured and controlled through ECS.
use super::*;
use crate::GuiInputSettings;
use bevy::window::PrimaryWindow;

#[cfg(test)]
#[path = "ecs_global_wait_tests.rs"]
mod tests;

/// Template for the single stage-wide wait pane. The asset is the local template
/// context; item_name also accepts registered ui:// URLs. Changing or clearing
/// the template disposes its previous pane. An active request retries on asset arrival.
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct GuiGlobalWaitSettings {
    pub asset: Option<Handle<FuiAsset>>,
    pub item_name: String,
}
impl GuiGlobalWaitSettings {
    pub fn new(asset: Handle<FuiAsset>, item_name: impl Into<String>) -> Self {
        Self {
            asset: Some(asset),
            item_name: item_name.into(),
        }
    }
}
#[derive(Message, Debug, Clone)]
pub enum GuiGlobalWaitCommand {
    Show {
        message: Option<String>,
    },
    /// Detach the pane and retain its model/view for reuse; cancel a pending Show.
    Hide,
    /// Cancel the request and dispose the retained pane, including hidden views.
    Dispose,
}
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GuiGlobalWaitError {
    #[error("global modal waiting has no configured template")]
    MissingTemplate,
    #[error("cannot resolve or construct global modal wait template {0}")]
    MissingResource(String),
}
/// Actual view and request state. A pending or failed request does not block input.
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct GuiGlobalWaitState {
    requested: bool,
    waiting: bool,
    view: Option<Entity>,
    error: Option<GuiGlobalWaitError>,
}
impl GuiGlobalWaitState {
    pub fn is_requested(&self) -> bool {
        self.requested
    }
    pub fn is_waiting(&self) -> bool {
        self.waiting
    }
    pub fn view(&self) -> Option<Entity> {
        self.view
    }
    pub fn error(&self) -> Option<&GuiGlobalWaitError> {
        self.error.as_ref()
    }
}
#[derive(Debug, Default)]
pub(super) struct GlobalWait {
    settings: GuiGlobalWaitSettings,
    pane: Option<GObjectId>,
    requested: bool,
    message: Option<String>,
    error: Option<GuiGlobalWaitError>,
}
impl GuiRuntime {
    /// Global waiting is separate from window modality and the popup stack.
    pub(crate) fn global_wait_pane(&self) -> Option<GObjectId> {
        self.global_wait.pane.filter(|&pane| {
            self.global_wait.requested
                && self.store.is_on_stage(pane)
                && self.store.display_visible(pane)
        })
    }
    fn dispose_global_wait(&mut self) {
        if let Some(pane) = self.global_wait.pane.take() {
            self.store.remove(pane);
        }
        self.global_wait.error = None;
    }
}

pub(crate) fn sync_global_wait(
    mut messages: MessageReader<GuiGlobalWaitCommand>,
    settings: Res<GuiGlobalWaitSettings>,
    assets: Res<Assets<FuiAsset>>,
    windows: Query<(Entity, &Window, Has<PrimaryWindow>)>,
    input: Res<GuiInputSettings>,
    mut runtime: NonSendMut<GuiRuntime>,
) {
    if runtime.global_wait.settings != *settings {
        runtime.dispose_global_wait();
        runtime.global_wait.settings = settings.clone();
    }
    let mut refresh_message = false;
    for command in messages.read() {
        match command {
            GuiGlobalWaitCommand::Show { message } => {
                runtime.global_wait.requested = true;
                runtime.global_wait.message = message.clone();
                runtime.global_wait.error = None;
                refresh_message = true;
            }
            GuiGlobalWaitCommand::Hide => {
                runtime.global_wait.requested = false;
                runtime.global_wait.error = None;
            }
            GuiGlobalWaitCommand::Dispose => {
                runtime.global_wait.requested = false;
                runtime.dispose_global_wait();
            }
        }
    }
    if runtime
        .global_wait
        .pane
        .is_some_and(|pane| !runtime.store.contains(pane))
    {
        runtime.global_wait.pane = None;
    }
    if !runtime.global_wait.requested {
        if let Some(pane) = runtime.global_wait.pane {
            runtime.store.detach_from_parent(pane);
            runtime.store.set_visible(pane, false);
        }
        return;
    }
    if runtime.global_wait.pane.is_none() {
        let Some(handle) = settings
            .asset
            .as_ref()
            .filter(|_| !settings.item_name.is_empty())
        else {
            runtime.global_wait.error = Some(GuiGlobalWaitError::MissingTemplate);
            return;
        };
        let Some(asset) = assets.get(handle) else {
            return;
        };
        runtime.register_package(handle.clone(), &asset.package);
        let template = runtime.resolve_template(&asset.package, &settings.item_name);
        let pane = template
            .as_ref()
            .and_then(|template| runtime.create_template(template));
        let Some(pane) = pane else {
            runtime.global_wait.error = Some(GuiGlobalWaitError::MissingResource(
                settings.item_name.clone(),
            ));
            return;
        };
        windows::viewport(&mut runtime, &windows, &input);
        let stage = runtime.root_container().unwrap();
        runtime
            .object_assets
            .entry(stage)
            .or_insert_with(|| handle.clone());
        runtime.global_wait.pane = Some(pane);
        runtime.global_wait.error = None;
        refresh_message = true;
    }
    windows::viewport(&mut runtime, &windows, &input);
    let pane = runtime.global_wait.pane.unwrap();
    let stage = runtime.root_container().unwrap();
    let stage_object = runtime.store.object(stage).unwrap();
    let size = stage_object.size();
    let last = stage_object.children().last().copied();
    if last != Some(pane) {
        let count = stage_object.children().len();
        runtime.store.add_child_at(stage, pane, count);
    }
    runtime.store.set_xy(pane, 0., 0.);
    runtime.store.set_size(pane, size.width, size.height);
    runtime.store.set_visible(pane, true);
    if refresh_message {
        let message = runtime.global_wait.message.clone().unwrap_or_default();
        runtime.store.set_object_text(pane, &message);
    }
    runtime.queue_subtree(pane);
}

pub(crate) fn sync_global_wait_state(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    mut state: ResMut<GuiGlobalWaitState>,
    layers: Query<&GlobalZIndex>,
) {
    let pane = runtime
        .global_wait
        .pane
        .filter(|&pane| runtime.store.contains(pane));
    let view = pane.and_then(|pane| runtime.entity_of(pane));
    if let Some(view) = view
        && layers
            .get(view)
            .ok()
            .is_none_or(|layer| layer.0 != i32::MAX)
    {
        commands.entity(view).insert(GlobalZIndex(i32::MAX));
    }
    let next = GuiGlobalWaitState {
        requested: runtime.global_wait.requested,
        waiting: runtime.global_wait_pane().is_some() && view.is_some(),
        view,
        error: runtime.global_wait.error.clone(),
    };
    if *state != next {
        *state = next;
    }
}

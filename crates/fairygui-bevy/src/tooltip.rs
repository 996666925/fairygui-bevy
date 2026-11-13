//! Native delayed tooltips, independent of popups and window modality.
use super::*;
use crate::{GuiInputSettings, GuiInputState};
use bevy::window::PrimaryWindow;
use fairygui_core::Point;

#[cfg(test)]
#[path = "ecs_tooltip_tests.rs"]
mod tests;

/// Default tooltip template, equivalent to UIConfig.tooltipsWin. It should have
/// a text/title property. Registered ui:// URLs also work as item names.
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct GuiTooltipSettings {
    pub asset: Option<Handle<FuiAsset>>,
    pub item_name: String,
}
impl GuiTooltipSettings {
    pub fn new(asset: Handle<FuiAsset>, item_name: impl Into<String>) -> Self {
        Self {
            asset: Some(asset),
            item_name: item_name.into(),
        }
    }
}

#[derive(Message, Debug, Clone)]
pub enum GuiTooltipCommand {
    /// Show text with the default template. None places it at the current mouse
    /// position plus (10, 20). Explicit positions are logical window coordinates.
    Show {
        text: String,
        position: Option<Vec2>,
    },
    /// Adopt an existing root owner or control view as a custom tooltip. Asset
    /// loading is retained for root owners; replacing/removing its source cancels.
    /// Hide detaches it; its original ECS owner still owns its lifetime.
    ShowView {
        view: Entity,
        position: Option<Vec2>,
    },
    /// Hide and retain the default template. Like native hideTooltips, this does
    /// not cancel a hover delay that has not fired yet.
    Hide,
    /// Hide, dispose the default template and cancel the current hover delay.
    /// Caller-owned custom views remain alive until their owner is removed.
    Dispose,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GuiTooltipError {
    #[error("tooltips have no configured default template")]
    MissingTemplate,
    #[error("cannot resolve or construct tooltip template {0}")]
    MissingResource(String),
    #[error("tooltip view is unavailable or cannot be adopted: {0}")]
    InvalidView(Entity),
}

/// Actual tooltip state, refreshed before Publish and after Sync. `view` also
/// exposes the retained default template while it is hidden.
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct GuiTooltipState {
    requested: bool,
    showing: bool,
    view: Option<Entity>,
    source: Option<Entity>,
    error: Option<GuiTooltipError>,
}
impl GuiTooltipState {
    pub fn is_requested(&self) -> bool {
        self.requested
    }
    pub fn is_showing(&self) -> bool {
        self.showing
    }
    pub fn view(&self) -> Option<Entity> {
        self.view
    }
    pub fn source(&self) -> Option<Entity> {
        self.source
    }
    pub fn error(&self) -> Option<&GuiTooltipError> {
        self.error.as_ref()
    }
}

#[derive(Clone)]
enum Request {
    Text {
        text: String,
        position: Option<Vec2>,
        source: Option<GObjectId>,
    },
    View {
        entity: Entity,
        position: Option<Vec2>,
        source: Option<ObjectCommandSource>,
    },
}
struct Hover {
    source: GObjectId,
    deadline: f64,
    fired: bool,
}
#[derive(Default)]
pub(super) struct Tooltip {
    cursor: bevy::ecs::message::MessageCursor<GuiTooltipCommand>,
    settings: GuiTooltipSettings,
    default_pane: Option<GObjectId>,
    shown: Option<GObjectId>,
    request: Option<Request>,
    hover: Option<Hover>,
    position: Option<Vec2>,
    // Adopted children can outlive their original parent when hidden/detached.
    // Keep their logical owner so disposing a page also disposes these children.
    custom_owners: HashMap<GObjectId, GObjectId>,
    error: Option<GuiTooltipError>,
    projected_layer: Option<(Entity, Option<GlobalZIndex>)>,
}

impl GuiRuntime {
    pub(crate) fn is_tooltip_part(&self, object: GObjectId) -> bool {
        self.tooltip
            .shown
            .is_some_and(|pane| self.is_descendant_of_or_self(object, pane))
    }
    pub(crate) fn hide_tooltip(&mut self) {
        if let Some(pane) = self.tooltip.shown.take() {
            self.store.detach_from_parent(pane);
        }
        self.tooltip.request = None;
        self.tooltip.position = None;
        self.tooltip.error = None;
    }
    fn dispose_default_tooltip(&mut self) {
        if let Some(pane) = self.tooltip.default_pane.take() {
            if self.tooltip.shown == Some(pane) {
                self.tooltip.shown = None;
            }
            self.store.remove(pane);
        }
    }
    fn tooltip_source(&self, hovered: Option<GObjectId>) -> Option<GObjectId> {
        let mut current = hovered.filter(|id| crate::input::eligible(self, *id));
        while let Some(id) = current {
            let object = self.store.object(id)?;
            if object.tooltips().is_some_and(|text| !text.is_empty()) {
                return Some(id);
            }
            current = object.parent();
        }
        None
    }
    fn cleanup_tooltips(&mut self) {
        let stale: Vec<_> = self
            .tooltip
            .custom_owners
            .iter()
            .filter(|(pane, owner)| !self.store.contains(**pane) || !self.store.contains(**owner))
            .map(|(pane, _)| *pane)
            .collect();
        for pane in stale {
            self.tooltip.custom_owners.remove(&pane);
            self.store.remove(pane);
        }
        if self
            .tooltip
            .default_pane
            .is_some_and(|pane| !self.store.contains(pane))
        {
            self.tooltip.default_pane = None;
        }
        if self
            .tooltip
            .shown
            .is_some_and(|pane| !self.store.contains(pane))
        {
            self.hide_tooltip();
        }
    }
    fn place_tooltip(&mut self) {
        let Some((pane, position, stage)) = self
            .tooltip
            .shown
            .zip(self.tooltip.position)
            .zip(self.root_container())
            .map(|((pane, position), stage)| (pane, position, stage))
        else {
            return;
        };
        let Some(point) = self
            .store
            .global_to_local(stage, Point::new(position.x, position.y))
        else {
            return;
        };
        let Some(object) = self.store.object(pane) else {
            return;
        };
        let size = Vec2::new(object.width(), object.height());
        let root = self.store.object(stage).unwrap();
        let position = tooltip_position(
            Vec2::new(point.x, point.y),
            size,
            Vec2::new(root.width(), root.height()),
        );
        self.store.set_xy(pane, position.x, position.y);
    }
}

// GRoot's native edge adjustment, including its second left shift at the bottom.
fn tooltip_position(mut point: Vec2, size: Vec2, viewport: Vec2) -> Vec2 {
    if point.x + size.x > viewport.x {
        point.x -= size.x + 1.;
        if point.x < 0. {
            point.x = 10.;
        }
    }
    if point.y + size.y > viewport.y {
        point.y -= size.y + 1.;
        if point.x - size.x - 1. > 0. {
            point.x -= size.x + 1.;
        }
        if point.y < 0. {
            point.y = 10.;
        }
    }
    point
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_tooltips(
    messages: Res<Messages<GuiTooltipCommand>>,
    settings: Res<GuiTooltipSettings>,
    assets: Res<Assets<FuiAsset>>,
    windows: Query<(Entity, &Window, Has<PrimaryWindow>)>,
    input: Res<GuiInputSettings>,
    state: Res<GuiInputState>,
    time: Res<Time>,
    objects: Query<&GObjectRef>,
    roots: Query<&GuiRootInstance>,
    sources: Query<&FairyGuiRoot>,
    window_sources: Query<&windows::FairyGuiWindow>,
    mut runtime: NonSendMut<GuiRuntime>,
) {
    runtime.cleanup_tooltips();
    let mut refresh = false;
    if runtime.tooltip.settings != *settings {
        runtime.dispose_default_tooltip();
        runtime.tooltip.settings = settings.clone();
        runtime.tooltip.error = None;
        refresh = true;
    }
    let window = windows
        .iter()
        .find(|(entity, _, primary)| input.window.map_or(*primary, |chosen| chosen == *entity));
    let hovered = state
        .hovered()
        .and_then(|entity| objects.get(entity).ok())
        .map(|object| object.0);
    let source = runtime.tooltip_source(hovered);
    if source != runtime.tooltip.hover.as_ref().map(|hover| hover.source) {
        if runtime.tooltip.hover.is_some() {
            runtime.hide_tooltip();
        }
        runtime.tooltip.hover = source.map(|source| Hover {
            source,
            deadline: time.elapsed_secs_f64() + 0.1,
            fired: false,
        });
    }
    if let Some(hover) = runtime.tooltip.hover.as_mut()
        && !hover.fired
        && time.elapsed_secs_f64() + 1e-9 >= hover.deadline
    {
        hover.fired = true;
        let source = hover.source;
        let text = runtime
            .store
            .object(source)
            .and_then(|object| object.tooltips())
            .unwrap_or_default()
            .to_owned();
        runtime.hide_tooltip();
        runtime.tooltip.request = Some(Request::Text {
            text,
            position: None,
            source: Some(source),
        });
        refresh = true;
    }
    let command_source = |entity| {
        sources
            .get(entity)
            .ok()
            .cloned()
            .map(ObjectCommandSource::Root)
            .or_else(|| {
                window_sources
                    .get(entity)
                    .ok()
                    .cloned()
                    .map(ObjectCommandSource::Window)
            })
    };
    let incoming: Vec<_> = runtime.tooltip.cursor.read(&messages).cloned().collect();
    for message in &incoming {
        let request = match message {
            GuiTooltipCommand::Show { text, position } => Some(Request::Text {
                text: text.clone(),
                position: *position,
                source: None,
            }),
            GuiTooltipCommand::ShowView { view, position } => Some(Request::View {
                entity: *view,
                position: *position,
                source: command_source(*view),
            }),
            GuiTooltipCommand::Hide => {
                runtime.hide_tooltip();
                continue;
            }
            GuiTooltipCommand::Dispose => {
                runtime.hide_tooltip();
                runtime.dispose_default_tooltip();
                if let Some(hover) = &mut runtime.tooltip.hover {
                    hover.fired = true;
                }
                continue;
            }
        };
        let position = match &request {
            Some(Request::Text { position, .. } | Request::View { position, .. }) => *position,
            None => None,
        };
        if position.is_some_and(|position| !position.is_finite()) {
            continue;
        }
        runtime.hide_tooltip();
        runtime.tooltip.request = request;
        refresh = true;
    }
    if window.is_some_and(|(_, window, _)| !window.focused) {
        runtime.hide_tooltip();
        return;
    }
    let Some(request) = runtime.tooltip.request.clone() else {
        return;
    };
    if runtime.tooltip.shown.is_some() && !refresh {
        return;
    }
    let (pane, position) = match request {
        Request::Text { text, position, .. } => {
            if runtime.tooltip.default_pane.is_none() {
                let Some(handle) = settings
                    .asset
                    .as_ref()
                    .filter(|_| !settings.item_name.is_empty())
                else {
                    runtime.tooltip.error = Some(GuiTooltipError::MissingTemplate);
                    return;
                };
                let Some(asset) = assets.get(handle) else {
                    return;
                };
                runtime.register_package(handle.clone(), &asset.package);
                let template = runtime.resolve_template(&asset.package, &settings.item_name);
                let Some(pane) = template
                    .as_ref()
                    .and_then(|template| runtime.create_template(template))
                else {
                    runtime.tooltip.error =
                        Some(GuiTooltipError::MissingResource(settings.item_name.clone()));
                    return;
                };
                runtime.tooltip.default_pane = Some(pane);
            }
            let pane = runtime.tooltip.default_pane.unwrap();
            runtime.store.set_object_text(pane, &text);
            let mut fields = vec![pane];
            runtime
                .store
                .for_each_descendant(pane, &mut |id| fields.push(id));
            let measurer = runtime.text_measurer.clone();
            for field in fields {
                measurer.borrow_mut().ensure(&mut runtime.store, field);
            }
            (pane, position)
        }
        Request::View {
            entity,
            position,
            source,
        } => {
            if let Some(expected) = source.as_ref()
                && command_source(entity)
                    .as_ref()
                    .is_none_or(|current| !current.same_template(expected))
            {
                runtime.hide_tooltip();
                return;
            }
            let object = objects
                .get(entity)
                .map(|id| id.0)
                .or_else(|_| roots.get(entity).map(|instance| instance.object))
                .ok()
                .or_else(|| runtime.ecs_window_object(entity));
            let Some(pane) = object.filter(|pane| runtime.store.contains(*pane)) else {
                if source.is_none() {
                    runtime.hide_tooltip();
                    runtime.tooltip.error = Some(GuiTooltipError::InvalidView(entity));
                }
                return;
            };
            if runtime
                .root_container()
                .is_some_and(|root| runtime.is_descendant_of_or_self(root, pane))
            {
                runtime.hide_tooltip();
                runtime.tooltip.error = Some(GuiTooltipError::InvalidView(entity));
                return;
            }
            // Record the original page/window for detached custom descendants.
            let mut owner = pane;
            while let Some(parent) = runtime
                .store
                .object(owner)
                .and_then(|object| object.parent())
            {
                if runtime.root_container() == Some(parent) {
                    break;
                }
                owner = parent;
            }
            if owner != pane {
                runtime.tooltip.custom_owners.insert(pane, owner);
            }
            (pane, position)
        }
    };
    windows::viewport(&mut runtime, &windows, &input);
    let stage = runtime.root_container().unwrap();
    if let Some(handle) = runtime.asset_handle_for(pane) {
        runtime.object_assets.entry(stage).or_insert(handle);
    }
    if runtime.popup_stack().contains(&pane) {
        runtime.hide_popup(Some(pane));
    }
    let gui = &mut *runtime;
    if let Some(root) = gui.root.as_mut() {
        root.detach(&mut gui.store, pane);
    }
    runtime.store.detach_from_stage(pane);
    if let Some(record) = runtime.roots.iter_mut().find(|root| root.object == pane) {
        record.attached = false;
    }
    let count = runtime.store.object(stage).unwrap().children().len();
    runtime.store.add_child_at(stage, pane, count);
    runtime.store.set_visible(pane, true);
    runtime.tooltip.shown = Some(pane);
    runtime.tooltip.position = Some(position.unwrap_or_else(|| {
        window
            .and_then(|(_, window, _)| window.cursor_position())
            .unwrap_or(Vec2::ZERO)
            + Vec2::new(10., 20.)
    }));
    runtime.tooltip.error = None;
    runtime.place_tooltip();
    runtime.queue_subtree(pane);
}

/// Run after object commands and synchronous text sizing, before view projection.
pub(crate) fn reconcile_tooltips(mut runtime: NonSendMut<GuiRuntime>) {
    runtime.cleanup_tooltips();
    if let Some(Request::Text {
        source: Some(source),
        ..
    }) = runtime.tooltip.request.as_ref()
        && runtime.tooltip_source(Some(*source)) != Some(*source)
    {
        runtime.hide_tooltip();
    }
    runtime.place_tooltip();
}

pub(crate) fn sync_tooltip_state(
    mut commands: Commands,
    mut runtime: NonSendMut<GuiRuntime>,
    mut state: ResMut<GuiTooltipState>,
    layers: Query<&GlobalZIndex>,
) {
    let shown = runtime
        .tooltip
        .shown
        .filter(|pane| runtime.store.is_on_stage(*pane) && runtime.store.display_visible(*pane));
    let overlay = shown.and_then(|pane| runtime.entity_of(pane));
    if runtime
        .tooltip
        .projected_layer
        .as_ref()
        .map(|(entity, _)| *entity)
        != overlay
    {
        if let Some((entity, layer)) = runtime.tooltip.projected_layer.take()
            && let Ok(mut entity) = commands.get_entity(entity)
        {
            if let Some(layer) = layer {
                entity.insert(layer);
            } else {
                entity.remove::<GlobalZIndex>();
            }
        }
        if let Some(entity) = overlay {
            runtime.tooltip.projected_layer = Some((entity, layers.get(entity).ok().copied()));
            commands.entity(entity).insert(GlobalZIndex(i32::MAX - 1));
        }
    }
    let next = GuiTooltipState {
        requested: runtime.tooltip.request.is_some(),
        showing: overlay.is_some(),
        view: overlay.or_else(|| {
            runtime
                .tooltip
                .default_pane
                .and_then(|pane| runtime.entity_of(pane))
        }),
        source: match runtime.tooltip.request.as_ref() {
            Some(Request::Text { source, .. }) => {
                source.and_then(|source| runtime.entity_of(source))
            }
            _ => None,
        },
        error: runtime.tooltip.error.clone(),
    };
    if *state != next {
        *state = next;
    }
}

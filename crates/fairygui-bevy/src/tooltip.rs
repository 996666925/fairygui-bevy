//! Native delayed tooltips, independent of popups and window modality.
use super::*;
use crate::{GuiInputSettings, GuiInputState};
use bevy::ecs::system::SystemParam;
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

/// Direct tooltip access from systems and observers. Requests change immediately;
/// pending assets materialize during Sync, including calls from PostLayout.
/// GuiTooltipState is the published ECS snapshot. Like GuiMut, this parameter
/// mutably borrows GuiRuntime: use a ParamSet when both are needed in one system.
#[derive(SystemParam)]
pub struct GuiTooltip<'w, 's> {
    runtime: NonSendMut<'w, GuiRuntime>,
    objects: Query<'w, 's, &'static GObjectRef>,
    roots: Query<'w, 's, &'static FairyGuiRoot>,
    windows: Query<'w, 's, &'static windows::FairyGuiWindow>,
}
impl std::fmt::Debug for GuiTooltip<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuiTooltip")
            .field("requested", &self.is_requested())
            .field("showing", &self.is_showing())
            .finish_non_exhaustive()
    }
}
impl GuiTooltip<'_, '_> {
    /// Show text using the default template. None uses the mouse position plus
    /// (10, 20) when the view becomes ready. Explicit positions are window logical
    /// coordinates. Nonfinite positions return false without changing the tooltip.
    pub fn show(&mut self, text: impl Into<String>, position: Option<Vec2>) -> bool {
        if position.is_some_and(|position| !position.is_finite()) {
            return false;
        }
        self.runtime.hide_tooltip();
        self.runtime.tooltip.request = Some(Request::Text {
            text: text.into(),
            position,
            source: None,
        });
        true
    }
    /// Adopt an existing root/window owner or control view. Unloaded owners retain
    /// the request; replacing/removing their template cancels it. Hide detaches the
    /// view, whose lifetime remains with its original ECS owner. Invalid entities,
    /// the stage itself and nonfinite positions return false without changing it.
    pub fn show_view(&mut self, view: Entity, position: Option<Vec2>) -> bool {
        if position.is_some_and(|position| !position.is_finite()) {
            return false;
        }
        let source = owner_template(view, &self.roots, &self.windows);
        let object = self
            .objects
            .get(view)
            .ok()
            .map(|reference| reference.0)
            .or_else(|| match source.as_ref() {
                Some(GuiOwnerTemplate::Root(_)) => self.runtime.ecs_root_object(view),
                Some(GuiOwnerTemplate::Window(_)) => self.runtime.ecs_window_object(view),
                None => None,
            });
        if let Some(object) = object {
            if !self.runtime.store.contains(object)
                || self
                    .runtime
                    .root_container()
                    .is_some_and(|stage| self.runtime.is_descendant_of_or_self(stage, object))
            {
                return false;
            }
        } else if source.is_none() {
            return false;
        }
        self.runtime.hide_tooltip();
        self.runtime.tooltip.request = Some(Request::View {
            entity: view,
            position,
            source,
        });
        true
    }
    /// Hide now, retaining the default view and any hover delay not yet fired.
    pub fn hide(&mut self) {
        self.runtime.hide_tooltip();
    }
    /// Hide, destroy the default native view and cancel the current hover delay.
    /// Bevy removes its entities during Sync; caller-owned custom views stay alive.
    pub fn dispose(&mut self) {
        self.runtime.hide_tooltip();
        self.runtime.dispose_default_tooltip();
        if let Some(hover) = &mut self.runtime.tooltip.hover {
            hover.fired = true;
        }
    }
    pub fn is_requested(&self) -> bool {
        self.runtime.tooltip.request.is_some()
    }
    /// Read the live native state; GuiTooltipState is refreshed by the scheduler.
    pub fn is_showing(&self) -> bool {
        self.runtime.tooltip.shown.is_some_and(|pane| {
            self.runtime.store.is_on_stage(pane) && self.runtime.store.display_visible(pane)
        })
    }
}

fn owner_template(
    entity: Entity,
    roots: &Query<&FairyGuiRoot>,
    windows: &Query<&windows::FairyGuiWindow>,
) -> Option<GuiOwnerTemplate> {
    roots
        .get(entity)
        .ok()
        .cloned()
        .map(GuiOwnerTemplate::Root)
        .or_else(|| {
            windows
                .get(entity)
                .ok()
                .cloned()
                .map(GuiOwnerTemplate::Window)
        })
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
        source: Option<GuiOwnerTemplate>,
    },
}
struct Hover {
    source: GObjectId,
    deadline: f64,
    fired: bool,
}
#[derive(Default)]
pub(super) struct Tooltip {
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
                && owner_template(entity, &sources, &window_sources)
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

/// Run after object edits and synchronous text sizing, before view projection.
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

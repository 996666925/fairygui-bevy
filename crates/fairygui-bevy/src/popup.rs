//! Entity-addressed popup operations and the current popup stack.
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use fairygui_core::{geom::Point, popup::MenuItem};

use crate::{
    FairyGuiRoot, FairyGuiWindow, FuiAsset, GObjectRef, GuiInputSettings, GuiRuntime,
    PopupMenuTemplates,
};

use crate::ecs::removal_status;
use crate::runtime::GuiOwnerTemplate;

#[cfg(test)]
#[path = "ecs_popup_tests.rs"]
mod tests;

/// Retained menu configuration. Template names may also be registered `ui://` URLs.
/// The runtime retains one menu; showing unchanged configuration preserves edits
/// and checked state. A separator row needs a separate separator template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiPopupMenu {
    pub content: String,
    pub item: String,
    pub separator: Option<String>,
    pub items: Vec<MenuItem>,
}

impl GuiPopupMenu {
    pub fn new(content: impl Into<String>, item: impl Into<String>, items: Vec<MenuItem>) -> Self {
        Self {
            content: content.into(),
            item: item.into(),
            separator: None,
            items,
        }
    }
}

/// Native placement relative to a control's logical origin.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GuiPopupDirection {
    #[default]
    Auto,
    Up,
    Down,
}

/// Direct access to popups and the retained menu from systems and observers.
/// Ready objects change immediately; unloaded root/window owners retain show
/// requests until Sync. GuiPopupState is the published snapshot. Like GuiMut,
/// this parameter mutably borrows GuiRuntime: use ParamSet when both are needed.
#[derive(SystemParam)]
pub struct GuiPopup<'w, 's> {
    runtime: NonSendMut<'w, GuiRuntime>,
    objects: Query<'w, 's, &'static GObjectRef>,
    roots: Query<'w, 's, &'static FairyGuiRoot>,
    window_sources: Query<'w, 's, &'static FairyGuiWindow>,
    assets: Res<'w, Assets<FuiAsset>>,
    windows: Query<'w, 's, (Entity, &'static Window, Has<PrimaryWindow>)>,
    settings: Res<'w, GuiInputSettings>,
    commands: Commands<'w, 's>,
    removed_roots: RemovedComponents<'w, 's, FairyGuiRoot>,
    removed_windows: RemovedComponents<'w, 's, FairyGuiWindow>,
}
impl std::fmt::Debug for GuiPopup<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuiPopup")
            .field("stack", &self.runtime.popup_stack())
            .field("pending", &self.runtime.pending_popups.len())
            .finish_non_exhaustive()
    }
}

// Only unresolved shows are retained. Hide/dispose always run synchronously.
pub(crate) struct PendingPopup {
    owner: Entity,
    source: GuiOwnerTemplate,
    request: PopupRequest,
    removals_before: usize,
}
enum PopupRequest {
    Show(Vec2),
    Anchored {
        anchor: Entity,
        direction: GuiPopupDirection,
    },
    Menu {
        menu: GuiPopupMenu,
        position: Vec2,
    },
}
impl PopupRequest {
    fn is_menu(&self) -> bool {
        matches!(self, Self::Menu { .. })
    }
}

impl GuiPopup<'_, '_> {
    /// Show an existing root/control at a stage-local position. Native windows
    /// use FairyGuiWindow.visible instead. Returns whether the show was accepted.
    pub fn show(&mut self, popup: Entity, position: Vec2) -> bool {
        if !position.is_finite() {
            return false;
        }
        self.request(popup, PopupRequest::Show(position))
    }
    /// Place relative to the anchor's current native geometry. A retained request
    /// reads geometry when its popup becomes ready; the anchor must then be ready.
    pub fn show_anchored(
        &mut self,
        popup: Entity,
        anchor: Entity,
        direction: GuiPopupDirection,
    ) -> bool {
        if self.object(anchor).is_none() && self.source(anchor).is_none() {
            return false;
        }
        self.request(popup, PopupRequest::Anchored { anchor, direction })
    }
    /// Resolve menu templates in the source's package. Unchanged configuration
    /// retains checked state, dynamic edits and views across repeated shows.
    pub fn show_menu(&mut self, source: Entity, menu: GuiPopupMenu, position: Vec2) -> bool {
        if !position.is_finite() {
            return false;
        }
        self.request(source, PopupRequest::Menu { menu, position })
    }
    /// Hide this popup and every popup above it; cancel this owner's pending shows.
    /// Hidden views stay alive. Returns whether anything was hidden or canceled.
    pub fn hide(&mut self, popup: Entity) -> bool {
        let before = self.runtime.pending_popups.len();
        self.runtime
            .pending_popups
            .retain(|pending| pending.owner != popup);
        let canceled = before != self.runtime.pending_popups.len();
        let hidden = self
            .object(popup)
            .filter(|&object| self.runtime.window_index(object).is_none())
            .is_some_and(|object| self.runtime.hide_popup(Some(object)));
        canceled || hidden
    }
    /// Hide all current popups and cancel every pending show, immediately.
    pub fn hide_all(&mut self) {
        self.runtime.pending_popups.clear();
        self.runtime.hide_popup(None);
    }
    /// Reopen the retained menu without replacing its dynamic edits.
    pub fn reopen_menu(&mut self, position: Vec2) -> bool {
        if !position.is_finite() {
            return false;
        }
        self.viewport();
        self.runtime
            .reopen_popup_menu(Point::new(position.x, position.y))
            .is_some()
    }
    /// Destroy the retained native menu now and cancel pending menu shows.
    /// Bevy applies its view despawns as deferred work.
    pub fn dispose_menu(&mut self) {
        self.runtime
            .pending_popups
            .retain(|pending| !pending.request.is_menu());
        self.runtime.dispose_popup_menu(&mut self.commands);
    }
    /// Current native state, accepting an owner or control view.
    pub fn is_open(&self, entity: Entity) -> bool {
        self.object(entity)
            .is_some_and(|object| self.runtime.popup_stack().contains(&object))
    }
    pub fn is_pending(&self, owner: Entity) -> bool {
        self.runtime
            .pending_popups
            .iter()
            .any(|pending| pending.owner == owner)
    }
    pub fn is_menu_open(&self) -> bool {
        self.runtime.is_popup_menu_open()
    }

    fn source(&self, entity: Entity) -> Option<GuiOwnerTemplate> {
        self.roots
            .get(entity)
            .ok()
            .cloned()
            .map(GuiOwnerTemplate::Root)
            .or_else(|| {
                self.window_sources
                    .get(entity)
                    .ok()
                    .cloned()
                    .map(GuiOwnerTemplate::Window)
            })
    }
    fn object(&self, entity: Entity) -> Option<crate::core::object::GObjectId> {
        self.objects
            .get(entity)
            .ok()
            .map(|reference| reference.0)
            .or_else(|| match self.source(entity) {
                Some(GuiOwnerTemplate::Root(source)) => self
                    .runtime
                    .ecs_root_source_matches(entity, &source)
                    .then(|| self.runtime.ecs_root_object(entity))
                    .flatten(),
                Some(GuiOwnerTemplate::Window(source)) => self
                    .runtime
                    .ecs_window_source_matches(entity, &source)
                    .then(|| self.runtime.ecs_window_object(entity))
                    .flatten(),
                None => None,
            })
            .filter(|&object| self.runtime.store().contains(object))
    }
    fn viewport(&mut self) {
        crate::runtime::windows::viewport(&mut self.runtime, &self.windows, &self.settings);
    }
    fn request(&mut self, owner: Entity, request: PopupRequest) -> bool {
        let source = self.source(owner);
        if !request.is_menu() && matches!(source, Some(GuiOwnerTemplate::Window(_))) {
            return false;
        }
        let (recent_removal, removals_before) = match source.as_ref() {
            Some(GuiOwnerTemplate::Root(_)) => removal_status(&self.removed_roots, owner),
            Some(GuiOwnerTemplate::Window(_)) => removal_status(&self.removed_windows, owner),
            None => (false, 0),
        };
        if !recent_removal && self.object(owner).is_some() {
            // A materialized request must not later be overwritten by an older
            // request that was issued while its owner was loading.
            let accepted = self.execute(owner, &request);
            if accepted {
                self.cancel_replaced(owner, request.is_menu());
            }
            return accepted;
        }
        let Some(source) = source else {
            return false;
        };
        self.cancel_replaced(owner, request.is_menu());
        self.runtime.pending_popups.push(PendingPopup {
            owner,
            source,
            request,
            removals_before,
        });
        true
    }
    fn cancel_replaced(&mut self, owner: Entity, menu: bool) {
        self.runtime.pending_popups.retain(|pending| {
            if menu {
                !pending.request.is_menu()
            } else {
                pending.owner != owner || pending.request.is_menu()
            }
        });
    }
    fn execute(&mut self, owner: Entity, request: &PopupRequest) -> bool {
        let Some(object) = self.object(owner) else {
            return false;
        };
        if !request.is_menu() && self.runtime.window_index(object).is_some() {
            return false;
        }
        self.viewport();
        match request {
            PopupRequest::Show(position) => self
                .runtime
                .show_popup(object, Point::new(position.x, position.y)),
            PopupRequest::Anchored { anchor, direction } => {
                let Some(anchor) = self.object(*anchor) else {
                    return false;
                };
                let Some(origin) = self.runtime.store().local_to_global(anchor, Point::ZERO) else {
                    return false;
                };
                let target_size = self.runtime.store().object(anchor).unwrap().size();
                let popup_size = self.runtime.store().object(object).unwrap().size();
                let viewport = self
                    .runtime
                    .store()
                    .object(self.runtime.root_container().unwrap())
                    .unwrap()
                    .size();
                let mut x = origin.x;
                if x + popup_size.width > viewport.width {
                    x += target_size.width - popup_size.width;
                }
                let mut y = origin.y + target_size.height;
                if *direction == GuiPopupDirection::Up
                    || *direction == GuiPopupDirection::Auto
                        && origin.y + popup_size.height > viewport.height
                {
                    y = origin.y - popup_size.height - 1.;
                    if y < 0. {
                        y = 0.;
                        x += target_size.width * 0.5;
                    }
                }
                self.runtime.show_popup(object, Point::new(x, y))
            }
            PopupRequest::Menu { menu, position } => {
                let package = self
                    .runtime
                    .store()
                    .object(object)
                    .and_then(|object| object.source_package_id())
                    .and_then(|id| self.runtime.registry().get_by_id(id))
                    .cloned()
                    .or_else(|| {
                        self.runtime.asset_handle_for(object).and_then(|handle| {
                            self.assets
                                .get(&handle)
                                .map(|asset| std::sync::Arc::new(asset.package.clone()))
                        })
                    });
                let Some(package) = package else {
                    return false;
                };
                self.runtime
                    .show_popup_menu_items(
                        &mut self.commands,
                        &package,
                        PopupMenuTemplates {
                            content: &menu.content,
                            item: &menu.item,
                            separator: menu.separator.as_deref(),
                        },
                        &menu.items,
                        Point::new(position.x, position.y),
                    )
                    .is_some()
            }
        }
    }
}

/// Current popup view entities from bottom to top. Windows are separate.
/// Refreshed before Publish and after Sync; hidden retained objects remain alive.
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct GuiPopupState {
    stack: Vec<Entity>,
    menu: Option<Entity>,
}

impl GuiPopupState {
    pub fn stack(&self) -> &[Entity] {
        &self.stack
    }
    pub fn menu(&self) -> Option<Entity> {
        self.menu
    }
    pub fn is_open(&self, view: Entity) -> bool {
        self.stack.contains(&view)
    }
}

/// Materialize only asset-dependent shows, after ECS owners and property edits.
pub(crate) fn sync_pending_popups(mut popup: GuiPopup) {
    let removed_roots: Vec<_> = popup
        .removed_roots
        .read_with_id()
        .map(|(entity, id)| (entity, id.id))
        .collect();
    let removed_windows: Vec<_> = popup
        .removed_windows
        .read_with_id()
        .map(|(entity, id)| (entity, id.id))
        .collect();
    let waiting = std::mem::take(&mut popup.runtime.pending_popups);
    for pending in waiting {
        let removed = match pending.source {
            GuiOwnerTemplate::Root(_) => &removed_roots,
            GuiOwnerTemplate::Window(_) => &removed_windows,
        };
        if removed
            .iter()
            .any(|&(owner, id)| owner == pending.owner && id >= pending.removals_before)
            || popup
                .source(pending.owner)
                .as_ref()
                .is_none_or(|source| !source.same_template(&pending.source))
        {
            continue;
        }
        if popup.object(pending.owner).is_some() {
            popup.execute(pending.owner, &pending.request);
        } else if popup.runtime.ecs_root_pending(pending.owner)
            || popup.runtime.ecs_window_pending(pending.owner)
        {
            popup.runtime.pending_popups.push(pending);
        }
    }
}

pub(crate) fn sync_popup_state(runtime: NonSend<GuiRuntime>, mut state: ResMut<GuiPopupState>) {
    let stack = runtime.popup_stack().iter().filter_map(|&object| {
        runtime
            .store()
            .contains(object)
            .then(|| runtime.entity_of(object))
            .flatten()
    });
    if !state.stack.iter().copied().eq(stack.clone()) {
        state.stack = stack.collect();
    }
    let menu = runtime
        .popup_menu_content()
        .and_then(|object| runtime.entity_of(object));
    if state.menu != menu {
        state.menu = menu;
    }
}

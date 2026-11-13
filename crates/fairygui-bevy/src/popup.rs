//! Entity-addressed popup operations and the current popup stack.
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use fairygui_core::{
    geom::{Point, Size},
    popup::MenuItem,
};

use crate::{
    FairyGuiRoot, FairyGuiWindow, FuiAsset, GObjectRef, GuiInputSettings, GuiRootInstance,
    GuiRuntime, PopupMenuTemplates,
};

use crate::runtime::ObjectCommandSource;

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

/// Popup positions use logical pixels in the FairyGUI stage.
/// Targets and menu sources accept root owners or control view entities.
#[derive(Message, Debug, Clone)]
pub enum GuiPopupCommand {
    Show {
        popup: Entity,
        position: Vec2,
    },
    Hide {
        popup: Entity,
    },
    /// Hide all current popups and cancel pending show requests.
    HideAll,
    /// Resolve local templates in the source control's package.
    ShowMenu {
        source: Entity,
        menu: GuiPopupMenu,
        position: Vec2,
    },
    ReopenMenu {
        position: Vec2,
    },
    DisposeMenu,
}

impl GuiPopupCommand {
    fn target(&self) -> Option<Entity> {
        match self {
            Self::Show { popup, .. } | Self::Hide { popup } => Some(*popup),
            Self::ShowMenu { source, .. } => Some(*source),
            _ => None,
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_popup_commands(
    mut commands: Commands,
    mut messages: MessageReader<GuiPopupCommand>,
    objects: Query<&GObjectRef>,
    roots: Query<&GuiRootInstance>,
    sources: Query<&FairyGuiRoot>,
    window_sources: Query<&FairyGuiWindow>,
    mut removed: RemovedComponents<FairyGuiRoot>,
    mut removed_windows: RemovedComponents<FairyGuiWindow>,
    mut pending: Local<Vec<(ObjectCommandSource, GuiPopupCommand)>>,
    assets: Res<Assets<FuiAsset>>,
    windows: Query<(Entity, &Window, Has<PrimaryWindow>)>,
    settings: Res<GuiInputSettings>,
    mut runtime: NonSendMut<GuiRuntime>,
) {
    for owner in removed.read().chain(removed_windows.read()) {
        pending.retain(|(_, command)| command.target() != Some(owner));
    }
    let waiting = std::mem::take(&mut *pending);
    for (expected, command) in waiting
        .into_iter()
        .map(|(source, command)| (Some(source), command))
        .chain(messages.read().cloned().map(|command| (None, command)))
    {
        match &command {
            GuiPopupCommand::HideAll => {
                pending.clear();
                runtime.hide_popup(None);
                continue;
            }
            GuiPopupCommand::DisposeMenu => {
                pending.retain(|(_, command)| !matches!(command, GuiPopupCommand::ShowMenu { .. }));
                runtime.dispose_popup_menu(&mut commands);
                continue;
            }
            GuiPopupCommand::Hide { popup } => {
                pending.retain(|(_, command)| command.target() != Some(*popup));
            }
            _ => {}
        }
        let position = match &command {
            GuiPopupCommand::Show { position, .. }
            | GuiPopupCommand::ShowMenu { position, .. }
            | GuiPopupCommand::ReopenMenu { position } => Some(*position),
            _ => None,
        };
        if position.is_some_and(|position| !position.is_finite()) {
            continue;
        }
        let object = if let Some(target) = command.target() {
            let source = sources
                .get(target)
                .ok()
                .cloned()
                .map(ObjectCommandSource::Root)
                .or_else(|| {
                    window_sources
                        .get(target)
                        .ok()
                        .cloned()
                        .map(ObjectCommandSource::Window)
                });
            if let Some(expected) = expected
                && source
                    .as_ref()
                    .is_none_or(|source| !source.same_template(&expected))
            {
                continue;
            }
            let object = objects
                .get(target)
                .map(|reference| reference.0)
                .or_else(|_| roots.get(target).map(|root| root.object))
                .ok()
                .or_else(|| runtime.ecs_window_object(target));
            if object.is_none() {
                if !matches!(command, GuiPopupCommand::Hide { .. })
                    && let Some(source) = source
                    && (runtime.ecs_root_pending(target) || runtime.ecs_window_pending(target))
                {
                    pending.push((source, command));
                }
                continue;
            }
            object
        } else {
            None
        };
        if position.is_some() {
            if let Some((_, window, _)) = windows.iter().find(|(entity, _, primary)| {
                settings.window.map_or(*primary, |chosen| chosen == *entity)
            }) {
                runtime.ensure_root(Size::new(window.width(), window.height()));
            } else if runtime.root_container().is_none() {
                runtime.ensure_root(Size::new(1280., 720.));
            }
        }
        match command {
            GuiPopupCommand::Show { position, .. } => {
                if runtime.window_index(object.unwrap()).is_some() {
                    tracing::warn!("use GuiWindowCommand to show a native window");
                    continue;
                }
                runtime.show_popup(object.unwrap(), Point::new(position.x, position.y));
            }
            GuiPopupCommand::Hide { .. } => {
                if object.is_some_and(|object| runtime.window_index(object).is_some()) {
                    tracing::warn!("use GuiWindowCommand to hide a native window");
                    continue;
                }
                runtime.hide_popup(object);
            }
            GuiPopupCommand::ShowMenu { menu, position, .. } => {
                let object = object.unwrap();
                let package = runtime
                    .store()
                    .object(object)
                    .and_then(|object| object.source_package_id())
                    .and_then(|id| runtime.registry().get_by_id(id))
                    .cloned()
                    .or_else(|| {
                        runtime.asset_handle_for(object).and_then(|handle| {
                            assets
                                .get(&handle)
                                .map(|asset| std::sync::Arc::new(asset.package.clone()))
                        })
                    });
                if let Some(package) = package {
                    runtime.show_popup_menu_items(
                        &mut commands,
                        &package,
                        PopupMenuTemplates {
                            content: &menu.content,
                            item: &menu.item,
                            separator: menu.separator.as_deref(),
                        },
                        &menu.items,
                        Point::new(position.x, position.y),
                    );
                }
            }
            GuiPopupCommand::ReopenMenu { position } => {
                runtime.reopen_popup_menu(Point::new(position.x, position.y));
            }
            _ => {}
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

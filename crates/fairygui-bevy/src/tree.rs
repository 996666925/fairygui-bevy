//! Entity snapshots for exported tree nodes, including retained collapsed cells.
use crate::{GObjectRef, GuiRuntime, properties::Property};
use bevy::prelude::*;

#[cfg(test)]
#[path = "ecs_tree_content_tests.rs"]
mod content_tests;
#[cfg(test)]
#[path = "ecs_tree_tests.rs"]
mod tests;

/// One keyed node in application-owned tree content. Keys are unique across the
/// whole tree, keeping row entities stable during insertion and reordering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiTreeItem {
    pub key: String,
    pub title: String,
    /// Some assigns an icon (empty clears it); None leaves the skin/current icon.
    pub icon: Option<String>,
    /// Local template or registered ui:// URL; None uses the tree default.
    pub template: Option<String>,
    pub is_folder: bool,
    /// Some forces this value when content changes; None preserves existing
    /// expansion by key and initializes a new folder collapsed (native default).
    pub expanded: Option<bool>,
    pub children: Vec<GuiTreeItem>,
}
impl GuiTreeItem {
    pub fn leaf(key: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            title: title.into(),
            icon: None,
            template: None,
            is_folder: false,
            expanded: None,
            children: Vec::new(),
        }
    }
    pub fn folder(key: impl Into<String>, title: impl Into<String>, children: Vec<Self>) -> Self {
        Self {
            is_folder: true,
            children,
            ..Self::leaf(key, title)
        }
    }
}

/// Configure a tree view's data through ECS. Changed configurations reconcile
/// keyed rows; removing the component leaves the current tree content intact.
#[derive(Component, Debug, Default, Clone, PartialEq, Eq)]
pub struct GuiTreeContent {
    pub items: Vec<GuiTreeItem>,
}
impl GuiTreeContent {
    pub fn new(items: Vec<GuiTreeItem>) -> Self {
        Self { items }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GuiTreeContentError {
    #[error("tree content requires a Tree control")]
    NotTree,
    #[error("tree node key must not be empty")]
    EmptyKey,
    #[error("duplicate tree node key {0}")]
    DuplicateKey(String),
    #[error("leaf node {0} cannot have children")]
    LeafChildren(String),
    #[error("tree depth exceeds the native level range")]
    TooDeep,
    #[error("cannot resolve tree node template {0}")]
    MissingTemplate(String),
    #[error("cannot construct tree node template {0}")]
    CreationFailed(String),
    #[error("tree node template must be a component: {0}")]
    InvalidTemplate(String),
}
#[derive(Component, Debug, Default, Clone, PartialEq, Eq)]
pub struct GuiTreeContentState {
    error: Option<GuiTreeContentError>,
    object: Option<crate::core::GObjectId>,
    revision: Option<bevy::ecs::change_detection::Tick>,
}
impl GuiTreeContentState {
    pub fn error(&self) -> Option<&GuiTreeContentError> {
        self.error.as_ref()
    }
}

pub(crate) fn content_pending(
    world: &World,
    entity: Entity,
    object: crate::core::GObjectId,
) -> bool {
    let view = world.entity(entity);
    view.get_ref::<GuiTreeContent>().is_some_and(|config| {
        view.get::<GuiTreeContentState>().is_none_or(|state| {
            state.object != Some(object)
                || state.revision.is_none_or(|revision| {
                    config
                        .last_changed()
                        .is_newer_than(revision, world.read_change_tick())
                })
        })
    })
}

#[derive(Debug, Clone)]
pub(crate) struct ContentRow {
    pub key: String,
    pub template: String,
    pub object: crate::core::GObjectId,
    pub is_folder: bool,
}
#[derive(Component, Debug, Clone)]
pub(crate) struct ContentBinding {
    pub object: crate::core::GObjectId,
    pub rows: Vec<ContentRow>,
}

#[allow(clippy::type_complexity)]
pub(crate) fn sync_tree_content(
    mut commands: Commands,
    mut runtime: NonSendMut<GuiRuntime>,
    configs: Query<(
        Entity,
        &GObjectRef,
        Ref<GuiTreeContent>,
        Option<&ContentBinding>,
        Option<&GuiTreeContentState>,
    )>,
    removed: Query<Entity, (With<GuiTreeContentState>, Without<GuiTreeContent>)>,
) {
    for entity in &removed {
        commands.entity(entity).remove::<GuiTreeContentState>();
    }
    for (entity, reference, config, binding, current) in &configs {
        if runtime.entity_of(reference.0) != Some(entity) {
            continue;
        }
        if !config.is_changed() && current.is_some_and(|state| state.object == Some(reference.0)) {
            continue;
        }
        let result = runtime.configure_tree_content(reference.0, &config, binding);
        let error = match result {
            Ok(binding) => {
                commands.entity(entity).insert(binding);
                None
            }
            Err(error) => Some(error),
        };
        let next = GuiTreeContentState {
            error,
            object: Some(reference.0),
            revision: Some(config.last_changed()),
        };
        if current != Some(&next) {
            commands.entity(entity).insert(next);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiTreeNodeState {
    key: Option<String>,
    index: usize,
    parent: Option<usize>,
    level: u8,
    folder: bool,
    expanded: bool,
    visible: bool,
    view: Option<Entity>,
}
impl GuiTreeNodeState {
    /// Application content key, absent on unconfigured exported nodes.
    pub fn key(&self) -> Option<&str> {
        self.key.as_deref()
    }
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn parent(&self) -> Option<usize> {
        self.parent
    }
    /// Zero-based exported level, excluding the implicit root.
    pub fn level(&self) -> u8 {
        self.level
    }
    pub fn is_folder(&self) -> bool {
        self.folder
    }
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }
    /// Membership in the expanded tree, independent of viewport clipping.
    pub fn is_visible(&self) -> bool {
        self.visible
    }
    pub fn view(&self) -> Option<Entity> {
        self.view
    }
}

/// Read-only exported tree hierarchy. Logical indices remain stable across
/// collapse/expand; collapsed cells keep their model and view entities.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct GuiTreeState {
    nodes: Vec<GuiTreeNodeState>,
    visible: Vec<usize>,
    selected: Vec<usize>,
    indent: i32,
    click_to_expand: i32,
}
impl GuiTreeState {
    pub fn node_by_key(&self, key: &str) -> Option<&GuiTreeNodeState> {
        self.nodes.iter().find(|node| node.key() == Some(key))
    }
    pub fn nodes(&self) -> &[GuiTreeNodeState] {
        &self.nodes
    }
    pub fn node(&self, index: usize) -> Option<&GuiTreeNodeState> {
        self.nodes.get(index)
    }
    pub fn visible_nodes(&self) -> &[usize] {
        &self.visible
    }
    pub fn selected_nodes(&self) -> &[usize] {
        &self.selected
    }
    pub fn indent(&self) -> i32 {
        self.indent
    }
    pub fn click_to_expand(&self) -> i32 {
        self.click_to_expand
    }
}

/// Logical tree membership on a retained cell, including collapsed descendants.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuiTreeNode {
    tree: Entity,
    index: usize,
    /// Writable native expanded state. Non-folder rows remain expanded.
    pub expanded: bool,
}
impl GuiTreeNode {
    pub fn tree(&self) -> Entity {
        self.tree
    }
    pub fn index(&self) -> usize {
        self.index
    }
}

impl Property for GuiTreeNode {
    fn read(runtime: &GuiRuntime, object: crate::core::GObjectId) -> Option<Self> {
        let (tree, row, index) = runtime.store().list_item_of(object)?;
        if row != object {
            return None;
        }
        let crate::core::widget::ControlData::Tree(data) = runtime.store().object(tree)?.control()
        else {
            return None;
        };
        Some(Self {
            tree: runtime.entity_of(tree)?,
            index,
            expanded: data.is_expanded(index),
        })
    }

    fn apply(
        &self,
        previous: &Self,
        native: &Self,
        runtime: &mut GuiRuntime,
        object: crate::core::GObjectId,
    ) {
        if self.expanded == previous.expanded || self.expanded == native.expanded {
            return;
        }
        let Some((tree, _, index)) = runtime.store().list_item_of(object) else {
            return;
        };
        runtime
            .store_mut()
            .set_tree_node_expanded(tree, index, self.expanded);
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn sync_tree_state(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    objects: Query<(
        Entity,
        &GObjectRef,
        Option<&GuiTreeState>,
        Option<&GuiTreeNode>,
        Option<&ContentBinding>,
    )>,
) {
    let mut rows = std::collections::HashMap::new();
    for (entity, reference, current, _, binding) in &objects {
        if runtime.entity_of(reference.0) != Some(entity) {
            continue;
        }
        let Some(crate::core::widget::ControlData::Tree(data)) =
            runtime.store().object(reference.0).map(|o| o.control())
        else {
            if current.is_some() {
                commands.entity(entity).remove::<GuiTreeState>();
            }
            continue;
        };
        let visible = data.visible_nodes();
        let cells = runtime.store().list_items(reference.0).unwrap_or_default();
        let mut ancestors: Vec<(u8, usize)> = Vec::new();
        let nodes = data
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                while ancestors
                    .last()
                    .is_some_and(|(level, _)| *level >= node.level)
                {
                    ancestors.pop();
                }
                let parent = ancestors.last().map(|(_, index)| *index);
                if node.is_folder {
                    ancestors.push((node.level, index));
                }
                let view = cells.get(index).and_then(|&id| runtime.entity_of(id));
                if let Some(view) = view {
                    rows.insert(
                        view,
                        GuiTreeNode {
                            tree: entity,
                            index,
                            expanded: data.is_expanded(index),
                        },
                    );
                }
                GuiTreeNodeState {
                    key: binding
                        .filter(|binding| binding.object == reference.0)
                        .and_then(|binding| binding.rows.get(index))
                        .map(|row| row.key.clone()),
                    index,
                    parent,
                    level: node.level,
                    folder: node.is_folder,
                    expanded: data.is_expanded(index),
                    visible: visible.contains(&index),
                    view,
                }
            })
            .collect();
        let next = GuiTreeState {
            nodes,
            visible,
            selected: runtime
                .store()
                .list_selections(reference.0)
                .unwrap_or_default(),
            indent: data.indent,
            click_to_expand: data.click_to_expand,
        };
        if current != Some(&next) {
            commands.entity(entity).insert(next);
        }
    }
    for (entity, _, _, current, _) in &objects {
        match rows.get(&entity) {
            Some(next) if current != Some(next) => {
                commands.entity(entity).insert(*next);
            }
            None if current.is_some() => {
                commands.entity(entity).remove::<GuiTreeNode>();
            }
            _ => {}
        }
    }
}

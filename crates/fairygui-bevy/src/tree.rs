//! Entity snapshots for exported tree nodes, including retained collapsed cells.
use crate::{GObjectRef, GuiRuntime};
use bevy::prelude::*;

#[cfg(test)]
#[path = "ecs_tree_tests.rs"]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiTreeNodeState {
    index: usize,
    parent: Option<usize>,
    level: u8,
    folder: bool,
    expanded: bool,
    visible: bool,
    view: Option<Entity>,
}
impl GuiTreeNodeState {
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
}
impl GuiTreeNode {
    pub fn tree(&self) -> Entity {
        self.tree
    }
    pub fn index(&self) -> usize {
        self.index
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
    )>,
) {
    let mut rows = std::collections::HashMap::new();
    for (entity, reference, current, _) in &objects {
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
                        },
                    );
                }
                GuiTreeNodeState {
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
    for (entity, _, _, current) in &objects {
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

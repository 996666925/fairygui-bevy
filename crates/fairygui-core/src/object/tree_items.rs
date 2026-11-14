//! Retained tree cells and exported node state, without renderer dependencies.
use super::*;
use crate::widget::ControlData;

impl ObjectStore {
    /// GTree.createCell samples indent once. Keyed reparenting to a new level
    /// represents a new native cell binding; ordinary layout/collapse retains it.
    pub(super) fn prepare_tree_indents(&mut self, tree: GObjectId) {
        let Some(ControlData::Tree(data)) = self.object(tree).map(|o| o.control()) else {
            return;
        };
        let indent = data.indent;
        let levels: Vec<_> = data.nodes.iter().map(|n| n.level).collect();
        for (index, level) in levels.into_iter().enumerate() {
            let Some(row) = self.list_item_at(tree, index) else {
                continue;
            };
            let Some(part) = self.child_by_name(row, "indent") else {
                continue;
            };
            if self.tree_cell_indents.get(&row) == Some(&(tree, level)) {
                continue;
            }
            let height = self.objects[part].height();
            self.set_size(part, f32::from(level) * indent as f32, height);
            self.tree_cell_indents.insert(row, (tree, level));
        }
    }
    /// Replace the logical preorder description after the adapter has prepared
    /// its cells. Validate all metadata before mutating the tree.
    pub fn replace_tree_nodes(
        &mut self,
        tree: GObjectId,
        nodes: Vec<crate::widget::TreeNodeData>,
        items: Vec<crate::widget::ListItemData>,
        expanded: Vec<bool>,
    ) -> bool {
        if nodes.len() != items.len() || nodes.len() != expanded.len() {
            return false;
        }
        let mut previous: Option<&crate::widget::TreeNodeData> = None;
        for node in &nodes {
            if previous.is_none() && node.level != 0 {
                return false;
            }
            if let Some(previous) = previous
                && (u16::from(node.level) > u16::from(previous.level) + 1
                    || (node.level > previous.level && !previous.is_folder))
            {
                return false;
            }
            previous = Some(node);
        }
        let Some(ControlData::Tree(data)) = self.objects.get_mut(tree).map(|o| &mut o.control)
        else {
            return false;
        };
        data.nodes = nodes;
        data.list.items = items;
        data.expanded = expanded;
        true
    }

    pub fn set_tree_node_expanded(
        &mut self,
        tree: GObjectId,
        index: usize,
        expanded: bool,
    ) -> bool {
        let changed = match self.objects.get_mut(tree).map(|object| &mut object.control) {
            Some(ControlData::Tree(data)) => data.set_expanded(index, expanded),
            _ => return false,
        };
        if changed {
            self.refresh_tree_visibility(tree);
        }
        changed
    }

    /// Apply native leaf/expanded controllers and indent-part widths after cell
    /// construction. Collapsed rows retain their identities but do not take space.
    pub fn initialize_tree_cells(&mut self, tree: GObjectId) {
        if let Some(ControlData::Tree(data)) = self.objects.get_mut(tree).map(|o| &mut o.control) {
            data.list.fold_invisible_items = true;
        } else {
            return;
        }
        self.refresh_tree_visibility(tree);
    }

    pub(super) fn sync_tree_cells(&mut self, tree: GObjectId) {
        let Some(ControlData::Tree(data)) = self.object(tree).map(|o| o.control()) else {
            return;
        };
        let states: Vec<_> = data
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.is_folder, data.is_expanded(index)))
            .collect();
        for (index, (folder, expanded)) in states.into_iter().enumerate() {
            let Some(row) = self.list_item_at(tree, index) else {
                continue;
            };
            self.set_controller_page_index(row, "leaf", i32::from(!folder));
            self.set_controller_page_index(row, "expanded", i32::from(expanded));
        }
    }

    pub(super) fn sync_tree_controller(&mut self, row: GObjectId, name: &str) {
        if name != "expanded" {
            return;
        }
        let Some(tree) = self.object(row).and_then(|object| object.parent()) else {
            return;
        };
        let Some(index) = self.list_item_index(tree, row) else {
            return;
        };
        let expanded = self
            .object(row)
            .and_then(|o| o.control().component())
            .and_then(|c| c.controller("expanded"))
            .is_some_and(|c| c.selected_index == 1);
        self.set_tree_node_expanded(tree, index, expanded);
    }

    /// Parsed item overrides target the compound cell, retaining native title
    /// and icon properties as well as the visible title/icon parts.
    pub fn apply_tree_item(&mut self, tree: GObjectId, index: usize) {
        let Some(ControlData::Tree(data)) = self.object(tree).map(|o| o.control()) else {
            return;
        };
        let Some(item) = data.list.items.get(index).cloned() else {
            return;
        };
        let Some(row) = self.list_item_at(tree, index) else {
            return;
        };
        if let Some(title) = &item.title {
            self.set_object_text(row, title);
        }
        if let Some(icon) = &item.icon {
            self.apply_icon_to(row, icon);
        }
        if let Some(name) = &item.name {
            self.set_name(row, name);
        }
        if let Some(ControlData::Button(data)) = self.objects.get_mut(row).map(|o| &mut o.control) {
            if let Some(title) = &item.selected_title {
                data.selected_title = Some(title.clone());
            }
            if let Some(icon) = &item.selected_icon {
                data.selected_icon = Some(icon.clone());
            }
        }
        self.apply_title(row);
        for (controller, page) in &item.controller_pages {
            self.set_controller_page_id(row, controller, page);
        }
    }
}

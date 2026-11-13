//! GList selection: live buttons override retained offscreen virtual records.
use super::{GObjectId, ObjectStore};
use crate::widget::{ControlData, ListData, ListSelectionMode};

#[cfg(test)]
#[path = "list_selection_tests.rs"]
mod tests;

impl ObjectStore {
    pub(super) fn list_data(&self, list: GObjectId) -> Option<&ListData> {
        match self.object(list)?.control() {
            ControlData::List(data) => Some(data),
            ControlData::Tree(data) => Some(&data.list),
            _ => None,
        }
    }

    pub(super) fn list_data_mut(&mut self, list: GObjectId) -> Option<&mut ListData> {
        match &mut self.objects.get_mut(list)?.control {
            ControlData::List(data) => Some(data),
            ControlData::Tree(data) => Some(&mut data.list),
            _ => None,
        }
    }

    /// Selected row indices in current child order. Non-button/common rows do
    /// not participate. Unlike a cached index, this follows reordering and direct
    /// button selection changes immediately.
    pub fn list_selections(&self, list: GObjectId) -> Option<Vec<usize>> {
        self.list_data(list)?;
        if let Some(state) = self.virtual_states.get(&list) {
            let mut result = Vec::new();
            for index in state
                .entries
                .iter()
                .take(state.geometry.count)
                .enumerate()
                .filter_map(|(index, row)| {
                    let selected = row
                        .object
                        .map_or(row.selected, |id| self.button_selected(id) == Some(true));
                    selected.then_some(index)
                })
            {
                let index = if state.looping && state.num_items > 0 {
                    index % state.num_items
                } else {
                    index
                };
                if !state.looping || !result.contains(&index) {
                    result.push(index);
                }
            }
            return Some(result);
        }
        Some(
            self.list_items(list)?
                .into_iter()
                .enumerate()
                .filter_map(|(i, id)| (self.button_selected(id) == Some(true)).then_some(i))
                .collect(),
        )
    }

    /// First selected row, or -1. None means this object is not a list/tree.
    pub fn list_selection(&self, list: GObjectId) -> Option<i32> {
        Some(
            self.list_selections(list)?
                .first()
                .map_or(-1, |i| *i as i32),
        )
    }

    pub(super) fn refresh_list_selection_snapshot(&mut self, list: GObjectId) {
        if self.list_selection_batches.contains(&list) {
            return;
        }
        let Some(indices) = self.list_selections(list) else {
            return;
        };
        let data = self.list_data_mut(list).unwrap();
        data.selected_index = indices.first().map_or(-1, |i| *i as i32);
        data.selected_indices = indices;
    }

    fn finish_list_selection(&mut self, list: GObjectId, outer: bool) {
        if outer {
            self.list_selection_batches.remove(&list);
            self.refresh_list_selection_snapshot(list);
        }
    }

    pub(super) fn initialize_list_item(&mut self, list: GObjectId, item: GObjectId) {
        if self.list_data(list).is_some() && !self.is_scroll_overlay(item) {
            self.set_button_selected(item, false);
            self.set_button_change_state_on_click(item, false);
        }
    }

    /// Change the mode without clearing existing selection (native setter).
    pub fn set_list_selection_mode(&mut self, list: GObjectId, mode: ListSelectionMode) -> bool {
        let Some(data) = self.list_data_mut(list) else {
            return false;
        };
        if data.selection_mode == mode {
            return false;
        }
        data.selection_mode = mode;
        true
    }

    /// Bind a controller on the list's parent; assigning does not select a row.
    pub fn set_list_selection_controller(
        &mut self,
        list: GObjectId,
        controller: Option<&str>,
    ) -> bool {
        let index = if let Some(name) = controller {
            let Some(index) = self
                .object(list)
                .and_then(|o| o.parent())
                .and_then(|p| self.object(p))
                .and_then(|o| o.control().component())
                .and_then(|c| c.controllers.iter().position(|c| c.name == name))
            else {
                return false;
            };
            index as i32
        } else {
            -1
        };
        let Some(data) = self.list_data_mut(list) else {
            return false;
        };
        if data.selection_controller == index {
            return false;
        }
        data.selection_controller = index;
        true
    }

    /// Native selectedIndex setter: replace the complete selection. Invalid
    /// indices clear it; non-button rows cannot acquire a selection.
    pub fn set_list_selection(&mut self, list: GObjectId, index: i32) -> bool {
        let Some(before) = self.list_selections(list) else {
            return false;
        };
        let outer = self.list_selection_batches.insert(list);
        let valid = usize::try_from(index)
            .ok()
            .filter(|i| *i < self.list_num_items(list));
        if let Some(index) = valid {
            if self.list_data(list).unwrap().selection_mode != ListSelectionMode::Single {
                self.clear_list_selection(list);
            }
            self.add_list_selection(list, index);
        } else {
            self.clear_list_selection(list);
        }
        self.finish_list_selection(list, outer);
        self.list_selections(list)
            .is_some_and(|after| after != before)
    }

    /// Add a row to the selection; Single clears other rows. No StateChanged
    /// event is emitted by programmatic selection, matching button.selected.
    pub fn add_list_selection(&mut self, list: GObjectId, index: usize) -> bool {
        self.add_list_selection_with_scroll(list, index, false)
    }

    pub fn add_list_selection_with_scroll(
        &mut self,
        list: GObjectId,
        index: usize,
        scroll_to_view: bool,
    ) -> bool {
        let Some(data) = self.list_data(list) else {
            return false;
        };
        let mode = data.selection_mode;
        if mode == ListSelectionMode::None {
            return false;
        }
        if index >= self.list_num_items(list) {
            return false;
        }
        let before = self.list_selections(list);
        let outer = self.list_selection_batches.insert(list);
        if mode == ListSelectionMode::Single {
            self.clear_list_selection(list);
        }
        if scroll_to_view {
            self.scroll_list_item_to_view(list, index, false, false);
        }
        self.list_data_mut(list).unwrap().last_selected_index = Some(index);
        if self.set_list_record_selected(list, index, true) {
            self.update_list_selection_controller(list, index);
        }
        self.finish_list_selection(list, outer);
        self.list_selections(list) != before
    }

    pub fn remove_list_selection(&mut self, list: GObjectId, index: usize) -> bool {
        if self
            .list_data(list)
            .is_none_or(|d| d.selection_mode == ListSelectionMode::None)
        {
            return false;
        }
        let before = self.list_selections(list);
        self.set_list_record_selected(list, index, false);
        self.refresh_list_selection_snapshot(list);
        self.list_selections(list) != before
    }

    /// Returns whether a live button changed; controller feedback is deliberately
    /// absent for an offscreen-only change, matching native addSelection.
    fn set_list_record_selected(&mut self, list: GObjectId, index: usize, selected: bool) -> bool {
        let item = if let Some(state) = self.virtual_states.get_mut(&list) {
            if index >= state.geometry.count {
                return false;
            }
            let row = &mut state.entries[index];
            row.selected = selected;
            row.object
        } else {
            self.list_item_at(list, index)
        };
        item.is_some_and(|item| self.set_button_selected(item, selected))
    }

    /// Clear even in None mode. The Shift anchor is retained.
    pub fn clear_list_selection(&mut self, list: GObjectId) -> bool {
        self.clear_list_selection_except(list, None)
    }

    fn clear_list_selection_except(&mut self, list: GObjectId, keep: Option<GObjectId>) -> bool {
        if self.list_data(list).is_none() {
            return false;
        }
        let outer = self.list_selection_batches.insert(list);
        if let Some(state) = self.virtual_states.get(&list) {
            let indices: Vec<_> = state
                .entries
                .iter()
                .take(state.geometry.count)
                .enumerate()
                .filter_map(|(index, row)| {
                    (!(keep.is_some() && row.object == keep)).then_some(index)
                })
                .collect();
            let before = self.list_selections(list);
            for index in indices {
                self.set_list_record_selected(list, index, false);
            }
            self.finish_list_selection(list, outer);
            return self.list_selections(list) != before;
        }
        let children = self.list_items(list).unwrap();
        let mut changed = false;
        for item in children {
            if Some(item) != keep {
                changed |= self.set_button_selected(item, false);
            }
        }
        self.finish_list_selection(list, outer);
        changed
    }

    /// Native selectAll/selectReverse operate even in Single and None modes.
    pub fn select_all_list_items(&mut self, list: GObjectId) -> bool {
        self.select_list_bulk(list, false)
    }

    pub fn reverse_list_selection(&mut self, list: GObjectId) -> bool {
        self.select_list_bulk(list, true)
    }

    fn select_list_bulk(&mut self, list: GObjectId, reverse: bool) -> bool {
        if self.list_data(list).is_none() {
            return false;
        }
        let outer = self.list_selection_batches.insert(list);
        if let Some(state) = self.virtual_states.get(&list) {
            let before = self.list_selections(list);
            let count = state.geometry.count;
            let mut last = None;
            for index in 0..count {
                let row = self.virtual_states[&list].entries[index].clone();
                if let Some(item) = row.object {
                    let selected = !reverse || self.button_selected(item) != Some(true);
                    if self.set_button_selected(item, selected) && selected {
                        last = Some(index);
                    }
                }
                self.virtual_states.get_mut(&list).unwrap().entries[index].selected =
                    !reverse || !row.selected;
            }
            if let Some(last) = last {
                self.update_list_selection_controller(list, last);
            }
            self.finish_list_selection(list, outer);
            return self.list_selections(list) != before;
        }
        let children = self.list_items(list).unwrap();
        let mut changed = false;
        let mut last = None;
        for (index, item) in children.into_iter().enumerate() {
            let selected = !reverse || self.button_selected(item) != Some(true);
            if self.set_button_selected(item, selected) {
                changed = true;
                if selected {
                    last = Some(index);
                }
            }
        }
        if let Some(last) = last {
            self.update_list_selection_controller(list, last);
        }
        self.finish_list_selection(list, outer);
        changed
    }

    /// Nearest list row containing a hit, including hits on the row's children.
    /// Returns (list, row, current child index).
    pub fn list_item_of(&self, target: GObjectId) -> Option<(GObjectId, GObjectId, usize)> {
        let mut item = target;
        while let Some(parent) = self.object(item)?.parent() {
            if self.list_data(parent).is_some() {
                let index = if self.is_virtual_list(parent) {
                    self.virtual_item_index(parent, item)?
                } else {
                    self.list_item_index(parent, item)?
                };
                return Some((parent, item, index));
            }
            item = parent;
        }
        None
    }

    pub fn select_clicked_item_with_modifiers(
        &mut self,
        target: GObjectId,
        shift: bool,
        ctrl: bool,
    ) -> bool {
        let Some((list, item, index)) = self.list_item_of(target) else {
            return false;
        };
        let data = self.list_data(list).unwrap();
        let mode = data.selection_mode;
        let anchor = data.last_selected_index;
        if mode == ListSelectionMode::None || self.button_selected(item).is_none() {
            return false;
        }
        let before = self.list_selections(list);
        let outer = self.list_selection_batches.insert(list);
        let selected = self.button_selected(item) == Some(true);
        let mut retain_anchor = false;
        if mode == ListSelectionMode::Single {
            if !selected {
                self.clear_list_selection_except(list, Some(item));
                self.set_button_selected(item, true);
            }
        } else if shift {
            if !selected {
                if let Some(anchor) = anchor {
                    let end = anchor
                        .max(index)
                        .min(self.list_num_items(list).saturating_sub(1));
                    for index in anchor.min(index)..=end {
                        self.set_list_record_selected(list, index, true);
                    }
                    retain_anchor = true;
                } else {
                    self.set_button_selected(item, true);
                }
            }
        } else if ctrl || mode == ListSelectionMode::MultipleSingleClick {
            self.set_button_selected(item, !selected);
        } else {
            self.clear_list_selection_except(list, Some(item));
            self.set_button_selected(item, true);
        }
        if !retain_anchor {
            self.list_data_mut(list).unwrap().last_selected_index = Some(index);
        }
        if self.button_selected(item) == Some(true) {
            self.update_list_selection_controller(list, index);
        }
        self.finish_list_selection(list, outer);
        self.list_selections(list) != before
    }

    fn update_list_selection_controller(&mut self, list: GObjectId, index: usize) {
        let Some((parent, name)) = self.list_controller(list) else {
            return;
        };
        if self.changing_controllers.contains(&(parent, name.clone()))
            || self.syncing_list_controllers.contains(&list)
            || self.objects[parent]
                .control
                .component()
                .and_then(|c| c.controller(&name))
                .is_none_or(|c| index >= c.page_count())
        {
            return;
        }
        self.syncing_list_controllers.insert(list);
        self.set_controller_page_index(parent, &name, index as i32);
        self.syncing_list_controllers.remove(&list);
    }

    fn list_controller(&self, list: GObjectId) -> Option<(GObjectId, String)> {
        let data = self.list_data(list)?;
        let index = usize::try_from(data.selection_controller).ok()?;
        let parent = self.object(list)?.parent()?;
        Some((
            parent,
            self.object(parent)?
                .control()
                .component()?
                .controllers
                .get(index)?
                .name
                .clone(),
        ))
    }

    pub(super) fn sync_related_lists(&mut self, parent: GObjectId, name: &str) {
        let Some(controller) = self
            .objects
            .get(parent)
            .and_then(|o| o.control.component())
            .and_then(|c| c.controller(name))
        else {
            return;
        };
        let index = controller.selected_index;
        let lists: Vec<_> = self.objects[parent]
            .children
            .iter()
            .copied()
            .filter(|&list| {
                !self.syncing_list_controllers.contains(&list)
                    && self
                        .list_controller(list)
                        .is_some_and(|(p, n)| p == parent && n == name)
            })
            .collect();
        for list in lists {
            self.set_list_selection(list, index);
        }
    }

    /// Complete initial controller selection after an adapter materializes rows.
    /// Does not rerun controller actions or touch unrelated list subtrees.
    pub fn initialize_list_selections(&mut self, root: GObjectId) {
        let mut lists = Vec::new();
        if self.list_data(root).is_some() {
            lists.push(root);
        }
        self.for_each_descendant(root, &mut |id| {
            if self.list_data(id).is_some() {
                lists.push(id);
            }
        });
        for list in lists {
            self.refresh_list_selection_snapshot(list);
            let Some((parent, name)) = self.list_controller(list) else {
                continue;
            };
            let index = self.objects[parent]
                .control
                .component()
                .unwrap()
                .controller(&name)
                .unwrap()
                .selected_index;
            self.set_list_selection(list, index);
        }
    }
}

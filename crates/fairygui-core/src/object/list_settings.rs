use super::{GObjectId, ObjectStore};
use crate::widget::{ControlData, ListSelectionMode};

/// Live list policies unrelated to its layout or selected rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListInteractionSettings {
    pub selection_mode: ListSelectionMode,
    /// Controller on the list's parent. Assignment does not select a row.
    pub selection_controller: Option<String>,
    /// Scroll the list's own pane when a materialized row is clicked.
    pub scroll_item_to_view_on_click: bool,
    /// Remove invisible rows from layout while retaining their logical indices.
    pub fold_invisible_items: bool,
}
impl Default for ListInteractionSettings {
    fn default() -> Self {
        Self {
            selection_mode: ListSelectionMode::Single,
            selection_controller: None,
            scroll_item_to_view_on_click: true,
            fold_invisible_items: false,
        }
    }
}

impl ObjectStore {
    pub fn list_interaction_settings(&self, list: GObjectId) -> Option<ListInteractionSettings> {
        let data = match self.object(list)?.control() {
            ControlData::List(data) => &**data,
            ControlData::Tree(data) => &data.list,
            _ => return None,
        };
        Some(ListInteractionSettings {
            selection_mode: data.selection_mode,
            selection_controller: self.list_selection_controller(list)?,
            scroll_item_to_view_on_click: data.scroll_item_to_view_on_click,
            fold_invisible_items: data.fold_invisible_items,
        })
    }

    /// Apply native interaction policies and relayout once if folding changes.
    /// An unresolved controller name retains the previous binding; the other
    /// fields still apply. Mode/binding assignments preserve current selection.
    pub fn set_list_interaction_settings(
        &mut self,
        list: GObjectId,
        settings: ListInteractionSettings,
    ) -> bool {
        let Some(data) = self.list_data_mut(list) else {
            return false;
        };
        let layout_changed = data.fold_invisible_items != settings.fold_invisible_items;
        let mut changed = layout_changed
            || data.scroll_item_to_view_on_click != settings.scroll_item_to_view_on_click;
        data.fold_invisible_items = settings.fold_invisible_items;
        data.scroll_item_to_view_on_click = settings.scroll_item_to_view_on_click;
        changed |= self.set_list_selection_mode(list, settings.selection_mode);
        changed |=
            self.set_list_selection_controller(list, settings.selection_controller.as_deref());
        if layout_changed {
            self.layout_list(list);
        }
        changed
    }
}

#[cfg(test)]
#[path = "list_settings_tests.rs"]
mod tests;

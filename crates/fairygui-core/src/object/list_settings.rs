use super::{GObjectId, ObjectStore};
use crate::widget::ControlData;

/// Live list policies unrelated to its layout or selected rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListInteractionSettings {
    /// Scroll the list's own pane when a materialized row is clicked.
    pub scroll_item_to_view_on_click: bool,
    /// Remove invisible rows from layout while retaining their logical indices.
    pub fold_invisible_items: bool,
}
impl Default for ListInteractionSettings {
    fn default() -> Self {
        Self {
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
            scroll_item_to_view_on_click: data.scroll_item_to_view_on_click,
            fold_invisible_items: data.fold_invisible_items,
        })
    }

    /// Apply both list policies and relayout once if folding changes.
    pub fn set_list_interaction_settings(
        &mut self,
        list: GObjectId,
        settings: ListInteractionSettings,
    ) -> bool {
        let Some(data) = self.list_data_mut(list) else {
            return false;
        };
        let layout_changed = data.fold_invisible_items != settings.fold_invisible_items;
        let changed = layout_changed
            || data.scroll_item_to_view_on_click != settings.scroll_item_to_view_on_click;
        if !changed {
            return false;
        }
        data.fold_invisible_items = settings.fold_invisible_items;
        data.scroll_item_to_view_on_click = settings.scroll_item_to_view_on_click;
        if layout_changed {
            self.layout_list(list);
        }
        true
    }
}

#[cfg(test)]
#[path = "list_settings_tests.rs"]
mod tests;

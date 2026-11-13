//! Read-only combo state. Mutate through entity-addressed GuiObjectCommand.
use crate::GuiPopupDirection;
use bevy::prelude::*;

#[cfg(test)]
#[path = "ecs_combo_tests.rs"]
mod tests;

/// Snapshot on every combo view, refreshed before input events publish and
/// after Sync. Programmatic setters do not emit a user StateChanged event.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct GuiComboBoxState {
    pub(crate) items: Vec<String>,
    pub(crate) values: Vec<String>,
    pub(crate) icons: Option<Vec<Option<String>>>,
    pub(crate) selected_index: i32,
    pub(crate) visible_item_count: i32,
    pub(crate) direction: GuiPopupDirection,
    pub(crate) selection_controller: Option<String>,
    pub(crate) dropdown: Option<Entity>,
    pub(crate) list: Option<Entity>,
}
impl GuiComboBoxState {
    pub fn items(&self) -> &[String] {
        &self.items
    }
    pub fn values(&self) -> &[String] {
        &self.values
    }
    pub fn icons(&self) -> Option<&[Option<String>]> {
        self.icons.as_deref()
    }
    /// Native requested index, including an out-of-range programmatic value.
    pub fn selected_index(&self) -> i32 {
        self.selected_index
    }
    pub fn selected_item(&self) -> Option<&str> {
        self.items
            .get(usize::try_from(self.selected_index).ok()?)
            .map(String::as_str)
    }
    pub fn value(&self) -> Option<&str> {
        self.values
            .get(usize::try_from(self.selected_index).ok()?)
            .map(String::as_str)
    }
    pub fn visible_item_count(&self) -> i32 {
        self.visible_item_count
    }
    pub fn popup_direction(&self) -> GuiPopupDirection {
        self.direction
    }
    pub fn selection_controller(&self) -> Option<&str> {
        self.selection_controller.as_deref()
    }
    pub fn is_open(&self) -> bool {
        self.dropdown.is_some()
    }
    pub fn dropdown(&self) -> Option<Entity> {
        self.dropdown
    }
    pub fn list(&self) -> Option<Entity> {
        self.list
    }
}

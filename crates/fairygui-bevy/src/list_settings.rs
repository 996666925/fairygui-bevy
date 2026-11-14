//! Writable list interaction policies, separate from row layout and selection.
use crate::{
    GuiRuntime,
    core::object::{GObjectId, ListInteractionSettings},
    properties::Property,
};
use bevy::prelude::*;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuiListSettings {
    pub scroll_item_to_view_on_click: bool,
    pub fold_invisible_items: bool,
}
impl Default for GuiListSettings {
    fn default() -> Self {
        Self {
            scroll_item_to_view_on_click: true,
            fold_invisible_items: false,
        }
    }
}

impl Property for GuiListSettings {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        runtime
            .store()
            .list_interaction_settings(object)
            .map(|s| Self {
                scroll_item_to_view_on_click: s.scroll_item_to_view_on_click,
                fold_invisible_items: s.fold_invisible_items,
            })
    }

    fn apply(&self, previous: &Self, native: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        let settings = ListInteractionSettings {
            scroll_item_to_view_on_click: if self.scroll_item_to_view_on_click
                != previous.scroll_item_to_view_on_click
            {
                self.scroll_item_to_view_on_click
            } else {
                native.scroll_item_to_view_on_click
            },
            fold_invisible_items: if self.fold_invisible_items != previous.fold_invisible_items {
                self.fold_invisible_items
            } else {
                native.fold_invisible_items
            },
        };
        runtime
            .store_mut()
            .set_list_interaction_settings(object, settings);
    }
}

#[cfg(test)]
#[path = "ecs_list_settings_tests.rs"]
mod tests;

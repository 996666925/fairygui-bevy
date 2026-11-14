//! Writable tree-wide policies, separate from per-node expansion and selection.
use crate::{
    GuiRuntime,
    core::object::{GObjectId, TreeSettings},
    properties::Property,
};
use bevy::prelude::*;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuiTreeSettings {
    /// Sampled when a cell is constructed. Retained cell geometry is preserved.
    pub indent: i32,
    /// 0 arrows only, 1 folder row toggles, 2 retains native inert behavior.
    pub click_to_expand: i32,
}
impl Default for GuiTreeSettings {
    fn default() -> Self {
        Self {
            indent: 15,
            click_to_expand: 0,
        }
    }
}

impl Property for GuiTreeSettings {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        runtime.store().tree_settings(object).map(|settings| Self {
            indent: settings.indent,
            click_to_expand: settings.click_to_expand,
        })
    }

    fn apply(&self, previous: &Self, native: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        let settings = TreeSettings {
            indent: if self.indent != previous.indent {
                self.indent
            } else {
                native.indent
            },
            click_to_expand: if self.click_to_expand != previous.click_to_expand {
                self.click_to_expand
            } else {
                native.click_to_expand
            },
        };
        runtime.store_mut().set_tree_settings(object, settings);
    }
}

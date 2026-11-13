//! Writable layout shared by native lists, trees and virtual lists.
use crate::{
    GuiRuntime, ListLayoutType,
    core::object::{GObjectId, ListLayoutSettings},
    properties::Property,
};
use bevy::prelude::*;

/// Edit the projected component to retain the package's other layout fields.
/// Counts must be nonnegative (zero infers a count from the available space).
/// Invalid merged virtual geometry leaves the complete previous layout intact.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuiListLayout {
    pub layout: ListLayoutType,
    pub line_count: i32,
    pub column_count: i32,
    pub line_gap: i32,
    pub column_gap: i32,
    /// 0 left, 1 center, 2 right. Moves the container, not logical row coordinates.
    pub align: i32,
    /// 0 top, 1 middle, 2 bottom.
    pub vertical_align: i32,
    pub auto_resize_item: bool,
}
impl Default for GuiListLayout {
    fn default() -> Self {
        Self::from_native(ListLayoutSettings::default())
    }
}
impl GuiListLayout {
    fn from_native(s: ListLayoutSettings) -> Self {
        Self {
            layout: s.layout,
            line_count: s.line_count,
            column_count: s.column_count,
            line_gap: s.line_gap,
            column_gap: s.column_gap,
            align: s.align,
            vertical_align: s.vertical_align,
            auto_resize_item: s.auto_resize_item,
        }
    }
}
impl Property for GuiListLayout {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        runtime
            .store()
            .list_layout_settings(object)
            .map(Self::from_native)
    }
    fn apply(&self, previous: &Self, native: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        let mut next = ListLayoutSettings::default();
        macro_rules! merge { ($($field:ident),* $(,)?) => {$(
            next.$field = if self.$field != previous.$field { self.$field } else { native.$field };
        )*}; }
        merge! { layout, line_count, column_count, line_gap, column_gap, align, vertical_align, auto_resize_item }
        runtime.store_mut().set_list_layout_settings(object, next);
    }
}

#[cfg(test)]
#[path = "ecs_list_layout_tests.rs"]
mod tests;

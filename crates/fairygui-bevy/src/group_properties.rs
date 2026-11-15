//! Writable GGroup policies; positions and dimensions use GuiTransform.
use crate::{
    GroupLayoutType, GuiRuntime,
    core::object::{GObjectId, GroupLayoutSettings},
    properties::Property,
};
use bevy::prelude::*;

/// Group members remain siblings. Horizontal/vertical arrangement preserves
/// their cross-axis alignment. Edit the projected component to keep other
/// exported fields; use GuiTransform.size to resize the group and its members.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct GuiGroupLayout {
    pub layout: GroupLayoutType,
    pub line_gap: i32,
    pub column_gap: i32,
    pub exclude_invisibles: bool,
    pub auto_size_disabled: bool,
    /// Ordinal among all members, including excluded invisible members.
    pub main_grid_index: Option<usize>,
    /// Finite, nonnegative minimum contribution before absorbing extra space.
    pub main_grid_min_size: f32,
}
impl Default for GuiGroupLayout {
    fn default() -> Self {
        Self::from_native(GroupLayoutSettings::default())
    }
}
impl GuiGroupLayout {
    fn from_native(s: GroupLayoutSettings) -> Self {
        Self {
            layout: s.layout,
            line_gap: s.line_gap,
            column_gap: s.column_gap,
            exclude_invisibles: s.exclude_invisibles,
            auto_size_disabled: s.auto_size_disabled,
            main_grid_index: s.main_grid_index,
            main_grid_min_size: s.main_grid_min_size,
        }
    }
}
impl Property for GuiGroupLayout {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        runtime
            .store()
            .group_layout_settings(object)
            .map(Self::from_native)
    }
    fn apply(&self, previous: &Self, native: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        let mut next = GroupLayoutSettings::default();
        macro_rules! merge { ($($field:ident),* $(,)?) => {$(
            next.$field = if self.$field != previous.$field { self.$field } else { native.$field };
        )*}; }
        merge! { layout, line_gap, column_gap, exclude_invisibles, auto_size_disabled, main_grid_index, main_grid_min_size }
        if !next.main_grid_min_size.is_finite() || next.main_grid_min_size < 0. {
            next.main_grid_min_size = native.main_grid_min_size;
        }
        if next
            .main_grid_index
            .is_some_and(|i| i32::try_from(i).is_err())
        {
            next.main_grid_index = native.main_grid_index;
        }
        runtime.store_mut().set_group_layout_settings(object, next);
    }
}

#[cfg(test)]
#[path = "ecs_group_properties_tests.rs"]
mod tests;

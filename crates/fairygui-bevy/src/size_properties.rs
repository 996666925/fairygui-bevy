//! Size restrictions apply before layout, relations and view projection.
use crate::{
    GuiRuntime,
    core::{
        Size,
        object::{GObjectId, SizeConstraints},
    },
    properties::Property,
};
use bevy::prelude::*;

/// Logical, unscaled bounds. A zero maximum leaves that axis unbounded.
/// Constraints reevaluate the retained requested size immediately; GuiTransform
/// publishes the resulting clamped size. Finite, nonnegative values are required.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct GuiSizeConstraints {
    pub min: Vec2,
    pub max: Vec2,
}
impl Property for GuiSizeConstraints {
    fn read(runtime: &GuiRuntime, id: GObjectId) -> Option<Self> {
        let s = runtime.store().size_constraints(id)?;
        Some(Self {
            min: Vec2::new(s.min.width, s.min.height),
            max: Vec2::new(s.max.width, s.max.height),
        })
    }
    fn apply(&self, previous: &Self, native: &Self, runtime: &mut GuiRuntime, id: GObjectId) {
        let merge = |value: f32, old: f32, current: f32| {
            if value != old && value.is_finite() && value >= 0. {
                value
            } else {
                current
            }
        };
        let mut min = Vec2::new(
            merge(self.min.x, previous.min.x, native.min.x),
            merge(self.min.y, previous.min.y, native.min.y),
        );
        let mut max = Vec2::new(
            merge(self.max.x, previous.max.x, native.max.x),
            merge(self.max.y, previous.max.y, native.max.y),
        );
        for axis in 0..2 {
            if max[axis] > 0. && max[axis] < min[axis] {
                min[axis] = native.min[axis];
                max[axis] = native.max[axis];
            }
        }
        runtime.store_mut().set_size_constraints(
            id,
            SizeConstraints {
                min: Size::new(min.x, min.y),
                max: Size::new(max.x, max.y),
            },
        );
    }
}

#[cfg(test)]
#[path = "ecs_size_constraints_tests.rs"]
mod tests;

//! Target and displayed values for native progress bar tweens.
use bevy::prelude::*;

#[cfg(test)]
#[path = "ecs_progress_tests.rs"]
mod tests;

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct GuiProgressState {
    pub(crate) min: f64,
    pub(crate) max: f64,
    pub(crate) value: f64,
    pub(crate) displayed_value: f64,
    pub(crate) title_type: i32,
    pub(crate) reverse: bool,
    pub(crate) tweening: bool,
}
impl GuiProgressState {
    pub fn min(&self) -> f64 {
        self.min
    }
    pub fn max(&self) -> f64 {
        self.max
    }
    /// The assigned value, including a tween's target as soon as it starts.
    pub fn value(&self) -> f64 {
        self.value
    }
    /// The last value rendered, before visual clamping and title rounding.
    pub fn displayed_value(&self) -> f64 {
        self.displayed_value
    }
    pub fn percent(&self) -> f64 {
        if self.max == self.min {
            0.
        } else {
            ((self.displayed_value - self.min) / (self.max - self.min)).clamp(0., 1.)
        }
    }
    pub fn title_type(&self) -> i32 {
        self.title_type
    }
    pub fn reverse(&self) -> bool {
        self.reverse
    }
    pub fn is_tweening(&self) -> bool {
        self.tweening
    }
}

/// Natural completion on the progress control view, after its final ECS
/// snapshot and geometry. Stop, replacement and disposal cancel pending events.
#[derive(EntityEvent, Debug, Clone, Copy, PartialEq)]
pub struct GuiProgressTweenComplete {
    pub entity: Entity,
    pub value: f64,
}

pub(crate) fn emit_completions(mut runtime: NonSendMut<crate::GuiRuntime>, mut commands: Commands) {
    for (object, value) in runtime.store_mut().take_progress_tween_completions() {
        if let Some(entity) = runtime.entity_of(object) {
            commands.trigger(GuiProgressTweenComplete { entity, value });
        }
    }
}

//! Slider range and input policy, projected before user observers run.
use bevy::prelude::*;

#[cfg(test)]
#[path = "ecs_slider_tests.rs"]
mod tests;

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct GuiSliderState {
    pub(crate) min: f64,
    pub(crate) max: f64,
    pub(crate) value: f64,
    pub(crate) whole_numbers: bool,
    pub(crate) reverse: bool,
    pub(crate) change_on_click: bool,
    pub(crate) can_drag: bool,
    pub(crate) title_type: i32,
}
impl GuiSliderState {
    pub fn min(&self) -> f64 {
        self.min
    }
    pub fn max(&self) -> f64 {
        self.max
    }
    pub fn value(&self) -> f64 {
        self.value
    }
    pub fn whole_numbers(&self) -> bool {
        self.whole_numbers
    }
    pub fn reverse(&self) -> bool {
        self.reverse
    }
    pub fn change_on_click(&self) -> bool {
        self.change_on_click
    }
    pub fn can_drag(&self) -> bool {
        self.can_drag
    }
    pub fn title_type(&self) -> i32 {
        self.title_type
    }
}

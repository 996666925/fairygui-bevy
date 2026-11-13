//! Slider range and input policy, projected before user observers run.
use bevy::prelude::*;

#[cfg(test)]
#[path = "ecs_slider_tests.rs"]
mod tests;

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct GuiSliderState {
    pub min: f64,
    pub max: f64,
    pub value: f64,
    pub whole_numbers: bool,
    pub(crate) reverse: bool,
    pub change_on_click: bool,
    pub can_drag: bool,
    pub title_type: i32,
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

impl GuiSliderState {
    pub fn new(min: f64, max: f64, value: f64) -> Self {
        Self {
            min,
            max,
            value,
            whole_numbers: false,
            reverse: false,
            change_on_click: true,
            can_drag: true,
            title_type: 0,
        }
    }
}

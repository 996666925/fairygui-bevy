//! Writable GButton configuration, separate from its selected value and base title/icon.
use crate::{
    ButtonMode, GuiRuntime,
    core::{object::GObjectId, widget::ControlData},
    properties::Property,
};
use bevy::prelude::*;

/// Native button properties. Base title/icon use GuiText/GuiIcon; selection uses
/// GuiButtonSelected. Settings applied before input affect the current click.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct GuiButtonSettings {
    pub mode: ButtonMode,
    pub change_state_on_click: bool,
    /// None or an empty string falls back to the base title/icon.
    pub selected_title: Option<String>,
    pub selected_icon: Option<String>,
    /// None or an empty string disables this button's click sound.
    pub sound: Option<String>,
    pub sound_volume_scale: f32,
    /// 0 none, 1 darken non-text children, 2 scale around the center pivot.
    pub down_effect: i32,
    /// Finite strength; scale (effect 2) requires a positive value.
    pub down_effect_value: f32,
}
impl Default for GuiButtonSettings {
    fn default() -> Self {
        Self {
            mode: ButtonMode::Common,
            change_state_on_click: true,
            selected_title: None,
            selected_icon: None,
            sound: None,
            sound_volume_scale: 1.,
            down_effect: 0,
            down_effect_value: 0.8,
        }
    }
}
impl Property for GuiButtonSettings {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        let ControlData::Button(data) = runtime.store().object(object)?.control() else {
            return None;
        };
        Some(Self {
            mode: data.mode,
            change_state_on_click: data.change_state_on_click,
            selected_title: data.selected_title.clone(),
            selected_icon: data.selected_icon.clone(),
            sound: data.sound.clone(),
            sound_volume_scale: data.sound_volume_scale,
            down_effect: data.down_effect,
            down_effect_value: data.down_effect_value,
        })
    }
    fn apply(&self, previous: &Self, native: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        let store = runtime.store_mut();
        macro_rules! apply { ($($field:ident => $method:ident),* $(,)?) => {$(
            if self.$field != previous.$field && self.$field != native.$field { store.$method(object, self.$field); }
        )*}; }
        apply! { mode => set_button_mode, change_state_on_click => set_button_change_state_on_click,
        sound_volume_scale => set_button_sound_volume }
        if self.selected_title != previous.selected_title
            && self.selected_title != native.selected_title
        {
            store.set_button_selected_title(object, self.selected_title.as_deref());
        }
        if self.selected_icon != previous.selected_icon
            && self.selected_icon != native.selected_icon
        {
            store.set_button_selected_icon(object, self.selected_icon.as_deref());
        }
        if self.sound != previous.sound && self.sound != native.sound {
            store.set_button_sound(object, self.sound.as_deref());
        }
        if self.down_effect != previous.down_effect
            || self.down_effect_value != previous.down_effect_value
        {
            let effect = if self.down_effect != previous.down_effect {
                self.down_effect
            } else {
                native.down_effect
            };
            let value = if self.down_effect_value != previous.down_effect_value {
                self.down_effect_value
            } else {
                native.down_effect_value
            };
            store.set_button_down_effect(object, effect, value);
        }
    }
}

#[cfg(test)]
#[path = "ecs_button_properties_tests.rs"]
mod tests;

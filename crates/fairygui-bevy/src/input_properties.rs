//! Writable native input properties, separate from text value and formatting.
use crate::{
    backend,
    core::{
        object::GObjectId,
        text::input::{InputRestriction, InputRestrictionError},
        widget::ControlData,
    },
    properties::Property,
    runtime::GuiRuntime,
    text_format::native_color,
};
use bevy::prelude::*;

/// Native GTextInput configuration. Compound controls forward to their input
/// title. User edits obey restriction/length; GuiText assignments stay unrestricted.
/// Single/multiline layout uses GuiTextFormat.single_line.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct GuiTextInputSettings {
    pub editable: bool,
    pub password: bool,
    /// Raw prompt, with UBB tags removed by presentation. Never enters GuiText.
    pub prompt_text: Option<String>,
    pub prompt_color: Color,
    /// Native character-class restriction. Invalid patterns retain the previous
    /// rule and log a warning. try_set_restrict validates before editing it.
    pub restrict: Option<String>,
    /// UTF-16 units for user insertions. Nonpositive uses the native default.
    pub max_length: i32,
}
impl Default for GuiTextInputSettings {
    fn default() -> Self {
        Self {
            editable: true,
            password: false,
            prompt_text: None,
            prompt_color: Color::srgb_u8(169, 169, 169),
            restrict: None,
            max_length: 0,
        }
    }
}
impl GuiTextInputSettings {
    /// Validate the native character class before replacing this component's
    /// restriction. Both None and an empty string allow all characters.
    pub fn try_set_restrict(&mut self, source: Option<&str>) -> Result<(), InputRestrictionError> {
        InputRestriction::new(source)?;
        self.restrict = source.map(str::to_owned);
        Ok(())
    }
}
impl Property for GuiTextInputSettings {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        let input = runtime.store().input_field_of(object)?;
        let ControlData::TextInput(data) = runtime.store().object(input)?.control() else {
            return None;
        };
        Some(Self {
            editable: data.editable,
            password: data.password,
            prompt_text: data.prompt_text.clone(),
            prompt_color: backend::to_bevy_color(data.prompt_color),
            restrict: data.restrict.clone(),
            max_length: data.max_length,
        })
    }
    fn apply(&self, previous: &Self, native: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        let store = runtime.store_mut();
        macro_rules! apply {
            ($($field:ident => $method:ident),* $(,)?) => {$(
                if self.$field != previous.$field && self.$field != native.$field {
                    store.$method(object, self.$field);
                }
            )*};
        }
        apply! { editable => set_input_editable, password => set_input_password, max_length => set_input_max_length }
        if self.prompt_text != previous.prompt_text && self.prompt_text != native.prompt_text {
            store.set_input_prompt(object, self.prompt_text.as_deref());
        }
        if self.prompt_color != previous.prompt_color
            && self.prompt_color != native.prompt_color
            && let Some(color) = native_color(self.prompt_color)
        {
            store.set_input_prompt_color(object, color);
        }
        if self.restrict != previous.restrict
            && self.restrict != native.restrict
            && let Err(error) = store.set_input_restrict(object, self.restrict.as_deref())
        {
            tracing::warn!(%error, ?object, "Invalid input restriction; previous rule retained");
        }
    }
}

#[cfg(test)]
#[path = "ecs_input_properties_tests.rs"]
mod tests;

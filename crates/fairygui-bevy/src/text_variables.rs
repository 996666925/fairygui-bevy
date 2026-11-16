//! Writable template variables; GuiText continues to contain the source text.
use crate::{
    GuiRuntime,
    core::{object::GObjectId, text::TextVariables},
    properties::Property,
};
use bevy::prelude::*;

/// None disables substitution. Some(empty) enables missing/default expansion.
/// Values are strings and are expanded once before UBB/HTML and text measurement.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct GuiTextVars(pub Option<TextVariables>);
impl GuiTextVars {
    pub fn new(values: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>) -> Self {
        Self(Some(
            values
                .into_iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
        ))
    }
    pub fn set(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.0
            .get_or_insert_with(Default::default)
            .insert(name.into(), value.into());
    }
    pub fn clear(&mut self) {
        self.0 = Some(Default::default());
    }
    pub fn disable(&mut self) {
        self.0 = None;
    }
}
impl Property for GuiTextVars {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        Some(Self(
            runtime
                .store()
                .object_text_field(object)?
                .template_vars
                .clone(),
        ))
    }
    fn apply(&self, previous: &Self, native: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        let next = match (&self.0, &previous.0, &native.0) {
            (Some(value), Some(previous), Some(native)) => {
                let mut next = native.clone();
                for name in previous.keys().chain(value.keys()) {
                    if value.get(name) != previous.get(name) {
                        if let Some(value) = value.get(name) {
                            next.insert(name.clone(), value.clone());
                        } else {
                            next.remove(name);
                        }
                    }
                }
                Some(next)
            }
            _ => self.0.clone(),
        };
        runtime.store_mut().set_object_text_variables(object, next);
    }
}

#[cfg(test)]
#[path = "ecs_text_variables_tests.rs"]
mod tests;

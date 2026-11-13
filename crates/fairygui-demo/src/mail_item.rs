//! One application component bound to both exported mail templates.
use bevy::prelude::*;
use fairygui_bevy::{core::transition::TransitionPlayOptions, prelude::*};

#[derive(Component, GuiExtension)]
#[gui(url = "ui://VirtualList/mailItem")]
pub(crate) struct MailItem {
    #[gui(path = "")]
    pub entity: Entity,
    #[gui(path = "timeText")]
    pub time: Entity,
}

impl MailItem {
    pub fn play_effect(&self, gui_edit: &mut GuiMut, delay: f32) {
        gui_edit.edit(self.entity, |object: &mut GuiObjectMut| {
            object.set_visible(false)
        });
        gui_edit.edit(self.entity, |object: &mut GuiObjectMut| {
            object.play_transition_with_options("t0", TransitionPlayOptions { delay, ..default() })
        });
    }
}

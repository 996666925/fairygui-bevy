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
    pub fn play_effect(&self, commands: &mut Commands, delay: f32) {
        commands.write_message(GuiObjectCommand::new(
            self.entity,
            GuiObjectAction::SetVisible(false),
        ));
        commands.write_message(GuiObjectCommand::new(
            self.entity,
            GuiObjectAction::PlayTransitionWithOptions {
                name: "t0".into(),
                options: TransitionPlayOptions { delay, ..default() },
            },
        ));
    }
}

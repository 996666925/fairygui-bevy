//! Pull-release responses applied before the next native scroll tick.
use crate::{GObjectRef, GuiRootInstance, GuiRuntime};
#[cfg(test)]
#[path = "ecs_pull_tests.rs"]
mod tests;
use bevy::prelude::*;

/// Set a refresh lock in logical pixels (zero releases it). Send in Respond to
/// affect the first release tick. The target must be an already materialized
/// scroll view or root owner; requests for absent targets are discarded.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub enum GuiPullCommand {
    LockHeader { target: Entity, size: f32 },
    LockFooter { target: Entity, size: f32 },
}

pub(crate) fn apply_pull_commands(
    mut messages: MessageReader<GuiPullCommand>,
    objects: Query<&GObjectRef>,
    roots: Query<&GuiRootInstance>,
    mut runtime: NonSendMut<GuiRuntime>,
) {
    for &message in messages.read() {
        let (target, size, header) = match message {
            GuiPullCommand::LockHeader { target, size } => (target, size, true),
            GuiPullCommand::LockFooter { target, size } => (target, size, false),
        };
        if !size.is_finite() || size < 0. {
            continue;
        }
        let object = objects
            .get(target)
            .map(|reference| reference.0)
            .or_else(|_| roots.get(target).map(|root| root.object))
            .ok()
            .or_else(|| runtime.ecs_window_object(target));
        let Some(object) = object else {
            continue;
        };
        if header {
            runtime.store_mut().lock_scroll_header(object, size);
        } else {
            runtime.store_mut().lock_scroll_footer(object, size);
        }
    }
}

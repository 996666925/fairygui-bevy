//! Read-only playback snapshots for MovieClip controls and Loader content.
pub use crate::core::object::AnimationPlaySettings as GuiAnimationPlaySettings;
use crate::{GObjectRef, GuiRuntime, movie::MovieClipPlayer};
use bevy::prelude::*;

#[cfg(test)]
#[path = "ecs_animation_tests.rs"]
mod tests;

/// Native Playing/Frame/timeScale plus the resolved player's timeline state.
/// A zero frame count means the Loader has no resolved movie (static image,
/// component, empty or pending resource). Playing stays true after a finite
/// playback ends, matching FairyGUI; consult `is_ended` to detect that state.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct GuiAnimationState {
    playing: bool,
    frame: i32,
    time_scale: f32,
    frame_count: usize,
    ended: bool,
}
impl GuiAnimationState {
    pub fn is_playing(&self) -> bool {
        self.playing
    }
    pub fn frame(&self) -> i32 {
        self.frame
    }
    pub fn time_scale(&self) -> f32 {
        self.time_scale
    }
    pub fn frame_count(&self) -> usize {
        self.frame_count
    }
    pub fn is_ended(&self) -> bool {
        self.ended
    }
}

pub(crate) fn sync_state(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    objects: Query<(
        Entity,
        &GObjectRef,
        Option<&MovieClipPlayer>,
        Option<&GuiAnimationState>,
    )>,
) {
    for (entity, object, player, current) in &objects {
        if runtime.entity_of(object.0) != Some(entity) {
            continue;
        }
        let Some((playing, frame)) = runtime.store().animation_state(object.0) else {
            if current.is_some() {
                commands.entity(entity).remove::<GuiAnimationState>();
            }
            continue;
        };
        let next = GuiAnimationState {
            playing,
            frame,
            time_scale: runtime.store().animation_time_scale(object.0).unwrap_or(1.),
            frame_count: player.map_or(0, MovieClipPlayer::frame_count),
            ended: player.is_some_and(MovieClipPlayer::is_ended),
        };
        if current != Some(&next) {
            commands.entity(entity).insert(next);
        }
    }
}

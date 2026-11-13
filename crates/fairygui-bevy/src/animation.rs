//! Writable native playback properties and resolved movie timeline metadata.
pub use crate::core::object::AnimationPlaySettings as GuiAnimationPlaySettings;
use crate::core::{object::GObjectId, widget::ControlData};
use crate::{GuiRuntime, movie::MovieClipPlayer, properties::Property};
use bevy::prelude::*;

#[cfg(test)]
#[path = "ecs_animation_tests.rs"]
mod tests;

/// Native Playing/Frame/timeScale plus the resolved player's timeline state.
/// A zero frame count means the Loader has no resolved movie (static image,
/// component, empty or pending resource). Playing stays true after a finite
/// playback ends, matching FairyGUI; consult `is_ended` to detect that state.
/// Edit playing/frame/time_scale directly. Unedited fields follow the player,
/// controller gears and transitions without seeking back to an old snapshot.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct GuiAnimationState {
    pub playing: bool,
    pub frame: i32,
    pub time_scale: f32,
    frame_count: usize,
    ended: bool,
}
impl Default for GuiAnimationState {
    fn default() -> Self {
        Self::new(true, 0, 1.)
    }
}
impl GuiAnimationState {
    /// Initial native properties, also usable on an owner waiting for assets.
    /// Frame bounds and speed validation apply when the object is available.
    pub const fn new(playing: bool, frame: i32, time_scale: f32) -> Self {
        Self {
            playing,
            frame,
            time_scale,
            frame_count: 0,
            ended: false,
        }
    }
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

impl Property for GuiAnimationState {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        let (playing, frame) = runtime.store().animation_state(object)?;
        Some(Self::new(
            playing,
            frame,
            runtime.store().animation_time_scale(object).unwrap_or(1.),
        ))
    }
    fn read_view(&mut self, world: &World, object: GObjectId) {
        let runtime = world.non_send::<GuiRuntime>();
        let player = runtime
            .entity_of(object)
            .filter(|&entity| {
                // URL properties can be consumed before the source reconciler.
                // A retained old movie must not describe the replacement URL.
                if let Some(ControlData::Loader(data)) =
                    runtime.store().object(object).map(|o| o.control())
                {
                    world
                        .get::<crate::runtime::MovieSource>(entity)
                        .is_some_and(|source| source.url == data.url.as_deref().unwrap_or_default())
                } else {
                    true
                }
            })
            .and_then(|entity| world.get::<MovieClipPlayer>(entity));
        self.frame_count = player.map_or(0, MovieClipPlayer::frame_count);
        self.ended = player.is_some_and(MovieClipPlayer::is_ended);
    }
    fn apply(&self, previous: &Self, native: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        let store = runtime.store_mut();
        if self.playing != previous.playing && self.playing != native.playing {
            store.set_animation_playing(object, self.playing);
        }
        if self.time_scale != previous.time_scale && self.time_scale != native.time_scale {
            store.set_animation_time_scale(object, self.time_scale);
        }
        if self.frame != previous.frame && self.frame != native.frame {
            store.set_animation_frame(object, self.frame);
        }
    }
}

pub(crate) fn sync_state(world: &mut World) {
    crate::properties::sync_property::<GuiAnimationState>(world);
}

#[cfg(test)]
#[path = "ecs_animation_properties_tests.rs"]
mod property_tests;

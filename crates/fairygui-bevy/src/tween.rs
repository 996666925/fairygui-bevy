//! Typed property endpoints for entity-addressed native tweens.
pub use crate::core::transition::TweenProperty as GuiTweenProperty;
#[cfg(test)]
#[path = "ecs_tween_tests.rs"]
mod tests;
use crate::core::transition::{Tween, TweenValue};
use bevy::prelude::*;

/// Natural completion on the control view entity, after Sync has published the
/// final geometry. Observe the view, including when the command used its root
/// owner. Stop, replacement and disposal suppress unconsumed completions.
#[derive(EntityEvent, Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuiTweenComplete {
    pub entity: Entity,
    pub property: GuiTweenProperty,
}

pub(crate) fn emit_completions(mut runtime: NonSendMut<crate::GuiRuntime>, mut commands: Commands) {
    for (object, property) in runtime.store_mut().take_tween_completions() {
        if let Some(entity) = runtime.entity_of(object) {
            commands.trigger(GuiTweenComplete { entity, property });
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GuiTweenTarget {
    Position(Vec2),
    Size(Vec2),
    Scale(Vec2),
    Alpha(f32),
    Rotation(f32),
}
impl GuiTweenTarget {
    pub(crate) fn apply(
        self,
        store: &mut crate::core::object::ObjectStore,
        object: crate::core::GObjectId,
        duration: f32,
        ease: i32,
    ) {
        if !duration.is_finite() || duration < 0. {
            return;
        }
        let Some(source) = store.object(object) else {
            return;
        };
        let value = |x, y| TweenValue { x, y, ..default() };
        let (property, start, end) = match self {
            Self::Position(end) if end.is_finite() => (
                GuiTweenProperty::Xy,
                value(source.x(), source.y()),
                value(end.x, end.y),
            ),
            Self::Size(end) if end.is_finite() && end.min_element() >= 0. => (
                GuiTweenProperty::Size,
                value(source.width(), source.height()),
                value(end.x, end.y),
            ),
            Self::Scale(end) if end.is_finite() => (
                GuiTweenProperty::Scale,
                value(source.scale_x(), source.scale_y()),
                value(end.x, end.y),
            ),
            Self::Alpha(end) if end.is_finite() => (
                GuiTweenProperty::Alpha,
                value(source.alpha(), 0.),
                value(end.clamp(0., 1.), 0.),
            ),
            Self::Rotation(end) if end.is_finite() => (
                GuiTweenProperty::Rotation,
                value(source.rotation(), 0.),
                value(end, 0.),
            ),
            _ => return,
        };
        store.kill_tween_property(object, property);
        let mut tween = Tween::new(object, property)
            .from_to(start, end)
            .with_duration(duration)
            .with_ease(ease);
        if duration == 0. {
            tween.advance(store, f32::EPSILON);
        }
        store.add_tween(tween);
    }
}

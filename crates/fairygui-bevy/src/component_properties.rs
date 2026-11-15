//! Writable native component input properties.
use crate::{GuiRuntime, properties::Property};
use bevy::prelude::*;
use fairygui_core::object::GObjectId;

/// Exported component/list edge fade widths in local logical pixels. Zero
/// disables soft clipping. Visible overflow and unclipped scroll panes ignore it.
/// Scroll panes fade only toward remaining content; GuiScrollSettings controls
/// whether the top/left edges also fade after scrolling away from the origin.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct GuiClipSoftness(pub Vec2);
impl Property for GuiClipSoftness {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        let softness = runtime
            .store()
            .object(object)?
            .control()
            .component()?
            .clip_softness;
        Some(Self(Vec2::new(softness.x, softness.y)))
    }
    fn apply(&self, _: &Self, _: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        runtime
            .store_mut()
            .set_clip_softness(object, crate::core::Point::new(self.0.x, self.0.y));
    }
}

/// GComponent.opaque controls blank-rectangle picking, independently of alpha.
/// False lets empty space pass through while children still receive input.
/// Custom pixel/child hit areas keep their native priority and geometry.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GuiOpaque(pub bool);
impl Property for GuiOpaque {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        Some(Self(
            runtime
                .store()
                .object(object)?
                .control()
                .component()?
                .opaque,
        ))
    }
    fn apply(&self, _: &Self, _: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        runtime.store_mut().set_component_opaque(object, self.0);
    }
}

#[cfg(test)]
#[path = "ecs_component_input_tests.rs"]
mod tests;

#[cfg(test)]
mod clip_tests {
    use super::*;
    use crate::core::package::ObjectType;
    use crate::properties::sync_property;
    #[test]
    fn ecs_softness_accepts_finite_widths_and_restores_invalid_edits() {
        let mut world = World::new();
        world.init_non_send::<GuiRuntime>();
        let id = world
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .create(ObjectType::Component);
        let entity = world
            .spawn((crate::GObjectRef(id), GuiClipSoftness(Vec2::new(24., 20.))))
            .id();
        sync_property::<GuiClipSoftness>(&mut world);
        assert_eq!(
            world.get::<GuiClipSoftness>(entity).unwrap().0,
            Vec2::new(24., 20.)
        );
        world.get_mut::<GuiClipSoftness>(entity).unwrap().0.x = f32::NAN;
        sync_property::<GuiClipSoftness>(&mut world);
        assert_eq!(
            world.get::<GuiClipSoftness>(entity).unwrap().0,
            Vec2::new(24., 20.)
        );
        world.get_mut::<GuiClipSoftness>(entity).unwrap().0 = Vec2::ZERO;
        sync_property::<GuiClipSoftness>(&mut world);
        assert_eq!(
            world
                .non_send::<GuiRuntime>()
                .store()
                .object(id)
                .unwrap()
                .control()
                .component()
                .unwrap()
                .clip_softness,
            crate::core::Point::ZERO
        );
    }
}

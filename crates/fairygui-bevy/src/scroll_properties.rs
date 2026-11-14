//! Writable ScrollPane settings, separate from its live position snapshot.
use crate::{GuiRuntime, core::object::GObjectId, properties::Property};
use bevy::prelude::*;

/// Native live scroll settings. Changing settings alone does not move the
/// viewport or restart a running tween. Edit the projected component to retain
/// the other settings exported by the package.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct GuiScrollSettings {
    /// New names resolve on the current parent. Existing references retain
    /// their owner when the pane moves; None unbinds without moving the pane.
    pub page_controller: Option<String>,
    pub mouse_wheel_enabled: bool,
    /// Logical pixels; a native line wheel step moves twice this distance.
    /// Zero restores the configured default, negative values reverse direction.
    pub scroll_step: f32,
    pub snap_to_item: bool,
    pub touch_effect: bool,
    pub bounceback_effect: bool,
    /// Finite, strictly between zero and one. Affects future release planning.
    pub deceleration_rate: f64,
}

impl Default for GuiScrollSettings {
    fn default() -> Self {
        let defaults = crate::core::scroll::ScrollDefaults::default();
        Self {
            page_controller: None,
            mouse_wheel_enabled: true,
            scroll_step: defaults.scroll_step,
            snap_to_item: false,
            touch_effect: defaults.touch_effect,
            bounceback_effect: defaults.bounceback_effect,
            deceleration_rate: crate::core::scroll::ScrollPhysics::default().deceleration_rate,
        }
    }
}

impl Property for GuiScrollSettings {
    fn read(runtime: &GuiRuntime, object: GObjectId) -> Option<Self> {
        let pane = runtime.store().scroll_pane(object)?;
        Some(Self {
            page_controller: runtime
                .store()
                .scroll_page_controller_name(object)
                .map(str::to_owned),
            mouse_wheel_enabled: pane.mouse_wheel_enabled(),
            scroll_step: pane.scroll_step(),
            snap_to_item: pane.snap_to_item,
            touch_effect: pane.touch_effect,
            bounceback_effect: pane.bounceback_effect,
            deceleration_rate: pane.physics().deceleration_rate,
        })
    }

    fn apply(&self, previous: &Self, native: &Self, runtime: &mut GuiRuntime, object: GObjectId) {
        let store = runtime.store_mut();
        if self.page_controller != previous.page_controller
            && self.page_controller != native.page_controller
        {
            store.set_scroll_page_controller_by_name(object, self.page_controller.as_deref());
        }
        macro_rules! apply { ($($field:ident => $method:ident),* $(,)?) => {$(
            if self.$field != previous.$field && self.$field != native.$field {
                store.$method(object, self.$field);
            }
        )*}; }
        apply! {
            mouse_wheel_enabled => set_scroll_mouse_wheel_enabled,
            scroll_step => set_scroll_step,
            snap_to_item => set_scroll_snap_to_item,
            touch_effect => set_scroll_touch_effect,
            bounceback_effect => set_scroll_bounceback_effect,
        }
        if self.deceleration_rate != previous.deceleration_rate
            && self.deceleration_rate != native.deceleration_rate
            && let Some(pane) = store.scroll_pane(object)
        {
            let mut physics = pane.physics();
            physics.deceleration_rate = self.deceleration_rate;
            store.set_scroll_physics(object, physics);
        }
    }
}

#[cfg(test)]
#[path = "ecs_scroll_properties_tests.rs"]
mod tests;

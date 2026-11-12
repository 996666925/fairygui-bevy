use super::{GObjectId, ObjectStore, dirty};
use crate::scroll::ScrollPane;

#[cfg(test)]
#[path = "scroll_wheel_tests.rs"]
mod tests;

impl ObjectStore {
    pub fn scroll_mouse_wheel_enabled(&self, container: GObjectId) -> Option<bool> {
        self.scroll_pane(container)
            .map(ScrollPane::mouse_wheel_enabled)
    }

    pub fn set_scroll_mouse_wheel_enabled(&mut self, container: GObjectId, enabled: bool) -> bool {
        self.objects
            .get_mut(container)
            .and_then(|o| o.control.component_mut())
            .and_then(|c| c.scroll_pane.as_mut())
            .is_some_and(|pane| pane.set_mouse_wheel_enabled(enabled))
    }

    pub fn scroll_step(&self, container: GObjectId) -> Option<f32> {
        self.scroll_pane(container).map(ScrollPane::scroll_step)
    }

    pub fn set_scroll_step(&mut self, container: GObjectId, step: f32) -> bool {
        self.objects
            .get_mut(container)
            .and_then(|o| o.control.component_mut())
            .and_then(|c| c.scroll_pane.as_mut())
            .is_some_and(|pane| pane.set_scroll_step(step))
    }

    /// Default native event bubbling visits every containing pane, even when an
    /// inner pane is disabled or at a boundary. Returns the number moved.
    pub fn scroll_mouse_wheel(&mut self, target: GObjectId, delta: f32) -> usize {
        self.route_scroll_wheel(target, |pane| pane.mouse_wheel(delta))
    }

    /// Smooth pixel extension with the same pane enable/bubbling policy.
    pub fn scroll_mouse_wheel_pixels(&mut self, target: GObjectId, x: f32, y: f32) -> usize {
        self.route_scroll_wheel(target, |pane| pane.mouse_wheel_pixels(x, y))
    }

    fn route_scroll_wheel(
        &mut self,
        target: GObjectId,
        mut apply: impl FnMut(&mut ScrollPane) -> bool,
    ) -> usize {
        let mut current = Some(target);
        let mut moved = 0;
        while let Some(id) = current {
            let Some(object) = self.object(id) else {
                break;
            };
            current = object.parent();
            self.prepare_scroll_snapping(id);
            let changed = self
                .objects
                .get_mut(id)
                .and_then(|o| o.control.component_mut())
                .and_then(|c| c.scroll_pane.as_mut())
                .is_some_and(&mut apply);
            if changed {
                self.touch(id, dirty::SCROLL);
                self.update_scroll_page_controller(id);
                moved += 1;
            }
        }
        moved
    }
}

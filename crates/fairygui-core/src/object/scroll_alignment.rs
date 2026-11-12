use super::{GObjectId, ObjectStore, dirty};
use crate::geom::{Point, Rect};

#[cfg(test)]
#[path = "scroll_alignment_tests.rs"]
mod tests;

impl ObjectStore {
    pub fn scroll_snap_to_item(&self, container: GObjectId) -> Option<bool> {
        self.scroll_pane(container).map(|p| p.snap_to_item)
    }

    /// Native flag setter: changing it alone does not move the viewport.
    pub fn set_scroll_snap_to_item(&mut self, container: GObjectId, enabled: bool) -> bool {
        self.objects
            .get_mut(container)
            .and_then(|o| o.control.component_mut())
            .and_then(|c| c.scroll_pane.as_mut())
            .is_some_and(|p| {
                if p.snap_to_item == enabled {
                    return false;
                }
                p.snap_to_item = enabled;
                if !enabled {
                    p.set_snap_items(Vec::new());
                }
                true
            })
    }
    /// Native GComponent/nonvirtual GList snapping against current logical
    /// children. Invisible children participate; registered pane bars do not.
    pub fn snapping_position(&self, container: GObjectId, position: Point) -> Option<Point> {
        self.object(container)?.control().component()?;
        if !position.x.is_finite() || !position.y.is_finite() {
            return None;
        }
        if let Some(state) = self.virtual_states.get(&container) {
            return Some(state.geometry.snapping_position(position, Point::ZERO));
        }
        Some(crate::scroll::snapping_position(
            &self.scroll_snap_items(container),
            position,
        ))
    }

    fn scroll_snap_items(&self, container: GObjectId) -> Vec<Rect> {
        self.content_children(container)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|id| {
                let o = self.object(id)?;
                Some(Rect::new(
                    o.x(),
                    o.y(),
                    o.width() * o.scale_x().abs(),
                    o.height() * o.scale_y().abs(),
                ))
            })
            .collect()
    }

    pub(super) fn prepare_scroll_snapping(&mut self, container: GObjectId) {
        if !self
            .scroll_pane(container)
            .is_some_and(|p| p.snap_to_item && !p.page_mode)
        {
            return;
        }
        let virtual_geometry = self
            .virtual_states
            .get(&container)
            .map(|state| std::sync::Arc::new(state.geometry.clone()));
        let items = if virtual_geometry.is_none() {
            self.scroll_snap_items(container)
        } else {
            Vec::new()
        };
        if let Some(pane) = self
            .objects
            .get_mut(container)
            .and_then(|o| o.control.component_mut())
            .and_then(|c| c.scroll_pane.as_mut())
        {
            if let Some(geometry) = virtual_geometry {
                pane.set_virtual_snap_geometry(geometry);
            } else {
                pane.set_snap_items(items);
            }
        }
    }

    /// Native programmatic refresh, including page/item alignment and rounding.
    /// set_scroll_pos remains the explicit raw-position/bounce escape hatch.
    pub fn set_scroll_pos_animated(
        &mut self,
        container: GObjectId,
        x: f32,
        y: f32,
        animated: bool,
    ) -> bool {
        self.prepare_scroll_snapping(container);
        let changed = self
            .objects
            .get_mut(container)
            .and_then(|o| o.control.component_mut())
            .and_then(|c| c.scroll_pane.as_mut())
            .is_some_and(|p| p.set_pos_animated(x, y, animated));
        if changed {
            self.touch(container, dirty::SCROLL);
            self.update_scroll_page_controller(container);
        }
        changed
    }
}

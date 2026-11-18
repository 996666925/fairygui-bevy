//! GObject.pixelSnapping rounds parent-local position before pivot compensation.
use super::{GObjectId, ObjectStore, dirty};
use crate::{GObject, Point};

impl GObject {
    pub fn pixel_snapping(&self) -> bool {
        self.pixel_snapping
    }

    /// Native handleXYChanged's position after subtracting an anchor pivot and
    /// optionally rounding it. Logical x/y remain unchanged. Math.round ties
    /// go toward positive infinity, including -0.5, unlike Rust's round().
    pub fn display_base_position(&self) -> Point {
        let mut p = Point::new(self.x, self.y);
        if self.pivot_as_anchor {
            p.offset(-self.pivot_x * self.width, -self.pivot_y * self.height);
        }
        if self.pixel_snapping {
            p.x = (f64::from(p.x) + 0.5).floor() as f32;
            p.y = (f64::from(p.y) + 0.5).floor() as f32;
        }
        p
    }
}

impl ObjectStore {
    /// Change display rounding without changing logical geometry, relations,
    /// group layout or XY gears. Same-value writes do not refresh image caches.
    pub fn set_pixel_snapping(&mut self, id: GObjectId, enabled: bool) -> bool {
        let Some(object) = self.objects.get_mut(id) else {
            return false;
        };
        if object.pixel_snapping_initialized && object.pixel_snapping == enabled {
            return false;
        }
        if object.display_transform.is_none() {
            object.initialize_display_transform();
        }
        object.pixel_snapping = enabled;
        object.pixel_snapping_initialized = true;
        object.refresh_display_position();
        self.touch(id, dirty::LOOK);
        true
    }
}

#[cfg(test)]
#[path = "pixel_snapping_tests.rs"]
mod tests;

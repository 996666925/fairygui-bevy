//! GImage.flip changes Sprite scale and cached display position, not texture UVs.
use super::{ObjectStore, dirty};
use crate::{
    GObject, Point,
    object::{GObjectId, object::DisplayTransformState},
    widget::{ControlData, FlipType},
};

impl GObject {
    /// Native Sprite linear matrix [a, b, c, d], including FairyGUI's
    /// setSkew(-skewX, skewY) conversion before Laya computes its axes.
    pub fn transform_axes(&self) -> [f64; 4] {
        let x = f64::from(self.rotation + self.skew_x) * 0.0174532922222222;
        let y = f64::from(self.rotation + self.skew_y) * 0.0174532922222222;
        [
            f64::from(self.scale_x) * y.cos(),
            f64::from(self.scale_x) * y.sin(),
            -f64::from(self.scale_y) * x.sin(),
            f64::from(self.scale_y) * x.cos(),
        ]
    }
    /// Actual cached Sprite position once native GImage flip handling is active.
    /// Logical x/y remain unchanged; resize/scale need not refresh this position.
    pub fn image_display_position(&self) -> Option<Point> {
        matches!(self.control, ControlData::Image(_))
            .then(|| self.display_position())
            .flatten()
    }

    /// Retained Sprite position after native XY/pivot/snap handlers. Flipped
    /// images and objects with an assigned pixelSnapping value retain it.
    pub fn display_position(&self) -> Option<Point> {
        self.display_transform.map(|state| state.position)
    }

    pub(crate) fn initialize_display_transform(&mut self) {
        self.display_transform = Some(DisplayTransformState {
            pivot_offset: Point::ZERO,
            position: Point::ZERO,
        });
        self.update_display_pivot_offset();
        self.refresh_display_position();
    }
    pub(crate) fn update_display_pivot_offset(&mut self) {
        let [a, b, c, d] = self.transform_axes();
        let Some(state) = &mut self.display_transform else {
            return;
        };
        let px = f64::from(self.pivot_x * self.width);
        let py = f64::from(self.pivot_y * self.height);
        state.pivot_offset =
            Point::new((px - px * a - py * c) as f32, (py - px * b - py * d) as f32);
    }
    pub(crate) fn refresh_display_position(&mut self) {
        let position = self.display_base_position();
        let Some(state) = &mut self.display_transform else {
            return;
        };
        let flipped =
            matches!(&self.control, ControlData::Image(data) if data.flip != FlipType::None);
        state.position = Point::new(
            position.x
                + state.pivot_offset.x
                + if flipped && self.scale_x == -1. {
                    self.width
                } else {
                    0.
                },
            position.y
                + state.pivot_offset.y
                + if flipped && self.scale_y == -1. {
                    self.height
                } else {
                    0.
                },
        );
    }
    pub(crate) fn apply_display_pivot(&mut self) {
        if self.pivot_x != 0. || self.pivot_y != 0. {
            self.update_display_pivot_offset();
            self.refresh_display_position();
        }
    }
}

impl ObjectStore {
    pub fn image_flip(&self, id: GObjectId) -> Option<FlipType> {
        let ControlData::Image(data) = self.object(id)?.control() else {
            return None;
        };
        Some(data.flip)
    }
    /// Native flip assignment resets scale to +/-1, captures the Size gear and
    /// refreshes Sprite position. Reassigning the same flip leaves scale alone.
    pub fn set_image_flip(&mut self, id: GObjectId, flip: FlipType) {
        if self.image_flip(id).is_none_or(|old| old == flip) {
            return;
        }
        let object = self.objects.get_mut(id).unwrap();
        if object.display_transform.is_none() {
            object.initialize_display_transform();
        }
        let ControlData::Image(data) = &mut object.control else {
            unreachable!()
        };
        data.flip = flip;
        let (x, y) = flip.scale();
        self.set_scale(id, x, y);
        self.objects.get_mut(id).unwrap().refresh_display_position();
        self.touch(id, dirty::LOOK);
    }
}

#[cfg(test)]
#[path = "image_tests.rs"]
mod tests;

//! GImage.flip changes Sprite scale and cached display position, not texture UVs.
use super::{ObjectStore, dirty};
use crate::{
    GObject, Point,
    object::{GObjectId, object::ImageTransformState},
    widget::{ControlData, FlipType},
};

impl GObject {
    /// Actual cached Sprite position once native GImage flip handling is active.
    /// Logical x/y remain unchanged; resize/scale need not refresh this position.
    pub fn image_display_position(&self) -> Option<Point> {
        self.image_transform.map(|state| state.position)
    }

    pub(crate) fn initialize_image_transform(&mut self) {
        self.image_transform = Some(ImageTransformState {
            pivot_offset: Point::ZERO,
            position: Point::ZERO,
        });
        self.update_image_pivot_offset();
        self.refresh_image_position();
    }
    pub(crate) fn update_image_pivot_offset(&mut self) {
        let Some(state) = &mut self.image_transform else {
            return;
        };
        let px = f64::from(self.pivot_x * self.width);
        let py = f64::from(self.pivot_y * self.height);
        let x = f64::from(self.rotation - self.skew_x) * 0.0174532922222222;
        let y = f64::from(self.rotation + self.skew_y) * 0.0174532922222222;
        state.pivot_offset = Point::new(
            (px - px * f64::from(self.scale_x) * y.cos() + py * f64::from(self.scale_y) * x.sin())
                as f32,
            (py - px * f64::from(self.scale_x) * y.sin() - py * f64::from(self.scale_y) * x.cos())
                as f32,
        );
    }
    pub(crate) fn refresh_image_position(&mut self) {
        let Some(state) = &mut self.image_transform else {
            return;
        };
        let anchor = if self.pivot_as_anchor {
            Point::new(self.pivot_x * self.width, self.pivot_y * self.height)
        } else {
            Point::ZERO
        };
        let flipped =
            matches!(&self.control, ControlData::Image(data) if data.flip != FlipType::None);
        state.position = Point::new(
            self.x - anchor.x
                + state.pivot_offset.x
                + if flipped && self.scale_x == -1. {
                    self.width
                } else {
                    0.
                },
            self.y - anchor.y
                + state.pivot_offset.y
                + if flipped && self.scale_y == -1. {
                    self.height
                } else {
                    0.
                },
        );
    }
    pub(crate) fn apply_image_pivot(&mut self) {
        if self.pivot_x != 0. || self.pivot_y != 0. {
            self.update_image_pivot_offset();
            self.refresh_image_position();
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
        if object.image_transform.is_none() {
            object.initialize_image_transform();
        }
        let ControlData::Image(data) = &mut object.control else {
            unreachable!()
        };
        data.flip = flip;
        let (x, y) = flip.scale();
        self.set_scale(id, x, y);
        self.objects.get_mut(id).unwrap().refresh_image_position();
        self.touch(id, dirty::LOOK);
    }
}

#[cfg(test)]
#[path = "image_tests.rs"]
mod tests;

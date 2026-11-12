use super::ScrollPane;
use crate::geom::Point;

#[cfg(test)]
#[path = "wheel_tests.rs"]
mod tests;

impl ScrollPane {
    pub fn mouse_wheel_enabled(&self) -> bool {
        self.mouse_wheel_enabled
    }

    pub fn set_mouse_wheel_enabled(&mut self, enabled: bool) -> bool {
        let changed = self.mouse_wheel_enabled != enabled;
        self.mouse_wheel_enabled = enabled;
        changed
    }

    pub fn scroll_step(&self) -> f32 {
        self.scroll_step
    }

    /// Native zero resets the default; finite negative steps reverse direction.
    pub fn set_scroll_step(&mut self, step: f32) -> bool {
        if !step.is_finite() {
            return false;
        }
        let step = if step == 0. { super::SCROLL_STEP } else { step };
        let changed = self.scroll_step != step;
        self.scroll_step = step;
        changed
    }

    /// Native scalar wheel input: positive means toward the content end.
    /// Magnitude is ignored. A horizontal-only overflow redirects the wheel.
    pub fn mouse_wheel(&mut self, delta: f32) -> bool {
        if !self.mouse_wheel_enabled || !delta.is_finite() || delta == 0. {
            return false;
        }
        let overlap = self.max_scroll_pos();
        let horizontal = overlap.x > 0. && overlap.y == 0.;
        let step = if self.page_mode {
            f64::from(if horizontal {
                self.view_size().width
            } else {
                self.view_size().height
            })
        } else {
            f64::from(self.scroll_step) * 2.
        };
        let old = if horizontal {
            self.pos().x
        } else {
            self.pos().y
        };
        let value = (f64::from(old) + step * f64::from(delta.signum()))
            .clamp(-f64::from(f32::MAX), f64::from(f32::MAX)) as f32;
        let next = if horizontal {
            Point::new(value, self.pos().y)
        } else {
            Point::new(self.pos().x, value)
        };
        self.apply_input_position(next, Some(horizontal))
    }

    /// Smooth pixel input is a backend extension. Positive pixels move toward
    /// the end. Page mode retains native page stepping; otherwise axes remain
    /// precise, with vertical-to-horizontal fallback for horizontal-only content.
    pub fn mouse_wheel_pixels(&mut self, x: f32, y: f32) -> bool {
        if !self.mouse_wheel_enabled || !x.is_finite() || !y.is_finite() || (x == 0. && y == 0.) {
            return false;
        }
        if self.page_mode {
            return self.mouse_wheel(if y != 0. { y } else { x });
        }
        let overlap = self.max_scroll_pos();
        let horizontal_only = overlap.x > 0. && overlap.y == 0.;
        let dx = if horizontal_only && x == 0. { y } else { x };
        let next = Point::new(
            self.pos().x + if overlap.x > 0. { dx } else { 0. },
            self.pos().y + if overlap.y > 0. { y } else { 0. },
        );
        self.apply_input_position(next, None)
    }

    pub(super) fn apply_input_position(&mut self, next: Point, axis: Option<bool>) -> bool {
        if !next.x.is_finite() || !next.y.is_finite() {
            return false;
        }
        let max = self.max_scroll_pos();
        let next = Point::new(
            if axis != Some(false) {
                self.loop_check_new_pos(next.x, false)
            } else {
                next.x
            },
            if axis != Some(true) {
                self.loop_check_new_pos(next.y, true)
            } else {
                next.y
            },
        );
        let next = Point::new(
            if axis != Some(false) {
                next.x.clamp(0., max.x)
            } else {
                next.x
            },
            if axis != Some(true) {
                next.y.clamp(0., max.y)
            } else {
                next.y
            },
        );
        if next == self.pos {
            return false;
        }
        if axis.is_some() {
            return self.commit_command_position(next, false);
        }
        self.pos = next;
        self.raw_bounce = false;
        self.motion.tween = None;
        self.motion.pending_release = None;
        self.animation = None;
        // Native immediate refresh floors negative container coordinates.
        self.display_pos = if axis.is_some() {
            Point::new(next.x.ceil(), next.y.ceil())
        } else {
            next
        };
        self.loop_check_current();
        self.note_scroll();
        self.sync_bar_activity();
        true
    }
}

//! Original ScrollPane loop checks, including command-axis asymmetry.
use super::{Point, ScrollPane};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ScrollLoop {
    #[default]
    None,
    Horizontal,
    Vertical,
}

#[derive(Debug, Default, Clone)]
pub(super) struct LoopState {
    pub mode: ScrollLoop,
    pub gaps: Point,
}

impl ScrollPane {
    pub fn loop_mode(&self) -> ScrollLoop {
        self.motion.looping.mode
    }

    pub fn set_loop_mode(&mut self, mode: ScrollLoop, gaps: Point) -> bool {
        if !gaps.x.is_finite() || !gaps.y.is_finite() {
            return false;
        }
        self.motion.looping = LoopState { mode, gaps };
        true
    }

    fn loop_part_size(&self, division: f64, vertical: bool) -> f64 {
        if vertical {
            (f64::from(self.content_size.height) + f64::from(self.motion.looping.gaps.y)) / division
        } else {
            (f64::from(self.content_size.width) + f64::from(self.motion.looping.gaps.x)) / division
        }
    }

    pub(super) fn loop_check_current(&mut self) -> bool {
        let vertical = match self.loop_mode() {
            ScrollLoop::None => return false,
            ScrollLoop::Horizontal => false,
            ScrollLoop::Vertical => true,
        };
        let max = self.max_scroll_pos();
        let max = if vertical { max.y } else { max.x };
        if max <= 0. {
            return false;
        }
        let old = if vertical { self.pos.y } else { self.pos.x };
        let half = self.loop_part_size(2., vertical);
        let value = if old < 0.001 {
            f64::from(old) + half
        } else if old >= max {
            f64::from(old) - half
        } else {
            return false;
        };
        if vertical {
            self.pos.y = value as f32;
        } else {
            self.pos.x = value as f32;
        }
        self.display_pos = Point::new(self.pos.x.ceil(), self.pos.y.ceil());
        true
    }

    pub(super) fn loop_check_new_pos(&mut self, value: f32, vertical: bool) -> f32 {
        // The original setPosY, like setPosX, checks `_loop == 1`.
        if self.loop_mode() != ScrollLoop::Horizontal {
            return value;
        }
        let max = self.max_scroll_pos();
        let max = f64::from(if vertical { max.y } else { max.x });
        if max == 0. {
            return value;
        }
        let pos = f64::from(if vertical { self.pos.y } else { self.pos.x });
        let mut value = f64::from(value);
        let half = self.loop_part_size(2., vertical);
        let sixth = self.loop_part_size(6., vertical);
        let replacement = if value < 0.001 {
            value += half;
            (value > pos && sixth > 0.)
                .then(|| (pos + ((value - pos) / sixth).ceil() * sixth).clamp(0., max))
        } else if value >= max {
            value -= half;
            (value < pos && sixth > 0.)
                .then(|| (pos - ((pos - value) / sixth).ceil() * sixth).clamp(0., max))
        } else {
            None
        };
        if let Some(pos) = replacement {
            if vertical {
                self.display_pos.y = pos.floor() as f32;
            } else {
                self.display_pos.x = pos.floor() as f32;
            }
        }
        value as f32
    }

    pub(super) fn loop_check_target(&self, start: &mut [f64; 2], target: &mut [f64; 2]) {
        let vertical = match self.loop_mode() {
            ScrollLoop::None => return,
            ScrollLoop::Horizontal => false,
            ScrollLoop::Vertical => true,
        };
        let axis = usize::from(vertical);
        let max = self.max_scroll_pos();
        let max = f64::from(if vertical { max.y } else { max.x });
        let half = self.loop_part_size(2., vertical);
        let shift = if target[axis] > 0. {
            -half
        } else if target[axis] < -max {
            half
        } else {
            return;
        };
        let candidate = start[axis] + shift;
        if candidate <= 0. && candidate >= -max {
            start[axis] = candidate;
            target[axis] += shift;
        }
    }
}

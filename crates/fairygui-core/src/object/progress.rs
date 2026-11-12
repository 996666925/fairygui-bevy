use super::*;
use crate::widget::ControlData;

#[derive(Debug, Clone, Copy)]
struct Bar {
    id: GObjectId,
    delta: Size,
    start: Point,
}
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct ProgressBars {
    horizontal: Option<Bar>,
    vertical: Option<Bar>,
}

impl ObjectStore {
    fn remember_progress_bars(&mut self, owner: GObjectId) {
        if self.progress_bars.contains_key(&owner) {
            return;
        }
        let Some(object) = self.object(owner) else {
            return;
        };
        if !matches!(object.control(), ControlData::ProgressBar(_)) {
            return;
        }
        let size = object.size();
        let bar = |name| {
            self.child_by_name(owner, name).and_then(|id| {
                self.object(id).map(|o| Bar {
                    id,
                    delta: Size::new(size.width - o.width(), size.height - o.height()),
                    start: Point::new(o.x(), o.y()),
                })
            })
        };
        let bars = ProgressBars {
            horizontal: bar("bar"),
            vertical: bar("bar_v"),
        };
        self.progress_bars.insert(owner, bars);
    }

    pub(crate) fn initialize_progress(&mut self, owner: GObjectId) {
        self.remember_progress_bars(owner);
        self.refresh_progress_object(owner);
    }

    pub(super) fn refresh_progress_object(&mut self, owner: GObjectId) {
        if !self.progress_bars.contains_key(&owner) {
            return;
        }
        if let Some((min, max, value)) = self.progress_state(owner) {
            self.refresh_progress(owner, value, min, max);
        }
    }

    pub(super) fn refresh_progress_bars(&mut self, owner: GObjectId, percent: f64) {
        self.remember_progress_bars(owner);
        let Some(&bars) = self.progress_bars.get(&owner) else {
            return;
        };
        let Some(object) = self.object(owner) else {
            return;
        };
        let size = object.size();
        let ControlData::ProgressBar(data) = object.control() else {
            return;
        };
        let reverse = data.reverse;
        for (bar, horizontal) in [(bars.horizontal, true), (bars.vertical, false)] {
            let Some(bar) = bar else {
                continue;
            };
            let Some(child) = self.object(bar.id) else {
                continue;
            };
            let has_fill = match child.control() {
                ControlData::Image(d) => d.fill_method != 0,
                ControlData::Loader(d) => d.fill_method != 0,
                _ => false,
            };
            if has_fill {
                self.set_image_fill_amount(bar.id, if reverse { 1.0 - percent } else { percent });
                continue;
            }
            let current = child.size();
            let position = Point::new(child.x(), child.y());
            if horizontal {
                let full = size.width - bar.delta.width;
                let width = (f64::from(full) * percent).round() as f32;
                self.set_size(bar.id, width, current.height);
                if reverse {
                    self.set_xy(bar.id, bar.start.x + full - width, position.y);
                }
            } else {
                let full = size.height - bar.delta.height;
                let height = (f64::from(full) * percent).round() as f32;
                self.set_size(bar.id, current.width, height);
                if reverse {
                    self.set_xy(bar.id, position.x, bar.start.y + full - height);
                }
            }
        }
        if let Some(ani) = self.child_by_name(owner, "ani") {
            self.set_animation_frame(ani, (percent * 100.0).floor() as i32);
        }
    }
}

#[cfg(test)]
#[path = "progress_tests.rs"]
mod tests;

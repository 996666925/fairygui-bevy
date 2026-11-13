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
    displayed: f64,
    tween: Option<ValueTween>,
}

#[derive(Debug, Clone, Copy)]
struct ValueTween {
    start: f64,
    end: f64,
    current: f64,
    elapsed: f64,
    duration: f64,
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
            displayed: self.progress_value(owner).unwrap_or_default(),
            tween: None,
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
        if let Some(value) = self.progress_value(owner) {
            self.update_progress(owner, value);
        }
    }

    /// Native value setter: keep the raw value and clamp only the visual ratio.
    /// Assigning the existing target leaves an active tween running.
    pub fn set_progress(&mut self, owner: GObjectId, value: impl Into<f64>) -> bool {
        let value = value.into();
        if !value.is_finite() || self.progress_value(owner).is_none_or(|old| old == value) {
            return false;
        }
        self.stop_progress_tween(owner);
        if let ControlData::ProgressBar(data) = &mut self.objects[owner].control {
            data.value = value;
        }
        self.initialize_progress(owner);
        true
    }

    pub fn progress_value(&self, owner: GObjectId) -> Option<f64> {
        match self.object(owner)?.control() {
            ControlData::ProgressBar(data) => Some(data.value),
            _ => None,
        }
    }

    /// The value most recently passed to GProgressBar.update, independent of
    /// its target and of the clamped bar percentage.
    pub fn progress_display_value(&self, owner: GObjectId) -> Option<f64> {
        let value = self.progress_value(owner)?;
        Some(
            self.progress_bars
                .get(&owner)
                .map_or(value, |bars| bars.displayed),
        )
    }

    pub fn progress_is_tweening(&self, owner: GObjectId) -> bool {
        self.progress_bars
            .get(&owner)
            .is_some_and(|bars| bars.tween.is_some())
    }

    pub fn set_progress_range(&mut self, owner: GObjectId, min: f64, max: f64) -> bool {
        if !min.is_finite() || !max.is_finite() || min > max {
            return false;
        }
        let Some(ControlData::ProgressBar(data)) =
            self.objects.get_mut(owner).map(|o| &mut o.control)
        else {
            return false;
        };
        if (data.min, data.max) == (min, max) {
            return false;
        }
        data.min = min;
        data.max = max;
        // Native min/max changes redraw the target without stopping the tween.
        self.initialize_progress(owner);
        true
    }

    pub fn set_progress_title_type(&mut self, owner: GObjectId, title_type: i32) -> bool {
        if !(0..=3).contains(&title_type) {
            return false;
        }
        let Some(ControlData::ProgressBar(data)) =
            self.objects.get_mut(owner).map(|o| &mut o.control)
        else {
            return false;
        };
        if data.title_type == title_type {
            return false;
        }
        data.title_type = title_type;
        // GProgressBar.titleType passes the enum value itself to update.
        // Preserve this immediate display; value and any tween are unchanged.
        self.update_progress(owner, f64::from(title_type));
        true
    }

    /// Linear tweenValue: publish the target now, animate the displayed value.
    /// Replacement starts at the old tween's current sample, not its target.
    pub fn tween_progress(&mut self, owner: GObjectId, value: f64, duration: f32) -> bool {
        if !value.is_finite() || !duration.is_finite() || duration < 0. {
            return false;
        }
        let Some(old) = self.progress_value(owner) else {
            return false;
        };
        self.remember_progress_bars(owner);
        let start = self.progress_bars[&owner]
            .tween
            .map_or(old, |tween| tween.current);
        self.stop_progress_tween(owner);
        if let ControlData::ProgressBar(data) = &mut self.objects[owner].control {
            data.value = value;
        }
        if duration == 0. {
            self.update_progress(owner, value);
            self.completed_progress_tweens.push((owner, value));
        } else {
            self.progress_bars.get_mut(&owner).unwrap().tween = Some(ValueTween {
                start,
                end: value,
                current: start,
                elapsed: 0.,
                duration: f64::from(duration),
            });
        }
        true
    }

    /// Freeze the display. The value getter continues to return its target.
    pub fn stop_progress_tween(&mut self, owner: GObjectId) {
        if let Some(bars) = self.progress_bars.get_mut(&owner) {
            bars.tween = None;
        }
        self.completed_progress_tweens
            .retain(|(id, _)| *id != owner);
    }

    pub(super) fn advance_progress_tweens(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0. {
            return;
        }
        let mut updates = Vec::new();
        for (&owner, bars) in &mut self.progress_bars {
            let Some(tween) = &mut bars.tween else {
                continue;
            };
            tween.elapsed = (tween.elapsed + f64::from(dt)).min(tween.duration);
            let done = tween.elapsed >= tween.duration;
            tween.current = if done {
                tween.end
            } else {
                tween.start + (tween.end - tween.start) * (tween.elapsed / tween.duration)
            };
            updates.push((owner, tween.current));
            if done {
                self.completed_progress_tweens.push((owner, tween.end));
                bars.tween = None;
            }
        }
        for (owner, value) in updates {
            self.update_progress(owner, value);
        }
    }

    pub fn take_progress_tween_completions(&mut self) -> Vec<(GObjectId, f64)> {
        std::mem::take(&mut self.completed_progress_tweens)
    }

    fn update_progress(&mut self, owner: GObjectId, value: f64) {
        let Some(ControlData::ProgressBar(data)) = self.object(owner).map(|o| o.control()) else {
            return;
        };
        let (min, max, title_type) = (data.min, data.max, data.title_type);
        let percent = if max == min {
            0.
        } else {
            ((value - min) / (max - min)).clamp(0., 1.)
        };
        self.remember_progress_bars(owner);
        self.progress_bars.get_mut(&owner).unwrap().displayed = value;
        self.refresh_progress_bars(owner, percent);
        if let Some(title) = self.child_by_name(owner, "title") {
            let text = match title_type {
                0 => format!("{}%", (percent * 100.).floor()),
                1 => format!("{}/{}", value.floor(), max.floor()),
                2 => value.floor().to_string(),
                3 => max.floor().to_string(),
                _ => return,
            };
            self.set_object_text(title, &text);
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

#[cfg(test)]
#[path = "progress_tween_tests.rs"]
mod tween_tests;

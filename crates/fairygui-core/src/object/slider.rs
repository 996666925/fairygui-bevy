//! Slider geometry, fractional values and native input percentages.
use super::*;
use crate::widget::ControlData;

#[cfg(test)]
#[path = "slider_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug)]
struct Bar {
    id: GObjectId,
    delta: Size,
    start: Point,
}
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct SliderBars {
    horizontal: Option<Bar>,
    vertical: Option<Bar>,
}

impl ObjectStore {
    fn remember_slider_bars(&mut self, owner: GObjectId) {
        if self.slider_bars.contains_key(&owner) {
            return;
        }
        let Some(object) = self.object(owner) else {
            return;
        };
        if !matches!(object.control(), ControlData::Slider(_)) {
            return;
        }
        let size = object.size();
        let bar = |name| {
            self.child_by_name(owner, name).and_then(|id| {
                self.object(id).map(|o| Bar {
                    id,
                    delta: Size::new(size.width - o.width(), size.height - o.height()),
                    start: o.position(),
                })
            })
        };
        self.slider_bars.insert(
            owner,
            SliderBars {
                horizontal: bar("bar"),
                vertical: bar("bar_v"),
            },
        );
    }
    pub(crate) fn initialize_slider(&mut self, owner: GObjectId) {
        self.remember_slider_bars(owner);
        self.refresh_slider_object(owner);
    }
    pub(super) fn refresh_slider_object(&mut self, owner: GObjectId) {
        let Some(bars) = self.slider_bars.get(&owner).copied() else {
            return;
        };
        let Some(object) = self.object(owner) else {
            return;
        };
        let ControlData::Slider(data) = object.control() else {
            return;
        };
        let percent = self.slider_percent(owner).unwrap_or_default();
        let (value, max, title_type, reverse, size) = (
            data.value,
            data.max,
            data.title_type,
            data.reverse,
            object.size(),
        );
        if let Some(title) = self.child_by_name(owner, "title") {
            let text = match title_type {
                0 => format!("{}%", (percent * 100.).floor()),
                1 => format!("{value}/{max}"),
                2 => value.to_string(),
                3 => max.to_string(),
                _ => String::new(),
            };
            self.set_object_text(title, &text);
        }
        for (bar, horizontal) in [(bars.horizontal, true), (bars.vertical, false)] {
            let Some(bar) = bar else { continue };
            let Some(child) = self.object(bar.id) else {
                continue;
            };
            let current = child.size();
            let position = child.position();
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
    }
    /// Programmatic values retain their fractional and out-of-range values;
    /// only the displayed percentage is clamped, as in GSlider.value.
    pub fn set_slider_value(&mut self, owner: GObjectId, value: impl Into<f64>) -> bool {
        let value = value.into();
        if !value.is_finite() {
            return false;
        }
        let Some(ControlData::Slider(data)) = self.objects.get_mut(owner).map(|o| &mut o.control)
        else {
            return false;
        };
        if data.value == value {
            return false;
        }
        data.value = value;
        self.initialize_slider(owner);
        true
    }
    pub fn slider_value(&self, owner: GObjectId) -> Option<f64> {
        let ControlData::Slider(data) = self.object(owner)?.control() else {
            return None;
        };
        Some(data.value)
    }
    pub fn slider_percent(&self, owner: GObjectId) -> Option<f64> {
        let ControlData::Slider(data) = self.object(owner)?.control() else {
            return None;
        };
        Some(if data.max == data.min {
            0.
        } else {
            ((data.value - data.min) / (data.max - data.min)).clamp(0., 1.)
        })
    }
    pub fn set_slider_range(&mut self, owner: GObjectId, min: f64, max: f64) -> bool {
        if !min.is_finite() || !max.is_finite() || min > max {
            return false;
        }
        let Some(ControlData::Slider(data)) = self.objects.get_mut(owner).map(|o| &mut o.control)
        else {
            return false;
        };
        if (data.min, data.max) == (min, max) {
            return false;
        }
        data.min = min;
        data.max = max;
        self.initialize_slider(owner);
        true
    }
    pub fn set_slider_whole_numbers(&mut self, owner: GObjectId, whole: bool) -> bool {
        let Some(ControlData::Slider(data)) = self.objects.get_mut(owner).map(|o| &mut o.control)
        else {
            return false;
        };
        if data.whole_numbers == whole {
            return false;
        }
        data.whole_numbers = whole;
        self.initialize_slider(owner);
        true
    }
    pub fn set_slider_change_on_click(&mut self, owner: GObjectId, change: bool) -> bool {
        let Some(ControlData::Slider(data)) = self.objects.get_mut(owner).map(|o| &mut o.control)
        else {
            return false;
        };
        let changed = data.change_on_click != change;
        data.change_on_click = change;
        changed
    }
    pub fn set_slider_can_drag(&mut self, owner: GObjectId, can_drag: bool) -> bool {
        let Some(ControlData::Slider(data)) = self.objects.get_mut(owner).map(|o| &mut o.control)
        else {
            return false;
        };
        let changed = data.can_drag != can_drag;
        data.can_drag = can_drag;
        changed
    }
    pub fn set_slider_title_type(&mut self, owner: GObjectId, title_type: i32) -> bool {
        if !(0..=3).contains(&title_type) {
            return false;
        }
        let Some(ControlData::Slider(data)) = self.objects.get_mut(owner).map(|o| &mut o.control)
        else {
            return false;
        };
        if data.title_type == title_type {
            return false;
        }
        data.title_type = title_type;
        self.initialize_slider(owner);
        true
    }
    /// Apply an input percentage. Whole-number rounding follows JS Math.round,
    /// including negative half values. Programmatic assignments do not round.
    pub fn set_slider_percent_from_input(&mut self, owner: GObjectId, percent: f64) -> bool {
        if !percent.is_finite() {
            return false;
        }
        let Some(ControlData::Slider(data)) = self.object(owner).map(|o| o.control()) else {
            return false;
        };
        let value = data.min + (data.max - data.min) * percent.clamp(0., 1.);
        let value = if data.whole_numbers {
            (value + 0.5).floor()
        } else {
            value
        };
        self.set_slider_value(owner, value)
    }
    pub fn slider_track(&self, owner: GObjectId) -> Option<(bool, f32)> {
        let bars = self.slider_bars.get(&owner)?;
        let size = self.object(owner)?.size();
        if let Some(bar) = bars.horizontal {
            Some((false, size.width - bar.delta.width))
        } else {
            bars.vertical
                .map(|bar| (true, size.height - bar.delta.height))
        }
    }
    /// Native track click uses the pointer in the grip's local coordinates.
    pub fn click_slider_at(&mut self, owner: GObjectId, position: Point) -> bool {
        let Some(ControlData::Slider(data)) = self.object(owner).map(|o| o.control()) else {
            return false;
        };
        if !data.change_on_click {
            return false;
        }
        let reverse = data.reverse;
        let Some(grip) = self.child_by_name(owner, "grip") else {
            return false;
        };
        let Some(point) = self
            .local_to_global(owner, position)
            .and_then(|point| self.global_to_local(grip, point))
        else {
            return false;
        };
        self.remember_slider_bars(owner);
        let Some((vertical, track)) = self.slider_track(owner).filter(|(_, track)| *track > 0.)
        else {
            return false;
        };
        let delta = f64::from(if vertical { point.y } else { point.x }) / f64::from(track);
        let percent =
            self.slider_percent(owner).unwrap_or_default() + if reverse { -delta } else { delta };
        self.set_slider_percent_from_input(owner, percent)
    }
    /// Absolute point helper, independent of the relative grip drag gesture.
    pub fn slider_value_at(&self, owner: GObjectId, position: Point) -> Option<f64> {
        let object = self.object(owner)?;
        let ControlData::Slider(data) = object.control() else {
            return None;
        };
        let (vertical, track) = self.slider_track(owner).unwrap_or((
            object.height() > object.width(),
            if object.height() > object.width() {
                object.height()
            } else {
                object.width()
            },
        ));
        if track <= 0. {
            return Some(data.min);
        }
        let percent = (f64::from(if vertical { position.y } else { position.x })
            / f64::from(track))
        .clamp(0., 1.);
        let percent = if data.reverse { 1. - percent } else { percent };
        let value = data.min + (data.max - data.min) * percent;
        Some(if data.whole_numbers {
            (value + 0.5).floor()
        } else {
            value
        })
    }
    pub fn set_slider_value_at(&mut self, owner: GObjectId, position: Point) -> bool {
        let Some(value) = self.slider_value_at(owner, position) else {
            return false;
        };
        self.set_slider_value(owner, value)
    }
}

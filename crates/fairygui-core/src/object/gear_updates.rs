use super::{ObjectStore, dirty};
use crate::{
    color::Color,
    object::GObjectId,
    widget::{ControlData, GearKind, GearValue},
};

impl ObjectStore {
    /// Native locks are per object: forwarded properties on another object can
    /// still update that other object's gears. Preserve nested caller locks.
    pub(crate) fn with_gear_lock<T>(
        &mut self,
        id: GObjectId,
        apply: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let previous = self.objects.get(id).is_some_and(|o| o.gear_locked);
        if let Some(object) = self.objects.get_mut(id) {
            object.gear_locked = true;
        }
        let result = apply(self);
        if let Some(object) = self.objects.get_mut(id) {
            object.gear_locked = previous;
        }
        result
    }

    pub(crate) fn text_field_of(&self, id: GObjectId) -> Option<GObjectId> {
        let object = self.object(id)?;
        if object.control().as_text_field().is_some() {
            return Some(id);
        }
        if matches!(
            object.control(),
            ControlData::Button(_) | ControlData::Label(_) | ControlData::ComboBox(_)
        ) {
            return self
                .child_by_name(id, "title")
                .and_then(|id| self.text_field_of(id));
        }
        None
    }

    pub(crate) fn gear_snapshot(&self, id: GObjectId, kind: GearKind) -> Option<GearValue> {
        let object = self.object(id)?;
        let black = Color::from_rgba8(0, 0, 0, 255);
        Some(match kind {
            GearKind::Xy => {
                let parent = object.parent().and_then(|id| self.object(id));
                GearValue::Xy {
                    x: object.x(),
                    y: object.y(),
                    px: object.x() / parent.map_or(0.0, |p| p.width()),
                    py: object.y() / parent.map_or(0.0, |p| p.height()),
                }
            }
            GearKind::Size => GearValue::Size {
                width: object.width(),
                height: object.height(),
                scale_x: object.scale_x(),
                scale_y: object.scale_y(),
            },
            GearKind::Look => GearValue::Look {
                alpha: object.alpha(),
                rotation: object.rotation(),
                grayed: object.grayed(),
                touchable: object.touchable(),
            },
            GearKind::Color => {
                let (color, stroke_color) = match object.control() {
                    ControlData::Image(d) => (d.color.unwrap_or(Color::WHITE), Color::TRANSPARENT),
                    ControlData::MovieClip(d) => {
                        (d.color.unwrap_or(Color::WHITE), Color::TRANSPARENT)
                    }
                    ControlData::Loader(d) => (d.color.unwrap_or(Color::WHITE), Color::TRANSPARENT),
                    ControlData::Graph(d) => (d.fill_color, Color::TRANSPARENT),
                    _ => self
                        .text_field_of(id)
                        .and_then(|id| self.object(id)?.control().as_text_field())
                        .map(|d| (d.color, d.stroke_color.unwrap_or(black)))
                        .unwrap_or((
                            if matches!(
                                object.control(),
                                ControlData::Button(_)
                                    | ControlData::Label(_)
                                    | ControlData::ComboBox(_)
                            ) {
                                black
                            } else {
                                Color::TRANSPARENT
                            },
                            Color::TRANSPARENT,
                        )),
                };
                GearValue::Color {
                    color,
                    stroke_color,
                }
            }
            GearKind::Animation => {
                let (playing, frame) = self.animation_state(id).unwrap_or((false, 0));
                GearValue::Animation { playing, frame }
            }
            GearKind::Text => {
                let text = match object.control() {
                    ControlData::Text(d) => d.text.clone(),
                    ControlData::TextInput(d) => d.text_field.text.clone(),
                    ControlData::Button(d) => d.title.clone(),
                    ControlData::Label(_) => self
                        .child_by_name(id, "title")
                        .and_then(|id| self.gear_snapshot(id, kind))
                        .and_then(|v| {
                            if let GearValue::Text(text) = v {
                                Some(text)
                            } else {
                                None
                            }
                        })
                        .unwrap_or_default(),
                    ControlData::ComboBox(_) => self
                        .child_by_name(id, "title")
                        .and_then(|id| self.gear_snapshot(id, kind))
                        .and_then(|v| {
                            if let GearValue::Text(text) = v {
                                Some(text)
                            } else {
                                None
                            }
                        })
                        .unwrap_or_default(),
                    _ => String::new(),
                };
                GearValue::Text(text)
            }
            GearKind::Icon => {
                let icon = match object.control() {
                    ControlData::Loader(d) => d.url.clone().unwrap_or_default(),
                    ControlData::Button(d) => d.icon.clone().unwrap_or_default(),
                    ControlData::Label(_) => self
                        .child_by_name(id, "icon")
                        .and_then(|id| self.gear_snapshot(id, kind))
                        .and_then(|v| {
                            if let GearValue::Icon(icon) = v {
                                Some(icon)
                            } else {
                                None
                            }
                        })
                        .unwrap_or_default(),
                    ControlData::ComboBox(_) => self
                        .child_by_name(id, "icon")
                        .and_then(|id| self.gear_snapshot(id, kind))
                        .and_then(|v| {
                            if let GearValue::Icon(icon) = v {
                                Some(icon)
                            } else {
                                None
                            }
                        })
                        .unwrap_or_default(),
                    _ => String::new(),
                };
                GearValue::Icon(icon)
            }
            GearKind::FontSize => GearValue::FontSize(
                self.text_field_of(id)
                    .and_then(|id| self.object(id)?.control().as_text_field())
                    .map_or(0, |d| d.font_size),
            ),
            GearKind::Display | GearKind::Display2 => return None,
        })
    }

    /// GObject.updateGear: capture only a bound gear on the active controller
    /// page. The null page has its own storage; it must not replace _default.
    pub(crate) fn update_gear_state(&mut self, id: GObjectId, kind: GearKind) {
        if self
            .object(id)
            .is_none_or(|o| o.under_construct || o.gear_locked)
        {
            return;
        }
        self.capture_gear_state(id, kind);
    }

    fn capture_gear_state(&mut self, id: GObjectId, kind: GearKind) {
        let Some(object) = self.object(id) else {
            return;
        };
        let Some(index) = object.gears.iter().position(|g| g.kind == kind) else {
            return;
        };
        let gear = &object.gears[index];
        if gear.controller_index < 0 {
            return;
        }
        let Some(owner) = gear.controller_owner.or(object.parent()) else {
            return;
        };
        let Some(controller) = self
            .object(owner)
            .and_then(|o| o.control().component())
            .and_then(|d| d.controllers.get(gear.controller_index as usize))
        else {
            return;
        };
        let page = controller.selected_page_id().map(str::to_owned);
        let Some(value) = self.gear_snapshot(id, kind) else {
            return;
        };
        let gear = &mut self.objects[id].gears[index];
        gear.controller_owner = Some(owner);
        if let Some(page) = page {
            gear.values.insert(Some(page), value);
        } else {
            gear.unselected_value = Some(value);
        }
    }

    /// RelationItem shifts every saved absolute page/default, then records the
    /// current value directly. Percent XY bypasses this adjustment in native.
    pub(crate) fn update_gear_relations(
        &mut self,
        id: GObjectId,
        kind: GearKind,
        dx: f32,
        dy: f32,
    ) {
        let Some(object) = self.objects.get_mut(id) else {
            return;
        };
        let Some(gear) = object.gears.iter_mut().find(|g| g.kind == kind) else {
            return;
        };
        if gear.controller_index < 0 || (kind == GearKind::Xy && gear.positions_in_percent) {
            return;
        }
        for value in gear
            .values
            .values_mut()
            .chain(gear.unselected_value.iter_mut())
        {
            match value {
                GearValue::Xy { x, y, .. } => {
                    *x += dx;
                    *y += dy;
                }
                GearValue::Size { width, height, .. } => {
                    *width += dx;
                    *height += dy;
                }
                _ => {}
            }
        }
        self.capture_gear_state(id, kind);
    }

    /// GTextField.fontSize changes display state but does not call updateGear(9)
    /// in the reference Laya implementation. Compound controls forward it.
    pub fn set_object_font_size(&mut self, id: GObjectId, size: i32) -> bool {
        let Some(target) = self.text_field_of(id) else {
            return false;
        };
        let changed = match &mut self.objects[target].control {
            ControlData::Text(d) => {
                let changed = d.font_size != size;
                d.font_size = size;
                changed
            }
            ControlData::TextInput(d) => {
                let changed = d.text_field.font_size != size;
                d.text_field.font_size = size;
                changed
            }
            _ => false,
        };
        if let Some(object) = self.objects.get_mut(id) {
            match &mut object.control {
                ControlData::Button(d) => d.title_font_size = size,
                ControlData::Label(d) => d.title_font_size = size,
                _ => {}
            }
        }
        if changed {
            self.touch(target, dirty::TEXT);
        }
        changed
    }

    pub(crate) fn initialize_resource_title(&mut self, id: GObjectId) {
        if !matches!(
            self.object(id).map(|o| o.control()),
            Some(ControlData::Button(_) | ControlData::Label(_) | ControlData::ComboBox(_))
        ) {
            return;
        }
        let title = self
            .child_by_name(id, "title")
            .and_then(|id| self.gear_snapshot(id, GearKind::Text));
        let icon = self
            .child_by_name(id, "icon")
            .and_then(|id| self.gear_snapshot(id, GearKind::Icon));
        let (title_out, icon_out) = match &mut self.objects[id].control {
            ControlData::Button(d) => (&mut d.title, &mut d.icon),
            ControlData::Label(d) => (&mut d.title, &mut d.icon),
            ControlData::ComboBox(d) => (&mut d.title, &mut d.icon),
            _ => return,
        };
        if let Some(GearValue::Text(title)) = title {
            *title_out = title;
        }
        if let Some(GearValue::Icon(icon)) = icon {
            *icon_out = Some(icon);
        }
    }
}

#[cfg(test)]
#[path = "gear_updates_tests.rs"]
mod tests;

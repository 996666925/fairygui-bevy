//! Native combo setters and parent selection-controller synchronization.
use super::*;
use crate::widget::ControlData;

#[cfg(test)]
#[path = "combo_tests.rs"]
mod tests;

impl ObjectStore {
    /// Native dropdown item selection forces the title/icon to be reapplied on
    /// reselect, without a transient -1 assignment to the selection controller.
    /// The input adapter is responsible for emitting the user change event.
    pub fn select_combo_index_from_input(&mut self, combo: GObjectId, index: i32) -> bool {
        let Some(ControlData::ComboBox(data)) = self.objects.get_mut(combo).map(|o| &mut o.control)
        else {
            return false;
        };
        data.selected_index = -1;
        self.select_combo_index(combo, index)
    }

    /// Native items setter: retain selection, clamp to the last remaining item,
    /// or select the first item when the previous index was -1. This setter does
    /// not update the selection controller or emit a user selection event.
    pub fn set_combo_items(&mut self, combo: GObjectId, items: Vec<String>) -> bool {
        let Some(ControlData::ComboBox(data)) = self.objects.get_mut(combo).map(|o| &mut o.control)
        else {
            return false;
        };
        data.items = items;
        let mut icon = None;
        let title = if data.items.is_empty() {
            data.selected_index = -1;
            if data.icons.is_some() {
                icon = Some(None);
            }
            String::new()
        } else {
            if data.selected_index >= data.items.len() as i32 {
                data.selected_index = data.items.len() as i32 - 1;
            } else if data.selected_index == -1 {
                data.selected_index = 0;
            }
            let index = usize::try_from(data.selected_index).ok();
            let title = index
                .and_then(|index| data.items.get(index))
                .cloned()
                .unwrap_or_default();
            if let Some(value) = data
                .icons
                .as_ref()
                .and_then(|icons| index.and_then(|index| icons.get(index)))
            {
                icon = Some(value.clone());
            }
            title
        };
        self.set_object_text(combo, &title);
        if let Some(icon) = icon {
            self.apply_icon_to(combo, icon.as_deref().unwrap_or_default());
        }
        true
    }

    /// Values are independent of captions and need not have the same length.
    pub fn set_combo_values(&mut self, combo: GObjectId, values: Vec<String>) -> bool {
        let Some(ControlData::ComboBox(data)) = self.objects.get_mut(combo).map(|o| &mut o.control)
        else {
            return false;
        };
        data.values = values;
        true
    }

    /// None removes the icon array while preserving the displayed icon. A
    /// present but short array preserves the icon of an uncovered valid item.
    pub fn set_combo_icons(
        &mut self,
        combo: GObjectId,
        icons: Option<Vec<Option<String>>>,
    ) -> bool {
        let Some(ControlData::ComboBox(data)) = self.objects.get_mut(combo).map(|o| &mut o.control)
        else {
            return false;
        };
        data.icons = icons;
        let icon = usize::try_from(data.selected_index)
            .ok()
            .and_then(|index| data.icons.as_ref()?.get(index))
            .cloned();
        if let Some(icon) = icon {
            self.apply_icon_to(combo, icon.as_deref().unwrap_or_default());
        }
        true
    }

    /// Missing values cancel selection; None follows the native null/empty
    /// value fallback. Out-of-range selection indices themselves are retained.
    pub fn select_combo_value(&mut self, combo: GObjectId, value: Option<&str>) -> bool {
        let Some(ControlData::ComboBox(data)) = self.object(combo).map(|o| o.control()) else {
            return false;
        };
        let index = data
            .index_of_value(value.unwrap_or_default())
            .map_or(-1, |index| index as i32);
        self.select_combo_index(combo, index)
    }

    pub fn set_combo_visible_item_count(&mut self, combo: GObjectId, count: i32) -> bool {
        let Some(ControlData::ComboBox(data)) = self.objects.get_mut(combo).map(|o| &mut o.control)
        else {
            return false;
        };
        data.visible_item_count = count;
        true
    }

    /// 0 = Auto, 1 = Up, 2 = Down, matching the exported PopupDirection enum.
    pub fn set_combo_popup_direction(&mut self, combo: GObjectId, direction: i32) -> bool {
        if !(0..=2).contains(&direction) {
            return false;
        }
        let Some(ControlData::ComboBox(data)) = self.objects.get_mut(combo).map(|o| &mut o.control)
        else {
            return false;
        };
        data.popup_direction = direction;
        true
    }

    /// Bind by parent controller name, or detach with None. Like the native
    /// setter, binding alone does not change the selection or controller page.
    pub fn set_combo_selection_controller(&mut self, combo: GObjectId, name: Option<&str>) -> bool {
        let Some(object) = self.object(combo) else {
            return false;
        };
        if !matches!(object.control(), ControlData::ComboBox(_)) {
            return false;
        }
        let index = if let Some(name) = name {
            let Some(index) = object
                .parent()
                .and_then(|parent| self.object(parent))
                .and_then(|parent| parent.control().component())
                .and_then(|data| {
                    data.controllers
                        .iter()
                        .position(|controller| controller.name == name)
                })
            else {
                return false;
            };
            index as i32
        } else {
            -1
        };
        let ControlData::ComboBox(data) = &mut self.objects[combo].control else {
            unreachable!()
        };
        data.selection_controller = index;
        true
    }

    pub fn combo_selection_controller(&self, combo: GObjectId) -> Option<&str> {
        let (parent, index) = self.combo_controller(combo)?;
        Some(
            &self
                .object(parent)?
                .control()
                .component()?
                .controllers
                .get(index)?
                .name,
        )
    }

    fn combo_controller(&self, combo: GObjectId) -> Option<(GObjectId, usize)> {
        let object = self.object(combo)?;
        let ControlData::ComboBox(data) = object.control() else {
            return None;
        };
        Some((
            object.parent()?,
            usize::try_from(data.selection_controller).ok()?,
        ))
    }

    pub(super) fn update_combo_selection_controller(&mut self, combo: GObjectId) {
        let Some((parent, index)) = self.combo_controller(combo) else {
            return;
        };
        let Some(controller) = self
            .object(parent)
            .and_then(|o| o.control().component())
            .and_then(|data| data.controllers.get(index))
        else {
            return;
        };
        let ControlData::ComboBox(data) = self.object(combo).unwrap().control() else {
            return;
        };
        let selected = data.selected_index;
        if selected >= controller.page_count() as i32 {
            return;
        }
        let name = controller.name.clone();
        if self.changing_controllers.contains(&(parent, name.clone())) {
            return;
        }
        // Native temporarily detaches the source combo while updating its
        // controller, so clamping controller indices cannot rewrite that combo.
        let ControlData::ComboBox(data) = &mut self.objects[combo].control else {
            return;
        };
        let binding = data.selection_controller;
        data.selection_controller = -1;
        self.set_controller_page_index(parent, &name, selected);
        if let Some(ControlData::ComboBox(data)) =
            self.objects.get_mut(combo).map(|o| &mut o.control)
        {
            data.selection_controller = binding;
        }
    }

    pub(super) fn sync_related_combos(&mut self, parent: GObjectId, name: &str) {
        let Some((controller_index, selected)) = self
            .object(parent)
            .and_then(|o| o.control().component())
            .and_then(|data| {
                data.controllers
                    .iter()
                    .enumerate()
                    .find(|(_, c)| c.name == name)
            })
            .map(|(index, controller)| (index, controller.selected_index))
        else {
            return;
        };
        let combos: Vec<_> = self.object(parent).unwrap().children().iter().copied()
            .filter(|&id| matches!(self.object(id).map(|o| o.control()), Some(ControlData::ComboBox(data)) if data.selection_controller == controller_index as i32)).collect();
        for combo in combos {
            self.select_combo_index(combo, selected);
        }
    }
}

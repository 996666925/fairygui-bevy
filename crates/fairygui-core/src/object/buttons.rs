use super::{ObjectStore, dirty};
use crate::object::GObjectId;
use crate::widget::{ButtonMode, ButtonState, ControlData};

#[derive(Debug)]
pub(super) struct ButtonFeedback {
    pub button: GObjectId,
    deadline: f64,
    state: ButtonState,
}

impl ObjectStore {
    /// Native GButton.fireClick: execute internal click logic immediately.
    /// Returns None for non-buttons, otherwise whether selection changed.
    /// This does not emit Click, synthesize pointer state or open linked popups.
    /// The adapter publishes StateChanged when the returned value is Some(true).
    pub fn fire_button_click(&mut self, id: GObjectId, down_effect: bool) -> Option<bool> {
        let ControlData::Button(data) = self.object(id)?.control() else {
            return None;
        };
        if down_effect && data.mode == ButtonMode::Common {
            self.apply_button_visual_state(id, ButtonState::Over);
            for (delay, state) in [(0.1, ButtonState::Down), (0.2, ButtonState::Up)] {
                self.button_feedback.push(ButtonFeedback {
                    button: id,
                    deadline: self.button_feedback_time + delay,
                    state,
                });
            }
        }
        Some(self.click_button(id))
    }

    /// Advance fireClick's two independent one-shot callbacks. Repeated calls
    /// append timers (native coverBefore=false), preserving registration order
    /// when multiple callbacks become due in the same frame. Timers operate on
    /// visual state directly, regardless of hover/press/grayed or stage state.
    pub fn advance_button_feedback(&mut self, delta: f64) {
        if !delta.is_finite() || delta < 0.0 {
            return;
        }
        let next = self.button_feedback_time + delta;
        if !next.is_finite() {
            return;
        }
        self.button_feedback_time = next;
        let mut pending = Vec::new();
        for feedback in std::mem::take(&mut self.button_feedback) {
            if !self.contains(feedback.button) {
                continue;
            }
            if feedback.deadline <= self.button_feedback_time + 1e-9 {
                self.apply_button_visual_state(feedback.button, feedback.state);
            } else {
                pending.push(feedback);
            }
        }
        self.button_feedback = pending;
    }

    pub(crate) fn initialize_button(&mut self, id: GObjectId) {
        if !matches!(
            self.objects.get(id).map(|o| &o.control),
            Some(ControlData::Button(_))
        ) {
            return;
        }
        let previous = self.initializing_controllers;
        self.initializing_controllers = true;
        if matches!(&self.objects[id].control, ControlData::Button(data) if data.mode == ButtonMode::Common || data.selected)
        {
            self.refresh_button_state(id);
        }
        self.apply_title(id);
        self.initializing_controllers = previous;
    }
    /// 关闭后仍可播放点击音效与投递 Click，点击不改变 Check/Radio 选择。
    pub fn set_button_change_state_on_click(&mut self, id: GObjectId, enabled: bool) {
        if let Some(ControlData::Button(data)) = self.objects.get_mut(id).map(|o| &mut o.control) {
            data.change_state_on_click = enabled;
        }
    }
    /// 按钮是否选中；普通按钮始终不参与选择。
    pub fn button_selected(&self, id: GObjectId) -> Option<bool> {
        match &self.objects.get(id)?.control {
            ControlData::Button(data) => Some(data.mode != ButtonMode::Common && data.selected),
            _ => None,
        }
    }

    /// 设置选择状态，同时更新外观、标题/图标与关联控制器。
    pub fn set_button_selected(&mut self, id: GObjectId, selected: bool) -> bool {
        self.change_button_selection(id, selected, true)
    }

    /// 执行按钮自身的点击行为，返回选择状态是否改变。
    /// StateChanged 事件由输入层投递；程序设置 selected 不投递事件。
    pub fn click_button(&mut self, id: GObjectId) -> bool {
        let Some(ControlData::Button(data)) = self.objects.get(id).map(|o| &o.control) else {
            return false;
        };
        let mode = data.mode;
        let selected = data.selected;
        let change = data.change_state_on_click;
        if let Some(sound) = data.sound.clone().filter(|sound| !sound.is_empty()) {
            let volume = data.sound_volume_scale;
            self.queue_sound(id, sound, volume);
        }
        match mode {
            ButtonMode::Check if change => self.set_button_selected(id, !selected),
            ButtonMode::Radio if change && !selected => self.set_button_selected(id, true),
            ButtonMode::Common => {
                self.select_button_page(id, false);
                false
            }
            _ => false,
        }
    }

    fn change_button_selection(&mut self, id: GObjectId, selected: bool, notify: bool) -> bool {
        let Some(ControlData::Button(data)) = self.objects.get_mut(id).map(|o| &mut o.control)
        else {
            return false;
        };
        if data.mode == ButtonMode::Common || data.selected == selected {
            return false;
        }
        data.selected = selected;
        self.refresh_button_state(id);
        self.apply_title(id);
        if notify {
            self.select_button_page(id, !selected);
        }
        if selected {
            self.adjust_button_depth(id);
        }
        if let Some(parent) = self.object(id).and_then(|o| o.parent()) {
            self.refresh_list_selection_snapshot(parent);
        }
        true
    }

    fn select_button_page(&mut self, id: GObjectId, opposite: bool) {
        let Some(object) = self.objects.get(id) else {
            return;
        };
        let ControlData::Button(button) = &object.control else {
            return;
        };
        let Some(parent) = object.parent else { return };
        let Some(controller) = self
            .objects
            .get(parent)
            .and_then(|o| o.control.component())
            .and_then(|data| data.controllers.get(button.related_controller as usize))
        else {
            return;
        };
        let index = button
            .related_page_id
            .as_deref()
            .and_then(|page| controller.index_of_id(page))
            .map_or(-1, |index| index as i32);
        if opposite && (button.mode != ButtonMode::Check || controller.selected_index != index) {
            return;
        }
        // Controller.oppositePageId chooses page 0, except when leaving page 0.
        let index = if opposite {
            if index > 0 {
                0
            } else if controller.page_count() > 1 {
                1
            } else {
                return;
            }
        } else {
            index
        };
        let name = controller.name.clone();
        self.set_controller_page_index(parent, &name, index);
    }

    pub(super) fn refresh_button_state(&mut self, id: GObjectId) -> bool {
        let Some(object) = self.objects.get(id) else {
            return false;
        };
        let ControlData::Button(data) = &object.control else {
            return false;
        };
        let controller = data.component.controller("button");
        let selected = data.selected && data.mode != ButtonMode::Common;
        let state = if object.grayed && controller.is_some_and(|c| c.has_page("disabled")) {
            if selected && controller.is_some_and(|c| c.has_page("selectedDisabled")) {
                ButtonState::SelectedDisabled
            } else {
                ButtonState::Disabled
            }
        } else if selected {
            if data.hovered {
                ButtonState::SelectedOver
            } else {
                ButtonState::Down
            }
        } else if data.hovered {
            ButtonState::Over
        } else {
            ButtonState::Up
        };
        self.apply_button_visual_state(id, state)
    }

    pub(super) fn refresh_button_grayed_state(&mut self, id: GObjectId) {
        let Some(object) = self.object(id) else {
            return;
        };
        let ControlData::Button(data) = object.control() else {
            return;
        };
        let selected = data.selected && data.mode != ButtonMode::Common;
        // Native handleGrayedChanged picks the base selected/up state on enable,
        // regardless of a previously recorded rollover or current pointer press.
        let state = match (object.grayed(), selected) {
            (true, true)
                if data
                    .component
                    .controller("button")
                    .is_some_and(|c| c.has_page("selectedDisabled")) =>
            {
                ButtonState::SelectedDisabled
            }
            (true, true) => ButtonState::Disabled,
            (true, false) => ButtonState::Disabled,
            (false, true) => ButtonState::Down,
            (false, false) => ButtonState::Up,
        };
        self.apply_button_visual_state(id, state);
    }

    pub(super) fn apply_button_visual_state(&mut self, id: GObjectId, state: ButtonState) -> bool {
        let Some(object) = self.objects.get_mut(id) else {
            return false;
        };
        let ControlData::Button(data) = &mut object.control else {
            return false;
        };
        data.interaction_state = state;
        let effect = data.down_effect;
        let value = data.down_effect_value;
        let down = matches!(
            state,
            ButtonState::Down | ButtonState::SelectedOver | ButtonState::SelectedDisabled
        );
        let scale = if effect == 2 && down != data.down_scaled && value.is_finite() && value > 0.0 {
            data.down_scaled = down;
            let factor = if down { value } else { value.recip() };
            Some(factor)
        } else {
            None
        };
        let children = if effect == 1 {
            object.children.clone()
        } else {
            Vec::new()
        };
        let changed = self.set_controller_page(id, "button", state.page_name());
        if let Some(factor) = scale {
            let object = self.objects.get(id).unwrap();
            self.set_scale(id, object.scale_x * factor, object.scale_y * factor);
        }
        if effect == 1 {
            // Native writes the supported Color property, excluding direct text
            // children. It restores white rather than the previous custom tint.
            let channel = if down {
                (value * 255.0).clamp(0.0, 255.0) as u8
            } else {
                255
            };
            let color = crate::color::Color::from_rgba8(channel, channel, channel, 255);
            for child in children {
                if self
                    .objects
                    .get(child)
                    .is_some_and(|o| o.control.as_text_field().is_none())
                {
                    self.set_control_color(child, color, None);
                }
            }
        }
        changed
    }

    /// Native rollover/out only updates a button that has an over controller page.
    /// Holding the mouse preserves its current visual state until mouse-up.
    pub fn set_button_hovered(&mut self, id: GObjectId, hovered: bool) {
        let Some(object) = self.objects.get_mut(id) else {
            return;
        };
        let ControlData::Button(data) = &mut object.control else {
            return;
        };
        if !data
            .component
            .controller("button")
            .is_some_and(|c| c.has_page("over"))
        {
            return;
        }
        data.hovered = hovered;
        if data.pressed
            || (object.grayed
                && data
                    .component
                    .controller("button")
                    .is_some_and(|c| c.has_page("disabled")))
        {
            return;
        }
        let selected = data.selected && data.mode != ButtonMode::Common;
        let state = match (selected, hovered) {
            (true, true) => ButtonState::SelectedOver,
            (true, false) => ButtonState::Down,
            (false, true) => ButtonState::Over,
            (false, false) => ButtonState::Up,
        };
        self.apply_button_visual_state(id, state);
    }

    /// Common buttons change visual state on press/release. Check/Radio buttons
    /// keep their selection-driven state, matching GButton's mouse handlers.
    pub fn set_button_pressed(&mut self, id: GObjectId, pressed: bool) {
        let Some(object) = self.objects.get_mut(id) else {
            return;
        };
        let ControlData::Button(data) = &mut object.control else {
            return;
        };
        if data.pressed == pressed {
            return;
        }
        data.pressed = pressed;
        if data.mode != ButtonMode::Common {
            return;
        }
        let disabled = object.grayed
            && data
                .component
                .controller("button")
                .is_some_and(|c| c.has_page("disabled"));
        let state = match (pressed, disabled) {
            (true, true) => ButtonState::SelectedDisabled,
            (true, false) => ButtonState::Down,
            (false, true) => ButtonState::Disabled,
            (false, false) if data.hovered => ButtonState::Over,
            _ => ButtonState::Up,
        };
        self.apply_button_visual_state(id, state);
    }

    /// Configure a button effect. Scale factors must be finite and positive.
    /// Exported effect=2 buttons receive the same center pivot during construction.
    pub fn set_button_down_effect(&mut self, id: GObjectId, effect: i32, value: f32) -> bool {
        if !(0..=2).contains(&effect) || !value.is_finite() || (effect == 2 && value <= 0.0) {
            return false;
        }
        let Some(ControlData::Button(data)) = self.objects.get(id).map(|o| &o.control) else {
            return false;
        };
        let state = data.interaction_state;
        self.apply_button_visual_state(id, ButtonState::Up);
        let object = self.objects.get_mut(id).unwrap();
        let ControlData::Button(data) = &mut object.control else {
            unreachable!()
        };
        data.down_effect = effect;
        data.down_effect_value = value;
        if effect == 2 {
            let anchor = object.pivot_as_anchor;
            self.set_pivot(id, 0.5, 0.5, anchor);
        }
        self.apply_button_visual_state(id, state);
        true
    }

    pub fn set_button_linked_popup(&mut self, id: GObjectId, popup: Option<GObjectId>) -> bool {
        if popup.is_some_and(|popup| popup == id || !self.contains(popup)) {
            return false;
        }
        let Some(ControlData::Button(data)) = self.objects.get_mut(id).map(|o| &mut o.control)
        else {
            return false;
        };
        data.linked_popup = popup;
        true
    }

    pub(super) fn sync_related_buttons(&mut self, parent: GObjectId, name: &str) {
        let Some(object) = self.objects.get(parent) else {
            return;
        };
        let Some(component) = object.control.component() else {
            return;
        };
        let Some(index) = component.controllers.iter().position(|c| c.name == name) else {
            return;
        };
        let page = component.controllers[index].selected_page_id();
        let updates: Vec<_> = object
            .children
            .iter()
            .filter_map(|id| {
                let ControlData::Button(data) = &self.objects.get(*id)?.control else {
                    return None;
                };
                (data.related_controller == index as i32)
                    .then_some((*id, data.related_page_id.as_deref() == page))
            })
            .collect();
        for (id, selected) in updates {
            self.change_button_selection(id, selected, false);
        }
    }

    fn adjust_button_depth(&mut self, id: GObjectId) {
        let Some(object) = self.objects.get(id) else {
            return;
        };
        let ControlData::Button(data) = &object.control else {
            return;
        };
        let related = data.related_controller;
        let Some(parent) = object.parent else { return };
        let Some(parent_object) = self.objects.get(parent) else {
            return;
        };
        if !parent_object
            .control
            .component()
            .and_then(|c| c.controllers.get(related as usize))
            .is_some_and(|c| c.auto_radio_group_depth)
        {
            return;
        }
        let Some(from) = parent_object.children.iter().position(|child| *child == id) else {
            return;
        };
        let last = parent_object.children.iter().rposition(|child| {
            *child != id
                && matches!(self.objects.get(*child).map(|o| &o.control),
                Some(ControlData::Button(button)) if button.related_controller == related)
        });
        if let Some(last) = last.filter(|last| *last > from) {
            self.objects
                .get_mut(parent)
                .unwrap()
                .children
                .swap(from, last);
            self.touch(parent, dirty::CHILDREN);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::ObjectType;
    use crate::widget::Controller;

    fn fixture(mode: ButtonMode) -> (ObjectStore, GObjectId, GObjectId) {
        let mut store = ObjectStore::new();
        let parent = store.create(ObjectType::Component);
        store.objects[parent]
            .control
            .component_mut()
            .unwrap()
            .controllers
            .push(Controller {
                name: "pages".into(),
                page_ids: vec!["first-id".into(), "second-id".into()],
                page_names: vec!["First".into(), "Second".into()],
                selected_index: 0,
                ..Default::default()
            });
        let button = store.create(ObjectType::Button);
        store.set_parent(button, Some(parent));
        let ControlData::Button(data) = &mut store.objects[button].control else {
            unreachable!()
        };
        data.mode = mode;
        data.related_controller = 0;
        data.related_page_id = Some("second-id".into());
        data.component.controllers.push(Controller {
            name: "button".into(),
            page_ids: ["a", "b", "c", "d", "e", "f"].map(str::to_owned).to_vec(),
            page_names: [
                "up",
                "down",
                "over",
                "selectedOver",
                "disabled",
                "selectedDisabled",
            ]
            .map(str::to_owned)
            .to_vec(),
            selected_index: 0,
            ..Default::default()
        });
        (store, parent, button)
    }

    fn page<'a>(store: &'a ObjectStore, id: GObjectId, name: &str) -> &'a str {
        store.objects[id]
            .control
            .component()
            .unwrap()
            .controller(name)
            .unwrap()
            .selected_page_name()
            .unwrap()
    }

    #[test]
    fn check_click_toggles_page_and_external_page_changes_update_selection() {
        let (mut store, parent, button) = fixture(ButtonMode::Check);
        assert!(store.click_button(button));
        assert_eq!(page(&store, parent, "pages"), "Second");
        assert_eq!(store.button_selected(button), Some(true));
        assert!(store.click_button(button));
        assert_eq!(page(&store, parent, "pages"), "First");
        assert_eq!(store.button_selected(button), Some(false));
        store.set_controller_page_id(parent, "pages", "second-id");
        assert_eq!(store.button_selected(button), Some(true));
        assert_eq!(page(&store, button, "button"), "down");
        store.set_controller_page_id(parent, "pages", "first-id");
        assert_eq!(page(&store, button, "button"), "up");
    }

    #[test]
    fn common_fire_click_changes_related_controller_immediately_without_selection_or_pointer_state()
    {
        let (mut store, parent, button) = fixture(ButtonMode::Common);
        assert_eq!(page(&store, parent, "pages"), "First");
        assert_eq!(store.fire_button_click(button, true), Some(false));
        assert_eq!(page(&store, parent, "pages"), "Second");
        assert_eq!(store.button_selected(button), Some(false));
        assert_eq!(page(&store, button, "button"), "over");
        store.advance_button_feedback(0.2);
        assert_eq!(page(&store, button, "button"), "up");
        assert_eq!(page(&store, parent, "pages"), "Second");
    }

    #[test]
    fn common_button_click_changes_page_without_becoming_selected() {
        let (mut store, parent, button) = fixture(ButtonMode::Common);
        assert!(!store.set_button_selected(button, true));
        assert!(!store.click_button(button));
        assert_eq!(page(&store, parent, "pages"), "Second");
        assert_eq!(store.button_selected(button), Some(false));
    }

    #[test]
    fn radio_selection_updates_siblings_and_swaps_to_top_of_group() {
        let (mut store, parent, first) = fixture(ButtonMode::Radio);
        store.objects[parent]
            .control
            .component_mut()
            .unwrap()
            .controllers[0]
            .auto_radio_group_depth = true;
        let second = store.create(ObjectType::Button);
        let data = store.objects[first].control.clone();
        store.objects[second].control = data;
        if let ControlData::Button(data) = &mut store.objects[second].control {
            data.related_page_id = Some("first-id".into());
        }
        store.set_parent(second, Some(parent));
        store.apply_all_controllers(parent);
        assert_eq!(store.button_selected(second), Some(true));
        assert!(store.click_button(first));
        assert_eq!(store.button_selected(second), Some(false));
        assert_eq!(store.objects[parent].children.last(), Some(&first));
        assert!(!store.click_button(first));
        assert_eq!(page(&store, parent, "pages"), "Second");
    }

    #[test]
    fn selected_visuals_survive_pointer_and_disabled_states() {
        let (mut store, _, button) = fixture(ButtonMode::Check);
        store.set_button_state(button, ButtonState::Over);
        store.set_button_selected(button, true);
        assert_eq!(page(&store, button, "button"), "selectedOver");
        store.set_button_state(button, ButtonState::Up);
        assert_eq!(page(&store, button, "button"), "down");
        store.set_grayed(button, true);
        assert_eq!(page(&store, button, "button"), "selectedDisabled");
        store.set_grayed(button, false);
        assert_eq!(page(&store, button, "button"), "down");
    }

    #[test]
    fn initially_selected_button_needs_no_related_controller_for_its_visuals() {
        let (mut store, _, button) = fixture(ButtonMode::Check);
        if let ControlData::Button(data) = &mut store.objects[button].control {
            data.related_controller = -1;
            data.selected = true;
        }
        store.initialize_button(button);
        assert_eq!(page(&store, button, "button"), "down");
        assert!(!store.has_gear_tweens());
    }

    #[test]
    fn selected_title_and_icon_restore_empty_base_values() {
        let (mut store, _, button) = fixture(ButtonMode::Check);
        let title = store.create(ObjectType::Text);
        store.set_name(title, "title");
        store.set_parent(title, Some(button));
        let icon = store.create(ObjectType::Loader);
        store.set_name(icon, "icon");
        store.set_parent(icon, Some(button));
        if let ControlData::Button(data) = &mut store.objects[button].control {
            data.selected_title = Some("Selected".into());
            data.selected_icon = Some("ui://selected".into());
        }
        store.set_button_selected(button, true);
        let ControlData::Text(text) = &store.objects[title].control else {
            unreachable!()
        };
        assert_eq!(text.text, "Selected");
        let ControlData::Loader(loader) = &store.objects[icon].control else {
            unreachable!()
        };
        assert_eq!(loader.url.as_deref(), Some("ui://selected"));
        store.set_button_selected(button, false);
        let ControlData::Text(text) = &store.objects[title].control else {
            unreachable!()
        };
        assert_eq!(text.text, "");
        let ControlData::Loader(loader) = &store.objects[icon].control else {
            unreachable!()
        };
        assert_eq!(loader.url.as_deref(), Some(""));
    }

    #[test]
    fn change_state_on_click_does_not_block_sound_or_programmatic_selection() {
        let (mut store, _, button) = fixture(ButtonMode::Check);
        if let ControlData::Button(data) = &mut store.objects[button].control {
            data.sound = Some("click.wav".into());
        }
        store.set_button_change_state_on_click(button, false);
        assert!(!store.click_button(button));
        assert_eq!(
            store.take_queued_sounds(),
            vec![(button, "click.wav".into(), 1.0)]
        );
        assert!(store.set_button_selected(button, true));
    }

    #[test]
    fn official_button_keeps_resource_sound_when_instance_has_no_override() {
        let package = crate::package::FuiPackage::parse(
            "Basics",
            include_bytes!("../../tests/fixtures/Basics.fui"),
        )
        .unwrap();
        let mut store = ObjectStore::new();
        let root = crate::widget::create_from_package(
            &mut store,
            &package,
            package.item_index_by_name("Demo_Controller").unwrap(),
            &crate::package::LoadContext::none(),
        )
        .unwrap();
        let button = store.child_by_name(root, "switchBtn").unwrap();
        assert!(store.click_button(button));
        let sounds = store.take_queued_sounds();
        assert_eq!(sounds.len(), 1);
        assert_eq!(sounds[0].0, button);
        assert!(sounds[0].1.ends_with("gojg7u"));
    }
}

#[cfg(test)]
#[path = "button_effect_tests.rs"]
mod effect_tests;

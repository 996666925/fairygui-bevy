use super::*;

#[test]
fn unmodified_native_fire_click_traces_match_immediate_actions_and_independent_feedback_timers() {
    let mut store = ObjectStore::new();
    let mut button = None;
    let mut image = None;
    let mut changes = 0;
    for line in include_str!("../../tests/fixtures/button-fire-reference.txt").lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(case) = line.strip_prefix("case ") {
            let fields: Vec<_> = case.split_whitespace().collect();
            store = ObjectStore::new();
            changes = 0;
            let id = store.create(ObjectType::Button);
            let child = store.create(ObjectType::Image);
            store.set_parent(child, Some(id));
            store.set_scale(id, 2.0, 3.0);
            store.set_control_color(child, Color::from_argb(0xffabcdef), None);
            let ControlData::Button(data) = &mut store.objects[id].control else {
                unreachable!()
            };
            data.mode = ButtonMode::from_code(fields[1].parse().unwrap());
            data.down_effect = fields[2].parse().unwrap();
            data.down_effect_value = 0.8;
            data.sound = Some("click.wav".into());
            if fields[3] == "1" {
                let pages = [
                    "up",
                    "down",
                    "over",
                    "selectedOver",
                    "disabled",
                    "selectedDisabled",
                ]
                .map(str::to_owned)
                .to_vec();
                data.component.controllers.push(Controller {
                    name: "button".into(),
                    page_ids: pages.clone(),
                    page_names: pages,
                    selected_index: 0,
                    ..Default::default()
                });
            }
            button = Some(id);
            image = Some(child);
            continue;
        }
        let (command, expected) = line.split_once(" | ").unwrap();
        let (command, value) = command.split_once(' ').unwrap();
        let id = button.unwrap();
        match command {
            "fire" => {
                if store.fire_button_click(id, value != "0") == Some(true) {
                    changes += 1;
                }
            }
            "wait" => store.advance_button_feedback(value.parse::<f64>().unwrap() / 1000.0),
            "hover" => store.set_button_hovered(id, value == "1"),
            "press" => store.set_button_pressed(id, value == "1"),
            "grayed" => store.set_grayed(id, value == "1"),
            "change" => store.set_button_change_state_on_click(id, value == "1"),
            _ => panic!("{line}"),
        }
        let expected: Vec<_> = expected.split_whitespace().collect();
        let object = store.object(id).unwrap();
        let ControlData::Button(data) = object.control() else {
            unreachable!()
        };
        for (value, column) in [(object.scale_x(), 0), (object.scale_y(), 1)] {
            assert!(
                (value - expected[column].parse::<f32>().unwrap()).abs() < 0.0001,
                "{line}"
            );
        }
        assert_eq!(data.down_scaled, expected[2] == "1", "{line}");
        assert_eq!(data.hovered, expected[3] == "1", "{line}");
        assert_eq!(data.pressed, expected[4] == "1", "{line}");
        assert_eq!(
            data.component
                .controller("button")
                .and_then(|c| c.selected_page_name())
                .unwrap_or("-"),
            expected[5],
            "{line}"
        );
        assert_eq!(data.selected, expected[6] == "1", "{line}");
        assert_eq!(
            store.queued_sound_count(),
            expected[7].parse::<usize>().unwrap(),
            "{line}"
        );
        assert_eq!(changes, expected[8].parse::<usize>().unwrap(), "{line}");
        assert_eq!(expected[9], "0", "native fireClick must not dispatch Click");
        assert_eq!(
            store.button_feedback.len(),
            expected[10].parse::<usize>().unwrap(),
            "{line}"
        );
        let ControlData::Image(data) = store.object(image.unwrap()).unwrap().control() else {
            unreachable!()
        };
        assert_eq!(
            format!("#{:06x}", data.color.unwrap().to_argb() & 0xffffff),
            expected[11].to_ascii_lowercase(),
            "{line}"
        );
    }
}

#[test]
fn fire_feedback_continues_offstage_but_disposal_cancels_both_timers_and_reused_slots_stay_unchanged()
 {
    let mut store = ObjectStore::new();
    let parent = store.create(ObjectType::Component);
    let button = store.create(ObjectType::Button);
    store.set_parent(button, Some(parent));
    store.attach_to_stage(parent);
    store.set_button_down_effect(button, 2, 0.8);
    assert_eq!(store.fire_button_click(button, true), Some(false));
    store.detach_from_stage(parent);
    store.set_visible(button, false);
    store.advance_button_feedback(0.1);
    assert!((store.object(button).unwrap().scale_x() - 0.8).abs() < 0.0001);
    store.remove(parent);
    assert!(store.button_feedback.is_empty());
    let replacement = store.create(ObjectType::Button);
    store.set_scale(replacement, 3.0, 4.0);
    store.advance_button_feedback(1.0);
    assert_eq!(store.object(replacement).unwrap().scale_x(), 3.0);
    assert_eq!(store.fire_button_click(button, true), None);
    let component = store.create(ObjectType::Component);
    assert_eq!(store.fire_button_click(component, true), None);
}

#[test]
fn invalid_feedback_deltas_do_not_poison_the_timer_clock_or_run_callbacks() {
    let mut store = ObjectStore::new();
    let button = store.create(ObjectType::Button);
    store.set_button_down_effect(button, 2, 0.8);
    store.fire_button_click(button, true);
    for delta in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0] {
        store.advance_button_feedback(delta);
    }
    assert_eq!(store.object(button).unwrap().scale_x(), 1.0);
    store.advance_button_feedback(0.1);
    assert!((store.object(button).unwrap().scale_x() - 0.8).abs() < 0.0001);
    store.advance_button_feedback(0.1);
    assert!((store.object(button).unwrap().scale_x() - 1.0).abs() < 0.0001);
}

#[test]
fn enabling_a_hovered_selected_button_uses_down_and_missing_selected_disabled_page_uses_disabled() {
    let mut store = ObjectStore::new();
    let button = store.create(ObjectType::Button);
    let ControlData::Button(data) = &mut store.objects[button].control else {
        unreachable!()
    };
    data.mode = ButtonMode::Check;
    let pages = ["up", "down", "over", "selectedOver", "disabled"]
        .map(str::to_owned)
        .to_vec();
    data.component.controllers.push(Controller {
        name: "button".into(),
        page_ids: pages.clone(),
        page_names: pages,
        selected_index: 0,
        ..Default::default()
    });
    store.set_button_hovered(button, true);
    store.set_button_selected(button, true);
    store.set_grayed(button, true);
    let state = |store: &ObjectStore| {
        let ControlData::Button(data) = store.object(button).unwrap().control() else {
            unreachable!()
        };
        data.interaction_state
    };
    assert_eq!(state(&store), ButtonState::Disabled);
    store.set_grayed(button, false);
    assert_eq!(
        state(&store),
        ButtonState::Down,
        "handleGrayedChanged deliberately ignores rollover"
    );
}

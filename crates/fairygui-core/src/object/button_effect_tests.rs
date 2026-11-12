use super::*;
use crate::color::Color;
use crate::package::ObjectType;
use crate::widget::Controller;

#[path = "button_fire_tests.rs"]
mod fire;

#[test]
fn executable_official_button_traces_match_effects_pointer_selection_and_disable_states() {
    let mut store = ObjectStore::new();
    let (mut button, mut image, mut text) = (None, None, None);
    for line in include_str!("../../tests/fixtures/button-reference.txt").lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some(case) = line.strip_prefix("case ") {
            store = ObjectStore::new();
            let values: Vec<_> = case.split_whitespace().collect();
            let id = store.create(ObjectType::Button);
            store.set_scale(id, 2.0, 3.0);
            let child = store.create(ObjectType::Image);
            let title = store.create(ObjectType::Text);
            store.set_parent(child, Some(id));
            store.set_parent(title, Some(id));
            store.set_control_color(child, Color::from_argb(0xffabcdef), None);
            store.set_control_color(title, Color::from_argb(0xff123456), None);
            let ControlData::Button(data) = &mut store.objects[id].control else {
                unreachable!()
            };
            data.mode = ButtonMode::from_code(values[1].parse().unwrap());
            data.down_effect = values[2].parse().unwrap();
            data.down_effect_value = values[3].parse().unwrap();
            if values[4] == "1" {
                let names = [
                    "up",
                    "down",
                    "over",
                    "selectedOver",
                    "disabled",
                    "selectedDisabled",
                ];
                data.component.controllers.push(Controller {
                    name: "button".into(),
                    page_ids: names.map(str::to_owned).to_vec(),
                    page_names: names.map(str::to_owned).to_vec(),
                    selected_index: 0,
                    ..Default::default()
                });
            }
            (button, image, text) = (Some(id), Some(child), Some(title));
            continue;
        }
        let (command, expected) = line.split_once(" | ").unwrap();
        let command: Vec<_> = command.split_whitespace().collect();
        let id = button.unwrap();
        let enabled = command[1] == "1";
        match command[0] {
            "state" => {
                let state = match command[1] {
                    "up" => ButtonState::Up,
                    "down" => ButtonState::Down,
                    "selectedOver" => ButtonState::SelectedOver,
                    "selectedDisabled" => ButtonState::SelectedDisabled,
                    "disabled" => ButtonState::Disabled,
                    other => panic!("{other}"),
                };
                store.apply_button_visual_state(id, state);
            }
            "hover" => store.set_button_hovered(id, enabled),
            "press" => store.set_button_pressed(id, enabled),
            "selected" => {
                store.set_button_selected(id, enabled);
            }
            "grayed" => store.set_grayed(id, enabled),
            "scale" => {
                store.set_scale(id, command[1].parse().unwrap(), command[2].parse().unwrap())
            }
            other => panic!("{other}"),
        }
        let values: Vec<_> = expected.split_whitespace().collect();
        let object = store.object(id).unwrap();
        let ControlData::Button(data) = object.control() else {
            unreachable!()
        };
        for (actual, expected) in [(object.scale_x(), values[0]), (object.scale_y(), values[1])] {
            let expected: f32 = expected.parse().unwrap();
            assert!((actual - expected).abs() < 0.0001, "{line}: {actual}");
        }
        assert_eq!(data.down_scaled, values[2] == "1", "{line}");
        assert_eq!(data.hovered, values[3] == "1", "{line}");
        assert_eq!(data.pressed, values[4] == "1", "{line}");
        assert_eq!(
            data.component
                .controller("button")
                .and_then(|c| c.selected_page_name())
                .unwrap_or("-"),
            values[5],
            "{line}"
        );
        let ControlData::Image(image) = store.object(image.unwrap()).unwrap().control() else {
            unreachable!()
        };
        assert_eq!(
            format!("#{:06x}", image.color.unwrap().to_argb() & 0xffffff),
            values[6].to_ascii_lowercase(),
            "{line}"
        );
        let color = store
            .object(text.unwrap())
            .unwrap()
            .control()
            .as_text_field()
            .unwrap()
            .color;
        assert_eq!(
            format!("#{:06x}", color.to_argb() & 0xffffff),
            values[7].to_ascii_lowercase(),
            "{line}"
        );
    }
}

#[test]
fn scale_effect_configuration_centers_the_pivot_and_invalid_values_cannot_poison_scale() {
    let mut store = ObjectStore::new();
    let button = store.create(ObjectType::Button);
    store.set_pivot(button, 0.1, 0.2, true);
    store.set_scale(button, -2.0, 3.0);
    assert!(store.set_button_down_effect(button, 2, 0.8));
    assert_eq!(
        store.object(button).unwrap().pivot(),
        crate::geom::Point::new(0.5, 0.5)
    );
    assert!(store.object(button).unwrap().pivot_as_anchor());
    store.set_button_pressed(button, true);
    assert_eq!(store.object(button).unwrap().scale_x(), -1.6);
    for value in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(!store.set_button_down_effect(button, 2, value));
    }
    assert!(store.set_button_down_effect(button, 0, 0.8));
    assert!((store.object(button).unwrap().scale_x() + 2.0).abs() < 0.0001);
    store.set_button_pressed(button, false);
    assert!((store.object(button).unwrap().scale_y() - 3.0).abs() < 0.0001);
}

#[test]
fn tint_effect_writes_supported_direct_children_and_restores_white_without_tinting_direct_text() {
    let mut store = ObjectStore::new();
    let button = store.create(ObjectType::Button);
    let image = store.create(ObjectType::Image);
    let text = store.create(ObjectType::RichText);
    let label = store.create(ObjectType::Label);
    let title = store.create(ObjectType::Text);
    let component = store.create(ObjectType::Component);
    let nested = store.create(ObjectType::Image);
    for child in [image, text, label, component] {
        store.set_parent(child, Some(button));
    }
    store.set_parent(title, Some(label));
    store.set_name(title, "title");
    store.set_parent(nested, Some(component));
    for child in [image, text, label, nested] {
        store.set_control_color(child, Color::from_argb(0xff123456), None);
    }
    store.set_button_down_effect(button, 1, 0.6);
    store.set_button_pressed(button, true);
    let ControlData::Image(data) = store.object(image).unwrap().control() else {
        unreachable!()
    };
    assert_eq!(data.color.unwrap(), Color::from_argb(0xff999999));
    let ControlData::Label(data) = store.object(label).unwrap().control() else {
        unreachable!()
    };
    assert_eq!(data.title_color, Some(Color::from_argb(0xff999999)));
    store.apply_titles();
    assert_eq!(
        store
            .object(title)
            .unwrap()
            .control()
            .as_text_field()
            .unwrap()
            .color,
        Color::from_argb(0xff999999)
    );
    store.set_button_pressed(button, false);
    let ControlData::Image(data) = store.object(image).unwrap().control() else {
        unreachable!()
    };
    assert_eq!(data.color, Some(Color::WHITE));
    assert_eq!(
        store
            .object(text)
            .unwrap()
            .control()
            .as_text_field()
            .unwrap()
            .color,
        Color::from_argb(0xff123456)
    );
    let ControlData::Image(data) = store.object(nested).unwrap().control() else {
        unreachable!()
    };
    assert_eq!(
        data.color,
        Some(Color::from_argb(0xff123456)),
        "plain components have no Color property"
    );
}

#[test]
fn exported_scale_button_constructs_with_center_pivot_without_a_parent_component() {
    use crate::package::{FuiPackage, LoadContext};
    let package =
        FuiPackage::parse("Basics", include_bytes!("../../tests/fixtures/Basics.fui")).unwrap();
    let mut store = ObjectStore::new();
    let button = crate::widget::create_from_package(
        &mut store,
        &package,
        package.item_index_by_id("rpmb13").unwrap(),
        &LoadContext::none(),
    )
    .unwrap();
    assert_eq!(store.object(button).unwrap().parent(), None);
    assert_eq!(
        store.object(button).unwrap().pivot(),
        crate::geom::Point::new(0.5, 0.5)
    );
    store.set_button_pressed(button, true);
    assert!((store.object(button).unwrap().scale_x() - 0.8).abs() < 0.0001);
    store.set_button_pressed(button, false);
    assert!((store.object(button).unwrap().scale_x() - 1.0).abs() < 0.0001);
}

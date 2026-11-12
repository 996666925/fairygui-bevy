use super::*;
use crate::{
    package::ObjectType,
    widget::{ButtonMode, ButtonState, Controller, Gear, GearKind, GearValue},
};
use std::collections::HashMap;

fn controller(name: &str, pages: &[&str], selected_index: i32) -> Controller {
    Controller {
        name: name.into(),
        page_ids: (0..pages.len()).map(|i| i.to_string()).collect(),
        page_names: pages.iter().map(|s| (*s).into()).collect(),
        selected_index,
        ..Default::default()
    }
}

#[test]
fn executable_native_component_button_controller_and_first_assignment_states_match() {
    let mut store = ObjectStore::new();
    let mut nodes = HashMap::new();
    let mut cases = 0;
    let mut steps = 0;
    let mut states = 0;
    let mut context = String::new();
    for line in include_str!("../../tests/fixtures/grayed-reference.txt").lines() {
        if line.starts_with('#') {
            continue;
        }
        let p: Vec<_> = line.split_whitespace().collect();
        match p[0] {
            "case" => {
                store = ObjectStore::new();
                nodes.clear();
                context = line.into();
                cases += 1;
            }
            "node" => {
                let kind = match p[2] {
                    "component" => ObjectType::Component,
                    "button" => ObjectType::Button,
                    "image" => ObjectType::Image,
                    "text" => ObjectType::Text,
                    "loader" => ObjectType::Loader,
                    "label" => ObjectType::Label,
                    "list" => ObjectType::List,
                    "tree" => ObjectType::Tree,
                    "slider" => ObjectType::Slider,
                    "progress" => ObjectType::ProgressBar,
                    "combo" => ObjectType::ComboBox,
                    "scroll" => ObjectType::ScrollBar,
                    _ => panic!("{line}"),
                };
                let id = store.create(kind);
                nodes.insert(p[1].to_string(), id);
                if p[3] != "-" {
                    store.set_parent(id, Some(nodes[p[3]]));
                }
                let control = &mut store.object_mut(id).unwrap().control;
                if p[4] == "1" {
                    control
                        .component_mut()
                        .unwrap()
                        .controllers
                        .push(controller("grayed", &["normal", "gray"], 0));
                }
                if let ControlData::Button(data) = control {
                    let selected = p[6] == "1";
                    data.mode = ButtonMode::Check;
                    data.selected = selected;
                    data.interaction_state = if selected {
                        ButtonState::Down
                    } else {
                        ButtonState::Up
                    };
                    let flags = p[5].parse::<u8>().unwrap();
                    if flags != 0 {
                        let mut pages = vec!["up", "down", "over", "selectedOver"];
                        if flags <= 2 {
                            pages.push("disabled");
                        }
                        if flags == 2 {
                            pages.push("selectedDisabled");
                        }
                        data.component.controllers.push(controller(
                            "button",
                            &pages,
                            i32::from(selected),
                        ));
                    }
                }
            }
            "gear" => {
                let (owner, target) = (nodes[p[1]], nodes[p[2]]);
                let mut gear = Gear::new(GearKind::Look);
                gear.controller_index = 0;
                for (page, grayed) in [("0", false), ("1", true)] {
                    gear.values.insert(
                        Some(page.into()),
                        GearValue::Look {
                            alpha: 1.0,
                            rotation: 0.0,
                            grayed,
                            touchable: true,
                        },
                    );
                }
                store.object_mut(target).unwrap().gears.push(gear);
                assert_eq!(store.object(target).unwrap().parent(), Some(owner));
            }
            "step" => {
                context = format!("{context} / {}", p[1]);
                steps += 1;
                let id = nodes[p[3]];
                match p[2] {
                    "gray" => store.set_grayed(id, p[4] == "1"),
                    "attach" => {
                        store.set_parent(id, Some(nodes[p[4]]));
                    }
                    "visual" => {
                        store.apply_button_visual_state(
                            id,
                            if p[4] == "selectedOver" {
                                ButtonState::SelectedOver
                            } else {
                                ButtonState::Over
                            },
                        );
                    }
                    _ => panic!("{line}"),
                }
            }
            "state" => {
                let object = store.object(nodes[p[1]]).unwrap();
                let gray = if !object.grayed_initialized {
                    "u"
                } else if object.grayed() {
                    "1"
                } else {
                    "0"
                };
                let component = object.control().component();
                let index = component
                    .and_then(|c| c.controller("grayed"))
                    .map_or(-1, |c| c.selected_index);
                let page = component
                    .and_then(|c| c.controller("button"))
                    .and_then(|c| c.selected_page_name())
                    .unwrap_or("-");
                let local = if component.is_some() { "u" } else { gray };
                assert_eq!(
                    (gray, index.to_string(), page, local),
                    (p[2], p[3].into(), p[4], p[5]),
                    "{context}: {}",
                    p[1]
                );
                states += 1;
            }
            _ => panic!("{line}"),
        }
    }
    assert_eq!((cases, steps, states), (27, 76, 273));
}

#[test]
fn grayed_controller_drives_look_gears_without_inheriting_gray_into_other_children() {
    let mut store = ObjectStore::new();
    let root = store.create(ObjectType::Component);
    store
        .object_mut(root)
        .unwrap()
        .control
        .component_mut()
        .unwrap()
        .controllers
        .push(controller("grayed", &["normal", "gray"], 0));
    let controlled = store.create(ObjectType::Component);
    store.set_parent(controlled, Some(root));
    let image = store.create(ObjectType::Image);
    store.set_parent(image, Some(controlled));
    let untouched = store.create(ObjectType::Graph);
    store.set_parent(untouched, Some(root));
    let mut gear = Gear::new(GearKind::Look);
    gear.controller_index = 0;
    for (page, grayed) in [("0", false), ("1", true)] {
        gear.values.insert(
            Some(page.into()),
            GearValue::Look {
                alpha: 1.0,
                rotation: 0.0,
                grayed,
                touchable: true,
            },
        );
    }
    store.object_mut(controlled).unwrap().gears.push(gear);
    store.set_grayed(root, true);
    assert!(store.object(image).unwrap().grayed());
    assert!(!store.object(untouched).unwrap().grayed());
    assert!(
        store.object(root).unwrap().touchable(),
        "grayed does not mean touchable=false"
    );
    store.set_grayed(root, false);
    assert!(!store.object(image).unwrap().grayed());
    assert_eq!(
        store
            .object(root)
            .unwrap()
            .control()
            .component()
            .unwrap()
            .controller("grayed")
            .unwrap()
            .selected_index,
        0
    );
}

#[test]
fn serialized_instance_gray_selects_controllers_without_starting_construction_tweens() {
    let mut store = ObjectStore::new();
    let root = store.create(ObjectType::Component);
    store
        .object_mut(root)
        .unwrap()
        .control
        .component_mut()
        .unwrap()
        .controllers
        .push(controller("grayed", &["normal", "gray"], 0));
    let child = store.create(ObjectType::Image);
    store.set_parent(child, Some(root));
    let mut gear = Gear::new(GearKind::Xy);
    gear.controller_index = 0;
    gear.tween = Some(Default::default());
    for (page, x) in [("0", 0.0), ("1", 100.0)] {
        gear.values.insert(
            Some(page.into()),
            GearValue::Xy {
                x,
                y: 0.0,
                px: 0.0,
                py: 0.0,
            },
        );
    }
    store.object_mut(child).unwrap().gears.push(gear);
    let object = store.object_mut(root).unwrap();
    object.grayed = true;
    object.grayed_initialized = true;
    store.initialize_grayed(root);
    assert_eq!(store.object(child).unwrap().x(), 100.0);
    assert!(!store.has_gear_tweens());
    store.set_grayed(root, false);
    assert!(
        store.has_gear_tweens(),
        "normal runtime controller changes may tween"
    );
}

#[test]
fn exported_instance_flags_reach_built_components_and_both_button_controller_paths() {
    use crate::{
        package::{FuiPackage, LoadContext},
        widget::create_from_package,
    };
    let package =
        FuiPackage::parse("Basics", include_bytes!("../../tests/fixtures/Grayed.fui")).unwrap();
    let mut store = ObjectStore::new();
    let root = create_from_package(
        &mut store,
        &package,
        package.item_index_by_name("Demo_Component").unwrap(),
        &LoadContext::none(),
    )
    .unwrap();
    let nested = store.child_by_name(root, "n17").unwrap();
    let image = store.child_by_name(nested, "n1").unwrap();
    let text = store.child_by_name(nested, "n2").unwrap();
    assert!(store.object(nested).unwrap().grayed());
    assert!(store.object(image).unwrap().grayed());
    assert!(store.object(text).unwrap().grayed());
    store.set_grayed(nested, false);
    assert!(!store.object(image).unwrap().grayed() && !store.object(text).unwrap().grayed());
    let buttons = create_from_package(
        &mut store,
        &package,
        package.item_index_by_name("Demo_Button").unwrap(),
        &LoadContext::none(),
    )
    .unwrap();
    let fallback = store.child_by_name(buttons, "n3").unwrap();
    let title = store.child_by_name(fallback, "title").unwrap();
    assert!(
        store.object(title).unwrap().grayed(),
        "button without disabled page uses component handler"
    );
    let controlled = store.child_by_name(buttons, "n35").unwrap();
    let title = store.child_by_name(controlled, "title").unwrap();
    assert!(store.object(controlled).unwrap().grayed());
    assert_eq!(
        store
            .object(controlled)
            .unwrap()
            .control()
            .component()
            .unwrap()
            .controller("grayed")
            .unwrap()
            .selected_index,
        1
    );
    assert!(
        !store.object(title).unwrap().grayed(),
        "grayed controller owns the presentation"
    );
    let disabled = store.child_by_name(buttons, "n38").unwrap();
    let skin = store.child_by_name(disabled, "n4").unwrap();
    assert!(store.object(disabled).unwrap().grayed());
    assert_eq!(
        store
            .object(disabled)
            .unwrap()
            .control()
            .component()
            .unwrap()
            .controller("button")
            .unwrap()
            .selected_page_name(),
        Some("selectedDisabled")
    );
    assert!(
        !store.object(skin).unwrap().grayed(),
        "disabled skin does not get an additional gray filter"
    );
    assert_eq!(store.button_selected(disabled), Some(true));
    store.set_button_selected(disabled, false);
    assert_eq!(
        store
            .object(disabled)
            .unwrap()
            .control()
            .component()
            .unwrap()
            .controller("button")
            .unwrap()
            .selected_page_name(),
        Some("disabled")
    );
    assert!(!store.has_gear_tweens());
}

use super::*;
use crate::{
    package::ObjectType,
    widget::{Controller, Gear, GearTweenConfig},
};

fn fixture(kind: ObjectType, gear_kind: GearKind) -> (ObjectStore, GObjectId, GObjectId) {
    let mut store = ObjectStore::new();
    let root = store.create(ObjectType::Component);
    store.set_size(root, 400.0, 200.0);
    store
        .object_mut(root)
        .unwrap()
        .control
        .component_mut()
        .unwrap()
        .controllers
        .push(Controller {
            name: "state".into(),
            page_ids: vec!["a".into(), "b".into(), "c".into()],
            page_names: vec!["A".into(), "B".into(), "C".into()],
            selected_index: 0,
            ..Default::default()
        });
    let child = store.create(kind);
    store.set_parent(child, Some(root));
    store.set_size(child, 50.0, 30.0);
    let mut gear = Gear::new(gear_kind);
    gear.controller_index = 0;
    gear.controller_owner = Some(root);
    let value = store.gear_snapshot(child, gear_kind).unwrap();
    gear.values.insert(None, value.clone());
    gear.values.insert(Some("a".into()), value.clone());
    gear.values.insert(Some("b".into()), value);
    store.object_mut(child).unwrap().gears.push(gear);
    (store, root, child)
}
fn saved(store: &ObjectStore, id: GObjectId, page: Option<&str>) -> GearValue {
    store.object(id).unwrap().gears[0].values[&page.map(str::to_owned)].clone()
}

#[test]
fn forwarded_font_and_ubb_edits_dirty_the_resolved_title_without_capturing_gears() {
    for wrapper in [ObjectType::Button, ObjectType::Label, ObjectType::ComboBox] {
        for kind in [ObjectType::Text, ObjectType::InputText] {
            let (mut store, _, text) = fixture(kind, GearKind::FontSize);
            let saved_values = store.object(text).unwrap().gears[0].values.clone();
            let owner = store.create(wrapper);
            store.set_name(text, "title");
            store.set_parent(text, Some(owner));
            store.take_dirty();
            assert!(store.set_object_font(owner, Some("ui://bmfont01font")));
            assert!(store.set_object_ubb_enabled(owner, true));
            assert!(store.set_object_html_enabled(owner, true));
            assert!(store.set_object_underline(owner, true));
            assert_eq!(store.take_dirty(), vec![(text, dirty::TEXT)]);
            let field = store
                .object(text)
                .unwrap()
                .control()
                .as_text_field()
                .unwrap();
            assert_eq!(field.font.as_deref(), Some("ui://bmfont01font"));
            assert!(field.ubb_enabled);
            assert!(field.html_enabled);
            assert!(field.underline);
            assert_eq!(store.object(text).unwrap().gears[0].values, saved_values);
            assert!(!store.set_object_font(owner, Some("ui://bmfont01font")));
            assert!(!store.set_object_ubb_enabled(owner, true));
            assert!(!store.set_object_html_enabled(owner, true));
            assert!(!store.set_object_underline(owner, true));
            assert!(store.take_dirty().is_empty());
            assert!(store.set_object_font(owner, None));
        }
    }
}

#[test]
fn runtime_numeric_edits_survive_page_roundtrips_without_changing_default_or_other_pages() {
    for kind in [GearKind::Xy, GearKind::Size, GearKind::Look] {
        let (mut store, root, child) = fixture(ObjectType::Image, kind);
        let default = saved(&store, child, None);
        let other = saved(&store, child, Some("b"));
        match kind {
            GearKind::Xy => store.set_xy(child, 80.0, 60.0),
            GearKind::Size => {
                store.set_size(child, 70.0, 45.0);
                store.set_scale(child, 1.5, 0.7);
            }
            GearKind::Look => {
                store.set_alpha(child, 0.3);
                store.set_rotation(child, 32.0);
                store.set_grayed(child, true);
                store.set_touchable(child, false);
            }
            _ => unreachable!(),
        }
        let edited = store.gear_snapshot(child, kind).unwrap();
        assert_eq!(saved(&store, child, Some("a")), edited);
        store.set_controller_page(root, "state", "B");
        assert_eq!(saved(&store, child, Some("b")), other);
        store.set_controller_page(root, "state", "A");
        assert_eq!(store.gear_snapshot(child, kind).unwrap(), edited);
        assert_eq!(saved(&store, child, None), default);
    }
}

#[test]
fn unselected_page_storage_is_distinct_from_the_default_fallback() {
    let (mut store, root, child) = fixture(ObjectType::Image, GearKind::Size);
    let default = saved(&store, child, None);
    store.set_controller_page_index(root, "state", -1);
    store.set_size(child, 123.0, 87.0);
    assert_eq!(saved(&store, child, None), default);
    assert_eq!(
        store.object(child).unwrap().gears[0].unselected_value,
        store.gear_snapshot(child, GearKind::Size)
    );
    store.set_controller_page(root, "state", "C");
    assert_eq!(
        store.size_of(child).unwrap(),
        crate::geom::Size::new(50.0, 30.0)
    );
    store.set_controller_page_index(root, "state", -1);
    assert_eq!(
        store.size_of(child).unwrap(),
        crate::geom::Size::new(123.0, 87.0)
    );
}

#[test]
fn gear_tween_ticks_and_replacement_do_not_capture_intermediate_page_values() {
    let (mut store, root, child) = fixture(ObjectType::Image, GearKind::Xy);
    let b = GearValue::Xy {
        x: 100.0,
        y: 40.0,
        px: 0.25,
        py: 0.2,
    };
    store.object_mut(child).unwrap().gears[0]
        .values
        .insert(Some("b".into()), b.clone());
    store.object_mut(child).unwrap().gears[0].tween = Some(GearTweenConfig {
        ease_type: 0,
        duration: 1.0,
        delay: 0.2,
    });
    store.set_controller_page(root, "state", "B");
    store.advance_gear_tweens(0.7);
    assert!((store.object(child).unwrap().x() - 50.0).abs() < 0.001);
    assert_eq!(saved(&store, child, Some("b")), b);
    store.set_controller_page(root, "state", "A");
    store.advance_gear_tweens(2.0);
    assert_eq!(store.object(child).unwrap().x(), 0.0);
    assert_eq!(saved(&store, child, Some("b")), b);
    assert!(!store.object(child).unwrap().gear_locked);
}

#[test]
fn binding_stays_with_original_controller_after_reparenting() {
    let (mut store, old, child) = fixture(ObjectType::Image, GearKind::Xy);
    let other = store.create(ObjectType::Component);
    store.set_size(other, 200.0, 100.0);
    let data = store
        .object(old)
        .unwrap()
        .control()
        .component()
        .unwrap()
        .clone();
    store.object_mut(other).unwrap().control = ControlData::Component(data);
    store.set_parent(child, Some(other));
    store.set_xy(child, 50.0, 30.0);
    assert_eq!(
        saved(&store, child, Some("a")),
        GearValue::Xy {
            x: 50.0,
            y: 30.0,
            px: 0.25,
            py: 0.3
        }
    );
    store.set_controller_page(other, "state", "B");
    assert_eq!(store.object(child).unwrap().x(), 50.0);
    store.set_controller_page(old, "state", "B");
    store.set_xy(child, 80.0, 40.0);
    assert_eq!(
        saved(&store, child, Some("b")),
        GearValue::Xy {
            x: 80.0,
            y: 40.0,
            px: 0.4,
            py: 0.4
        }
    );
}

#[test]
fn locks_are_local_and_grayed_forwarding_updates_descendant_look_storage() {
    let (mut store, root, component) = fixture(ObjectType::Component, GearKind::Look);
    store
        .object_mut(component)
        .unwrap()
        .control
        .component_mut()
        .unwrap()
        .controllers
        .push(Controller {
            name: "inner".into(),
            page_ids: vec!["x".into(), "y".into()],
            page_names: vec!["X".into(), "Y".into()],
            selected_index: 0,
            ..Default::default()
        });
    let image = store.create(ObjectType::Image);
    store.set_parent(image, Some(component));
    let mut gear = Gear::new(GearKind::Look);
    gear.controller_index = 0;
    gear.controller_owner = Some(component);
    gear.values.insert(
        Some("y".into()),
        GearValue::Look {
            alpha: 1.0,
            rotation: 0.0,
            grayed: false,
            touchable: true,
        },
    );
    store.object_mut(image).unwrap().gears.push(gear);
    let value = GearValue::Look {
        alpha: 1.0,
        rotation: 0.0,
        grayed: true,
        touchable: true,
    };
    store.object_mut(component).unwrap().gears[0]
        .values
        .insert(Some("b".into()), value.clone());
    store.set_controller_page(root, "state", "B");
    assert_eq!(saved(&store, component, Some("b")), value);
    assert!(matches!(
        saved(&store, image, Some("x")),
        GearValue::Look { grayed: true, .. }
    ));
    store.set_controller_page(component, "inner", "Y");
    assert!(!store.object(image).unwrap().grayed());
    store.set_controller_page(component, "inner", "X");
    assert!(store.object(image).unwrap().grayed());
}

#[test]
fn native_text_and_movie_color_setters_do_not_implicitly_capture_their_gears() {
    let (mut store, _, text) = fixture(ObjectType::Text, GearKind::Text);
    let initial = saved(&store, text, Some("a"));
    store.set_object_text(text, "edited");
    assert_eq!(saved(&store, text, Some("a")), initial);
    let (mut store, _, movie) = fixture(ObjectType::MovieClip, GearKind::Color);
    let initial = saved(&store, movie, Some("a"));
    store.set_control_color(movie, Color::new(1.0, 0.0, 0.0, 1.0), None);
    assert_eq!(saved(&store, movie, Some("a")), initial);
    let (mut store, _, movie) = fixture(ObjectType::MovieClip, GearKind::Animation);
    let initial = saved(&store, movie, Some("a"));
    store.report_animation_frame(movie, 6);
    assert_eq!(saved(&store, movie, Some("a")), initial);
    store.set_animation_playing(movie, true);
    assert_eq!(
        saved(&store, movie, Some("a")),
        GearValue::Animation {
            playing: true,
            frame: 6
        }
    );
}

fn trace_color(text: &str) -> Color {
    let bits = u32::from_str_radix(text.trim_start_matches('#'), 16).unwrap();
    Color::from_rgba8((bits >> 16) as u8, (bits >> 8) as u8, bits as u8, 255)
}

fn trace_value(value: Option<GearValue>, expected: &[&str], context: &str) {
    if expected == ["-"] {
        assert!(value.is_none(), "{context}: {value:?}");
        return;
    }
    let value = value.unwrap_or_else(|| panic!("missing {context}"));
    let numbers: Vec<f32> = match &value {
        GearValue::Xy { x, y, px, py } => vec![*x, *y, *px, *py],
        GearValue::Size {
            width,
            height,
            scale_x,
            scale_y,
        } => vec![*width, *height, *scale_x, *scale_y],
        GearValue::Look {
            alpha,
            rotation,
            grayed,
            touchable,
        } => vec![
            *alpha,
            *rotation,
            u8::from(*grayed) as f32,
            u8::from(*touchable) as f32,
        ],
        GearValue::Color {
            color: c,
            stroke_color: s,
        } => vec![c.r, c.g, c.b, c.a, s.r, s.g, s.b, s.a],
        GearValue::Animation { playing, frame } => vec![u8::from(*playing) as f32, *frame as f32],
        GearValue::FontSize(size) => vec![*size as f32],
        GearValue::Text(s) | GearValue::Icon(s) => {
            assert_eq!(
                s,
                if expected[0] == "~" { "" } else { expected[0] },
                "{context}"
            );
            return;
        }
    };
    assert_eq!(numbers.len(), expected.len(), "{context}: {value:?}");
    for (got, want) in numbers.into_iter().zip(expected) {
        let want = want.parse::<f32>().unwrap();
        assert!(
            (got - want).abs() < 0.00002,
            "{context}: {got} != {want}; {value:?}"
        );
    }
}

#[test]
fn executable_native_gear_defaults_page_storage_locks_forwarding_and_setter_calls_match() {
    let mut store = ObjectStore::new();
    let mut ids = None;
    let mut kind = GearKind::Xy;
    let mut other = None;
    let mut cases = 0;
    let mut steps = 0;
    let mut checks = 0;
    let mut context = String::new();
    for line in include_str!("../../tests/fixtures/gear-updates-reference.txt").lines() {
        if line.starts_with('#') {
            continue;
        }
        let p: Vec<_> = line.split_whitespace().collect();
        match p[0] {
            "case" => {
                kind = GearKind::from_code(p[2].parse().unwrap());
                let object_type = match p[3] {
                    "GImage" => ObjectType::Image,
                    "GMovieClip" => ObjectType::MovieClip,
                    "GLoader" => ObjectType::Loader,
                    "GGraph" => ObjectType::Graph,
                    "GTextField" => ObjectType::Text,
                    "GButton" => ObjectType::Button,
                    "GLabel" => ObjectType::Label,
                    "GComboBox" => ObjectType::ComboBox,
                    "GComponent" => ObjectType::Component,
                    _ => panic!("{line}"),
                };
                let (mut initial, root, child) = fixture(object_type, kind);
                initial.object_mut(child).unwrap().gears.clear();
                initial.set_xy(child, 10.0, 20.0);
                initial.set_alpha(child, 0.9);
                initial.set_rotation(child, 12.0);
                initial.set_grayed(child, false);
                if matches!(
                    object_type,
                    ObjectType::Button | ObjectType::Label | ObjectType::ComboBox
                ) {
                    let title = initial.create(ObjectType::Text);
                    initial.set_parent(title, Some(child));
                    initial.set_name(title, "title");
                    initial.set_object_text(title, "initial");
                    initial.set_object_font_size(title, 20);
                    initial.set_control_color(
                        title,
                        trace_color("#326496"),
                        Some(trace_color("#123456")),
                    );
                    let icon = initial.create(ObjectType::Loader);
                    initial.set_parent(icon, Some(child));
                    initial.set_name(icon, "icon");
                    initial.apply_icon_to(icon, "icon-a");
                    initial.initialize_resource_title(child);
                } else {
                    initial.set_object_text(child, "initial");
                    initial.set_object_font_size(child, 20);
                    initial.set_control_color(
                        child,
                        trace_color("#326496"),
                        Some(trace_color("#123456")),
                    );
                }
                initial.apply_icon_to(child, "icon-a");
                initial.set_animation_playing(child, true);
                initial.set_animation_frame(child, 2);
                initial.take_animation_commands(child);
                let value = initial.gear_snapshot(child, kind).unwrap();
                let b = match kind {
                    GearKind::Xy => GearValue::Xy {
                        x: 100.0,
                        y: 40.0,
                        px: 0.25,
                        py: 0.2,
                    },
                    GearKind::Size => GearValue::Size {
                        width: 90.0,
                        height: 70.0,
                        scale_x: 1.3,
                        scale_y: 0.8,
                    },
                    GearKind::Look => GearValue::Look {
                        alpha: 0.4,
                        rotation: 70.0,
                        grayed: true,
                        touchable: false,
                    },
                    GearKind::Color => GearValue::Color {
                        color: trace_color("#BD0044"),
                        stroke_color: trace_color("#55DD11"),
                    },
                    GearKind::Animation => GearValue::Animation {
                        playing: false,
                        frame: 7,
                    },
                    GearKind::Text => GearValue::Text("page-b".into()),
                    GearKind::Icon => GearValue::Icon("icon-b".into()),
                    GearKind::FontSize => GearValue::FontSize(32),
                    _ => unreachable!(),
                };
                let mut gear = Gear::new(kind);
                gear.controller_index = 0;
                gear.controller_owner = Some(root);
                gear.positions_in_percent = p[4] == "1";
                gear.values.insert(None, value.clone());
                gear.values.insert(Some("a".into()), value);
                gear.values.insert(Some("b".into()), b);
                initial.object_mut(child).unwrap().gears.push(gear);
                store = initial;
                ids = Some((root, child));
                other = None;
                context = p[1].into();
                cases += 1;
            }
            "step" => {
                steps += 1;
                context = format!("{} / {line}", context.split(" / ").next().unwrap());
                let (root, child) = ids.unwrap();
                let num = |i: usize| p[i].parse::<f32>().unwrap();
                match p[1] {
                    "page" => {
                        store.set_controller_page_index(root, "state", p[2].parse().unwrap());
                    }
                    "xy" => store.set_xy(child, num(2), num(3)),
                    "size" => store.set_size(child, num(2), num(3)),
                    "scale" => store.set_scale(child, num(2), num(3)),
                    "alpha" => store.set_alpha(child, num(2)),
                    "rotation" => store.set_rotation(child, num(2)),
                    "gray" => store.set_grayed(child, p[2] == "1"),
                    "touch" => store.set_touchable(child, p[2] == "1"),
                    "color" => store.set_control_color(child, trace_color(p[2]), None),
                    "outline" => store.set_control_outline_color(child, trace_color(p[2])),
                    "playing" => store.set_animation_playing(child, p[2] == "1"),
                    "frame" => store.set_animation_frame(child, p[2].parse().unwrap()),
                    "report" => store.report_animation_frame(child, p[2].parse().unwrap()),
                    "text" => {
                        store.set_object_text(child, if p[2] == "~" { "" } else { p[2] });
                    }
                    "icon" => {
                        store.apply_icon_to(child, if p[2] == "~" { "" } else { p[2] });
                    }
                    "font" => {
                        store.set_object_font_size(child, p[2].parse().unwrap());
                    }
                    "capture" => store.update_gear_state(child, kind),
                    "tween" => {
                        store.object_mut(child).unwrap().gears[0].tween = Some(GearTweenConfig {
                            ease_type: 0,
                            duration: 1.0,
                            delay: 0.0,
                        })
                    }
                    "tick" => store.advance_gear_tweens(num(2)),
                    "transition" => {
                        use crate::transition::{ActionType, TransitionValue};
                        let (action, value) = match kind {
                            GearKind::Xy => (
                                ActionType::Xy,
                                TransitionValue::Pair {
                                    use_x: true,
                                    use_y: true,
                                    x: 88.0,
                                    y: 66.0,
                                },
                            ),
                            GearKind::Size => (
                                ActionType::Size,
                                TransitionValue::Pair {
                                    use_x: true,
                                    use_y: true,
                                    x: 88.0,
                                    y: 66.0,
                                },
                            ),
                            GearKind::Look => (ActionType::Alpha, TransitionValue::Scalar(0.2)),
                            GearKind::Color => (
                                ActionType::Color,
                                TransitionValue::Color(trace_color("#FF5500")),
                            ),
                            GearKind::Animation => (
                                ActionType::Animation,
                                TransitionValue::Animation {
                                    playing: false,
                                    frame: 9,
                                },
                            ),
                            GearKind::Text => (
                                ActionType::Text,
                                TransitionValue::Text(Some("transition-text".into())),
                            ),
                            GearKind::Icon => (
                                ActionType::Icon,
                                TransitionValue::Text(Some("transition-icon".into())),
                            ),
                            _ => unreachable!(),
                        };
                        value.apply(&mut store, child, action);
                    }
                    "construct" => store.object_mut(child).unwrap().under_construct = p[2] == "1",
                    "locked" => store.object_mut(child).unwrap().gear_locked = p[2] == "1",
                    "relations" => store.update_gear_relations(child, kind, num(2), num(3)),
                    "parent_size" => {
                        let object = store.object_mut(root).unwrap();
                        object.width = num(2);
                        object.height = num(3);
                    }
                    "reparent" => {
                        let parent = store.create(ObjectType::Component);
                        store.set_size(parent, 200.0, 100.0);
                        store.object_mut(parent).unwrap().control = ControlData::Component(
                            store
                                .object(root)
                                .unwrap()
                                .control()
                                .component()
                                .unwrap()
                                .clone(),
                        );
                        store.set_parent(child, Some(parent));
                        other = Some(parent);
                    }
                    "other_page" => {
                        store.set_controller_page_index(
                            other.unwrap(),
                            "state",
                            p[2].parse().unwrap(),
                        );
                    }
                    _ => panic!("{line}"),
                }
            }
            "current" => {
                trace_value(store.gear_snapshot(ids.unwrap().1, kind), &p[1..], &context);
                checks += 1;
            }
            "default" => {
                trace_value(Some(saved(&store, ids.unwrap().1, None)), &p[1..], &context);
                checks += 1;
            }
            "saved" => {
                let gear = &store.object(ids.unwrap().1).unwrap().gears[0];
                let value = if p[1] == "null" {
                    gear.unselected_value.clone()
                } else {
                    gear.values.get(&Some(p[1].to_owned())).cloned()
                };
                trace_value(value, &p[2..], &format!("{context}: saved {}", p[1]));
                checks += 1;
            }
            _ => panic!("{line}"),
        }
    }
    assert_eq!((cases, steps, checks), (40, 279, 1674));
}

#[test]
fn transition_shake_ticks_and_stop_restoration_preserve_saved_xy_pages() {
    use crate::transition::{ActionType, Transition, TransitionItem, TransitionValue};
    let (mut store, root, child) = fixture(ObjectType::Image, GearKind::Xy);
    let original = saved(&store, child, Some("a"));
    let mut transition = Transition::default();
    transition.name = "shake".into();
    transition.total_duration = 1.0;
    transition.items.push(TransitionItem {
        action: ActionType::Shake,
        time: 0.0,
        target_index: 0,
        label: None,
        value: TransitionValue::Shake {
            amplitude: 20.0,
            duration: 1.0,
        },
        tween: None,
    });
    store
        .object_mut(root)
        .unwrap()
        .control
        .component_mut()
        .unwrap()
        .transitions
        .push(transition);
    store.play_transition(root, "shake", 1);
    store.advance_transitions(0.3);
    assert_ne!(
        store.object(child).unwrap().position(),
        crate::geom::Point::ZERO
    );
    assert_eq!(saved(&store, child, Some("a")), original);
    store.stop_transition(root, "shake");
    assert_eq!(
        store.object(child).unwrap().position(),
        crate::geom::Point::ZERO
    );
    assert_eq!(saved(&store, child, Some("a")), original);
}

#[test]
fn exported_missing_default_uses_instance_geometry_and_null_page_edits_leave_it_intact() {
    use crate::{
        package::{FuiPackage, LoadContext},
        widget::create_from_package,
    };
    let package =
        FuiPackage::parse("Basics", include_bytes!("../../tests/fixtures/Basics.fui")).unwrap();
    let mut store = ObjectStore::new();
    let root = create_from_package(
        &mut store,
        &package,
        package.item_index_by_name("Demo_Controller").unwrap(),
        &LoadContext::none(),
    )
    .unwrap();
    let child = store.child_by_name(root, "n3").unwrap();
    let gear = store
        .object(child)
        .unwrap()
        .gears
        .iter()
        .find(|g| g.kind == GearKind::Xy)
        .unwrap();
    assert!(matches!(
        gear.values[&None],
        GearValue::Xy {
            x: 181.0,
            y: 271.0,
            ..
        }
    ));
    let default = gear.values[&None].clone();
    let controller = store
        .object(root)
        .unwrap()
        .control()
        .component()
        .unwrap()
        .controllers[gear.controller_index as usize]
        .name
        .clone();
    store.set_controller_page_index(root, &controller, -1);
    assert_eq!(
        store.object(child).unwrap().position(),
        crate::geom::Point::new(181.0, 271.0)
    );
    store.set_xy(child, 191.0, 281.0);
    store.set_controller_page_index(root, &controller, 0);
    store.set_controller_page_index(root, &controller, -1);
    store.advance_gear_tweens(10.0);
    assert_eq!(
        store.object(child).unwrap().position(),
        crate::geom::Point::new(191.0, 281.0)
    );
    assert_eq!(
        store
            .object(child)
            .unwrap()
            .gears
            .iter()
            .find(|g| g.kind == GearKind::Xy)
            .unwrap()
            .values[&None],
        default
    );
}

#[test]
fn parent_resize_shifts_all_absolute_relation_pages_and_default_before_current_recapture() {
    use crate::widget::relation::{RelationDef, RelationItem};
    let (mut store, root, child) = fixture(ObjectType::Image, GearKind::Xy);
    store
        .object_mut(child)
        .unwrap()
        .relations
        .items
        .push(RelationItem {
            to_parent: true,
            target_index: None,
            defs: vec![RelationDef {
                relation_type: 6,
                percent: false,
                axis: 0,
            }],
        });
    store.object_mut(child).unwrap().gears[0].values.insert(
        Some("b".into()),
        GearValue::Xy {
            x: 100.0,
            y: 0.0,
            px: 0.25,
            py: 0.0,
        },
    );
    store.set_size(root, 500.0, 200.0);
    assert_eq!(store.object(child).unwrap().x(), 100.0);
    assert!(matches!(
        saved(&store, child, None),
        GearValue::Xy { x: 100.0, .. }
    ));
    assert!(matches!(
        saved(&store, child, Some("b")),
        GearValue::Xy { x: 200.0, .. }
    ));
    store.set_controller_page(root, "state", "B");
    assert_eq!(store.object(child).unwrap().x(), 200.0);
}

#[test]
fn compound_title_and_icon_setters_forward_only_the_property_being_assigned() {
    for kind in [ObjectType::Button, ObjectType::Label, ObjectType::ComboBox] {
        let (mut store, _, owner) = fixture(kind, GearKind::Icon);
        let title = store.create(ObjectType::Text);
        store.set_parent(title, Some(owner));
        store.set_name(title, "title");
        let icon = store.create(ObjectType::Loader);
        store.set_parent(icon, Some(owner));
        store.set_name(icon, "icon");
        store.set_object_text(owner, "base-title");
        store.apply_icon_to(owner, "icon-base");
        store.set_object_text(title, "independent-title");
        store.apply_icon_to(owner, "icon-next");
        assert_eq!(
            store.gear_snapshot(title, GearKind::Text),
            Some(GearValue::Text("independent-title".into()))
        );
        store.apply_icon_to(icon, "independent-icon");
        let stored = saved(&store, owner, Some("a"));
        store.set_object_text(owner, "title-next");
        assert_eq!(
            store.gear_snapshot(icon, GearKind::Icon),
            Some(GearValue::Icon("independent-icon".into()))
        );
        assert_eq!(saved(&store, owner, Some("a")), stored);
    }
}

#[test]
fn input_policy_edits_forward_titles_validate_before_mutating_and_preserve_programmatic_text() {
    for wrapper in [ObjectType::Label, ObjectType::ComboBox, ObjectType::Button] {
        let (mut store, _, input) = fixture(ObjectType::InputText, GearKind::Text);
        let gears = store.object(input).unwrap().gears[0].values.clone();
        let owner = store.create(wrapper);
        store.set_name(input, "title");
        store.set_parent(input, Some(owner));
        store.take_dirty();
        assert!(store.set_object_single_line(owner, true));
        assert!(store.set_input_max_length(owner, 3));
        assert!(store.set_input_restrict(owner, Some("0-9")).unwrap());
        assert_eq!(store.take_dirty(), vec![(input, dirty::TEXT)]);
        assert!(!store.set_input_max_length(owner, 3));
        assert!(!store.set_object_single_line(owner, true));
        assert!(!store.set_input_restrict(owner, Some("0-9")).unwrap());
        assert!(store.set_input_restrict(owner, Some("z-a")).is_err());
        assert!(store.take_dirty().is_empty());
        assert_eq!(store.object(input).unwrap().gears[0].values, gears);
        store.set_object_text(input, "abc12345\r\nEND\n");
        let field = store
            .object(input)
            .unwrap()
            .control()
            .as_text_field()
            .unwrap();
        assert_eq!(field.text, "abc12345END");
    }
}

#[test]
fn prompt_and_editable_settings_default_forward_noop_and_preserve_model_and_gears() {
    use crate::color::Color;
    for wrapper in [ObjectType::Label, ObjectType::Button, ObjectType::ComboBox] {
        let (mut store, _, input) = fixture(ObjectType::InputText, GearKind::Text);
        let owner = store.create(wrapper);
        store.set_name(input, "title");
        store.set_parent(input, Some(owner));
        store.take_dirty();
        let before = store.object(input).unwrap().gears[0].values.clone();
        let data = match store.object(input).unwrap().control() {
            ControlData::TextInput(d) => d,
            _ => unreachable!(),
        };
        assert!(data.editable);
        assert_eq!(data.prompt_color, Color::from_rgba8(169, 169, 169, 255));
        assert!(store.set_input_prompt(owner, Some("[b]Hint[/b]")));
        assert!(store.set_input_editable(owner, false));
        assert!(store.set_input_prompt_color(owner, Color::WHITE));
        assert_eq!(
            store.take_dirty(),
            vec![(input, dirty::TEXT | dirty::COLOR)]
        );
        assert!(!store.set_input_prompt(owner, Some("[b]Hint[/b]")));
        assert!(!store.set_input_editable(owner, false));
        assert!(!store.set_input_prompt_color(owner, Color::WHITE));
        assert!(store.take_dirty().is_empty());
        assert_eq!(store.object(input).unwrap().gears[0].values, before);
        store.set_object_text(input, "Programmatic readonly");
        assert_eq!(
            store
                .object(input)
                .unwrap()
                .control()
                .as_text_field()
                .unwrap()
                .text,
            "Programmatic readonly"
        );
    }
}

#[test]
fn password_setting_forwards_titles_preserves_real_text_and_gears_and_suppresses_noops() {
    for wrapper in [ObjectType::Label, ObjectType::Button, ObjectType::ComboBox] {
        let (mut store, _, input) = fixture(ObjectType::InputText, GearKind::Text);
        let owner = store.create(wrapper);
        store.set_name(input, "title");
        store.set_parent(input, Some(owner));
        store.set_object_text(input, "真实😀");
        let before = store.object(input).unwrap().gears[0].values.clone();
        store.take_dirty();
        assert!(!crate::widget::TextInputData::default().password);
        assert_eq!(store.input_field_of(owner), Some(input));
        assert!(store.set_input_password(owner, true));
        assert_eq!(store.take_dirty(), vec![(input, dirty::TEXT)]);
        assert!(!store.set_input_password(owner, true));
        assert!(store.take_dirty().is_empty());
        assert_eq!(
            store
                .object(input)
                .unwrap()
                .control()
                .as_text_field()
                .unwrap()
                .text,
            "真实😀"
        );
        assert_eq!(store.object(input).unwrap().gears[0].values, before);
        assert!(store.set_input_password(owner, false));
        let ordinary = store.create(ObjectType::Text);
        assert!(!store.set_input_password(ordinary, true));
    }
}

#[test]
fn stroke_width_forwards_titles_without_capturing_gears_and_keeps_native_fractional_and_negative_values()
 {
    for wrapper in [ObjectType::Button, ObjectType::Label, ObjectType::ComboBox] {
        let (mut store, _, text) = fixture(ObjectType::Text, GearKind::Color);
        let owner = store.create(wrapper);
        store.set_name(text, "title");
        store.set_parent(text, Some(owner));
        let before = store.object(text).unwrap().gears[0].values.clone();
        store.take_dirty();
        assert!(store.set_object_stroke(owner, 2.5));
        assert_eq!(store.take_dirty(), vec![(text, dirty::TEXT)]);
        assert!(!store.set_object_stroke(owner, 2.5));
        assert!(!store.set_object_stroke(owner, f32::NAN));
        assert!(!store.set_object_stroke(owner, f32::INFINITY));
        assert!(store.take_dirty().is_empty());
        assert_eq!(
            store
                .object(text)
                .unwrap()
                .control()
                .as_text_field()
                .unwrap()
                .stroke,
            2.5
        );
        assert!(store.set_object_stroke(owner, -1.));
        assert_eq!(
            store
                .object(text)
                .unwrap()
                .control()
                .as_text_field()
                .unwrap()
                .stroke,
            -1.
        );
        assert_eq!(store.object(text).unwrap().gears[0].values, before);
    }
}

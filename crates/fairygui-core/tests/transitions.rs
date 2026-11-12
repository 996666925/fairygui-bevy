//! 过渡动画的集成测试，使用官方示例包。

use fairygui_core::object::{GObjectId, ObjectStore};
use fairygui_core::package::{FuiPackage, LoadContext, PackageItemType};
use fairygui_core::widget::create_from_package;

const TRANSITION: &[u8] = include_bytes!("fixtures/Transition.fui");

#[test]
fn official_power_up_interval_reconstructs_movie_and_owner_motion_without_old_start_hooks() {
    use fairygui_core::object::AnimationCommand;
    use fairygui_core::transition::{ActionType, TransitionPlayOptions, TransitionValue};
    let package = FuiPackage::parse("Transition", TRANSITION).unwrap();
    let mut store = ObjectStore::new();
    let root = create_from_package(
        &mut store,
        &package,
        package.item_index_by_name("PowerUp").unwrap(),
        &LoadContext::none(),
    )
    .unwrap();
    store.set_xy(root, 500.0, 600.0);
    let definition = store
        .object(root)
        .unwrap()
        .control()
        .component()
        .unwrap()
        .transitions[0]
        .clone();
    let animation = definition
        .items
        .iter()
        .find(|item| item.action == ActionType::Animation)
        .unwrap();
    let start_time = animation.time + 0.25;
    let movie = store.child_by_name(root, "powEffect").unwrap();
    store.play_transition_with_options(
        root,
        "t0",
        TransitionPlayOptions {
            start_time,
            end_time: Some(start_time + 0.125),
            ..Default::default()
        },
    );
    let commands = store.take_animation_commands(movie);
    assert!(
        matches!(commands.last(), Some(AnimationCommand::Advance(seconds)) if (*seconds - 0.25).abs() < 0.0001)
    );
    let owner_motion = definition
        .items
        .iter()
        .rfind(|item| {
            item.action == ActionType::Xy && item.target_index < 0 && item.time <= start_time
        })
        .unwrap();
    let value = owner_motion.tween.as_ref().map_or_else(
        || owner_motion.value.clone(),
        |tween| {
            tween.value_at(fairygui_core::transition::evaluate(
                tween.ease_type,
                (start_time - owner_motion.time).min(tween.duration),
                tween.duration,
            ))
        },
    );
    let TransitionValue::Pair { x, y, .. } = value else {
        panic!("owner XY");
    };
    let position = store.object(root).unwrap().position();
    assert!((position.x - (500.0 + x)).abs() < 0.001);
    assert!((position.y - (600.0 + y)).abs() < 0.001);
    assert!(store.take_transition_hooks().iter().all(|hook| {
        hook.at_end
            || definition.items.iter().any(|item| {
                item.label.as_deref() == Some(hook.label.as_str()) && item.time >= start_time
            })
    }));
    store.advance_transitions(1.0);
    assert_eq!(
        store.transition_state(root, "t0"),
        Some((false, start_time + 0.125))
    );
}

#[test]
fn official_power_up_reverse_keeps_window_origin_and_returns_owner_motion_to_zero() {
    let package = FuiPackage::parse("Transition", TRANSITION).unwrap();
    let mut store = ObjectStore::new();
    let root = create_from_package(
        &mut store,
        &package,
        package.item_index_by_name("PowerUp").unwrap(),
        &LoadContext::none(),
    )
    .unwrap();
    store.set_xy(root, 500.0, 600.0);
    store.play_transition_reverse(root, "t0", 0.0);
    assert_eq!(
        store.position_of(root),
        Some(fairygui_core::geom::Point::new(460.0, 200.0))
    );
    store.advance_transitions(10.0);
    assert_eq!(
        store.position_of(root),
        Some(fairygui_core::geom::Point::new(500.0, 600.0))
    );
    assert_eq!(store.object(root).unwrap().alpha(), 1.0);
    assert!(!store.transition_state(root, "t0").unwrap().0);
}

#[test]
fn official_path_demo_nested_transition_drives_scale_and_stops_with_parent() {
    let package = FuiPackage::parse("Transition", TRANSITION).unwrap();
    let mut store = ObjectStore::new();
    let root = create_from_package(
        &mut store,
        &package,
        package.item_index_by_name("PathDemo").unwrap(),
        &LoadContext::none(),
    )
    .unwrap();
    let child = store.child_by_name(root, "n2").unwrap();
    let initial = store.position_of(child).unwrap();
    let config = store
        .object(root)
        .unwrap()
        .control()
        .component()
        .unwrap()
        .transitions
        .iter()
        .find(|t| t.name == "t1")
        .unwrap()
        .items
        .iter()
        .find(|i| i.action == fairygui_core::transition::ActionType::Scale)
        .unwrap()
        .tween
        .as_ref()
        .unwrap()
        .clone();
    let duration = config.duration;
    assert_eq!(config.ease_type, 5, "exported Scale uses QuadOut");
    store.play_transition(root, "t0", 1);
    store.advance_transitions(duration / 2.0);
    assert!(store.transition_state(root, "t1").unwrap().0);
    assert!((store.object(child).unwrap().scale_x() - 1.025).abs() < 0.0001);
    assert_ne!(store.position_of(child).unwrap(), initial);
    store.stop_transition(root, "t0");
    assert!(!store.transition_state(root, "t1").unwrap().0);
    assert!((store.object(child).unwrap().scale_x() - 0.8).abs() < 0.0001);
}

#[test]
fn official_good_hit_shakes_for_its_full_duration_and_restores_on_stop() {
    let package = FuiPackage::parse("Transition", TRANSITION).unwrap();
    let mut store = ObjectStore::new();
    let root = create_from_package(
        &mut store,
        &package,
        package.item_index_by_name("GoodHit").unwrap(),
        &LoadContext::none(),
    )
    .unwrap();
    store.set_xy(root, 200.0, 100.0);
    let transition = &store
        .object(root)
        .unwrap()
        .control()
        .component()
        .unwrap()
        .transitions[0];
    let (start, amplitude, duration) = transition
        .items
        .iter()
        .find_map(|item| {
            if let fairygui_core::transition::TransitionValue::Shake {
                amplitude,
                duration,
            } = item.value
            {
                Some((item.time, amplitude, duration))
            } else {
                None
            }
        })
        .unwrap();
    assert!(store.play_transition(root, "t0", 1));
    store.advance_transitions(start + duration / 2.0);
    assert!(store.transition_state(root, "t0").unwrap().0);
    let shaken = store.object(root).unwrap().position();
    assert!(((shaken.x - 200.0).abs() - amplitude * 0.25).abs() < 0.001);
    assert!(store.stop_transition_with_completion(root, "t0", false));
    assert_eq!(
        store.object(root).unwrap().position(),
        fairygui_core::geom::Point::new(200.0, 100.0)
    );
    store.play_transition(root, "t0", 1);
    store.advance_transitions(start + duration + 0.1);
    assert!(!store.transition_state(root, "t0").unwrap().0);
    assert_eq!(
        store.object(root).unwrap().position(),
        fairygui_core::geom::Point::new(200.0, 100.0)
    );
}

#[test]
fn official_path_demo_uses_exported_paths_and_keeps_infinite_tweens_running() {
    let package = FuiPackage::parse("Transition", TRANSITION).unwrap();
    let mut store = ObjectStore::new();
    let root = create_from_package(
        &mut store,
        &package,
        package.item_index_by_name("PathDemo").unwrap(),
        &LoadContext::none(),
    )
    .unwrap();
    let transition = &store
        .object(root)
        .unwrap()
        .control()
        .component()
        .unwrap()
        .transitions[0];
    let (target_index, config) = transition
        .items
        .iter()
        .find_map(|item| {
            item.tween
                .as_ref()
                .filter(|t| t.path.is_some())
                .map(|t| (item.target_index, t.clone()))
        })
        .unwrap();
    assert_eq!(config.repeat, -1);
    let target = store.child_at(root, target_index as usize).unwrap();
    let path = config.path.as_ref().unwrap();
    let offset = path.point_at(0.5);
    let fairygui_core::transition::TransitionValue::Pair { x, y, .. } = config.start_value else {
        panic!("XY start")
    };
    store.play_transition(root, "t0", 1);
    store.advance_transitions(config.duration * 0.5);
    assert!(
        store.transition_state(root, "t1").unwrap().0,
        "t0 must launch its same-owner nested t1"
    );
    let actual = store.object(target).unwrap().position();
    assert!((actual.x - x - offset.x).abs() < 0.001);
    assert!((actual.y - y - offset.y).abs() < 0.001);
    store.advance_transitions(config.duration * 2.0);
    assert!(store.transition_state(root, "t0").unwrap().0);
    store.stop_transition_with_completion(root, "t0", false);
    assert!(!store.transition_state(root, "t1").unwrap().0);
}

#[test]
fn official_power_up_owner_motion_preserves_window_origin_and_delivers_end_hook() {
    let package = FuiPackage::parse("Transition", TRANSITION).unwrap();
    let mut store = ObjectStore::new();
    let root = create_from_package(
        &mut store,
        &package,
        package.item_index_by_name("PowerUp").unwrap(),
        &LoadContext::none(),
    )
    .unwrap();
    store.set_xy(root, 500.0, 600.0);
    store.play_transition(root, "t0", 1);
    assert_eq!(
        store.object(root).unwrap().position(),
        fairygui_core::geom::Point::new(500.0, 600.0)
    );
    store.advance_transitions(10.0);
    let position = store.object(root).unwrap().position();
    assert_eq!(position, fairygui_core::geom::Point::new(460.0, 200.0));
    let hooks = store.take_transition_hooks();
    assert!(
        hooks
            .iter()
            .any(|hook| hook.label == "end" && hook.at_end && hook.owner == root)
    );
    assert!(!store.transition_state(root, "t0").unwrap().0);
}

/// 统计包内所有组件上的过渡数量。
fn count_transitions(package: &FuiPackage) -> (usize, usize) {
    let mut total = 0usize;
    let mut with_items = 0usize;

    for (index, item) in package.items().iter().enumerate() {
        if item.item_type != PackageItemType::Component {
            continue;
        }
        let mut store = ObjectStore::new();
        let Ok(root) = create_from_package(&mut store, package, index, &LoadContext::none()) else {
            continue;
        };

        let mut visit = |id: GObjectId| {
            let Some(object) = store.object(id) else {
                return;
            };
            let Some(data) = object.control().component() else {
                return;
            };
            total += data.transitions.len();
            with_items += data
                .transitions
                .iter()
                .filter(|transition| !transition.items.is_empty())
                .count();
        };
        visit(root);
        store.for_each_descendant(root, &mut visit);
    }

    (total, with_items)
}

#[test]
fn sample_package_contains_transitions() {
    let package = FuiPackage::parse("Transition", TRANSITION).expect("parse transition package");
    let (total, with_items) = count_transitions(&package);

    assert!(total > 0, "示例包中应当包含过渡动画");
    assert!(
        with_items > 0,
        "至少应当有过渡解析出了具体的动画项，共 {total} 个过渡"
    );
}

#[test]
fn transitions_have_non_negative_duration() {
    let package = FuiPackage::parse("Transition", TRANSITION).expect("parse transition package");

    for (index, item) in package.items().iter().enumerate() {
        if item.item_type != PackageItemType::Component {
            continue;
        }
        let mut store = ObjectStore::new();
        let Ok(root) = create_from_package(&mut store, &package, index, &LoadContext::none())
        else {
            continue;
        };

        let mut check = |id: GObjectId| {
            let Some(object) = store.object(id) else {
                return;
            };
            let Some(data) = object.control().component() else {
                return;
            };
            for transition in &data.transitions {
                assert!(
                    transition.total_duration >= 0.0,
                    "过渡 {} 的总时长不应为负",
                    transition.name
                );
                for entry in &transition.items {
                    assert!(entry.time >= 0.0, "过渡项的开始时刻不应为负");
                    assert!(
                        entry.end_time() >= entry.time,
                        "过渡项的结束时刻不应早于开始时刻"
                    );
                }
            }
        };
        check(root);
        store.for_each_descendant(root, &mut check);
    }
}

#[test]
fn transitions_can_be_played_by_name() {
    let package = FuiPackage::parse("Transition", TRANSITION).expect("parse transition package");

    for (index, item) in package.items().iter().enumerate() {
        if item.item_type != PackageItemType::Component {
            continue;
        }
        let mut store = ObjectStore::new();
        let Ok(root) = create_from_package(&mut store, &package, index, &LoadContext::none())
        else {
            continue;
        };

        // 找一个带具名过渡的容器。
        let mut found = None;
        let mut visit = |id: GObjectId| {
            if found.is_some() {
                return;
            }
            if let Some(object) = store.object(id)
                && let Some(data) = object.control().component()
                && let Some(transition) = data.transitions.iter().find(|t| !t.name.is_empty())
            {
                found = Some((id, transition.name.clone()));
            }
        };
        visit(root);
        store.for_each_descendant(root, &mut visit);

        let Some((id, name)) = found else {
            continue;
        };

        assert!(
            store.play_transition(id, &name, 1),
            "按名称应当能启动过渡 {name}"
        );
        assert!(!store.play_transition(id, "不存在的过渡", 1));

        // 推进足够长时间后应当自动结束。
        for _ in 0..600 {
            store.advance_transitions(1.0 / 60.0);
        }
        return;
    }
}

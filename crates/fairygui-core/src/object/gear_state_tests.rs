use super::*;
use crate::color::Color;
use crate::widget::{Controller, Gear, GearKind, GearTweenConfig, GearValue};

fn fixture(kind: ObjectType) -> (ObjectStore, GObjectId, GObjectId) {
    let mut store = ObjectStore::new();
    let owner = store.create(ObjectType::Component);
    store
        .object_mut(owner)
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
    store.set_parent(child, Some(owner));
    (store, owner, child)
}

fn add_display(store: &mut ObjectStore, child: GObjectId) {
    let mut display = Gear::new(GearKind::Display);
    display.controller_index = 0;
    display.pages.push("a".into());
    store.object_mut(child).unwrap().gears.push(display);
}

fn add_xy(store: &mut ObjectStore, child: GObjectId) {
    let mut xy = Gear::new(GearKind::Xy);
    xy.controller_index = 0;
    xy.tween = Some(GearTweenConfig {
        duration: 1.0,
        ease_type: 0,
        delay: 0.5,
    });
    for (id, x) in [("a", 0.0), ("b", 100.0), ("c", 200.0)] {
        xy.values.insert(
            Some(id.into()),
            GearValue::Xy {
                x,
                y: 0.0,
                px: 0.0,
                py: 0.0,
            },
        );
    }
    store.object_mut(child).unwrap().gears.push(xy);
}

#[test]
fn display_lock_keeps_a_hidden_page_connected_during_delay_and_tween() {
    let (mut store, owner, child) = fixture(ObjectType::Image);
    // Deliberately reverse the serialized order: display must still apply first.
    add_xy(&mut store, child);
    add_display(&mut store, child);
    store.initialize_controllers(owner);
    store.set_controller_page(owner, "state", "B");
    assert!(store.object(child).unwrap().display_visible());
    store.advance_gear_tweens(0.25);
    assert!(store.object(child).unwrap().display_visible());
    assert_eq!(store.object(child).unwrap().x(), 0.0);
    store.advance_gear_tweens(1.25);
    assert!(!store.object(child).unwrap().display_visible());
    assert!(
        store.object(child).unwrap().visible(),
        "gear never overwrites explicit visible"
    );
    assert_eq!(store.object(child).unwrap().x(), 100.0);
}

#[test]
fn explicit_hiding_is_not_overridden_by_a_display_lock_or_page_change() {
    let (mut store, owner, child) = fixture(ObjectType::Image);
    add_display(&mut store, child);
    add_xy(&mut store, child);
    store.set_visible(child, false);
    store.initialize_controllers(owner);
    store.set_controller_page(owner, "state", "B");
    assert!(!store.object(child).unwrap().display_visible());
    store.set_visible(child, true);
    assert!(store.object(child).unwrap().display_visible());
    store.advance_gear_tweens(2.0);
    assert!(!store.object(child).unwrap().display_visible());
}

#[test]
fn overlapping_locks_release_independently_and_stale_generations_are_ignored() {
    let (mut store, owner, child) = fixture(ObjectType::Image);
    add_display(&mut store, child);
    store.initialize_controllers(owner);
    let old = store.add_display_lock(child).unwrap();
    store.set_controller_page(owner, "state", "B");
    let first = store.add_display_lock(child).unwrap();
    let second = store.add_display_lock(child).unwrap();
    assert!(!store.release_display_lock(child, old));
    assert!(!store.release_display_lock(owner, first));
    assert!(store.release_display_lock(child, first));
    assert!(store.object(child).unwrap().display_visible());
    assert!(store.release_display_lock(child, second));
    assert!(!store.object(child).unwrap().display_visible());
}

#[test]
fn two_numeric_tweens_keep_display_connected_until_both_finish() {
    let (mut store, owner, child) = fixture(ObjectType::Image);
    add_display(&mut store, child);
    add_xy(&mut store, child);
    let mut size = Gear::new(GearKind::Size);
    size.controller_index = 0;
    size.tween = Some(GearTweenConfig {
        duration: 2.0,
        ease_type: 0,
        delay: 0.0,
    });
    for (id, width) in [("a", 10.0), ("b", 20.0)] {
        size.values.insert(
            Some(id.into()),
            GearValue::Size {
                width,
                height: 10.0,
                scale_x: 1.0,
                scale_y: 1.0,
            },
        );
    }
    store.object_mut(child).unwrap().gears.push(size);
    store.initialize_controllers(owner);
    store.set_controller_page(owner, "state", "B");
    store.advance_gear_tweens(1.5);
    assert!(store.object(child).unwrap().display_visible());
    store.advance_gear_tweens(0.5);
    assert!(!store.object(child).unwrap().display_visible());
}

#[test]
fn display2_and_or_conditions_combine_with_primary_display_and_locks() {
    for condition in [0, 1] {
        let (mut store, owner, child) = fixture(ObjectType::Image);
        add_display(&mut store, child);
        let mut second = Gear::new(GearKind::Display2);
        second.controller_index = 0;
        second.pages.push("b".into());
        second.condition = condition;
        store.object_mut(child).unwrap().gears.push(second);
        store.initialize_controllers(owner);
        assert_eq!(
            store.object(child).unwrap().display_visible(),
            condition == 1
        );
        store.set_controller_page(owner, "state", "B");
        assert_eq!(
            store.object(child).unwrap().display_visible(),
            condition == 1
        );
        let token = store.add_display_lock(child).unwrap();
        assert!(store.object(child).unwrap().display_visible());
        store.release_display_lock(child, token);
        assert_eq!(
            store.object(child).unwrap().display_visible(),
            condition == 1
        );
    }
}

#[test]
fn initialization_and_tween_cancellation_do_not_leak_display_locks() {
    let (mut store, owner, child) = fixture(ObjectType::Image);
    add_display(&mut store, child);
    add_xy(&mut store, child);
    store.initialize_controllers(owner);
    store.set_controller_page(owner, "state", "B");
    store.initialize_controllers(owner);
    assert!(!store.object(child).unwrap().display_visible());
    assert!(!store.has_gear_tweens());
    assert_eq!(store.object(child).unwrap().x(), 100.0);
}

#[test]
fn animation_gear_applies_playing_and_frame_to_movies_and_loaders() {
    for kind in [ObjectType::MovieClip, ObjectType::Loader] {
        let (mut store, owner, child) = fixture(kind);
        let mut gear = Gear::new(GearKind::Animation);
        gear.controller_index = 0;
        gear.tween = Some(GearTweenConfig::default());
        gear.values.insert(
            None,
            GearValue::Animation {
                playing: true,
                frame: 2,
            },
        );
        gear.values.insert(
            Some("b".into()),
            GearValue::Animation {
                playing: false,
                frame: 7,
            },
        );
        store.object_mut(child).unwrap().gears.push(gear);
        store.initialize_controllers(owner);
        assert_eq!(store.animation_state(child), Some((true, 2)));
        store.set_controller_page(owner, "state", "B");
        assert_eq!(store.animation_state(child), Some((false, 7)));
        store.report_animation_frame(child, 6);
        assert_eq!(store.animation_state(child), Some((false, 6)));
        assert!(!store.has_gear_tweens());
        store.set_controller_page(owner, "state", "C");
        assert_eq!(store.animation_state(child), Some((true, 2)));
    }
}

#[test]
fn color_gear_changes_fill_and_outline_immediately_even_if_tween_config_is_present() {
    let (mut store, owner, child) = fixture(ObjectType::Text);
    let mut gear = Gear::new(GearKind::Color);
    gear.controller_index = 0;
    gear.tween = Some(GearTweenConfig::default());
    gear.values.insert(
        Some("b".into()),
        GearValue::Color {
            color: Color::new(1.0, 0.0, 0.0, 1.0),
            stroke_color: Color::new(0.0, 1.0, 0.0, 1.0),
        },
    );
    store.object_mut(child).unwrap().gears.push(gear);
    store.initialize_controllers(owner);
    store.set_controller_page(owner, "state", "B");
    let data = store
        .object(child)
        .unwrap()
        .control()
        .as_text_field()
        .unwrap();
    assert_eq!(data.color, Color::new(1.0, 0.0, 0.0, 1.0));
    assert_eq!(data.stroke_color, Some(Color::new(0.0, 1.0, 0.0, 1.0)));
    assert!(!store.has_gear_tweens());
}

#[test]
fn label_color_properties_forward_to_the_title_and_graph_color_changes_fill() {
    let mut store = ObjectStore::new();
    let label = store.create(ObjectType::Label);
    let title = store.create(ObjectType::Text);
    store.set_parent(title, Some(label));
    store.set_name(title, "title");
    let graph = store.create(ObjectType::Graph);
    let color = Color::new(0.5, 0.25, 0.75, 1.0);
    store.set_control_color(label, color, Some(Color::WHITE));
    store.set_control_color(graph, color, None);
    assert_eq!(
        store
            .object(title)
            .unwrap()
            .control()
            .as_text_field()
            .unwrap()
            .color,
        color
    );
    let crate::widget::ControlData::Graph(data) = store.object(graph).unwrap().control() else {
        panic!("graph")
    };
    assert_eq!(data.fill_color, color);
}

#[test]
fn discrete_animation_and_sound_transition_actions_fire_once_per_play() {
    use crate::transition::{ActionType, Transition, TransitionItem, TransitionValue};
    let mut store = ObjectStore::new();
    let movie = store.create(ObjectType::MovieClip);
    let mut transition = Transition::default();
    transition.total_duration = 1.0;
    for (action, value) in [
        (
            ActionType::Animation,
            TransitionValue::Animation {
                playing: true,
                frame: 1,
            },
        ),
        (
            ActionType::Sound,
            TransitionValue::Sound {
                sound: Some("click.wav".into()),
                volume: 1.0,
            },
        ),
    ] {
        transition.items.push(TransitionItem {
            action,
            time: 0.0,
            target_index: -1,
            label: None,
            value,
            tween: None,
        });
    }
    transition.play(1, 0.0);
    transition.advance(&mut store, movie, 0.25);
    assert_eq!(store.animation_state(movie), Some((true, 1)));
    assert_eq!(store.take_queued_sounds().len(), 1);
    store.report_animation_frame(movie, 3);
    transition.advance(&mut store, movie, 0.25);
    assert_eq!(store.animation_state(movie), Some((true, 3)));
    assert!(store.take_queued_sounds().is_empty());
    transition.play(1, 0.0);
    transition.advance(&mut store, movie, 0.25);
    assert_eq!(store.animation_state(movie), Some((true, 1)));
    assert_eq!(store.take_queued_sounds().len(), 1);
}

#[test]
fn group_display_locks_invalidate_members_and_group_removal_restores_their_visibility() {
    let (mut store, owner, group) = fixture(ObjectType::Group);
    add_display(&mut store, group);
    let member = store.create(ObjectType::Image);
    store.set_parent(member, Some(owner));
    store.set_group(member, Some(group));
    store.initialize_controllers(owner);
    store.set_controller_page(owner, "state", "B");
    assert!(!store.display_visible(member));
    store.take_dirty();
    let token = store.add_display_lock(group).unwrap();
    assert!(store.display_visible(member));
    assert!(
        store
            .take_dirty()
            .iter()
            .any(|&(id, flags)| id == member && flags & dirty::LOOK != 0)
    );
    store.release_display_lock(group, token);
    assert!(!store.display_visible(member));
    store.take_dirty();
    store.remove(group);
    assert_eq!(store.object(member).unwrap().group(), None);
    assert!(store.display_visible(member));
    assert!(
        store
            .take_dirty()
            .iter()
            .any(|&(id, flags)| id == member && flags & dirty::LOOK != 0)
    );
}

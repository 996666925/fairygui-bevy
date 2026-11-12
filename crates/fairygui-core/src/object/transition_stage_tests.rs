use super::tests::{alpha, attach, close, fade, nested};
use super::*;

fn fixture(options: i32) -> (ObjectStore, GObjectId, GObjectId) {
    let mut store = ObjectStore::new();
    let stage = store.create(ObjectType::Component);
    let owner = store.create(ObjectType::Component);
    store.set_parent(owner, Some(stage));
    attach(&mut store, owner, "t0", vec![fade(1.0)]);
    store.objects[owner]
        .control
        .component_mut()
        .unwrap()
        .transitions[0]
        .options = options;
    (store, stage, owner)
}

#[test]
fn stage_mount_starts_auto_play_with_delay_and_same_flag_does_not_update_settings() {
    let (mut store, stage, owner) = fixture(0);
    assert!(
        store
            .set_transition_auto_play_with_options(owner, "t0", true, 2, 0.5)
            .unwrap()
    );
    assert!(!store.transition_state(owner, "t0").unwrap().0);
    store.set_alpha(owner, 0.75);
    store.attach_to_stage(stage);
    assert!(store.is_on_stage(owner));
    store.advance_transitions(0.25);
    close(alpha(&store, owner), 0.75);
    assert!(
        !store
            .set_transition_auto_play_with_options(owner, "t0", true, 9, 10.0)
            .unwrap()
    );
    store.advance_transitions(0.75);
    close(alpha(&store, owner), 0.5);
    let transition = &store.objects[owner]
        .control
        .component()
        .unwrap()
        .transitions[0];
    assert_eq!(
        (transition.auto_play_times, transition.auto_play_delay),
        (2, 0.5)
    );
}

#[test]
fn enabling_auto_play_on_stage_restarts_existing_play_but_disabling_leaves_it_running() {
    let (mut store, stage, owner) = fixture(0);
    store.attach_to_stage(stage);
    store.play_transition(owner, "t0", 1);
    store.advance_transitions(0.25);
    store.take_transition_hooks();
    store.set_transition_auto_play(owner, "t0", true).unwrap();
    close(alpha(&store, owner), 0.0);
    assert!(store.take_transition_hooks().iter().any(|hook| hook.at_end));
    store.set_transition_auto_play(owner, "t0", false).unwrap();
    store.advance_transitions(0.25);
    close(alpha(&store, owner), 0.25);
    assert!(store.transition_state(owner, "t0").unwrap().0);
}

#[test]
fn stage_removal_options_preserve_complete_or_continue_and_disabled_takes_priority() {
    for options in [0, 4, 2, 6] {
        let (mut store, stage, owner) = fixture(options);
        store.attach_to_stage(stage);
        store.play_transition(owner, "t0", 1);
        store.advance_transitions(0.25);
        store.detach_from_stage(stage);
        assert!(!store.is_on_stage(owner));
        close(alpha(&store, owner), if options == 4 { 1.0 } else { 0.25 });
        assert_eq!(
            store.transition_state(owner, "t0").unwrap().0,
            options & 2 != 0
        );
        store.advance_transitions(0.25);
        close(
            alpha(&store, owner),
            if options & 2 != 0 {
                0.5
            } else if options == 4 {
                1.0
            } else {
                0.25
            },
        );
    }
}

#[test]
fn readding_auto_play_skips_an_already_running_transition_but_restarts_a_stopped_one() {
    for options in [0, 2] {
        let (mut store, stage, owner) = fixture(options);
        store.set_transition_auto_play(owner, "t0", true).unwrap();
        store.attach_to_stage(stage);
        store.advance_transitions(0.25);
        store.detach_from_stage(stage);
        store.attach_to_stage(stage);
        close(alpha(&store, owner), if options == 0 { 0.0 } else { 0.25 });
    }
}

#[test]
fn explicit_visibility_does_not_disconnect_but_group_display_and_locks_do() {
    let (mut store, stage, owner) = fixture(0);
    let group = store.create(ObjectType::Group);
    store.set_parent(group, Some(stage));
    store.set_group(owner, Some(group));
    store.set_transition_auto_play(owner, "t0", true).unwrap();
    store.attach_to_stage(stage);
    store.set_visible(stage, false);
    store.set_visible(group, false);
    store.set_visible(owner, false);
    store.advance_transitions(0.25);
    assert!(store.is_on_stage(owner));
    close(alpha(&store, owner), 0.25);
    store.apply_display_gear(group, crate::widget::GearKind::Display, 0, false, 0);
    assert!(!store.is_on_stage(owner));
    assert!(!store.transition_state(owner, "t0").unwrap().0);
    let lock = store.add_display_lock(group).unwrap();
    assert!(store.is_on_stage(owner));
    assert!(store.transition_state(owner, "t0").unwrap().0);
    assert!(!store.display_visible(owner));
    store.release_display_lock(group, lock);
    assert!(!store.is_on_stage(owner));
}

#[test]
fn on_stage_reparent_dispatches_leave_and_enter_while_reordering_does_not() {
    let (mut store, stage, owner) = fixture(0);
    let other = store.create(ObjectType::Component);
    store.set_parent(other, Some(stage));
    store.set_transition_auto_play(owner, "t0", true).unwrap();
    store.attach_to_stage(stage);
    store.advance_transitions(0.25);
    store.add_child_at(stage, owner, 1);
    close(store.transition_state(owner, "t0").unwrap().1, 0.25);
    store.set_parent(owner, Some(other));
    close(store.transition_state(owner, "t0").unwrap().1, 0.0);
    assert!(store.is_on_stage(owner));
}

#[test]
fn adding_child_subtree_starts_only_new_auto_play_and_parent_nested_call_is_not_restarted() {
    let (mut store, stage, owner) = fixture(0);
    store.set_transition_auto_play(owner, "t0", true).unwrap();
    store.attach_to_stage(stage);
    store.advance_transitions(0.25);
    let parent = store.create(ObjectType::Component);
    let child = store.create(ObjectType::Component);
    store.set_parent(child, Some(parent));
    attach(&mut store, parent, "t0", vec![nested(0.0, 0, "t0", 1)]);
    attach(&mut store, child, "t0", vec![fade(1.0)]);
    store.set_transition_auto_play(parent, "t0", true).unwrap();
    store
        .set_transition_auto_play_with_options(child, "t0", true, 1, 0.5)
        .unwrap();
    store.set_parent(parent, Some(stage));
    // Parent starts child explicitly without delay; child's own added event must
    // not restart it with its configured 0.5 second auto delay.
    store.advance_transitions(0.25);
    close(alpha(&store, child), 0.25);
    close(alpha(&store, owner), 0.5);
}

#[test]
fn stage_stop_does_not_notify_parent_but_disabling_auto_offstage_does() {
    for disable_auto in [false, true] {
        let (mut store, stage, owner) = fixture(2);
        store.objects[owner]
            .control
            .component_mut()
            .unwrap()
            .transitions
            .clear();
        attach(&mut store, owner, "t0", vec![nested(0.0, -1, "child", 1)]);
        attach(&mut store, owner, "child", vec![fade(1.0)]);
        store.objects[owner]
            .control
            .component_mut()
            .unwrap()
            .transitions[0]
            .options = 2;
        if disable_auto {
            store.objects[owner]
                .control
                .component_mut()
                .unwrap()
                .transitions[1]
                .options = 2;
        }
        store.attach_to_stage(stage);
        store.play_transition(owner, "t0", 1);
        store.advance_transitions(0.25);
        store.detach_from_stage(stage);
        if disable_auto {
            store
                .set_transition_auto_play(owner, "child", true)
                .unwrap();
            store
                .set_transition_auto_play(owner, "child", false)
                .unwrap();
        }
        assert_eq!(
            store.transition_state(owner, "t0").unwrap().0,
            !disable_auto
        );
        assert!(!store.transition_state(owner, "child").unwrap().0);
    }
}

#[test]
fn window_hide_retains_objects_and_reshow_restarts_auto_while_bring_to_front_keeps_progress() {
    let (mut store, stage, owner) = fixture(0);
    store.detach_from_parent(owner);
    store.set_transition_auto_play(owner, "t0", true).unwrap();
    store.attach_to_stage(stage);
    let mut root = crate::root::Root::new(stage);
    let mut window = crate::window::Window::new(owner);
    window.show(&mut store, &mut root, false);
    store.advance_transitions(0.25);
    let other = store.create(ObjectType::Component);
    root.show_popup(&mut store, other, false);
    root.bring_to_front(&mut store, owner);
    close(store.transition_state(owner, "t0").unwrap().1, 0.25);
    window.hide(&mut store, &mut root);
    assert!(store.contains(owner));
    assert!(!store.is_on_stage(owner));
    window.show(&mut store, &mut root, false);
    assert!(store.is_on_stage(owner));
    close(store.transition_state(owner, "t0").unwrap().1, 0.0);
}

#[test]
fn disposal_cancels_disabled_auto_stop_and_does_not_disturb_other_stage_roots() {
    let (mut store, stage, owner) = fixture(6);
    store.attach_to_stage(stage);
    store.play_transition(owner, "t0", -1);
    let other = store.create(ObjectType::Component);
    attach(&mut store, other, "t0", vec![fade(1.0)]);
    store.set_transition_auto_play(other, "t0", true).unwrap();
    store.attach_to_stage(other);
    store.advance_transitions(0.25);
    store.remove(stage);
    assert!(!store.contains(owner));
    assert!(!store.is_on_stage(owner));
    assert!(store.is_on_stage(other));
    store.advance_transitions(0.25);
    close(alpha(&store, other), 0.5);
}

#[test]
fn repeated_disabled_flag_is_a_noop_and_invalid_delay_does_not_change_flags() {
    let (mut store, _, owner) = fixture(0);
    store.play_transition(owner, "t0", 1);
    assert!(!store.set_transition_auto_play(owner, "t0", false).unwrap());
    assert!(store.transition_state(owner, "t0").unwrap().0);
    assert!(
        store
            .set_transition_auto_play_with_options(owner, "t0", true, 2, f32::NAN)
            .is_err()
    );
    assert!(
        !store.objects[owner]
            .control
            .component()
            .unwrap()
            .transitions[0]
            .auto_play
    );
}

#[test]
fn display_and_display2_are_applied_atomically_before_stage_notifications() {
    use crate::widget::{Controller, Gear, GearKind};
    let (mut store, stage, owner) = fixture(0);
    store.objects[stage]
        .control
        .component_mut()
        .unwrap()
        .controllers
        .push(Controller {
            name: "state".into(),
            page_ids: vec!["a".into(), "b".into()],
            page_names: vec!["A".into(), "B".into()],
            selected_index: 0,
            ..Default::default()
        });
    let mut first = Gear::new(GearKind::Display);
    first.controller_index = 0;
    first.pages.push("a".into());
    let mut second = Gear::new(GearKind::Display2);
    second.controller_index = 0;
    second.pages.push("b".into());
    second.condition = 1;
    store.objects[owner].gears.extend([first, second]);
    store.initialize_controllers(stage);
    store.set_transition_auto_play(owner, "t0", true).unwrap();
    store.attach_to_stage(stage);
    store.advance_transitions(0.25);
    store.set_controller_page(stage, "state", "B");
    assert!(store.is_on_stage(owner));
    close(store.transition_state(owner, "t0").unwrap().1, 0.25);
    close(alpha(&store, owner), 0.25);
}

#[test]
fn gear_tween_display_lock_delays_stage_removal_until_the_last_lock_is_released() {
    use crate::widget::{Controller, Gear, GearKind, GearTweenConfig, GearValue};
    let (mut store, stage, owner) = fixture(0);
    store.objects[stage]
        .control
        .component_mut()
        .unwrap()
        .controllers
        .push(Controller {
            name: "state".into(),
            page_ids: vec!["a".into(), "b".into()],
            page_names: vec!["A".into(), "B".into()],
            selected_index: 0,
            ..Default::default()
        });
    let mut display = Gear::new(GearKind::Display);
    display.controller_index = 0;
    display.pages.push("a".into());
    let mut xy = Gear::new(GearKind::Xy);
    xy.controller_index = 0;
    xy.tween = Some(GearTweenConfig {
        duration: 1.0,
        delay: 0.0,
        ease_type: 0,
    });
    xy.values.insert(
        Some("a".into()),
        GearValue::Xy {
            x: 0.0,
            y: 0.0,
            px: 0.0,
            py: 0.0,
        },
    );
    xy.values.insert(
        Some("b".into()),
        GearValue::Xy {
            x: 100.0,
            y: 0.0,
            px: 0.0,
            py: 0.0,
        },
    );
    store.objects[owner].gears.extend([display, xy]);
    store.initialize_controllers(stage);
    store.set_transition_auto_play(owner, "t0", true).unwrap();
    store.attach_to_stage(stage);
    store.advance_transitions(0.25);
    store.set_controller_page(stage, "state", "B");
    assert!(store.is_on_stage(owner));
    close(store.transition_state(owner, "t0").unwrap().1, 0.25);
    store.advance_gear_tweens(1.0);
    assert!(!store.is_on_stage(owner));
    assert!(!store.transition_state(owner, "t0").unwrap().0);
    close(alpha(&store, owner), 0.25);
}

#[test]
fn transition_locks_connect_children_after_nested_calls_and_stay_held_across_repeats() {
    let mut store = ObjectStore::new();
    let owner = store.create(ObjectType::Component);
    let child = store.create(ObjectType::Component);
    store.set_parent(child, Some(owner));
    attach(&mut store, child, "t0", vec![fade(0.5)]);
    store
        .set_transition_auto_play_with_options(child, "t0", true, 1, 10.0)
        .unwrap();
    attach(
        &mut store,
        owner,
        "t0",
        vec![nested(0.0, 0, "t0", 1), nested(1.0, 0, "absent", 1)],
    );
    store.objects[owner]
        .control
        .component_mut()
        .unwrap()
        .transitions[0]
        .options = 1;
    store.apply_display_gear(child, crate::widget::GearKind::Display, 0, false, 0);
    store.attach_to_stage(owner);
    assert!(!store.is_on_stage(child));
    store.play_transition(owner, "t0", 2);
    assert!(store.is_on_stage(child));
    store.advance_transitions(0.25);
    close(alpha(&store, child), 0.5);
    store.advance_transitions(0.75);
    // Looping must retain the same locks and start the nested call immediately.
    assert!(store.is_on_stage(child));
    store.advance_transitions(0.25);
    close(alpha(&store, child), 0.5);
    store.advance_transitions(0.75);
    assert!(!store.is_on_stage(child));
    assert!(!store.transition_state(owner, "t0").unwrap().0);
}

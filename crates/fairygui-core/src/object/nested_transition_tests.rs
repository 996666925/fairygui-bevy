use super::*;
use crate::transition::{ActionType, TransitionItem, TransitionValue, TweenConfig};

pub(super) fn nested(time: f32, target_index: i16, name: &str, times: i32) -> TransitionItem {
    TransitionItem {
        action: ActionType::Transition,
        time,
        target_index,
        label: None,
        value: TransitionValue::Transition {
            name: Some(name.into()),
            times,
        },
        tween: None,
    }
}

pub(super) fn fade(duration: f32) -> TransitionItem {
    TransitionItem {
        action: ActionType::Alpha,
        time: 0.0,
        target_index: -1,
        label: Some("begin".into()),
        value: TransitionValue::None,
        tween: Some(TweenConfig {
            duration,
            ease_type: 0,
            repeat: 0,
            yoyo: false,
            end_label: Some("end".into()),
            path: None,
            start_value: TransitionValue::Scalar(0.0),
            end_value: TransitionValue::Scalar(1.0),
        }),
    }
}

pub(super) fn attach(
    store: &mut ObjectStore,
    owner: GObjectId,
    name: &str,
    items: Vec<TransitionItem>,
) {
    let mut transition = Transition::default();
    transition.name = name.into();
    transition.total_duration = items
        .iter()
        .map(TransitionItem::end_time)
        .fold(0.0, f32::max);
    transition.items = items;
    store.objects[owner]
        .control
        .component_mut()
        .unwrap()
        .transitions
        .push(transition);
}

pub(super) fn close(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 0.0001, "{actual} != {expected}");
}
pub(super) fn alpha(store: &ObjectStore, owner: GObjectId) -> f32 {
    store.objects[owner].alpha()
}

#[test]
fn same_owner_call_works_with_callee_before_caller_and_is_advanced_once() {
    let mut store = ObjectStore::new();
    let owner = store.create(ObjectType::Component);
    attach(&mut store, owner, "child", vec![fade(1.0)]);
    attach(
        &mut store,
        owner,
        "parent",
        vec![nested(0.2, -1, "child", 1)],
    );
    store.play_transition(owner, "parent", 1);
    store.advance_transitions(0.5);
    close(alpha(&store, owner), 0.3);
    close(store.transition_state(owner, "child").unwrap().1, 0.3);
    assert!(store.transition_state(owner, "parent").unwrap().0);
    store.advance_transitions(0.7);
    assert!(!store.transition_state(owner, "parent").unwrap().0);
    assert!(!store.transition_state(owner, "child").unwrap().0);
    close(store.transition_state(owner, "parent").unwrap().1, 1.2);
    assert_eq!(
        store
            .take_transition_hooks()
            .iter()
            .filter(|hook| hook.label == "end")
            .count(),
        1
    );
}

#[test]
fn nested_component_order_does_not_change_the_start_time() {
    for child_first in [false, true] {
        let mut store = ObjectStore::new();
        let (owner, child) = if child_first {
            let child = store.create(ObjectType::Component);
            (store.create(ObjectType::Component), child)
        } else {
            (
                store.create(ObjectType::Component),
                store.create(ObjectType::Component),
            )
        };
        store.set_parent(child, Some(owner));
        attach(&mut store, child, "child", vec![fade(1.0)]);
        attach(
            &mut store,
            owner,
            "parent",
            vec![nested(0.25, 0, "child", 1)],
        );
        store.play_transition(owner, "parent", 1);
        store.advance_transitions(0.75);
        close(alpha(&store, child), 0.5);
        close(store.transition_state(child, "child").unwrap().1, 0.5);
    }
}

#[test]
fn multiple_levels_and_parent_loops_retain_long_frame_remainder() {
    let mut store = ObjectStore::new();
    let owner = store.create(ObjectType::Component);
    attach(&mut store, owner, "leaf", vec![fade(1.0)]);
    attach(
        &mut store,
        owner,
        "middle",
        vec![nested(0.25, -1, "leaf", 1)],
    );
    attach(
        &mut store,
        owner,
        "parent",
        vec![nested(0.25, -1, "middle", 1)],
    );
    store.play_transition(owner, "parent", 3);
    store.advance_transitions(2.5);
    close(store.transition_state(owner, "parent").unwrap().1, 1.0);
    close(store.transition_state(owner, "middle").unwrap().1, 0.75);
    close(store.transition_state(owner, "leaf").unwrap().1, 0.5);
    close(alpha(&store, owner), 0.5);
    store.advance_transitions(2.0);
    assert!(!store.transition_state(owner, "parent").unwrap().0);
    assert_eq!(
        store
            .take_transition_hooks()
            .iter()
            .filter(|hook| hook.label == "end")
            .count(),
        3
    );
}

#[test]
fn stop_marker_clips_interpolation_without_firing_the_end_hook() {
    let mut store = ObjectStore::new();
    let owner = store.create(ObjectType::Component);
    attach(&mut store, owner, "child", vec![fade(2.0)]);
    attach(
        &mut store,
        owner,
        "parent",
        vec![nested(0.1, -1, "child", 1), nested(0.6, -1, "child", 0)],
    );
    store.play_transition(owner, "parent", 1);
    store.advance_transitions(1.0);
    close(alpha(&store, owner), 0.25);
    close(store.transition_state(owner, "child").unwrap().1, 0.5);
    assert!(!store.transition_state(owner, "parent").unwrap().0);
    assert!(
        !store
            .take_transition_hooks()
            .iter()
            .any(|hook| hook.label == "end")
    );
}

#[test]
fn unpaired_stop_marker_plays_only_the_zero_length_start_interval() {
    let mut store = ObjectStore::new();
    let owner = store.create(ObjectType::Component);
    attach(&mut store, owner, "child", vec![fade(2.0)]);
    attach(
        &mut store,
        owner,
        "parent",
        vec![nested(0.3, -1, "child", 0)],
    );
    store.play_transition(owner, "child", 1);
    store.play_transition(owner, "parent", 1);
    store.advance_transitions(1.0);
    close(alpha(&store, owner), 0.0);
    assert_eq!(store.transition_state(owner, "child"), Some((false, 0.0)));
    assert!(!store.transition_state(owner, "parent").unwrap().0);
}

#[test]
fn parent_waits_for_all_requested_child_repeats_even_with_a_clipped_interval() {
    let mut store = ObjectStore::new();
    let owner = store.create(ObjectType::Component);
    attach(&mut store, owner, "child", vec![fade(2.0)]);
    attach(
        &mut store,
        owner,
        "parent",
        vec![nested(0.1, -1, "child", 3), nested(0.6, -1, "child", 0)],
    );
    store.play_transition(owner, "parent", 1);
    store.advance_transitions(0.9);
    assert!(store.transition_state(owner, "parent").unwrap().0);
    close(alpha(&store, owner), 0.15);
    store.advance_transitions(0.7);
    assert!(!store.transition_state(owner, "parent").unwrap().0);
    close(alpha(&store, owner), 0.25);
}

#[test]
fn parent_stop_propagates_completion_or_cancellation_to_children() {
    for complete in [false, true] {
        let mut store = ObjectStore::new();
        let owner = store.create(ObjectType::Component);
        attach(&mut store, owner, "child", vec![fade(1.0)]);
        attach(
            &mut store,
            owner,
            "parent",
            vec![nested(0.2, -1, "child", 1)],
        );
        store.play_transition(owner, "parent", 1);
        store.advance_transitions(0.5);
        store.stop_transition_with_completion(owner, "parent", complete);
        close(alpha(&store, owner), if complete { 1.0 } else { 0.3 });
        assert!(!store.transition_state(owner, "child").unwrap().0);
        store.advance_transitions(5.0);
        close(alpha(&store, owner), if complete { 1.0 } else { 0.3 });
    }
}

#[test]
fn cancellation_during_parent_delay_stops_its_resolved_child_reference() {
    let mut store = ObjectStore::new();
    let owner = store.create(ObjectType::Component);
    attach(&mut store, owner, "child", vec![fade(1.0)]);
    attach(
        &mut store,
        owner,
        "parent",
        vec![nested(0.0, -1, "child", 1)],
    );
    store.play_transition(owner, "child", 1);
    store.play_transition_delayed(owner, "parent", 1, 1.0);
    store.advance_transitions(0.25);
    close(alpha(&store, owner), 0.25);
    store.stop_transition_with_completion(owner, "parent", false);
    assert!(!store.transition_state(owner, "child").unwrap().0);
    close(alpha(&store, owner), 0.25);
}

#[test]
fn replay_interrupt_notifies_previous_parent_at_the_correct_frame_time() {
    let mut store = ObjectStore::new();
    let owner = store.create(ObjectType::Component);
    attach(&mut store, owner, "child", vec![fade(1.0)]);
    attach(
        &mut store,
        owner,
        "first",
        vec![nested(0.0, -1, "child", 1)],
    );
    attach(
        &mut store,
        owner,
        "second",
        vec![nested(0.5, -1, "child", 1)],
    );
    store.play_transition(owner, "first", 1);
    store.play_transition(owner, "second", 1);
    store.advance_transitions(1.0);
    assert!(!store.transition_state(owner, "first").unwrap().0);
    close(store.transition_state(owner, "first").unwrap().1, 0.5);
    assert!(store.transition_state(owner, "second").unwrap().0);
    close(alpha(&store, owner), 0.5);
    store.advance_transitions(0.5);
    assert!(!store.transition_state(owner, "second").unwrap().0);
}

#[test]
fn explicit_child_stop_does_not_invent_a_completion_callback_for_the_parent() {
    let mut store = ObjectStore::new();
    let owner = store.create(ObjectType::Component);
    attach(&mut store, owner, "child", vec![fade(1.0)]);
    attach(
        &mut store,
        owner,
        "parent",
        vec![nested(0.0, -1, "child", 1)],
    );
    store.play_transition(owner, "parent", 1);
    store.advance_transitions(0.25);
    store.stop_transition(owner, "child");
    store.advance_transitions(2.0);
    // Native stop defaults processCallback=false. The parent remains waiting.
    assert!(store.transition_state(owner, "parent").unwrap().0);
    store.play_transition(owner, "parent", 1);
    store.advance_transitions(0.25);
    store.stop_transition_with_options(owner, "child", false, true);
    assert!(!store.transition_state(owner, "parent").unwrap().0);
}

#[test]
fn recursive_definitions_do_not_recurse_forever_and_other_items_still_run() {
    for indirect in [false, true] {
        let mut store = ObjectStore::new();
        let owner = store.create(ObjectType::Component);
        attach(
            &mut store,
            owner,
            "parent",
            vec![
                nested(0.0, -1, if indirect { "child" } else { "parent" }, 1),
                fade(1.0),
            ],
        );
        if indirect {
            attach(
                &mut store,
                owner,
                "child",
                vec![nested(0.0, -1, "parent", 1)],
            );
        }
        store.play_transition(owner, "parent", 1);
        store.advance_transitions(1.0);
        assert!(!store.transition_state(owner, "parent").unwrap().0);
        close(alpha(&store, owner), 1.0);
    }
}

#[test]
fn scoped_auto_play_does_not_restart_another_existing_root() {
    let mut store = ObjectStore::new();
    let first = store.create(ObjectType::Component);
    let second = store.create(ObjectType::Component);
    for owner in [first, second] {
        attach(&mut store, owner, "auto", vec![fade(1.0)]);
        store.objects[owner]
            .control
            .component_mut()
            .unwrap()
            .transitions[0]
            .auto_play = true;
        store.objects[owner]
            .control
            .component_mut()
            .unwrap()
            .transitions[0]
            .auto_play_times = 1;
    }
    store.start_auto_transitions_in(first);
    store.advance_transitions(0.3);
    store.start_auto_transitions_in(second);
    close(store.transition_state(first, "auto").unwrap().1, 0.3);
    close(alpha(&store, first), 0.3);
    close(alpha(&store, second), 0.0);
}

#[test]
fn nested_stop_releases_child_shake_and_display_locks() {
    let mut store = ObjectStore::new();
    let owner = store.create(ObjectType::Component);
    let child = store.create(ObjectType::Component);
    store.set_parent(child, Some(owner));
    let sprite = store.create(ObjectType::Image);
    store.set_parent(sprite, Some(child));
    let mut shake = nested(0.0, 0, "unused", 1);
    shake.action = ActionType::Shake;
    shake.value = TransitionValue::Shake {
        amplitude: 16.0,
        duration: 1.0,
    };
    attach(&mut store, child, "child", vec![shake]);
    store.objects[child]
        .control
        .component_mut()
        .unwrap()
        .transitions[0]
        .options = 1;
    store.apply_display_gear(sprite, crate::widget::GearKind::Display, 0, false, 0);
    attach(
        &mut store,
        owner,
        "parent",
        vec![nested(0.0, 0, "child", 1)],
    );
    store.play_transition(owner, "parent", 1);
    store.advance_transitions(0.5);
    assert!(store.display_visible(sprite));
    assert_ne!(store.position_of(sprite).unwrap(), Point::default());
    store.stop_transition_with_completion(owner, "parent", false);
    assert_eq!(store.position_of(sprite).unwrap(), Point::default());
    assert!(!store.display_visible(sprite));
}

#[test]
fn breakpoint_is_forwarded_to_grandchildren_and_complete_stop_respects_it() {
    let mut store = ObjectStore::new();
    let owner = store.create(ObjectType::Component);
    attach(&mut store, owner, "leaf", vec![fade(1.0)]);
    attach(
        &mut store,
        owner,
        "middle",
        vec![nested(0.25, -1, "leaf", 1)],
    );
    attach(
        &mut store,
        owner,
        "parent",
        vec![nested(0.0, -1, "middle", 1), nested(0.5, -1, "middle", 0)],
    );
    store.play_transition(owner, "parent", 1);
    store.advance_transitions(0.4);
    close(alpha(&store, owner), 0.15);
    store.stop_transition(owner, "parent");
    close(alpha(&store, owner), 0.25);
    assert!(
        !store
            .take_transition_hooks()
            .iter()
            .any(|hook| hook.label == "end")
    );
}

#[test]
fn zero_duration_nested_loop_yields_once_per_tick() {
    let mut store = ObjectStore::new();
    let owner = store.create(ObjectType::Component);
    let mut marker = nested(0.0, -1, "unused", 1);
    marker.action = ActionType::Visible;
    marker.value = TransitionValue::Visible(true);
    marker.label = Some("zero".into());
    attach(&mut store, owner, "child", vec![marker]);
    attach(
        &mut store,
        owner,
        "parent",
        vec![nested(0.0, -1, "child", -1)],
    );
    store.play_transition(owner, "parent", 1);
    assert_eq!(store.take_transition_hooks().len(), 1);
    for _ in 0..3 {
        store.advance_transitions(1.0);
        assert_eq!(store.take_transition_hooks().len(), 1);
    }
    store.stop_transition_with_completion(owner, "parent", false);
    assert!(!store.transition_state(owner, "child").unwrap().0);
}

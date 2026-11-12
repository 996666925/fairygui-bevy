use super::tests::{alpha, attach, close, fade, nested};
use super::*;
use crate::transition::{ActionType, GPath, PathPoint, TransitionItem, TransitionValue};

fn fixture(items: Vec<TransitionItem>) -> (ObjectStore, GObjectId) {
    let mut store = ObjectStore::new();
    let owner = store.create(ObjectType::Component);
    attach(&mut store, owner, "t0", items);
    (store, owner)
}

#[test]
fn pause_retains_progress_and_hooks_and_resumes_without_restarting() {
    let (mut store, owner) = fixture(vec![fade(1.0)]);
    store.play_transition(owner, "t0", 1);
    store.advance_transitions(0.25);
    store.take_transition_hooks();
    assert!(store.set_transition_paused(owner, "t0", true));
    store.advance_transitions(5.0);
    assert_eq!(store.transition_state(owner, "t0"), Some((true, 0.25)));
    close(alpha(&store, owner), 0.25);
    assert!(store.take_transition_hooks().is_empty());
    store.set_transition_paused(owner, "t0", false);
    store.advance_transitions(0.25);
    close(alpha(&store, owner), 0.5);
}

#[test]
fn startup_delay_uses_wall_time_and_pausing_freezes_the_remaining_delay() {
    let (mut store, owner) = fixture(vec![fade(1.0)]);
    store.set_alpha(owner, 0.3);
    store.set_transition_time_scale(owner, "t0", 2.0);
    store.play_transition_delayed(owner, "t0", 1, 1.0);
    store.advance_transitions(0.5);
    close(alpha(&store, owner), 0.3);
    store.set_transition_paused(owner, "t0", true);
    store.advance_transitions(1.0);
    assert_eq!(store.transition_state(owner, "t0"), Some((true, 0.0)));
    store.set_transition_paused(owner, "t0", false);
    store.advance_transitions(0.75);
    close(alpha(&store, owner), 0.5);
}

#[test]
fn speed_changes_preserve_progress_zero_freezes_and_speed_survives_replay() {
    let (mut store, owner) = fixture(vec![fade(1.0)]);
    store.play_transition(owner, "t0", 1);
    store.advance_transitions(0.25);
    store.set_transition_time_scale(owner, "t0", 2.0);
    store.advance_transitions(0.25);
    close(alpha(&store, owner), 0.75);
    store.set_transition_time_scale(owner, "t0", 0.0);
    store.advance_transitions(10.0);
    close(alpha(&store, owner), 0.75);
    assert!(!store.set_transition_time_scale(owner, "t0", f32::NAN));
    assert!(!store.set_transition_time_scale(owner, "t0", -1.0));
    store.set_transition_time_scale(owner, "t0", 0.5);
    store.advance_transitions(0.5);
    assert!(!store.transition_state(owner, "t0").unwrap().0);
    store.play_transition(owner, "t0", 1);
    store.advance_transitions(0.5);
    close(alpha(&store, owner), 0.25);
}

#[test]
fn nested_speed_is_inherited_once_and_changes_propagate_to_active_descendants() {
    let (mut store, owner) = fixture(vec![nested(0.5, -1, "middle", 1)]);
    attach(
        &mut store,
        owner,
        "middle",
        vec![nested(0.0, -1, "leaf", 1)],
    );
    attach(&mut store, owner, "leaf", vec![fade(1.0)]);
    store.set_transition_time_scale(owner, "t0", 2.0);
    store.play_transition(owner, "t0", 1);
    store.advance_transitions(0.5);
    close(alpha(&store, owner), 0.5);
    assert_eq!(store.transition_time_scale(owner, "leaf"), Some(2.0));
    store.set_transition_time_scale(owner, "t0", 0.5);
    assert_eq!(store.transition_time_scale(owner, "middle"), Some(0.5));
    assert_eq!(store.transition_time_scale(owner, "leaf"), Some(0.5));
    store.advance_transitions(1.0);
    close(alpha(&store, owner), 1.0);
    assert!(!store.transition_state(owner, "t0").unwrap().0);
}

#[test]
fn pausing_parent_freezes_nested_time_and_movie_playing_flags_are_restored() {
    let (mut store, owner) = fixture(vec![nested(0.0, -1, "child", 1)]);
    let movie = store.create(ObjectType::MovieClip);
    store.set_parent(movie, Some(owner));
    let loader = store.create(ObjectType::Loader);
    store.set_parent(loader, Some(owner));
    let mut animation = nested(0.0, 0, "unused", 1);
    animation.action = ActionType::Animation;
    animation.value = TransitionValue::Animation {
        playing: true,
        frame: -1,
    };
    let mut stopped = animation.clone();
    stopped.target_index = 1;
    stopped.value = TransitionValue::Animation {
        playing: false,
        frame: -1,
    };
    attach(
        &mut store,
        owner,
        "child",
        vec![fade(1.0), animation, stopped],
    );
    store.play_transition(owner, "t0", 1);
    store.advance_transitions(0.25);
    store.set_transition_paused(owner, "t0", true);
    store.advance_transitions(1.0);
    assert_eq!(store.transition_paused(owner, "child"), Some(true));
    close(store.transition_state(owner, "child").unwrap().1, 0.25);
    assert!(!store.animation_state(movie).unwrap().0);
    store.set_transition_paused(owner, "t0", false);
    assert!(store.animation_state(movie).unwrap().0);
    assert!(!store.animation_state(loader).unwrap().0);
    store.advance_transitions(0.25);
    close(alpha(&store, owner), 0.5);
}

#[test]
fn pending_animation_receives_speed_when_its_action_fires() {
    let mut animation = nested(0.5, 0, "unused", 1);
    animation.action = ActionType::Animation;
    animation.value = TransitionValue::Animation {
        playing: true,
        frame: 2,
    };
    let (mut store, owner) = fixture(vec![fade(1.0), animation]);
    let movie = store.create(ObjectType::MovieClip);
    store.set_parent(movie, Some(owner));
    store.set_transition_time_scale(owner, "t0", 2.0);
    store.play_transition(owner, "t0", 1);
    assert_eq!(store.animation_time_scale(movie), Some(1.0));
    store.advance_transitions(0.25);
    assert_eq!(store.animation_state(movie), Some((true, 2)));
    assert_eq!(store.animation_time_scale(movie), Some(2.0));
    store.set_transition_time_scale(owner, "t0", 0.5);
    assert_eq!(store.animation_time_scale(movie), Some(0.5));
}

#[test]
fn speed_change_during_startup_delay_reaches_resolved_targets_before_item_timers_exist() {
    let mut animation = nested(0.5, 0, "unused", 1);
    animation.action = ActionType::Animation;
    animation.value = TransitionValue::Animation {
        playing: true,
        frame: -1,
    };
    let (mut store, owner) = fixture(vec![nested(0.5, -1, "child", 1), animation]);
    let movie = store.create(ObjectType::MovieClip);
    store.set_parent(movie, Some(owner));
    attach(&mut store, owner, "child", vec![fade(1.0)]);
    store.play_transition(owner, "child", 1);
    store.play_transition_delayed(owner, "t0", 1, 1.0);
    store.set_transition_time_scale(owner, "t0", 2.0);
    assert_eq!(store.transition_time_scale(owner, "child"), Some(2.0));
    assert_eq!(store.animation_time_scale(movie), Some(2.0));
    store.advance_transitions(0.25);
    close(alpha(&store, owner), 0.5);
    assert_eq!(store.transition_state(owner, "t0"), Some((true, 0.0)));
}

#[test]
fn independently_resumed_child_can_complete_a_waiting_paused_parent() {
    let (mut store, owner) = fixture(vec![nested(0.0, -1, "child", 1)]);
    attach(&mut store, owner, "child", vec![fade(0.5)]);
    store.play_transition(owner, "t0", 1);
    store.set_transition_paused(owner, "t0", true);
    store.set_transition_paused(owner, "child", false);
    store.advance_transitions(0.5);
    assert!(!store.transition_state(owner, "t0").unwrap().0);
}

#[test]
fn reverse_schedules_items_backwards_and_swaps_tween_endpoints() {
    let mut turn = fade(0.5);
    turn.time = 1.0;
    turn.action = ActionType::Rotation;
    turn.label = Some("turn".into());
    turn.tween.as_mut().unwrap().start_value = TransitionValue::Scalar(0.0);
    turn.tween.as_mut().unwrap().end_value = TransitionValue::Scalar(90.0);
    turn.tween.as_mut().unwrap().end_label = Some("turned".into());
    let (mut store, owner) = fixture(vec![fade(1.0), turn]);
    store.play_transition_reverse(owner, "t0", 0.0);
    assert_eq!(store.transition_reversed(owner, "t0"), Some(true));
    close(store.objects[owner].rotation(), 90.0);
    store.advance_transitions(0.25);
    close(store.objects[owner].rotation(), 45.0);
    store.advance_transitions(0.5);
    close(alpha(&store, owner), 0.75);
    store.advance_transitions(0.75);
    close(alpha(&store, owner), 0.0);
    close(store.objects[owner].rotation(), 0.0);
    let labels: Vec<_> = store
        .take_transition_hooks()
        .into_iter()
        .map(|hook| hook.label)
        .collect();
    assert_eq!(labels, ["turn", "turned", "begin", "end"]);
    assert!(!store.transition_state(owner, "t0").unwrap().0);
}

#[test]
fn reverse_single_delayed_discrete_item_finishes_immediately() {
    let mut visible = nested(1.0, -1, "unused", 1);
    visible.action = ActionType::Visible;
    visible.value = TransitionValue::Visible(false);
    let (mut store, owner) = fixture(vec![visible]);
    store.play_transition_reverse(owner, "t0", 0.0);
    assert!(!store.objects[owner].visible());
    assert!(!store.transition_state(owner, "t0").unwrap().0);
}

#[test]
fn reverse_path_retains_native_shape_relative_to_swapped_start() {
    let mut xy = fade(1.0);
    xy.action = ActionType::Xy;
    xy.target_index = 0;
    let config = xy.tween.as_mut().unwrap();
    config.start_value = TransitionValue::Pair {
        use_x: true,
        use_y: true,
        x: 10.0,
        y: 20.0,
    };
    config.end_value = TransitionValue::Pair {
        use_x: true,
        use_y: true,
        x: 100.0,
        y: 200.0,
    };
    config.path = Some(GPath::new(&[
        PathPoint::straight(0.0, 0.0),
        PathPoint::straight(50.0, 10.0),
    ]));
    let (mut store, owner) = fixture(vec![xy]);
    let child = store.create(ObjectType::Image);
    store.set_parent(child, Some(owner));
    store.play_transition_reverse(owner, "t0", 0.0);
    store.advance_transitions(0.5);
    assert_eq!(store.position_of(child), Some(Point::new(125.0, 205.0)));
}

#[test]
fn reverse_nested_call_inherits_direction_and_waits_for_its_child() {
    let mut marker = nested(2.0, -1, "unused", 1);
    marker.action = ActionType::Visible;
    marker.value = TransitionValue::Visible(true);
    let (mut store, owner) = fixture(vec![nested(0.0, -1, "child", 1), marker]);
    attach(&mut store, owner, "child", vec![fade(1.0)]);
    store.play_transition_reverse(owner, "t0", 0.0);
    store.advance_transitions(2.25);
    assert_eq!(store.transition_reversed(owner, "child"), Some(true));
    close(alpha(&store, owner), 0.75);
    assert!(store.transition_state(owner, "t0").unwrap().0);
    store.advance_transitions(0.75);
    close(alpha(&store, owner), 0.0);
    assert!(!store.transition_state(owner, "t0").unwrap().0);
}

#[test]
fn reverse_retains_yoyo_and_can_pause_at_a_changed_speed() {
    let mut item = fade(1.0);
    item.tween.as_mut().unwrap().repeat = 1;
    item.tween.as_mut().unwrap().yoyo = true;
    let (mut store, owner) = fixture(vec![item]);
    store.set_transition_time_scale(owner, "t0", 2.0);
    store.play_transition_reverse(owner, "t0", 0.0);
    store.advance_transitions(0.125);
    close(alpha(&store, owner), 0.75);
    store.set_transition_paused(owner, "t0", true);
    store.advance_transitions(1.0);
    close(alpha(&store, owner), 0.75);
    store.set_transition_paused(owner, "t0", false);
    store.advance_transitions(0.5);
    close(alpha(&store, owner), 0.25);
    store.advance_transitions(0.375);
    close(alpha(&store, owner), 1.0);
    assert!(!store.transition_state(owner, "t0").unwrap().0);
}

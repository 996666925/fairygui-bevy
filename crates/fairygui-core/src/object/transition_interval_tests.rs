use super::tests::{alpha, attach, close, fade, nested};
use super::*;
use crate::transition::{ActionType, TransitionItem, TransitionPlayOptions, TransitionValue};

fn fixture(items: Vec<TransitionItem>) -> (ObjectStore, GObjectId) {
    let mut store = ObjectStore::new();
    let owner = store.create(ObjectType::Component);
    attach(&mut store, owner, "t0", items);
    (store, owner)
}

fn range(start_time: f32, end_time: Option<f32>) -> TransitionPlayOptions {
    TransitionPlayOptions {
        start_time,
        end_time,
        ..Default::default()
    }
}

fn shake(time: f32, duration: f32) -> TransitionItem {
    cue(
        ActionType::Shake,
        time,
        TransitionValue::Shake {
            amplitude: 16.0,
            duration,
        },
    )
}

#[test]
fn shake_breakpoint_restores_offset_and_never_emits_start_hooks() {
    let (mut store, owner) = fixture(vec![shake(0.0, 1.0)]);
    store.set_xy(owner, 100.0, 200.0);
    store.play_transition_with_options(owner, "t0", range(0.25, Some(0.5)));
    let position = store.object(owner).unwrap().position();
    close((position.x - 100.0).abs(), 9.0);
    store.set_xy(owner, position.x + 30.0, position.y - 20.0);
    store.advance_transitions(0.25);
    assert_eq!(
        store.position_of(owner),
        Some(crate::geom::Point::new(130.0, 180.0))
    );
    assert_eq!(store.transition_state(owner, "t0"), Some((false, 0.5)));
    assert!(store.take_transition_hooks().is_empty());
}

#[test]
fn future_shake_is_created_even_after_range_end_and_negative_breakpoint_is_disabled() {
    let (mut store, owner) = fixture(vec![shake(1.0, 1.0)]);
    store.play_transition_with_options(owner, "t0", range(0.0, Some(0.5)));
    store.advance_transitions(1.5);
    close(store.object(owner).unwrap().x().abs(), 4.0);
    assert!(store.transition_state(owner, "t0").unwrap().0);
    store.advance_transitions(0.5);
    close(store.object(owner).unwrap().x(), 0.0);
    assert_eq!(store.transition_state(owner, "t0"), Some((false, 2.0)));
}

#[test]
fn reverse_shake_breakpoint_uses_original_time_and_can_end_after_selected_end() {
    let (mut store, owner) = fixture(vec![
        shake(0.25, 1.0),
        cue(ActionType::Visible, 2.0, TransitionValue::Visible(true)),
    ]);
    store.play_transition_with_options(
        owner,
        "t0",
        TransitionPlayOptions {
            reversed: true,
            ..range(0.0, Some(0.5))
        },
    );
    store.advance_transitions(0.875);
    close(store.object(owner).unwrap().x().abs(), 12.25);
    store.advance_transitions(0.125);
    close(store.object(owner).unwrap().x(), 0.0);
    assert_eq!(store.transition_state(owner, "t0"), Some((false, 1.0)));
    assert!(
        store
            .take_transition_hooks()
            .iter()
            .all(|hook| hook.label != "at0.25")
    );
}

#[test]
fn reverse_shake_with_original_time_after_end_plays_for_full_duration() {
    let (mut store, owner) = fixture(vec![
        shake(1.25, 1.0),
        cue(ActionType::Visible, 2.5, TransitionValue::Visible(true)),
    ]);
    store.play_transition_with_options(
        owner,
        "t0",
        TransitionPlayOptions {
            reversed: true,
            ..range(0.0, Some(0.5))
        },
    );
    store.advance_transitions(1.0);
    close(store.object(owner).unwrap().x().abs(), 1.0);
    assert!(store.transition_state(owner, "t0").unwrap().0);
    store.advance_transitions(0.25);
    assert_eq!(store.transition_state(owner, "t0"), Some((false, 1.25)));
    close(store.object(owner).unwrap().x(), 0.0);
}

#[test]
fn completed_historical_shake_leaves_no_offset_when_start_is_past_its_breakpoint() {
    let (mut store, owner) = fixture(vec![shake(0.0, 1.0)]);
    store.set_xy(owner, 50.0, 60.0);
    store.play_transition_with_options(owner, "t0", range(0.75, Some(0.25)));
    assert_eq!(
        store.position_of(owner),
        Some(crate::geom::Point::new(50.0, 60.0))
    );
    assert!(!store.transition_state(owner, "t0").unwrap().0);
    assert!(store.take_transition_hooks().is_empty());
}

#[test]
fn clipped_tween_stops_writing_properties_while_later_shake_keeps_transition_alive() {
    let (mut store, owner) = fixture(vec![fade(2.0), shake(1.0, 1.0)]);
    store.play_transition_with_options(owner, "t0", range(0.0, Some(0.5)));
    store.advance_transitions(0.5);
    close(alpha(&store, owner), 0.25);
    store.set_alpha(owner, 0.9);
    store.advance_transitions(1.0);
    assert!(store.transition_state(owner, "t0").unwrap().0);
    close(alpha(&store, owner), 0.9);
    store.stop_transition(owner, "t0");
    close(alpha(&store, owner), 0.9);
    assert!(
        store
            .take_transition_hooks()
            .iter()
            .all(|hook| !hook.at_end)
    );
}

fn cue(action: ActionType, time: f32, value: TransitionValue) -> TransitionItem {
    TransitionItem {
        action,
        time,
        value,
        target_index: -1,
        label: Some(format!("at{time}")),
        tween: None,
    }
}

#[test]
fn interior_start_samples_immediately_and_breakpoint_skips_end_hook() {
    let (mut store, owner) = fixture(vec![fade(2.0)]);
    assert!(store.play_transition_with_options(owner, "t0", range(0.5, Some(1.5))));
    close(alpha(&store, owner), 0.25);
    assert!(store.take_transition_hooks().is_empty());
    store.advance_transitions(2.0);
    close(alpha(&store, owner), 0.75);
    assert_eq!(store.transition_state(owner, "t0"), Some((false, 1.5)));
    assert!(store.take_transition_hooks().is_empty());
}

#[test]
fn historical_end_hooks_fire_but_old_start_hooks_and_sounds_do_not() {
    let sound = |time| {
        cue(
            ActionType::Sound,
            time,
            TransitionValue::Sound {
                sound: Some("beep.wav".into()),
                volume: 1.0,
            },
        )
    };
    let (mut store, owner) = fixture(vec![fade(0.5), sound(0.25), sound(1.0), sound(1.5)]);
    store.play_transition_with_options(owner, "t0", range(1.0, None));
    let hooks = store.take_transition_hooks();
    assert_eq!(
        hooks
            .iter()
            .map(|hook| (hook.label.as_str(), hook.at_end))
            .collect::<Vec<_>>(),
        vec![("end", true), ("at1", false)]
    );
    assert_eq!(store.take_queued_sounds().len(), 1);
    store.advance_transitions(0.5);
    assert_eq!(store.take_queued_sounds().len(), 1);
    assert_eq!(store.take_transition_hooks()[0].label, "at1.5");
}

#[test]
fn historical_properties_use_source_order_and_discrete_cues_precede_end_filter() {
    let (mut store, owner) = fixture(vec![
        cue(ActionType::Alpha, 0.8, TransitionValue::Scalar(0.8)),
        cue(ActionType::Alpha, 0.2, TransitionValue::Scalar(0.2)),
        fade(2.0),
    ]);
    // Both historical discrete assignments run, even though end < start.
    // The tween samples at its breakpoint last.
    store.play_transition_with_options(owner, "t0", range(1.0, Some(0.5)));
    close(alpha(&store, owner), 0.25);
    assert!(!store.transition_state(owner, "t0").unwrap().0);
    let (mut store, owner) = fixture(vec![
        cue(ActionType::Alpha, 0.8, TransitionValue::Scalar(0.8)),
        cue(ActionType::Alpha, 0.2, TransitionValue::Scalar(0.2)),
    ]);
    store.play_transition_with_options(owner, "t0", range(1.0, Some(0.5)));
    close(alpha(&store, owner), 0.2);
}

#[test]
fn delay_defers_initial_seek_and_each_repeat_restarts_at_range_start() {
    let (mut store, owner) = fixture(vec![fade(2.0)]);
    store.set_alpha(owner, 0.9);
    store.play_transition_with_options(
        owner,
        "t0",
        TransitionPlayOptions {
            delay: 0.5,
            times: 2,
            ..range(0.5, Some(1.5))
        },
    );
    close(alpha(&store, owner), 0.9);
    store.advance_transitions(0.75);
    close(alpha(&store, owner), 0.375);
    store.advance_transitions(0.75);
    close(alpha(&store, owner), 0.25);
    assert_eq!(store.transition_state(owner, "t0"), Some((true, 0.5)));
    store.advance_transitions(1.0);
    close(alpha(&store, owner), 0.75);
    assert!(!store.transition_state(owner, "t0").unwrap().0);
}

#[test]
fn nested_ranges_offset_by_original_cue_time_and_stop_marker_bounds_the_child() {
    let (mut store, owner) = fixture(vec![
        nested(0.5, -1, "child", 1),
        nested(1.5, -1, "child", 0),
    ]);
    attach(&mut store, owner, "child", vec![fade(2.0)]);
    store.play_transition_with_options(owner, "t0", range(1.0, Some(2.0)));
    close(alpha(&store, owner), 0.25);
    assert_eq!(store.transition_state(owner, "child"), Some((true, 0.5)));
    store.advance_transitions(0.5);
    close(alpha(&store, owner), 0.5);
    assert_eq!(store.transition_state(owner, "child"), Some((false, 1.0)));
}

#[test]
fn end_before_start_still_waits_for_historical_nested_calls_without_a_child_breakpoint() {
    let (mut store, owner) = fixture(vec![nested(1.0, -1, "child", 1)]);
    attach(&mut store, owner, "child", vec![fade(1.0)]);
    store.play_transition_with_options(owner, "t0", range(1.25, Some(0.5)));
    close(alpha(&store, owner), 0.25);
    assert!(store.transition_state(owner, "t0").unwrap().0);
    store.advance_transitions(0.75);
    close(alpha(&store, owner), 1.0);
    assert!(!store.transition_state(owner, "t0").unwrap().0);
    assert!(!store.transition_state(owner, "child").unwrap().0);
}

#[test]
fn reverse_ranges_sample_swapped_endpoints_and_filter_hooks_by_original_time() {
    let mut early = fade(0.5);
    early.label = Some("original-early".into());
    let mut late = fade(0.5);
    late.time = 1.5;
    late.label = Some("original-late".into());
    let (mut store, owner) = fixture(vec![early, late]);
    store.play_transition_with_options(
        owner,
        "t0",
        TransitionPlayOptions {
            reversed: true,
            ..range(0.25, Some(0.4))
        },
    );
    close(alpha(&store, owner), 0.5);
    assert_eq!(store.take_transition_hooks()[0].label, "original-late");
    store.advance_transitions(1.0);
    close(alpha(&store, owner), 0.2);
    assert!(store.take_transition_hooks().is_empty());
}

#[test]
fn reversed_nested_ranges_offset_by_original_time_instead_of_reverse_schedule() {
    let (mut store, owner) = fixture(vec![
        nested(0.5, -1, "child", 1),
        cue(ActionType::Visible, 2.0, TransitionValue::Visible(true)),
    ]);
    attach(&mut store, owner, "child", vec![fade(2.0)]);
    store.play_transition_with_options(
        owner,
        "t0",
        TransitionPlayOptions {
            reversed: true,
            ..range(1.75, Some(1.9))
        },
    );
    assert_eq!(store.transition_state(owner, "child"), Some((true, 1.25)));
    close(alpha(&store, owner), 0.375);
    store.advance_transitions(0.15);
    close(alpha(&store, owner), 0.3);
    assert!(!store.transition_state(owner, "child").unwrap().0);
}

#[test]
fn owner_relative_ranges_recapture_origin_at_each_repeat() {
    let mut motion = fade(1.0);
    motion.action = ActionType::Xy;
    let pair = |x| TransitionValue::Pair {
        use_x: true,
        use_y: false,
        x,
        y: 0.0,
    };
    motion.tween.as_mut().unwrap().start_value = pair(0.0);
    motion.tween.as_mut().unwrap().end_value = pair(10.0);
    let (mut store, owner) = fixture(vec![motion]);
    store.set_xy(owner, 100.0, 200.0);
    store.play_transition_with_options(
        owner,
        "t0",
        TransitionPlayOptions {
            times: 2,
            ..range(0.25, Some(0.75))
        },
    );
    close(store.object(owner).unwrap().x(), 102.5);
    store.advance_transitions(0.5);
    close(store.object(owner).unwrap().x(), 110.0);
    store.advance_transitions(0.5);
    close(store.object(owner).unwrap().x(), 115.0);
    close(store.object(owner).unwrap().y(), 200.0);
}

fn animation(time: f32, playing: bool, frame: i32) -> TransitionItem {
    let mut item = cue(
        ActionType::Animation,
        time,
        TransitionValue::Animation { playing, frame },
    );
    item.target_index = 0;
    item
}

#[test]
fn movie_catch_up_sums_only_playing_segments_and_does_not_change_speed() {
    let (mut store, owner) = fixture(vec![
        animation(0.25, true, -1),
        animation(0.5, false, -1),
        animation(0.75, true, -1),
        animation(1.0, false, -1),
        cue(ActionType::Visible, 2.0, TransitionValue::Visible(true)),
    ]);
    let movie = store.create(ObjectType::MovieClip);
    store.set_parent(movie, Some(owner));
    store.set_animation_playing(movie, false);
    store.set_animation_time_scale(movie, 0.5);
    store.set_transition_time_scale(owner, "t0", 3.0);
    store.play_transition_with_options(owner, "t0", range(1.0, None));
    assert_eq!(store.animation_state(movie), Some((false, 0)));
    assert_eq!(store.animation_time_scale(movie), Some(0.5));
    assert_eq!(
        store.take_animation_commands(movie),
        vec![
            AnimationCommand::SetFrame(0),
            AnimationCommand::Advance(0.5)
        ]
    );
    assert_eq!(store.take_transition_hooks()[0].label, "at1");
}

#[test]
fn explicit_movie_frame_discards_earlier_catch_up_and_pending_ranges_compose() {
    let (mut store, owner) = fixture(vec![animation(0.0, true, 0), animation(0.5, true, 2)]);
    let movie = store.create(ObjectType::Loader);
    store.set_parent(movie, Some(owner));
    store.play_transition_with_options(owner, "t0", range(1.0, None));
    assert_eq!(
        store.take_animation_commands(movie),
        vec![
            AnimationCommand::SetFrame(2),
            AnimationCommand::Advance(0.5)
        ]
    );
    // Two later seeks without a frame cue must append to the pending result.
    attach(&mut store, owner, "resume", vec![animation(0.0, true, -1)]);
    store.play_transition_with_options(owner, "resume", range(0.25, None));
    store.play_transition_with_options(owner, "resume", range(0.25, None));
    assert_eq!(
        store.take_animation_commands(movie),
        vec![
            AnimationCommand::SetFrame(2),
            AnimationCommand::Advance(0.25),
            AnimationCommand::Advance(0.25)
        ]
    );
}

#[test]
fn reverse_does_not_queue_animation_catch_up() {
    let (mut store, owner) = fixture(vec![animation(0.0, true, -1)]);
    let movie = store.create(ObjectType::MovieClip);
    store.set_parent(movie, Some(owner));
    store.play_transition_with_options(
        owner,
        "t0",
        TransitionPlayOptions {
            reversed: true,
            ..range(1.0, None)
        },
    );
    assert!(store.take_animation_commands(movie).is_empty());
}

#[test]
fn invalid_options_leave_playback_untouched_and_removed_objects_drop_commands() {
    let (mut store, owner) = fixture(vec![fade(2.0)]);
    store.play_transition(owner, "t0", 1);
    store.advance_transitions(0.25);
    for options in [
        range(f32::NAN, None),
        range(0.0, Some(f32::INFINITY)),
        range(-0.5, None),
    ] {
        assert!(!store.play_transition_with_options(owner, "t0", options));
        assert_eq!(store.transition_state(owner, "t0"), Some((true, 0.25)));
    }
    let movie = store.create(ObjectType::MovieClip);
    store.set_parent(movie, Some(owner));
    assert!(store.advance_animation(movie, 0.5));
    assert!(!store.advance_animation(movie, -0.5));
    assert!(!store.advance_animation(owner, 1.0));
    store.remove(owner);
    assert!(store.take_animation_commands(movie).is_empty());
}

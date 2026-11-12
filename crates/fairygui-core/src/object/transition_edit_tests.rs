use super::tests::{alpha, attach, close, fade, nested};
use super::*;
use crate::transition::{
    ActionType, TransitionEditError, TransitionEditValue as Edit, TransitionItem,
    TransitionValue as Value,
};

fn fixture(items: Vec<TransitionItem>) -> (ObjectStore, GObjectId) {
    let mut store = ObjectStore::new();
    let owner = store.create(ObjectType::Component);
    attach(&mut store, owner, "t0", items);
    (store, owner)
}

fn cue(action: ActionType, value: Value) -> TransitionItem {
    TransitionItem {
        action,
        value,
        label: Some("cue".into()),
        time: 0.5,
        target_index: -1,
        tween: None,
    }
}

#[test]
fn edits_update_all_labels_but_hooks_bind_only_first_and_clear_preserves_label_messages() {
    let (mut store, owner) = fixture(vec![fade(1.0), fade(1.0)]);
    assert_eq!(
        store.set_transition_value(owner, "t0", "begin", Edit::Scalar(0.2)),
        Ok(2)
    );
    store
        .set_transition_hook(owner, "t0", "begin", Some(10))
        .unwrap();
    store
        .set_transition_hook(owner, "t0", "end", Some(20))
        .unwrap();
    store.play_transition(owner, "t0", 1);
    close(alpha(&store, owner), 0.2);
    assert_eq!(store.take_transition_hooks().len(), 2);
    assert_eq!(
        store
            .take_transition_callbacks()
            .iter()
            .map(|event| event.listener)
            .collect::<Vec<_>>(),
        vec![10]
    );
    store
        .set_transition_hook(owner, "t0", "end", Some(30))
        .unwrap();
    store.advance_transitions(1.0);
    assert_eq!(store.take_transition_callbacks()[0].listener, 30);
    store.clear_transition_hooks(owner, "t0").unwrap();
    store.play_transition(owner, "t0", 1);
    store.advance_transitions(1.0);
    assert!(store.take_transition_callbacks().is_empty());
    assert!(!store.take_transition_hooks().is_empty());
}

#[test]
fn playing_and_pending_tweens_keep_created_values_and_duration_until_next_pass() {
    for cue_time in [0.0, 0.5] {
        let mut item = fade(1.0);
        item.time = cue_time;
        let (mut store, owner) = fixture(vec![item]);
        store.play_transition(owner, "t0", 2);
        store
            .set_transition_value(owner, "t0", "end", Edit::Scalar(0.5))
            .unwrap();
        store
            .set_transition_duration(owner, "t0", "begin", 2.0)
            .unwrap();
        close(
            store.transition_label_time(owner, "t0", "end").unwrap(),
            cue_time + 2.0,
        );
        store.advance_transitions(cue_time + 0.5);
        close(alpha(&store, owner), 0.5);
        store.advance_transitions(0.5 + cue_time + 1.0);
        close(alpha(&store, owner), 0.25);
    }
}

#[test]
fn startup_delay_defers_parameter_capture_and_duration_does_not_rewrite_nominal_total() {
    let (mut store, owner) = fixture(vec![fade(1.0)]);
    store.play_transition_delayed(owner, "t0", 1, 0.5);
    store
        .set_transition_duration(owner, "t0", "begin", 2.0)
        .unwrap();
    store
        .set_transition_value(owner, "t0", "end", Edit::Scalar(0.5))
        .unwrap();
    store.advance_transitions(1.5);
    close(alpha(&store, owner), 0.25);
    let definition = &store
        .object(owner)
        .unwrap()
        .control()
        .component()
        .unwrap()
        .transitions[0];
    close(definition.total_duration, 1.0);
    store.stop_transition_with_completion(owner, "t0", false);
    store.play_transition_reverse(owner, "t0", 0.0);
    // New duration=2, exported total=1: the reverse task starts at -1.
    close(alpha(&store, owner), 0.25);
}

#[test]
fn discrete_actions_read_live_values_and_partial_edits_preserve_other_fields() {
    let (mut store, owner) = fixture(vec![cue(ActionType::Text, Value::Text(Some("old".into())))]);
    let text = store.create(ObjectType::Text);
    store.set_parent(text, Some(owner));
    store
        .set_transition_target(owner, "t0", "cue", Some(text))
        .unwrap();
    store.play_transition(owner, "t0", 1);
    store
        .set_transition_value(owner, "t0", "cue", Edit::Text(Some("edited".into())))
        .unwrap();
    store.advance_transitions(0.5);
    assert_eq!(
        store
            .object(text)
            .unwrap()
            .control()
            .as_text_field()
            .unwrap()
            .text,
        "edited"
    );
    store
        .set_transition_value(owner, "t0", "cue", Edit::Text(None))
        .unwrap();
    store.play_transition(owner, "t0", 1);
    store.advance_transitions(0.5);
    assert_eq!(
        store
            .object(text)
            .unwrap()
            .control()
            .as_text_field()
            .unwrap()
            .text,
        ""
    );
    let (mut store, owner) = fixture(vec![cue(
        ActionType::Animation,
        Value::Animation {
            frame: 0,
            playing: false,
        },
    )]);
    let movie = store.create(ObjectType::MovieClip);
    store.set_parent(movie, Some(owner));
    store
        .set_transition_target(owner, "t0", "cue", Some(movie))
        .unwrap();
    store
        .set_transition_value(
            owner,
            "t0",
            "cue",
            Edit::Animation {
                frame: 2,
                playing: None,
            },
        )
        .unwrap();
    store.play_transition(owner, "t0", 1);
    store.advance_transitions(0.5);
    assert_eq!(store.animation_state(movie), Some((false, 2)));
}

#[test]
fn retarget_active_tween_keeps_interpolation_and_end_labels_cannot_retarget_or_resize() {
    let (mut store, owner) = fixture(vec![fade(1.0)]);
    let first = store.create(ObjectType::Graph);
    let second = store.create(ObjectType::Graph);
    store.set_parent(first, Some(owner));
    store.set_parent(second, Some(owner));
    store
        .set_transition_target(owner, "t0", "begin", Some(first))
        .unwrap();
    store.play_transition(owner, "t0", 1);
    store.advance_transitions(0.25);
    store
        .set_transition_target(owner, "t0", "begin", Some(second))
        .unwrap();
    store.advance_transitions(0.25);
    close(alpha(&store, first), 0.25);
    close(alpha(&store, second), 0.5);
    assert!(
        store
            .set_transition_target(owner, "t0", "end", None)
            .is_err()
    );
    assert!(
        store
            .set_transition_duration(owner, "t0", "end", 1.0)
            .is_err()
    );
    store
        .set_transition_target(owner, "t0", "begin", None)
        .unwrap();
    store.advance_transitions(0.25);
    close(alpha(&store, owner), 0.75);
}

#[test]
fn nested_name_changes_wait_for_replay_but_play_count_is_live_at_the_cue() {
    let mut call = nested(0.5, -1, "child", 1);
    call.label = Some("call".into());
    let (mut store, owner) = fixture(vec![call]);
    attach(&mut store, owner, "child", vec![fade(1.0)]);
    attach(&mut store, owner, "other", vec![fade(2.0)]);
    store.play_transition(owner, "t0", 1);
    store
        .set_transition_value(
            owner,
            "t0",
            "call",
            Edit::Transition {
                name: Some("other".into()),
                times: Some(2),
            },
        )
        .unwrap();
    store.advance_transitions(1.75);
    assert!(store.transition_state(owner, "child").unwrap().0);
    assert!(!store.transition_state(owner, "other").unwrap().0);
    close(alpha(&store, owner), 0.25);
    store.stop_transition_with_completion(owner, "t0", false);
    store.play_transition(owner, "t0", 1);
    store.advance_transitions(0.75);
    assert!(store.transition_state(owner, "other").unwrap().0);
}

#[test]
fn sound_edit_preserves_the_references_first_played_clip_cache_but_updates_volume() {
    let (mut store, owner) = fixture(vec![cue(
        ActionType::Sound,
        Value::Sound {
            sound: Some("old.wav".into()),
            volume: 1.0,
        },
    )]);
    store.play_transition(owner, "t0", 1);
    store.advance_transitions(0.5);
    store.take_queued_sounds();
    store
        .set_transition_value(
            owner,
            "t0",
            "cue",
            Edit::Sound {
                sound: Some("new.wav".into()),
                volume: Some(0.25),
            },
        )
        .unwrap();
    store.play_transition(owner, "t0", 1);
    store.advance_transitions(0.5);
    let sounds = store.take_queued_sounds();
    assert_eq!((sounds[0].1.as_str(), sounds[0].2), ("old.wav", 0.25));
    let (mut store, owner) = fixture(vec![cue(
        ActionType::Sound,
        Value::Sound {
            sound: Some(String::new()),
            volume: 1.0,
        },
    )]);
    store.play_transition(owner, "t0", 1);
    store.advance_transitions(0.5);
    assert!(store.take_queued_sounds().is_empty());
}

#[test]
fn invalid_edits_are_atomic_and_unknown_labels_do_not_interrupt_playback() {
    let mut visible = cue(ActionType::Visible, Value::Visible(true));
    visible.label = Some("begin".into());
    let (mut store, owner) = fixture(vec![fade(1.0), visible]);
    assert!(matches!(
        store.set_transition_value(owner, "t0", "begin", Edit::Scalar(0.25)),
        Err(TransitionEditError::IncompatibleValue { .. })
    ));
    assert!(
        store
            .set_transition_duration(owner, "t0", "begin", f32::NAN)
            .is_err()
    );
    assert!(
        store
            .set_transition_hook(owner, "t0", "absent", Some(1))
            .is_err()
    );
    assert_eq!(store.transition_label_time(owner, "t0", "absent"), None);
    store.play_transition(owner, "t0", 1);
    store.advance_transitions(0.25);
    close(alpha(&store, owner), 0.25);
    assert!(
        store
            .set_transition_value(owner, "t0", "absent", Edit::Scalar(0.1))
            .is_err()
    );
    assert_eq!(store.transition_state(owner, "t0"), Some((true, 0.25)));
}

fn motion(percent: bool) -> TransitionItem {
    let mut item = fade(1.0);
    item.action = ActionType::Xy;
    item.time = 0.5;
    item.target_index = 0;
    let pair = |x, y| {
        if percent {
            Value::PercentPosition {
                use_x: true,
                use_y: true,
                x,
                y,
            }
        } else {
            Value::Pair {
                use_x: true,
                use_y: true,
                x,
                y,
            }
        }
    };
    item.tween.as_mut().unwrap().start_value = pair(
        if percent { 0.1 } else { 10.0 },
        if percent { 0.2 } else { 20.0 },
    );
    item.tween.as_mut().unwrap().end_value = pair(
        if percent { 0.5 } else { 110.0 },
        if percent { 0.6 } else { 120.0 },
    );
    item
}

#[test]
fn pending_percent_positions_read_live_values_at_start_while_absolute_positions_keep_copies() {
    for percent in [false, true] {
        let (mut store, owner) = fixture(vec![motion(percent)]);
        store.set_size(owner, 200.0, 100.0);
        let target = store.create(ObjectType::Graph);
        store.set_parent(target, Some(owner));
        store.play_transition(owner, "t0", 1);
        store
            .set_transition_value(owner, "t0", "end", Edit::Pair { x: 0.75, y: 0.8 })
            .unwrap();
        store.advance_transitions(1.0);
        let position = store.object(target).unwrap().position();
        close(position.x, if percent { 85.0 } else { 60.0 });
        close(position.y, if percent { 50.0 } else { 70.0 });
    }
}

#[test]
fn relation_layout_shifts_absolute_definitions_and_preserves_percent_endpoints() {
    for percent in [false, true] {
        let (mut store, owner) = fixture(vec![motion(percent)]);
        store.set_size(owner, 100.0, 100.0);
        let target = store.create(ObjectType::Graph);
        store.set_parent(target, Some(owner));
        store.objects[target]
            .relations
            .items
            .push(crate::widget::relation::RelationItem {
                to_parent: true,
                target_index: None,
                defs: vec![crate::widget::relation::RelationDef {
                    relation_type: 6,
                    percent: false,
                    axis: 0,
                }],
            });
        store.set_size(owner, 200.0, 100.0);
        store.play_transition(owner, "t0", 1);
        store.advance_transitions(1.0);
        close(
            store.object(target).unwrap().x(),
            if percent { 60.0 } else { 160.0 },
        );
    }
}

#[test]
fn relation_changes_during_sampling_reach_later_items_before_their_tweener_is_created() {
    let mut resize = cue(
        ActionType::Size,
        Value::Pair {
            use_x: true,
            use_y: true,
            x: 200.0,
            y: 100.0,
        },
    );
    resize.time = 0.0;
    let (mut store, owner) = fixture(vec![resize, motion(false)]);
    store.set_size(owner, 100.0, 100.0);
    let target = store.create(ObjectType::Graph);
    store.set_parent(target, Some(owner));
    store.objects[target]
        .relations
        .items
        .push(crate::widget::relation::RelationItem {
            to_parent: true,
            target_index: None,
            defs: vec![crate::widget::relation::RelationDef {
                relation_type: 6,
                percent: false,
                axis: 0,
            }],
        });
    store.play_transition(owner, "t0", 1);
    store.advance_transitions(1.0);
    close(store.object(target).unwrap().x(), 160.0);
}

#[test]
fn retargeting_a_task_not_created_for_a_foreign_target_does_not_resurrect_it_mid_pass() {
    let (mut store, owner) = fixture(vec![
        fade(1.0),
        cue(ActionType::Visible, Value::Visible(true)),
    ]);
    let other = store.create(ObjectType::Graph);
    store
        .set_transition_target(owner, "t0", "begin", Some(other))
        .unwrap();
    store.play_transition(owner, "t0", 1);
    assert!(store.transition_state(owner, "t0").unwrap().0);
    store.advance_transitions(0.25);
    store.set_parent(other, Some(owner));
    store
        .set_transition_target(owner, "t0", "begin", Some(other))
        .unwrap();
    store.advance_transitions(0.25);
    close(alpha(&store, other), 1.0);
    store.play_transition(owner, "t0", 1);
    store.advance_transitions(0.5);
    close(alpha(&store, other), 0.5);
}

#[test]
fn zero_transition_count_is_one_pass_and_differs_from_movie_clip_loop_counts() {
    let (mut store, owner) = fixture(vec![fade(1.0)]);
    store.play_transition(owner, "t0", 0);
    store.advance_transitions(2.5);
    close(alpha(&store, owner), 1.0);
    assert_eq!(store.transition_state(owner, "t0"), Some((false, 1.0)));
    assert_eq!(
        store
            .take_transition_hooks()
            .iter()
            .filter(|hook| hook.at_end)
            .count(),
        1
    );
}

#[test]
fn shake_edits_preserve_created_amplitude_and_duration_until_the_next_pass() {
    let (mut store, owner) = fixture(vec![cue(
        ActionType::Shake,
        Value::Shake {
            amplitude: 16.0,
            duration: 1.0,
        },
    )]);
    store.play_transition(owner, "t0", 2);
    store
        .set_transition_value(
            owner,
            "t0",
            "cue",
            Edit::Shake {
                amplitude: 64.0,
                duration: Some(2.0),
            },
        )
        .unwrap();
    store.advance_transitions(1.0);
    close(store.object(owner).unwrap().x().abs(), 4.0);
    store.advance_transitions(2.0);
    close(store.object(owner).unwrap().x().abs(), 16.0);
}

#[test]
fn same_item_start_label_takes_precedence_over_its_end_label_and_hook_can_be_removed() {
    let mut item = fade(1.0);
    item.tween.as_mut().unwrap().end_label = Some("begin".into());
    let (mut store, owner) = fixture(vec![item]);
    assert_eq!(
        store.set_transition_value(owner, "t0", "begin", Edit::Scalar(0.25)),
        Ok(1)
    );
    assert_eq!(store.transition_label_time(owner, "t0", "begin"), Some(0.0));
    store
        .set_transition_hook(owner, "t0", "begin", Some(1))
        .unwrap();
    store
        .set_transition_hook(owner, "t0", "begin", None)
        .unwrap();
    store.play_transition(owner, "t0", 1);
    close(alpha(&store, owner), 0.25);
    store.advance_transitions(1.0);
    close(alpha(&store, owner), 1.0);
    assert!(store.take_transition_callbacks().is_empty());
}

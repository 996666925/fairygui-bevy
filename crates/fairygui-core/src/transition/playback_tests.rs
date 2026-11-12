use super::*;
use crate::geom::Point;
use crate::object::ObjectStore;
use crate::package::ObjectType;

fn item(action: ActionType, time: f32, value: TransitionValue) -> TransitionItem {
    TransitionItem {
        action,
        time,
        target_index: -1,
        label: None,
        value,
        tween: None,
    }
}

fn tween(action: ActionType, start: TransitionValue, end: TransitionValue) -> TransitionItem {
    let mut item = item(action, 0.0, TransitionValue::None);
    item.tween = Some(TweenConfig {
        duration: 1.0,
        ease_type: 0,
        repeat: 0,
        yoyo: false,
        end_label: None,
        path: None,
        start_value: start,
        end_value: end,
    });
    item
}

fn pair(use_x: bool, use_y: bool, x: f32, y: f32) -> TransitionValue {
    TransitionValue::Pair { use_x, use_y, x, y }
}

fn fixture(items: Vec<TransitionItem>) -> (ObjectStore, GObjectId, Transition) {
    let mut store = ObjectStore::new();
    let owner = store.create(ObjectType::Component);
    let duration = items
        .iter()
        .map(TransitionItem::end_time)
        .fold(0.0, f32::max);
    let transition = Transition {
        name: "timeline".into(),
        items,
        total_duration: duration,
        ..Default::default()
    };
    (store, owner, transition)
}

fn close(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 0.0001, "{actual} != {expected}");
}

#[test]
fn repeat_and_yoyo_extend_playback_and_end_hook_fires_once() {
    let mut alpha = tween(
        ActionType::Alpha,
        TransitionValue::Scalar(0.0),
        TransitionValue::Scalar(1.0),
    );
    let config = alpha.tween.as_mut().unwrap();
    config.repeat = 1;
    config.yoyo = true;
    config.end_label = Some("finished".into());
    let (mut store, owner, mut transition) = fixture(vec![alpha]);
    transition.play(1, 0.0);
    assert!(transition.advance(&mut store, owner, 1.25));
    close(store.object(owner).unwrap().alpha(), 0.75);
    assert_eq!(transition.time(), 1.25);
    assert!(store.take_transition_hooks().is_empty());
    assert!(!transition.advance(&mut store, owner, 0.75));
    close(store.object(owner).unwrap().alpha(), 0.0);
    let hooks = store.take_transition_hooks();
    assert_eq!(hooks.len(), 1);
    assert_eq!(
        (hooks[0].label.as_str(), hooks[0].at_end),
        ("finished", true)
    );
    transition.advance(&mut store, owner, 1.0);
    assert!(store.take_transition_hooks().is_empty());
}

#[test]
fn end_label_never_jumps_to_another_item_with_the_same_label() {
    let mut alpha = tween(
        ActionType::Alpha,
        TransitionValue::Scalar(0.0),
        TransitionValue::Scalar(1.0),
    );
    alpha.tween.as_mut().unwrap().end_label = Some("marker".into());
    let mut marker = item(ActionType::Visible, 0.25, TransitionValue::Visible(true));
    marker.label = Some("marker".into());
    let (mut store, owner, mut transition) = fixture(vec![alpha, marker]);
    transition.play(1, 0.0);
    assert!(!transition.advance(&mut store, owner, 1.0));
    assert_eq!(transition.time(), 1.0);
    assert_eq!(store.take_transition_hooks().len(), 2);
    assert!(transition.seek_to_label("marker"));
    // Official getLabelTime uses the first matching start/end label in item order.
    assert_eq!(transition.time(), 1.0);
}

#[test]
fn rotation_values_are_degrees() {
    let (mut store, owner, mut transition) = fixture(vec![tween(
        ActionType::Rotation,
        TransitionValue::Scalar(0.0),
        TransitionValue::Scalar(90.0),
    )]);
    transition.play(1, 0.0);
    transition.advance(&mut store, owner, 0.5);
    assert_eq!(store.object(owner).unwrap().rotation(), 45.0);
}

#[test]
fn text_and_icon_items_write_the_correct_widget_properties_and_title_child() {
    let (mut store, owner, mut transition) = fixture(vec![
        item(
            ActionType::Text,
            0.0,
            TransitionValue::Text(Some("New title".into())),
        ),
        item(
            ActionType::Icon,
            0.0,
            TransitionValue::Text(Some("ui://new-icon".into())),
        ),
    ]);
    let button = store.create(ObjectType::Button);
    store.set_parent(button, Some(owner));
    let title = store.create(ObjectType::Text);
    store.set_name(title, "title");
    store.set_parent(title, Some(button));
    let icon = store.create(ObjectType::Loader);
    store.set_name(icon, "icon");
    store.set_parent(icon, Some(button));
    for item in &mut transition.items {
        item.target_index = 0;
    }
    transition.play(1, 0.0);
    transition.advance(&mut store, owner, 0.0);
    let crate::widget::ControlData::Button(data) = store.object(button).unwrap().control() else {
        unreachable!()
    };
    assert_eq!(data.title, "New title");
    assert_eq!(data.icon.as_deref(), Some("ui://new-icon"));
    let crate::widget::ControlData::Loader(loader) = store.object(icon).unwrap().control() else {
        unreachable!()
    };
    assert_eq!(loader.url.as_deref(), Some("ui://new-icon"));
    assert_eq!(
        store
            .object(title)
            .unwrap()
            .control()
            .as_text_field()
            .unwrap()
            .text,
        "New title"
    );
}

#[test]
fn unspecified_tween_start_is_captured_at_start_and_end_uses_that_value() {
    let mut xy = tween(
        ActionType::Xy,
        pair(false, true, 0.0, 70.0),
        pair(true, false, 150.0, 0.0),
    );
    xy.time = 0.5;
    xy.target_index = 0;
    let (mut store, owner, mut transition) = fixture(vec![xy]);
    let child = store.create(ObjectType::Image);
    store.set_parent(child, Some(owner));
    store.set_xy(child, 20.0, 30.0);
    transition.play(1, 0.0);
    transition.advance(&mut store, owner, 0.25);
    store.set_xy(child, 50.0, 60.0);
    transition.advance(&mut store, owner, 0.75);
    assert_eq!(
        store.object(child).unwrap().position(),
        Point::new(100.0, 70.0)
    );
}

#[test]
fn owner_xy_is_relative_and_loop_overflow_is_retained() {
    let (mut store, owner, mut transition) = fixture(vec![tween(
        ActionType::Xy,
        pair(true, true, 0.0, 0.0),
        pair(true, true, 20.0, 10.0),
    )]);
    store.set_xy(owner, 100.0, 200.0);
    transition.play(3, 0.0);
    transition.advance(&mut store, owner, 2.5);
    assert_eq!(
        store.object(owner).unwrap().position(),
        Point::new(150.0, 225.0)
    );
    assert_eq!(transition.time(), 0.5);
    assert!(!transition.advance(&mut store, owner, 0.5));
    assert_eq!(
        store.object(owner).unwrap().position(),
        Point::new(160.0, 230.0)
    );
}

#[test]
fn percentage_endpoints_use_owner_size_and_mixed_absolute_start() {
    let mut xy = tween(
        ActionType::Xy,
        pair(true, true, 25.0, 40.0),
        TransitionValue::PercentPosition {
            use_x: true,
            use_y: true,
            x: 0.5,
            y: 0.8,
        },
    );
    xy.target_index = 0;
    let (mut store, owner, mut transition) = fixture(vec![xy]);
    store.set_size(owner, 200.0, 100.0);
    let child = store.create(ObjectType::Image);
    store.set_parent(child, Some(owner));
    transition.play(1, 0.0);
    transition.advance(&mut store, owner, 0.5);
    assert_eq!(
        store.object(child).unwrap().position(),
        Point::new(62.5, 60.0)
    );
}

#[test]
fn path_offsets_apply_to_captured_start_instead_of_lerping_to_end() {
    let mut xy = tween(
        ActionType::Xy,
        pair(false, false, 0.0, 0.0),
        pair(true, true, 900.0, 800.0),
    );
    xy.target_index = 0;
    xy.tween.as_mut().unwrap().path = Some(GPath::new(&[
        PathPoint::straight(0.0, 0.0),
        PathPoint::straight(100.0, 50.0),
    ]));
    let (mut store, owner, mut transition) = fixture(vec![xy]);
    let child = store.create(ObjectType::Image);
    store.set_parent(child, Some(owner));
    store.set_xy(child, 50.0, 70.0);
    transition.play(1, 0.0);
    transition.advance(&mut store, owner, 0.5);
    assert_eq!(
        store.object(child).unwrap().position(),
        Point::new(100.0, 95.0)
    );
}

#[test]
fn target_binding_survives_reorder_and_does_not_retarget_after_removal() {
    let mut alpha = item(ActionType::Alpha, 0.0, TransitionValue::Scalar(0.2));
    alpha.target_index = 0;
    let (mut store, owner, mut transition) = fixture(vec![alpha]);
    let first = store.create(ObjectType::Image);
    let second = store.create(ObjectType::Image);
    store.set_parent(first, Some(owner));
    store.set_parent(second, Some(owner));
    transition.bind_targets(&store, owner);
    store.add_child_at(owner, second, 0);
    transition.play(1, 0.0);
    transition.advance(&mut store, owner, 0.0);
    close(store.object(first).unwrap().alpha(), 0.2);
    assert_eq!(store.object(second).unwrap().alpha(), 1.0);
    store.remove(first);
    transition.play(1, 0.0);
    transition.advance(&mut store, owner, 1.0);
    assert_eq!(store.object(second).unwrap().alpha(), 1.0);
}

#[test]
fn shake_decays_and_restores_without_removing_external_movement() {
    let (mut store, owner, mut transition) = fixture(vec![item(
        ActionType::Shake,
        0.0,
        TransitionValue::Shake {
            amplitude: 16.0,
            duration: 1.0,
        },
    )]);
    store.set_xy(owner, 100.0, 200.0);
    transition.play(1, 0.0);
    assert!(transition.advance(&mut store, owner, 0.5));
    let shaken = store.object(owner).unwrap().position();
    close((shaken.x - 100.0).abs(), 4.0);
    close((shaken.y - 200.0).abs(), 4.0);
    store.set_xy(owner, shaken.x + 30.0, shaken.y - 20.0);
    assert!(!transition.advance(&mut store, owner, 0.5));
    assert_eq!(
        store.object(owner).unwrap().position(),
        Point::new(130.0, 180.0)
    );
    transition.play(1, 0.0);
    transition.advance(&mut store, owner, 0.25);
    transition.stop_and_restore(&mut store);
    assert_eq!(
        store.object(owner).unwrap().position(),
        Point::new(130.0, 180.0)
    );
}

#[test]
fn completed_tween_does_not_overwrite_later_property_edits() {
    let (mut store, owner, mut transition) = fixture(vec![
        tween(
            ActionType::Alpha,
            TransitionValue::Scalar(0.0),
            TransitionValue::Scalar(1.0),
        ),
        item(ActionType::Visible, 2.0, TransitionValue::Visible(true)),
    ]);
    transition.play(1, 0.0);
    transition.advance(&mut store, owner, 1.25);
    store.set_alpha(owner, 0.4);
    transition.advance(&mut store, owner, 0.25);
    close(store.object(owner).unwrap().alpha(), 0.4);
}

#[test]
fn long_frames_cross_all_finite_loops_and_do_not_repeat_discrete_actions_per_frame() {
    let (mut store, owner, mut transition) = fixture(vec![
        item(
            ActionType::Sound,
            0.0,
            TransitionValue::Sound {
                sound: Some("tick.wav".into()),
                volume: 1.0,
            },
        ),
        item(ActionType::Visible, 1.0, TransitionValue::Visible(true)),
    ]);
    transition.play(4, 0.0);
    transition.advance(&mut store, owner, 3.25);
    assert_eq!(store.take_queued_sounds().len(), 4);
    close(transition.time(), 0.25);
    transition.advance(&mut store, owner, 0.1);
    assert!(store.take_queued_sounds().is_empty());
    assert!(!transition.advance(&mut store, owner, 1.0));
}

#[test]
fn transition_display_lock_covers_playback_and_releases_on_cancel() {
    let mut alpha = tween(
        ActionType::Alpha,
        TransitionValue::Scalar(0.0),
        TransitionValue::Scalar(1.0),
    );
    alpha.target_index = 0;
    let (mut store, owner, mut transition) = fixture(vec![alpha]);
    let child = store.create(ObjectType::Image);
    store.set_parent(child, Some(owner));
    store.apply_display_gear(child, crate::widget::GearKind::Display, 0, false, 0);
    assert!(!store.display_visible(child));
    transition.options = 1;
    transition.play(1, 0.5);
    transition.advance(&mut store, owner, 0.25);
    assert!(!store.display_visible(child));
    transition.advance(&mut store, owner, 0.5);
    assert!(store.display_visible(child));
    transition.stop_and_restore(&mut store);
    assert!(!store.display_visible(child));
}

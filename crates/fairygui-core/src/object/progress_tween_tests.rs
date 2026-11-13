use super::*;

fn fixture() -> (ObjectStore, GObjectId, GObjectId, GObjectId) {
    let mut store = ObjectStore::new();
    let owner = store.create(ObjectType::ProgressBar);
    store.set_size(owner, 120., 20.);
    let bar = store.create(ObjectType::Graph);
    store.set_parent(bar, Some(owner));
    store.set_name(bar, "bar");
    store.set_size(bar, 100., 10.);
    let title = store.create(ObjectType::Text);
    store.set_parent(title, Some(owner));
    store.set_name(title, "title");
    store.initialize_progress(owner);
    (store, owner, bar, title)
}
fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-6, "{actual} != {expected}");
}

#[test]
fn raw_fractional_values_clamp_only_visuals_and_titles_use_native_floor() {
    let (mut store, owner, bar, title) = fixture();
    store.set_progress_range(owner, -2., 2.9);
    store.set_progress_title_type(owner, 1);
    store.set_progress(owner, -0.25);
    assert_eq!(store.object_text(title), Some("-1/2"));
    assert_eq!(store.progress_value(owner), Some(-0.25));
    assert_eq!(store.object(bar).unwrap().width(), 36.);
    store.set_progress(owner, 1000.25);
    assert_eq!(store.progress_value(owner), Some(1000.25));
    assert_eq!(store.object(bar).unwrap().width(), 100.);
    store.set_progress(owner, -1000.25);
    assert_eq!(store.object(bar).unwrap().width(), 0.);
    for (min, max) in [(3., 2.), (f64::NAN, 2.), (0., f64::INFINITY)] {
        assert!(!store.set_progress_range(owner, min, max));
    }
    assert!(!store.set_progress(owner, f64::NAN));
    store.set_progress_range(owner, 2., 2.);
    assert_eq!(store.object(bar).unwrap().width(), 0.);
}

#[test]
fn target_is_immediate_display_is_linear_and_replacement_uses_the_current_tween_sample() {
    let (mut store, owner, bar, title) = fixture();
    store.set_progress(owner, 0.25);
    store.tween_progress(owner, 100.25, 2.);
    assert_eq!(store.progress_value(owner), Some(100.25));
    assert_eq!(store.progress_display_value(owner), Some(0.25));
    assert!(store.progress_is_tweening(owner));
    store.advance_tweens(0.5);
    near(store.progress_display_value(owner).unwrap(), 25.25);
    assert_eq!(store.object(bar).unwrap().width(), 25.);
    assert_eq!(store.object_text(title), Some("25%"));
    assert!(
        !store.set_progress(owner, 100.25),
        "same target does not kill native tween"
    );
    store.tween_progress(owner, 75.25, 1.);
    store.advance_tweens(0.5);
    near(store.progress_display_value(owner).unwrap(), 50.25);
    assert!(store.take_progress_tween_completions().is_empty());
    store.advance_tweens(0.5);
    assert_eq!(store.progress_display_value(owner), Some(75.25));
    assert!(!store.progress_is_tweening(owner));
    assert_eq!(store.take_progress_tween_completions(), [(owner, 75.25)]);
    store.advance_tweens(10.);
    assert!(store.take_progress_tween_completions().is_empty());
}

#[test]
fn range_resize_and_title_changes_redraw_without_replacing_the_tween_sample() {
    let (mut store, owner, bar, title) = fixture();
    store.set_progress(owner, 0.);
    store.tween_progress(owner, 100., 2.);
    store.advance_tweens(0.5); // sample=25
    store.set_size(owner, 220., 20.); // native resize draws target=100
    assert_eq!(store.progress_display_value(owner), Some(100.));
    assert_eq!(store.object(bar).unwrap().width(), 200.);
    store.set_progress_range(owner, 0., 200.);
    assert_eq!(store.object(bar).unwrap().width(), 100.);
    store.set_progress_title_type(owner, 2); // native update(titleType)
    assert_eq!(store.progress_value(owner), Some(100.));
    assert_eq!(store.progress_display_value(owner), Some(2.));
    assert_eq!(store.object_text(title), Some("2"));
    store.tween_progress(owner, 75., 1.); // starts at sample=25, not display=2 or target=100
    store.advance_tweens(0.5);
    assert_eq!(store.progress_display_value(owner), Some(50.));
    assert_eq!(store.object(bar).unwrap().width(), 50.);
    assert_eq!(store.object_text(title), Some("50"));
}

#[test]
fn stopping_setters_invalid_requests_and_disposal_have_distinct_lifecycles() {
    let (mut store, owner, _, _) = fixture();
    store.set_progress(owner, 0.);
    store.tween_progress(owner, 100., 2.);
    store.advance_tweens(0.5);
    for (value, duration) in [(f64::NAN, 1.), (20., -1.), (20., f32::NAN)] {
        assert!(!store.tween_progress(owner, value, duration));
        assert!(store.progress_is_tweening(owner));
    }
    for dt in [0., -1., f32::NAN] {
        store.advance_tweens(dt);
    }
    assert_eq!(store.progress_display_value(owner), Some(25.));
    store.stop_progress_tween(owner);
    store.advance_tweens(10.);
    assert_eq!(store.progress_value(owner), Some(100.));
    assert_eq!(store.progress_display_value(owner), Some(25.));
    store.tween_progress(owner, 50., 1.); // no active sample: starts at old target=100
    store.advance_tweens(0.5);
    assert_eq!(store.progress_display_value(owner), Some(75.));
    store.set_progress(owner, 12.5);
    store.advance_tweens(10.);
    assert_eq!(store.progress_display_value(owner), Some(12.5));
    assert!(store.take_progress_tween_completions().is_empty());
    store.tween_progress(owner, 25., 0.);
    assert_eq!(store.progress_display_value(owner), Some(25.));
    store.stop_progress_tween(owner); // cancels undelivered immediate completion
    assert!(store.take_progress_tween_completions().is_empty());
    store.tween_progress(owner, 50., 0.);
    store.remove(owner);
    assert!(store.progress_bars.is_empty());
    assert!(store.take_progress_tween_completions().is_empty());
}

#[test]
fn tween_samples_preserve_precision_beyond_f32_integer_resolution() {
    let (mut store, owner, _, _) = fixture();
    store.set_progress_range(owner, 16_777_216., 16_777_220.);
    store.set_progress(owner, 16_777_216.25);
    store.tween_progress(owner, 16_777_218.25, 1.);
    store.advance_tweens(0.25);
    assert_eq!(store.progress_display_value(owner), Some(16_777_216.75));
    assert_eq!(store.progress_value(owner), Some(16_777_218.25));
}

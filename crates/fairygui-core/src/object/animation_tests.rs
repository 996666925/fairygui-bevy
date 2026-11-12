use super::*;

#[test]
fn movie_ranges_rewind_and_frame_commands_preserve_order_without_changing_playing_or_speed() {
    let mut store = ObjectStore::new();
    let movie = store.create(ObjectType::Loader);
    store.set_animation_playing(movie, false);
    store.set_animation_time_scale(movie, 0.5);
    let settings = AnimationPlaySettings {
        start: 1,
        end: 3,
        times: 2,
        end_at: 0,
    };
    assert!(store.set_animation_play_settings(movie, settings));
    store.advance_animation(movie, 0.5);
    assert!(store.rewind_animation(movie));
    store.set_animation_frame(movie, 2);
    assert_eq!(store.animation_state(movie), Some((false, 2)));
    assert_eq!(store.animation_time_scale(movie), Some(0.5));
    assert_eq!(
        store.take_animation_commands(movie),
        vec![
            AnimationCommand::SetPlaySettings(settings),
            AnimationCommand::Advance(0.5),
            AnimationCommand::Rewind,
            AnimationCommand::SetFrame(2)
        ]
    );
}

#[test]
fn invalid_movie_ranges_leave_pending_commands_and_model_frame_untouched() {
    let mut store = ObjectStore::new();
    let movie = store.create(ObjectType::MovieClip);
    let plain = store.create(ObjectType::Graph);
    store.set_animation_frame(movie, 2);
    assert!(!store.set_animation_play_settings(
        movie,
        AnimationPlaySettings {
            start: -1,
            ..Default::default()
        }
    ));
    assert!(!store.set_animation_play_settings(
        movie,
        AnimationPlaySettings {
            end: -2,
            ..Default::default()
        }
    ));
    assert!(!store.set_animation_play_settings(
        movie,
        AnimationPlaySettings {
            end_at: -2,
            ..Default::default()
        }
    ));
    assert!(!store.set_animation_play_settings(plain, AnimationPlaySettings::default()));
    assert!(!store.rewind_animation(plain));
    assert_eq!(store.animation_state(movie).unwrap().1, 2);
    assert_eq!(
        store.take_animation_commands(movie),
        vec![AnimationCommand::SetFrame(2)]
    );
}

#[test]
fn animation_completions_are_consumed_once_and_subtree_removal_discards_pending_reports() {
    let mut store = ObjectStore::new();
    let owner = store.create(ObjectType::Component);
    let movie = store.create(ObjectType::MovieClip);
    store.set_parent(movie, Some(owner));
    store.report_animation_complete(owner); // non-movie ignored
    store.report_animation_complete(movie);
    assert_eq!(store.take_animation_completions(), vec![movie]);
    assert!(store.take_animation_completions().is_empty());
    store.report_animation_complete(movie);
    store.set_animation_play_settings(movie, AnimationPlaySettings::default());
    store.remove(owner);
    assert!(store.take_animation_completions().is_empty());
    assert!(store.take_animation_commands(movie).is_empty());
}

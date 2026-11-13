use super::*;

fn clip(count: usize, interval: i32, swing: bool, repeat_delay: i32) -> MovieClipData {
    MovieClipData {
        interval,
        swing,
        repeat_delay,
        frames: (0..count)
            .map(|_| fairygui_core::package::MovieFrame {
                add_delay: 0,
                sprite_id: None,
                texture: None,
            })
            .collect(),
    }
}

#[test]
fn ordinary_movie_updates_follow_stage_membership_including_explicitly_hidden_movies() {
    use fairygui_core::package::ObjectType;
    let mut gui = crate::GuiRuntime::default();
    let movie = gui.store_mut().create(ObjectType::MovieClip);
    gui.store_mut().attach_to_stage(movie);
    gui.store_mut().set_animation_playing(movie, true);
    let mut app = App::new();
    app.insert_non_send(gui)
        .init_resource::<Time>()
        .add_systems(Update, advance_movie_clips);
    let entity = app
        .world_mut()
        .spawn((
            crate::GObjectRef(movie),
            MovieClipPlayer::new(&clip(4, 100, false, 0), 0, true).unwrap(),
        ))
        .id();
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_millis(50));
    app.update();
    app.world_mut()
        .non_send_mut::<crate::GuiRuntime>()
        .store_mut()
        .set_visible(movie, false);
    app.update();
    assert_eq!(
        app.world().get::<MovieClipPlayer>(entity).unwrap().frame(),
        1
    );
    app.world_mut()
        .non_send_mut::<crate::GuiRuntime>()
        .store_mut()
        .detach_from_stage(movie);
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_secs(1));
    app.update();
    assert_eq!(
        app.world().get::<MovieClipPlayer>(entity).unwrap().frame(),
        1
    );
    app.world_mut()
        .non_send_mut::<crate::GuiRuntime>()
        .store_mut()
        .advance_animation(movie, 0.125);
    app.update();
    assert_eq!(
        app.world().get::<MovieClipPlayer>(entity).unwrap().frame(),
        2,
        "DeltaTime commands still run offstage"
    );
    app.world_mut()
        .non_send_mut::<crate::GuiRuntime>()
        .store_mut()
        .attach_to_stage(movie);
    app.update();
    assert_eq!(
        app.world().get::<MovieClipPlayer>(entity).unwrap().frame(),
        3
    );
}

#[test]
fn executable_official_movie_traces_match_each_command_and_state() {
    let mut player = None;
    let mut case = "";
    let mut completions = 0;
    let mut commands = 0;
    for (line_number, line) in include_str!("../tests/fixtures/movie-reference.txt")
        .lines()
        .enumerate()
    {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if line.starts_with("case ") {
            let values: Vec<_> = line.split_whitespace().collect();
            case = values[1];
            let mut data = clip(
                values[2].parse().unwrap(),
                values[3].parse().unwrap(),
                values[4] == "1",
                values[5].parse().unwrap(),
            );
            for (frame, delay) in data.frames.iter_mut().zip(values[7].split(',')) {
                frame.add_delay = delay.parse().unwrap();
            }
            player = Some(MovieClipPlayer::new(&data, values[6].parse().unwrap(), true).unwrap());
            completions = 0;
            continue;
        }
        let (command, expected) = line.split_once(" | ").unwrap();
        let arguments: Vec<_> = command.split_whitespace().collect();
        let expected: Vec<f64> = expected
            .split_whitespace()
            .map(|value| value.parse().unwrap())
            .collect();
        let player = player.as_mut().unwrap();
        match arguments[0] {
            "update" => {
                player.advance(arguments[1].parse::<f32>().unwrap() / 1000.0);
            }
            "advance" => {
                player.advance_time(arguments[1].parse::<f32>().unwrap() / 1000.0);
            }
            "settings" => {
                assert!(player.set_play_settings(AnimationPlaySettings {
                    start: arguments[1].parse().unwrap(),
                    end: arguments[2].parse().unwrap(),
                    times: arguments[3].parse().unwrap(),
                    end_at: arguments[4].parse().unwrap()
                }));
            }
            "frame" => player.goto(arguments[1].parse().unwrap()),
            "speed" => {
                player.set_time_scale(arguments[1].parse().unwrap());
            }
            "playing" => {
                if arguments[1] == "1" {
                    player.play();
                } else {
                    player.stop();
                }
            }
            "rewind" => player.rewind(),
            _ => panic!("unknown reference command: {command}"),
        }
        completions += usize::from(player.take_completion());
        let status = match player.status {
            PlaybackStatus::Playing => 0,
            PlaybackStatus::NextLoop => 1,
            PlaybackStatus::Ending => 2,
            PlaybackStatus::Ended => 3,
        };
        let actual = [
            player.frame as f64,
            f64::from(player.forward),
            f64::from(player.repeated),
            f64::from(status),
            player.time,
            f64::from(player.playing),
            f64::from(player.time_scale),
            f64::from(player.remaining_times),
            completions as f64,
        ];
        for (column, (actual, expected)) in actual.into_iter().zip(expected).enumerate() {
            assert!(
                (actual - expected).abs() < 0.001,
                "{case} line {} {command}, column {column}: {actual} != native {expected}",
                line_number + 1
            );
        }
        commands += 1;
    }
    assert!(
        commands > 1200,
        "fixture must exercise all captured command streams"
    );
}

#[test]
fn finite_range_holds_end_then_finishes_once_and_replays_only_after_new_settings() {
    let mut player = MovieClipPlayer::new(&clip(6, 100, false, 0), 0, true).unwrap();
    let settings = AnimationPlaySettings {
        start: 1,
        end: 3,
        times: 2,
        end_at: 5,
    };
    player.set_play_settings(settings);
    for (index, expected) in [2, 3, 1, 2, 3, 5].into_iter().enumerate() {
        player.advance(0.1);
        assert_eq!(player.frame(), expected);
        assert_eq!(player.take_completion(), index == 5);
    }
    assert!(player.is_ended());
    assert!(
        player.is_playing(),
        "native Playing is independent of the ended status"
    );
    player.goto(2);
    player.rewind();
    player.stop();
    player.play();
    player.advance(1.0);
    assert_eq!(
        player.frame(),
        0,
        "frame/Playing/rewind do not restart an ended range"
    );
    assert!(!player.take_completion());
    player.set_play_settings(settings);
    player.advance(0.1);
    assert_eq!(player.frame(), 2);
    assert!(!player.is_ended());
}

#[test]
fn same_frame_seek_retains_fractional_time_and_sync_copies_only_timing_state() {
    let mut source = MovieClipPlayer::new(&clip(4, 125, true, 250), 0, true).unwrap();
    source.advance_time(0.5); // backward at frame 2
    source.advance(0.0625);
    source.goto(2); // same frame must preserve the half-frame accumulated above
    let mut receiver = MovieClipPlayer::new(&clip(4, 125, true, 250), 0, false).unwrap();
    receiver.set_time_scale(2.0);
    receiver.set_play_settings(AnimationPlaySettings {
        start: 1,
        end: 2,
        times: 1,
        end_at: 3,
    });
    receiver.sync_status(&source);
    assert!(!receiver.is_playing());
    assert_eq!(receiver.time_scale(), 2.0);
    receiver.play();
    receiver.advance(0.03125);
    assert_eq!(
        receiver.frame(),
        1,
        "synchronized fractional time and reverse direction survive"
    );
    assert_eq!(receiver.remaining_times, 1);
    receiver.rewind();
    assert_eq!(
        (
            receiver.frame(),
            receiver.forward,
            receiver.repeated,
            receiver.time
        ),
        (0, true, false, 0.0)
    );
}

#[test]
fn model_play_settings_reach_delayed_player_and_completion_is_published_once() {
    use fairygui_core::package::ObjectType;
    let mut gui = crate::GuiRuntime::default();
    let movie = gui.store_mut().create(ObjectType::MovieClip);
    gui.store_mut().attach_to_stage(movie);
    gui.store_mut().set_animation_playing(movie, true);
    assert!(gui.store_mut().set_animation_play_settings(
        movie,
        AnimationPlaySettings {
            start: 1,
            end: 2,
            times: 1,
            end_at: 3
        }
    ));
    let mut app = App::new();
    app.insert_non_send(gui)
        .init_resource::<Time>()
        .add_systems(Update, advance_movie_clips);
    app.update(); // no source yet; the range command must survive
    let entity = app
        .world_mut()
        .spawn((
            crate::GObjectRef(movie),
            MovieClipPlayer::new(&clip(4, 100, false, 0), 1, true).unwrap(),
        ))
        .id();
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_millis(100));
    app.update();
    assert_eq!(
        app.world()
            .non_send::<crate::GuiRuntime>()
            .store()
            .animation_state(movie),
        Some((true, 2))
    );
    assert_eq!(
        app.world_mut()
            .non_send_mut::<crate::GuiRuntime>()
            .store_mut()
            .take_animation_completions()
            .len(),
        0
    );
    app.update();
    assert_eq!(
        app.world().get::<MovieClipPlayer>(entity).unwrap().frame(),
        3
    );
    assert_eq!(
        app.world_mut()
            .non_send_mut::<crate::GuiRuntime>()
            .store_mut()
            .take_animation_completions(),
        vec![movie]
    );
    app.update();
    assert_eq!(
        app.world_mut()
            .non_send_mut::<crate::GuiRuntime>()
            .store_mut()
            .take_animation_completions()
            .len(),
        0
    );
    app.world_mut()
        .non_send_mut::<crate::GuiRuntime>()
        .store_mut()
        .rewind_animation(movie);
    app.update();
    assert_eq!(
        app.world().get::<MovieClipPlayer>(entity).unwrap().frame(),
        0
    );
    assert!(
        app.world()
            .get::<MovieClipPlayer>(entity)
            .unwrap()
            .is_ended()
    );
}

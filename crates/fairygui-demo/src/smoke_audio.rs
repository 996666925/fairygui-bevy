//! Exercise default button sounds through Bevy's decoder, sink and cleanup.
use super::*;
use bevy::audio::{AudioPlayer, AudioSink, AudioSinkPlayback};

const PATH: &str = "ui/Basics_gojg7u.wav";
#[derive(Resource, Default)]
struct State {
    phase: u8,
    transient: Option<Entity>,
    retained: Option<Entity>,
    sink: Option<Entity>,
    volume: f32,
    frames: u32,
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<State>()
        .add_systems(Startup, setup)
        .add_systems(Update, drive.in_set(GuiSystems::Respond))
        .add_systems(Last, inspect);
}

fn setup(
    mut state: ResMut<State>,
    mut config: ResMut<GuiConfig>,
    mut assets: ResMut<Assets<FuiAsset>>,
    server: Res<AssetServer>,
    mut commands: Commands,
) {
    config.button.sound = Some(PATH.into());
    let package = assets.add(FuiAsset {
        package: fairygui_core::FuiPackage::parse(
            "ButtonDefaults",
            include_bytes!("../../fairygui-core/tests/fixtures/ButtonDefaults.fui"),
        )
        .unwrap(),
        res_key: "ui/ButtonDefaults".into(),
    });
    state.transient = Some(commands.spawn(FairyGuiRoot::new(package, "Button1")).id());
    let mut source = FairyGuiRoot::new(server.load("ui/Basics.fui"), "Demo_Controller");
    source.active = false;
    state.retained = Some(commands.spawn(source).id());
}

#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    gui: GuiQuery,
    mut objects: GuiMut,
    players: Query<(), With<AudioPlayer>>,
    time: Res<Time<bevy::time::Real>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    assert!(
        time.elapsed_secs_f64() < 90.,
        "audio smoke timeout at phase {} (a playback device is required)",
        state.phase
    );
    match state.phase {
        0 => {
            let Some(settings) = state.transient.and_then(|owner| gui.button_settings(owner))
            else {
                return;
            };
            let Some(retained) = state
                .retained
                .and_then(|owner| gui.child_by_name(owner, "switchBtn"))
            else {
                return;
            };
            assert_eq!(settings.sound.as_deref(), Some(PATH));
            state.volume = settings.sound_volume_scale.clamp(0., 1.);
            let owner = state.transient.unwrap();
            assert!(objects.get_mut(owner).unwrap().fire_click(false));
            // Playback belongs to the click; removing its owner in the same
            // frame must not discard it or change the shared asset path.
            commands.entity(owner).despawn();
            state.retained = Some(retained);
            state.phase = 1;
        }
        2 | 5 if state.sink.is_some_and(|entity| !players.contains(entity)) => {
            state.sink = None;
            state.phase += 1;
        }
        3 => {
            let owner = state.retained.unwrap();
            let mut settings = gui.button_settings(owner).unwrap().clone();
            assert_ne!(
                settings.sound.as_deref(),
                Some(PATH),
                "an exported sound overrides the global default"
            );
            assert!(
                settings
                    .sound
                    .as_deref()
                    .is_some_and(|sound| sound.starts_with("ui://"))
            );
            settings.sound_volume_scale = 0.35;
            let mut button = objects.get_mut(owner).unwrap();
            button.set_button_settings(settings);
            assert!(button.fire_click(false));
            state.volume = 0.35;
            state.phase = 4;
        }
        6 => {
            let owner = state.retained.unwrap();
            let mut settings = gui.button_settings(owner).unwrap().clone();
            settings.sound = Some(String::new());
            let mut button = objects.get_mut(owner).unwrap();
            button.set_button_settings(settings);
            assert!(button.fire_click(false));
            state.frames = 0;
            state.phase = 7;
        }
        7 => {
            state.frames += 1;
            if state.frames >= 3 {
                assert!(
                    players.is_empty(),
                    "an explicitly muted button must not create playback"
                );
                eprintln!(
                    "audio smoke PASSED: global default, raw asset path, same-frame owner disposal, packaged WAV decoding, live gain, sink creation, automatic cleanup and explicit mute"
                );
                state.phase = 8;
                exit.write(AppExit::Success);
            }
        }
        _ => (),
    }
}

fn inspect(
    mut state: ResMut<State>,
    players: Query<(Entity, &AudioPlayer, &AudioSink)>,
    server: Res<AssetServer>,
) {
    if !matches!(state.phase, 1 | 4) {
        return;
    }
    for (entity, player, sink) in &players {
        let Some(path) = server.get_path(player.0.id()) else {
            continue;
        };
        assert_eq!(path.path().to_str().unwrap(), PATH);
        assert!((sink.volume().to_linear() - state.volume).abs() < 0.0001);
        state.sink = Some(entity);
        state.phase += 1;
    }
}

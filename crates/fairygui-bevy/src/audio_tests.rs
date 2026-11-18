use super::*;
use crate::prelude::*;
use bevy::{
    audio::{AudioPlayer, AudioSource, PlaybackMode, PlaybackSettings},
    ecs::system::RunSystemOnce,
};

fn fixture() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<FuiAsset>()
        .init_asset::<AudioSource>()
        .init_non_send::<GuiRuntime>();
    app
}

#[test]
fn sound_paths_use_the_registered_sound_package_and_keep_external_paths_verbatim() {
    let mut app = fixture();
    let package = FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui"),
    )
    .unwrap();
    let handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: package.clone(),
            res_key: "shared/audio/Basics".into(),
        });
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .register_package(handle, &package);
    let sound = package
        .items()
        .iter()
        .find(|item| item.item_type == fairygui_core::package::PackageItemType::Sound)
        .unwrap();
    let urls = [
        format!("ui://{}{}", package.id(), sound.id),
        format!("ui://{}/{}", package.name(), sound.name),
    ];
    let runtime = app.world().non_send::<GuiRuntime>();
    let assets = app.world().resource::<Assets<FuiAsset>>();
    for url in urls {
        assert_eq!(
            sound_asset_path(runtime, assets, &url),
            Some(format!("shared/audio/{}", sound.file.as_deref().unwrap()))
        );
    }
    for path in [
        "audio/click.wav",
        "shared://click.mp3",
        "https://example.test/click.wav",
    ] {
        assert_eq!(
            sound_asset_path(runtime, assets, path).as_deref(),
            Some(path)
        );
    }
    assert_eq!(
        sound_asset_path(runtime, assets, "ui://Missing/sound"),
        None
    );
    assert_eq!(sound_asset_path(runtime, assets, ""), None);
}

#[test]
fn playback_is_independent_of_owner_lifetime_and_clamps_gain_without_replaying() {
    let mut app = fixture();
    let volumes = [-1., 0.5, 2., f32::NAN, f32::INFINITY, f32::NEG_INFINITY];
    {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        let owner = runtime
            .store_mut()
            .create(fairygui_core::package::ObjectType::Button);
        for (i, volume) in volumes.into_iter().enumerate() {
            runtime
                .store_mut()
                .queue_sound(owner, format!("audio/click{i}.wav"), volume);
        }
        runtime
            .store_mut()
            .queue_sound(owner, "ui://Missing/sound", 1.);
        runtime.store_mut().queue_sound(owner, "", 1.);
        runtime.store_mut().remove(owner);
    }
    app.world_mut().run_system_once(play_queued_sounds).unwrap();
    let audio: Vec<_> = {
        let mut query = app.world_mut().query::<(&AudioPlayer, &PlaybackSettings)>();
        query
            .iter(app.world())
            .map(|(player, settings)| (player.0.clone(), *settings))
            .collect()
    };
    assert_eq!(audio.len(), volumes.len());
    let server = app.world().resource::<AssetServer>();
    for (i, (player, settings)) in audio.iter().enumerate() {
        assert_eq!(
            server
                .get_path(player.id())
                .unwrap()
                .path()
                .to_str()
                .unwrap(),
            format!("audio/click{i}.wav")
        );
        assert_eq!(settings.volume.to_linear(), [0., 0.5, 1., 0., 1., 0.][i]);
        assert!(matches!(settings.mode, PlaybackMode::Despawn));
    }
    app.world_mut().run_system_once(play_queued_sounds).unwrap();
    assert_eq!(
        app.world_mut()
            .query::<&AudioPlayer>()
            .iter(app.world())
            .count(),
        volumes.len()
    );
}

fn gui_app() -> (App, Handle<FuiAsset>) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: FuiPackage::parse(
                "ButtonDefaults",
                include_bytes!("../../fairygui-core/tests/fixtures/ButtonDefaults.fui"),
            )
            .unwrap(),
            res_key: "ui/ButtonDefaults".into(),
        });
    (app, asset)
}

#[test]
fn ecs_defaults_are_visible_at_initialize_and_changes_only_affect_new_buttons() {
    let (mut app, asset) = gui_app();
    #[derive(Resource, Default)]
    struct Seen(Vec<(Entity, Option<String>)>);
    app.init_resource::<Seen>().add_systems(
        Update,
        (|gui: GuiQuery, roots: Query<Entity, Added<GuiRootView>>, mut seen: ResMut<Seen>| {
            for root in &roots {
                seen.0
                    .push((root, gui.button_settings(root).unwrap().sound.clone()));
            }
        })
        .in_set(GuiSystems::Initialize),
    );
    app.world_mut().resource_mut::<GuiConfig>().button.sound = Some("audio/global.wav".into());
    let first = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), "Button1"))
        .id();
    app.update();
    assert_eq!(
        app.world().resource::<Seen>().0,
        [(first, Some("audio/global.wav".into()))]
    );
    app.world_mut().resource_mut::<GuiConfig>().button.sound = Some("audio/next.wav".into());
    let next = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "Button1"))
        .id();
    app.update();
    assert_eq!(
        app.world().resource::<Seen>().0[1],
        (next, Some("audio/next.wav".into()))
    );
    let first_view = app.world().get::<GuiRootView>(first).unwrap().0;
    assert_eq!(
        app.world()
            .get::<GuiButtonSettings>(first_view)
            .unwrap()
            .sound
            .as_deref(),
        Some("audio/global.wav")
    );
}

#[test]
fn async_roots_inherit_defaults_and_explicit_ecs_muting_overrides_them() {
    let (mut app, asset) = gui_app();
    app.world_mut().resource_mut::<GuiConfig>().button.sound = Some("audio/global.wav".into());
    let first = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), "Button2").with_build_budget(1))
        .id();
    for _ in 0..100 {
        app.update();
        if app.world().get::<GuiRootView>(first).is_some() {
            break;
        }
    }
    let first_view = app.world().get::<GuiRootView>(first).unwrap().0;
    assert_eq!(
        app.world()
            .get::<GuiButtonSettings>(first_view)
            .unwrap()
            .sound
            .as_deref(),
        Some("audio/global.wav")
    );
    app.world_mut()
        .get_mut::<GuiButtonSettings>(first_view)
        .unwrap()
        .sound = Some(String::new());
    app.world_mut().resource_mut::<GuiConfig>().button.sound = Some("audio/next.wav".into());
    let next = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "Button1"))
        .id();
    app.update();
    let next_view = app.world().get::<GuiRootView>(next).unwrap().0;
    assert_eq!(
        app.world()
            .get::<GuiButtonSettings>(next_view)
            .unwrap()
            .sound
            .as_deref(),
        Some("audio/next.wav")
    );
    assert_eq!(
        app.world()
            .get::<GuiButtonSettings>(first_view)
            .unwrap()
            .sound
            .as_deref(),
        Some("")
    );
}

use crate::core::{object::EventKind, widget::ControlData};
use crate::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};

const BASE_ICON: &str = "ui://9leh0eyfrpmb6";
const SELECTED_ICON: &str = "ui://9leh0eyfrpmbh";

fn fixture(loaded: bool) -> (App, Entity, Handle<FuiAsset>) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let asset = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), "Button2"))
        .id();
    if loaded {
        load(&mut app, &asset);
        app.update();
    }
    (app, owner, asset)
}
fn load(app: &mut App, handle: &Handle<FuiAsset>) {
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(
            handle.id(),
            FuiAsset {
                package: crate::core::FuiPackage::parse(
                    "Basics",
                    include_bytes!("../../fairygui-demo/assets/ui/Basics.fui"),
                )
                .unwrap(),
                res_key: "ui/Basics".into(),
            },
        )
        .unwrap();
}
fn child(app: &mut App, owner: Entity, name: &str) -> Entity {
    SystemState::<GuiQuery>::new(app.world_mut())
        .get(app.world())
        .unwrap()
        .child_by_name(owner, name)
        .unwrap()
}
fn view(app: &App, owner: Entity) -> Entity {
    app.world().get::<GuiRootView>(owner).unwrap().0
}

#[test]
fn delayed_owner_settings_select_before_initialize_and_keep_base_title_and_icon() {
    #[derive(Resource, Default)]
    struct Seen(bool);
    fn inspect(roots: Query<Entity, Added<GuiRootView>>, gui: GuiQuery, mut seen: ResMut<Seen>) {
        for owner in &roots {
            assert_eq!(gui.button_settings(owner).unwrap().mode, ButtonMode::Check);
            assert_eq!(gui.text(owner), Some("Base"));
            assert_eq!(
                gui.text(gui.child_by_name(owner, "title").unwrap()),
                Some("Selected")
            );
            assert_eq!(
                gui.icon(gui.child_by_name(owner, "icon").unwrap()),
                Some(SELECTED_ICON)
            );
            seen.0 = true;
        }
    }
    let (mut app, owner, asset) = fixture(false);
    app.init_resource::<Seen>()
        .add_systems(Update, inspect.in_set(GuiSystems::Initialize));
    app.world_mut().entity_mut(owner).insert((
        GuiButtonSettings {
            mode: ButtonMode::Check,
            selected_title: Some("Selected".into()),
            selected_icon: Some(SELECTED_ICON.into()),
            ..default()
        },
        GuiButtonSelected(true),
        GuiText::new("Base"),
        GuiIcon(BASE_ICON.into()),
    ));
    app.update();
    assert!(!app.world().resource::<Seen>().0);
    load(&mut app, &asset);
    app.update();
    assert!(app.world().resource::<Seen>().0);
    let button = view(&app, owner);
    let title = child(&mut app, owner, "title");
    let icon = child(&mut app, owner, "icon");
    assert!(app.world().get::<GuiButtonSelected>(button).unwrap().0);
    assert_eq!(
        app.world().get::<GuiIcon>(button).unwrap().value(),
        BASE_ICON
    );
    app.world_mut()
        .get_mut::<GuiButtonSettings>(button)
        .unwrap()
        .selected_title = Some("Updated".into());
    app.world_mut()
        .get_mut::<GuiButtonSettings>(button)
        .unwrap()
        .selected_icon = None;
    app.update();
    assert_eq!(
        app.world().get::<GuiText>(title).unwrap().value(),
        "Updated"
    );
    assert_eq!(app.world().get::<GuiText>(button).unwrap().value(), "Base");
    assert_eq!(app.world().get::<GuiIcon>(icon).unwrap().value(), BASE_ICON);
    app.world_mut()
        .get_mut::<GuiIcon>(button)
        .unwrap()
        .0
        .clear();
    app.world_mut()
        .get_mut::<GuiButtonSettings>(button)
        .unwrap()
        .selected_icon = Some(SELECTED_ICON.into());
    app.update();
    app.world_mut()
        .get_mut::<GuiButtonSettings>(button)
        .unwrap()
        .selected_icon = None;
    app.update();
    assert_eq!(app.world().get::<GuiIcon>(icon).unwrap().value(), "");
}

#[test]
fn current_frame_settings_control_internal_clicks_and_mode_reset_does_not_emit_state_changed() {
    #[derive(Resource, Default)]
    struct Request(Option<Entity>);
    #[derive(Resource, Default)]
    struct Changes(usize);
    fn click(
        mut request: ResMut<Request>,
        references: Query<&GObjectRef>,
        mut runtime: NonSendMut<GuiRuntime>,
    ) {
        if let Some(entity) = request.0.take() {
            runtime.fire_button_click(references.get(entity).unwrap().0, false);
        }
    }
    let (mut app, owner, _) = fixture(true);
    app.init_resource::<Request>()
        .init_resource::<Changes>()
        .add_systems(
            Update,
            click.after(GuiSystems::Input).before(GuiSystems::Publish),
        );
    let button = view(&app, owner);
    app.world_mut().entity_mut(button).observe(
        |event: On<GuiEvent>, mut changes: ResMut<Changes>| {
            if event.event.kind == EventKind::StateChanged {
                changes.0 += 1;
            }
        },
    );
    {
        let mut settings = app
            .world_mut()
            .get_mut::<GuiButtonSettings>(button)
            .unwrap();
        settings.mode = ButtonMode::Check;
        settings.change_state_on_click = false;
    }
    app.world_mut().resource_mut::<Request>().0 = Some(button);
    app.update();
    assert!(!app.world().get::<GuiButtonSelected>(button).unwrap().0);
    app.world_mut()
        .get_mut::<GuiButtonSettings>(button)
        .unwrap()
        .change_state_on_click = true;
    app.world_mut().resource_mut::<Request>().0 = Some(button);
    app.update();
    assert!(app.world().get::<GuiButtonSelected>(button).unwrap().0);
    assert_eq!(app.world().resource::<Changes>().0, 1);
    app.world_mut()
        .get_mut::<GuiButtonSettings>(button)
        .unwrap()
        .mode = ButtonMode::Common;
    app.update();
    assert!(!app.world().get::<GuiButtonSelected>(button).unwrap().0);
    assert_eq!(app.world().resource::<Changes>().0, 1);
    app.world_mut()
        .get_mut::<GuiButtonSettings>(button)
        .unwrap()
        .mode = ButtonMode::Radio;
    app.world_mut()
        .get_mut::<GuiButtonSelected>(button)
        .unwrap()
        .0 = true;
    app.update();
    app.world_mut().resource_mut::<Request>().0 = Some(button);
    app.update();
    assert!(app.world().get::<GuiButtonSelected>(button).unwrap().0);
    assert_eq!(app.world().resource::<Changes>().0, 1);
}

#[test]
fn effect_edits_restore_held_scale_and_invalid_values_do_not_block_other_settings() {
    let (mut app, owner, _) = fixture(true);
    let button = view(&app, owner);
    let object = app.world().get::<GObjectRef>(button).unwrap().0;
    app.world_mut()
        .get_mut::<GuiTransform>(button)
        .unwrap()
        .scale = Vec2::new(2., 3.);
    app.world_mut()
        .get_mut::<GuiButtonSettings>(button)
        .unwrap()
        .down_effect = 2;
    app.update();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_button_pressed(object, true);
    app.update();
    assert!(
        app.world()
            .get::<GuiTransform>(button)
            .unwrap()
            .scale
            .abs_diff_eq(Vec2::new(1.6, 2.4), 0.001)
    );
    app.world_mut()
        .get_mut::<GuiButtonSettings>(button)
        .unwrap()
        .down_effect_value = 0.5;
    app.update();
    assert!(
        app.world()
            .get::<GuiTransform>(button)
            .unwrap()
            .scale
            .abs_diff_eq(Vec2::new(1., 1.5), 0.001)
    );
    {
        let mut settings = app
            .world_mut()
            .get_mut::<GuiButtonSettings>(button)
            .unwrap();
        settings.down_effect = 99;
        settings.down_effect_value = f32::NAN;
        settings.sound_volume_scale = f32::INFINITY;
        settings.change_state_on_click = false;
        settings.selected_title = Some("Still updated".into());
    }
    app.update();
    let settings = app.world().get::<GuiButtonSettings>(button).unwrap();
    assert_eq!((settings.down_effect, settings.down_effect_value), (2, 0.5));
    assert_eq!(settings.sound_volume_scale, 1.);
    assert!(!settings.change_state_on_click);
    assert_eq!(settings.selected_title.as_deref(), Some("Still updated"));
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_button_pressed(object, false);
    app.update();
    assert!(
        app.world()
            .get::<GuiTransform>(button)
            .unwrap()
            .scale
            .abs_diff_eq(Vec2::new(2., 3.), 0.001)
    );
}

#[test]
fn direct_edits_merge_with_components_and_retained_views_dispose_with_the_owner() {
    let (mut app, owner, _) = fixture(true);
    let button = view(&app, owner);
    let object = app.world().get::<GObjectRef>(button).unwrap().0;
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_button_sound_volume(object, 0.35);
    app.world_mut()
        .get_mut::<GuiButtonSettings>(button)
        .unwrap()
        .selected_title = Some("Merged".into());
    app.update();
    let settings = app
        .world()
        .get::<GuiButtonSettings>(button)
        .unwrap()
        .clone();
    assert_eq!(settings.sound_volume_scale, 0.35);
    let configured = GuiButtonSettings {
        mode: ButtonMode::Check,
        change_state_on_click: false,
        selected_title: Some("Direct".into()),
        sound: Some("click.wav".into()),
        ..settings
    };
    app.world_mut()
        .gui_mut(owner)
        .unwrap()
        .set_button_settings(configured.clone());
    let ControlData::Button(data) = app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .object(object)
        .unwrap()
        .control()
    else {
        panic!()
    };
    assert_eq!(data.mode, ButtonMode::Check);
    app.update();
    assert_eq!(
        app.world().get::<GuiButtonSettings>(button),
        Some(&configured)
    );
    {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        assert!(!runtime.store_mut().click_button(object));
        assert_eq!(
            runtime.store_mut().take_queued_sounds(),
            vec![(object, "click.wav".into(), 0.35)]
        );
    }
    let tick = app
        .world()
        .entity(button)
        .get_ref::<GuiButtonSettings>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .entity(button)
            .get_ref::<GuiButtonSettings>()
            .unwrap()
            .last_changed(),
        tick
    );
    let child = app
        .world_mut()
        .spawn((Node::default(), ChildOf(button)))
        .id();
    app.world_mut()
        .get_mut::<GuiButtonSettings>(button)
        .unwrap()
        .mode = ButtonMode::Common;
    app.update();
    assert!(app.world().get_entity(child).is_ok());
    app.world_mut().despawn(owner);
    app.update();
    assert!(app.world().get_entity(button).is_err());
    assert!(app.world().get_entity(child).is_err());
}

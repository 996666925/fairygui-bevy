use crate::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};

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
        .spawn(FairyGuiRoot::new(asset.clone(), "Demo_Graph"))
        .id();
    if loaded {
        load(&mut app, &asset);
        app.update();
    }
    (app, owner, asset)
}
fn load(app: &mut App, asset: &Handle<FuiAsset>) {
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(
            asset.id(),
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
fn child(app: &mut App, owner: Entity) -> Entity {
    SystemState::<GuiQuery>::new(app.world_mut())
        .get(app.world())
        .unwrap()
        .child_by_name(owner, "n10")
        .unwrap()
}
fn native(app: &App, entity: Entity) -> &crate::core::GObject {
    let id = app.world().get::<GObjectRef>(entity).unwrap().0;
    app.world()
        .non_send::<GuiRuntime>()
        .store()
        .object(id)
        .unwrap()
}

#[test]
fn delayed_owner_effects_are_available_before_initialize_and_view_edits_survive() {
    #[derive(Resource, Default)]
    struct Seen(bool);
    fn inspect(roots: Query<Entity, Added<GuiRootView>>, gui: GuiQuery, mut seen: ResMut<Seen>) {
        for owner in &roots {
            assert_eq!(gui.blend_mode(owner).unwrap().0, Some(BlendMode::Multiply));
            assert_eq!(gui.color_filter(owner).unwrap().0.unwrap().hue, 0.5);
            seen.0 = true;
        }
    }
    let (mut app, owner, asset) = fixture(false);
    app.init_resource::<Seen>()
        .add_systems(Update, inspect.in_set(GuiSystems::Initialize));
    app.world_mut().entity_mut(owner).insert((
        GuiBlendMode(Some(BlendMode::Multiply)),
        GuiColorFilter(Some(ColorFilterParams {
            hue: 0.5,
            ..default()
        })),
    ));
    for _ in 0..3 {
        app.update();
    }
    assert!(!app.world().resource::<Seen>().0);
    load(&mut app, &asset);
    app.update();
    assert!(app.world().resource::<Seen>().0);
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    app.world_mut().get_mut::<GuiBlendMode>(view).unwrap().0 = None;
    app.world_mut().get_mut::<GuiColorFilter>(view).unwrap().0 = None;
    app.update();
    assert!(!native(&app, view).blend_mode_is_set());
    assert!(native(&app, view).color_filter().is_none());
    assert_eq!(app.world().get::<GuiBlendMode>(owner).unwrap().0, None);
    assert_eq!(app.world().get::<GuiColorFilter>(owner).unwrap().0, None);
}

#[test]
fn explicit_normal_and_inherited_blending_remain_distinct_through_ecs_and_direct_edits() {
    let (mut app, owner, _) = fixture(true);
    let child = child(&mut app, owner);
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    app.world_mut().get_mut::<GuiBlendMode>(view).unwrap().0 = Some(BlendMode::Add);
    app.world_mut().get_mut::<GuiBlendMode>(child).unwrap().0 = Some(BlendMode::Normal);
    app.update();
    assert!(native(&app, child).blend_mode_is_set());
    assert_eq!(native(&app, child).blend_mode(), BlendMode::Normal);
    assert_eq!(native(&app, view).blend_mode(), BlendMode::Add);
    app.world_mut().gui_mut(child).unwrap().set_blend_mode(None);
    assert!(!native(&app, child).blend_mode_is_set());
    app.update();
    assert_eq!(app.world().get::<GuiBlendMode>(child).unwrap().0, None);
    app.world_mut()
        .gui_mut(child)
        .unwrap()
        .set_blend_mode(Some(BlendMode::Screen));
    app.update();
    assert_eq!(
        app.world().get::<GuiBlendMode>(child).unwrap().0,
        Some(BlendMode::Screen)
    );
}

#[test]
fn filter_channel_edits_merge_native_animation_and_gray_keeps_the_stored_filter() {
    let (mut app, owner, _) = fixture(true);
    let child = child(&mut app, owner);
    let id = app.world().get::<GObjectRef>(child).unwrap().0;
    app.world_mut().get_mut::<GuiColorFilter>(child).unwrap().0 = Some(ColorFilterParams {
        brightness: 0.1,
        ..default()
    });
    app.update();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_color_filter(
            id,
            Some(ColorFilterParams {
                brightness: 0.3,
                ..default()
            }),
        );
    app.world_mut()
        .get_mut::<GuiColorFilter>(child)
        .unwrap()
        .0
        .as_mut()
        .unwrap()
        .hue = 0.5;
    app.world_mut().get_mut::<GuiDisplay>(child).unwrap().grayed = true;
    app.update();
    let filter = native(&app, child).color_filter().unwrap();
    assert_eq!(filter.brightness, 0.3);
    assert_eq!(filter.hue, 0.5);
    assert_eq!(
        crate::runtime::appearance::local_color_matrix(native(&app, child)),
        Some(crate::core::color_matrix::ColorMatrix::LAYA_GRAY)
    );
    app.world_mut()
        .get_mut::<GuiColorFilter>(child)
        .unwrap()
        .0
        .as_mut()
        .unwrap()
        .saturation = -0.4;
    app.update();
    assert_eq!(native(&app, child).color_filter().unwrap().saturation, -0.4);
    app.world_mut().get_mut::<GuiDisplay>(child).unwrap().grayed = false;
    app.update();
    assert_eq!(
        crate::runtime::appearance::local_color_matrix(native(&app, child)),
        Some(native(&app, child).color_filter().unwrap().to_matrix())
    );
    app.world_mut()
        .gui_mut(child)
        .unwrap()
        .set_color_filter(None);
    app.update();
    assert!(
        app.world()
            .get::<GuiColorFilter>(child)
            .unwrap()
            .0
            .is_none()
    );
    // An explicit zero filter stays present, unlike a removed filter.
    app.world_mut()
        .gui_mut(child)
        .unwrap()
        .set_color_filter(Some(default()));
    app.update();
    assert_eq!(native(&app, child).color_filter(), Some(default()));
    app.world_mut().despawn(owner);
    app.update();
    assert!(app.world().get_entity(child).is_err());
}

#[test]
fn native_nan_filter_parameters_keep_stable_snapshots_and_render_finite_matrices() {
    let (mut app, owner, _) = fixture(true);
    let child = child(&mut app, owner);
    let value = GuiColorFilter(Some(ColorFilterParams {
        brightness: f32::NAN,
        contrast: f32::INFINITY,
        saturation: f32::NEG_INFINITY,
        hue: 3.,
    }));
    app.world_mut().entity_mut(child).insert(value);
    app.update();
    assert_eq!(app.world().get::<GuiColorFilter>(child), Some(&value));
    assert!(
        native(&app, child)
            .color_filter()
            .unwrap()
            .to_matrix()
            .values
            .iter()
            .all(|v| v.is_finite())
    );
    let tick = app
        .world()
        .entity(child)
        .get_ref::<GuiColorFilter>()
        .unwrap()
        .last_changed();
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(
        app.world()
            .entity(child)
            .get_ref::<GuiColorFilter>()
            .unwrap()
            .last_changed(),
        tick
    );
    let id = app.world().get::<GObjectRef>(child).unwrap().0;
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_color_filter(
            id,
            Some(ColorFilterParams {
                brightness: 0.4,
                ..default()
            }),
        );
    app.world_mut()
        .get_mut::<GuiColorFilter>(child)
        .unwrap()
        .0
        .as_mut()
        .unwrap()
        .hue = -0.5;
    app.update();
    assert_eq!(native(&app, child).color_filter().unwrap().brightness, 0.4);
    assert_eq!(native(&app, child).color_filter().unwrap().hue, -0.5);
}

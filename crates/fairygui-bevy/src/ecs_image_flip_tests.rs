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
        .spawn((
            FairyGuiRoot::new(asset.clone(), "pic")
                .with_size(Vec2::new(200., 100.))
                .with_position(Vec2::new(300., 200.)),
            GuiImageFlip(FlipType::Horizontal),
        ))
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
fn projected_origin(app: &App, view: Entity) -> Vec2 {
    let node = app.world().get::<Node>(view).unwrap();
    let t = app.world().get::<UiTransform>(view).unwrap();
    let (Val::Px(x), Val::Px(y), Val::Px(w), Val::Px(h), Val::Px(tx), Val::Px(ty)) = (
        node.left,
        node.top,
        node.width,
        node.height,
        t.translation.x,
        t.translation.y,
    ) else {
        panic!()
    };
    Vec2::new(x + w * 0.5 + tx, y + h * 0.5 + ty)
        + t.rotation * (t.scale * Vec2::new(-w * 0.5, -h * 0.5))
}

#[test]
fn delayed_image_flip_sets_native_scale_before_initialize_and_never_double_flips_uvs() {
    #[derive(Resource, Default)]
    struct Seen(bool);
    fn inspect(roots: Query<Entity, Added<GuiRootView>>, gui: GuiQuery, mut seen: ResMut<Seen>) {
        for owner in &roots {
            assert_eq!(gui.image_flip(owner), Some(FlipType::Horizontal));
            assert_eq!(gui.transform(owner).unwrap().scale, Vec2::new(-1., 1.));
            assert_eq!(
                gui.local_to_global(owner, Vec2::ZERO),
                Some(Vec2::new(500., 200.))
            );
            seen.0 = true;
        }
    }
    let (mut app, owner, asset) = fixture(false);
    app.init_resource::<Seen>()
        .add_systems(Update, inspect.in_set(GuiSystems::Initialize));
    for _ in 0..3 {
        app.update();
    }
    assert!(!app.world().resource::<Seen>().0);
    load(&mut app, &asset);
    app.update();
    assert!(app.world().resource::<Seen>().0);
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    assert_eq!(projected_origin(&app, view), Vec2::new(500., 200.));
    let image = app.world().get::<ImageNode>(view).unwrap();
    assert!(!image.flip_x && !image.flip_y);
    let tick = app
        .world()
        .entity(view)
        .get_ref::<GuiImageFlip>()
        .unwrap()
        .last_changed();
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(
        app.world()
            .entity(view)
            .get_ref::<GuiImageFlip>()
            .unwrap()
            .last_changed(),
        tick
    );
    app.world_mut().despawn(owner);
    app.update();
    assert!(app.world().get_entity(view).is_err());
}

#[test]
fn flip_edits_merge_explicit_transform_changes_and_repeated_assignment_keeps_custom_scale() {
    let (mut app, owner, _) = fixture(true);
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    app.world_mut().get_mut::<GuiImageFlip>(view).unwrap().0 = FlipType::Vertical;
    app.world_mut().get_mut::<GuiTransform>(view).unwrap().scale = Vec2::new(2., 0.5);
    app.update();
    assert_eq!(
        app.world().get::<GuiTransform>(view).unwrap().scale,
        Vec2::new(2., 0.5)
    );
    assert_eq!(projected_origin(&app, view), Vec2::new(300., 300.));
    app.world_mut()
        .gui_mut(view)
        .unwrap()
        .set_image_flip(FlipType::Vertical);
    app.update();
    assert_eq!(
        app.world().get::<GuiTransform>(view).unwrap().scale,
        Vec2::new(2., 0.5)
    );
    app.world_mut()
        .gui_mut(view)
        .unwrap()
        .set_image_flip(FlipType::Both);
    app.update();
    assert_eq!(
        app.world().get::<GuiImageFlip>(owner).unwrap().0,
        FlipType::Both
    );
    assert_eq!(
        app.world().get::<GuiTransform>(view).unwrap().scale,
        Vec2::splat(-1.)
    );
    assert_eq!(projected_origin(&app, view), Vec2::new(500., 300.));
    // Native zero-pivot size edits retain the old compensation until XY changes.
    app.world_mut().get_mut::<GuiTransform>(view).unwrap().size = Vec2::new(240., 120.);
    app.update();
    assert_eq!(projected_origin(&app, view), Vec2::new(500., 300.));
    app.world_mut()
        .get_mut::<GuiTransform>(view)
        .unwrap()
        .position
        .x += 10.;
    app.update();
    assert_eq!(projected_origin(&app, view), Vec2::new(550., 320.));
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    assert_eq!(
        query
            .get(app.world())
            .unwrap()
            .local_to_global(owner, Vec2::ZERO),
        Some(Vec2::new(550., 320.))
    );
}

#[test]
fn flip_components_are_only_projected_for_gimage_objects() {
    let (mut app, _, _) = fixture(true);
    let object = app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .create(crate::core::package::ObjectType::MovieClip);
    let entity = app
        .world_mut()
        .spawn((GObjectRef(object), GuiImageFlip(FlipType::Both)))
        .id();
    app.update();
    assert!(app.world().get::<GuiImageFlip>(entity).is_none());
}

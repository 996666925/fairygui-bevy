use super::*;
use crate::core::package::ObjectType;
use crate::{prelude::*, properties::sync_property};

#[test]
fn constraints_merge_native_axis_edits_and_reject_invalid_axes_independently() {
    let mut world = World::new();
    world.init_non_send::<GuiRuntime>();
    let id = world
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .create(ObjectType::Component);
    let entity = world.spawn(GObjectRef(id)).id();
    sync_property::<GuiSizeConstraints>(&mut world);
    world.get_mut::<GuiSizeConstraints>(entity).unwrap().max.x = 180.;
    world
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_size_constraints(
            id,
            SizeConstraints {
                min: Size::new(20., 10.),
                max: Size::new(0., 90.),
            },
        );
    sync_property::<GuiSizeConstraints>(&mut world);
    let s = world.get::<GuiSizeConstraints>(entity).unwrap();
    assert_eq!((s.min, s.max), (Vec2::new(20., 10.), Vec2::new(180., 90.)));
    {
        let mut s = world.get_mut::<GuiSizeConstraints>(entity).unwrap();
        s.min.x = 300.;
        s.max.y = 120.;
    }
    sync_property::<GuiSizeConstraints>(&mut world);
    let s = world.get::<GuiSizeConstraints>(entity).unwrap();
    assert_eq!((s.min, s.max), (Vec2::new(20., 10.), Vec2::new(180., 120.)));
}

#[test]
fn ecs_geometry_retains_the_other_requested_axis_and_restores_size_after_limits_clear() {
    use bevy::ecs::system::SystemState;
    #[derive(Resource, Default)]
    struct Seen(Vec<fairygui_core::object::EventKind>);
    fn size_changed(event: On<GuiSizeChanged>, mut seen: ResMut<Seen>) {
        seen.0.push(event.event.kind.clone());
    }

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
            package: crate::core::FuiPackage::parse(
                "MainMenu",
                include_bytes!("../../fairygui-demo/assets/ui/MainMenu.fui"),
            )
            .unwrap(),
            res_key: "ui/MainMenu".into(),
        });
    let owner = app
        .world_mut()
        .spawn((
            FairyGuiRoot::new(asset, "Main")
                .with_position(Vec2::new(100., 80.))
                .with_size(Vec2::new(400., 200.)),
            GuiSizeConstraints {
                min: Vec2::ZERO,
                max: Vec2::new(250., 120.),
            },
        ))
        .id();
    app.update();
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    let (view, mut t) = {
        let gui = query.get(app.world()).unwrap();
        assert_eq!(gui.transform(owner).unwrap().size(), Vec2::new(250., 120.));
        assert_eq!(
            gui.size_constraints(owner).unwrap().max,
            Vec2::new(250., 120.)
        );
        let view = gui.view(owner).unwrap();
        (view, *gui.transform(view).unwrap())
    };
    app.init_resource::<Seen>();
    app.world_mut().entity_mut(view).observe(size_changed);
    t.size.x = 450.;
    app.world_mut().entity_mut(view).insert(t);
    app.update();
    let native = app.world().get::<GObjectRef>(view).unwrap().0;
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .object(native)
            .unwrap()
            .requested_size(),
        Size::new(450., 200.)
    );
    app.update();
    assert_eq!(
        app.world().resource::<Seen>().0,
        [fairygui_core::object::EventKind::SizeChanged]
    );
    app.world_mut().resource_mut::<Seen>().0.clear();
    app.world_mut()
        .entity_mut(owner)
        .insert(GuiSizeConstraints::default());
    app.update();
    app.update();
    let gui = query.get(app.world()).unwrap();
    assert_eq!(gui.transform(owner).unwrap().size(), Vec2::new(450., 200.));
    let node = app.world().get::<Node>(view).unwrap();
    assert_eq!((node.width, node.height), (Val::Px(450.), Val::Px(200.)));
    assert_eq!(
        app.world().resource::<Seen>().0,
        [fairygui_core::object::EventKind::SizeChanged]
    );
    let before = *gui.transform(owner).unwrap();
    app.update();
    assert_eq!(
        *query.get(app.world()).unwrap().transform(owner).unwrap(),
        before
    );
}

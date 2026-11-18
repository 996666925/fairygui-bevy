use super::*;
use crate::{
    core::Size,
    prelude::*,
    properties::{sync, sync_property},
};
use bevy::ecs::system::SystemState;

#[test]
fn viewport_fields_merge_native_outer_and_margin_edits_and_reject_nonfinite_axes() {
    let mut world = World::new();
    world.init_non_send::<GuiRuntime>();
    let id = {
        let mut runtime = world.non_send_mut::<GuiRuntime>();
        let id = runtime
            .store_mut()
            .create(crate::core::package::ObjectType::Component);
        runtime.store_mut().set_size(id, 120., 80.);
        id
    };
    let view = world.spawn(GObjectRef(id)).id();
    sync_property::<GuiViewSize>(&mut world);
    world.get_mut::<GuiViewSize>(view).unwrap().0.x = 200.;
    {
        let mut runtime = world.non_send_mut::<GuiRuntime>();
        runtime
            .store_mut()
            .set_component_margin(id, crate::core::geom::Margin::new(7., 11., 13., 19.));
        runtime.store_mut().set_height(id, 150.);
    }
    sync_property::<GuiViewSize>(&mut world);
    assert_eq!(
        world.get::<GuiViewSize>(view).unwrap().0,
        Vec2::new(200., 120.)
    );
    assert_eq!(
        world
            .non_send::<GuiRuntime>()
            .store()
            .object(id)
            .unwrap()
            .size(),
        Size::new(220., 150.)
    );
    world.get_mut::<GuiViewSize>(view).unwrap().0 = Vec2::new(f32::NAN, 60.);
    sync_property::<GuiViewSize>(&mut world);
    assert_eq!(
        world.get::<GuiViewSize>(view).unwrap().0,
        Vec2::new(200., 60.)
    );
    world.get_mut::<GuiViewSize>(view).unwrap().0 = Vec2::new(90., 60.);
    sync_property::<GuiTransform>(&mut world);
    world.get_mut::<GuiTransform>(view).unwrap().size.y = 175.;
    sync(&mut world);
    assert_eq!(
        world.get::<GuiViewSize>(view).unwrap().0,
        Vec2::new(90., 145.)
    );
    assert_eq!(
        world
            .non_send::<GuiRuntime>()
            .store()
            .object(id)
            .unwrap()
            .requested_size(),
        Size::new(110., 175.)
    );
}

#[test]
fn viewport_root_configuration_query_and_direct_edits_apply_once_and_retain_other_size_requests() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let handle = app
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
            FairyGuiRoot::new(handle, "Main"),
            GuiMargin::new(10., 12., 20., 18.),
            GuiViewSize(Vec2::new(400., 300.)),
            GuiSizeConstraints {
                min: Vec2::ZERO,
                max: Vec2::new(250., 200.),
            },
        ))
        .id();
    app.update();
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    {
        let gui = query.get(app.world()).unwrap();
        for entity in [owner, view] {
            assert_eq!(gui.view_size(entity), Some(Vec2::new(220., 170.)));
            assert_eq!(gui.transform(entity).unwrap().size, Vec2::new(250., 200.));
        }
    }
    let id = app.world().get::<GObjectRef>(view).unwrap().0;
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .object(id)
            .unwrap()
            .requested_size(),
        Size::new(430., 330.)
    );
    app.world_mut().get_mut::<GuiViewSize>(owner).unwrap().0.x = 100.;
    app.update();
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .object(id)
            .unwrap()
            .requested_size(),
        Size::new(130., 330.)
    );
    app.world_mut()
        .entity_mut(owner)
        .insert(GuiSizeConstraints::default());
    app.update();
    let gui = query.get(app.world()).unwrap();
    assert_eq!(gui.view_size(owner), Some(Vec2::new(100., 300.)));
    app.world_mut().gui_mut(owner).unwrap().set_view_height(80.);
    app.update();
    assert_eq!(
        query.get(app.world()).unwrap().view_size(owner),
        Some(Vec2::new(100., 80.))
    );
    app.world_mut()
        .gui_mut(view)
        .unwrap()
        .set_view_size(Vec2::new(120., 90.));
    app.update();
    assert_eq!(
        query.get(app.world()).unwrap().view_size(view),
        Some(Vec2::new(120., 90.))
    );
    // Margin changes affect the view of the existing outer size. The old view
    // request must not reapply itself as a persistent size constraint.
    app.world_mut().get_mut::<GuiMargin>(owner).unwrap().right = 40.;
    app.update();
    assert_eq!(
        query.get(app.world()).unwrap().view_size(owner),
        Some(Vec2::new(100., 90.))
    );
    let tick = app
        .world()
        .entity(view)
        .get_ref::<GuiViewSize>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .entity(view)
            .get_ref::<GuiViewSize>()
            .unwrap()
            .last_changed(),
        tick
    );
}

use super::*;
use crate::{prelude::*, properties::sync_property};
use bevy::ecs::system::SystemState;

#[test]
fn snapping_projects_native_changes_without_replaying_old_ecs_values() {
    let mut world = World::new();
    world.init_non_send::<GuiRuntime>();
    let id = world
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .create(crate::core::package::ObjectType::Component);
    let view = world.spawn((GObjectRef(id), GuiPixelSnapping(true))).id();
    sync_property::<GuiPixelSnapping>(&mut world);
    assert!(
        world
            .non_send::<GuiRuntime>()
            .store()
            .object(id)
            .unwrap()
            .pixel_snapping()
    );
    world
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_pixel_snapping(id, false);
    sync_property::<GuiPixelSnapping>(&mut world);
    assert!(!world.get::<GuiPixelSnapping>(view).unwrap().0);
    world.get_mut::<GuiPixelSnapping>(view).unwrap().0 = true;
    sync_property::<GuiPixelSnapping>(&mut world);
    assert!(
        world
            .non_send::<GuiRuntime>()
            .store()
            .object(id)
            .unwrap()
            .pixel_snapping()
    );
}

#[test]
fn initial_root_snapping_query_and_direct_edits_keep_logical_geometry_and_event_counts() {
    #[derive(Resource, Default)]
    struct Events(usize);
    fn moved(_: On<GuiXyChanged>, mut events: ResMut<Events>) {
        events.0 += 1;
    }
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>()
    .init_resource::<Events>();
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
            FairyGuiRoot::new(asset, "Main").with_position(Vec2::new(10.5, -1.5)),
            GuiPixelSnapping(true),
        ))
        .id();
    app.update();
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    app.world_mut().entity_mut(view).observe(moved);
    let geometry = *app.world().get::<GuiTransform>(view).unwrap();
    app.world_mut()
        .gui_mut(owner)
        .unwrap()
        .set_pixel_snapping(false);
    app.update();
    app.update();
    let mut state = SystemState::<GuiQuery>::new(app.world_mut());
    let gui = state.get(app.world()).unwrap();
    for e in [owner, view] {
        assert_eq!(gui.pixel_snapping(e), Some(false));
        assert_eq!(*gui.transform(e).unwrap(), geometry);
    }
    assert_eq!(app.world().resource::<Events>().0, 0);
    app.world_mut()
        .get_mut::<GuiPixelSnapping>(owner)
        .unwrap()
        .0 = true;
    app.update();
    assert!(app.world().get::<GuiPixelSnapping>(view).unwrap().0);
}

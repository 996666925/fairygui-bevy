use super::*;
use crate::core::{Point, Rect, package::FuiPackage};
use crate::{FairyGuiPlugin, FairyGuiRoot, FuiAsset, GuiWorldExt};
use bevy::ecs::system::SystemState;

#[test]
fn ecs_space_tracks_native_anchor_skew_reflection_scroll_and_ancestor_edits_without_renderer_layout()
 {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let package = FuiPackage::parse(
        "ScrollPane",
        include_bytes!("../../fairygui-demo/assets/ui/ScrollPane.fui"),
    )
    .unwrap();
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package,
            res_key: "ui/ScrollPane".into(),
        });
    let owner = app.world_mut().spawn(FairyGuiRoot::new(asset, "Main")).id();
    app.update();
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    let list = query
        .get(app.world())
        .unwrap()
        .child_by_name(owner, "list")
        .unwrap();
    let row = query.get(app.world()).unwrap().list(list).unwrap().items()[0];
    let root = app
        .world()
        .get::<crate::GuiRootInstance>(owner)
        .unwrap()
        .object;
    let row_id = app.world().get::<crate::GObjectRef>(row).unwrap().0;
    {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        let store = runtime.store_mut();
        store.set_xy(root, 40., 70.);
        store.set_pivot(root, 0.5, 0.5, true);
        store.set_scale(root, -2., 1.5);
        store.set_rotation(root, 35.);
        store.set_skew(root, 8., -11.);
        store.set_pivot(row_id, 0.5, 0.5, true);
    }
    app.update();
    let gui = query.get(app.world()).unwrap();
    let p = Vec2::new(9., -12.);
    let global = gui.local_to_global(row, p).unwrap();
    let native = app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .local_to_global(row_id, Point::new(p.x, p.y))
        .unwrap();
    assert!((global - Vec2::new(native.x, native.y)).length() < 0.001);
    assert!((gui.global_to_local(row, global).unwrap() - p).length() < 0.001);
    let rect = gui
        .local_to_global_rect(row, Rect::new(0., 0., 40., 30.))
        .unwrap();
    assert!(rect.width < 0., "preserve two-corner reflected dimensions");
    let via_owner = gui.local_to_global(owner, Vec2::ZERO).unwrap();
    assert_eq!(
        gui.local_to_global(gui.view(owner).unwrap(), Vec2::ZERO),
        Some(via_owner)
    );
    assert!(gui.local_to_global(row, Vec2::splat(f32::NAN)).is_none());
    let tick = app
        .world()
        .entity(row)
        .get_ref::<GuiSpace>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .entity(row)
            .get_ref::<GuiSpace>()
            .unwrap()
            .last_changed(),
        tick
    );
    if let Some(mut object) = app.world_mut().gui_mut(owner) {
        object.set_position(Vec2::new(60., 90.))
    }
    app.update();
    let shifted = query
        .get(app.world())
        .unwrap()
        .local_to_global(row, p)
        .unwrap();
    assert!((shifted - global - Vec2::splat(20.)).length() < 0.001);
    let list_id = app.world().get::<crate::GObjectRef>(list).unwrap().0;
    {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        runtime.store_mut().set_size(list_id, 472., 100.);
        runtime.store_mut().layout_scroll_bars(list_id);
        runtime.store_mut().layout_list(list_id);
        runtime.store_mut().set_scroll_pos(list_id, 0., 80., false);
    }
    app.update();
    let scrolled = query
        .get(app.world())
        .unwrap()
        .local_to_global(row, p)
        .unwrap();
    assert!((scrolled - shifted).length() > 40.);
    let native = app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .local_to_global(row_id, Point::new(p.x, p.y))
        .unwrap();
    assert!((scrolled - Vec2::new(native.x, native.y)).length() < 0.001);
    if let Some(mut object) = app.world_mut().gui_mut(owner) {
        object.set_scale(Vec2::new(0., 1.))
    }
    app.update();
    assert!(
        query
            .get(app.world())
            .unwrap()
            .global_to_local(row, global)
            .is_none()
    );
    app.world_mut().despawn(owner);
    app.update();
    assert!(query.get(app.world()).unwrap().space(row).is_none());
}

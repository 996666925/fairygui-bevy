use super::*;
use crate::prelude::*;
use bevy::ecs::system::SystemState;

#[test]
fn exported_relations_keep_parent_identity_after_ecs_reparenting_and_original_parent_disposal() {
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
                "Basics",
                include_bytes!("../../fairygui-demo/assets/ui/Basics.fui"),
            )
            .unwrap(),
            res_key: "ui/Basics".into(),
        });
    let first = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), "Button1").with_size(Vec2::new(160., 80.)))
        .id();
    let second = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "Button1").with_size(Vec2::new(200., 100.)))
        .id();
    app.update();
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    let gui = query.get(app.world()).unwrap();
    let title = gui.child_by_name(first, "title").unwrap();
    let first_view = gui.view(first).unwrap();
    let second_view = gui.view(second).unwrap();
    let title_id = app.world().get::<GObjectRef>(title).unwrap().0;
    let second_id = app.world().get::<GObjectRef>(second_view).unwrap().0;
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_parent(title_id, Some(second_id));
    app.update();
    app.world_mut()
        .gui_mut(first)
        .unwrap()
        .set_size(Vec2::new(260., 120.));
    app.world_mut()
        .gui_mut(second)
        .unwrap()
        .set_size(Vec2::new(500., 300.));
    app.update();
    let gui = query.get(app.world()).unwrap();
    assert_eq!(gui.transform(title).unwrap().size(), Vec2::new(260., 120.));
    assert_eq!(
        app.world().get::<ChildOf>(title).unwrap().parent(),
        second_view
    );
    let node = app.world().get::<Node>(title).unwrap();
    assert_eq!((node.width, node.height), (Val::Px(260.), Val::Px(120.)));
    app.world_mut().despawn(first);
    app.update();
    assert!(!app.world().entities().contains(first_view));
    app.world_mut()
        .gui_mut(second)
        .unwrap()
        .set_size(Vec2::new(600., 400.));
    app.update();
    assert_eq!(
        query
            .get(app.world())
            .unwrap()
            .transform(title)
            .unwrap()
            .size(),
        Vec2::new(260., 120.)
    );
}

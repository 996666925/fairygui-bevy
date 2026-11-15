use super::*;
use crate::prelude::*;
use bevy::ecs::system::SystemState;

#[test]
fn exported_centered_group_is_correct_on_first_frame_and_after_ecs_member_edits() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        crate::FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: FuiPackage::parse(
                "ModalWaiting",
                include_bytes!("../../fairygui-demo/assets/ui/ModalWaiting.fui"),
            )
            .unwrap(),
            res_key: "ui/ModalWaiting".into(),
        });
    let owner = app
        .world_mut()
        .spawn(
            FairyGuiRoot::new(asset, "WindowModalWaiting")
                .with_position(Vec2::new(31., 17.))
                .with_size(Vec2::new(400., 300.)),
        )
        .id();
    app.update();
    let mut queries = SystemState::<GuiQuery>::new(app.world_mut());
    let gui = queries.get(app.world()).unwrap();
    let group = gui.child_by_name(owner, "n4").unwrap();
    let image = gui.child_by_name(owner, "n1").unwrap();
    let movie = gui.child_by_name(owner, "n3").unwrap();
    assert_eq!(
        gui.transform(group).unwrap().position(),
        Vec2::new(155., 110.)
    );
    assert_eq!(
        gui.transform(image).unwrap().position(),
        Vec2::new(155., 110.)
    );
    assert_eq!(
        gui.transform(movie).unwrap().position(),
        Vec2::new(177., 136.)
    );
    let mut transform = *gui.transform(image).unwrap();
    transform.size.x = 130.;
    app.world_mut().entity_mut(image).insert(transform);
    app.update();
    let gui = queries.get(app.world()).unwrap();
    assert_eq!(gui.transform(group).unwrap().size(), Vec2::new(130., 80.));
    assert_eq!(
        gui.transform(group).unwrap().position(),
        Vec2::new(135., 110.)
    );
    assert_eq!(
        gui.transform(image).unwrap().position(),
        Vec2::new(135., 110.)
    );
    assert_eq!(
        gui.transform(movie).unwrap().position(),
        Vec2::new(157., 136.)
    );
    for entity in [group, image, movie] {
        let transform = gui.transform(entity).unwrap();
        let node = app.world().get::<Node>(entity).unwrap();
        assert_eq!(
            (node.left, node.top),
            (Val::Px(transform.position.x), Val::Px(transform.position.y))
        );
    }
    let stable: Vec<_> = [group, image, movie]
        .into_iter()
        .map(|entity| *gui.transform(entity).unwrap())
        .collect();
    app.update();
    let gui = queries.get(app.world()).unwrap();
    assert_eq!(
        [group, image, movie]
            .into_iter()
            .map(|entity| *gui.transform(entity).unwrap())
            .collect::<Vec<_>>(),
        stable
    );
    // Parent resizing must keep the already enlarged group centered without rebuilding.
    let mut root = *gui.transform(owner).unwrap();
    root.size = Vec2::new(600., 500.);
    app.world_mut().entity_mut(owner).insert(root);
    app.update();
    let gui = queries.get(app.world()).unwrap();
    assert_eq!(
        gui.transform(group).unwrap().position(),
        Vec2::new(235., 210.)
    );
    assert_eq!(
        gui.transform(image).unwrap().position(),
        Vec2::new(235., 210.)
    );
    assert_eq!(
        gui.transform(movie).unwrap().position(),
        Vec2::new(257., 236.)
    );
}

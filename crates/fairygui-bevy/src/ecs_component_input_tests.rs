use crate::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};

#[test]
fn delayed_root_opaque_is_ready_before_initialize_and_direct_edits_preserve_visibility() {
    #[derive(Resource, Default)]
    struct Seen(bool);
    fn inspect(roots: Query<Entity, Added<GuiRootView>>, gui: GuiQuery, mut seen: ResMut<Seen>) {
        for owner in &roots {
            assert_eq!(gui.opaque(owner), Some(true));
            seen.0 = true;
        }
    }
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>()
    .init_resource::<Seen>()
    .add_systems(Update, inspect.in_set(GuiSystems::Initialize));
    let asset = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
    let owner = app
        .world_mut()
        .spawn((
            FairyGuiRoot::new(asset.clone(), "Demo_Graph"),
            GuiOpaque(true),
        ))
        .id();
    for _ in 0..3 {
        app.update();
    }
    assert!(!app.world().resource::<Seen>().0);
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
    app.update();
    assert!(app.world().resource::<Seen>().0);
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    let display = *app.world().get::<GuiDisplay>(view).unwrap();
    app.world_mut().get_mut::<GuiOpaque>(view).unwrap().0 = false;
    app.update();
    assert_eq!(app.world().get::<GuiOpaque>(owner), Some(&GuiOpaque(false)));
    assert_eq!(app.world().get::<GuiDisplay>(view), Some(&display));
    app.world_mut().gui_mut(owner).unwrap().set_opaque(true);
    app.update();
    let mut gui = SystemState::<GuiQuery>::new(app.world_mut());
    assert_eq!(gui.get(app.world()).unwrap().opaque(owner), Some(true));
    let tick = app
        .world()
        .entity(view)
        .get_ref::<GuiOpaque>()
        .unwrap()
        .last_changed();
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(
        app.world()
            .entity(view)
            .get_ref::<GuiOpaque>()
            .unwrap()
            .last_changed(),
        tick
    );
    let image = app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .create(crate::core::package::ObjectType::Image);
    let image = app
        .world_mut()
        .spawn((GObjectRef(image), GuiOpaque(true)))
        .id();
    app.update();
    assert!(app.world().get::<GuiOpaque>(image).is_none());
    app.world_mut().despawn(owner);
    app.update();
    assert!(app.world().get_entity(view).is_err());
}

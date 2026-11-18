use super::*;
use crate::{prelude::*, properties::sync_property};
use bevy::ecs::system::SystemState;

#[test]
fn margin_fields_merge_native_edits_and_restore_only_nonfinite_application_edits() {
    let mut world = World::new();
    world.init_non_send::<GuiRuntime>();
    let id = world
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .create(crate::core::package::ObjectType::Component);
    let entity = world.spawn(crate::GObjectRef(id)).id();
    sync_property::<GuiMargin>(&mut world);
    world.get_mut::<GuiMargin>(entity).unwrap().right = 20.;
    world
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_component_margin(id, Margin::new(7., 9., 11., 13.));
    sync_property::<GuiMargin>(&mut world);
    assert_eq!(
        *world.get::<GuiMargin>(entity).unwrap(),
        GuiMargin::new(7., 9., 20., 13.)
    );
    {
        let mut m = world.get_mut::<GuiMargin>(entity).unwrap();
        m.left = f32::NAN;
        m.top = -5.;
        m.right = f32::INFINITY;
    }
    sync_property::<GuiMargin>(&mut world);
    assert_eq!(
        *world.get::<GuiMargin>(entity).unwrap(),
        GuiMargin::new(7., -5., 20., 13.)
    );
}

#[test]
fn root_margin_supports_initial_values_query_and_direct_edit_without_size_events() {
    #[derive(Resource, Default)]
    struct Seen(usize);
    fn resized(_: On<GuiSizeChanged>, mut seen: ResMut<Seen>) {
        seen.0 += 1;
    }
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>()
    .init_resource::<Seen>();
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
        .spawn((FairyGuiRoot::new(asset, "Main"), GuiMargin::all(8.)))
        .id();
    app.update();
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    app.world_mut().entity_mut(view).observe(resized);
    let size = app.world().get::<GuiTransform>(view).unwrap().size;
    app.world_mut()
        .gui_mut(owner)
        .unwrap()
        .set_margin(GuiMargin::new(4., 6., 8., 10.));
    app.update();
    app.update();
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    let gui = query.get(app.world()).unwrap();
    for e in [owner, view] {
        assert_eq!(*gui.margin(e).unwrap(), GuiMargin::new(4., 6., 8., 10.));
        assert_eq!(gui.transform(e).unwrap().size, size);
    }
    assert_eq!(app.world().resource::<Seen>().0, 0);
    app.world_mut().get_mut::<GuiMargin>(owner).unwrap().bottom = 15.;
    app.update();
    assert_eq!(app.world().get::<GuiMargin>(view).unwrap().bottom, 15.);
}

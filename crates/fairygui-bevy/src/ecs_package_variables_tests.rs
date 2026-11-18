use crate::prelude::*;
use bevy::{ecs::system::RunSystemOnce, prelude::*};

fn package() -> FuiAsset {
    FuiAsset {
        package: crate::core::FuiPackage::parse(
            "ControllerEnv",
            include_bytes!("../../fairygui-core/tests/fixtures/ControllerEnv.fui"),
        )
        .unwrap(),
        res_key: "ui/ControllerEnv".into(),
    }
}
fn fixture(loaded: bool, item: &str) -> (App, Handle<FuiAsset>, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let asset = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
    if loaded {
        app.world_mut()
            .resource_mut::<Assets<FuiAsset>>()
            .insert(asset.id(), package())
            .unwrap();
    }
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), item))
        .id();
    (app, asset, owner)
}
fn view(app: &App, owner: Entity) -> Entity {
    app.world().get::<GuiRootView>(owner).unwrap().0
}
fn selected(app: &App, owner: Entity) -> i32 {
    app.world()
        .get::<GuiControllers>(view(app, owner))
        .unwrap()
        .get("c1")
        .unwrap()
        .selected_index
}

#[test]
fn late_assets_read_variables_before_initialize_and_publish_initial_gear_geometry() {
    #[derive(Resource, Default)]
    struct Seen(bool);
    let (mut app, asset, owner) = fixture(false, "Demo_Controller");
    app.world_mut()
        .resource_mut::<GuiConfig>()
        .package_variables
        .insert("mode".into(), "".into());
    let expected = {
        let package = package();
        let mut store = crate::core::object::ObjectStore::new();
        let root = crate::core::widget::create_from_package(
            &mut store,
            &package.package,
            package
                .package
                .item_index_by_name("Demo_Controller")
                .unwrap(),
            &crate::core::package::LoadContext::none(),
        )
        .unwrap();
        store.set_controller_page_index(root, "c1", 1);
        store.advance_gear_tweens(10.);
        store
            .object(store.child_by_name(root, "n3").unwrap())
            .unwrap()
            .position()
    };
    app.init_resource::<Seen>().add_systems(
        Update,
        (move |gui: GuiQuery, roots: Query<Entity, Added<GuiRootView>>, mut seen: ResMut<Seen>| {
            for root in &roots {
                assert_eq!(gui.controller(root, "c1").unwrap().selected_index, 1);
                assert_eq!(gui.controller(root, "c1").unwrap().previous_index(), -1);
                assert_eq!(gui.controller(root, "c2").unwrap().selected_index, 0);
                let child = gui.child_by_name(root, "n3").unwrap();
                assert_eq!(
                    gui.transform(child).unwrap().position(),
                    Vec2::new(expected.x, expected.y)
                );
                seen.0 = true;
            }
        })
        .in_set(GuiSystems::Initialize),
    );
    app.update();
    assert!(!app.world().resource::<Seen>().0);
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(asset.id(), package())
        .unwrap();
    app.update();
    assert!(app.world().resource::<Seen>().0);
    assert_eq!(selected(&app, owner), 1);
}

#[test]
fn replacing_and_removing_variables_only_changes_new_controllers() {
    let (mut app, asset, owner) = fixture(true, "Demo_Controller");
    app.world_mut()
        .resource_mut::<GuiConfig>()
        .package_variables
        .insert("mode".into(), "".into());
    app.update();
    assert_eq!(selected(&app, owner), 1);
    let old_view = view(&app, owner);
    app.world_mut()
        .resource_mut::<GuiConfig>()
        .package_variables
        .remove("mode");
    let next = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "Demo_Controller"))
        .id();
    app.update();
    assert_eq!(selected(&app, owner), 1);
    assert_eq!(view(&app, owner), old_view);
    assert_eq!(selected(&app, next), 0);
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .registry()
            .get_var("mode"),
        None
    );
}

#[test]
fn pending_builds_use_current_variables_instead_of_capturing_the_request_frame() {
    let (mut app, _, owner) = fixture(true, "Demo_Controller");
    app.world_mut()
        .get_mut::<FairyGuiRoot>(owner)
        .unwrap()
        .build_budget = Some(1);
    app.update();
    assert!(app.world().get::<GuiRootView>(owner).is_none());
    app.world_mut()
        .resource_mut::<GuiConfig>()
        .package_variables
        .insert("mode".into(), "".into());
    for _ in 0..200 {
        app.update();
        if app.world().get::<GuiRootView>(owner).is_some() {
            break;
        }
    }
    assert_eq!(selected(&app, owner), 1);
}

#[test]
fn explicit_controller_components_override_the_global_homepage() {
    let (mut app, _, owner) = fixture(true, "Demo_Controller");
    app.world_mut()
        .resource_mut::<GuiConfig>()
        .package_variables
        .insert("mode".into(), "".into());
    app.world_mut()
        .entity_mut(owner)
        .insert(GuiControllers::new([("c1", 0)]));
    app.update();
    assert_eq!(selected(&app, owner), 0);
    app.world_mut()
        .run_system_once(move |gui: GuiQuery| {
            assert_eq!(gui.controller(owner, "c1").unwrap().previous_index(), 1);
        })
        .unwrap();
}

#[test]
fn nested_and_replaced_roots_use_the_same_global_variables() {
    let (mut app, _, owner) = fixture(true, "Demo_Popup");
    app.world_mut()
        .resource_mut::<GuiConfig>()
        .package_variables
        .insert("mode".into(), "".into());
    app.update();
    app.world_mut()
        .run_system_once(move |gui: GuiQuery| {
            let nested = gui.get(owner).unwrap().children()[0];
            assert_eq!(gui.controller(nested, "c1").unwrap().selected_index, 1);
        })
        .unwrap();
    app.world_mut()
        .resource_mut::<GuiConfig>()
        .package_variables
        .clear();
    app.world_mut()
        .get_mut::<FairyGuiRoot>(owner)
        .unwrap()
        .item_name = "Demo_Controller".into();
    app.update();
    assert_eq!(selected(&app, owner), 0);
}

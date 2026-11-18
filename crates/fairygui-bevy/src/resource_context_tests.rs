use super::*;
use crate::{FairyGuiPlugin, GuiIcon, GuiObjectInfo, GuiTextFormat};
use fairygui_core::package::{ObjectType, PackageItemType, ResourceContext};

fn fixture() -> (App, Handle<FuiAsset>, FuiPackage) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let package = FuiPackage::parse(
        "ResourceVariants",
        include_bytes!("../../fairygui-core/tests/fixtures/ResourceVariants.fui"),
    )
    .unwrap();
    let handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: package.clone(),
            res_key: "ui/ResourceVariants".into(),
        });
    (app, handle, package)
}

fn select(app: &mut App, branch: &str, level: Option<i32>) {
    let mut config = app.world_mut().resource_mut::<GuiConfig>();
    config.package_branch = branch.into();
    config.content_scale_level = level;
}

fn object(app: &App, owner: Entity) -> GObjectId {
    app.world().get::<GuiRootInstance>(owner).unwrap().object
}

fn view(app: &App, id: GObjectId) -> Entity {
    app.world().non_send::<GuiRuntime>().entity_of(id).unwrap()
}

#[test]
fn config_selects_display_assets_before_initial_views_and_preserves_existing_roots() {
    let (mut app, asset, package) = fixture();
    let image = package
        .items()
        .iter()
        .find(|item| item.item_type == PackageItemType::Image)
        .unwrap();
    select(&mut app, "night", Some(1));
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), &image.name))
        .id();
    let movie = package
        .items()
        .iter()
        .find(|item| item.item_type == PackageItemType::MovieClip)
        .unwrap();
    let movie_owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), &movie.name))
        .id();
    app.update();
    let id = object(&app, owner);
    let entity = view(&app, id);
    assert_eq!(
        app.world()
            .get::<GuiObjectInfo>(entity)
            .unwrap()
            .package_item_id(),
        Some(image.id.as_str())
    );
    assert_eq!(
        app.world().get::<ImageNode>(entity).unwrap().rect,
        Some(Rect::new(14., 18., 134., 78.))
    );
    let movie_entity = view(&app, object(&app, movie_owner));
    assert_eq!(
        app.world()
            .get::<crate::movie::MovieClipPlayer>(movie_entity)
            .unwrap()
            .current()
            .unwrap()
            .original_size,
        fairygui_core::Size::new(120., 60.)
    );
    select(&mut app, "day", Some(0));
    let next = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, &image.name))
        .id();
    app.update();
    let gui = app.world().non_send::<GuiRuntime>();
    assert_eq!(
        gui.store.object(id).unwrap().content_item_id(),
        Some("v0hd")
    );
    assert_eq!(
        gui.store
            .object(object(&app, next))
            .unwrap()
            .content_item_id(),
        Some(image.id.as_str())
    );
    assert_eq!(view(&app, id), entity);
    assert_eq!(
        app.world().get::<ImageNode>(entity).unwrap().rect,
        Some(Rect::new(14., 18., 134., 78.))
    );
}

#[test]
fn dpi_and_ui_scale_update_only_future_requests_and_explicit_levels_take_priority() {
    let (mut app, asset, package) = fixture();
    let window = app
        .world_mut()
        .spawn((
            Window {
                resolution: bevy::window::WindowResolution::new(800, 600)
                    .with_scale_factor_override(1.25),
                ..default()
            },
            bevy::window::PrimaryWindow,
        ))
        .id();
    app.insert_resource(UiScale(1.2));
    let image = package
        .items()
        .iter()
        .find(|item| item.item_type == PackageItemType::Image)
        .unwrap();
    select(&mut app, "night", None);
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), &image.name))
        .id();
    app.update();
    let id = object(&app, owner);
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store
            .object(id)
            .unwrap()
            .content_item_id(),
        Some("v0hd")
    );
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .resolution
        .set_scale_factor_override(Some(3.));
    app.update();
    let gui = app.world().non_send::<GuiRuntime>();
    assert_eq!(gui.registry.content_scale_level(), 3);
    assert_eq!(
        gui.store
            .object(id)
            .unwrap()
            .resource_context()
            .content_scale_level,
        1
    );
    select(&mut app, "night", Some(-1));
    let next = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, &image.name))
        .id();
    app.update();
    let gui = app.world().non_send::<GuiRuntime>();
    assert_eq!(gui.registry.content_scale_level(), 0);
    assert_eq!(
        gui.store
            .object(object(&app, next))
            .unwrap()
            .content_item_id(),
        Some("v0night")
    );
}

#[test]
fn same_frame_ecs_loader_and_font_requests_capture_new_config_and_reload_keeps_entities() {
    let (mut app, asset, _) = fixture();
    select(&mut app, "day", Some(0));
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "Demo_Popup"))
        .id();
    app.update();
    let root = object(&app, owner);
    let (loader, field) = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let loader = gui.store.create(ObjectType::Loader);
        gui.store
            .apply_icon_to(loader, "ui://ResourceVariants/nlge1k");
        gui.store.set_parent(loader, Some(root));
        gui.queue_subtree(loader);
        let field = gui.store.create(ObjectType::Text);
        gui.store.apply_text_to(field, "123");
        gui.store.set_parent(field, Some(root));
        gui.queue_subtree(field);
        (loader, field)
    };
    app.update();
    let loader_view = view(&app, loader);
    let field_view = view(&app, field);
    select(&mut app, "night", Some(1));
    app.world_mut().get_mut::<GuiIcon>(loader_view).unwrap().0 =
        "ui://ResourceVariants/grid_header_png".into();
    app.world_mut()
        .get_mut::<GuiTextFormat>(field_view)
        .unwrap()
        .font = Some("ui://ResourceVariants/HitNumber".into());
    app.update();
    let expected = ResourceContext {
        branch: "night".into(),
        content_scale_level: 1,
    };
    let gui = app.world().non_send::<GuiRuntime>();
    assert_eq!(
        gui.store.object(loader).unwrap().resource_context(),
        &expected
    );
    assert_eq!(
        gui.store.object(field).unwrap().resource_context(),
        &expected
    );
    let child = app
        .world()
        .get::<movie_sources::MovieSource>(loader_view)
        .unwrap()
        .visual
        .unwrap();
    assert_eq!(
        app.world().get::<ImageNode>(child).unwrap().rect,
        Some(Rect::new(14., 18., 134., 78.))
    );
    select(&mut app, "night", Some(0));
    app.update();
    assert_eq!(
        app.world()
            .get::<movie_sources::MovieSource>(loader_view)
            .unwrap()
            .visual,
        Some(child)
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .retry_loader(loader);
    app.update();
    assert_eq!(view(&app, loader), loader_view);
    assert!(app.world().get_entity(child).is_err());
    let child = app
        .world()
        .get::<movie_sources::MovieSource>(loader_view)
        .unwrap()
        .visual
        .unwrap();
    assert_eq!(
        app.world().get::<ImageNode>(child).unwrap().rect,
        Some(Rect::new(7., 9., 67., 39.))
    );
}

#[test]
fn asynchronous_roots_keep_their_resource_choice_until_the_whole_tree_is_published() {
    let (mut app, asset, _) = fixture();
    select(&mut app, "night", Some(1));
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "Demo_Popup").with_build_budget(1))
        .id();
    app.update();
    assert!(app.world().get::<GuiRootView>(owner).is_none());
    select(&mut app, "day", Some(0));
    for _ in 0..200 {
        app.update();
        if app.world().get::<GuiRootView>(owner).is_some() {
            break;
        }
    }
    let gui = app.world().non_send::<GuiRuntime>();
    let root = object(&app, owner);
    assert_eq!(
        gui.store.object(root).unwrap().content_item_id(),
        Some("v3night")
    );
    gui.store.for_each_descendant(root, &mut |id| {
        assert_eq!(
            gui.store.object(id).unwrap().resource_context().branch,
            "night"
        );
    });
}

#[test]
fn component_loaders_retain_their_branch_until_an_explicit_reload() {
    let (mut app, asset, _) = fixture();
    select(&mut app, "night", Some(1));
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "Demo_Text"))
        .id();
    app.update();
    let root = object(&app, owner);
    let loader = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let loader = gui.store.create(ObjectType::Loader);
        gui.store
            .apply_icon_to(loader, "ui://ResourceVariants/Demo_Popup");
        gui.store.set_parent(loader, Some(root));
        gui.queue_subtree(loader);
        loader
    };
    app.update();
    let content = app
        .world()
        .non_send::<GuiRuntime>()
        .loader_component(loader)
        .unwrap();
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store
            .object(content)
            .unwrap()
            .content_item_id(),
        Some("v3night")
    );
    select(&mut app, "day", Some(0));
    app.update();
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .loader_component(loader),
        Some(content)
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .retry_loader(loader);
    app.update();
    let gui = app.world().non_send::<GuiRuntime>();
    let next = gui.loader_component(loader).unwrap();
    assert_ne!(next, content);
    assert!(!gui.store.contains(content));
    assert_eq!(
        gui.store.object(next).unwrap().resource_context().branch,
        "day"
    );
    assert_eq!(
        gui.store.object(next).unwrap().source_size(),
        fairygui_core::Size::new(40., 20.)
    );
}

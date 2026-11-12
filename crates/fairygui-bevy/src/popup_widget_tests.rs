use super::*;
use bevy::ecs::system::RunSystemOnce;
use fairygui_core::geom::Size;

#[path = "popup_menu_tests.rs"]
mod menu;
#[path = "popup_template_tests.rs"]
mod templates;

fn fixture_unspawned() -> (App, FuiPackage, GObjectId) {
    let menu = FuiPackage::parse(
        "MainMenu",
        include_bytes!("../../fairygui-core/tests/fixtures/MainMenu.fui"),
    )
    .unwrap();
    let package = FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui"),
    )
    .unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<FuiAsset>()
        .init_asset::<Image>()
        .init_asset::<Font>()
        .init_resource::<GuiFont>();
    let menu_handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: menu.clone(),
            res_key: "wrong/MainMenu".into(),
        });
    let handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: package.clone(),
            res_key: "correct/Basics".into(),
        });
    let mut gui = GuiRuntime::default();
    gui.create_root(menu_handle, &menu, "Main").unwrap();
    let index = gui.create_root(handle, &package, "Demo_ComboBox").unwrap();
    let object = gui.root_object(index).unwrap();
    let combo = gui.store().child_by_name(object, "n1").unwrap();
    gui.ensure_root(Size::new(1280.0, 720.0));
    app.insert_non_send(gui).add_systems(
        Update,
        (
            cleanup_retained_popups,
            spawn_pending_roots,
            spawn_pending_objects,
            sync_dirty,
            sync_stage_hierarchy,
        )
            .chain(),
    );
    (app, package, combo)
}

fn fixture() -> (App, FuiPackage, GObjectId) {
    let (mut app, package, combo) = fixture_unspawned();
    app.update();
    (app, package, combo)
}

fn with_commands<T>(app: &mut App, action: impl FnOnce(&mut GuiRuntime, &mut Commands) -> T) -> T {
    let mut gui = app.world_mut().remove_non_send::<GuiRuntime>().unwrap();
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let value = action(&mut gui, &mut Commands::new(&mut queue, app.world()));
    queue.apply(app.world_mut());
    app.insert_non_send(gui);
    value
}

#[test]
fn combo_retains_complete_dropdown_and_items_and_uses_its_own_package_directory() {
    let (mut app, package, combo) = fixture();
    let dropdown = with_commands(&mut app, |gui, commands| {
        gui.show_combo_dropdown(commands, combo, &package).unwrap()
    });
    app.update();
    let (entity, children, list) = {
        let gui = app.world().non_send::<GuiRuntime>();
        let popup = gui.store().combo_popup(combo).unwrap();
        let children = gui.store().object(popup.list).unwrap().children().to_vec();
        assert_eq!(children.len(), 8);
        assert_eq!(gui.popup_stack(), &[dropdown]);
        assert_eq!(gui.store().list_selection(popup.list), Some(-1));
        assert_eq!(
            gui.store().object(popup.dropdown).unwrap().width(),
            gui.store().object(combo).unwrap().width()
        );
        (gui.entity_of(dropdown).unwrap(), children, popup.list)
    };
    let background = {
        let gui = app.world().non_send::<GuiRuntime>();
        let background = gui.store().child_by_name(dropdown, "n0").unwrap();
        gui.entity_of(background).unwrap()
    };
    let atlas = app.world().get::<ImageNode>(background).unwrap().image.id();
    let path = app
        .world()
        .resource::<AssetServer>()
        .get_path(atlas)
        .unwrap();
    assert!(
        path.path().to_string_lossy().starts_with("correct/Basics"),
        "{path}"
    );
    with_commands(&mut app, |gui, commands| {
        assert!(gui.close_combo_dropdown(commands, combo));
    });
    app.update();
    assert_eq!(
        app.world().get::<Visibility>(entity),
        Some(&Visibility::Hidden)
    );
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .contains(dropdown)
    );
    let reopened = with_commands(&mut app, |gui, commands| {
        gui.show_combo_dropdown(commands, combo, &package).unwrap()
    });
    app.update();
    assert_eq!(reopened, dropdown);
    let gui = app.world().non_send::<GuiRuntime>();
    assert_eq!(gui.entity_of(dropdown), Some(entity));
    assert_eq!(gui.store().object(list).unwrap().children(), children);
    assert_eq!(
        app.world().get::<Visibility>(entity),
        Some(&Visibility::Inherited)
    );
}

#[test]
fn mixed_stack_closes_only_above_hit_and_menu_retains_its_real_list_and_selection_delay() {
    let (mut app, package, combo) = fixture();
    let parent = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let parent = gui
            .store_mut()
            .create(fairygui_core::package::ObjectType::Component);
        gui.show_popup(parent, Point::ZERO);
        gui.store_mut().set_parent(combo, Some(parent));
        parent
    };
    let dropdown = with_commands(&mut app, |gui, commands| {
        gui.show_combo_dropdown(commands, combo, &package).unwrap()
    });
    let menu = with_commands(&mut app, |gui, commands| {
        gui.show_popup_menu(
            commands,
            &package,
            "PopupMenu",
            "PopupMenuItem",
            &["First", "Second"],
            Point::new(40.0, 50.0),
        )
        .unwrap()
    });
    let menu_items = {
        let gui = app.world().non_send::<GuiRuntime>();
        assert_eq!(gui.popup_stack(), &[parent, dropdown, menu]);
        let list = gui.store().child_by_name(menu, "list").unwrap();
        gui.store().object(list).unwrap().children().to_vec()
    };
    assert_eq!(menu_items.len(), 2, "items belong to the actual menu list");
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let popup = gui.store().combo_popup(combo).unwrap();
        let item = gui.store().child_at(popup.list, 0).unwrap();
        gui.check_popups(Some(item));
        assert_eq!(gui.popup_stack(), &[parent, dropdown]);
        assert!(!gui.is_popup_menu_open());
        assert!(gui.store().contains(menu));
    }
    let reopened = with_commands(&mut app, |gui, commands| {
        gui.show_popup_menu(
            commands,
            &package,
            "PopupMenu",
            "PopupMenuItem",
            &["First", "Second"],
            Point::ZERO,
        )
        .unwrap()
    });
    assert_eq!(reopened, menu);
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.activate_menu_item(menu_items[0], None);
        gui.advance_menu_click(0.05);
        assert_eq!(gui.popup_stack(), &[parent, dropdown, menu]);
        gui.advance_menu_click(0.051);
        assert_eq!(gui.popup_stack(), &[parent, dropdown]);
        assert!(!gui.is_popup_menu_open());
        assert!(gui.store().contains(menu_items[0]));
        gui.hide_popup(None);
        assert!(gui.popup_stack().is_empty());
        assert!(gui.store().combo_popup(combo).is_none());
    }
}

#[test]
fn disabled_menu_item_stays_open_and_disposing_combo_reclaims_hidden_cached_subtree() {
    let (mut app, package, combo) = fixture();
    let menu = with_commands(&mut app, |gui, commands| {
        gui.show_popup_menu(
            commands,
            &package,
            "PopupMenu",
            "PopupMenuItem",
            &["Disabled"],
            Point::ZERO,
        )
        .unwrap()
    });
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let list = gui.store().child_by_name(menu, "list").unwrap();
        let item = gui.store().child_at(list, 0).unwrap();
        gui.store_mut().set_grayed(item, true);
        gui.activate_menu_item(item, None);
        gui.advance_menu_click(0.2);
        assert!(gui.is_popup_menu_open());
        assert_eq!(gui.store().list_selection(list), Some(-1));
        gui.hide_popup(None);
    }
    let dropdown = with_commands(&mut app, |gui, commands| {
        gui.show_combo_dropdown(commands, combo, &package).unwrap()
    });
    app.update();
    let entity = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(dropdown)
        .unwrap();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .hide_popup(None);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .remove(combo);
    app.world_mut()
        .run_system_once(cleanup_retained_popups)
        .unwrap();
    app.update();
    assert!(
        !app.world()
            .non_send::<GuiRuntime>()
            .store()
            .contains(dropdown)
    );
    assert!(app.world().get_entity(entity).is_err());
}

#[test]
fn first_dropdown_and_internal_root_created_together_render_the_referenced_package_background() {
    let (mut app, package, combo) = fixture_unspawned();
    let dropdown = with_commands(&mut app, |gui, commands| {
        gui.show_combo_dropdown(commands, combo, &package).unwrap()
    });
    app.update();
    app.update();
    let background = {
        let gui = app.world().non_send::<GuiRuntime>();
        let background = gui.store().child_by_name(dropdown, "n0").unwrap();
        gui.entity_of(background).unwrap()
    };
    let image = app
        .world()
        .get::<ImageNode>(background)
        .expect("the first popup needs an atlas quad as well as text");
    let path = app
        .world()
        .resource::<AssetServer>()
        .get_path(image.image.id())
        .unwrap();
    assert!(
        path.path().to_string_lossy().starts_with("correct/Basics"),
        "{path}"
    );
}

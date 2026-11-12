use super::*;
use fairygui_core::popup::MenuItem;

fn packages(registered: bool) -> (App, FuiPackage, FuiPackage, GObjectId, Handle<FuiAsset>) {
    let (mut app, _, _) = fixture();
    let host = FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-core/tests/fixtures/PopupHost.fui"),
    )
    .unwrap();
    let theme = FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-core/tests/fixtures/PopupTheme.fui"),
    )
    .unwrap();
    let host_handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: host.clone(),
            res_key: "host/Basics".into(),
        });
    let theme_handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: theme.clone(),
            res_key: "theme/Basics".into(),
        });
    let combo = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        if registered {
            gui.register_package(theme_handle.clone(), &theme);
        }
        let index = gui
            .create_root(host_handle, &host, "Demo_ComboBox")
            .unwrap();
        let root = gui.root_object(index).unwrap();
        gui.store.child_by_name(root, "n1").unwrap()
    };
    app.add_systems(Update, sync_movie_sources.before(spawn_pending_roots));
    app.update();
    (app, host, theme, combo, theme_handle)
}

fn path_for_image(app: &App, image: Entity) -> String {
    let handle = &app.world().get::<ImageNode>(image).unwrap().image;
    app.world()
        .resource::<AssetServer>()
        .get_path(handle.id())
        .unwrap()
        .path()
        .to_string_lossy()
        .into_owned()
}

fn image_object_path(app: &App, object: GObjectId) -> String {
    path_for_image(
        app,
        app.world()
            .non_send::<GuiRuntime>()
            .entity_of(object)
            .unwrap(),
    )
}

fn canonical(package: &FuiPackage, name: &str) -> String {
    format!(
        "ui://{}{}",
        package.id(),
        package
            .item(package.item_index_by_name(name).unwrap())
            .unwrap()
            .id
    )
}

#[test]
fn combo_uses_foreign_dropdown_and_returning_host_rows_and_preserves_each_atlas_owner_on_reparent()
{
    let (mut app, host, theme, combo, _) = packages(true);
    let dropdown = with_commands(&mut app, |gui, commands| {
        gui.show_combo_dropdown(commands, combo, &host).unwrap()
    });
    app.update();
    let (background, rows, list, original_entity) = {
        let gui = app.world().non_send::<GuiRuntime>();
        let popup = gui.store.combo_popup(combo).unwrap();
        let background = gui.store.child_by_name(dropdown, "n0").unwrap();
        let rows = gui.store.object(popup.list).unwrap().children().to_vec();
        assert_eq!(
            gui.store.object(dropdown).unwrap().source_package_id(),
            Some(theme.id())
        );
        assert_eq!(
            gui.store.object(rows[0]).unwrap().source_package_id(),
            Some(host.id())
        );
        assert_eq!(popup.item_url, Some(canonical(&host, "PopupMenuItem")));
        (background, rows, popup.list, gui.entity_of(dropdown))
    };
    assert!(image_object_path(&app, background).starts_with("theme/Basics"));
    let row_image = {
        let gui = app.world().non_send::<GuiRuntime>();
        let mut image = None;
        gui.store.for_each_descendant(rows[0], &mut |id| {
            if gui
                .store
                .object(id)
                .is_some_and(|object| matches!(object.control(), ControlData::Image(_)))
            {
                image = Some(id);
            }
        });
        image.unwrap()
    };
    assert!(image_object_path(&app, row_image).starts_with("host/Basics"));
    with_commands(&mut app, |gui, commands| {
        gui.close_combo_dropdown(commands, combo);
        let parent = gui
            .store
            .create(fairygui_core::package::ObjectType::Component);
        gui.store.set_parent(combo, Some(parent));
        assert_eq!(
            gui.show_combo_dropdown(commands, combo, &host),
            Some(dropdown)
        );
    });
    app.update();
    let gui = app.world().non_send::<GuiRuntime>();
    assert_eq!(gui.entity_of(dropdown), original_entity);
    assert_eq!(gui.store.object(list).unwrap().children(), rows);
    assert!(image_object_path(&app, background).starts_with("theme/Basics"));
}

#[test]
fn missing_foreign_template_leaves_no_partial_popup_and_can_retry_after_registration() {
    let (mut app, host, theme, combo, handle) = packages(false);
    let initial_count = app.world().non_send::<GuiRuntime>().store.len();
    for _ in 0..2 {
        assert!(
            with_commands(&mut app, |gui, commands| gui
                .show_combo_dropdown(commands, combo, &host))
            .is_none()
        );
        assert_eq!(
            app.world().non_send::<GuiRuntime>().store.len(),
            initial_count
        );
        assert!(
            app.world()
                .non_send::<GuiRuntime>()
                .popup_stack()
                .is_empty()
        );
    }
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .register_package(handle, &theme);
    let dropdown = with_commands(&mut app, |gui, commands| {
        gui.show_combo_dropdown(commands, combo, &host).unwrap()
    });
    app.update();
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .entity_of(dropdown)
            .is_some()
    );
}

#[test]
fn menu_aliases_share_canonical_cache_and_mixed_template_rows_keep_their_package_assets() {
    let (mut app, host, theme, _, _) = packages(true);
    let items = [
        MenuItem::action("action", "Action", 4),
        MenuItem {
            name: "separator".into(),
            ..MenuItem::separator()
        },
    ];
    let content = with_commands(&mut app, |gui, commands| {
        gui.show_popup_menu_items(
            commands,
            &host,
            PopupMenuTemplates {
                content: "ui://PopupTheme/PopupMenu",
                item: "ui://PopupHost/PopupMenuItem",
                separator: Some("ui://PopupTheme/VtSeperator"),
            },
            &items,
            Point::ZERO,
        )
        .unwrap()
    });
    app.update();
    let (row, separator, background, entity) = {
        let gui = app.world().non_send::<GuiRuntime>();
        (
            gui.popup_menu_item("action").unwrap().1,
            gui.popup_menu_item("separator").unwrap().1,
            gui.store.child_by_name(content, "n0").unwrap(),
            gui.entity_of(content),
        )
    };
    assert!(
        image_object_path(&app, background).starts_with("host/Basics"),
        "embedded foreign image overrides content's theme source"
    );
    assert!(image_object_path(&app, separator).starts_with("theme/Basics"));
    let content_url = canonical(&theme, "PopupMenu");
    let row_url = canonical(&host, "PopupMenuItem");
    let separator_url = canonical(&theme, "VtSeperator");
    with_commands(&mut app, |gui, commands| {
        gui.close_popup_menu(commands);
        assert_eq!(
            gui.show_popup_menu_items(
                commands,
                &host,
                PopupMenuTemplates {
                    content: &content_url,
                    item: &row_url,
                    separator: Some(&separator_url),
                },
                &items,
                Point::ZERO
            ),
            Some(content)
        );
        assert_eq!(gui.popup_menu_item("action").unwrap().1, row);
        assert_eq!(gui.popup_menu_item("separator").unwrap().1, separator);
        assert!(gui.remove_popup_menu_item("separator"));
        assert_eq!(
            gui.add_popup_menu_item(&host, items[1].clone()),
            Some(separator)
        );
        assert!(
            gui.show_popup_menu_items(
                commands,
                &host,
                PopupMenuTemplates {
                    content: "ui://Missing/PopupMenu",
                    item: &row_url,
                    separator: None,
                },
                &items[..1],
                Point::ZERO
            )
            .is_none()
        );
        assert_eq!(gui.popup_menu_content(), Some(content));
    });
    app.update();
    assert_eq!(
        app.world().non_send::<GuiRuntime>().entity_of(content),
        entity
    );
}

#[test]
fn initial_and_edited_icons_resolve_foreign_assets_without_binding_equal_local_item_ids() {
    let (mut app, host, theme, _, _) = packages(true);
    let theme_icon = canonical(&theme, "VtSeperator");
    let content = with_commands(&mut app, |gui, commands| {
        gui.show_popup_menu_items(
            commands,
            &host,
            PopupMenuTemplates {
                content: "PopupMenu",
                item: "PopupMenuItem",
                separator: None,
            },
            &[MenuItem {
                icon: Some(theme_icon),
                ..MenuItem::new("icon")
            }],
            Point::ZERO,
        )
        .unwrap()
    });
    app.update();
    let icon = {
        let gui = app.world().non_send::<GuiRuntime>();
        let row = gui.popup_menu_item("icon").unwrap().1;
        gui.entity_of(gui.store.child_by_name(row, "icon").unwrap())
            .unwrap()
    };
    let visual = app
        .world()
        .get::<MovieSource>(icon)
        .unwrap()
        .visual
        .unwrap();
    assert!(
        path_for_image(&app, visual).starts_with("theme/Basics"),
        "initial foreign icon uses its own atlas"
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .update_popup_menu_item("icon", |item| {
            item.icon = Some(canonical(&host, "VtSeperator"))
        });
    app.update();
    let visual = app
        .world()
        .get::<MovieSource>(icon)
        .unwrap()
        .visual
        .unwrap();
    assert!(path_for_image(&app, visual).starts_with("host/Basics"));
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .update_popup_menu_item("icon", |item| {
            item.icon = Some("ui://Missing/VtSeperator".into())
        });
    app.update();
    assert!(
        app.world()
            .get::<MovieSource>(icon)
            .unwrap()
            .visual
            .is_none()
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .update_popup_menu_item("icon", |item| item.icon = Some("icons/new.png".into()));
    app.update();
    let visual = app
        .world()
        .get::<MovieSource>(icon)
        .unwrap()
        .visual
        .unwrap();
    assert_eq!(
        path_for_image(&app, visual),
        "host/icons/new.png",
        "relative loader paths retain their source even inside a popup"
    );
    assert!(app.world().non_send::<GuiRuntime>().store.contains(content));
}

#[test]
fn normal_and_deferred_roots_render_embedded_foreign_components_using_source_handles() {
    let (mut app, host, theme, _, theme_handle) = packages(true);
    let index = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let index = gui
            .create_root(theme_handle.clone(), &theme, "Demo_Popup")
            .unwrap();
        gui.create_roots_async(theme_handle, &theme, &["Demo_Popup"], 1)
            .unwrap();
        index
    };
    {
        let mut gui = app.world_mut().remove_non_send::<GuiRuntime>().unwrap();
        gui.advance_builds(app.world().resource::<Assets<FuiAsset>>());
        let root = gui.roots.last().unwrap().object;
        let foreign = gui.store.child_by_name(root, "n0").unwrap();
        assert_eq!(
            gui.asset_handle_for(foreign),
            gui.package_handles.get(host.id()).cloned(),
            "deferred root records dependency handles before spawning views"
        );
        app.insert_non_send(gui);
    }
    app.update();
    let gui = app.world().non_send::<GuiRuntime>();
    for root in [
        gui.root_object(index).unwrap(),
        gui.roots.last().unwrap().object,
    ] {
        let foreign = gui.store.child_by_name(root, "n0").unwrap();
        let local = gui.store.child_by_name(root, "n1").unwrap();
        assert_eq!(
            gui.store.object(foreign).unwrap().source_package_id(),
            Some(host.id())
        );
        assert_eq!(
            gui.asset_handle_for(foreign),
            gui.package_handles.get(host.id()).cloned()
        );
        assert_eq!(
            gui.asset_handle_for(local),
            gui.package_handles.get(theme.id()).cloned()
        );
        assert!(
            gui.entity_of(gui.store.child_by_name(foreign, "title").unwrap())
                .is_some()
        );
    }
}

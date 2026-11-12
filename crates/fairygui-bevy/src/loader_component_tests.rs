use super::*;
use fairygui_core::package::ObjectType;

fn fixture(url: &str) -> (App, GObjectId, FuiPackage) {
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
    gui.register_package(handle, &package);
    let root = gui.create_root(menu_handle, &menu, "Main").unwrap();
    let root = gui.root_object(root).unwrap();
    let loader = gui.store.create(ObjectType::Loader);
    gui.store.set_parent(loader, Some(root));
    gui.store.set_size(loader, 301.0, 201.0);
    gui.store.set_loader_layout(loader, 1, 1, 1);
    gui.store.apply_icon_to(loader, url);
    gui.queue_subtree(loader);
    app.insert_non_send(gui).add_systems(
        Update,
        (
            sync_movie_sources,
            sync_loader_components,
            cleanup_retained_popups,
            spawn_pending_roots,
            spawn_pending_objects,
            content::sync_content_layout,
            sync_dirty,
            sync_appearance,
            sync_stage_hierarchy,
        )
            .chain()
            .in_set(crate::GuiSystems::Sync),
    );
    (app, loader, package)
}

fn content(app: &App, loader: GObjectId) -> GObjectId {
    app.world()
        .non_send::<GuiRuntime>()
        .loader_component(loader)
        .unwrap()
}

#[test]
fn initial_foreign_component_is_a_real_owned_subtree_with_correct_first_frame_assets_and_layout() {
    let (mut app, loader, package) = fixture("ui://Basics/Component12");
    app.update();
    let gui = app.world().non_send::<GuiRuntime>();
    let component = content(&app, loader);
    let object = gui.store.object(component).unwrap();
    assert_eq!(object.parent(), Some(loader));
    assert_eq!(object.size(), Size::new(400.0, 297.0));
    let expected = 201.0 / 297.0;
    assert!((object.scale_x() - expected).abs() < 0.0001);
    assert_eq!(object.x(), ((301.0 - 400.0 * expected) / 2.0).floor());
    assert_eq!(object.source_package_id(), Some(package.id()));
    let component_view = gui.entity_of(component).unwrap();
    let loader_view = gui.entity_of(loader).unwrap();
    assert_eq!(
        app.world().get::<ChildOf>(component_view).unwrap().parent(),
        loader_view
    );
    let background = gui.store.child_by_name(component, "n9").unwrap();
    let image = app
        .world()
        .get::<ImageNode>(gui.entity_of(background).unwrap())
        .unwrap();
    let path = app
        .world()
        .resource::<AssetServer>()
        .get_path(image.image.id())
        .unwrap();
    assert!(
        path.path().to_string_lossy().starts_with("correct/Basics"),
        "{path}"
    );
    assert_eq!(
        app.world().get::<MovieSource>(loader_view).unwrap().visual,
        None
    );
    assert!(gui.store.is_on_stage(component));
}

#[test]
fn resize_uses_component_relations_and_layout_changes_keep_the_same_content_and_view() {
    let (mut app, loader, _) = fixture("ui://Basics/Component12");
    app.update();
    let component = content(&app, loader);
    let view = app.world().non_send::<GuiRuntime>().entity_of(component);
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store.set_loader_use_resize(loader, true);
        gui.store.set_loader_layout(loader, 4, 2, 2);
        gui.store.set_size(loader, 200.0, 150.0);
    }
    app.update();
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        assert_eq!(gui.loader_component(loader), Some(component));
        assert_eq!(gui.entity_of(component), view);
        assert_eq!(
            gui.store.object(component).unwrap().size(),
            Size::new(200.0, 150.0)
        );
        let background = gui.store.child_by_name(component, "n9").unwrap();
        assert_eq!(
            gui.store.object(background).unwrap().size(),
            Size::new(200.0, 150.0)
        );
        gui.store.set_loader_auto_size(loader, true);
    }
    app.update();
    let gui = app.world().non_send::<GuiRuntime>();
    assert_eq!(
        gui.store.object(loader).unwrap().size(),
        Size::new(400.0, 297.0)
    );
    assert_eq!(
        gui.store.object(component).unwrap().size(),
        Size::new(400.0, 297.0)
    );
    assert_eq!(gui.store.object(component).unwrap().x(), 0.0);
}

#[test]
fn url_switches_dispose_components_images_and_movies_without_replacing_the_loader_or_leaving_stale_views()
 {
    let (mut app, loader, package) = fixture("ui://Basics/Component12");
    app.update();
    let component = content(&app, loader);
    let (old_views, owner_view) = {
        let gui = app.world().non_send::<GuiRuntime>();
        let mut objects = vec![component];
        gui.store
            .for_each_descendant(component, &mut |id| objects.push(id));
        (
            objects
                .into_iter()
                .filter_map(|id| gui.entity_of(id))
                .collect::<Vec<_>>(),
            gui.entity_of(loader).unwrap(),
        )
    };
    let movie = package
        .items()
        .iter()
        .find(|item| item.item_type == PackageItemType::MovieClip)
        .unwrap();
    for url in [
        format!("ui://{}{}", package.id(), movie.id),
        "ui://Basics/VtSeperator".into(),
        "ui://Basics/PopupMenuItem".into(),
        "ui://Missing/Component12".into(),
        String::new(),
    ] {
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store
            .apply_icon_to(loader, &url);
        app.update();
        let gui = app.world().non_send::<GuiRuntime>();
        assert_eq!(gui.entity_of(loader), Some(owner_view));
        assert!(!gui.store.contains(component));
        for view in &old_views {
            assert!(app.world().get_entity(*view).is_err());
        }
        if url == "ui://Basics/PopupMenuItem" {
            assert!(matches!(
                gui.store
                    .object(gui.loader_component(loader).unwrap())
                    .unwrap()
                    .control(),
                ControlData::Button(_)
            ));
        } else {
            assert!(gui.loader_component(loader).is_none());
        }
        if url.is_empty() {
            assert!(
                app.world()
                    .get::<MovieSource>(owner_view)
                    .unwrap()
                    .visual
                    .is_none()
            );
            assert!(
                app.world()
                    .get::<crate::movie::MovieClipPlayer>(owner_view)
                    .is_none()
            );
        }
    }
}

#[test]
fn retained_loader_preserves_contents_offstage_and_owner_disposal_reclaims_every_object_and_view() {
    let (mut app, loader, _) = fixture("ui://Basics/Component12");
    app.update();
    let component = content(&app, loader);
    let (component_view, loader_view) = {
        let gui = app.world().non_send::<GuiRuntime>();
        (
            gui.entity_of(component).unwrap(),
            gui.entity_of(loader).unwrap(),
        )
    };
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store
        .detach_from_parent(loader);
    app.update();
    assert_eq!(content(&app, loader), component);
    assert_eq!(
        app.world().get::<Visibility>(component_view),
        Some(&Visibility::Hidden)
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store
        .attach_to_stage(loader);
    app.update();
    assert_eq!(
        app.world().get::<Visibility>(component_view),
        Some(&Visibility::Inherited)
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store
        .remove(loader);
    app.update();
    let gui = app.world().non_send::<GuiRuntime>();
    assert!(!gui.store.contains(component));
    assert!(gui.loader_components.is_empty());
    assert!(app.world().get_entity(component_view).is_err());
    assert!(app.world().get_entity(loader_view).is_err());
}

#[test]
fn missing_package_retries_and_recursive_loader_reference_never_grows_the_object_store() {
    let (mut app, loader, _) = fixture("ui://PopupTheme/Demo_Loader");
    app.update();
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .loader_component(loader)
            .is_none()
    );
    let theme = FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-core/tests/fixtures/PopupTheme.fui"),
    )
    .unwrap();
    let _handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: theme,
            res_key: "theme/Basics".into(),
        });
    app.update();
    let component = content(&app, loader);
    let nested = {
        let gui = app.world().non_send::<GuiRuntime>();
        gui.store.child_by_name(component, "n1").unwrap()
    };
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store
        .apply_icon_to(nested, "ui://PopupTheme/Demo_Loader");
    app.update();
    let count = app.world().non_send::<GuiRuntime>().store.len();
    for _ in 0..5 {
        app.update();
    }
    let gui = app.world().non_send::<GuiRuntime>();
    assert_eq!(gui.store.len(), count);
    assert!(gui.loader_component(nested).is_none());
    assert_eq!(gui.failed_loader_components.len(), 1);
}

#[test]
fn nested_loader_contents_build_in_the_same_frame_and_clicks_reach_loaded_buttons() {
    use bevy::{
        input::mouse::MouseWheel,
        picking::hover::Hovered,
        ui::{ComputedStackIndex, RelativeCursorPosition},
        window::PrimaryWindow,
    };
    use fairygui_core::object::EventKind;
    let (mut app, loader, _) = fixture("ui://Basics/Demo_Loader");
    app.init_resource::<ButtonInput<MouseButton>>()
        .add_message::<MouseWheel>();
    crate::input::register(&mut app);
    app.configure_sets(
        Update,
        crate::GuiSystems::Input.before(crate::GuiSystems::Sync),
    );
    let mut window = Window {
        focused: true,
        ..Default::default()
    };
    window.set_cursor_position(Some(Vec2::new(10.0, 10.0)));
    app.world_mut().spawn((window, PrimaryWindow));
    app.update();
    let nested = {
        let gui = app.world().non_send::<GuiRuntime>();
        let root = gui.loader_component(loader).unwrap();
        gui.store.child_by_name(root, "n1").unwrap()
    };
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store
        .apply_icon_to(nested, "ui://Basics/PopupMenuItem");
    app.update();
    let (_button, view) = {
        let gui = app.world().non_send::<GuiRuntime>();
        let button = gui.loader_component(nested).unwrap();
        (button, gui.entity_of(button).unwrap())
    };
    app.world_mut().entity_mut(view).insert((
        Hovered(true),
        ComputedNode::default(),
        ComputedStackIndex(999),
        RelativeCursorPosition {
            cursor_over: true,
            normalized: Some(Vec2::ZERO),
        },
    ));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    app.update();
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let events: Vec<_> = gui.drain_events().collect();
        assert!(events.iter().any(|event| event.kind == EventKind::Click));
        assert!(events.iter().any(|event| event.pointer.is_some()));
        gui.store.apply_icon_to(loader, "");
    }
    app.update();
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .loader_components
            .is_empty(),
        "clearing an outer loader clears nested ownership in the same sync"
    );
}

#[test]
fn an_owned_components_transition_starts_on_stage_and_is_reclaimed_on_url_clear() {
    let (mut app, loader, _) = fixture("ui://Transition/PowerUp");
    let transition = FuiPackage::parse(
        "Transition",
        include_bytes!("../../fairygui-core/tests/fixtures/Transition.fui"),
    )
    .unwrap();
    let _handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: transition,
            res_key: "effect/Transition".into(),
        });
    app.update();
    let component = content(&app, loader);
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store
            .set_transition_auto_play(component, "t0", true)
            .unwrap();
        gui.store.detach_from_parent(loader);
        gui.store.attach_to_stage(loader);
        assert!(gui.store.transition_state(component, "t0").unwrap().0);
        gui.store.advance_transitions(0.2);
        assert!(gui.store.transition_state(component, "t0").unwrap().1 > 0.0);
        gui.store.apply_icon_to(loader, "");
    }
    app.update();
    assert!(
        !app.world()
            .non_send::<GuiRuntime>()
            .store
            .contains(component)
    );
}

#[test]
fn loaded_component_materializes_list_rows_and_applies_item_titles_after_template_titles() {
    let (mut app, loader, _) = fixture("ui://Basics/Demo_List");
    app.update();
    let gui = app.world().non_send::<GuiRuntime>();
    let root = gui.loader_component(loader).unwrap();
    let mut checked = 0;
    for (id, object) in gui.store.iter() {
        if !gui.is_descendant_of_or_self(id, root) {
            continue;
        }
        let ControlData::List(data) = object.control() else {
            continue;
        };
        assert_eq!(object.children().len(), data.items.len());
        for (index, item) in data.items.iter().enumerate() {
            let Some(title) = &item.title else {
                continue;
            };
            let row = gui.store.child_at(id, index).unwrap();
            let target = gui.store.child_by_name(row, "title").unwrap_or(row);
            let actual = gui
                .store
                .object(target)
                .unwrap()
                .control()
                .as_text_field()
                .unwrap();
            assert_eq!(&actual.text, title);
            assert!(gui.entity_of(row).is_some());
            checked += 1;
        }
    }
    assert!(
        checked > 0,
        "official serialized list data exercises content initialization"
    );
}

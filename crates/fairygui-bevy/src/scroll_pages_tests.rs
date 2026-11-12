//! Exported Bag pagination through actual materialization and retained views.
use super::*;
use fairygui_core::geom::Size;

#[test]
fn actual_bevy_layout_honors_empty_model_tail_and_updates_it_after_resize() {
    let (mut app, _, list) = super::list_bar_tests::fixture_with_layout(true);
    app.add_systems(Update, sync_scroll_extents.after(sync_dirty));
    let row = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        for item in gui.store().list_items(list).unwrap() {
            gui.store_mut().remove(item);
        }
        let row = gui
            .store_mut()
            .create(fairygui_core::package::ObjectType::Graph);
        gui.store_mut().set_parent(row, Some(list));
        gui.store_mut().set_size(row, 20., 20.);
        gui.queue_subtree(row);
        gui.store_mut().set_size(list, 100., 80.);
        gui.store_mut().layout_scroll_bars(list);
        gui.store_mut()
            .update_scroll_content_size(list, Size::new(100., 240.));
        row
    };
    app.update();
    app.update();
    let (view, extent) = {
        let gui = app.world().non_send::<GuiRuntime>();
        let view = gui.entity_of(list).unwrap();
        let extent = app
            .world()
            .get::<Children>(view)
            .unwrap()
            .iter()
            .find(|&entity| {
                app.world()
                    .get::<scroll_extent::ScrollExtent>(entity)
                    .is_some()
            })
            .unwrap();
        assert!(app.world().get::<GObjectRef>(extent).is_none());
        assert_eq!(
            app.world().get::<bevy::picking::Pickable>(extent),
            Some(&bevy::picking::Pickable::IGNORE)
        );
        (view, extent)
    };
    for (content, position) in [(240., 160.), (400., 320.)] {
        {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            gui.store_mut()
                .update_scroll_content_size(list, Size::new(100., content));
            gui.store_mut().set_scroll_pos(list, 0., position, false);
        }
        app.update();
        app.update();
        let node = app.world().get::<ComputedNode>(view).unwrap();
        assert_eq!(
            node.scroll_position.y, position,
            "actual Bevy layout must not clamp to the small row's extent"
        );
        assert_eq!(
            app.world().get::<Node>(extent).unwrap().height,
            Val::Px(content)
        );
        assert_eq!(
            app.world()
                .non_send::<GuiRuntime>()
                .store()
                .list_item_count(list),
            1
        );
    }
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .remove(list);
    app.update();
    assert!(app.world().get_entity(extent).is_err());
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .entity_of(row)
            .is_none()
    );
}

#[test]
fn preparing_a_new_subtree_does_not_relayout_an_unrelated_live_list() {
    let package = FuiPackage::parse(
        "Bag",
        include_bytes!("../../fairygui-core/tests/fixtures/Bag.fui"),
    )
    .unwrap();
    let mut gui = GuiRuntime::default();
    let index = gui
        .create_root(Handle::default(), &package, "BagWin")
        .unwrap();
    let root = gui.root_object(index).unwrap();
    gui.prepare_subtree(&package, root);
    let list = gui.store().child_by_name(root, "list").unwrap();
    let row = gui.store().list_item_at(list, 0).unwrap();
    gui.store_mut().set_xy(row, 1234., 456.);
    gui.store_mut()
        .update_scroll_content_size(list, Size::new(10000., 1000.));
    let new = gui
        .create_root(Handle::default(), &package, "Main")
        .unwrap();
    let new = gui.root_object(new).unwrap();
    gui.prepare_subtree(&package, new);
    assert_eq!(
        gui.store().object(row).unwrap().position(),
        fairygui_core::geom::Point::new(1234., 456.)
    );
    assert_eq!(
        gui.store().scroll_pane(list).unwrap().content_size(),
        Size::new(10000., 1000.)
    );
}

#[test]
fn bag_page_binding_initializes_after_rows_for_roots_and_dynamic_windows_and_reuses_views() {
    for as_window in [false, true] {
        let package = FuiPackage::parse(
            "Bag",
            include_bytes!("../../fairygui-core/tests/fixtures/Bag.fui"),
        )
        .unwrap();
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<FuiAsset>()
            .init_asset::<Image>()
            .init_asset::<Font>()
            .init_resource::<GuiFont>();
        let handle = app
            .world_mut()
            .resource_mut::<Assets<FuiAsset>>()
            .add(FuiAsset {
                package: package.clone(),
                res_key: "ui/Bag".into(),
            });
        let mut gui = GuiRuntime::default();
        let (owner, window) = if as_window {
            gui.create_root(handle.clone(), &package, "Main").unwrap();
            let index = gui
                .show_window(&package, "BagWin", true, Size::new(1280., 720.))
                .unwrap();
            (gui.window(index).unwrap().owner(), Some(index))
        } else {
            let index = gui.create_root(handle, &package, "BagWin").unwrap();
            (gui.root_object(index).unwrap(), None)
        };
        let list = gui.store().child_by_name(owner, "list").unwrap();
        let ControlData::List(data) = gui.store().object(list).unwrap().control() else {
            panic!("Bag list")
        };
        let template = gui
            .resolve_template(&package, data.default_item.as_deref().unwrap())
            .unwrap();
        for i in 0..45 {
            let row = gui.create_template(&template).unwrap();
            gui.store_mut().set_parent(row, Some(list));
            gui.store_mut().set_object_text(row, &i.to_string());
        }
        let name = gui
            .store()
            .object(owner)
            .unwrap()
            .control()
            .component()
            .unwrap()
            .controllers[0]
            .name
            .clone();
        gui.store_mut().set_controller_page_index(owner, &name, 1);
        app.insert_non_send(gui).add_systems(
            Update,
            (
                spawn_pending_roots,
                spawn_pending_objects,
                sync_dirty,
                sync_stage_hierarchy,
            )
                .chain(),
        );
        app.update();
        app.update();
        let (entity, rows, view) = {
            let gui = app.world().non_send::<GuiRuntime>();
            let pane = gui.store().scroll_pane(list).unwrap();
            assert_eq!(gui.store().scroll_page_controller(list), Some((owner, 0)));
            assert_eq!(gui.store().list_item_count(list), 45);
            assert!(pane.max_scroll_pos().x > pane.view_size().width);
            assert_eq!(pane.content_size().width, pane.view_size().width * 3.);
            assert_eq!(pane.max_scroll_pos().x, pane.view_size().width * 2.);
            assert_eq!(pane.pos().x, pane.view_size().width);
            assert_eq!(pane.display_pos().x, 0.);
            let rows: Vec<_> = gui
                .store()
                .list_items(list)
                .unwrap()
                .into_iter()
                .map(|id| (id, gui.entity_of(id).unwrap()))
                .collect();
            (gui.entity_of(list).unwrap(), rows, pane.view_size().width)
        };
        for dt in [0.125, 0.125, 0.25] {
            app.world_mut()
                .non_send_mut::<GuiRuntime>()
                .store_mut()
                .advance_scrolls(dt);
            app.update();
            let gui = app.world().non_send::<GuiRuntime>();
            assert_eq!(
                app.world().get::<ScrollPosition>(entity).unwrap().0.x,
                gui.store().scroll_pane(list).unwrap().display_pos().x
            );
            assert!(
                rows.iter()
                    .all(|(id, entity)| gui.entity_of(*id) == Some(*entity))
            );
        }
        assert_eq!(app.world().get::<ScrollPosition>(entity).unwrap().0.x, view);
        {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            let before = gui.store().list_selections(list);
            gui.store_mut().set_controller_page_index(owner, &name, 2);
            assert_eq!(gui.store().scroll_pane(list).unwrap().pos().x, view * 2.);
            gui.store_mut().set_scroll_mouse_wheel_enabled(list, true);
            gui.store_mut().scroll_mouse_wheel(list, -1.);
            assert_eq!(gui.store().scroll_current_page_x(list), Some(1));
            assert_eq!(gui.store().list_selections(list), before);
            if let Some(window) = window {
                gui.hide_window(window);
                gui.show_existing_window(window);
            }
        }
        app.update();
        let gui = app.world().non_send::<GuiRuntime>();
        assert_eq!(gui.entity_of(list), Some(entity));
        assert!(
            rows.iter()
                .all(|(id, entity)| gui.entity_of(*id) == Some(*entity))
        );
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .remove(owner);
        app.update();
        let gui = app.world().non_send::<GuiRuntime>();
        assert_eq!(gui.store().scroll_page_controller(list), None);
        assert!(app.world().get_entity(entity).is_err());
        assert!(
            rows.iter()
                .all(|(_, entity)| app.world().get_entity(*entity).is_err())
        );
    }
}

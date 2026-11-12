use super::*;

fn fixture() -> (App, GObjectId, GObjectId) {
    let package = FuiPackage::parse(
        "VirtualList",
        include_bytes!("../../fairygui-demo/assets/ui/VirtualList.fui"),
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
            res_key: "ui/VirtualList".into(),
        });
    let mut gui = GuiRuntime::default();
    let index = gui.create_root(handle, &package, "Main").unwrap();
    let root = gui.root_object(index).unwrap();
    let list = gui.store().child_by_name(root, "mailList").unwrap();
    gui.enable_virtual_list(list, |store, index, row| {
        store.set_object_text(row, &format!("Row {index}"));
    })
    .unwrap();
    gui.set_virtual_item_count(list, 1000).unwrap();
    app.insert_non_send(gui).add_systems(
        Update,
        (
            sync_virtual_lists,
            spawn_pending_roots,
            spawn_pending_objects,
            sync_dirty,
            sync_stage_hierarchy,
        )
            .chain(),
    );
    app.update();
    (app, root, list)
}

#[test]
fn virtual_thousand_rows_refresh_automatically_retain_views_and_dispose_their_pool() {
    let (mut app, root, list) = fixture();
    let (views, initial_objects) = {
        let gui = app.world().non_send::<GuiRuntime>();
        let info = gui.store().virtual_list_info(list).unwrap();
        assert!(info.active_count > 1 && info.active_count < 30, "{info:?}");
        assert_eq!(info.item_count, 1000);
        let views: Vec<_> = gui
            .store()
            .list_items(list)
            .unwrap()
            .into_iter()
            .map(|id| (id, gui.entity_of(id).unwrap()))
            .collect();
        for &(id, _) in &views {
            let index = gui.store().virtual_item_index(list, id).unwrap();
            let ControlData::Button(data) = gui.store().object(id).unwrap().control() else {
                unreachable!()
            };
            assert_eq!(data.title, format!("Row {index}"));
        }
        (views, gui.store().len())
    };
    for fraction in [0.5, 1., 0.2, 0.] {
        {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            let max = gui.store().scroll_pane(list).unwrap().max_scroll_pos();
            gui.store_mut()
                .set_scroll_pos(list, 0., max.y * fraction, false);
        }
        app.update();
        let gui = app.world().non_send::<GuiRuntime>();
        assert!(gui.store().len() <= initial_objects + 40);
        assert!(
            views
                .iter()
                .all(|&(id, view)| gui.entity_of(id) == Some(view))
        );
        assert!(gui.store().virtual_list_info(list).unwrap().active_count < 30);
    }
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        assert!(
            gui.store_mut()
                .add_list_selection_with_scroll(list, 500, true)
        );
    }
    app.update();
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let row = gui.store().virtual_item_object(list, 500).unwrap();
        assert_eq!(gui.store().list_item_of(row).unwrap().2, 500);
        assert_eq!(gui.store().button_selected(row), Some(true));
        gui.set_virtual_item_count(list, 0).unwrap();
        assert_eq!(gui.store().list_item_count(list), 0);
        assert!(gui.store().virtual_list_info(list).unwrap().pooled_count > 0);
    }
    app.update();
    for &(_, view) in &views {
        assert_eq!(
            app.world().get::<Visibility>(view),
            Some(&Visibility::Hidden)
        );
    }
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .remove(root);
    app.update();
    assert!(
        views
            .iter()
            .all(|&(_, view)| app.world().get_entity(view).is_err())
    );
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .virtual_renderers
            .is_empty()
    );
}

#[test]
fn exported_loop_list_retains_views_and_native_record_mapping_through_wraps_and_pooling() {
    let package = FuiPackage::parse(
        "LoopList",
        include_bytes!("../../fairygui-demo/assets/ui/LoopList.fui"),
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
            res_key: "ui/LoopList".into(),
        });
    let mut gui = GuiRuntime::default();
    let index = gui.create_root(handle, &package, "Main").unwrap();
    let root = gui.root_object(index).unwrap();
    let list = gui.store().child_by_name(root, "list").unwrap();
    let icons: Vec<_> = (1..=5)
        .map(|i| {
            gui.registry()
                .item_url("LoopList", &format!("n{i}"))
                .unwrap()
        })
        .collect();
    let renderer_icons = icons.clone();
    gui.enable_loop_virtual_list(list, move |store, index, row| {
        store.set_pivot(row, 0.5, 0.5, false);
        store.apply_icon_to(row, &renderer_icons[index]);
    })
    .unwrap();
    gui.set_virtual_item_count(list, 5).unwrap();
    app.insert_non_send(gui).add_systems(
        Update,
        (
            sync_virtual_lists,
            spawn_pending_roots,
            spawn_pending_objects,
            sync_dirty,
            sync_stage_hierarchy,
        )
            .chain(),
    );
    app.update();
    let mut views = HashMap::new();
    for x in [0., 1e6, 5., 0., 350., 1e6] {
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_scroll_pos_animated(list, x, 0., false);
        app.update();
        let gui = app.world().non_send::<GuiRuntime>();
        let info = gui.store().virtual_list_info(list).unwrap();
        assert!(info.looping);
        assert_eq!((info.item_count, info.real_item_count), (5, 30));
        assert!(info.active_count > 1 && info.active_count < 20);
        assert!(!gui.store().scroll_pane(list).unwrap().bounceback_effect);
        for row in gui.store().list_items(list).unwrap() {
            let index = gui.store().virtual_item_index(list, row).unwrap();
            let record = gui.store().virtual_item_record_index(list, row).unwrap();
            assert_eq!(index, record % 5);
            let ControlData::Button(button) = gui.store().object(row).unwrap().control() else {
                unreachable!()
            };
            assert_eq!(button.icon.as_ref(), Some(&icons[index]));
            let entity = gui.entity_of(row).unwrap();
            if let Some(old) = views.insert(row, entity) {
                assert_eq!(old, entity);
            }
            assert_eq!(
                app.world().get::<Visibility>(entity),
                Some(&Visibility::Inherited)
            );
        }
        let ControlData::List(data) = gui.store().object(list).unwrap().control() else {
            unreachable!()
        };
        if data.children_render_order == 2 {
            let apex = gui
                .store()
                .list_item_at(list, data.apex_index as usize)
                .unwrap();
            let z = app
                .world()
                .get::<ZIndex>(gui.entity_of(apex).unwrap())
                .unwrap()
                .0;
            assert!(gui.store().list_items(list).unwrap().iter().all(|&r| {
                app.world()
                    .get::<ZIndex>(gui.entity_of(r).unwrap())
                    .unwrap()
                    .0
                    <= z
            }));
        }
    }
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .set_virtual_item_count(list, 0)
        .unwrap();
    app.update();
    for &entity in views.values() {
        assert_eq!(
            app.world().get::<Visibility>(entity),
            Some(&Visibility::Hidden)
        );
    }
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .set_virtual_item_count(list, 5)
        .unwrap();
    app.update();
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .list_item_count(list)
            > 0
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .remove(root);
    app.update();
    assert!(
        views
            .values()
            .all(|&view| app.world().get_entity(view).is_err())
    );
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .virtual_renderers
            .is_empty()
    );
}

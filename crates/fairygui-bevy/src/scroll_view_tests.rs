//! No renderer or window: exercise retained ScrollPosition and virtual ranges.
use super::*;
use fairygui_core::{geom::Size, package::ObjectType, widget::ListLayoutType};

#[test]
fn exported_snap_flag_aligns_retained_commands_and_refreshes_resized_item_geometry() {
    let package = FuiPackage::parse(
        "ListEffect",
        include_bytes!("../../fairygui-demo/assets/ui/ListEffect.fui"),
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
            res_key: "ui/ListEffect".into(),
        });
    let mut gui = GuiRuntime::default();
    let index = gui.create_root(handle, &package, "Main").unwrap();
    let root = gui.root_object(index).unwrap();
    let list = gui.store().child_by_name(root, "mailList").unwrap();
    assert_eq!(gui.store().scroll_snap_to_item(list), Some(true));
    gui.store_mut().set_size(list, 200., 100.);
    let mut rows = Vec::new();
    for height in [30., 60., 80., 100., 120.] {
        let row = gui.store_mut().create(ObjectType::Component);
        gui.store_mut().set_parent(row, Some(list));
        gui.store_mut().set_size(row, 180., height);
        rows.push(row);
    }
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
    let (entity, views) = {
        let gui = app.world().non_send::<GuiRuntime>();
        (
            gui.entity_of(list).unwrap(),
            rows.iter()
                .map(|&r| gui.entity_of(r).unwrap())
                .collect::<Vec<_>>(),
        )
    };
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_scroll_pos_animated(list, 0., 55., true);
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_pane(list)
            .unwrap()
            .pos()
            .y,
        30.
    );
    for dt in [0.125, 0.375] {
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .advance_scrolls(dt);
        app.update();
        let gui = app.world().non_send::<GuiRuntime>();
        let display = gui.store().scroll_pane(list).unwrap().display_pos();
        assert_eq!(
            app.world().get::<ScrollPosition>(entity).unwrap().0.y,
            display.y
        );
        assert!(
            rows.iter()
                .zip(&views)
                .all(|(&r, &v)| gui.entity_of(r) == Some(v))
        );
    }
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut().set_size(rows[1], 180., 100.);
        gui.store_mut().layout_list(list);
        gui.store_mut()
            .set_scroll_pos_animated(list, 0., 80., false);
        assert_eq!(
            gui.store().scroll_pane(list).unwrap().pos().y,
            130.,
            "strict half-item boundary uses resized geometry"
        );
        assert!(gui.store_mut().set_scroll_snap_to_item(list, false));
        assert_eq!(
            gui.store().scroll_pane(list).unwrap().pos().y,
            130.,
            "flag assignment is inert"
        );
        gui.store_mut()
            .set_scroll_pos_animated(list, 0., 135.25, false);
    }
    app.update();
    assert_eq!(app.world().get::<ScrollPosition>(entity).unwrap().0.y, 136.);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .remove(root);
    app.update();
    assert!(app.world().get_entity(entity).is_err());
    assert!(views.iter().all(|&v| app.world().get_entity(v).is_err()));
}

#[test]
fn scroll_view_projects_display_offset_and_visible_range_through_an_actual_exported_list() {
    let package = FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui"),
    )
    .unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<FuiAsset>()
        .init_asset::<Image>()
        .init_asset::<Font>();
    app.init_resource::<GuiFont>();
    let handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: package.clone(),
            res_key: "ui/Basics".into(),
        });
    let mut gui = GuiRuntime::default();
    let root_index = gui.create_root(handle, &package, "Demo_List").unwrap();
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
    let list = app.world().non_send::<GuiRuntime>().store().iter().find_map(|(id, o)| {
        matches!(o.control(), ControlData::List(data) if data.layout == ListLayoutType::SingleColumn
            && data.component.scroll_pane.is_some()).then_some(id)
    }).unwrap();
    let index;
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut().set_size(list, 200., 100.);
        for _ in 0..8 {
            let row = gui.store_mut().create(ObjectType::Component);
            gui.store_mut().set_parent(row, Some(list));
            gui.store_mut().set_size(row, 180., 40.);
            gui.queue_subtree(row);
        }
        gui.store_mut().layout_list(list);
        index = gui.store().list_item_count(list) - 1;
        gui.store_mut().set_scroll_pos(list, 0., 0., false);
        gui.register_virtual_list(list);
    }
    app.update();
    let entity = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(list)
        .unwrap();
    assert_eq!(
        app.world().get::<Node>(entity).unwrap().overflow,
        Overflow::scroll(),
        "exported list overflow must actually enable Bevy scrolling and clipping"
    );
    let sizes: Vec<Size> = {
        let gui = app.world().non_send::<GuiRuntime>();
        gui.store()
            .list_items(list)
            .unwrap()
            .into_iter()
            .map(|r| gui.store().object(r).unwrap().size())
            .collect()
    };
    let initial_range = app
        .world()
        .non_send::<GuiRuntime>()
        .virtual_list_range(list, &sizes, 0)
        .unwrap();
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.take_scrolled_virtual_lists();
        assert!(
            gui.store_mut()
                .scroll_list_item_to_view(list, index, true, false)
        );
        let pane = gui.store().scroll_pane(list).unwrap();
        assert!(pane.pos().y > 0.);
        assert_eq!(pane.display_pos().y, 0.);
        assert_eq!(
            gui.virtual_list_range(list, &sizes, 0).unwrap(),
            initial_range
        );
    }
    app.update();
    assert_eq!(
        app.world().get::<ScrollPosition>(entity).unwrap().0,
        Vec2::ZERO
    );
    for dt in [0.125, 0.125, 0.25] {
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .advance_scrolls(dt);
        app.update();
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let display = gui.store().scroll_pane(list).unwrap().display_pos();
        assert!(gui.take_scrolled_virtual_lists().contains(&list));
        assert_eq!(
            app.world().get::<ScrollPosition>(entity).unwrap().0,
            Vec2::new(display.x, display.y)
        );
    }
    let gui = app.world().non_send::<GuiRuntime>();
    assert!(!gui.store().scroll_pane(list).unwrap().is_animating());
    assert_ne!(
        gui.virtual_list_range(list, &sizes, 0).unwrap(),
        initial_range
    );
    let root = gui.root_object(root_index).unwrap();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .remove(root);
    // The runtime's normal Sync path reclaims the owner with its animated state.
    app.update();
    assert!(app.world().get_entity(entity).is_err());
}

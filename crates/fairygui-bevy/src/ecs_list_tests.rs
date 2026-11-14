use super::*;
use crate::{
    FairyGuiPlugin, FairyGuiRoot, FuiAsset, GuiQuery, GuiRootInstance, GuiSystems, GuiText,
    GuiTransform, GuiWorldExt,
};
use fairygui_core::package::{FuiPackage, ObjectType};

fn fixture(package: &str, bytes: &[u8], name: &str) -> (App, Entity, GObjectId, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let package = FuiPackage::parse(package, bytes).unwrap();
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            res_key: format!("ui/{}", package.name()),
            package,
        });
    let owner = app.world_mut().spawn(FairyGuiRoot::new(asset, "Main")).id();
    app.update();
    let root = app.world().get::<GuiRootInstance>(owner).unwrap().object;
    let runtime = app.world().non_send::<GuiRuntime>();
    let list = runtime.store().child_by_name(root, name).unwrap();
    let view = runtime.entity_of(list).unwrap();
    (app, owner, list, view)
}

fn config(count: usize) -> GuiVirtualList {
    GuiVirtualList::new(count, |index, row| {
        row.set_text(&format!("Mail {index}"));
        row.set_controller_page("IsRead", i32::from(index % 2 == 0));
        row.set_controller_page("c1", i32::from(index % 3 == 0));
        if let Some(mut time) = row.child_by_path("timeText") {
            time.set_text("ECS row");
        }
        assert!(row.child_by_path("missing/child").is_none());
        row.set_size(Vec2::new(row.size().x, 45. + (index % 3) as f32 * 6.));
    })
}

#[test]
fn writable_selection_waits_for_content_and_tracks_native_and_row_edits() {
    use crate::{GuiButtonSelected, GuiListSelection};
    let (mut app, _, list, view) = fixture(
        "VirtualList",
        include_bytes!("../../fairygui-demo/assets/ui/VirtualList.fui"),
        "mailList",
    );
    app.world_mut().entity_mut(view).insert((
        GuiListContent::new(8, |_, row| row.set_text("Mail")),
        GuiListSelection(vec![2]),
    ));
    app.update();
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .list_selections(list),
        Some(vec![2])
    );
    let rows = app
        .world()
        .get::<GuiListState>(view)
        .unwrap()
        .items()
        .to_vec();
    assert!(app.world().get::<GuiButtonSelected>(rows[2]).unwrap().0);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_list_selection_mode(list, fairygui_core::widget::ListSelectionMode::Multiple);
    app.world_mut().get_mut::<GuiListSelection>(view).unwrap().0 = vec![5, 1, 5, usize::MAX];
    app.update();
    assert_eq!(app.world().get::<GuiListSelection>(view).unwrap().0, [1, 5]);
    for (index, &row) in rows.iter().enumerate() {
        assert_eq!(
            app.world().get::<GuiButtonSelected>(row).unwrap().0,
            [1, 5].contains(&index)
        );
    }
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_list_selection(list, 3);
    app.update();
    assert_eq!(
        app.world().get::<GuiListSelection>(view).unwrap().0,
        [3],
        "native changes must not replay old ECS writes"
    );
    app.world_mut()
        .get_mut::<GuiButtonSelected>(rows[4])
        .unwrap()
        .0 = true;
    app.update();
    assert_eq!(app.world().get::<GuiListSelection>(view).unwrap().0, [3, 4]);
    app.world_mut()
        .get_mut::<GuiListSelection>(view)
        .unwrap()
        .0
        .clear();
    app.update();
    assert!(
        app.world()
            .get::<GuiListSelection>(view)
            .unwrap()
            .0
            .is_empty()
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_list_selection_mode(list, fairygui_core::widget::ListSelectionMode::Single);
    app.world_mut().get_mut::<GuiListSelection>(view).unwrap().0 = vec![6, 2, usize::MAX];
    app.update();
    assert_eq!(app.world().get::<GuiListSelection>(view).unwrap().0, [2]);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_list_selection_mode(list, fairygui_core::widget::ListSelectionMode::None);
    app.world_mut().get_mut::<GuiListSelection>(view).unwrap().0 = vec![4];
    app.update();
    assert!(
        app.world()
            .get::<GuiListSelection>(view)
            .unwrap()
            .0
            .is_empty()
    );
    let tick = app
        .world()
        .entity(view)
        .get_ref::<GuiListSelection>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .entity(view)
            .get_ref::<GuiListSelection>()
            .unwrap()
            .last_changed(),
        tick
    );
    assert!(
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .drain_events()
            .all(|event| !matches!(
                event.kind,
                crate::core::object::EventKind::ClickItem
                    | crate::core::object::EventKind::StateChanged
            ))
    );
}

#[test]
fn writable_virtual_selection_retains_offscreen_indices_without_scrolling_and_survives_reconfiguration()
 {
    use crate::{GuiButtonSelected, GuiListSelection};
    let (mut app, _, list, view) = fixture(
        "VirtualList",
        include_bytes!("../../fairygui-demo/assets/ui/VirtualList.fui"),
        "mailList",
    );
    app.world_mut()
        .entity_mut(view)
        .insert((config(1000), GuiListSelection(vec![900])));
    app.update();
    let runtime = app.world().non_send::<GuiRuntime>();
    assert_eq!(runtime.store().list_selections(list), Some(vec![900]));
    assert!(runtime.store().virtual_item_object(list, 900).is_none());
    assert_eq!(
        runtime.store().scroll_pane(list).unwrap().pos(),
        crate::core::Point::ZERO
    );
    assert!(
        runtime
            .store()
            .virtual_list_info(list)
            .unwrap()
            .active_count
            < 30
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .scroll_list_item_to_view(list, 900, false, false);
    app.update();
    let runtime = app.world().non_send::<GuiRuntime>();
    let row = runtime.store().virtual_item_object(list, 900).unwrap();
    assert!(
        app.world()
            .get::<GuiButtonSelected>(runtime.entity_of(row).unwrap())
            .unwrap()
            .0
    );
    app.world_mut()
        .get_mut::<GuiVirtualList>(view)
        .unwrap()
        .item_count = 10;
    app.world_mut().get_mut::<GuiListSelection>(view).unwrap().0 = vec![2];
    app.update();
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .list_selections(list),
        Some(vec![2])
    );
    assert_eq!(app.world().get::<GuiListSelection>(view).unwrap().0, [2]);
}

#[test]
fn writable_row_visibility_and_size_reflow_list_geometry_in_the_same_frame() {
    use crate::{GuiButtonSelected, GuiDisplay, GuiListLayout, GuiListSelection, GuiListSettings};
    let (mut app, _owner, list, view) = fixture(
        "ListEffect",
        include_bytes!("../../fairygui-demo/assets/ui/ListEffect.fui"),
        "mailList",
    );
    app.world_mut()
        .entity_mut(view)
        .insert(GuiListContent::new(10, |_, row| row.set_text("Mail")));
    app.update();
    let rows = app
        .world()
        .get::<GuiListState>(view)
        .unwrap()
        .items()
        .to_vec();
    let height = app.world().get::<GuiTransform>(rows[0]).unwrap().size.y;
    let width = app.world().get::<GuiTransform>(view).unwrap().size.x + 37.;
    app.world_mut()
        .get_mut::<GuiButtonSelected>(rows[1])
        .unwrap()
        .0 = true;
    // These components are synchronized in different passes. Layout must see
    // the final values together, regardless of entity iteration order.
    app.world_mut()
        .get_mut::<GuiListLayout>(view)
        .unwrap()
        .line_gap = 7;
    app.world_mut()
        .get_mut::<GuiListSettings>(view)
        .unwrap()
        .fold_invisible_items = true;
    app.world_mut()
        .get_mut::<GuiDisplay>(rows[1])
        .unwrap()
        .visible = false;
    app.world_mut()
        .get_mut::<GuiTransform>(rows[0])
        .unwrap()
        .size
        .y += 17.;
    app.world_mut()
        .get_mut::<GuiTransform>(view)
        .unwrap()
        .size
        .x = width;
    app.update();
    let transform = |entity| *app.world().get::<GuiTransform>(entity).unwrap();
    assert_eq!(transform(rows[2]).position.y, height + 17. + 7.);
    assert_eq!(transform(rows[0]).size.x, transform(rows[2]).size.x);
    assert_eq!(
        transform(rows[0]).size.x,
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_pane(list)
            .unwrap()
            .view_size()
            .width
    );
    let last = transform(rows[9]);
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_pane(list)
            .unwrap()
            .content_size()
            .height,
        last.position.y + last.size.y,
    );
    assert_eq!(app.world().get::<GuiListState>(view).unwrap().items(), rows);
    assert_eq!(app.world().get::<GuiListSelection>(view).unwrap().0, [1]);

    // Later edits must also invalidate layout when its settings are unchanged.
    app.world_mut()
        .get_mut::<GuiDisplay>(rows[0])
        .unwrap()
        .visible = false;
    app.world_mut()
        .get_mut::<GuiDisplay>(rows[1])
        .unwrap()
        .visible = true;
    app.world_mut()
        .get_mut::<GuiTransform>(rows[1])
        .unwrap()
        .size
        .y += 11.;
    app.update();
    assert_eq!(
        app.world().get::<GuiTransform>(rows[1]).unwrap().position.y,
        0.
    );
    assert_eq!(
        app.world().get::<GuiTransform>(rows[2]).unwrap().position.y,
        height + 11. + 7.
    );
    let ticks: Vec<_> = rows
        .iter()
        .map(|&row| {
            app.world()
                .entity(row)
                .get_ref::<GuiTransform>()
                .unwrap()
                .last_changed()
        })
        .collect();
    app.update();
    for (&row, tick) in rows.iter().zip(ticks) {
        assert_eq!(
            app.world()
                .entity(row)
                .get_ref::<GuiTransform>()
                .unwrap()
                .last_changed(),
            tick
        );
    }
}

#[test]
fn child_view_snapshots_track_native_geometry_hidden_rows_and_display_scroll_without_tick_churn() {
    use bevy::ecs::system::SystemState;
    let (mut app, owner, _, list) = fixture(
        "ListEffect",
        include_bytes!("../../fairygui-demo/assets/ui/ListEffect.fui"),
        "mailList",
    );
    app.world_mut()
        .entity_mut(list)
        .insert(GuiListContent::new(10, |_, row| row.set_text("Mail")));
    app.update();
    let rows = app
        .world()
        .get::<GuiListState>(list)
        .unwrap()
        .items()
        .to_vec();
    assert_eq!(rows.len(), 10);
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    let gui = query.get(app.world()).unwrap();
    assert_eq!(
        gui.get(list).unwrap().children_in_view().unwrap(),
        &rows[..4]
    );
    assert_eq!(gui.is_child_in_view(list, rows[0]), Some(true));
    assert_eq!(gui.is_child_in_view(list, rows[4]), Some(false));
    assert_eq!(
        gui.is_child_in_view(owner, list),
        Some(true),
        "owner resolves to its view"
    );
    assert_eq!(
        gui.is_child_in_view(owner, rows[0]),
        None,
        "must be a direct child"
    );
    let tick = app
        .world()
        .entity(list)
        .get_ref::<crate::GuiObjectInfo>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .entity(list)
            .get_ref::<crate::GuiObjectInfo>()
            .unwrap()
            .last_changed(),
        tick
    );
    if let Some(mut object) = app.world_mut().gui_mut(rows[0]) {
        object.set_visible(false)
    }
    app.update();
    assert_eq!(
        query
            .get(app.world())
            .unwrap()
            .is_child_in_view(list, rows[0]),
        Some(true)
    );
    if let Some(mut object) = app.world_mut().gui_mut(list) {
        object.scroll_to(Vec2::new(0., 500.), false)
    }
    app.update();
    let gui = query.get(app.world()).unwrap();
    assert_eq!(gui.is_child_in_view(list, rows[0]), Some(false));
    assert_eq!(
        gui.is_child_in_view(list, rows[3]),
        Some(true),
        "exact trailing edge is included"
    );
    assert_eq!(gui.is_child_in_view(list, rows[7]), Some(true));
    assert_eq!(gui.is_child_in_view(list, rows[8]), Some(false));
    app.world_mut().despawn(owner);
    app.update();
    assert!(query.get(app.world()).unwrap().get(list).is_none());
}

#[derive(Resource, Default)]
struct ItemEvents(Vec<crate::GuiListItemClick>);

fn observe_item(
    event: On<crate::GuiListItemClick>,
    rows: Query<&GuiListRow>,
    selection: Query<&crate::GuiListSelection>,
    mut log: ResMut<ItemEvents>,
    _commands: Commands,
    mut gui_edit: crate::GuiMut,
) {
    let row = rows.get(event.item).unwrap();
    assert_eq!((row.list(), row.index()), (event.entity, event.index));
    assert!(selection.get(event.entity).unwrap().0.is_empty());
    log.0.push(event.event().clone());
    gui_edit.edit(event.item, |object: &mut crate::GuiObjectMut| {
        object.set_alpha(0.42)
    });
}

#[test]
fn virtual_item_entity_observer_gets_the_clicked_row_without_selection_and_applies_same_frame_commands()
 {
    let (mut app, owner, list, view) = fixture(
        "VirtualList",
        include_bytes!("../../fairygui-demo/assets/ui/VirtualList.fui"),
        "mailList",
    );
    app.init_resource::<ItemEvents>();
    app.world_mut()
        .entity_mut(view)
        .insert(config(1000))
        .observe(observe_item);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_list_selection_mode(list, fairygui_core::widget::ListSelectionMode::None);
    app.update();
    if let Some(mut object) = app.world_mut().gui_mut(view) {
        object.scroll_to(Vec2::new(0., 3000.), false)
    }
    app.update();
    let row = app.world().get::<GuiListState>(view).unwrap().items()[1];
    let index = app.world().get::<GuiListRow>(row).unwrap().index();
    assert!(index > 20, "must report a virtual logical index");
    let object = app.world().get::<crate::GObjectRef>(row).unwrap().0;
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .emit_list_item_click(
            object,
            fairygui_core::Point::new(10., 20.),
            true,
            true,
            Some(7),
        );
    app.update();
    let events = &app.world().resource::<ItemEvents>().0;
    assert_eq!(events.len(), 1);
    let event = &events[0];
    assert_eq!((event.entity, event.item, event.index), (view, row, index));
    assert!(event.shift && event.ctrl);
    assert_eq!(event.event.touch_id, Some(7));
    assert!(!event.event.bubbling);
    assert_eq!(
        app.world().get::<crate::GuiDisplay>(row).unwrap().alpha(),
        0.42
    );
    let messages: Vec<_> = app
        .world_mut()
        .resource_mut::<Messages<crate::GuiListItemClick>>()
        .drain()
        .collect();
    assert_eq!(messages.len(), 1);
    assert_eq!(
        (messages[0].entity, messages[0].item, messages[0].index),
        (view, row, index)
    );
    app.world_mut().despawn(owner);
    app.update();
    assert_eq!(app.world().resource::<ItemEvents>().0.len(), 1);
}

fn assert_list_snapshot(app: &App, list: GObjectId, view: Entity) {
    let runtime = app.world().non_send::<GuiRuntime>();
    let store = runtime.store();
    let state = app.world().get::<GuiListState>(view).unwrap();
    assert_eq!(state.item_count(), store.list_num_items(list));
    assert_eq!(
        state.first_item_in_view(),
        store.list_first_item_in_view(list)
    );
    let items = store.list_items(list).unwrap();
    assert_eq!(
        state.items(),
        items
            .iter()
            .map(|&id| runtime.entity_of(id).unwrap())
            .collect::<Vec<_>>()
    );
    for (child_index, &item) in items.iter().enumerate() {
        let row = app
            .world()
            .get::<GuiListRow>(runtime.entity_of(item).unwrap())
            .unwrap();
        assert_eq!(row.list(), view);
        assert_eq!(
            row.index(),
            store.virtual_item_index(list, item).unwrap_or(child_index)
        );
        assert_eq!(
            row.record_index(),
            store.virtual_item_record_index(list, item)
        );
    }
}

#[test]
fn ecs_list_components_render_only_visible_rows_and_reconfigure_before_measurement() {
    let (mut app, owner, list, view) = fixture(
        "VirtualList",
        include_bytes!("../../fairygui-demo/assets/ui/VirtualList.fui"),
        "mailList",
    );
    app.world_mut().entity_mut(view).insert(config(1000));
    app.update();
    assert_list_snapshot(&app, list, view);
    let info = *app
        .world()
        .get::<GuiVirtualListState>(view)
        .unwrap()
        .info()
        .unwrap();
    assert_eq!(info.item_count, 1000);
    assert!(info.active_count > 1 && info.active_count < 30);
    assert!(
        app.world()
            .get::<GuiVirtualListState>(view)
            .unwrap()
            .error()
            .is_none()
    );
    let rows = {
        let runtime = app.world().non_send::<GuiRuntime>();
        runtime
            .store()
            .list_items(list)
            .unwrap()
            .into_iter()
            .map(|row| (row, runtime.entity_of(row).unwrap()))
            .collect::<Vec<_>>()
    };
    for &(row, entity) in &rows {
        let runtime = app.world().non_send::<GuiRuntime>();
        let index = runtime.store().virtual_item_index(list, row).unwrap();
        assert_eq!(
            app.world().get::<GuiText>(entity).unwrap().value(),
            format!("Mail {index}")
        );
        assert_eq!(
            app.world().get::<GuiTransform>(entity).unwrap().size().y,
            45. + (index % 3) as f32 * 6.
        );
        let time = runtime.store().child_by_name(row, "timeText").unwrap();
        assert_eq!(
            app.world()
                .get::<GuiText>(runtime.entity_of(time).unwrap())
                .unwrap()
                .value(),
            "ECS row"
        );
    }
    let unchanged = app
        .world()
        .entity(view)
        .get_ref::<GuiVirtualListState>()
        .unwrap()
        .last_changed();
    app.update();
    assert_list_snapshot(&app, list, view);
    assert_eq!(
        app.world()
            .entity(view)
            .get_ref::<GuiVirtualListState>()
            .unwrap()
            .last_changed(),
        unchanged
    );
    if let Some(mut object) = app.world_mut().gui_mut(view) {
        object.select_list_index(500, true)
    }
    app.update();
    assert_list_snapshot(&app, list, view);
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .list_selection(list),
        Some(500)
    );
    assert!(
        app.world()
            .get::<GuiVirtualListState>(view)
            .unwrap()
            .info()
            .unwrap()
            .first_index
            > 400
    );

    // The replacement callback indexes a shorter data set. It must never render
    // the previous 1000-item range while count and renderer change together.
    let labels = ["Only row".to_string()];
    app.world_mut()
        .entity_mut(view)
        .insert(GuiVirtualList::new(labels.len(), move |index, row| {
            row.set_text(&labels[index])
        }));
    app.update();
    assert_list_snapshot(&app, list, view);
    assert_eq!(
        app.world()
            .get::<GuiVirtualListState>(view)
            .unwrap()
            .info()
            .unwrap()
            .item_count,
        1
    );
    let row = app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .virtual_item_object(list, 0)
        .unwrap();
    let row_entity = app.world().non_send::<GuiRuntime>().entity_of(row).unwrap();
    assert_eq!(
        app.world().get::<GuiText>(row_entity).unwrap().value(),
        "Only row"
    );
    assert!(
        rows.iter()
            .any(|&(old, entity)| old == row && entity == row_entity)
    );
    app.world_mut()
        .get_mut::<GuiVirtualList>(view)
        .unwrap()
        .item_count = 0;
    app.update();
    assert_eq!(
        app.world()
            .get::<GuiVirtualListState>(view)
            .unwrap()
            .info()
            .unwrap()
            .active_count,
        0
    );
    app.world_mut().despawn(owner);
    app.update();
    assert!(app.world().get::<GuiVirtualListState>(view).is_none());
    assert!(
        rows.iter()
            .all(|&(_, entity)| app.world().get_entity(entity).is_err())
    );
}

#[test]
fn post_layout_scroll_materializes_and_projects_the_destination_rows_in_the_same_update() {
    #[derive(Resource)]
    struct Scroll(Entity);
    fn scroll(target: Res<Scroll>, mut commands: crate::GuiMut, mut once: Local<bool>) {
        if *once {
            return;
        }
        *once = true;
        commands.edit(target.0, |object: &mut crate::GuiObjectMut| {
            object.set_scroll_percentage(Vec2::new(0., 1.), false)
        });
    }
    let (mut app, _, list, view) = fixture(
        "VirtualList",
        include_bytes!("../../fairygui-demo/assets/ui/VirtualList.fui"),
        "mailList",
    );
    app.world_mut().entity_mut(view).insert(config(1000));
    app.insert_resource(Scroll(view))
        .add_systems(Update, scroll.in_set(GuiSystems::PostLayout));
    app.update();
    let scroll = app.world().get::<crate::GuiScrollState>(view).unwrap();
    assert_eq!(scroll.position().y, scroll.max_position().y);
    let rows = app.world().get::<GuiListState>(view).unwrap().items();
    assert!(
        rows.iter()
            .any(|&row| app.world().get::<GuiListRow>(row).unwrap().index() == 999)
    );
    assert!(
        rows.iter()
            .all(|&row| app.world().get::<GuiListRow>(row).unwrap().index() > 950)
    );
    assert_list_snapshot(&app, list, view);
}

#[test]
fn ecs_list_removal_drops_callback_clears_rows_and_reinsertion_reuses_the_pool() {
    let (mut app, _owner, list, view) = fixture(
        "VirtualList",
        include_bytes!("../../fairygui-demo/assets/ui/VirtualList.fui"),
        "mailList",
    );
    let captured = Arc::new(String::from("Captured"));
    let weak = Arc::downgrade(&captured);
    app.world_mut()
        .entity_mut(view)
        .insert(GuiVirtualList::new(5, move |_, row| {
            row.set_text(&captured)
        }));
    app.update();
    assert!(weak.upgrade().is_some());
    let rows = app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .list_items(list)
        .unwrap();
    app.world_mut().entity_mut(view).remove::<GuiVirtualList>();
    app.update();
    assert!(weak.upgrade().is_none());
    assert_list_snapshot(&app, list, view);
    for &row in &rows {
        let entity = app.world().non_send::<GuiRuntime>().entity_of(row).unwrap();
        assert!(app.world().get::<GuiListRow>(entity).is_none());
    }
    assert!(app.world().get::<GuiVirtualListState>(view).is_none());
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .list_num_items(list),
        0
    );
    app.world_mut().entity_mut(view).insert(config(5));
    app.update();
    assert_list_snapshot(&app, list, view);
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .list_items(list)
            .unwrap()
            .iter()
            .any(|row| rows.contains(row))
    );
    // Removal/reinsertion between updates starts a fresh binding immediately.
    app.world_mut()
        .entity_mut(view)
        .remove::<GuiVirtualList>()
        .insert(config(2));
    app.update();
    assert_eq!(
        app.world()
            .get::<GuiVirtualListState>(view)
            .unwrap()
            .info()
            .unwrap()
            .item_count,
        2
    );
}

#[test]
fn ecs_list_loop_mode_uses_logical_indices_and_rejects_mode_changes_without_replacing_rows() {
    let (mut app, _owner, list, view) = fixture(
        "LoopList",
        include_bytes!("../../fairygui-demo/assets/ui/LoopList.fui"),
        "list",
    );
    let icons = {
        let runtime = app.world().non_send::<GuiRuntime>();
        (1..=5)
            .map(|index| {
                runtime
                    .registry()
                    .item_url("LoopList", &format!("n{index}"))
                    .unwrap()
            })
            .collect::<Vec<_>>()
    };
    app.world_mut().entity_mut(view).insert(
        GuiVirtualList::new(5, move |index, row| {
            assert!(index < icons.len());
            row.set_icon(&icons[index]);
            row.set_pivot(Vec2::splat(0.5), false);
        })
        .looping(),
    );
    app.update();
    assert_list_snapshot(&app, list, view);
    let info = *app
        .world()
        .get::<GuiVirtualListState>(view)
        .unwrap()
        .info()
        .unwrap();
    assert!(info.looping);
    assert_eq!(info.real_item_count, 30);
    let first = app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .list_items(list)
        .unwrap()[0];
    let entity = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(first)
        .unwrap();
    assert_eq!(
        app.world().get::<GuiTransform>(entity).unwrap().pivot(),
        Vec2::splat(0.5)
    );
    app.world_mut().entity_mut(view).insert(config(7));
    app.update();
    let state = app.world().get::<GuiVirtualListState>(view).unwrap();
    assert_eq!(state.error(), Some(&VirtualListError::LoopModeMismatch));
    assert_eq!(state.info().unwrap().item_count, 5);
    assert_eq!(
        app.world().non_send::<GuiRuntime>().entity_of(first),
        Some(entity)
    );
}

#[test]
fn ecs_list_configuration_errors_are_visible_and_do_not_change_unsupported_controls() {
    let (mut app, owner, _, _) = fixture(
        "VirtualList",
        include_bytes!("../../fairygui-demo/assets/ui/VirtualList.fui"),
        "mailList",
    );
    let root = app.world().get::<GuiRootInstance>(owner).unwrap().object;
    let graph = {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        let graph = runtime.store_mut().create(ObjectType::Graph);
        runtime.store_mut().set_parent(graph, Some(root));
        runtime.queue_subtree(graph);
        graph
    };
    app.update();
    let view = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(graph)
        .unwrap();
    app.world_mut().entity_mut(view).insert(config(100));
    app.update();
    let state = app.world().get::<GuiVirtualListState>(view).unwrap();
    assert!(state.info().is_none());
    assert!(state.error().is_some());
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .object(graph)
            .unwrap()
            .kind(),
        ObjectType::Graph
    );
}

#[test]
fn ecs_list_rows_track_ordinary_membership_order_and_unchanged_ticks() {
    let (mut app, _owner, list, view) = fixture(
        "VirtualList",
        include_bytes!("../../fairygui-demo/assets/ui/VirtualList.fui"),
        "mailList",
    );
    assert_list_snapshot(&app, list, view);
    let row = {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        let row = runtime.store_mut().create(ObjectType::Button);
        runtime.store_mut().set_parent(row, Some(list));
        runtime.queue_subtree(row);
        row
    };
    app.update();
    assert_list_snapshot(&app, list, view);
    let entity = app.world().non_send::<GuiRuntime>().entity_of(row).unwrap();
    assert!(
        app.world()
            .get::<GuiListRow>(entity)
            .unwrap()
            .record_index()
            .is_none()
    );
    let list_tick = app
        .world()
        .entity(view)
        .get_ref::<GuiListState>()
        .unwrap()
        .last_changed();
    let row_tick = app
        .world()
        .entity(entity)
        .get_ref::<GuiListRow>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .entity(view)
            .get_ref::<GuiListState>()
            .unwrap()
            .last_changed(),
        list_tick
    );
    assert_eq!(
        app.world()
            .entity(entity)
            .get_ref::<GuiListRow>()
            .unwrap()
            .last_changed(),
        row_tick
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_parent(row, None);
    app.update();
    assert_list_snapshot(&app, list, view);
    assert!(app.world().get::<GuiListRow>(entity).is_none());
}

#[test]
fn ecs_post_layout_sees_rebound_rows_and_projects_commands_in_the_same_frame() {
    #[derive(Component)]
    struct Carousel;
    #[derive(Resource, Default)]
    struct Seen(Vec<(Entity, usize)>);
    fn decorate(
        gui: GuiQuery,
        lists: Query<(Entity, &GuiListState), With<Carousel>>,
        rows: Query<&GuiListRow>,
        mut seen: ResMut<Seen>,
        _commands: Commands,
        mut gui_edit: crate::GuiMut,
    ) {
        seen.0.clear();
        for (entity, list) in &lists {
            assert_eq!(gui.list(entity), Some(list));
            for &row in list.items() {
                let binding = rows.get(row).unwrap();
                assert_eq!(binding.list(), entity);
                assert!(gui.transform(row).is_some());
                seen.0.push((row, binding.index()));
                gui_edit.edit(row, |object: &mut crate::GuiObjectMut| {
                    object.set_scale(Vec2::splat(1.2))
                });
                gui_edit.edit(row, |object: &mut crate::GuiObjectMut| {
                    object.set_text(format!("index {}", binding.index()))
                });
            }
        }
    }
    let (mut app, _owner, list, view) = fixture(
        "VirtualList",
        include_bytes!("../../fairygui-demo/assets/ui/VirtualList.fui"),
        "mailList",
    );
    app.init_resource::<Seen>()
        .add_systems(Update, decorate.in_set(GuiSystems::PostLayout));
    app.world_mut()
        .entity_mut(view)
        .insert((Carousel, config(1000)));
    app.update();
    let before = app.world().resource::<Seen>().0.clone();
    assert!(!before.is_empty());
    if let Some(mut object) = app.world_mut().gui_mut(view) {
        object.select_list_index(500, true)
    }
    app.update();
    assert_list_snapshot(&app, list, view);
    let after = &app.world().resource::<Seen>().0;
    assert!(after.iter().all(|&(_, index)| index > 400));
    assert!(after.iter().any(|&(entity, index)| {
        before
            .iter()
            .any(|&(old, old_index)| old == entity && old_index != index)
    }));
    for &(entity, index) in after {
        assert_eq!(
            app.world().get::<GuiText>(entity).unwrap().value(),
            format!("index {index}")
        );
        assert_eq!(
            app.world().get::<GuiTransform>(entity).unwrap().scale(),
            Vec2::splat(1.2)
        );
        let object = app.world().get::<GObjectRef>(entity).unwrap().0;
        assert_eq!(
            app.world()
                .non_send::<GuiRuntime>()
                .store()
                .object(object)
                .unwrap()
                .scale_x(),
            1.2
        );
    }
}

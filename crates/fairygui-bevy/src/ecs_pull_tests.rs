use super::*;
use crate::prelude::*;
use bevy::ecs::system::SystemState;
use fairygui_core::{
    object::{EventKind, GObjectId},
    package::FuiPackage,
};

fn fixture() -> (App, Entity, GObjectId, Entity, GObjectId, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>()
    .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        std::time::Duration::from_millis(150),
    ));
    let package = FuiPackage::parse(
        "PullToRefresh",
        include_bytes!("../../fairygui-demo/assets/ui/PullToRefresh.fui"),
    )
    .unwrap();
    let handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package,
            res_key: "ui/PullToRefresh".into(),
        });
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(handle, "Main"))
        .id();
    app.update();
    let root = app.world().get::<GuiRootInstance>(owner).unwrap().object;
    let runtime = app.world().non_send::<GuiRuntime>();
    let first = runtime.store().child_by_name(root, "list1").unwrap();
    let second = runtime.store().child_by_name(root, "list2").unwrap();
    let views = (
        runtime.entity_of(first).unwrap(),
        runtime.entity_of(second).unwrap(),
    );
    (app, owner, first, views.0, second, views.1)
}

fn titles(count: usize) -> GuiListContent {
    GuiListContent::new(count, move |index, row| {
        row.set_text(&format!("Item {}", count - index - 1));
    })
}

#[test]
fn ecs_ordinary_list_content_retains_leading_rows_disposes_trailing_rows_and_keeps_overlays() {
    let (mut app, owner, list, view, _, _) = fixture();
    let header = *app
        .world()
        .get::<GuiScrollState>(view)
        .unwrap()
        .header()
        .unwrap();
    let original = app
        .world()
        .get::<GuiListState>(view)
        .unwrap()
        .items()
        .to_vec();
    assert_eq!(original.len(), 4);
    app.world_mut().entity_mut(view).insert(titles(1));
    app.update();
    assert_eq!(
        app.world().get::<GuiListState>(view).unwrap().items(),
        &original[..1]
    );
    assert!(
        original[1..]
            .iter()
            .all(|&row| app.world().get_entity(row).is_err())
    );
    assert!(app.world().get_entity(header.entity).is_ok());
    assert_eq!(
        app.world().get::<GuiText>(original[0]).unwrap().value(),
        "Item 0"
    );
    app.world_mut().entity_mut(view).insert(titles(6));
    app.update();
    let state = app.world().get::<GuiListState>(view).unwrap();
    assert_eq!(state.item_count(), 6);
    assert_eq!(state.items()[0], original[0]);
    assert!(!state.items().contains(&header.entity));
    assert!(
        app.world()
            .get::<GuiListContentState>(view)
            .unwrap()
            .error()
            .is_none()
    );
    let rows = state.items().to_vec();
    let runtime = app.world().non_send::<GuiRuntime>();
    let template_size = runtime
        .store()
        .object(app.world().get::<GObjectRef>(rows[0]).unwrap().0)
        .unwrap()
        .size();
    for (index, &row) in rows.iter().enumerate() {
        assert_eq!(
            app.world().get::<GuiText>(row).unwrap().value(),
            format!("Item {}", 5 - index)
        );
        let binding = app.world().get::<GuiListRow>(row).unwrap();
        assert_eq!(binding.list(), view);
        assert_eq!(binding.index(), index);
        assert!(binding.record_index().is_none());
        assert_eq!(
            app.world().get::<GuiTransform>(row).unwrap().size(),
            Vec2::new(template_size.width, template_size.height)
        );
        assert!(
            runtime
                .asset_handle_for(app.world().get::<GObjectRef>(row).unwrap().0)
                .is_some()
        );
    }
    assert_eq!(runtime.store().list_item_count(list), 6);
    let tick = app
        .world()
        .entity(view)
        .get_ref::<GuiListContentState>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .entity(view)
            .get_ref::<GuiListContentState>()
            .unwrap()
            .last_changed(),
        tick
    );
    // Removing a nonvirtual configuration releases its closure and preserves rows.
    let data = std::sync::Arc::new(String::from("Shared"));
    let weak = std::sync::Arc::downgrade(&data);
    app.world_mut()
        .get_mut::<GuiListContent>(view)
        .unwrap()
        .set_renderer(move |_, row| row.set_text(&data));
    app.update();
    assert!(weak.upgrade().is_some());
    assert_eq!(
        app.world().get::<GuiText>(rows[0]).unwrap().value(),
        "Shared"
    );
    app.world_mut().entity_mut(view).remove::<GuiListContent>();
    app.update();
    assert!(weak.upgrade().is_none());
    assert!(app.world().get::<GuiListContentState>(view).is_none());
    assert_eq!(app.world().get::<GuiListState>(view).unwrap().items(), rows);
    app.world_mut().entity_mut(view).insert(titles(0));
    app.update();
    assert!(rows.iter().all(|&row| app.world().get_entity(row).is_err()));
    assert!(
        app.world()
            .get::<GuiListState>(view)
            .unwrap()
            .items()
            .is_empty()
    );
    assert!(app.world().get_entity(header.entity).is_ok());
    app.world_mut().despawn(owner);
    app.update();
    assert!(app.world().get_entity(view).is_err());
    assert!(app.world().get_entity(header.entity).is_err());
}

#[test]
fn ecs_ordinary_list_configuration_errors_preserve_content_and_virtual_mode() {
    let (mut app, owner, list, view, _, _) = fixture();
    let original = app
        .world()
        .get::<GuiListState>(view)
        .unwrap()
        .items()
        .to_vec();
    let root = app.world().get::<GuiRootView>(owner).unwrap().0;
    app.world_mut().entity_mut(root).insert(titles(10));
    app.update();
    assert_eq!(
        app.world()
            .get::<GuiListContentState>(root)
            .unwrap()
            .error(),
        Some(&GuiListContentError::NotList)
    );
    let empty_list = {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        let empty = runtime
            .store_mut()
            .create(fairygui_core::package::ObjectType::List);
        let row = runtime
            .store_mut()
            .create(fairygui_core::package::ObjectType::Button);
        let root_object = runtime.store().object(list).unwrap().parent();
        runtime.store_mut().set_parent(empty, root_object);
        runtime.store_mut().set_parent(row, Some(empty));
        runtime.queue_subtree(empty);
        empty
    };
    app.update();
    let empty_view = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(empty_list)
        .unwrap();
    let existing = app
        .world()
        .get::<GuiListState>(empty_view)
        .unwrap()
        .items()
        .to_vec();
    app.world_mut().entity_mut(empty_view).insert(titles(5));
    app.update();
    assert_eq!(
        app.world()
            .get::<GuiListContentState>(empty_view)
            .unwrap()
            .error(),
        Some(&GuiListContentError::MissingDefaultItem)
    );
    assert_eq!(
        app.world().get::<GuiListState>(empty_view).unwrap().items(),
        existing
    );
    // Existing rows and shrinking do not require a default template.
    app.world_mut().entity_mut(empty_view).insert(titles(1));
    app.update();
    assert!(
        app.world()
            .get::<GuiListContentState>(empty_view)
            .unwrap()
            .error()
            .is_none()
    );
    assert_eq!(
        app.world().get::<GuiListState>(view).unwrap().items(),
        original
    );
    app.world_mut().entity_mut(view).insert(titles(1));
    app.update();
    app.world_mut()
        .entity_mut(view)
        .insert(GuiVirtualList::new(100, |index, row| {
            row.set_text(&format!("Virtual {index}"))
        }));
    app.update();
    assert_eq!(
        app.world()
            .get::<GuiListContentState>(view)
            .unwrap()
            .error(),
        Some(&GuiListContentError::VirtualList)
    );
    assert_eq!(
        app.world().get::<GuiListState>(view).unwrap().item_count(),
        100
    );
    assert!(
        app.world()
            .get::<GuiVirtualListState>(view)
            .unwrap()
            .error()
            .is_none()
    );
}

#[derive(Default, Resource)]
struct Responses(usize);
fn respond(
    gui: GuiQuery,
    mut events: MessageReader<GuiPullRelease>,
    mut commands: Commands,
    mut responses: ResMut<Responses>,
    mut gui_edit: crate::GuiMut,
) {
    for event in events.read() {
        assert!(event.event.is_at_target() && !event.event.bubbling);
        assert_eq!(event.event.kind, EventKind::PullDownRelease);
        let scroll = gui.scroll(event.entity).unwrap();
        assert_eq!(scroll.display_position().y, -90.);
        let header = scroll.header().unwrap();
        assert_eq!(header.size.y, 90.);
        assert_eq!(header.source_size.y, 71.);
        assert_eq!(scroll.header_locked_size(), 0.);
        commands.write_message(GuiPullCommand::LockHeader {
            target: event.entity,
            size: header.source_size.y,
        });
        gui_edit.edit(header.entity, |object: &mut crate::GuiObjectMut| {
            object.set_controller_page("c1", 2)
        });
        responses.0 += 1;
    }
}

#[test]
fn ecs_pull_response_applies_lock_before_first_release_tick_and_updates_entity_state() {
    let (mut app, owner, list, view, _, footer_view) = fixture();
    app.init_resource::<Responses>()
        .add_systems(Update, respond.in_set(GuiSystems::Respond));
    let header = app
        .world()
        .get::<GuiScrollState>(view)
        .unwrap()
        .header()
        .unwrap()
        .entity;
    {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        runtime.store_mut().begin_scroll_drag(list);
        runtime.store_mut().drag_scroll_by(list, 0., 180., 0.1);
        runtime.store_mut().end_scroll_drag(list);
    }
    app.update();
    let scroll = app.world().get::<GuiScrollState>(view).unwrap();
    assert_eq!(scroll.header_locked_size(), 71.);
    assert!((-90. ..=-71.).contains(&scroll.display_position().y));
    assert_eq!(app.world().resource::<Responses>().0, 1);
    assert_eq!(
        app.world()
            .get::<GuiControllers>(header)
            .unwrap()
            .get("c1")
            .unwrap()
            .selected_index(),
        2
    );
    app.update();
    assert_eq!(app.world().resource::<Responses>().0, 1);
    assert_eq!(
        app.world()
            .get::<GuiScrollState>(view)
            .unwrap()
            .display_position()
            .y,
        -71.
    );
    // Message ordering, invalid input and disposed targets cannot alter a lock.
    app.world_mut().write_message(GuiPullCommand::LockHeader {
        target: view,
        size: 35.,
    });
    for size in [-1., f32::NAN, f32::INFINITY] {
        app.world_mut()
            .write_message(GuiPullCommand::LockHeader { target: view, size });
    }
    app.world_mut().write_message(GuiPullCommand::LockFooter {
        target: footer_view,
        size: 71.,
    });
    app.update();
    assert_eq!(
        app.world()
            .get::<GuiScrollState>(view)
            .unwrap()
            .header_locked_size(),
        35.
    );
    assert_eq!(
        app.world()
            .get::<GuiScrollState>(footer_view)
            .unwrap()
            .footer_locked_size(),
        71.
    );
    app.world_mut().write_message(GuiPullCommand::LockHeader {
        target: view,
        size: 0.,
    });
    app.world_mut().write_message(GuiPullCommand::LockFooter {
        target: footer_view,
        size: 0.,
    });
    app.update();
    app.update();
    app.update();
    assert_eq!(
        app.world()
            .get::<GuiScrollState>(view)
            .unwrap()
            .header_locked_size(),
        0.
    );
    assert_eq!(
        app.world()
            .get::<GuiScrollState>(footer_view)
            .unwrap()
            .footer_locked_size(),
        0.
    );
    assert!(!app.world().get::<GuiDisplay>(header).unwrap().on_stage());
    app.world_mut().despawn(owner);
    app.update();
    app.world_mut().write_message(GuiPullCommand::LockHeader {
        target: view,
        size: 71.,
    });
    app.update();
    assert!(app.world().get_entity(header).is_err());
}

#[test]
fn ecs_controller_snapshots_match_native_pages_and_observe_indirect_changes_before_publish() {
    #[derive(Resource)]
    struct Source(Entity);
    #[derive(Default, Resource)]
    struct Seen(i32);
    fn observe(gui: GuiQuery, source: Res<Source>, mut seen: ResMut<Seen>) {
        seen.0 = gui.controller(source.0, "c1").unwrap().selected_index();
    }
    let (mut app, owner, _list, view, _, _) = fixture();
    let header = app
        .world()
        .get::<GuiScrollState>(view)
        .unwrap()
        .header()
        .unwrap()
        .entity;
    app.insert_resource(Source(header))
        .init_resource::<Seen>()
        .add_systems(Update, observe.in_set(GuiSystems::Respond));
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    assert!(query.get(app.world()).unwrap().controllers(owner).is_none());
    assert!(
        query
            .get(app.world())
            .unwrap()
            .controllers(header)
            .is_some()
    );
    let source = app.world().get::<GObjectRef>(header).unwrap().0;
    let initial = app
        .world()
        .get::<GuiControllers>(header)
        .unwrap()
        .get("c1")
        .unwrap()
        .clone();
    let runtime = app.world().non_send::<GuiRuntime>();
    let native = runtime
        .store()
        .object(source)
        .unwrap()
        .control()
        .component()
        .unwrap()
        .controller("c1")
        .unwrap();
    assert_eq!(initial.page_ids(), native.page_ids);
    assert_eq!(initial.page_names(), native.page_names);
    assert_eq!(initial.selected_page_id(), native.selected_page_id());
    assert_eq!(initial.selected_page_name(), native.selected_page_name());
    let tick = app
        .world()
        .entity(header)
        .get_ref::<GuiControllers>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .entity(header)
            .get_ref::<GuiControllers>()
            .unwrap()
            .last_changed(),
        tick
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_controller_page_index(source, "c1", 2);
    app.update();
    assert_eq!(app.world().resource::<Seen>().0, 2);
    let state = app
        .world()
        .get::<GuiControllers>(header)
        .unwrap()
        .get("c1")
        .unwrap();
    assert_eq!(state.previous_index(), initial.selected_index());
    assert_eq!(state.selected_index(), 2);
    // A controller change on a retained offstage header must not start playback.
    assert!(
        !app.world()
            .non_send::<GuiRuntime>()
            .store()
            .transition_state(source, "t0")
            .unwrap()
            .0
    );
    // All native controls with no controllers omit the snapshot component.
    let runtime = app.world().non_send::<GuiRuntime>();
    let mut checked = 0;
    for (id, object) in runtime.store().iter() {
        if let Some(entity) = runtime.entity_of(id)
            && object
                .control()
                .component()
                .is_none_or(|component| component.controllers.is_empty())
        {
            assert!(app.world().get::<GuiControllers>(entity).is_none());
            checked += 1;
        }
    }
    assert!(checked > 10);
}

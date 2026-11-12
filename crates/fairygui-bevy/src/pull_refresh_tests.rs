use super::*;
use fairygui_core::geom::Size;

fn fixture() -> (App, usize, GObjectId, GObjectId) {
    let (mut app, _, _) = super::list_bar_tests::fixture_with_layout(true);
    app.add_systems(Update, sync_scroll_extents.after(sync_dirty));
    app.add_systems(
        PostUpdate,
        project_scroll_overflow
            .after(bevy::ui::UiSystems::Layout)
            .before(bevy::ui::update::update_clipping_system),
    );
    let package = FuiPackage::parse(
        "PullToRefresh",
        include_bytes!("../../fairygui-demo/assets/ui/PullToRefresh.fui"),
    )
    .unwrap();
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: package.clone(),
            res_key: "ui/PullToRefresh".into(),
        });
    let index = app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .create_root(asset, &package, "Main")
        .unwrap();
    app.update();
    app.update();
    let gui = app.world().non_send::<GuiRuntime>();
    let root = gui.root_object(index).unwrap();
    let first = gui.store().child_by_name(root, "list1").unwrap();
    let second = gui.store().child_by_name(root, "list2").unwrap();
    (app, index, first, second)
}

#[test]
fn exported_refresh_components_keep_views_and_clip_at_native_positions_through_lock_and_disposal() {
    let (mut app, index, first, second) = fixture();
    let gui = app.world().non_send::<GuiRuntime>();
    let header = gui.store().scroll_header(first).unwrap();
    let footer = gui.store().scroll_footer(second).unwrap();
    assert_eq!(gui.store().list_item_count(first), 4);
    assert_eq!(gui.store().list_item_count(second), 4);
    let header_view = gui.entity_of(header).unwrap();
    let footer_view = gui.entity_of(footer).unwrap();
    for (list, part, view) in [(first, header, header_view), (second, footer, footer_view)] {
        let part_z = app.world().get::<ZIndex>(view).unwrap().0;
        for row in gui.store().list_items(list).unwrap() {
            let row_z = app
                .world()
                .get::<ZIndex>(gui.entity_of(row).unwrap())
                .unwrap()
                .0;
            assert!(
                part_z < row_z,
                "refresh part {part:?} must paint behind content"
            );
        }
    }
    assert_eq!(
        *app.world().get::<Visibility>(header_view).unwrap(),
        Visibility::Hidden
    );
    assert_eq!(
        *app.world().get::<Visibility>(footer_view).unwrap(),
        Visibility::Hidden
    );
    assert_eq!(
        app.world()
            .get::<bevy::ui::IgnoreScroll>(header_view)
            .unwrap()
            .0,
        bevy::math::BVec2::TRUE
    );
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut().begin_scroll_drag(first);
        gui.store_mut().drag_scroll_by(first, 0., 180., 0.1);
        gui.store_mut().end_scroll_drag(first);
        gui.store_mut().lock_scroll_header(first, 71.);
        gui.store_mut().advance_scrolls(0.3);
        let view = gui.store().scroll_pane(second).unwrap().view_size();
        gui.store_mut()
            .update_scroll_content_size(second, Size::new(view.width, 100.));
        gui.store_mut().lock_scroll_footer(second, 71.);
        gui.store_mut().advance_scrolls(0.3);
    }
    app.update();
    app.update();
    let gui = app.world().non_send::<GuiRuntime>();
    assert_eq!(gui.entity_of(header), Some(header_view));
    assert_eq!(gui.entity_of(footer), Some(footer_view));
    assert_ne!(
        *app.world().get::<Visibility>(header_view).unwrap(),
        Visibility::Hidden
    );
    for (list, part, offset) in [
        (first, header, 0.),
        (
            second,
            footer,
            gui.store().scroll_pane(second).unwrap().view_size().height - 71.,
        ),
    ] {
        let owner = gui.entity_of(list).unwrap();
        let entity = gui.entity_of(part).unwrap();
        let owner_node = app.world().get::<ComputedNode>(owner).unwrap();
        let owner_transform = app.world().get::<UiGlobalTransform>(owner).unwrap();
        let node = app.world().get::<ComputedNode>(entity).unwrap();
        let transform = app.world().get::<UiGlobalTransform>(entity).unwrap();
        let expected = owner_transform.transform_point2(owner_node.content_box().min)
            + owner_transform
                .transform_vector2(Vec2::new(0., offset / owner_node.inverse_scale_factor()));
        let actual = transform.transform_point2(node.content_box().min);
        assert!(
            (actual - expected).length() <= 1.,
            "refresh position actual={actual:?} expected={expected:?}"
        );
        assert!((node.size().y * node.inverse_scale_factor() - 71.).abs() <= 1.);
        assert!(
            app.world()
                .get::<bevy::ui::CalculatedClip>(entity)
                .is_some()
        );
    }
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut().lock_scroll_header(first, 0.);
        gui.store_mut().lock_scroll_footer(second, 0.);
        gui.store_mut().advance_scrolls(0.3);
    }
    app.update();
    assert_eq!(
        *app.world().get::<Visibility>(header_view).unwrap(),
        Visibility::Hidden
    );
    assert_eq!(
        *app.world().get::<Visibility>(footer_view).unwrap(),
        Visibility::Hidden
    );
    let root = app
        .world()
        .non_send::<GuiRuntime>()
        .root_object(index)
        .unwrap();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .remove(root);
    app.update();
    assert!(app.world().get_entity(header_view).is_err());
    assert!(app.world().get_entity(footer_view).is_err());
}

#[derive(Resource, Default)]
struct Responses(usize);

fn respond(
    mut gui: NonSendMut<GuiRuntime>,
    mut events: MessageReader<GuiPullRelease>,
    mut responses: ResMut<Responses>,
) {
    for release in events.read() {
        assert!(release.event.is_at_target());
        assert!(!release.event.bubbling);
        assert_eq!(
            release.event.kind,
            fairygui_core::object::EventKind::PullDownRelease
        );
        assert_eq!(
            gui.store()
                .scroll_pane(release.event.target)
                .unwrap()
                .display_pos()
                .y,
            -90.
        );
        gui.store_mut()
            .lock_scroll_header(release.event.target, 71.);
        responses.0 += 1;
    }
}
fn advance(mut gui: NonSendMut<GuiRuntime>) {
    gui.store_mut().advance_scrolls(0.15);
}

#[test]
fn pull_message_is_target_only_and_response_lock_precedes_first_release_tick() {
    let (mut app, _, list, _) = fixture();
    app.init_resource::<Responses>()
        .add_message::<GuiPullRelease>()
        .configure_sets(
            Update,
            (
                crate::GuiSystems::Publish,
                crate::GuiSystems::Respond,
                crate::GuiSystems::Advance,
                crate::GuiSystems::Sync,
            )
                .chain(),
        )
        .add_systems(
            Update,
            emit_pull_release_messages.in_set(crate::GuiSystems::Publish),
        )
        .add_systems(Update, respond.in_set(crate::GuiSystems::Respond))
        .add_systems(Update, advance.in_set(crate::GuiSystems::Advance));
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut().begin_scroll_drag(list);
        gui.store_mut().drag_scroll_by(list, 0., 180., 0.1);
        gui.store_mut().end_scroll_drag(list);
    }
    app.update();
    let display = app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .scroll_pane(list)
        .unwrap()
        .display_pos()
        .y;
    assert!((-90. ..=-71.).contains(&display));
    app.update();
    assert_eq!(app.world().resource::<Responses>().0, 1);
    let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
    assert_eq!(gui.store().scroll_pane(list).unwrap().display_pos().y, -71.);
    let events: Vec<_> = gui
        .drain_events()
        .filter(|e| e.kind == fairygui_core::object::EventKind::PullDownRelease)
        .collect();
    assert_eq!(events.len(), 1);
    assert!(events[0].is_at_target());
}

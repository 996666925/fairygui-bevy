use super::*;

fn fixture() -> (App, GObjectId, GObjectId) {
    fixture_with_layout(false)
}

#[test]
fn retained_bar_view_projects_hold_fade_hide_and_wake_without_recreation() {
    let (mut app, _, list) = fixture();
    app.add_systems(
        Update,
        appearance::sync_appearance.after(sync_stage_hierarchy),
    );
    let bar = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut().set_size(list, 300., 100.);
        gui.store_mut()
            .update_scroll_content_size(list, fairygui_core::geom::Size::new(250., 600.));
        gui.store_mut().layout_scroll_bars(list);
        gui.store_mut()
            .set_scroll_bar_display(list, fairygui_core::scroll::ScrollBarDisplayType::Auto);
        let bar = gui.store().scroll_pane(list).unwrap().vt_bar.unwrap();
        gui.store_mut().set_scroll_bar_grip_dragging(bar, true);
        bar
    };
    app.update();
    let view = app.world().non_send::<GuiRuntime>().entity_of(bar).unwrap();
    assert_ne!(
        *app.world().get::<Visibility>(view).unwrap(),
        Visibility::Hidden
    );
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut().advance_scrolls(5.);
        gui.store_mut().sync_scroll_bars(list);
    }
    app.update();
    assert_ne!(
        *app.world().get::<Visibility>(view).unwrap(),
        Visibility::Hidden
    );
    let mut original_parts = Vec::new();
    app.world()
        .non_send::<GuiRuntime>()
        .store()
        .for_each_descendant(bar, &mut |id| original_parts.push(id));
    let original_images: Vec<_> = original_parts
        .into_iter()
        .filter_map(|id| app.world().non_send::<GuiRuntime>().entity_of(id))
        .filter_map(|entity| {
            app.world()
                .get::<ImageNode>(entity)
                .map(|image| (entity, image.color.alpha()))
        })
        .collect();
    assert!(original_images.iter().any(|(_, alpha)| *alpha > 0.));
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut().set_scroll_bar_grip_dragging(bar, false);
        gui.store_mut().advance_scrolls(0.75);
        gui.store_mut().sync_scroll_bars(list);
    }
    app.update();
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .object(bar)
            .unwrap()
            .alpha(),
        0.25
    );
    for (entity, original_alpha) in original_images {
        assert!(
            (app.world().get::<ImageNode>(entity).unwrap().color.alpha() - original_alpha * 0.25)
                .abs()
                < 1e-5
        );
    }
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .advance_scrolls(0.25);
    app.update();
    assert_eq!(
        *app.world().get::<Visibility>(view).unwrap(),
        Visibility::Hidden
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_scroll_bar_grip_dragging(bar, true);
    app.update();
    assert_ne!(
        *app.world().get::<Visibility>(view).unwrap(),
        Visibility::Hidden
    );
    assert_eq!(
        app.world().non_send::<GuiRuntime>().entity_of(bar),
        Some(view)
    );
}

pub(super) fn fixture_with_layout(layout: bool) -> (App, GObjectId, GObjectId) {
    let package = FuiPackage::parse(
        "Chat",
        include_bytes!("../../fairygui-core/tests/fixtures/Chat.fui"),
    )
    .unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<FuiAsset>()
        .init_asset::<Image>()
        .init_asset::<Font>()
        .init_resource::<GuiFont>();
    if layout {
        use bevy::camera::{ComputedCameraValues, RenderTargetInfo, Viewport};
        app.init_asset::<TextureAtlasLayout>().add_plugins((
            bevy::input::InputPlugin,
            bevy::transform::TransformPlugin,
            bevy::text::TextPlugin,
            bevy::picking::PickingPlugin,
            bevy::picking::InteractionPlugin,
            bevy::ui::UiPlugin,
        ));
        app.world_mut().spawn((
            Camera2d,
            Camera {
                computed: ComputedCameraValues {
                    target_info: Some(RenderTargetInfo {
                        physical_size: UVec2::new(1280, 720),
                        scale_factor: 1.,
                    }),
                    ..default()
                },
                viewport: Some(Viewport {
                    physical_size: UVec2::new(1280, 720),
                    ..default()
                }),
                ..default()
            },
        ));
    }
    let handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: package.clone(),
            res_key: "ui/Chat".into(),
        });
    let mut gui = GuiRuntime::default();
    let index = gui.create_root(handle, &package, "Main").unwrap();
    let root = gui.root_object(index).unwrap();
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
    let gui = app.world().non_send::<GuiRuntime>();
    let list = gui
        .store()
        .iter()
        .find_map(|(id, o)| {
            matches!(o.control(), ControlData::List(_))
                .then_some(id)
                .filter(|&id| {
                    gui.store()
                        .scroll_pane(id)
                        .is_some_and(|p| p.vt_bar.is_some())
                })
        })
        .unwrap();
    (app, root, list)
}

#[test]
fn exported_chat_bar_views_are_fixed_and_detached_reuse_removes_only_runtime_scroll_marker() {
    let (mut app, root, list) = fixture();
    let bar = app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .scroll_pane(list)
        .unwrap()
        .vt_bar
        .unwrap();
    let view = app.world().non_send::<GuiRuntime>().entity_of(bar).unwrap();
    assert_eq!(
        app.world().get::<bevy::ui::IgnoreScroll>(view).unwrap().0,
        bevy::math::BVec2::TRUE
    );
    let row = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let row = gui
            .store_mut()
            .create(fairygui_core::package::ObjectType::Graph);
        gui.store_mut().set_parent(row, Some(list));
        gui.store_mut().set_size(row, 300., 400.);
        gui.queue_subtree(row);
        gui.store_mut().layout_list(list);
        row
    };
    app.update();
    let row_view = app.world().non_send::<GuiRuntime>().entity_of(row).unwrap();
    assert!(
        app.world()
            .get::<bevy::ui::IgnoreScroll>(row_view)
            .is_none()
    );
    let original_node = app.world().get::<Node>(view).unwrap().clone();
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut().set_scroll_pos(list, 0., 80., false);
        gui.store_mut().layout_list(list);
    }
    app.update();
    assert_eq!(
        app.world().get::<Node>(view).unwrap().top,
        original_node.top
    );
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .list_item_of(bar)
            .is_none()
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_parent(bar, Some(root));
    app.update();
    assert!(app.world().get::<bevy::ui::IgnoreScroll>(view).is_none());
    assert_eq!(
        app.world().non_send::<GuiRuntime>().entity_of(bar),
        Some(view)
    );
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_pane(list)
            .unwrap()
            .vt_bar,
        None
    );
    // Application-authored IgnoreScroll on ordinary content is not owned here.
    app.world_mut()
        .entity_mut(row_view)
        .insert(bevy::ui::IgnoreScroll(bevy::math::BVec2::TRUE));
    app.update();
    assert!(
        app.world()
            .get::<bevy::ui::IgnoreScroll>(row_view)
            .is_some()
    );
}

#[test]
fn combo_row_replacement_preserves_registered_bar_and_content_indices() {
    let (mut app, _root, list) = fixture();
    let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
    let bar = gui.store().scroll_pane(list).unwrap().vt_bar.unwrap();
    let package = gui.registry.get_by_name("Chat").unwrap().clone();
    let template = gui.resolve_template(&package, "chatItem");
    // Use an actual exported item URL from the chat list.
    let template = template
        .or_else(|| {
            let ControlData::List(data) = gui.store().object(list).unwrap().control() else {
                return None;
            };
            gui.resolve_template(&package, data.default_item.as_deref()?)
        })
        .unwrap();
    let captions = vec!["one".to_owned(), "two".to_owned()];
    assert!(gui.replace_popup_items(list, &template, &captions, &[], &[]));
    gui.store_mut().add_child_at(list, bar, 0);
    assert_eq!(gui.store().list_item_count(list), 2);
    assert_eq!(gui.store().scroll_pane(list).unwrap().vt_bar, Some(bar));
    assert!(gui.store().object(bar).is_some());
    assert!(gui.replace_popup_items(list, &template, &["three".into()], &[], &[]));
    assert_eq!(gui.store().list_item_count(list), 1);
    assert!(gui.store().object(bar).is_some());
    assert_eq!(
        gui.store()
            .list_item_index(list, gui.store().list_item_at(list, 0).unwrap()),
        Some(0)
    );
}

#[test]
fn real_headless_bevy_layout_keeps_the_bar_stationary_while_rows_scroll() {
    let (mut app, _, list) = fixture_with_layout(true);
    let (row, bar) = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        for item in gui.store().list_items(list).unwrap() {
            gui.store_mut().remove(item);
        }
        gui.store_mut().set_size(list, 300., 100.);
        let bar = gui.store().scroll_pane(list).unwrap().vt_bar.unwrap();
        let row = gui
            .store_mut()
            .create(fairygui_core::package::ObjectType::Graph);
        gui.store_mut().set_parent(row, Some(list));
        gui.store_mut().set_size(row, 250., 400.);
        gui.queue_subtree(row);
        gui.store_mut().layout_list(list);
        gui.store_mut().layout_scroll_bars(list);
        (row, bar)
    };
    app.update();
    app.update();
    let gui = app.world().non_send::<GuiRuntime>();
    let row_view = gui.entity_of(row).unwrap();
    let bar_view = gui.entity_of(bar).unwrap();
    let initial_row = app
        .world()
        .get::<UiGlobalTransform>(row_view)
        .unwrap()
        .translation;
    let initial_bar = app
        .world()
        .get::<UiGlobalTransform>(bar_view)
        .unwrap()
        .translation;
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_scroll_pos(list, 0., 60., false);
    app.update();
    let row_position = app
        .world()
        .get::<UiGlobalTransform>(row_view)
        .unwrap()
        .translation;
    let bar_position = app
        .world()
        .get::<UiGlobalTransform>(bar_view)
        .unwrap()
        .translation;
    assert_eq!(bar_position, initial_bar);
    assert_eq!(row_position, initial_row - Vec2::new(0., 60.));
}

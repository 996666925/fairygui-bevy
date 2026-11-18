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

#[test]
fn changed_margins_project_content_and_owner_bars_at_the_same_coordinates_as_core() {
    let (mut app, _, list) = fixture_with_layout(true);
    app.add_systems(Update, sync_scroll_extents.after(sync_dirty));
    app.add_systems(
        PostUpdate,
        project_scroll_overflow
            .after(bevy::ui::UiSystems::Layout)
            .before(bevy::ui::update::update_clipping_system),
    );
    let (row, bar) = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        for row in gui.store().list_items(list).unwrap() {
            gui.store_mut().remove(row);
        }
        gui.store_mut().set_size(list, 300., 100.);
        let row = gui
            .store_mut()
            .create(fairygui_core::package::ObjectType::Graph);
        gui.store_mut().set_size(row, 250., 400.);
        gui.store_mut().set_parent(row, Some(list));
        gui.queue_subtree(row);
        let bar = gui.store().scroll_pane(list).unwrap().vt_bar.unwrap();
        (row, bar)
    };
    for margin in [
        crate::GuiMargin::all(0.),
        crate::GuiMargin::new(10., 15., 20., 25.),
    ] {
        let list_view = app
            .world()
            .non_send::<GuiRuntime>()
            .entity_of(list)
            .unwrap();
        app.world_mut()
            .gui_mut(list_view)
            .unwrap()
            .set_margin(margin);
        app.update();
        app.update();
        let gui = app.world().non_send::<GuiRuntime>();
        let owner_node = app.world().get::<ComputedNode>(list_view).unwrap();
        let owner_transform = app.world().get::<UiGlobalTransform>(list_view).unwrap();
        let owner_native = gui
            .store()
            .local_to_global(list, fairygui_core::Point::ZERO)
            .unwrap();
        for id in [row, bar] {
            let view = gui.entity_of(id).unwrap();
            let node = app.world().get::<ComputedNode>(view).unwrap();
            let transform = app.world().get::<UiGlobalTransform>(view).unwrap();
            let point = gui
                .store()
                .local_to_global(id, fairygui_core::Point::ZERO)
                .unwrap();
            let expected = owner_transform.transform_point2(owner_node.content_box().min)
                + owner_transform.transform_vector2(
                    Vec2::new(point.x - owner_native.x, point.y - owner_native.y)
                        / owner_node.inverse_scale_factor(),
                );
            let actual = transform.transform_point2(node.content_box().min);
            assert!(
                (actual - expected).length() <= 1.,
                "{margin:?} {id:?}: {actual:?} vs {expected:?}"
            );
        }
    }
}

#[test]
fn ecs_view_size_resizes_outer_geometry_rows_and_clips_in_the_same_frame() {
    let (mut app, _, list) = fixture_with_layout(true);
    app.add_systems(
        Update,
        crate::properties::sync
            .after(spawn_pending_objects)
            .before(sync_dirty),
    );
    app.add_systems(Update, sync_scroll_extents.after(sync_dirty));
    app.add_systems(
        PostUpdate,
        project_scroll_overflow
            .after(bevy::ui::UiSystems::Layout)
            .before(bevy::ui::update::update_clipping_system),
    );
    app.add_systems(
        PostUpdate,
        crate::native_clipping::project.after(bevy::ui::update::update_clipping_system),
    );
    let row = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        for row in gui.store().list_items(list).unwrap() {
            gui.store_mut().remove(row);
        }
        let row = gui
            .store_mut()
            .create(fairygui_core::package::ObjectType::Graph);
        gui.store_mut().set_size(row, 250., 400.);
        gui.store_mut().set_parent(row, Some(list));
        gui.queue_subtree(row);
        row
    };
    app.update();
    let view = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(list)
        .unwrap();
    app.world_mut().entity_mut(view).insert((
        crate::GuiMargin::new(10., 12., 20., 18.),
        crate::GuiViewSize(Vec2::new(180.25, 90.75)),
    ));
    app.update();
    let gui = app.world().non_send::<GuiRuntime>();
    let bar = gui.store().scroll_pane(list).unwrap().vt_bar.unwrap();
    let bar_width = gui.store().object(bar).unwrap().width();
    let object = gui.store().object(list).unwrap();
    assert_eq!(
        gui.store().component_view_size(list),
        Some(fairygui_core::Size::new(180.25, 90.75))
    );
    assert_eq!(object.width(), 210.25 + bar_width);
    assert_eq!(object.height(), 120.75);
    assert_eq!(
        app.world().get::<crate::GuiViewSize>(view).unwrap().0,
        Vec2::new(180.25, 90.75)
    );
    let node = app.world().get::<ComputedNode>(view).unwrap();
    assert!(
        (node.size() * node.inverse_scale_factor())
            .abs_diff_eq(Vec2::new(object.width(), object.height()), 0.001)
    );
    let owner_t = app.world().get::<UiGlobalTransform>(view).unwrap();
    let row_view = gui.entity_of(row).unwrap();
    let clip = app
        .world()
        .get::<bevy::ui::CalculatedClip>(row_view)
        .unwrap();
    for (p, inside) in [(Vec2::new(20., 25.), true), (Vec2::new(20., 105.), false)] {
        let screen =
            owner_t.transform_point2(node.border_box().min + p / node.inverse_scale_factor());
        assert_eq!(clip.contains_point(screen), inside);
    }
}

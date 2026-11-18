use super::*;
use fairygui_core::{Point, package::ObjectType};

fn fixture() -> (App, GObjectId) {
    let (mut app, root, _) = super::list_bar_tests::fixture_with_layout(true);
    app.add_systems(
        Update,
        crate::properties::sync
            .after(spawn_pending_objects)
            .before(sync_dirty),
    );
    let view = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(root)
        .unwrap();
    app.world_mut()
        .entity_mut(view)
        .insert(bevy::ui::LayoutConfig {
            use_rounding: false,
        });
    app.add_systems(
        PostUpdate,
        crate::skew_projection::project
            .after(bevy::ui::UiSystems::Layout)
            .before(project_scroll_overflow)
            .before(bevy::ui::update::update_clipping_system),
    );
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
    (app, root)
}

fn assert_native_points(app: &App, root: GObjectId, objects: &[GObjectId]) {
    let gui = app.world().non_send::<GuiRuntime>();
    let root_view = gui.entity_of(root).unwrap();
    let root_t = app.world().get::<UiGlobalTransform>(root_view).unwrap();
    let root_n = app.world().get::<ComputedNode>(root_view).unwrap();
    for &id in objects {
        let view = gui.entity_of(id).unwrap();
        let t = app.world().get::<UiGlobalTransform>(view).unwrap();
        let n = app.world().get::<ComputedNode>(view).unwrap();
        let object = gui.store().object(id).unwrap();
        let anchor = if object.pivot_as_anchor() {
            Vec2::new(
                object.pivot().x * object.width(),
                object.pivot().y * object.height(),
            )
        } else {
            Vec2::ZERO
        };
        for p in [
            Vec2::ZERO,
            Vec2::new(13., 9.),
            Vec2::new(object.width(), object.height()),
        ] {
            let global = gui
                .store()
                .local_to_global(id, Point::new(p.x, p.y))
                .unwrap();
            let local = gui.store().global_to_local(root, global).unwrap();
            let expected = root_t.transform_point2(
                root_n.border_box().min
                    + Vec2::new(local.x, local.y) / root_n.inverse_scale_factor(),
            );
            let actual =
                t.transform_point2(n.border_box().min + (p + anchor) / n.inverse_scale_factor());
            assert!(
                actual.abs_diff_eq(expected, 1.1),
                "{id:?} at {p:?}: {actual:?} vs {expected:?}"
            );
        }
    }
}

#[test]
fn nested_skews_preserve_pivots_reflections_degenerate_scales_and_reset_without_accumulation() {
    for dpi in [1., 1.75] {
        let (mut app, root) = fixture();
        app.world_mut().resource_mut::<bevy::ui::UiScale>().0 = dpi;
        let (owner, child, image) = {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            let store = gui.store_mut();
            let owner = store.create(ObjectType::Component);
            store.set_parent(owner, Some(root));
            store.set_xy(owner, 130., 90.);
            store.set_size(owner, 120., 80.);
            store.set_pivot(owner, 0.25, 0.75, true);
            store.set_rotation(owner, 27.);
            let child = store.create(ObjectType::Component);
            store.set_parent(child, Some(owner));
            store.set_xy(child, 17., 23.);
            store.set_size(child, 60., 40.);
            store.set_pivot(child, 0.5, 0.25, false);
            store.set_rotation(child, -19.);
            store.set_skew(child, -12., 21.);
            let image = store.create(ObjectType::Image);
            store.set_parent(image, Some(child));
            store.set_size(image, 40., 20.);
            store.set_xy(image, 9., 11.);
            store.set_pivot(image, 0.25, 0.75, true);
            store.set_image_flip(image, crate::FlipType::Horizontal);
            store.set_skew(image, 15., -8.);
            gui.queue_subtree(owner);
            (owner, child, image)
        };
        for (sx, sy, kx, ky) in [
            (-1.2, 0.75, 18., -11.),
            (-1.2, 0.75, 18., -11.),
            (0., 0.75, 28., 14.),
            (1.5, 0., -20., 16.),
            (1., 1., 0., 0.),
            (1., 1., 0., 0.),
        ] {
            {
                let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
                gui.store_mut().set_scale(owner, sx, sy);
            }
            let entity = app.world().non_send::<GuiRuntime>().entity_of(owner);
            if let Some(entity) = entity {
                app.world_mut()
                    .get_mut::<crate::GuiTransform>(entity)
                    .unwrap()
                    .skew = Vec2::new(kx, ky);
            } else {
                app.world_mut()
                    .non_send_mut::<GuiRuntime>()
                    .store_mut()
                    .set_skew(owner, kx, ky);
            }
            app.update();
            app.update();
            assert_native_points(&app, root, &[owner, child, image]);
        }
        {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            gui.store_mut().set_skew(child, 0., 0.);
            gui.store_mut().set_skew(image, 0., 0.);
        }
        app.update();
        assert_native_points(&app, root, &[owner, child, image]);
    }
}

#[test]
fn skewed_pane_and_content_keep_scrollbar_clip_and_application_child_geometry_coherent() {
    let (mut app, root, list) = super::list_bar_tests::fixture_with_layout(true);
    app.add_systems(
        PostUpdate,
        crate::skew_projection::project
            .after(bevy::ui::UiSystems::Layout)
            .before(project_scroll_overflow),
    );
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
    let (row, bar) = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        for row in gui.store().list_items(list).unwrap() {
            gui.store_mut().remove(row);
        }
        gui.store_mut().set_size(list, 300., 100.);
        gui.store_mut().set_pivot(list, 0.5, 0.5, false);
        gui.store_mut().set_skew(list, 20., -15.);
        gui.store_mut()
            .set_component_margin(list, fairygui_core::geom::Margin::new(10., 12., 20., 18.));
        let row = gui.store_mut().create(ObjectType::Component);
        gui.store_mut().set_parent(row, Some(list));
        gui.store_mut().set_size(row, 250., 400.);
        gui.store_mut().set_skew(row, -12., 18.);
        let bar = gui.store().scroll_pane(list).unwrap().vt_bar.unwrap();
        gui.queue_subtree(row);
        (row, bar)
    };
    app.update();
    app.update();
    assert_native_points(&app, root, &[list, row, bar]);
    let gui = app.world().non_send::<GuiRuntime>();
    let row_view = gui.entity_of(row).unwrap();
    let owner = gui.entity_of(list).unwrap();
    let application = app
        .world_mut()
        .spawn((
            Node {
                left: Val::Px(7.),
                top: Val::Px(11.),
                width: Val::Px(20.),
                height: Val::Px(10.),
                position_type: PositionType::Absolute,
                ..default()
            },
            ChildOf(row_view),
        ))
        .id();
    app.update();
    let row_t = app.world().get::<UiGlobalTransform>(row_view).unwrap();
    let row_n = app.world().get::<ComputedNode>(row_view).unwrap();
    let app_t = app.world().get::<UiGlobalTransform>(application).unwrap();
    let app_n = app.world().get::<ComputedNode>(application).unwrap();
    let expected = row_t.transform_point2(
        row_n.border_box().min + Vec2::new(7., 11.) / row_n.inverse_scale_factor(),
    );
    assert!(
        app_t
            .transform_point2(app_n.border_box().min)
            .abs_diff_eq(expected, 0.6)
    );
    let owner_t = app.world().get::<UiGlobalTransform>(owner).unwrap();
    let owner_n = app.world().get::<ComputedNode>(owner).unwrap();
    let clip = app
        .world()
        .get::<bevy::ui::CalculatedClip>(row_view)
        .unwrap();
    for (p, inside) in [(Vec2::new(20., 25.), true), (Vec2::new(5., 25.), false)] {
        let screen =
            owner_t.transform_point2(owner_n.border_box().min + p / owner_n.inverse_scale_factor());
        assert_eq!(clip.contains_point(screen), inside);
    }
}

use super::*;
use bevy::math::{Affine2, Mat2};
use fairygui_core::{
    FuiPackage,
    geom::{Point, Size},
};

#[test]
fn actual_pointer_drag_uses_the_scroll_panes_rotated_and_skewed_local_axes() {
    let (mut app, window) = setup();
    let package = FuiPackage::parse(
        "ListEffect",
        include_bytes!("../../../fairygui-demo/assets/ui/ListEffect.fui"),
    )
    .unwrap();
    let list = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let root = gui
            .create_root(Handle::default(), &package, "Main")
            .unwrap();
        let root = gui.root_object(root).unwrap();
        gui.store_mut().attach_to_stage(root);
        let list = gui.store().child_by_name(root, "mailList").unwrap();
        gui.store_mut().set_size(list, 200., 100.);
        for height in [80., 100., 120.] {
            let row = gui
                .store_mut()
                .create(fairygui_core::package::ObjectType::Component);
            gui.store_mut().set_parent(row, Some(list));
            gui.store_mut().set_size(row, 180., height);
        }
        gui.store_mut().layout_list(list);
        gui.store_mut().layout_scroll_bars(list);
        gui.store_mut()
            .update_scroll_content_size(list, Size::new(180., 500.));
        list
    };
    let (entity, _) = target(&mut app, 100);
    let dpi = 1.75;
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .resolution
        .set_scale_factor_override(Some(dpi));
    let local_to_screen =
        Mat2::from_angle(0.73) * Mat2::from_cols(Vec2::new(1.2, 0.35), Vec2::new(-0.2, 0.85));
    app.world_mut().entity_mut(entity).insert((
        GObjectRef(list),
        ComputedNode {
            size: Vec2::new(200., 100.) * dpi,
            inverse_scale_factor: 1. / dpi,
            ..default()
        },
        UiGlobalTransform::from(Affine2 {
            matrix2: local_to_screen,
            translation: Vec2::new(180., 140.) * dpi,
        }),
    ));

    let start = Vec2::new(150., 120.);
    let local_drag = Vec2::new(0., -40.);
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(start));
    press(&mut app);
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(start + local_to_screen * local_drag));
    app.update();

    let pane = app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .scroll_pane(list)
        .unwrap();
    assert!((pane.display_pos().y - 40.).abs() < 0.01);
    assert_eq!(pane.display_pos().x, 0.);
}

#[test]
fn actual_pointer_drag_releases_to_item_snap_after_a_stationary_hold_without_clicking() {
    let (mut app, window) = setup();
    let package = FuiPackage::parse(
        "ListEffect",
        include_bytes!("../../../fairygui-demo/assets/ui/ListEffect.fui"),
    )
    .unwrap();
    let list = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let index = gui
            .create_root(Handle::default(), &package, "Main")
            .unwrap();
        let root = gui.root_object(index).unwrap();
        gui.store_mut().attach_to_stage(root);
        let list = gui.store().child_by_name(root, "mailList").unwrap();
        gui.store_mut().set_size(list, 200., 100.);
        for height in [30., 60., 80., 100., 120.] {
            let row = gui.store_mut().create(ObjectType::Component);
            gui.store_mut().set_parent(row, Some(list));
            gui.store_mut().set_size(row, 180., height);
        }
        gui.store_mut().layout_list(list);
        gui.store_mut().layout_scroll_bars(list);
        list
    };
    let (entity, _) = target(&mut app, 100);
    app.world_mut().entity_mut(entity).insert(GObjectRef(list));
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(Vec2::new(10., 100.)));
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_millis(16));
    press(&mut app);
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(Vec2::new(10., 45.)));
    app.update();
    {
        let gui = app.world().non_send::<GuiRuntime>();
        let pane = gui.store().scroll_pane(list).unwrap();
        assert_eq!(pane.display_pos().y, 55.);
        assert_eq!(pane.pos().y, 55.);
    }
    // A hold ages the last movement; it must not keep renewing its velocity.
    for _ in 0..30 {
        app.update();
    }
    release(&mut app);
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let pane = gui.store().scroll_pane(list).unwrap();
        assert!(pane.is_animating());
        assert_eq!(
            pane.pos().y,
            55.,
            "type-2 logical position follows display, not its future endpoint"
        );
        gui.store_mut().advance_scrolls(0.3);
        assert_eq!(
            gui.store().scroll_pane(list).unwrap().pos(),
            Point::new(0., 30.)
        );
        assert!(!gui.store().scroll_pane(list).unwrap().is_animating());
        assert!(
            gui.drain_events()
                .all(|event| event.kind != EventKind::Click)
        );
    }
    // Out-of-range display must remain logically clamped and still mark dirty.
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut().set_scroll_pos(list, 0., 0., false);
        gui.store_mut().begin_scroll_drag(list);
        gui.store_mut().take_dirty();
        assert!(gui.store_mut().drag_scroll_by(list, 0., 20., 0.1));
        let pane = gui.store().scroll_pane(list).unwrap();
        assert_eq!(pane.pos().y, 0.);
        assert_eq!(pane.display_pos().y, -10.);
        assert!(
            gui.store_mut()
                .take_dirty()
                .iter()
                .any(|(id, _)| *id == list)
        );
        gui.store_mut().end_scroll_drag(list);
        gui.store_mut().advance_scrolls(0.3);
        assert_eq!(
            gui.store().scroll_pane(list).unwrap().display_pos(),
            Point::ZERO
        );
        gui.store_mut()
            .update_scroll_content_size(list, Size::new(180., 500.));
    }
}

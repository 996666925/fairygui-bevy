use super::*;
use fairygui_core::{FuiPackage, geom::Point};

#[test]
fn exported_item_snap_is_used_by_line_input_while_pixel_input_and_flag_changes_remain_explicit() {
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
        // This fixture picks the list rectangle without materialized row graphics.
        gui.store_mut().set_component_opaque(list, true);
        gui.store_mut().set_size(list, 200., 100.);
        for height in [30., 60., 80., 100., 120.] {
            let row = gui.store_mut().create(ObjectType::Component);
            gui.store_mut().set_parent(row, Some(list));
            gui.store_mut().set_size(row, 180., height);
        }
        gui.store_mut().layout_list(list);
        gui.store_mut().layout_scroll_bars(list);
        assert_eq!(gui.store().scroll_snap_to_item(list), Some(true));
        assert_eq!(gui.store().scroll_mouse_wheel_enabled(list), Some(false));
        list
    };
    let (entity, _) = target(&mut app, 100);
    app.world_mut().entity_mut(entity).insert(GObjectRef(list));
    let send = |app: &mut App, unit, y| {
        app.world_mut().write_message(MouseWheel {
            unit,
            x: 0.,
            y,
            window,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
        app.update();
    };
    send(&mut app, MouseScrollUnit::Line, -1.);
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_pane(list)
            .unwrap()
            .pos(),
        Point::ZERO
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_scroll_mouse_wheel_enabled(list, true);
    send(&mut app, MouseScrollUnit::Line, -1.);
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
    send(&mut app, MouseScrollUnit::Line, -1.);
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_pane(list)
            .unwrap()
            .pos()
            .y,
        90.
    );
    send(&mut app, MouseScrollUnit::Pixel, -0.25);
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_pane(list)
            .unwrap()
            .display_pos()
            .y,
        90.25
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_scroll_snap_to_item(list, false);
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_pane(list)
            .unwrap()
            .pos()
            .y,
        90.25
    );
    send(&mut app, MouseScrollUnit::Line, -1.);
    let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
    assert_eq!(gui.store().scroll_pane(list).unwrap().pos().y, 140.25);
    assert!(gui.drain_events().all(|e| e.kind != EventKind::Click));
}

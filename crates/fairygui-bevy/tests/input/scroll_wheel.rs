use super::*;
use fairygui_core::{
    FuiPackage,
    geom::{Point, Size},
    scroll::ScrollType,
};

fn send(app: &mut App, window: Entity, unit: MouseScrollUnit, x: f32, y: f32) {
    app.world_mut().write_message(MouseWheel {
        unit,
        x,
        y,
        window,
        phase: bevy::input::touch::TouchPhase::Moved,
    });
    app.update();
}

fn exported(app: &mut App, kind: ScrollType) -> GObjectId {
    let package = FuiPackage::parse(
        "Basics",
        include_bytes!("../../../fairygui-core/tests/fixtures/Basics.fui"),
    )
    .unwrap();
    let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
    let index = gui
        .create_root(Handle::default(), &package, "Demo_List")
        .unwrap();
    let root = gui.root_object(index).unwrap();
    gui.store_mut().attach_to_stage(root);
    let mut children = Vec::new();
    gui.store()
        .for_each_descendant(root, &mut |id| children.push(id));
    let list = children
        .into_iter()
        .find(|&id| {
            gui.store()
                .scroll_pane(id)
                .is_some_and(|p| p.scroll_type == kind && !p.page_mode)
        })
        .unwrap();
    gui.store_mut().set_size(list, 100., 80.);
    gui.store_mut().layout_scroll_bars(list);
    gui.store_mut()
        .update_scroll_content_size(list, Size::new(1000., 1000.));
    gui.store_mut().set_scroll_mouse_wheel_enabled(list, true);
    list
}

#[test]
fn actual_wheel_router_uses_raw_nested_hit_and_bubbles_through_disabled_or_bounded_inner_panes() {
    let (mut app, window) = setup();
    let outer = exported(&mut app, ScrollType::Vertical);
    let inner = exported(&mut app, ScrollType::Horizontal);
    let (_, leaf) = target(&mut app, 50);
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let button = gui.store_mut().create(ObjectType::Button);
        gui.store_mut().set_parent(button, Some(outer));
        gui.store_mut().set_parent(inner, Some(button));
        gui.store_mut().set_parent(leaf, Some(inner));
        gui.store_mut().set_scroll_step(inner, 12.5);
    }
    send(&mut app, window, MouseScrollUnit::Line, 0., -100.);
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        assert_eq!(
            gui.store().scroll_pane(inner).unwrap().pos(),
            Point::new(25., 0.)
        );
        assert_eq!(
            gui.store().scroll_pane(outer).unwrap().pos(),
            Point::new(0., 50.)
        );
        let events: Vec<_> = gui
            .drain_events()
            .filter(|e| e.kind == EventKind::MouseWheel)
            .collect();
        assert!(events.iter().all(|e| e.target == leaf));
        assert!(events.iter().any(|e| e.current_target == inner));
        assert!(events.iter().any(|e| e.current_target == outer));
        gui.store_mut().set_scroll_mouse_wheel_enabled(inner, false);
    }
    send(&mut app, window, MouseScrollUnit::Line, 0., -0.01);
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_pane(outer)
            .unwrap()
            .pos()
            .y,
        100.
    );
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_pane(inner)
            .unwrap()
            .pos()
            .x,
        25.
    );
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut().set_scroll_mouse_wheel_enabled(inner, true);
        gui.store_mut().set_scroll_pos(inner, 900., 0., false);
    }
    send(&mut app, window, MouseScrollUnit::Line, 0., -1.);
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_pane(outer)
            .unwrap()
            .pos()
            .y,
        150.
    );
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_pane(inner)
            .unwrap()
            .pos()
            .x,
        900.
    );
    send(&mut app, window, MouseScrollUnit::Line, 1., 0.);
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_pane(outer)
            .unwrap()
            .pos()
            .y,
        150.,
        "native line input ignores horizontal-only DOM delta"
    );
    send(&mut app, window, MouseScrollUnit::Pixel, 0., 0.25);
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_pane(outer)
            .unwrap()
            .pos()
            .y,
        149.75
    );
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_pane(inner)
            .unwrap()
            .pos()
            .x,
        899.75
    );
}

#[test]
fn exported_hidden_page_pane_defaults_wheel_off_then_explicit_enable_uses_viewport_steps() {
    let (mut app, window) = setup();
    let package = FuiPackage::parse(
        "ScrollPane",
        include_bytes!("../../../fairygui-demo/assets/ui/ScrollPane.fui"),
    )
    .unwrap();
    let pane = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let index = gui
            .create_root(Handle::default(), &package, "item")
            .unwrap();
        let root = gui.root_object(index).unwrap();
        gui.store_mut().attach_to_stage(root);
        gui.store_mut().set_size(root, 100., 80.);
        gui.store_mut().layout_scroll_bars(root);
        gui.store_mut()
            .update_scroll_content_size(root, Size::new(1000., 80.));
        assert!(gui.store().scroll_pane(root).unwrap().page_mode);
        assert_eq!(gui.store().scroll_mouse_wheel_enabled(root), Some(false));
        root
    };
    let (entity, _) = target(&mut app, 50);
    app.world_mut().entity_mut(entity).insert(GObjectRef(pane));
    send(&mut app, window, MouseScrollUnit::Line, 0., -1.);
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_pane(pane)
            .unwrap()
            .pos()
            .x,
        0.
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_scroll_mouse_wheel_enabled(pane, true);
    send(&mut app, window, MouseScrollUnit::Line, 0., -0.001);
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_pane(pane)
            .unwrap()
            .pos()
            .x,
        100.
    );
    send(&mut app, window, MouseScrollUnit::Pixel, 0., -0.25);
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_pane(pane)
            .unwrap()
            .pos()
            .x,
        200.
    );
    app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
    send(&mut app, window, MouseScrollUnit::Line, 0., -1.);
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_pane(pane)
            .unwrap()
            .pos()
            .x,
        200.
    );
}

#[test]
fn release_closing_window_rejects_same_frame_wheel_on_disposed_hit() {
    let (mut app, window) = setup();
    let pane = exported(&mut app, ScrollType::Vertical);
    let package = FuiPackage::parse(
        "MainMenu",
        include_bytes!("../../../fairygui-core/tests/fixtures/MainMenu.fui"),
    )
    .unwrap();
    let (index, close) = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let index = gui
            .show_window(&package, "Main", true, Size::new(800., 600.))
            .unwrap();
        let owner = gui.window(index).unwrap().owner();
        gui.store_mut().set_parent(pane, Some(owner));
        let close = gui.store_mut().create(ObjectType::Button);
        gui.store_mut().set_parent(close, Some(pane));
        gui.window_mut(index).unwrap().set_close_button(close);
        (index, close)
    };
    let (entity, _) = target(&mut app, 100);
    app.world_mut().entity_mut(entity).insert(GObjectRef(close));
    press(&mut app);
    assert_eq!(
        app.world().resource::<GuiInputState>().captured(),
        Some(close)
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .drain_events()
        .for_each(drop);
    app.world_mut().write_message(MouseWheel {
        unit: MouseScrollUnit::Line,
        x: 0.,
        y: -1.,
        window,
        phase: bevy::input::touch::TouchPhase::Moved,
    });
    release(&mut app);
    let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
    assert!(gui.window(index).is_none());
    assert!(!gui.store().contains(close));
    assert!(!gui.store().contains(pane));
    assert!(gui.drain_events().all(|e| e.kind != EventKind::MouseWheel));
}

#[test]
fn release_hiding_combo_popup_rejects_same_frame_wheel_on_retained_off_stage_pane() {
    let (mut app, window) = setup();
    let package = FuiPackage::parse(
        "Basics",
        include_bytes!("../../../fairygui-core/tests/fixtures/Basics.fui"),
    )
    .unwrap();
    let handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: package.clone(),
            res_key: "ui/Basics".into(),
        });
    let combo = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let root = gui.create_root(handle, &package, "Demo_ComboBox").unwrap();
        let root = gui.root_object(root).unwrap();
        gui.store_mut().attach_to_stage(root);
        gui.store().child_by_name(root, "n1").unwrap()
    };
    let (combo_entity, _) = target(&mut app, 50);
    app.world_mut()
        .entity_mut(combo_entity)
        .insert(GObjectRef(combo));
    press(&mut app);
    release(&mut app);
    let pane = exported(&mut app, ScrollType::Vertical);
    let leaf = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let list = gui.store().combo_popup(combo).unwrap().list;
        let item = gui.store().child_at(list, 0).unwrap();
        gui.store_mut().set_parent(pane, Some(item));
        let leaf = gui.store_mut().create(ObjectType::Button);
        gui.store_mut().set_parent(leaf, Some(pane));
        gui.store_mut().set_size(leaf, 100., 1000.);
        leaf
    };
    let (entity, _) = target(&mut app, 100);
    app.world_mut().entity_mut(entity).insert(GObjectRef(leaf));
    app.world_mut()
        .entity_mut(combo_entity)
        .insert(Hovered(false));
    press(&mut app);
    assert_eq!(
        app.world().resource::<GuiInputState>().captured(),
        Some(leaf)
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .drain_events()
        .for_each(drop);
    app.world_mut().write_message(MouseWheel {
        unit: MouseScrollUnit::Line,
        x: 0.,
        y: -1.,
        window,
        phase: bevy::input::touch::TouchPhase::Moved,
    });
    release(&mut app);
    let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
    assert!(gui.popup_stack().is_empty());
    assert!(!gui.store().is_on_stage(leaf));
    assert_eq!(gui.store().scroll_pane(pane).unwrap().pos(), Point::ZERO);
    assert!(gui.drain_events().all(|e| e.kind != EventKind::MouseWheel));
}

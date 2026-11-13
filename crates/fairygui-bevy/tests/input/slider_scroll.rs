//! A track press bubbles into a row/pane; a grip press stops at the grip.
use super::*;
use bevy::input::touch::TouchPhase;
use fairygui_core::{FuiPackage, Size, widget::ControlData};

struct Fixture {
    app: App,
    window: Entity,
    pane: GObjectId,
    row: GObjectId,
    slider: GObjectId,
    grip: GObjectId,
    hit: GObjectId,
    entity: Entity,
}
fn fixture(grip_hit: bool) -> Fixture {
    let (mut app, window) = setup();
    let (entity, hit) = target(&mut app, 100);
    let (pane, row, slider, grip) = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let list = FuiPackage::parse(
            "ListEffect",
            include_bytes!("../../../fairygui-demo/assets/ui/ListEffect.fui"),
        )
        .unwrap();
        let root = gui.create_root(Handle::default(), &list, "Main").unwrap();
        let root = gui.root_object(root).unwrap();
        gui.store_mut().attach_to_stage(root);
        gui.store_mut().set_xy(root, 0., 0.);
        let pane = gui.store().child_by_name(root, "mailList").unwrap();
        gui.store_mut().set_xy(pane, 0., 0.);
        gui.store_mut().set_size(pane, 300., 200.);
        gui.store_mut()
            .update_scroll_content_size(pane, Size::new(300., 1000.));
        let row = gui.store_mut().create(ObjectType::Button);
        gui.store_mut().set_parent(row, Some(pane));
        gui.store_mut().set_size(row, 300., 30.);
        gui.store_mut().set_xy(row, 0., 80.);
        let basics = FuiPackage::parse(
            "Basics",
            include_bytes!("../../../fairygui-demo/assets/ui/Basics.fui"),
        )
        .unwrap();
        let slider = gui
            .create_root(Handle::default(), &basics, "Slider_HZ")
            .unwrap();
        let slider = gui.root_object(slider).unwrap();
        gui.store_mut().set_parent(slider, Some(row));
        gui.store_mut().set_xy(slider, 0., 0.);
        gui.store_mut().set_size(slider, 200., 23.);
        let grip = gui.store().child_by_name(slider, "grip").unwrap();
        let parent = if grip_hit {
            grip
        } else {
            gui.store().child_by_name(slider, "bar").unwrap()
        };
        gui.store_mut().set_parent(hit, Some(parent));
        gui.store_mut().set_size(hit, 10., 10.);
        (pane, row, slider, grip)
    };
    Fixture {
        app,
        window,
        pane,
        row,
        slider,
        grip,
        hit,
        entity,
    }
}
fn value(f: &Fixture) -> f64 {
    f.app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .slider_value(f.slider)
        .unwrap()
}
fn scroll(f: &Fixture) -> f32 {
    f.app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .scroll_pane(f.pane)
        .unwrap()
        .display_pos()
        .y
}
fn events(f: &mut Fixture) -> Vec<fairygui_core::object::UiEvent> {
    f.app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .drain_events()
        .collect()
}
fn cursor(f: &mut Fixture, position: Vec2) {
    f.app
        .world_mut()
        .get_mut::<Window>(f.window)
        .unwrap()
        .set_cursor_position(Some(position));
}
fn assert_down_path(
    f: &Fixture,
    events: &[fairygui_core::object::UiEvent],
    grip: bool,
    touch: Option<u64>,
) {
    let path: Vec<_> = events
        .iter()
        .filter(|e| e.kind == EventKind::MouseDown && e.touch_id == touch)
        .map(|e| e.current_target)
        .collect();
    if grip {
        // This exported grip is itself a button. Its changing skin children
        // resolve to that button, while its row must never replace it.
        assert_eq!(path, [f.grip]);
        let ControlData::Button(row) = f
            .app
            .world()
            .non_send::<GuiRuntime>()
            .store()
            .object(f.row)
            .unwrap()
            .control()
        else {
            panic!("button row")
        };
        assert!(!row.pressed);
    } else {
        assert_eq!(
            path.first(),
            Some(&f.hit),
            "the parent button must not replace the track hit"
        );
        assert!(path.contains(&f.slider) && path.contains(&f.row) && path.contains(&f.pane));
    }
}

#[test]
fn mouse_track_changes_once_then_allows_the_parent_pane_to_scroll() {
    for enabled in [true, false] {
        let mut f = fixture(false);
        f.app
            .world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_slider_change_on_click(f.slider, enabled);
        cursor(&mut f, Vec2::new(150., 95.));
        press(&mut f.app);
        let pressed = value(&f);
        assert_eq!(pressed > 50., enabled);
        assert_eq!(captured_object(&f.app), Some(f.hit));
        let down = events(&mut f);
        assert_down_path(&f, &down, false, None);
        assert_eq!(
            down.iter()
                .filter(|e| e.kind == EventKind::StateChanged && e.is_at_target())
                .count(),
            usize::from(enabled)
        );
        cursor(&mut f, Vec2::new(150., 55.));
        f.app.update();
        assert!((scroll(&f) - 40.).abs() < 0.01);
        assert_eq!(
            value(&f),
            pressed,
            "track movement must not drag the slider"
        );
        release(&mut f.app);
        assert!(
            events(&mut f)
                .iter()
                .all(|e| e.kind != EventKind::StateChanged && e.kind != EventKind::Click)
        );
    }
}

#[test]
fn mouse_grip_stops_down_at_the_skin_and_keeps_drag_away_from_the_button_and_pane() {
    let mut f = fixture(true);
    cursor(&mut f, Vec2::new(100., 95.));
    press(&mut f.app);
    assert_eq!(value(&f), 50.);
    let down = events(&mut f);
    assert_down_path(&f, &down, true, None);
    cursor(&mut f, Vec2::new(147., 55.)); // quarter of the 188px track
    f.app.update();
    assert!((value(&f) - 75.).abs() < 0.001);
    assert_eq!(scroll(&f), 0.);
    release(&mut f.app);
    assert!(
        events(&mut f)
            .iter()
            .any(|e| e.kind == EventKind::MouseUp && e.current_target == f.pane),
        "only MouseDown is stopped"
    );
    f.app
        .world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Right);
    f.app.update();
    assert!(
        events(&mut f)
            .iter()
            .any(|e| e.kind == EventKind::RightMouseDown && e.current_target == f.pane)
    );
}

#[test]
fn touch_track_preserves_the_finger_id_and_allows_parent_scroll_with_either_click_policy() {
    for enabled in [true, false] {
        let mut f = fixture(false);
        f.app.init_resource::<bevy::picking::hover::HoverMap>();
        f.app
            .world_mut()
            .get_mut::<Window>(f.window)
            .unwrap()
            .set_cursor_position(None);
        f.app
            .world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_slider_change_on_click(f.slider, enabled);
        touches::hits(&mut f.app, 71, &[f.entity]);
        touches::send(
            &mut f.app,
            f.window,
            71,
            TouchPhase::Started,
            Vec2::new(150., 95.),
        );
        let pressed = value(&f);
        assert_eq!(pressed > 50., enabled);
        let down = events(&mut f);
        assert_down_path(&f, &down, false, Some(71));
        assert_eq!(
            down.iter()
                .filter(|e| e.kind == EventKind::StateChanged
                    && e.is_at_target()
                    && e.touch_id == Some(71))
                .count(),
            usize::from(enabled)
        );
        touches::send(
            &mut f.app,
            f.window,
            71,
            TouchPhase::Moved,
            Vec2::new(150., 55.),
        );
        assert!((scroll(&f) - 40.).abs() < 0.01);
        assert_eq!(value(&f), pressed);
        touches::send(
            &mut f.app,
            f.window,
            71,
            TouchPhase::Ended,
            Vec2::new(150., 55.),
        );
        assert!(
            events(&mut f)
                .iter()
                .all(|e| e.kind != EventKind::StateChanged && e.kind != EventKind::Click)
        );
    }
}

#[test]
fn touch_grip_stops_bubbling_and_a_second_finger_cannot_move_or_finish_its_drag() {
    let mut f = fixture(true);
    f.app.init_resource::<bevy::picking::hover::HoverMap>();
    f.app
        .world_mut()
        .get_mut::<Window>(f.window)
        .unwrap()
        .set_cursor_position(None);
    touches::hits(&mut f.app, 71, &[f.entity]);
    touches::hits(&mut f.app, 72, &[f.entity]);
    touches::send(
        &mut f.app,
        f.window,
        71,
        TouchPhase::Started,
        Vec2::new(100., 95.),
    );
    let down = events(&mut f);
    assert_down_path(&f, &down, true, Some(71));
    assert!(
        down.iter()
            .filter(|e| e.kind == EventKind::TouchBegin)
            .all(|e| e.current_target == f.hit || e.current_target == f.grip)
    );
    touches::send(
        &mut f.app,
        f.window,
        72,
        TouchPhase::Started,
        Vec2::new(100., 95.),
    );
    touches::send(
        &mut f.app,
        f.window,
        72,
        TouchPhase::Moved,
        Vec2::new(194., 55.),
    );
    assert_eq!(value(&f), 50.);
    touches::send(
        &mut f.app,
        f.window,
        72,
        TouchPhase::Ended,
        Vec2::new(194., 55.),
    );
    touches::send(
        &mut f.app,
        f.window,
        71,
        TouchPhase::Moved,
        Vec2::new(147., 55.),
    );
    assert!((value(&f) - 75.).abs() < 0.001);
    assert_eq!(scroll(&f), 0.);
    let changes = events(&mut f);
    assert!(
        changes
            .iter()
            .filter(|e| e.kind == EventKind::StateChanged)
            .all(|e| e.touch_id == Some(71))
    );
    touches::send(
        &mut f.app,
        f.window,
        71,
        TouchPhase::Ended,
        Vec2::new(147., 55.),
    );
    assert_eq!(captured_touch_object(&f.app, 71), None);
}

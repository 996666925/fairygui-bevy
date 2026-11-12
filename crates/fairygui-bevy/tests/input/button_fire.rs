use super::*;
use fairygui_core::{
    geom::{Point, Size},
    package::FuiPackage,
    widget::{ButtonState, ControlData},
};
use std::time::Duration;

#[test]
fn programmatic_check_click_changes_its_related_page_immediately_and_emits_only_state_changed() {
    let (mut app, _) = setup();
    let package = FuiPackage::parse(
        "Basics",
        include_bytes!("../../../fairygui-core/tests/fixtures/Basics.fui"),
    )
    .unwrap();
    let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
    let root = gui
        .create_root(Handle::default(), &package, "Demo_Controller")
        .unwrap();
    let root = gui.root_object(root).unwrap();
    let button = gui.store().child_by_name(root, "n11").unwrap();
    gui.ensure_root(Size::new(1280.0, 720.0));
    let popup = gui.store_mut().create(ObjectType::Component);
    gui.store_mut().set_button_linked_popup(button, Some(popup));
    let other_popup = gui.store_mut().create(ObjectType::Component);
    gui.show_popup(other_popup, Point::ZERO);
    gui.store_mut().set_visible(button, false);
    gui.store_mut().set_grayed(button, true);
    assert!(
        gui.fire_button_click(button, true),
        "explicit calls work while hidden/grayed/offstage"
    );
    assert_eq!(gui.store().button_selected(button), Some(true));
    assert_eq!(
        gui.popup_stack(),
        &[other_popup],
        "no root check or linked-popup mouse-down action"
    );
    assert!(
        matches!(gui.store().object(root).unwrap().control(), ControlData::Component(data) if data.controller("c1").unwrap().selected_index == 1)
    );
    let events: Vec<_> = gui.drain_events().collect();
    assert!(!events.is_empty());
    assert!(
        events
            .iter()
            .all(|event| event.kind == EventKind::StateChanged && event.pointer.is_none())
    );
    assert_eq!(
        events.iter().filter(|event| event.is_at_target()).count(),
        1
    );
    assert!(
        events
            .iter()
            .any(|event| event.current_target == root && event.bubbling)
    );
    gui.store_mut()
        .set_button_change_state_on_click(button, false);
    assert!(gui.fire_button_click(button, false));
    assert_eq!(gui.store().button_selected(button), Some(true));
    assert_eq!(gui.drain_events().count(), 0);
    assert!(!gui.fire_button_click(popup, true));
}

#[test]
fn common_feedback_advances_without_a_window_and_never_acquires_pointer_capture_or_dispatches_click()
 {
    let (mut app, window) = setup();
    app.world_mut().despawn(window);
    let button = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let button = gui.store_mut().create(ObjectType::Button);
        gui.store_mut().set_button_down_effect(button, 2, 0.8);
        assert!(gui.fire_button_click(button, true));
        button
    };
    let state = |app: &App| {
        let gui = app.world().non_send::<GuiRuntime>();
        let object = gui.store().object(button).unwrap();
        let ControlData::Button(data) = object.control() else {
            unreachable!()
        };
        (
            object.scale_x(),
            data.interaction_state,
            data.hovered,
            data.pressed,
        )
    };
    assert_eq!(state(&app), (1.0, ButtonState::Over, false, false));
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(99));
    app.update();
    assert_eq!(state(&app).1, ButtonState::Over);
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(1));
    app.update();
    assert_eq!(state(&app), (0.8, ButtonState::Down, false, false));
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(100));
    app.update();
    assert_eq!(state(&app), (1.0, ButtonState::Up, false, false));
    let input = app.world().resource::<GuiInputState>();
    assert!(input.captured().is_none());
    let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
    assert_eq!(gui.drain_events().count(), 0);
    assert!(gui.fire_button_click(button, false));
    assert_eq!(gui.drain_events().count(), 0);
}

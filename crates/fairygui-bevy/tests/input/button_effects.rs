use super::*;
use fairygui_core::geom::{Point, Size};

fn button(app: &mut App) -> (Entity, GObjectId) {
    let (entity, _) = target(app, 10);
    let id = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let id = gui.store_mut().create(ObjectType::Button);
        gui.store_mut().set_xy(id, 60.0, 40.0);
        gui.store_mut().set_size(id, 100.0, 30.0);
        gui.store_mut().attach_to_stage(id);
        id
    };
    app.world_mut().entity_mut(entity).insert(GObjectRef(id));
    (entity, id)
}

#[test]
fn pointer_capture_preserves_scale_when_leaving_and_restores_it_on_focus_loss() {
    let (mut app, window) = setup();
    let (entity, id) = button(&mut app);
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut().set_scale(id, 2.0, 3.0);
        assert!(gui.store_mut().set_button_down_effect(id, 2, 0.8));
    }
    press(&mut app);
    let scale = |app: &App| {
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .object(id)
            .unwrap()
            .scale_x()
    };
    assert!((scale(&app) - 1.6).abs() < 0.0001);
    app.world_mut().entity_mut(entity).insert(Hovered(false));
    app.update();
    assert!(
        (scale(&app) - 1.6).abs() < 0.0001,
        "roll-out is not mouse-up"
    );
    app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
    app.update();
    assert!((scale(&app) - 2.0).abs() < 0.0001);
    release(&mut app);
    assert!(clicks(&mut app).is_empty());
}

#[test]
fn linked_popup_opens_on_press_retains_views_and_does_not_reopen_on_its_closing_press() {
    let (mut app, _) = setup();
    let (entity, id) = button(&mut app);
    let popup = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let popup = gui.store_mut().create(ObjectType::Component);
        gui.store_mut().set_size(popup, 120.0, 80.0);
        gui.store_mut().set_button_linked_popup(id, Some(popup));
        popup
    };
    press(&mut app);
    {
        let gui = app.world().non_send::<GuiRuntime>();
        assert_eq!(gui.popup_stack(), &[popup]);
        assert_eq!(
            gui.store().object(popup).unwrap().position(),
            Point::new(60.0, 70.0)
        );
        assert!(gui.store().is_on_stage(popup));
    }
    release(&mut app);
    assert_eq!(app.world().non_send::<GuiRuntime>().popup_stack(), &[popup]);
    press(&mut app);
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .popup_stack()
            .is_empty()
    );
    release(&mut app);
    press(&mut app);
    assert_eq!(app.world().non_send::<GuiRuntime>().popup_stack(), &[popup]);
    release(&mut app);
    // Clicking inside the popup keeps it; the next background press closes it.
    let (inside, child) = target(&mut app, 20);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_parent(child, Some(popup));
    press(&mut app);
    assert_eq!(app.world().non_send::<GuiRuntime>().popup_stack(), &[popup]);
    release(&mut app);
    app.world_mut().entity_mut(inside).insert(Hovered(false));
    app.world_mut().entity_mut(entity).insert(Hovered(false));
    press(&mut app);
    let gui = app.world().non_send::<GuiRuntime>();
    assert!(gui.popup_stack().is_empty());
    assert!(gui.store().contains(popup));
    assert!(!gui.store().is_on_stage(popup));
}

#[test]
fn linked_window_uses_window_visibility_and_preserves_its_modal_registration() {
    let (mut app, _) = setup();
    let (_, id) = button(&mut app);
    let package = fairygui_core::package::FuiPackage::parse(
        "MainMenu",
        include_bytes!("../../../fairygui-core/tests/fixtures/MainMenu.fui"),
    )
    .unwrap();
    let index = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let index = gui
            .show_window(&package, "Main", false, Size::new(800.0, 600.0))
            .unwrap();
        let owner = gui.window(index).unwrap().owner();
        gui.hide_window(index);
        gui.store_mut().set_button_linked_popup(id, Some(owner));
        index
    };
    press(&mut app);
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .window(index)
            .unwrap()
            .is_showing()
    );
    release(&mut app);
    press(&mut app);
    let gui = app.world().non_send::<GuiRuntime>();
    assert!(!gui.window(index).unwrap().is_showing());
    assert_eq!(gui.window_count(), 1);
    assert!(gui.popup_stack().is_empty());
}

#[test]
fn nested_popup_checks_close_only_the_descendants_above_the_clicked_popup() {
    let (mut app, _) = setup();
    let (entity, id) = button(&mut app);
    let (first, second) = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.ensure_root(Size::new(800.0, 600.0));
        let first = gui.store_mut().create(ObjectType::Component);
        let second = gui.store_mut().create(ObjectType::Component);
        gui.show_popup(first, Point::ZERO);
        gui.show_popup(second, Point::ZERO);
        gui.store_mut().set_parent(id, Some(first));
        (first, second)
    };
    press(&mut app);
    let gui = app.world().non_send::<GuiRuntime>();
    assert_eq!(gui.popup_stack(), &[first]);
    assert!(!gui.store().is_on_stage(second));
    assert!(app.world().get::<Hovered>(entity).unwrap().0);
}

#[test]
fn popup_above_a_modal_window_can_receive_input_and_a_later_modal_blocks_it() {
    let (mut app, _) = setup();
    let package = fairygui_core::package::FuiPackage::parse(
        "MainMenu",
        include_bytes!("../../../fairygui-core/tests/fixtures/MainMenu.fui"),
    )
    .unwrap();
    let (_, id) = button(&mut app);
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.show_window(&package, "Main", true, Size::new(800.0, 600.0))
            .unwrap();
        gui.show_popup(id, Point::ZERO);
        assert!(gui.is_in_top_modal(id));
    }
    press(&mut app);
    release(&mut app);
    assert_eq!(clicks(&mut app).len(), 1);
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.show_window(&package, "Main", true, Size::new(800.0, 600.0))
            .unwrap();
        assert!(!gui.is_in_top_modal(id));
    }
    press(&mut app);
    release(&mut app);
    assert!(clicks(&mut app).is_empty());
}

#[test]
fn internal_root_rectangle_does_not_occlude_buttons_when_retained_popups_are_closed() {
    let (mut app, _) = setup();
    let (_, id) = button(&mut app);
    let (root, popup) = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let root = gui.ensure_root(Size::new(800.0, 600.0));
        let popup = gui.store_mut().create(ObjectType::Component);
        gui.store_mut().set_button_linked_popup(id, Some(popup));
        (root, popup)
    };
    app.world_mut().spawn((
        GObjectRef(root),
        Hovered(true),
        ComputedNode::default(),
        ComputedStackIndex(100),
    ));
    for opened in [true, false, true] {
        press(&mut app);
        assert_eq!(
            app.world()
                .non_send::<GuiRuntime>()
                .popup_stack()
                .contains(&popup),
            opened
        );
        release(&mut app);
    }
}

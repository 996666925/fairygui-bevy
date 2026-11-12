//! Exercise actual routing systems without a renderer or OS window.
#[path = "input/bar_visibility.rs"]
mod bar_visibility;
#[path = "input/button_effects.rs"]
mod button_effects;
#[path = "input/button_fire.rs"]
mod button_fire;
#[path = "input/list_selection.rs"]
mod list_selection;
#[path = "input/polygons.rs"]
mod polygons;
#[path = "input/popup_stack.rs"]
mod popup_stack;
#[path = "input/scroll_alignment.rs"]
mod scroll_alignment;
#[path = "input/scroll_motion.rs"]
mod scroll_motion;
#[path = "input/scroll_pages.rs"]
mod scroll_pages;
#[path = "input/scroll_wheel.rs"]
mod scroll_wheel;
#[path = "input/touches.rs"]
mod touches;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::{ComputedStackIndex, RelativeCursorPosition};
use bevy::window::PrimaryWindow;
use fairygui_bevy::prelude::*;
use fairygui_core::object::{EventKind, GObjectId};
use fairygui_core::package::ObjectType;

fn setup() -> (App, Entity) {
    let mut app = App::new();
    app.init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<Time>()
        .init_resource::<Assets<FuiAsset>>()
        .init_non_send::<GuiRuntime>()
        .add_message::<MouseWheel>();
    fairygui_bevy::input::register(&mut app);
    let mut window = Window {
        focused: true,
        ..default()
    };
    window.set_cursor_position(Some(Vec2::new(10.0, 10.0)));
    let window = app.world_mut().spawn((window, PrimaryWindow)).id();
    (app, window)
}

fn target(app: &mut App, stack: u32) -> (Entity, GObjectId) {
    let id = app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .create(ObjectType::Component);
    let entity = app
        .world_mut()
        .spawn((
            GObjectRef(id),
            Hovered(true),
            ComputedNode::default(),
            ComputedStackIndex(stack),
            RelativeCursorPosition {
                cursor_over: true,
                normalized: Some(Vec2::ZERO),
            },
        ))
        .id();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .attach_to_stage(id);
    (entity, id)
}

fn press(app: &mut App) {
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
}

fn release(app: &mut App) {
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
}

fn press_right(app: &mut App) {
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Right);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
}

fn release_right(app: &mut App) {
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Right);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
}

fn clicks(app: &mut App) -> Vec<fairygui_core::object::UiEvent> {
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .drain_events()
        .filter(|e| e.kind == EventKind::Click && e.is_at_target())
        .collect()
}

#[test]
fn official_check_button_click_drives_controller_and_emits_state_changed_once() {
    let (mut app, _) = setup();
    let package = fairygui_core::package::FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui"),
    )
    .unwrap();
    let button = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let index = gui
            .create_root(Handle::default(), &package, "Demo_Controller")
            .unwrap();
        let root = gui.root_object(index).unwrap();
        gui.store_mut().attach_to_stage(root);
        gui.store().child_by_name(root, "n11").unwrap()
    };
    let (entity, _) = target(&mut app, 10);
    app.world_mut()
        .entity_mut(entity)
        .insert(GObjectRef(button));
    app.update();
    press(&mut app);
    release(&mut app);
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        assert_eq!(gui.store().button_selected(button), Some(true));
        let events: Vec<_> = gui
            .drain_events()
            .filter(|e| e.kind == EventKind::StateChanged && e.is_at_target())
            .collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].target, button);
        assert_eq!(
            events[0].pointer,
            Some(fairygui_core::geom::Point::new(10.0, 10.0))
        );
        let parent = gui.store().object(button).unwrap().parent().unwrap();
        assert_eq!(
            gui.store()
                .object(parent)
                .unwrap()
                .control()
                .component()
                .unwrap()
                .controller("c1")
                .unwrap()
                .selected_index,
            1
        );
    }
    press(&mut app);
    release(&mut app);
    assert_eq!(
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store()
            .button_selected(button),
        Some(false)
    );
}

#[test]
fn a_display_disconnected_child_cannot_hit_until_a_valid_display_lock_connects_it() {
    let (mut app, _) = setup();
    let package = fairygui_core::package::FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui"),
    )
    .unwrap();
    let hidden = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let index = gui
            .create_root(Handle::default(), &package, "Demo_Controller")
            .unwrap();
        let button = gui.root_object(index).unwrap();
        gui.store_mut().attach_to_stage(button);
        gui.store()
            .object(button)
            .unwrap()
            .children()
            .iter()
            .copied()
            .find(|id| {
                let object = gui.store().object(*id).unwrap();
                object.visible() && !object.display_connected()
            })
            .unwrap()
    };
    app.world_mut().spawn((
        GObjectRef(hidden),
        Hovered(true),
        ComputedNode::default(),
        RelativeCursorPosition {
            cursor_over: true,
            normalized: Some(Vec2::ZERO),
        },
    ));
    press(&mut app);
    assert_eq!(app.world().resource::<GuiInputState>().captured(), None);
    release(&mut app);
    let token = app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .add_display_lock(hidden)
        .unwrap();
    press(&mut app);
    assert_eq!(
        app.world().resource::<GuiInputState>().captured(),
        Some(hidden)
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .release_display_lock(hidden, token);
    release(&mut app);
    assert!(clicks(&mut app).is_empty());
}

#[test]
fn bevy_picking_updates_hover_before_routing_and_preserves_pixel_rejection() {
    use bevy::picking::{
        InteractionPlugin, Pickable, PickingPlugin,
        backend::{HitData, PointerHits},
        pointer::PointerId,
    };
    let (mut app, _) = setup();
    app.add_plugins((PickingPlugin, InteractionPlugin));
    app.world_mut().spawn(PointerId::Mouse);
    let camera = app.world_mut().spawn_empty().id();
    let (back_entity, back) = target(&mut app, 1);
    let (front_entity, front) = target(&mut app, 2);
    for entity in [back_entity, front_entity] {
        app.world_mut().entity_mut(entity).insert((
            Hovered(false),
            Pickable {
                should_block_lower: false,
                is_hoverable: true,
            },
        ));
    }
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_size(front, 10.0, 10.0);
    app.world_mut()
        .entity_mut(front_entity)
        .insert(fairygui_bevy::runtime::PixelHitTest(std::sync::Arc::new(
            fairygui_core::package::PixelHitTestData {
                pixel_width: 10,
                scale: 1.0,
                pixels: vec![0; 13],
            },
        )));
    let hits = || {
        PointerHits::new(
            PointerId::Mouse,
            vec![
                (front_entity, HitData::new(camera, 0.0, None, None)),
                (back_entity, HitData::new(camera, 1.0, None, None)),
            ],
            0.5,
        )
    };
    app.world_mut().write_message(hits());
    press(&mut app);
    assert!(app.world().get::<Hovered>(front_entity).unwrap().get());
    assert_eq!(
        app.world().resource::<GuiInputState>().captured(),
        Some(back)
    );
    app.world_mut().write_message(hits());
    release(&mut app);
    let events = clicks(&mut app);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].target, back);
}

#[test]
fn unrelated_blocking_pickable_prevents_gui_pointer_capture() {
    use bevy::picking::{
        InteractionPlugin, Pickable, PickingPlugin,
        backend::{HitData, PointerHits},
        pointer::PointerId,
    };
    let (mut app, _) = setup();
    app.add_plugins((PickingPlugin, InteractionPlugin));
    app.world_mut().spawn(PointerId::Mouse);
    let camera = app.world_mut().spawn_empty().id();
    let (gui_entity, _) = target(&mut app, 1);
    app.world_mut()
        .entity_mut(gui_entity)
        .insert(Hovered(false));
    let blocker = app.world_mut().spawn(Pickable::default()).id();
    app.world_mut().write_message(PointerHits::new(
        PointerId::Mouse,
        vec![
            (blocker, HitData::new(camera, 0.0, None, None)),
            (gui_entity, HitData::new(camera, 1.0, None, None)),
        ],
        0.5,
    ));
    press(&mut app);
    assert_eq!(app.world().resource::<GuiInputState>().captured(), None);
    assert!(!app.world().get::<Hovered>(gui_entity).unwrap().get());
}

#[test]
fn topmost_target_clicks_once_with_pointer() {
    let (mut app, _) = setup();
    target(&mut app, 1);
    let (_, top) = target(&mut app, 10);
    press(&mut app);
    assert_eq!(
        app.world().resource::<GuiInputState>().captured(),
        Some(top)
    );
    release(&mut app);
    let events = clicks(&mut app);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].target, top);
    assert_eq!(events[0].pointer.unwrap().x, 10.0);
}

#[test]
fn moving_away_and_back_does_not_click_after_drag() {
    let (mut app, window) = setup();
    target(&mut app, 0);
    press(&mut app);
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(Vec2::splat(50.0)));
    app.update();
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(Vec2::splat(10.0)));
    release(&mut app);
    assert!(clicks(&mut app).is_empty());
}

#[test]
fn mouse_move_uses_current_hit_while_drag_uses_the_pressed_target() {
    let (mut app, window) = setup();
    let (pressed_entity, pressed_target) = target(&mut app, 1);
    press(&mut app);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .drain_events()
        .for_each(drop);

    app.world_mut()
        .entity_mut(pressed_entity)
        .insert(Hovered(false));
    let (_, move_target) = target(&mut app, 2);
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(Vec2::splat(50.0)));
    app.update();

    let events: Vec<_> = app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .drain_events()
        .collect();
    assert!(events.iter().any(|event| {
        event.kind == EventKind::MouseMove && event.target == move_target && event.is_at_target()
    }));
    assert!(events.iter().any(|event| {
        event.kind == EventKind::MouseDrag && event.target == pressed_target && event.is_at_target()
    }));

    release(&mut app);
    let events: Vec<_> = app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .drain_events()
        .collect();
    let up = events
        .iter()
        .position(|event| event.kind == EventKind::MouseUp && event.target == pressed_target)
        .unwrap();
    let drag_end = events
        .iter()
        .position(|event| event.kind == EventKind::MouseDragEnd && event.target == pressed_target)
        .unwrap();
    assert!(up < drag_end);
    assert!(!events.iter().any(|event| event.kind == EventKind::Click));
}

#[test]
fn release_over_another_target_does_not_click_or_transfer_capture() {
    let (mut app, _) = setup();
    let (entity, first) = target(&mut app, 0);
    press(&mut app);
    app.world_mut().entity_mut(entity).insert(Hovered(false));
    target(&mut app, 2);
    app.update();
    assert_eq!(
        app.world().resource::<GuiInputState>().captured(),
        Some(first)
    );
    release(&mut app);
    assert!(clicks(&mut app).is_empty());
}

#[test]
fn focus_loss_cancels_capture_without_click() {
    let (mut app, window) = setup();
    target(&mut app, 0);
    press(&mut app);
    app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
    app.update();
    assert!(
        !app.world()
            .resource::<GuiInputState>()
            .wants_pointer_input()
    );
    release(&mut app);
    assert!(clicks(&mut app).is_empty());
}

#[test]
fn removed_hidden_or_offstage_capture_is_cancelled() {
    for state in 0..3 {
        let (mut app, _) = setup();
        let (_, id) = target(&mut app, 0);
        press(&mut app);
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        if state == 0 {
            runtime.store_mut().remove(id);
        } else if state == 1 {
            runtime.store_mut().set_visible(id, false);
        } else {
            runtime.store_mut().detach_from_stage(id);
        }
        app.update();
        assert_eq!(app.world().resource::<GuiInputState>().captured(), None);
        release(&mut app);
        assert!(clicks(&mut app).is_empty());
    }
}

#[test]
fn disabled_ancestor_blocks_all_pointer_routes() {
    let (mut app, window) = setup();
    let (_, parent) = target(&mut app, 0);
    let (_, child) = target(&mut app, 2);
    let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
    runtime.store_mut().set_parent(child, Some(parent));
    runtime.store_mut().set_touchable(parent, false);
    app.world_mut().write_message(MouseWheel {
        unit: MouseScrollUnit::Line,
        x: 0.0,
        y: 1.0,
        window,
        phase: bevy::input::touch::TouchPhase::Moved,
    });
    press(&mut app);
    release(&mut app);
    assert_eq!(
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .pending_event_count(),
        0
    );
}

#[test]
fn transparent_pixels_cannot_capture_or_receive_wheel() {
    let (mut app, window) = setup();
    let (entity, id) = target(&mut app, 0);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_size(id, 10.0, 10.0);
    app.world_mut()
        .entity_mut(entity)
        .insert(fairygui_bevy::runtime::PixelHitTest(std::sync::Arc::new(
            fairygui_core::package::PixelHitTestData {
                pixel_width: 10,
                scale: 1.0,
                pixels: vec![0; 13],
            },
        )));
    app.world_mut().write_message(MouseWheel {
        unit: MouseScrollUnit::Pixel,
        x: 0.0,
        y: 5.0,
        window,
        phase: bevy::input::touch::TouchPhase::Moved,
    });
    press(&mut app);
    release(&mut app);
    assert_eq!(
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .pending_event_count(),
        0
    );
}

fn scroll_container(app: &mut App) -> GObjectId {
    let package = fairygui_core::package::FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui"),
    )
    .unwrap();
    let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
    for item in package.items() {
        let Some(index) = gui.create_root(Handle::default(), &package, &item.name) else {
            continue;
        };
        let root = gui.root_object(index).unwrap();
        gui.store_mut().attach_to_stage(root);
        let mut objects = vec![root];
        gui.store()
            .for_each_descendant(root, &mut |id| objects.push(id));
        if let Some(id) = objects
            .into_iter()
            .find(|id| gui.store().scroll_pane(*id).is_some())
        {
            gui.store_mut()
                .update_scroll_content_size(id, fairygui_core::geom::Size::new(2000.0, 2000.0));
            return id;
        }
    }
    panic!("fixture needs a scroll container");
}

#[test]
fn slider_child_owns_drag_instead_of_scroll_parent() {
    let (mut app, window) = setup();
    let container = scroll_container(&mut app);
    let (_, child) = target(&mut app, 3);
    let slider = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let package = fairygui_core::package::FuiPackage::parse(
            "Basics",
            include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui"),
        )
        .unwrap();
        let mut found = None;
        for item in package.items() {
            let Some(index) = gui.create_root(Handle::default(), &package, &item.name) else {
                continue;
            };
            let root = gui.root_object(index).unwrap();
            gui.store_mut().attach_to_stage(root);
            gui.store().for_each_descendant(root, &mut |id| {
                if matches!(gui.store().object(id).map(|o| o.control()),
                    Some(fairygui_core::widget::ControlData::Slider(data)) if data.min == 0 && data.max == 100) {
                    found = Some(id);
                }
            });
            if found.is_some() {
                break;
            }
        }
        let slider = found.expect("Basics contains a configured slider");
        gui.store_mut().set_size(slider, 200.0, 20.0);
        gui.store_mut().set_parent(slider, Some(container));
        gui.store_mut().set_parent(child, Some(slider));
        slider
    };
    let entity = app
        .world_mut()
        .spawn((
            GObjectRef(slider),
            Hovered(true),
            ComputedNode::default(),
            ComputedStackIndex(1),
            RelativeCursorPosition {
                cursor_over: true,
                normalized: Some(Vec2::ZERO),
            },
        ))
        .id();
    let before = app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .scroll_pane(container)
        .unwrap()
        .pos();
    press(&mut app);
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .slider_value(slider),
        Some(50)
    );
    app.world_mut()
        .get_mut::<RelativeCursorPosition>(entity)
        .unwrap()
        .normalized = Some(Vec2::new(1.5, 0.5));
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(Vec2::new(80.0, 80.0)));
    app.update();
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .slider_value(slider),
        Some(100)
    );
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_pane(container)
            .unwrap()
            .pos(),
        before
    );
    release(&mut app);
    assert!(clicks(&mut app).is_empty());
}

#[test]
fn wheel_pixels_and_lines_have_distinct_units_and_ignore_other_windows() {
    let (mut app, window) = setup();
    let container = scroll_container(&mut app);
    let (_, child) = target(&mut app, 3);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_parent(child, Some(container));
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_scroll_mouse_wheel_enabled(container, true);
    for (unit, amount) in [
        (MouseScrollUnit::Pixel, 1.0),
        (
            MouseScrollUnit::Line,
            fairygui_core::scroll::SCROLL_STEP * 2.,
        ),
    ] {
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_scroll_pos(container, 0.0, 100.0, false);
        let before = app
            .world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_pane(container)
            .unwrap()
            .pos()
            .y;
        app.world_mut().write_message(MouseWheel {
            unit,
            x: 0.0,
            y: -1.0,
            window,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
        app.update();
        let after = app
            .world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_pane(container)
            .unwrap()
            .pos()
            .y;
        assert!((after - before - amount).abs() < 0.01);
    }
    let other = app.world_mut().spawn_empty().id();
    let before = app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .scroll_pane(container)
        .unwrap()
        .pos();
    app.world_mut().write_message(MouseWheel {
        unit: MouseScrollUnit::Line,
        x: 0.0,
        y: -1.0,
        window: other,
        phase: bevy::input::touch::TouchPhase::Moved,
    });
    app.update();
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_pane(container)
            .unwrap()
            .pos(),
        before
    );
}

#[test]
fn opening_modal_cancels_existing_background_capture() {
    let (mut app, _) = setup();
    target(&mut app, 1);
    press(&mut app);
    let package = fairygui_core::package::FuiPackage::parse(
        "MainMenu",
        include_bytes!("../../fairygui-core/tests/fixtures/MainMenu.fui"),
    )
    .unwrap();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .show_window(
            &package,
            "Main",
            true,
            fairygui_core::geom::Size::new(800.0, 600.0),
        )
        .unwrap();
    app.update();
    assert_eq!(app.world().resource::<GuiInputState>().captured(), None);
    release(&mut app);
    assert!(clicks(&mut app).is_empty());
}

#[test]
fn demo_listener_on_parent_receives_one_bubbled_click() {
    let (mut app, _) = setup();
    let (_, parent) = target(&mut app, 1);
    let (_, child) = target(&mut app, 2);
    let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
    gui.store_mut().set_parent(child, Some(parent));
    press(&mut app);
    release(&mut app);
    let events: Vec<_> = clicks(&mut app);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].target, child);
    assert!(events[0].pointer.is_some());
}

#[test]
fn right_pointer_events_bubble_without_emitting_left_click_events() {
    let (mut app, _) = setup();
    let (_, parent) = target(&mut app, 1);
    let (_, child) = target(&mut app, 2);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_parent(child, Some(parent));

    press_right(&mut app);
    assert_eq!(
        app.world().resource::<GuiInputState>().captured(),
        Some(child)
    );
    release_right(&mut app);

    let events: Vec<_> = app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .drain_events()
        .collect();
    let pointer_events: Vec<_> = events
        .iter()
        .filter(|event| {
            matches!(
                event.kind,
                EventKind::RightMouseDown | EventKind::RightMouseUp | EventKind::RightClick
            )
        })
        .collect();
    assert_eq!(pointer_events.len(), 6);
    for (event, kind) in pointer_events.iter().zip([
        EventKind::RightMouseDown,
        EventKind::RightMouseDown,
        EventKind::RightMouseUp,
        EventKind::RightMouseUp,
        EventKind::RightClick,
        EventKind::RightClick,
    ]) {
        assert_eq!(event.kind, kind);
    }
    for (event, (current_target, bubbling)) in pointer_events.iter().zip([
        (child, false),
        (parent, true),
        (child, false),
        (parent, true),
        (child, false),
        (parent, true),
    ]) {
        assert_eq!(event.target, child);
        assert_eq!(event.current_target, current_target);
        assert_eq!(event.bubbling, bubbling);
        assert_eq!(
            event.pointer,
            Some(fairygui_core::geom::Point::new(10.0, 10.0))
        );
    }
    assert!(!events.iter().any(|event| {
        matches!(
            event.kind,
            EventKind::MouseDown | EventKind::MouseUp | EventKind::Click | EventKind::ClickItem
        )
    }));
}

#[test]
fn right_release_on_another_target_sends_up_to_capture_but_does_not_click() {
    let (mut app, _) = setup();
    let (entity, captured) = target(&mut app, 1);
    press_right(&mut app);
    app.world_mut().entity_mut(entity).insert(Hovered(false));
    target(&mut app, 2);
    app.update();
    release_right(&mut app);

    let events: Vec<_> = app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .drain_events()
        .collect();
    assert!(events.iter().any(|event| {
        event.kind == EventKind::RightMouseUp && event.target == captured && event.is_at_target()
    }));
    assert!(
        !events
            .iter()
            .any(|event| event.kind == EventKind::RightClick)
    );
    assert!(!events.iter().any(|event| {
        matches!(
            event.kind,
            EventKind::MouseDown | EventKind::MouseUp | EventKind::Click
        )
    }));
}

#[test]
fn moving_away_and_back_does_not_right_click_after_drag_threshold() {
    let (mut app, window) = setup();
    let (entity, target) = target(&mut app, 0);
    press_right(&mut app);
    app.world_mut().entity_mut(entity).insert(Hovered(false));
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(Vec2::splat(50.0)));
    app.update();
    app.world_mut().entity_mut(entity).insert(Hovered(true));
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(Vec2::splat(10.0)));
    app.update();
    release_right(&mut app);

    let events: Vec<_> = app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .drain_events()
        .collect();
    assert!(
        events
            .iter()
            .any(|event| { event.kind == EventKind::RightMouseUp && event.target == target })
    );
    assert!(
        !events
            .iter()
            .any(|event| event.kind == EventKind::RightClick)
    );
    assert!(
        events
            .iter()
            .any(|event| event.kind == EventKind::MouseDrag)
    );
    assert!(
        events
            .iter()
            .any(|event| event.kind == EventKind::MouseDragEnd)
    );
}

#[test]
fn button_background_swapping_does_not_cancel_click() {
    let (mut app, _) = setup();
    let (old_entity, old) = target(&mut app, 2);
    let button = app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .create(ObjectType::Button);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .attach_to_stage(button);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_parent(old, Some(button));
    press(&mut app);
    assert_eq!(
        app.world().resource::<GuiInputState>().captured(),
        Some(button)
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_visible(old, false);
    app.world_mut()
        .entity_mut(old_entity)
        .insert(Hovered(false));
    let (_, new) = target(&mut app, 3);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_parent(new, Some(button));
    release(&mut app);
    let events = clicks(&mut app);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].target, button);
}

#[test]
fn plugin_publishes_clicks_before_application_response_in_same_frame() {
    #[derive(Resource, Default)]
    struct Received(usize);
    fn respond(event: On<GuiEvent>, mut received: ResMut<Received>) {
        if event.event().event.kind == EventKind::Click {
            received.0 += 1;
        }
    }
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<MouseWheel>()
    .init_resource::<Received>();
    let mut window = Window {
        focused: true,
        ..default()
    };
    window.set_cursor_position(Some(Vec2::splat(10.0)));
    app.world_mut().spawn((window, PrimaryWindow));
    let (entity, _) = target(&mut app, 1);
    app.world_mut().entity_mut(entity).observe(respond);
    press(&mut app);
    release(&mut app);
    assert_eq!(app.world().resource::<Received>().0, 1);
}

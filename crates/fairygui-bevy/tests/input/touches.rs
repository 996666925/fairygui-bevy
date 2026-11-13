use super::*;
use bevy::{
    input::touch::{TouchInput, TouchPhase},
    picking::{backend::HitData, hover::HoverMap, pointer::PointerId},
};
use fairygui_core::{Point, widget::ControlData};

pub(super) fn fixture() -> (App, Entity) {
    let (mut app, window) = setup();
    app.init_resource::<HoverMap>();
    // Touch routing must work even if the window has no mouse cursor.
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(None);
    (app, window)
}

pub(super) fn hits(app: &mut App, id: u64, entities: &[Entity]) {
    let hits = entities
        .iter()
        .enumerate()
        .map(|(index, &entity)| {
            (
                entity,
                HitData::new(Entity::PLACEHOLDER, index as f32, Some(Vec3::ZERO), None),
            )
        })
        .collect();
    app.world_mut()
        .resource_mut::<HoverMap>()
        .insert(PointerId::Touch(id), hits);
}

pub(super) fn send(app: &mut App, window: Entity, id: u64, phase: TouchPhase, position: Vec2) {
    app.world_mut().write_message(TouchInput {
        id,
        phase,
        position,
        window,
        force: None,
    });
    app.update();
}

fn events(app: &mut App) -> Vec<fairygui_core::object::UiEvent> {
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .drain_events()
        .collect()
}

#[test]
fn two_fingers_capture_and_release_independently_with_bubbling_ids() {
    let (mut app, window) = fixture();
    let (_, parent) = target(&mut app, 0);
    let (first_entity, first) = target(&mut app, 1);
    let (second_entity, second) = target(&mut app, 2);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_parent(first, Some(parent));
    hits(&mut app, 101, &[first_entity]);
    hits(&mut app, 202, &[second_entity]);
    send(
        &mut app,
        window,
        101,
        TouchPhase::Started,
        Vec2::splat(10.0),
    );
    send(
        &mut app,
        window,
        202,
        TouchPhase::Started,
        Vec2::splat(80.0),
    );
    assert_eq!(captured_touch_object(&app, 101), Some(first));
    assert_eq!(captured_touch_object(&app, 202), Some(second));
    send(&mut app, window, 101, TouchPhase::Ended, Vec2::splat(10.0));
    assert_eq!(captured_touch_object(&app, 101), None);
    assert_eq!(captured_touch_object(&app, 202), Some(second));
    assert!(
        app.world()
            .resource::<GuiInputState>()
            .wants_pointer_input()
    );
    send(&mut app, window, 202, TouchPhase::Ended, Vec2::splat(80.0));
    assert!(
        !app.world()
            .resource::<GuiInputState>()
            .wants_pointer_input()
    );
    let events = events(&mut app);
    let click: Vec<_> = events
        .iter()
        .filter(|e| e.kind == EventKind::Click)
        .collect();
    assert_eq!(click.len(), 3);
    assert_eq!(
        (click[0].target, click[0].current_target, click[0].touch_id),
        (first, first, Some(101))
    );
    assert_eq!(
        (click[1].target, click[1].current_target, click[1].touch_id),
        (first, parent, Some(101))
    );
    assert!(click[1].bubbling);
    assert_eq!((click[2].target, click[2].touch_id), (second, Some(202)));
    for id in [101, 202] {
        let at_target: Vec<_> = events
            .iter()
            .filter(|e| e.touch_id == Some(id) && e.is_at_target())
            .map(|e| e.kind.clone())
            .collect();
        assert_eq!(
            at_target,
            [
                EventKind::RollOver,
                EventKind::TouchBegin,
                EventKind::MouseDown,
                EventKind::TouchEnd,
                EventKind::MouseUp,
                EventKind::Click,
                EventKind::RollOut
            ]
        );
    }
}

#[test]
fn touch_movement_uses_its_own_position_and_cancels_click_after_returning() {
    let (mut app, window) = fixture();
    let (entity, id) = target(&mut app, 1);
    // Mouse RelativeCursorPosition must never supply touch coordinates.
    app.world_mut()
        .entity_mut(entity)
        .insert(RelativeCursorPosition {
            cursor_over: false,
            normalized: None,
        });
    hits(&mut app, 7, &[entity]);
    send(&mut app, window, 7, TouchPhase::Started, Vec2::splat(10.0));
    send(&mut app, window, 7, TouchPhase::Moved, Vec2::splat(80.0));
    send(&mut app, window, 7, TouchPhase::Ended, Vec2::splat(10.0));
    let events = events(&mut app);
    for kind in [
        EventKind::TouchMove,
        EventKind::MouseMove,
        EventKind::MouseDrag,
    ] {
        let event = events.iter().find(|e| e.kind == kind).unwrap();
        assert_eq!(event.target, id);
        assert_eq!(event.touch_id, Some(7));
        assert_eq!(event.pointer, Some(Point::new(80.0, 80.0)));
    }
    assert_eq!(
        events
            .iter()
            .filter(|e| e.kind == EventKind::MouseDragEnd)
            .count(),
        1
    );
    assert!(!events.iter().any(|e| e.kind == EventKind::Click));
}

#[test]
fn cancellation_and_policy_changes_release_stationary_touches_without_click() {
    for policy in 0..6 {
        let (mut app, window) = fixture();
        let (entity, id) = target(&mut app, 1);
        hits(&mut app, 42, &[entity]);
        send(&mut app, window, 42, TouchPhase::Started, Vec2::splat(10.0));
        events(&mut app);
        match policy {
            0 => send(
                &mut app,
                window,
                42,
                TouchPhase::Canceled,
                Vec2::splat(10.0),
            ),
            1 => {
                app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
                app.update();
            }
            2 => {
                app.world_mut()
                    .non_send_mut::<GuiRuntime>()
                    .store_mut()
                    .set_visible(id, false);
                app.update();
            }
            3 => {
                app.world_mut()
                    .non_send_mut::<GuiRuntime>()
                    .store_mut()
                    .detach_from_stage(id);
                app.update();
            }
            4 => {
                app.world_mut()
                    .non_send_mut::<GuiRuntime>()
                    .store_mut()
                    .remove(id);
                app.update();
            }
            _ => {
                app.world_mut()
                    .non_send_mut::<GuiRuntime>()
                    .store_mut()
                    .set_touchable(id, false);
                app.update();
            }
        }
        assert_eq!(captured_touch_object(&app, 42), None);
        assert!(
            !app.world()
                .resource::<GuiInputState>()
                .wants_pointer_input()
        );
        let events = events(&mut app);
        assert!(!events.iter().any(|e| e.kind == EventKind::Click));
        assert_eq!(
            events
                .iter()
                .filter(|e| e.kind == EventKind::TouchEnd)
                .count(),
            usize::from(policy != 4)
        );
    }
}

#[test]
fn mouse_cannot_capture_an_entity_hovered_only_by_touch() {
    let (mut app, window) = fixture();
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(Vec2::splat(10.0)));
    let (entity, id) = target(&mut app, 1);
    hits(&mut app, 5, &[entity]);
    press(&mut app);
    assert_eq!(captured_object(&app), None);
    release(&mut app);
    send(&mut app, window, 5, TouchPhase::Started, Vec2::splat(10.0));
    assert_eq!(captured_touch_object(&app, 5), Some(id));
    assert!(!events(&mut app).iter().any(|e| e.kind == EventKind::Click));
}

#[test]
fn picked_text_children_route_to_their_gui_owner_for_mouse_and_touch() {
    let (mut app, window) = fixture();
    let (entity, owner) = target(&mut app, 1);
    let span = app.world_mut().spawn(ChildOf(entity)).id();
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(Vec2::splat(10.0)));
    hits(&mut app, 1, &[span]);
    app.world_mut().resource_mut::<HoverMap>().insert(
        PointerId::Mouse,
        [(
            span,
            HitData::new(Entity::PLACEHOLDER, 0.0, Some(Vec3::ZERO), None),
        )]
        .into_iter()
        .collect(),
    );
    press(&mut app);
    release(&mut app);
    send(&mut app, window, 1, TouchPhase::Started, Vec2::splat(10.0));
    send(&mut app, window, 1, TouchPhase::Ended, Vec2::splat(10.0));
    let clicks: Vec<_> = events(&mut app)
        .into_iter()
        .filter(|e| e.kind == EventKind::Click)
        .collect();
    assert_eq!(clicks.len(), 2);
    assert_eq!((clicks[0].target, clicks[0].touch_id), (owner, None));
    assert_eq!((clicks[1].target, clicks[1].touch_id), (owner, Some(1)));
}

#[test]
fn releasing_one_finger_keeps_a_shared_button_pressed_until_the_last_release() {
    let (mut app, window) = fixture();
    let (entity, _) = target(&mut app, 1);
    let button = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let button = gui.store_mut().create(ObjectType::Button);
        gui.store_mut().attach_to_stage(button);
        button
    };
    app.world_mut()
        .entity_mut(entity)
        .insert(GObjectRef(button));
    for id in [1, 2] {
        hits(&mut app, id, &[entity]);
        send(&mut app, window, id, TouchPhase::Started, Vec2::splat(10.0));
    }
    for (id, pressed) in [(1, true), (2, false)] {
        send(&mut app, window, id, TouchPhase::Ended, Vec2::splat(10.0));
        let ControlData::Button(button) = app
            .world()
            .non_send::<GuiRuntime>()
            .store()
            .object(button)
            .unwrap()
            .control()
        else {
            panic!("button");
        };
        assert_eq!(button.pressed, pressed);
    }
}

#[test]
fn mouse_release_does_not_clear_a_button_still_held_by_touch() {
    let (mut app, window) = fixture();
    let (entity, _) = target(&mut app, 1);
    let button = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let button = gui.store_mut().create(ObjectType::Button);
        gui.store_mut().attach_to_stage(button);
        button
    };
    app.world_mut()
        .entity_mut(entity)
        .insert(GObjectRef(button));
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(Vec2::splat(10.0)));
    app.init_resource::<HoverMap>();
    app.world_mut().resource_mut::<HoverMap>().insert(
        PointerId::Mouse,
        [(
            entity,
            HitData::new(Entity::PLACEHOLDER, 0.0, Some(Vec3::ZERO), None),
        )]
        .into_iter()
        .collect(),
    );
    hits(&mut app, 1, &[entity]);
    press(&mut app);
    send(&mut app, window, 1, TouchPhase::Started, Vec2::splat(10.0));
    release(&mut app);
    let ControlData::Button(state) = app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .object(button)
        .unwrap()
        .control()
    else {
        panic!("button");
    };
    assert!(state.pressed);
    send(&mut app, window, 1, TouchPhase::Ended, Vec2::splat(10.0));
    let ControlData::Button(state) = app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .object(button)
        .unwrap()
        .control()
    else {
        panic!("button");
    };
    assert!(!state.pressed);
}

#[test]
fn touch_capture_drags_an_exported_scroll_pane_and_uses_touch_coordinates() {
    let (mut app, window) = fixture();
    let pane = super::scroll_container(&mut app);
    let (entity, _) = target(&mut app, 1);
    app.world_mut().entity_mut(entity).insert(GObjectRef(pane));
    hits(&mut app, 15, &[entity]);
    send(
        &mut app,
        window,
        15,
        TouchPhase::Started,
        Vec2::new(20.0, 20.0),
    );
    send(
        &mut app,
        window,
        15,
        TouchPhase::Moved,
        Vec2::new(20.0, -20.0),
    );
    let offset = app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .scroll_pane(pane)
        .unwrap()
        .display_pos()
        .y;
    assert!((offset - 40.0).abs() < 0.01);
    send(
        &mut app,
        window,
        15,
        TouchPhase::Ended,
        Vec2::new(20.0, -20.0),
    );
    let events = events(&mut app);
    assert!(
        events
            .iter()
            .any(|e| e.kind == EventKind::TouchMove && e.touch_id == Some(15))
    );
    assert!(!events.iter().any(|e| e.kind == EventKind::Click));
}

#[test]
fn touch_click_selects_a_list_row_and_keeps_its_finger_id() {
    let (mut app, list, rows, entities) = super::list_selection::fixture();
    app.init_resource::<HoverMap>();
    hits(&mut app, 77, &[entities[2]]);
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .unwrap();
    send(&mut app, window, 77, TouchPhase::Started, Vec2::splat(10.0));
    send(&mut app, window, 77, TouchPhase::Ended, Vec2::splat(10.0));
    let gui = app.world().non_send::<GuiRuntime>();
    assert_eq!(gui.store().list_selections(list), Some(vec![2]));
    assert_eq!(gui.store().button_selected(rows[2]), Some(true));
    let events = events(&mut app);
    let item = events
        .iter()
        .find(|e| e.kind == EventKind::ClickItem)
        .unwrap();
    assert_eq!(item.touch_id, Some(77));
}

#[test]
fn a_second_finger_cannot_drive_or_finish_the_first_fingers_scroll_gesture() {
    let (mut app, window) = fixture();
    let pane = super::scroll_container(&mut app);
    let (entity, _) = target(&mut app, 1);
    app.world_mut().entity_mut(entity).insert(GObjectRef(pane));
    for id in [1, 2] {
        hits(&mut app, id, &[entity]);
        send(&mut app, window, id, TouchPhase::Started, Vec2::splat(20.0));
    }
    let offset = |app: &App| {
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_pane(pane)
            .unwrap()
            .display_pos()
            .y
    };
    send(
        &mut app,
        window,
        2,
        TouchPhase::Moved,
        Vec2::new(20.0, -20.0),
    );
    assert_eq!(offset(&app), 0.0);
    send(
        &mut app,
        window,
        1,
        TouchPhase::Moved,
        Vec2::new(20.0, -20.0),
    );
    assert!((offset(&app) - 40.0).abs() < 0.01);
    send(
        &mut app,
        window,
        2,
        TouchPhase::Ended,
        Vec2::new(20.0, -20.0),
    );
    send(
        &mut app,
        window,
        1,
        TouchPhase::Moved,
        Vec2::new(20.0, -60.0),
    );
    assert!((offset(&app) - 80.0).abs() < 0.01);
    send(
        &mut app,
        window,
        1,
        TouchPhase::Canceled,
        Vec2::new(20.0, -60.0),
    );
}

#[test]
fn touch_scroll_drag_uses_rotated_local_axes_at_noninteger_dpi() {
    use bevy::math::{Affine2, Mat2};
    let (mut app, window) = fixture();
    let pane = super::scroll_container(&mut app);
    let (entity, _) = target(&mut app, 1);
    let dpi = 1.75;
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .resolution
        .set_scale_factor_override(Some(dpi));
    let local_to_screen =
        Mat2::from_angle(0.73) * Mat2::from_cols(Vec2::new(1.2, 0.35), Vec2::new(-0.2, 0.85));
    app.world_mut().entity_mut(entity).insert((
        GObjectRef(pane),
        ComputedNode {
            size: Vec2::new(200.0, 100.0) * dpi,
            inverse_scale_factor: 1.0 / dpi,
            ..default()
        },
        UiGlobalTransform::from(Affine2 {
            matrix2: local_to_screen,
            translation: Vec2::new(180.0, 140.0) * dpi,
        }),
    ));
    let start = Vec2::new(150.0, 120.0);
    hits(&mut app, 3, &[entity]);
    send(&mut app, window, 3, TouchPhase::Started, start);
    send(
        &mut app,
        window,
        3,
        TouchPhase::Moved,
        start + local_to_screen * Vec2::new(0.0, -40.0),
    );
    let at = app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .scroll_pane(pane)
        .unwrap()
        .display_pos();
    assert!((at.y - 40.0).abs() < 0.01);
    assert!(at.x.abs() < 0.01);
}

#[test]
fn touch_drag_and_drop_is_owned_by_the_requested_finger() {
    let (mut app, window) = fixture();
    let (entity, source) = target(&mut app, 1);
    let (other_entity, other_target) = target(&mut app, 2);
    hits(&mut app, 1, &[entity]);
    hits(&mut app, 2, &[other_entity]);
    send(&mut app, window, 1, TouchPhase::Started, Vec2::splat(20.0));
    send(&mut app, window, 2, TouchPhase::Started, Vec2::splat(20.0));
    let icon = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let icon = gui.store_mut().create(ObjectType::Image);
        gui.start_touch_drag(
            source,
            Some(icon),
            Some("touch payload".into()),
            Point::new(20.0, 20.0),
            1,
        );
        icon
    };
    send(&mut app, window, 2, TouchPhase::Ended, Vec2::splat(20.0));
    assert!(app.world().non_send::<GuiRuntime>().is_dragging());
    hits(&mut app, 1, &[other_entity]);
    send(
        &mut app,
        window,
        1,
        TouchPhase::Moved,
        Vec2::new(40.0, 70.0),
    );
    let gui = app.world().non_send::<GuiRuntime>();
    assert_eq!(
        gui.store().object(icon).unwrap().position(),
        Point::new(40.0, 70.0)
    );
    assert_eq!(gui.drag_data(), Some("touch payload"));
    send(
        &mut app,
        window,
        1,
        TouchPhase::Ended,
        Vec2::new(40.0, 70.0),
    );
    assert!(!app.world().non_send::<GuiRuntime>().is_dragging());
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .object(icon)
            .is_none()
    );
    let events = events(&mut app);
    assert!(
        !events
            .iter()
            .any(|e| e.kind == EventKind::Click && e.touch_id == Some(1))
    );
    let drop = events.iter().find(|e| e.kind == EventKind::Drop).unwrap();
    assert_eq!(drop.target, other_target);
    assert_eq!(drop.touch_id, Some(1));
    assert_eq!(
        drop.data,
        fairygui_core::object::EventData::Drop {
            source,
            data: Some("touch payload".into()),
        }
    );
}

#[test]
fn cancelling_a_touch_owned_drop_reclaims_the_icon_without_drop_event() {
    let (mut app, window) = fixture();
    let (entity, source) = target(&mut app, 1);
    let (drop_entity, _) = target(&mut app, 2);
    hits(&mut app, 5, &[entity]);
    send(&mut app, window, 5, TouchPhase::Started, Vec2::splat(20.0));
    let icon = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let icon = gui.store_mut().create(ObjectType::Image);
        gui.start_touch_drag(source, Some(icon), Some("cancelled".into()), Point::ZERO, 5);
        icon
    };
    hits(&mut app, 5, &[drop_entity]);
    send(&mut app, window, 5, TouchPhase::Canceled, Vec2::splat(80.0));
    let gui = app.world().non_send::<GuiRuntime>();
    assert!(!gui.is_dragging());
    assert!(gui.store().object(icon).is_none());
    assert!(
        !events(&mut app)
            .iter()
            .any(|event| event.kind == EventKind::Drop)
    );
}

#[test]
fn a_stationary_touch_emits_no_moves_and_can_tap_in_a_single_frame() {
    let (mut app, window) = fixture();
    let (entity, _) = target(&mut app, 1);
    hits(&mut app, 1, &[entity]);
    send(&mut app, window, 1, TouchPhase::Started, Vec2::splat(20.0));
    events(&mut app);
    app.update();
    assert!(events(&mut app).is_empty());
    send(&mut app, window, 1, TouchPhase::Ended, Vec2::splat(20.0));
    assert_eq!(
        events(&mut app)
            .iter()
            .filter(|e| e.kind == EventKind::Click)
            .count(),
        1
    );
    for phase in [TouchPhase::Started, TouchPhase::Ended] {
        app.world_mut().write_message(TouchInput {
            id: 1,
            phase,
            window,
            position: Vec2::splat(20.0),
            force: None,
        });
    }
    app.update();
    assert_eq!(
        events(&mut app)
            .iter()
            .filter(|e| e.kind == EventKind::Click)
            .count(),
        1
    );
    assert_eq!(captured_touch_object(&app, 1), None);
}

#[test]
fn transparent_touch_pixels_are_rejected_and_a_foreign_window_is_ignored() {
    let (mut app, window) = fixture();
    let (back_entity, back) = target(&mut app, 1);
    let (front_entity, front) = target(&mut app, 2);
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
    hits(&mut app, 3, &[front_entity, back_entity]);
    let other_window = app.world_mut().spawn(Window::default()).id();
    send(
        &mut app,
        other_window,
        3,
        TouchPhase::Started,
        Vec2::splat(10.0),
    );
    assert_eq!(captured_touch_object(&app, 3), None);
    send(&mut app, window, 3, TouchPhase::Started, Vec2::splat(10.0));
    assert_eq!(captured_touch_object(&app, 3), Some(back));
    // Releasing over another eligible GUI target does not transfer a click.
    hits(&mut app, 3, &[]);
    send(&mut app, window, 3, TouchPhase::Ended, Vec2::splat(10.0));
    assert!(!events(&mut app).iter().any(|e| e.kind == EventKind::Click));
}

#[test]
fn touch_events_from_picking_are_published_to_entity_observers_in_the_same_frame() {
    use bevy::picking::{InteractionPlugin, PickingPlugin, backend::PointerHits};
    #[derive(Resource, Default)]
    struct Received(Vec<(EventKind, Option<u64>)>);
    let (mut app, window) = setup();
    app.add_plugins((PickingPlugin, InteractionPlugin))
        .init_resource::<Received>();
    app.add_systems(
        Update,
        fairygui_bevy::runtime::emit_gui_events.after(GuiSystems::Input),
    );
    let (entity, _) = target(&mut app, 1);
    app.world_mut()
        .entity_mut(entity)
        .insert(bevy::picking::Pickable {
            should_block_lower: false,
            is_hoverable: true,
        });
    app.world_mut().entity_mut(entity).observe(
        |event: On<GuiEvent>, mut received: ResMut<Received>| {
            received
                .0
                .push((event.event.kind.clone(), event.event.touch_id));
        },
    );
    app.world_mut().spawn(PointerId::Touch(99));
    for phase in [TouchPhase::Started, TouchPhase::Ended] {
        app.world_mut().write_message(PointerHits::new(
            PointerId::Touch(99),
            vec![(
                entity,
                HitData::new(Entity::PLACEHOLDER, 0.0, Some(Vec3::ZERO), None),
            )],
            0.5,
        ));
        send(&mut app, window, 99, phase, Vec2::splat(10.0));
    }
    assert!(
        app.world()
            .resource::<Received>()
            .0
            .contains(&(EventKind::Click, Some(99)))
    );
}

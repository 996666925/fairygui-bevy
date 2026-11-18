use super::*;
use bevy::input::touch::TouchPhase;

#[test]
fn mouse_and_touch_reject_stale_hover_hits_outside_the_shared_paint_clip() {
    use bevy::ui::CalculatedClip;
    for touch in [false, true] {
        let (mut app, window) = if touch { touches::fixture() } else { setup() };
        let (below, lower) = target(&mut app, 1);
        let (above, upper) = target(&mut app, 2);
        app.world_mut()
            .entity_mut(above)
            .insert(CalculatedClip::default().with_rect(
                Rect::from_corners(Vec2::splat(15.), Vec2::splat(25.)),
                &UiGlobalTransform::default(),
            ));
        if touch {
            touches::hits(&mut app, 1, &[above, below]);
            touches::send(&mut app, window, 1, TouchPhase::Started, Vec2::splat(10.));
            assert_eq!(captured_touch_object(&app, 1), Some(lower));
            touches::send(&mut app, window, 1, TouchPhase::Ended, Vec2::splat(10.));
            touches::hits(&mut app, 2, &[above, below]);
            touches::send(&mut app, window, 2, TouchPhase::Started, Vec2::splat(20.));
            assert_eq!(captured_touch_object(&app, 2), Some(upper));
            touches::send(&mut app, window, 2, TouchPhase::Ended, Vec2::splat(20.));
        } else {
            press(&mut app);
            assert_eq!(captured_object(&app), Some(lower));
            release(&mut app);
            app.world_mut()
                .get_mut::<Window>(window)
                .unwrap()
                .set_cursor_position(Some(Vec2::splat(20.)));
            press(&mut app);
            assert_eq!(captured_object(&app), Some(upper));
            release(&mut app);
        }
        app.world_mut()
            .entity_mut(above)
            .insert(CalculatedClip::FullyClipped);
        if touch {
            touches::hits(&mut app, 3, &[above, below]);
            touches::send(&mut app, window, 3, TouchPhase::Started, Vec2::splat(20.));
            assert_eq!(captured_touch_object(&app, 3), Some(lower));
        } else {
            press(&mut app);
            assert_eq!(captured_object(&app), Some(lower));
        }
    }
}

#[test]
fn transparent_component_passes_blank_clicks_to_lower_target_but_keeps_child_events_and_capture() {
    let (mut app, _) = setup();
    let (_, lower) = target(&mut app, 1);
    let (_, upper) = target(&mut app, 2);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_component_opaque(upper, false);
    press(&mut app);
    release(&mut app);
    assert_eq!(
        clicks(&mut app)
            .iter()
            .map(|e| e.target)
            .collect::<Vec<_>>(),
        vec![lower]
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_component_opaque(upper, true);
    press(&mut app);
    assert_eq!(captured_object(&app), Some(upper));
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_component_opaque(upper, false);
    release(&mut app);
    assert_eq!(clicks(&mut app)[0].target, upper);
    assert_eq!(captured_object(&app), None);
    let (_, child) = target(&mut app, 3);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_parent(child, Some(upper));
    press(&mut app);
    release(&mut app);
    let events: Vec<_> = app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .drain_events()
        .filter(|e| e.kind == EventKind::Click)
        .collect();
    assert!(events.iter().any(|e| e.target == child));
    assert!(
        events
            .iter()
            .any(|e| e.target == child && e.current_target == upper && e.bubbling)
    );
}

#[test]
fn touch_picking_uses_opaque_without_disabling_transparent_parents() {
    let (mut app, window) = touches::fixture();
    let (below, lower) = target(&mut app, 1);
    let (above, upper) = target(&mut app, 2);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_component_opaque(upper, false);
    touches::hits(&mut app, 99, &[above, below]);
    touches::send(&mut app, window, 99, TouchPhase::Started, Vec2::splat(10.));
    assert_eq!(captured_touch_object(&app, 99), Some(lower));
    touches::send(&mut app, window, 99, TouchPhase::Ended, Vec2::splat(10.));
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_component_opaque(upper, true);
    touches::hits(&mut app, 100, &[above, below]);
    touches::send(&mut app, window, 100, TouchPhase::Started, Vec2::splat(10.));
    assert_eq!(captured_touch_object(&app, 100), Some(upper));
}

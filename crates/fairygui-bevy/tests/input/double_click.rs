use super::*;
use bevy::input::touch::TouchPhase;
use std::time::Duration;

fn advance(app: &mut App, millis: u64) {
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(millis));
}
fn observed(app: &mut App) -> Vec<(EventKind, u8, Option<u64>)> {
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .drain_events()
        .filter(|event| {
            event.is_at_target()
                && matches!(
                    event.kind,
                    EventKind::Click
                        | EventKind::DoubleClick
                        | EventKind::RightClick
                        | EventKind::ClickItem
                )
        })
        .map(|event| (event.kind, event.click_count, event.touch_id))
        .collect()
}
fn click(app: &mut App) {
    press(app);
    release(app);
}

#[test]
fn double_click_releases_preserve_native_time_axis_boundaries_targets_and_repeated_counts() {
    let (mut app, window) = setup();
    let (first, _) = target(&mut app, 1);
    advance(&mut app, 1000);
    click(&mut app);
    assert_eq!(observed(&mut app), [(EventKind::Click, 1, None)]);
    advance(&mut app, 349);
    // A nearby different object is still the second native release.
    app.world_mut().entity_mut(first).insert(Hovered(false));
    target(&mut app, 2);
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(Vec2::new(19., 19.)));
    click(&mut app);
    assert_eq!(
        observed(&mut app),
        [
            (EventKind::Click, 2, None),
            (EventKind::DoubleClick, 2, None)
        ]
    );
    advance(&mut app, 1);
    click(&mut app);
    assert_eq!(
        observed(&mut app),
        [
            (EventKind::Click, 2, None),
            (EventKind::DoubleClick, 2, None)
        ]
    );
    advance(&mut app, 350);
    click(&mut app);
    assert_eq!(observed(&mut app), [(EventKind::Click, 1, None)]);
    advance(&mut app, 1);
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(Vec2::new(29., 19.)));
    click(&mut app);
    assert_eq!(
        observed(&mut app),
        [(EventKind::Click, 1, None)],
        "exact ten-pixel repeat is not double"
    );
}

#[test]
fn right_clicks_share_mouse_button_history_but_never_dispatch_double_click() {
    let (mut app, _) = setup();
    target(&mut app, 1);
    advance(&mut app, 1000);
    click(&mut app);
    observed(&mut app);
    advance(&mut app, 50);
    press_right(&mut app);
    release_right(&mut app);
    assert_eq!(observed(&mut app), [(EventKind::RightClick, 1, None)]);
    advance(&mut app, 50);
    press_right(&mut app);
    release_right(&mut app);
    assert_eq!(observed(&mut app), [(EventKind::RightClick, 2, None)]);
    advance(&mut app, 50);
    click(&mut app);
    assert_eq!(observed(&mut app), [(EventKind::Click, 1, None)]);
}

#[test]
fn dragged_or_invalidated_releases_reset_double_click_history() {
    for hidden in [false, true] {
        let (mut app, window) = setup();
        let (view, id) = target(&mut app, 1);
        advance(&mut app, 1000);
        click(&mut app);
        observed(&mut app);
        advance(&mut app, 50);
        press(&mut app);
        if hidden {
            app.world_mut()
                .non_send_mut::<GuiRuntime>()
                .store_mut()
                .set_visible(id, false);
        } else {
            app.world_mut()
                .get_mut::<Window>(window)
                .unwrap()
                .set_cursor_position(Some(Vec2::new(30., 10.)));
        }
        app.update();
        release(&mut app);
        assert!(observed(&mut app).is_empty());
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_visible(id, true);
        app.world_mut().entity_mut(view).insert(Hovered(true));
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .set_cursor_position(Some(Vec2::splat(10.)));
        advance(&mut app, 50);
        click(&mut app);
        assert_eq!(observed(&mut app), [(EventKind::Click, 1, None)]);
    }
}

#[test]
fn touch_double_clicks_keep_ids_independent_of_mouse_and_cancelled_touch_dispatch() {
    let (mut app, window) = touches::fixture();
    let (view, _) = target(&mut app, 1);
    advance(&mut app, 1000);
    for id in [11, 22] {
        touches::hits(&mut app, id, &[view]);
        touches::send(&mut app, window, id, TouchPhase::Started, Vec2::splat(10.));
        touches::send(&mut app, window, id, TouchPhase::Ended, Vec2::splat(10.));
        assert_eq!(observed(&mut app), [(EventKind::Click, 1, Some(id))]);
    }
    advance(&mut app, 100);
    touches::send(&mut app, window, 11, TouchPhase::Started, Vec2::splat(10.));
    touches::send(&mut app, window, 11, TouchPhase::Ended, Vec2::splat(10.));
    assert_eq!(
        observed(&mut app),
        [
            (EventKind::Click, 2, Some(11)),
            (EventKind::DoubleClick, 2, Some(11))
        ]
    );
    touches::hits(&mut app, 33, &[view]);
    touches::send(&mut app, window, 33, TouchPhase::Started, Vec2::splat(10.));
    touches::send(&mut app, window, 33, TouchPhase::Canceled, Vec2::splat(10.));
    assert!(observed(&mut app).is_empty());
    // Native cancellation still records a release before skipping Click.
    advance(&mut app, 50);
    touches::send(&mut app, window, 33, TouchPhase::Started, Vec2::splat(10.));
    touches::send(&mut app, window, 33, TouchPhase::Ended, Vec2::splat(10.));
    assert_eq!(
        observed(&mut app),
        [
            (EventKind::Click, 2, Some(33)),
            (EventKind::DoubleClick, 2, Some(33))
        ]
    );
    advance(&mut app, 350);
    touches::send(&mut app, window, 11, TouchPhase::Started, Vec2::splat(10.));
    touches::send(&mut app, window, 11, TouchPhase::Ended, Vec2::splat(10.));
    assert_eq!(observed(&mut app), [(EventKind::Click, 1, Some(11))]);
}

#[test]
fn click_item_keeps_second_release_metadata_without_repeating_selection() {
    let (mut app, list, rows, views) = list_selection::fixture();
    app.world_mut().entity_mut(views[0]).insert(Hovered(true));
    advance(&mut app, 1000);
    click(&mut app);
    observed(&mut app);
    advance(&mut app, 100);
    click(&mut app);
    assert_eq!(
        observed(&mut app),
        [
            (EventKind::Click, 2, None),
            (EventKind::ClickItem, 2, None),
            (EventKind::DoubleClick, 2, None)
        ]
    );
    let gui = app.world().non_send::<GuiRuntime>();
    assert_eq!(gui.store().list_selection(list), Some(0));
    assert_eq!(gui.store().list_item_at(list, 0), Some(rows[0]));
}

#[test]
fn physical_click_uses_real_time_even_when_virtual_time_is_paused() {
    let (mut app, _) = setup();
    target(&mut app, 1);
    let mut real = Time::<Real>::default();
    real.advance_by(Duration::from_secs(1));
    app.insert_resource(real);
    click(&mut app);
    observed(&mut app);
    app.world_mut()
        .resource_mut::<Time<Real>>()
        .advance_by(Duration::from_millis(350));
    click(&mut app);
    assert_eq!(observed(&mut app), [(EventKind::Click, 1, None)]);
}

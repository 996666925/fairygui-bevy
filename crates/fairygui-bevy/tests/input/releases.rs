use super::*;
use bevy::input::touch::TouchPhase;

struct Fixture {
    app: App,
    window: Entity,
    views: [Entity; 5],
    ids: [GObjectId; 5],
}
fn fixture() -> Fixture {
    let (mut app, window) = setup();
    let pairs = std::array::from_fn::<_, 5, _>(|i| target(&mut app, i as u32));
    let views = pairs.map(|pair| pair.0);
    let ids = pairs.map(|pair| pair.1);
    for (index, &view) in views.iter().enumerate() {
        app.world_mut().entity_mut(view).insert(Hovered(index == 3));
    }
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        for (child, parent) in [(1, 0), (2, 0), (3, 1), (4, 2)] {
            gui.store_mut().set_parent(ids[child], Some(ids[parent]));
        }
    }
    Fixture {
        app,
        window,
        views,
        ids,
    }
}

#[test]
fn all_contacts_release_to_the_hit_path_then_the_saved_press_path_once() {
    for contact in 0..3 {
        for mutation in 0..4 {
            let mut f = fixture();
            match contact {
                0 => press(&mut f.app),
                1 => press_right(&mut f.app),
                _ => {
                    f.app.init_resource::<bevy::picking::hover::HoverMap>();
                    touches::hits(&mut f.app, 42, &[f.views[3]]);
                    touches::send(
                        &mut f.app,
                        f.window,
                        42,
                        TouchPhase::Started,
                        Vec2::splat(10.),
                    );
                }
            }
            {
                let mut gui = f.app.world_mut().non_send_mut::<GuiRuntime>();
                match mutation {
                    0 => {}
                    1 => {
                        gui.store_mut().set_parent(f.ids[3], Some(f.ids[2]));
                    }
                    2 => {
                        gui.store_mut().detach_from_parent(f.ids[3]);
                    }
                    _ => {
                        gui.store_mut().remove(f.ids[3]);
                    }
                }
                gui.drain_events().for_each(drop);
            }
            f.app
                .world_mut()
                .entity_mut(f.views[3])
                .insert(Hovered(false));
            f.app
                .world_mut()
                .entity_mut(f.views[4])
                .insert(Hovered(true));
            match contact {
                0 => release(&mut f.app),
                1 => release_right(&mut f.app),
                _ => {
                    touches::hits(&mut f.app, 42, &[f.views[4]]);
                    touches::send(
                        &mut f.app,
                        f.window,
                        42,
                        TouchPhase::Ended,
                        Vec2::splat(10.),
                    );
                }
            }
            let kind = if contact == 1 {
                EventKind::RightMouseUp
            } else {
                EventKind::MouseUp
            };
            let events: Vec<_> = f
                .app
                .world_mut()
                .non_send_mut::<GuiRuntime>()
                .drain_events()
                .filter(|event| event.kind == kind)
                .collect();
            let mut expected = vec![f.ids[4], f.ids[2], f.ids[0]];
            if mutation != 3 {
                expected.push(f.ids[3]);
            }
            expected.push(f.ids[1]);
            assert_eq!(
                events
                    .iter()
                    .map(|event| event.current_target)
                    .collect::<Vec<_>>(),
                expected,
                "contact={contact} mutation={mutation}"
            );
            for event in &events {
                assert_eq!(event.target, f.ids[4]);
                assert_eq!(event.bubbling, event.current_target != event.target);
                assert_eq!(event.touch_id, (contact == 2).then_some(42));
                assert_eq!(event.pointer, Some(fairygui_core::Point::new(10., 10.)));
            }
        }
    }
}

#[derive(Resource, Default)]
struct Seen(Vec<(Entity, Option<Entity>)>);
fn drag_end(mut event: On<GuiMouseDragEnd>, mut seen: ResMut<Seen>) {
    seen.0.push((event.entity, event.target));
    event.propagate(false);
}

#[test]
fn drag_end_keeps_press_time_recipients_and_release_hit_metadata_after_reparenting() {
    let mut f = fixture();
    f.app.init_resource::<Seen>().add_systems(
        Update,
        fairygui_bevy::runtime::emit_gui_events.after(GuiSystems::Input),
    );
    for index in [3, 1, 0] {
        f.app
            .world_mut()
            .entity_mut(f.views[index])
            .observe(drag_end);
    }
    press(&mut f.app);
    f.app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_parent(f.ids[3], Some(f.ids[2]));
    f.app
        .world_mut()
        .entity_mut(f.views[3])
        .insert(Hovered(false));
    f.app
        .world_mut()
        .entity_mut(f.views[4])
        .insert(Hovered(true));
    f.app
        .world_mut()
        .get_mut::<Window>(f.window)
        .unwrap()
        .set_cursor_position(Some(Vec2::new(40., 10.)));
    f.app.update();
    release(&mut f.app);
    assert_eq!(
        f.app.world().resource::<Seen>().0,
        [3, 1, 0].map(|index| (f.views[index], Some(f.views[4])))
    );
    let events: Vec<_> = f
        .app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .drain_events()
        .filter(|event| event.kind == EventKind::MouseDragEnd)
        .collect();
    assert_eq!(
        events
            .iter()
            .map(|event| event.current_target)
            .collect::<Vec<_>>(),
        [f.ids[3], f.ids[1], f.ids[0]]
    );
    assert!(events.iter().all(|event| event.target == f.ids[4]));
}

#[test]
fn release_outside_gui_has_no_gui_target_and_still_notifies_pressed_controls() {
    let mut f = fixture();
    f.app.init_resource::<Seen>().add_systems(
        Update,
        fairygui_bevy::runtime::emit_gui_events.after(GuiSystems::Input),
    );
    for index in [3, 1, 0] {
        f.app.world_mut().entity_mut(f.views[index]).observe(
            |event: On<GuiMouseUp>, mut seen: ResMut<Seen>| {
                seen.0.push((event.entity, event.target));
            },
        );
    }
    press(&mut f.app);
    f.app
        .world_mut()
        .entity_mut(f.views[3])
        .insert(Hovered(false));
    release(&mut f.app);
    assert_eq!(
        f.app.world().resource::<Seen>().0,
        [3, 1, 0].map(|index| (f.views[index], None))
    );
    assert_eq!(
        f.app.world().non_send::<GuiRuntime>().root_container(),
        None,
        "outside release must not create a FairyGUI GRoot"
    );
}

#[test]
fn touchcancel_uses_last_valid_position_and_preserves_native_release_history() {
    let (mut app, window) = touches::fixture();
    let (view, id) = target(&mut app, 1);
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_secs(1));
    touches::hits(&mut app, 42, &[view]);
    touches::send(&mut app, window, 42, TouchPhase::Started, Vec2::splat(10.));
    touches::send(
        &mut app,
        window,
        42,
        TouchPhase::Canceled,
        Vec2::splat(500.),
    );
    let events: Vec<_> = app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .drain_events()
        .collect();
    assert!(
        events
            .iter()
            .any(|event| event.kind == EventKind::MouseUp && event.current_target == id)
    );
    assert!(
        events
            .iter()
            .filter(|event| matches!(event.kind, EventKind::TouchEnd | EventKind::MouseUp))
            .all(|event| event.pointer == Some(fairygui_core::Point::new(10., 10.)))
    );
    assert!(!events.iter().any(|event| event.kind == EventKind::Click));
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_millis(50));
    touches::send(&mut app, window, 42, TouchPhase::Started, Vec2::splat(10.));
    touches::send(&mut app, window, 42, TouchPhase::Ended, Vec2::splat(10.));
    assert_eq!(clicks(&mut app)[0].click_count, 2);
}

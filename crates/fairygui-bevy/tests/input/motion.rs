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
    for (i, &view) in views.iter().enumerate() {
        app.world_mut().entity_mut(view).insert(Hovered(i == 3));
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
fn begin(f: &mut Fixture, contact: u8) {
    match contact {
        0 => press(&mut f.app),
        1 => press_right(&mut f.app),
        _ => {
            f.app.init_resource::<bevy::picking::hover::HoverMap>();
            touches::hits(&mut f.app, 71, &[f.views[3]]);
            touches::send(
                &mut f.app,
                f.window,
                71,
                TouchPhase::Started,
                Vec2::splat(10.),
            );
        }
    }
}
fn move_to(f: &mut Fixture, contact: u8, hit: bool) {
    f.app
        .world_mut()
        .entity_mut(f.views[3])
        .insert(Hovered(false));
    f.app
        .world_mut()
        .entity_mut(f.views[4])
        .insert(Hovered(hit));
    if contact == 2 {
        touches::hits(&mut f.app, 71, if hit { &f.views[4..] } else { &[] });
        touches::send(
            &mut f.app,
            f.window,
            71,
            TouchPhase::Moved,
            Vec2::new(40., 10.),
        );
    } else {
        f.app
            .world_mut()
            .get_mut::<Window>(f.window)
            .unwrap()
            .set_cursor_position(Some(Vec2::new(40., 10.)));
        f.app.update();
    }
}

#[test]
fn mouse_buttons_and_touch_drag_keep_old_recipients_after_tree_changes() {
    for contact in 0..3 {
        for mutation in 0..5 {
            let mut f = fixture();
            begin(&mut f, contact);
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
                    3 => {
                        gui.store_mut().remove(f.ids[3]);
                    }
                    _ => {
                        gui.store_mut().detach_from_parent(f.ids[1]);
                    }
                }
                gui.drain_events().for_each(drop);
            }
            move_to(&mut f, contact, true);
            let events: Vec<_> = f
                .app
                .world_mut()
                .non_send_mut::<GuiRuntime>()
                .drain_events()
                .filter(|event| event.kind == EventKind::MouseDrag)
                .collect();
            let expected: Vec<_> = [3, 1, 0]
                .into_iter()
                .filter(|&i| mutation != 3 || i != 3)
                .map(|i| f.ids[i])
                .collect();
            assert_eq!(
                events
                    .iter()
                    .map(|event| event.current_target)
                    .collect::<Vec<_>>(),
                expected,
                "contact={contact} mutation={mutation}"
            );
            for event in events {
                assert_eq!(event.target, f.ids[4]);
                assert!(event.bubbling);
                assert_eq!(event.touch_id, (contact == 2).then_some(71));
                assert_eq!(event.pointer, Some(fairygui_core::Point::new(40., 10.)));
            }
        }
    }
}

#[derive(Resource, Default)]
struct Seen(Vec<(Entity, Option<Entity>)>);
#[test]
fn dragging_outside_gui_retains_capture_and_notifies_old_recipients_with_no_hit() {
    for contact in 0..3 {
        let mut f = fixture();
        f.app.init_resource::<Seen>().add_systems(
            Update,
            fairygui_bevy::runtime::emit_gui_events.after(GuiSystems::Input),
        );
        for i in [3, 1, 0] {
            f.app.world_mut().entity_mut(f.views[i]).observe(
                |event: On<GuiMouseDrag>, mut seen: ResMut<Seen>| {
                    seen.0.push((event.entity, event.target));
                },
            );
        }
        begin(&mut f, contact);
        move_to(&mut f, contact, false);
        assert_eq!(
            f.app.world().resource::<Seen>().0,
            [3, 1, 0].map(|i| (f.views[i], None))
        );
        assert!(
            f.app
                .world()
                .resource::<GuiInputState>()
                .wants_pointer_input()
        );
        assert_eq!(
            f.app.world().resource::<GuiInputState>().captured(),
            Some(f.views[3])
        );
        assert_eq!(
            f.app.world().non_send::<GuiRuntime>().root_container(),
            None
        );
    }
}

#[test]
fn touch_ending_at_a_new_position_does_not_generate_move_or_start_a_scroll_gesture() {
    for prior_move in [false, true] {
        let (mut app, window) = touches::fixture();
        let pane = scroll_container(&mut app);
        let (view, child) = target(&mut app, 3);
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_parent(child, Some(pane));
        touches::hits(&mut app, 71, &[view]);
        touches::send(
            &mut app,
            window,
            71,
            TouchPhase::Started,
            Vec2::new(100., 100.),
        );
        if prior_move {
            touches::send(
                &mut app,
                window,
                71,
                TouchPhase::Moved,
                Vec2::new(100., 60.),
            );
        }
        let before = app
            .world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_pane(pane)
            .unwrap()
            .display_pos();
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .drain_events()
            .for_each(drop);
        touches::send(
            &mut app,
            window,
            71,
            TouchPhase::Ended,
            Vec2::new(100., 10.),
        );
        assert_eq!(
            app.world()
                .non_send::<GuiRuntime>()
                .store()
                .scroll_pane(pane)
                .unwrap()
                .display_pos(),
            before
        );
        let events: Vec<_> = app
            .world_mut()
            .non_send_mut::<GuiRuntime>()
            .drain_events()
            .collect();
        assert!(
            !events.iter().any(|event| matches!(
                event.kind,
                EventKind::TouchMove
                    | EventKind::MouseMove
                    | EventKind::MouseDrag
                    | EventKind::DragStart
            )),
            "prior_move={prior_move}"
        );
        assert_eq!(
            events
                .iter()
                .any(|event| event.kind == EventKind::MouseDragEnd),
            prior_move
        );
        assert!(events.iter().any(|event| event.kind == EventKind::MouseUp
            && event.pointer == Some(fairygui_core::Point::new(100., 10.))));
    }
}

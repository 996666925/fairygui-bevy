use super::*;
use bevy::input::touch::TouchPhase;

struct Fixture {
    app: App,
    window: Entity,
    views: [Entity; 6],
    ids: [GObjectId; 6],
}
fn fixture(touch: bool) -> Fixture {
    let (mut app, window) = if touch { touches::fixture() } else { setup() };
    let pairs = std::array::from_fn::<_, 6, _>(|i| target(&mut app, i as u32));
    let views = pairs.map(|pair| pair.0);
    let ids = pairs.map(|pair| pair.1);
    for &view in &views {
        app.world_mut().entity_mut(view).insert(Hovered(false));
    }
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        for (child, parent) in [(1, 0), (2, 0), (3, 1), (4, 1), (5, 2)] {
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
fn observed(app: &mut App) -> Vec<(bool, GObjectId, Option<u64>)> {
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .drain_events()
        .filter_map(|event| {
            let entering = match event.kind {
                EventKind::RollOver => true,
                EventKind::RollOut => false,
                _ => return None,
            };
            assert!(event.is_at_target());
            assert!(!event.bubbling);
            Some((entering, event.target, event.touch_id))
        })
        .collect()
}
fn sample(
    f: &mut Fixture,
    contact: Option<u64>,
    phase: TouchPhase,
    index: Option<usize>,
    position: Vec2,
) {
    if let Some(id) = contact {
        touches::hits(
            &mut f.app,
            id,
            &index.map(|i| f.views[i]).into_iter().collect::<Vec<_>>(),
        );
        touches::send(&mut f.app, f.window, id, phase, position);
    } else {
        for (i, &view) in f.views.iter().enumerate() {
            f.app
                .world_mut()
                .entity_mut(view)
                .insert(Hovered(index == Some(i)));
        }
        f.app
            .world_mut()
            .get_mut::<Window>(f.window)
            .unwrap()
            .set_cursor_position(Some(position));
        f.app.update();
    }
}

#[test]
fn mouse_and_touch_only_notify_changed_branches_below_the_common_ancestor() {
    for contact in [None, Some(71)] {
        let mut f = fixture(contact.is_some());
        let cases = [
            (Some(3), vec![(true, 3), (true, 1), (true, 0)]),
            (Some(4), vec![(false, 3), (true, 4)]),
            (Some(1), vec![(false, 4)]),
            (Some(3), vec![(true, 3)]),
            (Some(5), vec![(false, 3), (false, 1), (true, 5), (true, 2)]),
            (None, vec![(false, 5), (false, 2), (false, 0)]),
        ];
        for (step, (hit, expected)) in cases.into_iter().enumerate() {
            sample(
                &mut f,
                contact,
                if step == 0 {
                    TouchPhase::Started
                } else {
                    TouchPhase::Moved
                },
                hit,
                Vec2::new(10. + step as f32, 20.),
            );
            let expected: Vec<_> = expected
                .into_iter()
                .map(|(enter, i)| (enter, f.ids[i], contact))
                .collect();
            assert_eq!(
                observed(&mut f.app),
                expected,
                "contact={contact:?} step={step}"
            );
        }
        sample(
            &mut f,
            contact,
            TouchPhase::Ended,
            None,
            Vec2::new(15., 20.),
        );
        assert!(observed(&mut f.app).is_empty());
    }
}

#[test]
fn hover_targets_and_exit_positions_are_independent_for_simultaneous_contacts() {
    let mut f = fixture(true);
    sample(
        &mut f,
        Some(11),
        TouchPhase::Started,
        Some(3),
        Vec2::splat(10.),
    );
    observed(&mut f.app);
    sample(
        &mut f,
        Some(22),
        TouchPhase::Started,
        Some(4),
        Vec2::splat(20.),
    );
    assert_eq!(
        observed(&mut f.app),
        [4, 1, 0].map(|i| (true, f.ids[i], Some(22)))
    );
    sample(
        &mut f,
        Some(11),
        TouchPhase::Moved,
        Some(4),
        Vec2::splat(40.),
    );
    let events: Vec<_> = f
        .app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .drain_events()
        .filter(|event| matches!(event.kind, EventKind::RollOver | EventKind::RollOut))
        .collect();
    assert_eq!(
        events
            .iter()
            .map(|event| (event.kind.clone(), event.target, event.touch_id))
            .collect::<Vec<_>>(),
        [
            (EventKind::RollOut, f.ids[3], Some(11)),
            (EventKind::RollOver, f.ids[4], Some(11))
        ]
    );
    assert!(
        events
            .iter()
            .all(|event| event.pointer == Some(fairygui_core::Point::new(40., 40.))),
        "exit uses the new sample, not the previous contact position"
    );
    sample(
        &mut f,
        Some(11),
        TouchPhase::Ended,
        Some(4),
        Vec2::splat(40.),
    );
    assert_eq!(
        observed(&mut f.app),
        [4, 1, 0].map(|i| (false, f.ids[i], Some(11)))
    );
    assert_eq!(
        f.app.world().resource::<GuiInputState>().hovered_touch(22),
        Some(f.views[4])
    );
    sample(
        &mut f,
        Some(22),
        TouchPhase::Canceled,
        None,
        Vec2::splat(500.),
    );
    let events: Vec<_> = f
        .app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .drain_events()
        .filter(|event| event.kind == EventKind::RollOut)
        .collect();
    assert_eq!(
        events.iter().map(|event| event.target).collect::<Vec<_>>(),
        [f.ids[4], f.ids[1], f.ids[0]]
    );
    assert!(
        events
            .iter()
            .all(|event| event.pointer == Some(fairygui_core::Point::new(20., 20.)))
    );
}

#[derive(Resource, Default)]
struct Seen(Vec<(bool, Entity, Option<Entity>, Option<u64>)>);
fn enter(mut event: On<GuiRollOver>, mut seen: ResMut<Seen>) {
    assert!(!event.event.bubbling);
    seen.0
        .push((true, event.entity, event.target, event.event.touch_id));
    event.propagate(false);
}
fn leave(mut event: On<GuiRollOut>, mut seen: ResMut<Seen>) {
    assert!(!event.event.bubbling);
    seen.0
        .push((false, event.entity, event.target, event.event.touch_id));
    event.propagate(false);
}
#[test]
fn hover_observers_get_direct_targets_and_independent_stops_on_each_changed_node() {
    let mut f = fixture(false);
    f.app.init_resource::<Seen>().add_systems(
        Update,
        fairygui_bevy::runtime::emit_gui_events.after(GuiSystems::Input),
    );
    for view in f.views {
        f.app
            .world_mut()
            .entity_mut(view)
            .observe(enter)
            .observe(leave);
    }
    sample(&mut f, None, TouchPhase::Moved, Some(3), Vec2::splat(10.));
    assert_eq!(
        f.app.world().resource::<Seen>().0,
        [3, 1, 0].map(|i| (true, f.views[i], Some(f.views[i]), None))
    );
    f.app.world_mut().resource_mut::<Seen>().0.clear();
    sample(&mut f, None, TouchPhase::Moved, Some(4), Vec2::splat(20.));
    assert_eq!(
        f.app.world().resource::<Seen>().0,
        [
            (false, f.views[3], Some(f.views[3]), None),
            (true, f.views[4], Some(f.views[4]), None)
        ]
    );
    f.app.world_mut().resource_mut::<Seen>().0.clear();
    sample(&mut f, None, TouchPhase::Moved, None, Vec2::splat(30.));
    assert_eq!(
        f.app.world().resource::<Seen>().0,
        [4, 1, 0].map(|i| (false, f.views[i], Some(f.views[i]), None))
    );
}

#[test]
fn outgoing_hover_includes_existing_offstage_nodes_and_skips_disposed_subtrees() {
    for disposed in [false, true] {
        let mut f = fixture(false);
        sample(&mut f, None, TouchPhase::Moved, Some(3), Vec2::splat(10.));
        observed(&mut f.app);
        {
            let mut gui = f.app.world_mut().non_send_mut::<GuiRuntime>();
            if disposed {
                gui.store_mut().remove(f.ids[1]);
            } else {
                gui.store_mut().detach_from_parent(f.ids[1]);
            }
        }
        sample(&mut f, None, TouchPhase::Moved, Some(5), Vec2::splat(20.));
        let mut expected = vec![];
        if !disposed {
            expected.extend([3, 1].map(|i| (false, f.ids[i], None)));
        }
        expected.extend([5, 2, 0].map(|i| (true, f.ids[i], None)));
        assert_eq!(observed(&mut f.app), expected);
    }
}

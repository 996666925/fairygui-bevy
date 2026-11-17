use super::*;
use fairygui_core::{package::ObjectType, pointer::ClickTargets};

#[derive(Resource)]
struct Case {
    ids: Vec<GObjectId>,
    views: Vec<Option<Entity>>,
    target: GObjectId,
    stop: Option<GObjectId>,
}
#[derive(Resource, Default)]
struct Seen(Vec<String>);

fn record(context: &mut GuiEvent, stopped: bool, case: &Case, seen: &mut Seen) -> bool {
    let ordinal = case
        .ids
        .iter()
        .position(|id| *id == context.event.current_target)
        .unwrap()
        + 1;
    let GuiEventData::Text(value) = &context.event.data else {
        panic!("shared payload");
    };
    let value: usize = value.parse().unwrap();
    seen.0
        .push(format!("{ordinal}:{value}:{}", u8::from(stopped)));
    context.event.data = GuiEventData::Text((value + 1).to_string());
    assert_eq!(context.event.target, case.target);
    assert_eq!(
        context.target,
        case.views[case.ids.iter().position(|id| *id == case.target).unwrap()]
    );
    assert_eq!(
        context.event.bubbling,
        context.event.current_target != case.target
    );
    assert_eq!(context.event.touch_id, Some(41));
    assert_eq!(
        context.event.pointer,
        Some(fairygui_core::Point::new(20., 30.))
    );
    Some(context.event.current_target) == case.stop
}
fn up(mut event: On<GuiMouseUp>, case: Res<Case>, mut seen: ResMut<Seen>) {
    let stopped = !event.get_propagate();
    if record(&mut event, stopped, &case, &mut seen) {
        event.propagate(false);
    }
}
fn right_up(mut event: On<GuiRightMouseUp>, case: Res<Case>, mut seen: ResMut<Seen>) {
    let stopped = !event.get_propagate();
    if record(&mut event, stopped, &case, &mut seen) {
        event.propagate(false);
    }
}

#[test]
fn an_unmaterialized_first_press_recipient_still_bounds_an_inherited_stop() {
    let mut app = App::new();
    app.init_non_send::<GuiRuntime>()
        .init_resource::<Seen>()
        .add_systems(Update, emit_gui_events);
    let (ids, route) = {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        let store = runtime.store_mut();
        let ids: Vec<_> = (0..5)
            .map(|_| store.create(ObjectType::Component))
            .collect();
        for (child, parent) in [(1, 0), (2, 1), (3, 0), (4, 3)] {
            store.set_parent(ids[child], Some(ids[parent]));
        }
        store.attach_to_stage(ids[0]);
        let press = ClickTargets::begin(store, ids[2], None);
        let route = press.release_route(store, ids[4]);
        (ids, route)
    };
    let views = ids
        .iter()
        .enumerate()
        .map(|(i, &id)| (i != 2).then(|| app.world_mut().spawn(GObjectRef(id)).observe(up).id()))
        .collect();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .dispatch_pointer_release(
            UiEvent::new(EventKind::MouseUp, ids[4])
                .with_touch(41)
                .with_pointer(fairygui_core::Point::new(20., 30.))
                .with_data(GuiEventData::Text("0".into())),
            route,
        );
    app.insert_resource(Case {
        target: ids[4],
        stop: Some(ids[4]),
        ids,
        views,
    });
    app.update();
    assert_eq!(app.world().resource::<Seen>().0, ["5:0:0"]);
}

#[test]
fn release_observer_order_stops_and_payloads_match_560_native_bubble_traces() {
    let mut count = 0;
    for line in
        include_str!("../../fairygui-core/tests/fixtures/pointer-release-reference.txt").lines()
    {
        let v: Vec<_> = line.split_whitespace().collect();
        if v.first() != Some(&"case") {
            continue;
        }
        let number = |i: usize| v[i].parse::<usize>().unwrap();
        let mut app = App::new();
        app.init_non_send::<GuiRuntime>()
            .init_resource::<Seen>()
            .add_systems(Update, emit_gui_events);
        let (ids, route) = {
            let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
            let store = runtime.store_mut();
            let ids: Vec<_> = (0..6)
                .map(|_| store.create(ObjectType::Component))
                .collect();
            for (index, parent) in [0, 1, 1, 2, 2, 3].into_iter().enumerate() {
                if parent > 0 {
                    store.set_parent(ids[index], Some(ids[parent - 1]));
                }
            }
            store.attach_to_stage(ids[0]);
            let press =
                ClickTargets::begin(store, ids[3], number(4).checked_sub(1).map(|i| ids[i]));
            match v[3] {
                "none" => {}
                "detach" => {
                    store.detach_from_parent(ids[3]);
                }
                "reparent" => {
                    store.set_parent(ids[3], Some(ids[2]));
                }
                "remove" => {
                    store.remove(ids[3]);
                }
                "detach-parent" => {
                    store.detach_from_parent(ids[1]);
                }
                _ => unreachable!(),
            }
            let route = press.release_route(store, ids[number(2) - 1]);
            assert!(
                !press.is_empty(),
                "MouseUp must not consume clickTest's saved path"
            );
            let parse_ids = |field: &str| -> Vec<_> {
                if field == "_" {
                    vec![]
                } else {
                    field
                        .split(',')
                        .map(|i| ids[i.parse::<usize>().unwrap() - 1])
                        .collect()
                }
            };
            assert_eq!(route.bubble, parse_ids(v[6]), "bubble {line}");
            assert_eq!(route.pressed, parse_ids(v[7]), "tail {line}");
            (ids, route)
        };
        let mut views = Vec::new();
        for &id in &ids {
            let alive = app.world().non_send::<GuiRuntime>().store().contains(id);
            views.push(alive.then(|| {
                app.world_mut()
                    .spawn(GObjectRef(id))
                    .observe(up)
                    .observe(right_up)
                    .id()
            }));
        }
        let target = ids[number(2) - 1];
        let kind = if number(1) == 0 {
            EventKind::MouseUp
        } else {
            EventKind::RightMouseUp
        };
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .dispatch_pointer_release(
                UiEvent::new(kind, target)
                    .with_touch(41)
                    .with_pointer(fairygui_core::Point::new(20., 30.))
                    .with_data(GuiEventData::Text("0".into())),
                route,
            );
        app.insert_resource(Case {
            stop: number(5).checked_sub(1).map(|i| ids[i]),
            ids,
            views,
            target,
        });
        app.update();
        let expected: Vec<_> = if v[8] == "_" {
            vec![]
        } else {
            v[8].split(',').map(str::to_owned).collect()
        };
        assert_eq!(app.world().resource::<Seen>().0, expected, "{line}");
        count += 1;
    }
    assert_eq!(count, 560);
}

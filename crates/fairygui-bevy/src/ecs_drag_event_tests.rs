use super::*;
use fairygui_core::{package::ObjectType, pointer::ClickTargets};

#[derive(Resource)]
struct Case {
    ids: Vec<GObjectId>,
    views: Vec<Option<Entity>>,
    hit: GObjectId,
    move_stop: Option<GObjectId>,
    drag_stop: Option<GObjectId>,
}
#[derive(Resource, Default)]
struct Seen(Vec<String>);
fn moved(mut event: On<GuiMouseMove>, case: Res<Case>) {
    if Some(event.event.current_target) == case.move_stop {
        event.propagate(false);
    }
}
fn dragged(mut event: On<GuiMouseDrag>, case: Res<Case>, mut seen: ResMut<Seen>) {
    let ordinal = case
        .ids
        .iter()
        .position(|&id| id == event.event.current_target)
        .unwrap()
        + 1;
    let hit = case.ids.iter().position(|&id| id == case.hit).unwrap() + 1;
    let GuiEventData::Text(value) = &event.event.data else {
        panic!("shared drag payload");
    };
    let value: usize = value.parse().unwrap();
    seen.0.push(format!(
        "{ordinal}:{hit}:{value}:{}",
        u8::from(!event.get_propagate())
    ));
    assert_eq!(event.event.target, case.hit);
    assert_eq!(event.target, case.views[hit - 1]);
    assert_eq!(event.event.bubbling, event.event.current_target != case.hit);
    assert_eq!(event.event.touch_id, Some(51));
    assert_eq!(
        event.event.pointer,
        Some(fairygui_core::Point::new(20., 30.))
    );
    event.event.data = GuiEventData::Text((value + 1).to_string());
    if Some(event.event.current_target) == case.drag_stop {
        event.propagate(false);
    }
}

#[test]
fn saved_drag_recipients_targets_and_stops_match_560_native_mouse_and_touch_traces() {
    let mut count = 0;
    for line in
        include_str!("../../fairygui-core/tests/fixtures/pointer-drag-reference.txt").lines()
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
        let (ids, press) = {
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
                ClickTargets::begin(store, ids[3], number(3).checked_sub(1).map(|i| ids[i]));
            match v[2] {
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
            (ids, press)
        };
        let views = ids
            .iter()
            .map(|&id| {
                let alive = app.world().non_send::<GuiRuntime>().store().contains(id);
                alive.then(|| {
                    app.world_mut()
                        .spawn(GObjectRef(id))
                        .observe(moved)
                        .observe(dragged)
                        .id()
                })
            })
            .collect();
        let hit = ids[number(1) - 1];
        {
            let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
            runtime.dispatch_event(UiEvent::new(EventKind::MouseMove, hit));
            runtime.dispatch_press_path(
                UiEvent::new(EventKind::MouseDrag, hit)
                    .with_touch(51)
                    .with_pointer(fairygui_core::Point::new(20., 30.))
                    .with_data(GuiEventData::Text("0".into())),
                press.targets(),
            );
        }
        app.insert_resource(Case {
            move_stop: number(4).checked_sub(1).map(|i| ids[i]),
            drag_stop: number(5).checked_sub(1).map(|i| ids[i]),
            ids,
            views,
            hit,
        });
        app.update();
        let expected: Vec<_> = if v[6] == "_" {
            vec![]
        } else {
            v[6].split(',').map(str::to_owned).collect()
        };
        assert_eq!(app.world().resource::<Seen>().0, expected, "{line}");
        count += 1;
    }
    assert_eq!(count, 560);
}

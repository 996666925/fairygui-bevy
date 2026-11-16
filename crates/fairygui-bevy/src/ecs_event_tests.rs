use super::*;
use fairygui_core::{
    object::{EventData, EventKind},
    package::ObjectType,
};

#[derive(Resource, Default)]
struct Received(Vec<GuiEvent>);
fn received<E: GuiEventType>(event: On<E>, mut received: ResMut<Received>) {
    received.0.push(event.context().clone());
}
struct Fixture {
    app: App,
    ids: [GObjectId; 3],
    views: [Entity; 3],
    decoration: Entity,
}
fn fixture() -> Fixture {
    let mut app = App::new();
    app.init_non_send::<GuiRuntime>()
        .init_resource::<Received>()
        .add_systems(Update, emit_gui_events);
    let ids = {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        let store = runtime.store_mut();
        let ids = std::array::from_fn(|_| store.create(ObjectType::Component));
        store.set_parent(ids[1], Some(ids[0]));
        store.set_parent(ids[2], Some(ids[1]));
        ids
    };
    let views = ids.map(|id| {
        app.world_mut()
            .spawn(GObjectRef(id))
            .observe(received::<GuiClick>)
            .observe(received::<GuiMouseMove>)
            .observe(received::<GuiMouseDown>)
            .observe(received::<GuiCustomEvent>)
            .id()
    });
    let decoration = app
        .world_mut()
        .spawn_empty()
        .observe(received::<GuiClick>)
        .observe(received::<GuiMouseMove>)
        .observe(received::<GuiMouseDown>)
        .observe(received::<GuiCustomEvent>)
        .id();
    // Deliberately disagree with the logical native hierarchy.
    app.world_mut()
        .entity_mut(views[2])
        .insert(ChildOf(decoration));
    Fixture {
        app,
        ids,
        views,
        decoration,
    }
}
fn dispatch(f: &mut Fixture, kind: EventKind) {
    f.app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .dispatch_event(
            UiEvent::new(kind, f.ids[2])
                .with_touch(71)
                .with_pointer(fairygui_core::Point::new(20., 30.))
                .with_data(EventData::Text("original".into())),
        );
}

#[test]
fn one_event_traverses_logical_recipients_and_updates_both_entity_and_native_metadata() {
    let mut f = fixture();
    // A stale map must not bypass the live GObjectRef identity check.
    f.app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .entities
        .insert(f.ids[2], f.views[0]);
    dispatch(&mut f, EventKind::Click);
    f.app.update();
    let events = &f.app.world().resource::<Received>().0;
    assert_eq!(
        events.iter().map(|e| e.entity).collect::<Vec<_>>(),
        f.views.into_iter().rev().collect::<Vec<_>>()
    );
    for (event, &id) in events.iter().zip(f.ids.iter().rev()) {
        assert_eq!(event.target, Some(f.views[2]));
        assert_eq!(event.event.target, f.ids[2]);
        assert_eq!(event.event.current_target, id);
        assert_eq!(event.event.bubbling, id != f.ids[2]);
        assert_eq!(event.event.touch_id, Some(71));
        assert_eq!(
            event.event.pointer,
            Some(fairygui_core::Point::new(20., 30.))
        );
        assert_eq!(event.event.data, EventData::Text("original".into()));
        assert_ne!(event.entity, f.decoration);
    }
}

#[test]
fn entity_custom_dispatch_captures_its_path_and_preserves_each_payload() {
    use crate::{GuiMut, GuiWorldExt};
    use bevy::ecs::system::SystemState;
    let mut f = fixture();
    for system_param in [false, true] {
        let data = GuiEventData::Text(format!("payload {system_param}"));
        if system_param {
            let mut state = SystemState::<GuiMut>::new(f.app.world_mut());
            assert!(
                state
                    .get_mut(f.app.world_mut())
                    .unwrap()
                    .dispatch_custom(f.views[2], "saved", data)
            );
        } else {
            assert!(
                f.app
                    .world_mut()
                    .gui_dispatch_custom(f.views[2], "saved", data)
            );
        }
    }
    assert!(f.app.world().resource::<Received>().0.is_empty());
    // Neither Bevy ChildOf nor a native reparent after dispatch can reroute it.
    f.app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_parent(f.ids[2], None);
    f.app.update();
    let events = &f.app.world().resource::<Received>().0;
    assert_eq!(events.len(), 6);
    for (group, system_param) in events.as_chunks::<3>().0.iter().zip([false, true]) {
        assert_eq!(
            group.iter().map(|e| e.entity).collect::<Vec<_>>(),
            f.views.into_iter().rev().collect::<Vec<_>>()
        );
        for event in group {
            assert_eq!(event.target, Some(f.views[2]));
            assert_eq!(event.event.kind, EventKind::Other("saved".into()));
            assert_eq!(
                event.event.data,
                GuiEventData::Text(format!("payload {system_param}"))
            );
            assert_eq!(event.event.pointer, None);
            assert_eq!(event.event.touch_id, None);
        }
    }
    assert!(
        !f.app
            .world_mut()
            .gui_dispatch_custom(f.decoration, "invalid", GuiEventData::None)
    );
    f.app.world_mut().despawn(f.views[2]);
    let mut state = SystemState::<GuiMut>::new(f.app.world_mut());
    assert!(!state.get_mut(f.app.world_mut()).unwrap().dispatch_custom(
        f.views[2],
        "gone",
        GuiEventData::None
    ));
    assert!(!World::new().gui_dispatch_custom(
        Entity::PLACEHOLDER,
        "no runtime",
        GuiEventData::None
    ));
}

#[test]
fn custom_dispatch_from_an_observer_publishes_next_frame_with_independent_propagation() {
    use crate::{GuiMut, GuiWorldExt};
    let mut f = fixture();
    f.app.world_mut().entity_mut(f.views[2]).observe(
        |mut event: On<GuiCustomEvent>, mut gui: GuiMut| {
            if event.name() == "outer" {
                event.propagate(false);
                assert!(gui.dispatch_custom(
                    event.entity,
                    "inner",
                    GuiEventData::Text("next frame".into())
                ));
            }
        },
    );
    assert!(
        f.app
            .world_mut()
            .gui_dispatch_custom(f.views[2], "outer", GuiEventData::None)
    );
    f.app.update();
    let events = &f.app.world().resource::<Received>().0;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].entity, f.views[2]);
    assert_eq!(events[0].event.kind, EventKind::Other("outer".into()));
    f.app.update();
    let events = &f.app.world().resource::<Received>().0;
    assert_eq!(events.len(), 4);
    assert_eq!(
        events[1..].iter().map(|e| e.entity).collect::<Vec<_>>(),
        f.views.into_iter().rev().collect::<Vec<_>>()
    );
    assert!(
        events[1..]
            .iter()
            .all(|e| e.event.kind == EventKind::Other("inner".into())
                && e.event.data == GuiEventData::Text("next frame".into()))
    );
    f.app.update();
    assert_eq!(f.app.world().resource::<Received>().0.len(), 4);
}

#[test]
fn observers_can_mutate_the_shared_payload_and_stop_only_the_current_dispatch() {
    let mut f = fixture();
    f.app
        .world_mut()
        .entity_mut(f.views[2])
        .observe(|mut event: On<GuiClick>| {
            assert_eq!(event.original_event_target(), event.target.unwrap());
            event.event.data = EventData::Text("handled".into());
        });
    f.app
        .world_mut()
        .entity_mut(f.views[1])
        .observe(|mut event: On<GuiClick>| {
            event.propagate(false);
        });
    dispatch(&mut f, EventKind::Click);
    dispatch(&mut f, EventKind::Click); // identical events still have independent triggers
    dispatch(&mut f, EventKind::MouseMove);
    f.app.update();
    let events = &f.app.world().resource::<Received>().0;
    for &view in &f.views[1..] {
        assert_eq!(
            events
                .iter()
                .filter(|e| e.entity == view && e.event.kind == EventKind::Click)
                .count(),
            2,
            "stopping does not skip other observers on the same entity"
        );
    }
    assert!(
        events
            .iter()
            .all(|e| e.entity != f.views[0] || e.event.kind != EventKind::Click)
    );
    assert!(
        events
            .iter()
            .filter(|e| e.entity == f.views[1] && e.event.kind == EventKind::Click)
            .all(|e| e.event.data == EventData::Text("handled".into()))
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| e.event.kind == EventKind::MouseMove)
            .count(),
        3
    );
}

#[test]
fn captured_paths_keep_native_stops_and_do_not_follow_later_reparenting() {
    let mut f = fixture();
    f.app
        .world_mut()
        .entity_mut(f.views[1])
        .observe(|mut event: On<GuiMouseDown>| {
            event.propagate(true);
        });
    f.app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .dispatch_event_until(UiEvent::new(EventKind::MouseDown, f.ids[2]), Some(f.ids[1]));
    let replacement = f
        .app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .create(ObjectType::Component);
    let replacement_view = f
        .app
        .world_mut()
        .spawn(GObjectRef(replacement))
        .observe(received::<GuiClick>)
        .observe(received::<GuiMouseMove>)
        .observe(received::<GuiMouseDown>)
        .observe(received::<GuiCustomEvent>)
        .id();
    f.app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_parent(f.ids[2], Some(replacement));
    f.app.update();
    let events = &f.app.world().resource::<Received>().0;
    assert_eq!(
        events.iter().map(|e| e.entity).collect::<Vec<_>>(),
        [f.views[2], f.views[1]]
    );
    assert!(
        events
            .iter()
            .all(|e| e.entity != f.views[0] && e.entity != replacement_view)
    );
}

#[test]
fn missing_views_are_skipped_without_losing_the_original_native_target() {
    let mut f = fixture();
    dispatch(&mut f, EventKind::Click);
    f.app
        .world_mut()
        .entity_mut(f.views[2])
        .remove::<GObjectRef>();
    f.app.update();
    let events = &f.app.world().resource::<Received>().0;
    assert_eq!(events.len(), 2);
    for (event, (&entity, &id)) in events
        .iter()
        .zip(f.views[..2].iter().rev().zip(f.ids[..2].iter().rev()))
    {
        assert_eq!(event.entity, entity);
        assert_eq!(event.target, None);
        assert_eq!(event.event.current_target, id);
        assert_eq!(event.event.target, f.ids[2]);
        assert!(event.event.bubbling);
    }
}

#[test]
fn a_direct_notification_and_nested_dispatch_do_not_inherit_another_events_stop() {
    let mut f = fixture();
    let root = f.views[0];
    let root_id = f.ids[0];
    f.app.world_mut().entity_mut(f.views[2]).observe(
        move |mut event: On<GuiClick>, mut commands: Commands| {
            event.propagate(false);
            commands.trigger(GuiCustomEvent::new(root, root_id, "nested"));
        },
    );
    dispatch(&mut f, EventKind::Click);
    f.app.update();
    let events = &f.app.world().resource::<Received>().0;
    assert_eq!(events.len(), 2);
    assert!(
        events
            .iter()
            .any(|e| e.entity == f.views[2] && e.event.kind == EventKind::Click)
    );
    assert!(
        events
            .iter()
            .any(|e| e.entity == root && e.event.kind == EventKind::Other("nested".into()))
    );
    f.app.world_mut().resource_mut::<Received>().0.clear();
    f.app
        .world_mut()
        .trigger(GuiMouseMove::new(f.views[2], f.ids[2]));
    assert_eq!(
        f.app.world().resource::<Received>().0.len(),
        1,
        "direct notifications don't walk ChildOf"
    );
}

#[test]
fn typed_input_notifications_keep_committed_text_and_never_bubble_to_parent_controls() {
    #[derive(Resource, Default)]
    struct Texts(Vec<(Entity, String)>);
    let mut f = fixture();
    f.app.init_resource::<Texts>();
    for entity in f.views {
        f.app
            .world_mut()
            .entity_mut(entity)
            .observe(|event: On<GuiInputEvent>, mut seen: ResMut<Texts>| {
                seen.0.push((event.entity, event.value().to_owned()));
            })
            .observe(|event: On<GuiChange>, mut seen: ResMut<Texts>| {
                seen.0.push((event.entity, event.value().to_owned()));
            })
            .observe(|event: On<GuiEnter>, mut seen: ResMut<Texts>| {
                seen.0.push((event.entity, event.value().to_owned()));
            });
    }
    let mut runtime = f.app.world_mut().non_send_mut::<GuiRuntime>();
    for kind in [EventKind::Input, EventKind::Change, EventKind::Enter] {
        runtime.emit_input_event(kind, f.ids[2], "中文 secret");
    }
    f.app.update();
    assert_eq!(
        f.app.world().resource::<Texts>().0,
        vec![(f.views[2], "中文 secret".into()); 3]
    );
}

#[test]
fn click_observers_read_current_button_state_and_write_commands_in_the_same_frame() {
    use crate::prelude::*;
    use bevy::{
        ecs::system::SystemState,
        picking::hover::Hovered,
        ui::{ComputedStackIndex, RelativeCursorPosition},
        window::PrimaryWindow,
    };
    #[derive(Resource, Default)]
    struct Selected(Vec<bool>);
    fn click(
        mut event: On<GuiClick>,
        gui: GuiQuery,
        selected: Query<&GuiButtonSelected>,
        mut seen: ResMut<Selected>,
        _commands: Commands,
        mut gui_edit: crate::GuiMut,
    ) {
        let checked = selected.get(event.entity).unwrap().0;
        seen.0.push(checked);
        if seen.0.len() == 1 {
            event.propagate(false);
        }
        let parent = gui.get(event.entity).unwrap().parent().unwrap();
        let label = gui.child_by_name(parent, "n8").unwrap();
        gui_edit.edit(label, |object: &mut crate::GuiObjectMut| {
            object.set_text(format!("Checked: {checked}"))
        });
    }
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>()
    .init_resource::<Selected>()
    .init_resource::<Received>();
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: fairygui_core::FuiPackage::parse(
                "Basics",
                include_bytes!("../../fairygui-demo/assets/ui/Basics.fui"),
            )
            .unwrap(),
            res_key: "ui/Basics".into(),
        });
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "Demo_Button"))
        .id();
    let mut window = Window {
        focused: true,
        ..default()
    };
    window.set_cursor_position(Some(Vec2::splat(20.)));
    app.world_mut().spawn((window, PrimaryWindow));
    app.update();
    let (root, button, label) = {
        let mut query = SystemState::<GuiQuery>::new(app.world_mut());
        let gui = query.get(app.world()).unwrap();
        (
            gui.view(owner).unwrap(),
            gui.child_by_name(owner, "n4").unwrap(),
            gui.child_by_name(owner, "n8").unwrap(),
        )
    };
    app.world_mut()
        .entity_mut(root)
        .observe(received::<GuiClick>)
        .observe(received::<GuiMouseMove>)
        .observe(received::<GuiMouseDown>)
        .observe(received::<GuiCustomEvent>);
    app.world_mut().entity_mut(button).observe(click).insert((
        Hovered(true),
        ComputedStackIndex(1000),
        RelativeCursorPosition {
            cursor_over: true,
            normalized: Some(Vec2::ZERO),
        },
    ));
    for (index, expected) in [true, false].into_iter().enumerate() {
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        assert_eq!(app.world().resource::<Selected>().0[index], expected);
        assert_eq!(
            app.world().get::<GuiText>(label).unwrap().value(),
            format!("Checked: {expected}")
        );
        assert_eq!(
            app.world()
                .resource::<Received>()
                .0
                .iter()
                .filter(|e| e.event.kind == EventKind::Click)
                .count(),
            index
        );
    }
}

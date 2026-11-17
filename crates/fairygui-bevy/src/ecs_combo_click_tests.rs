use super::*;
use bevy::{
    input::touch::{TouchInput, TouchPhase},
    picking::{backend::HitData, hover::HoverMap, hover::Hovered, pointer::PointerId},
    ui::{ComputedStackIndex, RelativeCursorPosition},
    window::PrimaryWindow,
};

#[derive(Resource)]
struct Target(Entity);
#[derive(Resource, Default, Debug)]
struct Seen {
    items: Vec<(usize, i32, bool)>,
    changes: Vec<(i32, String, bool, Option<u64>, u8)>,
    custom: usize,
}

fn item_clicked(
    event: On<GuiListItemClick>,
    target: Res<Target>,
    gui: GuiQuery,
    mut seen: ResMut<Seen>,
) {
    let combo = gui.combo_box(target.0).unwrap();
    seen.items
        .push((event.index, combo.selected_index(), combo.is_open()));
}
fn changed(event: On<GuiStateChanged>, gui: GuiQuery, mut seen: ResMut<Seen>) {
    let combo = gui.combo_box(event.entity).unwrap();
    seen.changes.push((
        combo.selected_index(),
        gui.text(event.entity).unwrap().into(),
        combo.is_open(),
        event.event.touch_id,
        event.event.click_count,
    ));
}
fn setup() -> (App, Entity, Entity, Entity, Vec<Entity>) {
    let (mut app, owner, _) = fixture(true);
    let mut window = Window {
        focused: true,
        ..default()
    };
    window.set_cursor_position(Some(Vec2::splat(10.)));
    let window = app.world_mut().spawn((window, PrimaryWindow)).id();
    let combo = view(&app, owner);
    {
        let mut state = app.world_mut().get_mut::<GuiComboBoxState>(combo).unwrap();
        state.items = vec!["A".into(), "B".into(), "C".into()];
        state.selected_index = 1;
    }
    app.update();
    open(&mut app, owner);
    let state = snapshot(&mut app, owner);
    let list = state.list().unwrap();
    let rows = SystemState::<GuiQuery>::new(app.world_mut())
        .get(app.world())
        .unwrap()
        .get(list)
        .unwrap()
        .children()
        .to_vec();
    app.insert_resource(Target(combo)).init_resource::<Seen>();
    app.world_mut().entity_mut(list).observe(item_clicked);
    app.world_mut().entity_mut(combo).observe(changed);
    (app, combo, window, list, rows)
}
fn pointer(app: &mut App, window: Entity, target: Entity, touch: bool, down: bool, id: u64) {
    if touch {
        app.init_resource::<HoverMap>();
        app.world_mut().resource_mut::<HoverMap>().insert(
            PointerId::Touch(id),
            [(
                target,
                HitData::new(Entity::PLACEHOLDER, 0., Some(Vec3::ZERO), None),
            )]
            .into(),
        );
        app.world_mut().write_message(TouchInput {
            window,
            id,
            phase: if down {
                TouchPhase::Started
            } else {
                TouchPhase::Ended
            },
            position: Vec2::splat(10.),
            force: None,
        });
    } else {
        let entities: Vec<_> = app
            .world_mut()
            .query_filtered::<Entity, With<GObjectRef>>()
            .iter(app.world())
            .collect();
        for entity in entities {
            app.world_mut()
                .entity_mut(entity)
                .insert(Hovered(entity == target));
        }
        app.world_mut().entity_mut(target).insert((
            ComputedStackIndex(200),
            RelativeCursorPosition {
                cursor_over: true,
                normalized: Some(Vec2::ZERO),
            },
        ));
        let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        if down {
            mouse.press(MouseButton::Left);
        } else {
            mouse.release(MouseButton::Left);
        }
    }
}
fn update(app: &mut App) {
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
}

#[test]
fn mouse_and_touch_publish_item_before_combo_change_and_apply_observer_edits() {
    for touch in [false, true] {
        let (mut app, combo, window, list, rows) = setup();
        app.world_mut().entity_mut(list).observe(
            |_: On<GuiListItemClick>,
             target: Res<Target>,
             mut states: Query<&mut GuiComboBoxState>,
             mut edit: GuiMut| {
                states.get_mut(target.0).unwrap().items[0] = "Edited".into();
                assert!(edit.dispatch_custom(target.0, "after-item", GuiEventData::None));
            },
        );
        app.world_mut()
            .entity_mut(combo)
            .observe(|_: On<GuiCustomEvent>, mut seen: ResMut<Seen>| seen.custom += 1);
        for down in [true, false] {
            pointer(&mut app, window, rows[0], touch, down, 71);
            update(&mut app);
        }
        let seen = app.world().resource::<Seen>();
        assert_eq!(seen.items, [(0, 1, true)], "touch={touch}");
        assert_eq!(
            seen.changes,
            [(0, "Edited".into(), false, touch.then_some(71), 1)],
            "touch={touch}"
        );
        assert_eq!(
            seen.custom, 0,
            "observer notifications wait for next Publish"
        );
        assert_eq!(app.world().get::<GuiListSelection>(list).unwrap().0, [0]);
        update(&mut app);
        assert_eq!(app.world().resource::<Seen>().custom, 1);
    }
}

#[test]
fn reselect_applies_title_after_item_observer_without_a_second_frame_or_extra_change() {
    let (mut app, combo, window, list, rows) = setup();
    app.world_mut().entity_mut(list).observe(
        |_: On<GuiListItemClick>, target: Res<Target>, mut text: Query<&mut GuiText>| {
            text.get_mut(target.0).unwrap().0 = "Temporary".into();
        },
    );
    for down in [true, false] {
        pointer(&mut app, window, rows[1], false, down, 0);
        update(&mut app);
    }
    let seen = app.world().resource::<Seen>();
    assert_eq!(seen.items, [(1, 1, true)]);
    assert_eq!(seen.changes, [(1, "B".into(), false, None, 1)]);
    assert_eq!(app.world().get::<GuiText>(combo).unwrap().0, "B");
    update(&mut app);
    assert_eq!(app.world().resource::<Seen>().changes.len(), 1);
}

#[test]
fn two_touches_released_together_keep_first_deferred_choice_and_both_item_events() {
    let (mut app, _, window, _, rows) = setup();
    for down in [true, false] {
        pointer(&mut app, window, rows[0], true, down, 11);
        pointer(&mut app, window, rows[2], true, down, 22);
        update(&mut app);
    }
    let seen = app.world().resource::<Seen>();
    assert_eq!(seen.items, [(0, 1, true), (2, 1, true)]);
    assert_eq!(seen.changes, [(0, "A".into(), false, Some(11), 1)]);
}

#[test]
fn an_item_observer_can_close_the_dropdown_without_closing_a_new_unrelated_popup() {
    let (mut app, combo, window, list, rows) = setup();
    #[derive(Resource)]
    struct Other(fairygui_core::object::GObjectId);
    let other = app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .create(fairygui_core::package::ObjectType::Component);
    app.insert_resource(Other(other));
    app.world_mut().entity_mut(list).observe(
        |_: On<GuiListItemClick>, other: Res<Other>, mut runtime: NonSendMut<GuiRuntime>| {
            runtime.hide_popup(None);
            assert!(runtime.show_popup(other.0, fairygui_core::Point::ZERO));
        },
    );
    for down in [true, false] {
        pointer(&mut app, window, rows[0], false, down, 0);
        update(&mut app);
    }
    assert_eq!(app.world().non_send::<GuiRuntime>().popup_stack(), [other]);
    assert_eq!(
        app.world()
            .get::<GuiComboBoxState>(combo)
            .unwrap()
            .selected_index(),
        0
    );
    assert_eq!(app.world().resource::<Seen>().changes.len(), 1);
}

use super::*;
use crate::{GuiControllers, GuiListItemClick, GuiListSelection, GuiMenuAction, GuiText};
use bevy::{
    input::touch::{TouchInput, TouchPhase},
    picking::{backend::HitData, hover::HoverMap, hover::Hovered, pointer::PointerId},
    ui::{ComputedStackIndex, RelativeCursorPosition},
    window::PrimaryWindow,
};

#[derive(Resource, Default)]
struct Seen {
    items: Vec<(Entity, usize, i32)>,
    actions: Vec<GuiMenuAction>,
    parent_actions: usize,
}
fn selected(
    event: On<GuiListItemClick>,
    controllers: Query<&GuiControllers>,
    mut seen: ResMut<Seen>,
) {
    let checked = controllers
        .get(event.item)
        .unwrap()
        .get("checked")
        .unwrap()
        .selected_index();
    seen.items.push((event.item, event.index, checked));
}
fn action(event: On<GuiMenuAction>, popup: Res<GuiPopupState>, mut seen: ResMut<Seen>) {
    assert!(!popup.is_open(event.menu));
    seen.actions.push((*event).clone());
}
fn setup() -> (App, Entity, Entity, Entity, Entity, Entity) {
    let (mut app, asset) = fixture();
    load(&mut app, &asset, true);
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "Demo_Popup"))
        .id();
    let mut window = Window {
        focused: true,
        ..default()
    };
    window.set_cursor_position(Some(Vec2::splat(10.)));
    let window = app.world_mut().spawn((window, PrimaryWindow)).id();
    app.update();
    show_menu(&mut app, owner);
    app.update();
    let menu = app.world().resource::<GuiPopupState>().menu().unwrap();
    let mut state = SystemState::<GuiQuery>::new(app.world_mut());
    let gui = state.get(app.world()).unwrap();
    let list = gui.child_by_name(menu, "list").unwrap();
    let row = gui.child_by_name(list, "toggle").unwrap();
    app.init_resource::<Seen>();
    app.world_mut().entity_mut(list).observe(selected);
    app.world_mut().entity_mut(row).observe(action);
    app.world_mut()
        .entity_mut(menu)
        .observe(|_: On<GuiMenuAction>, mut seen: ResMut<Seen>| seen.parent_actions += 1);
    (app, owner, menu, list, row, window)
}
fn click(app: &mut App, row: Entity, window: Entity, touch: bool) {
    for down in [true, false] {
        if touch {
            app.init_resource::<HoverMap>();
            app.world_mut().resource_mut::<HoverMap>().insert(
                PointerId::Touch(41),
                [(
                    row,
                    HitData::new(Entity::PLACEHOLDER, 0., Some(Vec3::ZERO), None),
                )]
                .into(),
            );
            app.world_mut().write_message(TouchInput {
                window,
                id: 41,
                phase: if down {
                    TouchPhase::Started
                } else {
                    TouchPhase::Ended
                },
                position: Vec2::splat(10.),
                force: None,
            });
        } else {
            app.world_mut().entity_mut(row).insert((
                Hovered(true),
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
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
    }
}

#[test]
fn mouse_and_touch_publish_list_item_then_delayed_target_only_entity_action() {
    for touch in [false, true] {
        let (mut app, _, menu, list, row, window) = setup();
        click(&mut app, row, window, touch);
        assert_eq!(app.world().resource::<Seen>().items, [(row, 0, 1)]);
        assert!(app.world().resource::<Seen>().actions.is_empty());
        assert_eq!(app.world().get::<GuiListSelection>(list).unwrap().0, [0]);
        // The native delay has not toggled checked or hidden the retained menu.
        assert!(app.world().resource::<GuiPopupState>().is_open(menu));
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .advance_menu_click(0.101);
        app.update();
        let seen = app.world().resource::<Seen>();
        assert_eq!(seen.actions.len(), 1);
        let event = &seen.actions[0];
        assert_eq!(
            (event.entity, event.menu, event.action),
            (row, menu, Some(73))
        );
        assert!(event.checked);
        assert_eq!(seen.parent_actions, 0, "menu actions do not bubble");
        assert!(
            app.world().get::<GuiText>(row).is_some(),
            "hidden rows remain addressable"
        );
        let messages: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<GuiMenuAction>>()
            .drain()
            .collect();
        assert_eq!(messages.len(), 1);
        assert_eq!((messages[0].entity, messages[0].menu), (row, menu));
    }
}

#[test]
fn immediate_item_observer_can_disable_action_before_native_delay_finishes() {
    let (mut app, _, menu, list, row, window) = setup();
    app.world_mut().entity_mut(list).observe(
        |event: On<GuiListItemClick>, mut display: Query<&mut crate::GuiDisplay>| {
            display.get_mut(event.item).unwrap().grayed = true;
        },
    );
    click(&mut app, row, window, false);
    assert_eq!(app.world().resource::<Seen>().items.len(), 1);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .advance_menu_click(0.101);
    app.update();
    assert!(app.world().resource::<Seen>().actions.is_empty());
    assert!(app.world().resource::<GuiPopupState>().is_open(menu));
    assert!(
        app.world()
            .get::<GuiListSelection>(list)
            .unwrap()
            .0
            .is_empty()
    );
}

#[test]
fn replaced_view_between_completion_and_publish_drops_action_without_retargeting() {
    let (mut app, _, _, _, row, _) = setup();
    let id = app.world().get::<GObjectRef>(row).unwrap().0;
    {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        runtime.activate_menu_item(id, None);
        runtime.advance_menu_click(0.101);
    }
    app.world_mut().entity_mut(row).remove::<GObjectRef>();
    app.world_mut().spawn(GObjectRef(id)).observe(action);
    app.update();
    assert!(app.world().resource::<Seen>().actions.is_empty());
    assert!(
        app.world_mut()
            .resource_mut::<Messages<GuiMenuAction>>()
            .drain()
            .next()
            .is_none()
    );
}

#[test]
fn action_observer_can_reopen_retained_menu_in_the_same_frame() {
    let (mut app, _, menu, _, row, window) = setup();
    app.world_mut()
        .entity_mut(row)
        .observe(|_: On<GuiMenuAction>, mut popup: GuiPopup| {
            assert!(popup.reopen_menu(Vec2::new(90., 100.)));
        });
    click(&mut app, row, window, false);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .advance_menu_click(0.101);
    app.update();
    assert_eq!(app.world().resource::<Seen>().actions.len(), 1);
    assert_eq!(app.world().resource::<GuiPopupState>().menu(), Some(menu));
    assert_eq!(
        app.world()
            .get::<crate::GuiTransform>(menu)
            .unwrap()
            .position(),
        Vec2::new(90., 100.)
    );
}

#[test]
fn ecs_caption_enable_and_checked_edits_are_read_at_delayed_activation() {
    let (mut app, _, menu, list, row, window) = setup();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .update_popup_menu_item("toggle", |item| item.enabled = false);
    app.update();
    app.world_mut()
        .get_mut::<crate::GuiDisplay>(row)
        .unwrap()
        .grayed = false;
    app.world_mut()
        .get_mut::<crate::GuiListSettings>(list)
        .unwrap()
        .selection_mode = crate::ListSelectionMode::None;
    app.world_mut().entity_mut(list).observe(
        |event: On<GuiListItemClick>,
         mut text: Query<&mut GuiText>,
         mut controllers: Query<&mut GuiControllers>| {
            text.get_mut(event.item).unwrap().0 = "Edited in observer".into();
            controllers
                .get_mut(event.item)
                .unwrap()
                .get_mut("checked")
                .unwrap()
                .selected_index = 2;
        },
    );
    click(&mut app, row, window, false);
    assert_eq!(app.world().resource::<Seen>().items, [(row, 0, 1)]);
    assert!(
        app.world()
            .get::<GuiListSelection>(list)
            .unwrap()
            .0
            .is_empty(),
        "menu honors None selection mode"
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .advance_menu_click(0.101);
    app.update();
    let seen = app.world().resource::<Seen>();
    assert_eq!(seen.actions.len(), 1);
    assert_eq!(seen.actions[0].caption, "Edited in observer");
    assert!(
        !seen.actions[0].checked,
        "current checked page toggles from 2 to 1"
    );
    assert_eq!(seen.actions[0].menu, menu);
    let runtime = app.world().non_send::<GuiRuntime>();
    let item = runtime.popup_menu_item("toggle").unwrap().0;
    assert!(item.enabled);
    assert_eq!(item.caption, "Edited in observer");
}

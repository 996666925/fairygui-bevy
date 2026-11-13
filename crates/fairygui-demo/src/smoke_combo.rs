//! Configure a real combo through ECS, then select and reselect with picking.
use super::*;

#[derive(Resource, Default)]
struct State {
    step: u8,
    frame: u32,
    since: u32,
    cursor: Option<Vec2>,
    combo: Option<Entity>,
    pane: Option<Entity>,
    rows: Vec<Entity>,
}
#[derive(Resource, Default)]
struct Events(Vec<(i32, String)>);
fn changed(event: On<GuiEvent>, gui: GuiQuery, mut events: ResMut<Events>) {
    if event.event.kind == fairygui_core::object::EventKind::StateChanged && !event.event.bubbling {
        let combo = gui.combo_box(event.entity).unwrap();
        events
            .0
            .push((combo.selected_index(), combo.value().unwrap().into()));
    }
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<State>()
        .init_resource::<Events>()
        .add_systems(
            PreUpdate,
            drive
                .after(InputSystems)
                .before(UiSystems::Focus)
                .before(PickingSystems::ProcessInput),
        );
}
#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    demo: Res<Demo>,
    controller: Res<demos::DemoController>,
    gui: GuiQuery,
    events: Res<Events>,
    existing: Query<()>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    mut windows: Query<(Entity, &mut Window), With<bevy::window::PrimaryWindow>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
    mut gui_edit: GuiMut,
) {
    state.frame += 1;
    assert!(
        state.frame < 2200,
        "combo smoke timed out at step {}",
        state.step
    );
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    let center =
        |entity| nodes.get(entity).unwrap().1.transform_point2(Vec2::ZERO) / window.scale_factor();
    let elapsed = state.frame - state.since;
    let mut next = false;
    match state.step {
        0 if state.frame > 30 => {
            if let Some(button) = demo
                .menu_owner
                .and_then(|root| gui.child_by_name(root, "n1"))
                && nodes.get(button).is_ok_and(|(node, _)| node.size().x > 0.)
            {
                state.cursor = Some(center(button));
                next = true;
            }
        }
        1 | 4 | 8 | 11 | 14 | 17 | 23 if elapsed >= 3 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        2 | 5 | 9 | 12 | 15 | 18 | 24 if elapsed >= 3 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        3 if elapsed > 8 => {
            if let Some(button) = demo
                .open
                .as_ref()
                .and_then(|(root, _, _)| gui.find_by_name(*root, "btn_ComboBox"))
            {
                state.cursor = Some(center(button));
                next = true;
            }
        }
        6 if elapsed > 8 => {
            if let Some(combo) = controller
                .basics_page_owner
                .and_then(|root| gui.child_by_name(root, "n1"))
                && let Some(exported) = gui.combo_box(combo)
            {
                assert_eq!(exported.items().len(), 8);
                commands.entity(combo).observe(changed);

                gui_edit.edit(combo, |object: &mut GuiObjectMut| {
                    object.set_combo_items(vec!["Apple".into(), "Banana".into(), "Cherry".into()])
                });

                gui_edit.edit(combo, |object: &mut GuiObjectMut| {
                    object.set_combo_values(vec!["a".into(), "b".into(), "c".into()])
                });

                gui_edit.edit(combo, |object: &mut GuiObjectMut| {
                    object.set_combo_visible_item_count(2)
                });

                gui_edit.edit(combo, |object: &mut GuiObjectMut| {
                    object.set_combo_popup_direction(GuiPopupDirection::Down)
                });

                gui_edit.edit(combo, |object: &mut GuiObjectMut| {
                    object.set_combo_selection(1)
                });

                state.combo = Some(combo);
                next = true;
            }
        }
        7 if elapsed > 5 => {
            let combo = state.combo.unwrap();
            assert_eq!(gui.combo_box(combo).unwrap().value(), Some("b"));
            assert_eq!(gui.text(combo), Some("Banana"));
            assert!(events.0.is_empty());
            state.cursor = Some(center(combo));
            next = true;
        }
        10 if elapsed > 12 => {
            let combo = gui.combo_box(state.combo.unwrap()).unwrap();
            assert!(combo.is_open());
            assert_eq!(combo.visible_item_count(), 2);
            let list = combo.list().unwrap();
            let rows = gui.get(list).unwrap().children();
            assert_eq!(rows.len(), 3);
            for (index, &row) in rows.iter().enumerate() {
                assert_eq!(gui.text(row), Some(["Apple", "Banana", "Cherry"][index]));
                assert_eq!(gui.get(row).unwrap().name(), ["a", "b", "c"][index]);
            }
            state.pane = combo.dropdown();
            state.rows = rows.to_vec();
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-combo-ecs.png"));
            state.cursor = Some(center(rows[0]));
            next = true;
        }
        13 if elapsed > 6 => {
            let combo = gui.combo_box(state.combo.unwrap()).unwrap();
            assert_eq!(events.0, [(0, "a".into())]);
            assert_eq!(combo.value(), Some("a"));
            assert!(!combo.is_open());
            state.cursor = Some(center(state.combo.unwrap()));
            next = true;
        }
        16 if elapsed > 6 => {
            let combo = gui.combo_box(state.combo.unwrap()).unwrap();
            assert_eq!(combo.dropdown(), state.pane);
            assert_eq!(
                gui.get(combo.list().unwrap()).unwrap().children(),
                state.rows
            );
            state.cursor = Some(center(state.rows[0]));
            next = true;
        }
        19 if elapsed > 6 => {
            assert_eq!(events.0, [(0, "a".into()), (0, "a".into())]);
            gui_edit.edit(state.combo.unwrap(), |object: &mut GuiObjectMut| {
                object.set_combo_value(Some("c".into()))
            });
            next = true;
        }
        20 if elapsed > 5 => {
            assert_eq!(
                gui.combo_box(state.combo.unwrap())
                    .unwrap()
                    .selected_index(),
                2
            );
            assert_eq!(
                events.0.len(),
                2,
                "programmatic selection must not emit a user change"
            );
            gui_edit.edit(state.combo.unwrap(), |object: &mut GuiObjectMut| {
                object.set_combo_value(Some("missing".into()))
            });
            next = true;
        }
        21 if elapsed > 5 => {
            assert_eq!(
                gui.combo_box(state.combo.unwrap())
                    .unwrap()
                    .selected_index(),
                -1
            );

            gui_edit.edit(state.combo.unwrap(), |object: &mut GuiObjectMut| {
                object.set_combo_items(vec!["Reloaded A".into(), "Reloaded B".into()])
            });

            gui_edit.edit(state.combo.unwrap(), |object: &mut GuiObjectMut| {
                object.set_combo_values(vec!["new-a".into(), "new-b".into()])
            });

            gui_edit.edit(state.combo.unwrap(), |object: &mut GuiObjectMut| {
                object.set_combo_visible_item_count(1)
            });

            gui_edit.edit(state.combo.unwrap(), |object: &mut GuiObjectMut| {
                object.set_combo_popup_direction(GuiPopupDirection::Up)
            });

            gui_edit.edit(state.combo.unwrap(), |object: &mut GuiObjectMut| {
                object.set_combo_selection(1)
            });

            next = true;
        }
        22 if elapsed > 5 => {
            assert_eq!(
                gui.combo_box(state.combo.unwrap()).unwrap().value(),
                Some("new-b")
            );
            state.cursor = Some(center(state.combo.unwrap()));
            next = true;
        }
        25 if elapsed > 12 => {
            let combo = gui.combo_box(state.combo.unwrap()).unwrap();
            assert_eq!(combo.dropdown(), state.pane);
            let rows = gui.get(combo.list().unwrap()).unwrap().children();
            assert_eq!(rows.len(), 2);
            assert_eq!(gui.text(rows[0]), Some("Reloaded A"));
            assert_eq!(gui.get(rows[1]).unwrap().name(), "new-b");
            assert!(
                gui.transform(state.pane.unwrap()).unwrap().position().y
                    < gui
                        .space(state.combo.unwrap())
                        .unwrap()
                        .local_to_global(Vec2::ZERO)
                        .unwrap()
                        .y
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-combo-ecs-up.png"));
            next = true;
        }
        26 if elapsed > 8 => {
            commands
                .entity(controller.basics_page_owner.unwrap())
                .despawn();
            next = true;
        }
        27 if elapsed > 6 => {
            assert!(!existing.contains(state.pane.unwrap()));
            assert!(!existing.contains(state.combo.unwrap()));
            info!(
                "demo smoke PASSED: ECS combo items/values/count/direction, current observer snapshots, real select/reselect, programmatic value/missing value, retained dropdown/rows, replacement on reopen and owner disposal"
            );
            exit.write(AppExit::Success);
            next = true;
        }
        _ => {}
    }
    if next {
        state.step += 1;
        state.since = state.frame;
    }
    window.set_cursor_position(state.cursor);
    move_pointer(window_entity, state.cursor, &mut pointers);
}

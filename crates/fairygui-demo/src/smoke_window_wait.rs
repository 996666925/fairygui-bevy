//! Actual ModalWaiting demo button input, blocking, reuse and disposal.
use super::*;

#[derive(Resource, Default)]
struct State {
    step: u8,
    frame: u32,
    since: u32,
    page: Option<Entity>,
    owner: Option<Entity>,
    pane: Option<Entity>,
    cursor: Option<Vec2>,
    clicks: usize,
    initial: Vec2,
    global_view: Option<Entity>,
    global_phase: u8,
    global_since: u32,
    global_clicks: usize,
    global_done: bool,
}
fn count(event: On<GuiEvent>, mut state: ResMut<State>) {
    if event.event().event.kind == fairygui_bevy::core::object::EventKind::Click {
        state.clicks += 1;
    }
}
fn count_global(event: On<GuiEvent>, mut state: ResMut<State>) {
    if event.event().event.kind == fairygui_bevy::core::object::EventKind::Click {
        state.global_clicks += 1;
    }
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<State>().add_systems(
        PreUpdate,
        drive
            .after(InputSystems)
            .before(UiSystems::Focus)
            .before(PickingSystems::ProcessInput),
    );
}
fn point(entity: Entity, nodes: &Query<(&ComputedNode, &UiGlobalTransform)>, factor: f32) -> Vec2 {
    nodes.get(entity).unwrap().1.transform_point2(Vec2::ZERO) / factor
}
#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    demo: Res<Demo>,
    gui: GuiQuery,
    global: Res<GuiGlobalWaitState>,
    owners: Query<(Entity, &GuiWindowState), With<crate::modal_wait_demo::DemoWindow>>,
    existing: Query<()>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    assets: Res<AssetServer>,
    mut windows: Query<(Entity, &mut Window)>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
    time: Res<Time<bevy::time::Real>>,
) {
    state.frame += 1;
    assert!(
        time.elapsed_secs_f64() < 45.,
        "window wait smoke timeout at step {}",
        state.step
    );
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    let elapsed = state.frame - state.since;
    let factor = window.scale_factor();
    if state.step == 1 && !state.global_done {
        let global_elapsed = state.frame - state.global_since;
        let mut advance_global = false;
        match state.global_phase {
            0 if elapsed >= 20 && global.is_waiting() && !owners.is_empty() => {
                let button = gui.child_by_name(state.page.unwrap(), "n0").unwrap();
                commands.entity(button).observe(count_global);
                state.global_view = global.view();
                state.cursor = Some(point(button, &nodes, factor));
                assert_eq!(
                    gui.transform(global.view().unwrap()).unwrap().size(),
                    Vec2::new(window.width(), window.height())
                );
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk("target/fairygui-demo-global-modal-wait.png"));
                advance_global = true;
            }
            1 if global_elapsed >= 4 => {
                mouse.press(MouseButton::Left);
                advance_global = true;
            }
            2 if global_elapsed >= 4 => {
                mouse.release(MouseButton::Left);
                advance_global = true;
            }
            3 if global_elapsed >= 8 => {
                assert_eq!(state.global_clicks, 0);
                assert!(owners.iter().all(|(_, state)| !state.is_showing()));
                window.resolution.set(1100., 650.);
                advance_global = true;
            }
            4 if global_elapsed >= 8 => {
                assert!(global.is_waiting());
                assert_eq!(
                    gui.transform(global.view().unwrap()).unwrap().size(),
                    Vec2::new(window.width(), window.height())
                );
                advance_global = true;
            }
            5 if !global.is_requested() && !global.is_waiting() => {
                assert_eq!(global.view(), state.global_view);
                assert!(!gui.display(state.global_view.unwrap()).unwrap().on_stage());
                state.global_done = true;
                state.since = state.frame;
            }
            _ => {}
        }
        if advance_global {
            state.global_phase += 1;
            state.global_since = state.frame;
        }
        window.set_cursor_position(state.cursor);
        move_pointer(window_entity, state.cursor, &mut pointers);
        return;
    }
    let mut next = false;
    match state.step {
        0 => {
            let Some(menu) = demo.menu_owner.filter(|&owner| gui.view(owner).is_some()) else {
                return;
            };
            commands.write_message(GuiObjectCommand::new(
                menu,
                GuiObjectAction::SetVisible(false),
            ));
            state.page = Some(
                commands
                    .spawn(FairyGuiRoot::new(
                        assets.load("ui/ModalWaiting.fui"),
                        "Main",
                    ))
                    .id(),
            );
            next = true;
        }
        1 if elapsed >= 20 && !owners.is_empty() => {
            let (owner, _) = owners.iter().next().unwrap();
            state.owner = Some(owner);
            let button = gui.child_by_name(state.page.unwrap(), "n0").unwrap();
            state.cursor = Some(point(button, &nodes, factor));
            next = true;
        }
        2 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        3 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        4 if elapsed >= 8 => {
            assert_eq!(state.global_clicks, 1);
            assert!(owners.get(state.owner.unwrap()).unwrap().1.is_showing());
            let button = gui.child_by_name(state.owner.unwrap(), "n1").unwrap();
            commands.entity(button).observe(count);
            state.cursor = Some(point(button, &nodes, factor));
            next = true;
        }
        5 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        6 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        7 if elapsed >= 8 => {
            let owner = state.owner.unwrap();
            let window = owners.get(owner).unwrap().1;
            assert!(window.is_modal_waiting());
            assert_eq!(window.modal_wait_request(), 1);
            assert!(window.modal_wait_error().is_none());
            state.pane = window.modal_wait_pane();
            assert!(state.pane.is_some());
            assert_eq!(state.clicks, 1);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-window-modal-wait.png"));
            next = true;
        }
        8 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        9 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        10 if elapsed >= 8 => {
            assert_eq!(
                state.clicks, 1,
                "modal pane must intercept the start button"
            );
            let owner = state.owner.unwrap();
            let window_state = owners.get(owner).unwrap().1;
            assert!(window_state.is_modal_waiting());
            state.initial = gui.transform(owner).unwrap().position();
            state.cursor = Some(point(window_state.drag_area().unwrap(), &nodes, factor));
            next = true;
        }
        11 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        12 if elapsed >= 4 => {
            *state.cursor.as_mut().unwrap() += Vec2::new(75., 30.);
            next = true;
        }
        13 if elapsed >= 8 => {
            assert!(
                (gui.transform(state.owner.unwrap()).unwrap().position() - state.initial).length()
                    > 40.
            );
            assert!(
                owners
                    .get(state.owner.unwrap())
                    .unwrap()
                    .1
                    .is_modal_waiting()
            );
            mouse.release(MouseButton::Left);
            next = true;
        }
        14 if elapsed >= 8
            && !owners
                .get(state.owner.unwrap())
                .unwrap()
                .1
                .is_modal_waiting() =>
        {
            assert_eq!(
                owners
                    .get(state.owner.unwrap())
                    .unwrap()
                    .1
                    .modal_wait_pane(),
                state.pane
            );
            assert!(!gui.display(state.pane.unwrap()).unwrap().on_stage());
            let button = gui.child_by_name(state.owner.unwrap(), "n1").unwrap();
            state.cursor = Some(point(button, &nodes, factor));
            next = true;
        }
        15 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        16 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        17 if elapsed >= 8 => {
            let window_state = owners.get(state.owner.unwrap()).unwrap().1;
            assert!(window_state.is_modal_waiting());
            assert_eq!(window_state.modal_wait_pane(), state.pane);
            assert_eq!(state.clicks, 2);
            state.cursor = Some(point(window_state.close_button().unwrap(), &nodes, factor));
            next = true;
        }
        18 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        19 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        20 if elapsed >= 8 => {
            let window_state = owners.get(state.owner.unwrap()).unwrap().1;
            assert!(!window_state.is_showing());
            assert!(!window_state.is_modal_waiting());
            commands.entity(state.page.unwrap()).despawn();
            next = true;
        }
        21 if elapsed >= 8 => {
            assert!(!existing.contains(state.pane.unwrap()));
            assert!(!existing.contains(state.owner.unwrap()));
            println!(
                "demo smoke PASSED: actual ModalWaiting buttons, content-only blocking, draggable title, timed request completion, retained pane reuse, close during wait and detached-pane disposal"
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

//! Actual picking -> typed click/double-click observers, independently bubbled.
use super::*;
use bevy::input::touch::{TouchInput, TouchPhase};
use fairygui_core::object::EventKind;

#[derive(Resource, Default)]
struct Seen {
    events: Vec<(EventKind, u8, Option<u64>)>,
    bubbled: usize,
}
fn click(event: On<GuiClick>, mut seen: ResMut<Seen>) {
    assert_eq!(event.target, Some(event.entity));
    seen.events
        .push((EventKind::Click, event.click_count(), event.event.touch_id));
}
fn double_click(event: On<GuiDoubleClick>, mut seen: ResMut<Seen>) {
    assert!(event.is_double_click());
    seen.events.push((
        EventKind::DoubleClick,
        event.click_count(),
        event.event.touch_id,
    ));
}
fn right(event: On<GuiRightClick>, mut seen: ResMut<Seen>) {
    seen.events.push((
        EventKind::RightClick,
        event.click_count(),
        event.event.touch_id,
    ));
}
fn ancestor(event: On<GuiDoubleClick>, mut seen: ResMut<Seen>) {
    assert!(event.event.bubbling && event.target != Some(event.entity));
    assert!(event.is_double_click());
    seen.bubbled += 1;
}

#[derive(Resource, Default)]
struct State {
    frame: u32,
    since: u32,
    step: u8,
    round: u8,
    owner: Option<Entity>,
    button: Option<Entity>,
    cursor: Option<Vec2>,
}
pub(super) fn install(app: &mut App) {
    app.world_mut().spawn(PointerId::Touch(77));
    app.init_resource::<State>()
        .init_resource::<Seen>()
        // Real Time advances deterministically even on slow validation GPUs.
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f64(1. / 60.),
        ))
        .add_systems(
            PreUpdate,
            drive
                .after(InputSystems)
                .before(bevy::input_focus::InputFocusSystems::Dispatch)
                .before(UiSystems::Focus)
                .before(PickingSystems::ProcessInput),
        );
}

#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    mut seen: ResMut<Seen>,
    demo: Res<Demo>,
    server: Res<AssetServer>,
    gui: GuiQuery,
    nodes: Query<&UiGlobalTransform>,
    mut windows: Query<(Entity, &mut Window)>,
    selected: Query<&GuiButtonSelected>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    mut touches: MessageWriter<TouchInput>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    assert!(
        state.frame < 600,
        "double click smoke timed out at round {} step {}",
        state.round,
        state.step
    );
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    let elapsed = state.frame - state.since;
    let mut next = false;
    match state.step {
        0 if state.frame > 25 => {
            let Some(menu) = demo.menu_owner.and_then(|owner| gui.view(owner)) else {
                return;
            };
            let mut display = *gui.display(menu).unwrap();
            display.visible = false;
            commands.entity(menu).insert(display);
            state.owner = Some(
                commands
                    .spawn(
                        FairyGuiRoot::new(server.load("ui/Basics.fui"), "Demo_Button")
                            .with_position(Vec2::new(80., 100.)),
                    )
                    .id(),
            );
            next = true;
        }
        1 if elapsed > 12 && gui.view(state.owner.unwrap()).is_some() => {
            let owner = state.owner.unwrap();
            let button = gui.child_by_name(owner, "n13").unwrap();
            for &child in gui.get(owner).unwrap().children() {
                let mut display = *gui.display(child).unwrap();
                display.visible = child == button;
                commands.entity(child).insert(display);
            }
            commands
                .entity(button)
                .insert((
                    GuiButtonSettings {
                        mode: ButtonMode::Check,
                        ..default()
                    },
                    GuiButtonSelected(false),
                ))
                .observe(click)
                .observe(double_click)
                .observe(right);
            commands.entity(gui.view(owner).unwrap()).observe(ancestor);
            state.button = Some(button);
            next = true;
        }
        2 if elapsed > 8 => {
            state.cursor = Some(
                nodes
                    .get(state.button.unwrap())
                    .unwrap()
                    .transform_point2(Vec2::ZERO)
                    / window.scale_factor(),
            );
            next = true;
        }
        3 if elapsed > 3 => {
            let button = if state.round < 4 {
                MouseButton::Left
            } else {
                MouseButton::Right
            };
            if state.round >= 6 {
                touches.write(TouchInput {
                    window: window_entity,
                    id: 77,
                    phase: TouchPhase::Started,
                    position: state.cursor.unwrap(),
                    force: None,
                });
            } else {
                mouse.press(button);
            }
            next = true;
        }
        4 if elapsed > 3 => {
            let button = if state.round < 4 {
                MouseButton::Left
            } else {
                MouseButton::Right
            };
            if state.round >= 6 {
                touches.write(TouchInput {
                    window: window_entity,
                    id: 77,
                    phase: TouchPhase::Ended,
                    position: state.cursor.unwrap(),
                    force: None,
                });
            } else {
                mouse.release(button);
            }
            next = true;
        }
        5 if elapsed > 3 => {
            let double = matches!(state.round, 1 | 2 | 5 | 7);
            let touch = (state.round >= 6).then_some(77);
            let kind = if (4..6).contains(&state.round) {
                EventKind::RightClick
            } else {
                EventKind::Click
            };
            let mut expected = vec![(kind, if double { 2 } else { 1 }, touch)];
            if double && state.round != 5 {
                expected.push((EventKind::DoubleClick, 2, touch));
            }
            assert_eq!(seen.events, expected, "round {}", state.round);
            assert_eq!(seen.bubbled, usize::from(double && state.round != 5));
            let selected = selected.get(state.button.unwrap()).unwrap().0;
            assert_eq!(
                selected,
                matches!(state.round, 0 | 2 | 6),
                "double click must not toggle selection again"
            );
            println!(
                "double click smoke round {} PASSED: {:?}",
                state.round, seen.events
            );
            seen.events.clear();
            seen.bubbled = 0;
            if state.round == 2 {
                next = true;
            } else if state.round < 7 {
                if state.round == 6 {
                    // Bevy retires a touch picking entity on Ended. Each new
                    // contact needs a fresh pointer, even when its OS ID repeats.
                    commands.spawn(PointerId::Touch(77));
                }
                state.round += 1;
                state.step = 3;
                state.since = state.frame;
            } else {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk("target/fairygui-demo-double-click.png"));
                state.step = 7;
                state.since = state.frame;
            }
        }
        6 if elapsed > 24 => {
            state.round += 1;
            state.step = 3;
            state.since = state.frame;
        }
        7 if elapsed > 8 => {
            commands.entity(state.owner.unwrap()).despawn();
            next = true;
        }
        8 if elapsed > 8 => {
            assert!(gui.view(state.owner.unwrap()).is_none());
            println!(
                "demo smoke PASSED: typed mouse/touch double clicks, ordinary click ordering, independent ancestor propagation, repeat/timeout, right-click metadata and one button toggle per click"
            );
            exit.write(AppExit::Success);
        }
        _ => {}
    }
    if next {
        state.step += 1;
        state.since = state.frame;
    }
    window.set_cursor_position(state.cursor);
    move_pointer(window_entity, state.cursor, &mut pointers);
    if state.round >= 6
        && let Some(position) = state.cursor
    {
        let target =
            bevy::camera::RenderTarget::Window(bevy::window::WindowRef::Entity(window_entity))
                .normalize(Some(window_entity))
                .unwrap();
        pointers.write(PointerInput::new(
            PointerId::Touch(77),
            Location { target, position },
            PointerAction::Move { delta: Vec2::ZERO },
        ));
    }
}

//! Actual picking after a pressed control moves, hides, detaches or is disposed.
use super::*;

#[derive(Resource, Default)]
struct Seen {
    clicks: Vec<(bool, Entity, Option<Entity>, Option<u64>)>,
    releases: Vec<Release>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Release {
    right: bool,
    entity: Entity,
    target: Option<Entity>,
    touch: Option<u64>,
    stopped: bool,
}
fn click(event: On<GuiClick>, mut seen: ResMut<Seen>) {
    seen.clicks
        .push((false, event.entity, event.target, event.event.touch_id));
}
fn right(event: On<GuiRightClick>, mut seen: ResMut<Seen>) {
    seen.clicks
        .push((true, event.entity, event.target, event.event.touch_id));
}

fn up(event: On<GuiMouseUp>, mut seen: ResMut<Seen>) {
    seen.releases.push(Release {
        right: false,
        entity: event.entity,
        target: event.target,
        touch: event.event.touch_id,
        stopped: !event.get_propagate(),
    });
}
fn right_up(mut event: On<GuiRightMouseUp>, state: Res<State>, mut seen: ResMut<Seen>) {
    seen.releases.push(Release {
        right: true,
        entity: event.entity,
        target: event.target,
        touch: event.event.touch_id,
        stopped: !event.get_propagate(),
    });
    if state.case == 1 && Some(event.entity) == state.root {
        event.propagate(false);
    }
}

#[derive(Resource, Default)]
struct State {
    frame: u32,
    since: u32,
    step: u8,
    case: u8,
    owner: Option<Entity>,
    root: Option<Entity>,
    button: Option<Entity>,
    cursor: Option<Vec2>,
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<State>()
        .init_resource::<Seen>()
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
    mut seen: ResMut<Seen>,
    demo: Res<Demo>,
    server: Res<AssetServer>,
    gui: GuiQuery,
    mut edit: GuiMut,
    nodes: Query<&UiGlobalTransform>,
    selected: Query<&GuiButtonSelected>,
    mut windows: Query<(Entity, &mut Window)>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    mut touches: MessageWriter<bevy::input::touch::TouchInput>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    use bevy::input::touch::{TouchInput, TouchPhase};
    state.frame += 1;
    assert!(
        state.frame < 600,
        "click recipient smoke timed out at {}/{}",
        state.case,
        state.step
    );
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    let elapsed = state.frame - state.since;
    let touch_id = 77 + u64::from(state.case);
    let mut next = false;
    match state.step {
        0 if state.frame > 25 => {
            let Some(menu) = demo.menu_owner.filter(|&owner| gui.view(owner).is_some()) else {
                return;
            };
            edit.get_mut(menu).unwrap().set_visible(false);
            state.owner = Some(
                commands
                    .spawn(
                        FairyGuiRoot::new(server.load("ui/Basics.fui"), "Demo_Button")
                            .with_position(Vec2::new(60., 60.))
                            .with_size(Vec2::new(700., 450.)),
                    )
                    .id(),
            );
            next = true;
        }
        1 if elapsed > 8 && gui.view(state.owner.unwrap()).is_some() => {
            let owner = state.owner.unwrap();
            let root = gui.view(owner).unwrap();
            let button = gui.child_by_name(owner, "n13").unwrap();
            for &child in gui.get(owner).unwrap().children() {
                edit.get_mut(child).unwrap().set_visible(child == button);
            }
            edit.get_mut(button)
                .unwrap()
                .set_position(Vec2::new(40., 80.));
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
                .observe(right)
                .observe(up)
                .observe(right_up);
            commands
                .entity(root)
                .observe(click)
                .observe(right)
                .observe(up)
                .observe(right_up);
            commands.queue(move |world: &mut World| {
                let object = world.get::<GObjectRef>(root).unwrap().0;
                world
                    .non_send_mut::<GuiRuntime>()
                    .store_mut()
                    .set_component_opaque(object, true);
            });
            if state.case >= 2 {
                commands.spawn(PointerId::Touch(touch_id));
            }
            state.root = Some(root);
            state.button = Some(button);
            seen.clicks.clear();
            seen.releases.clear();
            next = true;
        }
        2 if elapsed > 10 => {
            state.cursor = Some(
                nodes
                    .get(state.button.unwrap())
                    .unwrap()
                    .transform_point2(Vec2::ZERO)
                    / window.scale_factor(),
            );
            next = true;
        }
        3 if elapsed > 4 => {
            if state.case >= 2 {
                touches.write(TouchInput {
                    window: window_entity,
                    id: touch_id,
                    phase: TouchPhase::Started,
                    position: state.cursor.unwrap(),
                    force: None,
                });
            } else {
                mouse.press(if state.case == 1 {
                    MouseButton::Right
                } else {
                    MouseButton::Left
                });
            }
            next = true;
        }
        4 if elapsed > 4 => {
            let button = state.button.unwrap();
            match state.case {
                0 => edit
                    .get_mut(button)
                    .unwrap()
                    .set_position(Vec2::new(420., 80.)),
                1 => edit.get_mut(button).unwrap().set_visible(false),
                2 | 3 => {
                    let remove = state.case == 3;
                    commands.queue(move |world: &mut World| {
                        let object = world.get::<GObjectRef>(button).unwrap().0;
                        let mut runtime = world.non_send_mut::<GuiRuntime>();
                        if remove {
                            runtime.store_mut().remove(object);
                        } else {
                            runtime.store_mut().detach_from_parent(object);
                        }
                    });
                }
                4 => edit
                    .get_mut(state.root.unwrap())
                    .unwrap()
                    .set_visible(false),
                _ => unreachable!(),
            }
            next = true;
        }
        5 if elapsed > 8 => {
            if state.case >= 2 {
                touches.write(TouchInput {
                    window: window_entity,
                    id: touch_id,
                    phase: TouchPhase::Ended,
                    position: state.cursor.unwrap(),
                    force: None,
                });
            } else {
                mouse.release(if state.case == 1 {
                    MouseButton::Right
                } else {
                    MouseButton::Left
                });
            }
            next = true;
        }
        6 if elapsed > 6 => {
            let root = state.root.unwrap();
            let button = state.button.unwrap();
            let expected = if state.case < 2 || state.case == 4 {
                let right = state.case == 1;
                let touch = (state.case >= 2).then_some(touch_id);
                vec![
                    (right, button, Some(button), touch),
                    (right, root, Some(button), touch),
                ]
            } else {
                vec![(false, root, Some(root), Some(touch_id))]
            };
            assert_eq!(seen.clicks, expected, "case {}", state.case);
            let touch = (state.case >= 2).then_some(touch_id);
            let right = state.case == 1;
            let mut expected_up = vec![Release {
                right,
                entity: root,
                target: Some(root),
                touch,
                stopped: false,
            }];
            if state.case != 3 {
                expected_up.push(Release {
                    right,
                    entity: button,
                    target: Some(root),
                    touch,
                    stopped: right,
                });
            }
            if state.case == 4 {
                expected_up = [button, root]
                    .map(|entity| Release {
                        right: false,
                        entity,
                        target: None,
                        touch,
                        stopped: false,
                    })
                    .to_vec();
            }
            assert_eq!(seen.releases, expected_up, "release case {}", state.case);
            println!(
                "release recipient smoke case {} PASSED: {:?}",
                state.case, seen.releases
            );
            if state.case < 2 || state.case == 4 {
                assert_eq!(selected.get(button).unwrap().0, state.case != 1);
            }
            println!(
                "click recipient smoke case {} PASSED: {:?}",
                state.case, seen.clicks
            );
            if state.case == 0 {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk("target/fairygui-demo-click-targets.png"));
            }
            next = true;
        }
        7 if elapsed > 10 => {
            if state.case == 2 {
                let button = state.button.unwrap();
                commands.queue(move |world: &mut World| {
                    if let Some(reference) = world.get::<GObjectRef>(button) {
                        let object = reference.0;
                        world
                            .non_send_mut::<GuiRuntime>()
                            .store_mut()
                            .remove(object);
                    }
                });
            }
            commands.entity(state.owner.unwrap()).despawn();
            next = true;
        }
        8 if elapsed > 8 => {
            assert!(gui.view(state.owner.unwrap()).is_none());
            if state.case == 4 {
                println!(
                    "demo smoke PASSED: original click recipients after moving/hiding controls, native ancestor fallback after detach/disposal, hit-path release then saved press recipients, inherited stops, external stage targets, typed observers and one button toggle"
                );
                exit.write(AppExit::Success);
            } else {
                state.case += 1;
                state.step = 0;
                state.since = state.frame;
            }
        }
        _ => {}
    }
    if next {
        state.step += 1;
        state.since = state.frame;
    }
    window.set_cursor_position(state.cursor);
    move_pointer(window_entity, state.cursor, &mut pointers);
    if state.case >= 2
        && let Some(position) = state.cursor
    {
        let target =
            bevy::camera::RenderTarget::Window(bevy::window::WindowRef::Entity(window_entity))
                .normalize(Some(window_entity))
                .unwrap();
        pointers.write(PointerInput::new(
            PointerId::Touch(touch_id),
            Location { target, position },
            PointerAction::Move { delta: Vec2::ZERO },
        ));
    }
}

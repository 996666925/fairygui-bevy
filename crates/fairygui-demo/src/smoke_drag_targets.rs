//! Real picking -> captured drag observers across a change of logical parent.
use super::*;
use bevy::input::touch::{TouchInput, TouchPhase};

#[derive(Debug, PartialEq)]
struct Sample {
    entity: Entity,
    target: Option<Entity>,
    touch: Option<u64>,
    stopped: bool,
}
#[derive(Resource, Default)]
struct Seen {
    drags: Vec<Sample>,
    moves: usize,
    clicks: usize,
}
fn dragged(mut event: On<GuiMouseDrag>, state: Res<State>, mut seen: ResMut<Seen>) {
    seen.drags.push(Sample {
        entity: event.entity,
        target: event.target,
        touch: event.event.touch_id,
        stopped: !event.get_propagate(),
    });
    if state.case == 1 && Some(event.entity) == state.button {
        event.propagate(false);
    }
}
fn moved(mut event: On<GuiMouseMove>, mut seen: ResMut<Seen>) {
    seen.moves += 1;
    event.propagate(false);
}
fn clicked(_: On<GuiClick>, mut seen: ResMut<Seen>) {
    seen.clicks += 1;
}
#[derive(Resource, Default)]
struct State {
    frame: u32,
    since: u32,
    step: u8,
    case: u8,
    owners: Option<[Entity; 2]>,
    roots: Option<[Entity; 2]>,
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
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    input: Res<GuiInputState>,
    mut windows: Query<(Entity, &mut Window)>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    mut touches: MessageWriter<TouchInput>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    assert!(
        state.frame < 900,
        "drag recipient smoke timed out at {}/{}",
        state.case,
        state.step
    );
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    let elapsed = state.frame - state.since;
    let touch_id = 90 + u64::from(state.case);
    let mut next = false;
    match state.step {
        0 if state.frame > 25 => {
            let Some(menu) = demo.menu_owner.filter(|&owner| gui.view(owner).is_some()) else {
                return;
            };
            edit.get_mut(menu).unwrap().set_visible(false);
            state.owners = Some([40., 550.].map(|x| {
                commands
                    .spawn(
                        FairyGuiRoot::new(server.load("ui/Basics.fui"), "Demo_Button")
                            .with_position(Vec2::new(x, 40.))
                            .with_size(Vec2::new(450., 400.)),
                    )
                    .id()
            }));
            next = true;
        }
        1 if elapsed > 10
            && state
                .owners
                .unwrap()
                .into_iter()
                .all(|owner| gui.view(owner).is_some()) =>
        {
            let owners = state.owners.unwrap();
            let roots = owners.map(|owner| gui.view(owner).unwrap());
            let button = gui.child_by_name(owners[0], "n13").unwrap();
            for owner in owners {
                for &child in gui.get(owner).unwrap().children() {
                    edit.get_mut(child).unwrap().set_visible(child == button);
                }
            }
            edit.get_mut(button)
                .unwrap()
                .set_position(Vec2::new(40., 80.));
            for entity in [button, roots[0], roots[1]] {
                commands.entity(entity).observe(dragged).observe(clicked);
            }
            commands.entity(roots[1]).observe(moved);
            commands.queue(move |world: &mut World| {
                let objects = roots.map(|entity| world.get::<GObjectRef>(entity).unwrap().0);
                let mut runtime = world.non_send_mut::<GuiRuntime>();
                for object in objects {
                    runtime.store_mut().set_component_opaque(object, true);
                }
            });
            if state.case >= 2 {
                commands.spawn(PointerId::Touch(touch_id));
            }
            state.roots = Some(roots);
            state.button = Some(button);
            next = true;
        }
        2 if elapsed > 10 => {
            state.cursor = Some(
                nodes
                    .get(state.button.unwrap())
                    .unwrap()
                    .1
                    .transform_point2(Vec2::ZERO)
                    / window.scale_factor(),
            );
            next = true;
        }
        3 if elapsed > 4 => {
            seen.drags.clear();
            seen.moves = 0;
            seen.clicks = 0;
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
            let destination = state.roots.unwrap()[1];
            commands.queue(move |world: &mut World| {
                let child = world.get::<GObjectRef>(button).unwrap().0;
                let parent = world.get::<GObjectRef>(destination).unwrap().0;
                let mut runtime = world.non_send_mut::<GuiRuntime>();
                runtime.store_mut().set_parent(child, Some(parent));
                runtime.store_mut().set_xy(child, 40., 80.);
            });
            if state.case == 3 {
                edit.get_mut(destination).unwrap().set_visible(false);
            }
            next = true;
        }
        5 if elapsed > 10 => {
            let root = state.roots.unwrap()[1];
            let (node, transform) = nodes.get(root).unwrap();
            let point = transform.transform_point2(
                node.content_box().min + Vec2::new(300., 300.) * window.scale_factor(),
            ) / window.scale_factor();
            seen.drags.clear();
            seen.moves = 0;
            seen.clicks = 0;
            if state.case == 4 {
                // Ended receives a changed OS coordinate, without a Move sample.
                // Keep the mouse pointer still so it cannot generate motion.
                touches.write(TouchInput {
                    window: window_entity,
                    id: touch_id,
                    phase: TouchPhase::Ended,
                    position: point,
                    force: None,
                });
                next = true;
            } else {
                state.cursor = Some(point);
                if state.case >= 2 {
                    touches.write(TouchInput {
                        window: window_entity,
                        id: touch_id,
                        phase: TouchPhase::Moved,
                        position: point,
                        force: None,
                    });
                }
                next = true;
            }
        }
        6 if elapsed > 8 => {
            let [source, destination] = state.roots.unwrap();
            let button = state.button.unwrap();
            let touch = (state.case >= 2).then_some(touch_id);
            let target = (state.case != 3).then_some(destination);
            let recipients = if state.case == 4 {
                vec![]
            } else if state.case == 1 {
                vec![button]
            } else {
                vec![button, source]
            };
            let expected: Vec<_> = recipients
                .into_iter()
                .map(|entity| Sample {
                    entity,
                    target,
                    touch,
                    stopped: false,
                })
                .collect();
            assert_eq!(seen.drags, expected, "case {}", state.case);
            if state.case < 3 {
                assert!(
                    seen.moves > 0,
                    "stopped MouseMove must still precede independent drag dispatch"
                );
            }
            if state.case == 4 {
                assert_eq!(seen.moves, 0);
            }
            assert_eq!(seen.clicks, 0);
            if state.case < 3 {
                assert_eq!(input.captured(), Some(button));
            }
            println!(
                "drag recipient smoke case {} PASSED: {:?}; stopped moves={}",
                state.case, seen.drags, seen.moves
            );
            if state.case == 2 {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk("target/fairygui-demo-drag-targets.png"));
            }
            if state.case != 4 {
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
            }
            next = true;
        }
        7 if elapsed > 8 => {
            assert_eq!(seen.clicks, 0);
            for owner in state.owners.unwrap() {
                commands.entity(owner).despawn();
            }
            next = true;
        }
        8 if elapsed > 8 => {
            assert!(
                state
                    .owners
                    .unwrap()
                    .into_iter()
                    .all(|owner| gui.view(owner).is_none())
            );
            if state.case == 4 {
                println!(
                    "demo smoke PASSED: saved drag recipients after reparenting, current hit targets, independent move/drag stops, outside-GUI motion and no touch-end movement"
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

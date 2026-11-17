//! Real tree clicks preserve expansion edits made after MouseDown.
use super::*;
use bevy::input::touch::{TouchInput, TouchPhase};

#[derive(Resource, Default)]
struct Seen(Vec<bool>);
fn clicked(event: On<GuiListItemClick>, gui: GuiQuery, mut seen: ResMut<Seen>) {
    let state = gui.tree(event.entity).unwrap();
    assert_eq!(event.index, 0);
    seen.0.push(state.node(0).unwrap().is_expanded());
}
#[derive(Resource, Default)]
struct State {
    frame: u32,
    since: u32,
    step: u8,
    case: u8,
    owner: Option<Entity>,
    tree: Option<Entity>,
    folder: Option<Entity>,
    arrow: Option<Entity>,
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
    mut windows: Query<(Entity, &mut Window)>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    mut touches: MessageWriter<TouchInput>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    assert!(
        state.frame < 700,
        "tree click smoke timed out at {}/{}",
        state.case,
        state.step
    );
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    let elapsed = state.frame - state.since;
    let touch = state.case % 2 == 1;
    let id = 110 + u64::from(state.case);
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
                        FairyGuiRoot::new(server.load("ui/TreeView.fui"), "Main")
                            .with_position(Vec2::new(60., 60.)),
                    )
                    .id(),
            );
            next = true;
        }
        1 if elapsed > 10 => {
            let Some(tree) = gui.child_by_name(state.owner.unwrap(), "tree") else {
                return;
            };
            let Some(tree_state) = gui.tree(tree) else {
                return;
            };
            let folder = tree_state.node(0).unwrap().view().unwrap();
            state.tree = Some(tree);
            state.folder = Some(folder);
            state.arrow = gui.child_by_name(folder, "expandButton");
            commands.entity(tree).observe(clicked);
            next = true;
        }
        2 if elapsed > 8 => {
            edit.get_mut(state.tree.unwrap())
                .unwrap()
                .set_tree_expanded(0, !touch);
            if touch {
                commands.spawn(PointerId::Touch(id));
            }
            seen.0.clear();
            next = true;
        }
        3 if elapsed > 8 => {
            let entity = if state.case == 2 {
                state.arrow.unwrap()
            } else {
                state.folder.unwrap()
            };
            let (node, transform) = nodes.get(entity).unwrap();
            let local = if state.case == 2 {
                Vec2::ZERO
            } else {
                Vec2::new(node.size().x * 0.2, 0.)
            };
            state.cursor = Some(transform.transform_point2(local) / window.scale_factor());
            next = true;
        }
        4 if elapsed > 4 => {
            if touch {
                touches.write(TouchInput {
                    window: window_entity,
                    id,
                    phase: TouchPhase::Started,
                    position: state.cursor.unwrap(),
                    force: None,
                });
            } else {
                mouse.press(MouseButton::Left);
            }
            next = true;
        }
        5 if elapsed > 4 => {
            match state.case {
                0 => edit
                    .get_mut(state.tree.unwrap())
                    .unwrap()
                    .set_tree_expanded(0, false),
                1 => edit
                    .get_mut(state.folder.unwrap())
                    .unwrap()
                    .set_controller_page("expanded", 1),
                2 => {}
                3 => {
                    let mut tree = edit.get_mut(state.tree.unwrap()).unwrap();
                    tree.set_tree_expanded(0, true);
                    tree.set_tree_expanded(0, false);
                }
                _ => unreachable!(),
            }
            next = true;
        }
        6 if elapsed > 7 => {
            if touch {
                touches.write(TouchInput {
                    window: window_entity,
                    id,
                    phase: TouchPhase::Ended,
                    position: state.cursor.unwrap(),
                    force: None,
                });
            } else {
                mouse.release(MouseButton::Left);
            }
            next = true;
        }
        7 if elapsed > 8 => {
            let expanded = gui
                .tree(state.tree.unwrap())
                .unwrap()
                .node(0)
                .unwrap()
                .is_expanded();
            assert_eq!(expanded, touch, "case {}", state.case);
            assert_eq!(seen.0, [touch], "one item click observes final expansion");
            assert_eq!(
                gui.controllers(state.folder.unwrap())
                    .unwrap()
                    .get("expanded")
                    .unwrap()
                    .selected_index(),
                i32::from(touch)
            );
            println!(
                "tree click snapshot smoke case {} PASSED: expanded={expanded}, item clicks={:?}",
                state.case, seen.0
            );
            if state.case == 1 {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk("target/fairygui-demo-tree-click.png"));
            }
            if state.case == 3 {
                next = true;
            } else {
                state.case += 1;
                state.step = 2;
                state.since = state.frame;
            }
        }
        8 if elapsed > 10 => {
            commands.entity(state.owner.unwrap()).despawn();
            next = true;
        }
        9 if elapsed > 8 => {
            assert!(gui.view(state.owner.unwrap()).is_none());
            println!(
                "demo smoke PASSED: tree folder press snapshots, expansion edits between down/release, controller and arrow changes, restored state, typed item observers and mouse/touch input"
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
    if touch
        && state.step >= 3
        && state.step <= 7
        && let Some(position) = state.cursor
    {
        let target =
            bevy::camera::RenderTarget::Window(bevy::window::WindowRef::Entity(window_entity))
                .normalize(Some(window_entity))
                .unwrap();
        pointers.write(PointerInput::new(
            PointerId::Touch(id),
            Location { target, position },
            PointerAction::Move { delta: Vec2::ZERO },
        ));
    }
}

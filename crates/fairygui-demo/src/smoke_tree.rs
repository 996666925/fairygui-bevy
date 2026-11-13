//! Exported tree rendering, actual row/arrow input and entity commands.
use super::*;

#[derive(Resource, Default)]
struct State {
    step: u8,
    frame: u32,
    since: u32,
    owner: Option<Entity>,
    tree: Option<Entity>,
    rows: Vec<Entity>,
    cursor: Option<Vec2>,
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
#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    demo: Res<Demo>,
    gui: GuiQuery,
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
        time.elapsed_secs_f64() < 40.,
        "tree smoke timeout at step {}",
        state.step
    );
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    let elapsed = state.frame - state.since;
    let center =
        |entity| nodes.get(entity).unwrap().1.transform_point2(Vec2::ZERO) / window.scale_factor();
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
            let owner = commands
                .spawn(FairyGuiRoot::new(assets.load("ui/TreeView.fui"), "Main"))
                .id();
            commands.write_message(GuiObjectCommand::new(
                owner,
                GuiObjectAction::SetPosition(Vec2::new(40., 30.)),
            ));
            state.owner = Some(owner);
            next = true;
        }
        1 if elapsed >= 16 => {
            let Some(tree) = gui.child_by_name(state.owner.unwrap(), "tree") else {
                return;
            };
            let Some(tree_state) = gui.tree(tree) else {
                return;
            };
            assert_eq!(tree_state.nodes().len(), 7);
            state.rows = tree_state
                .nodes()
                .iter()
                .map(|node| node.view().unwrap())
                .collect();
            state.tree = Some(tree);
            assert_eq!(gui.text(state.rows[0]), Some("Folder 1"));
            assert_eq!(gui.text(state.rows[1]), Some("Leaf 1"));
            assert_eq!(
                gui.transform(state.rows[1]).unwrap().size().x,
                gui.scroll(tree).unwrap().view_size().x
            );
            assert_eq!(gui.icon(state.rows[1]), Some("ui://5nx1f8vzua5o8"));
            state.cursor = Some(center(state.rows[0]));
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
            assert_eq!(
                gui.tree(state.tree.unwrap()).unwrap().visible_nodes(),
                [0, 5, 6]
            );
            assert_eq!(gui.transform(state.rows[5]).unwrap().position().y, 48.);
            let arrow = gui.child_by_name(state.rows[0], "expandButton").unwrap();
            state.cursor = Some(center(arrow));
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
            assert_eq!(
                gui.tree(state.tree.unwrap()).unwrap().visible_nodes().len(),
                7
            );
            assert_eq!(
                gui.tree(state.tree.unwrap())
                    .unwrap()
                    .node(1)
                    .unwrap()
                    .view(),
                Some(state.rows[1])
            );
            commands.write_message(GuiObjectCommand::new(
                state.tree.unwrap(),
                GuiObjectAction::CollapseTree,
            ));
            next = true;
        }
        8 if elapsed >= 8 => {
            assert_eq!(
                gui.tree(state.tree.unwrap()).unwrap().visible_nodes(),
                [0, 5]
            );
            commands.write_message(GuiObjectCommand::new(
                state.tree.unwrap(),
                GuiObjectAction::SelectTreeNode {
                    index: 3,
                    scroll_to_view: true,
                },
            ));
            next = true;
        }
        9 if elapsed >= 8 => {
            let tree = gui.tree(state.tree.unwrap()).unwrap();
            assert_eq!(tree.selected_nodes(), [3]);
            assert_eq!(tree.visible_nodes(), [0, 1, 2, 3, 4, 5]);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-ecs-tree.png"));
            next = true;
        }
        10 if elapsed >= 8 => {
            commands.entity(state.owner.unwrap()).despawn();
            next = true;
        }
        11 if elapsed >= 8 => {
            assert!(state.rows.iter().all(|&row| !existing.contains(row)));
            println!(
                "demo smoke PASSED: exported tree titles/icons and indentation, actual folder/arrow clicks, collapse reflow, retained rows, hidden-node selection and ECS disposal"
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

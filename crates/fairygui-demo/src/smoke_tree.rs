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
    dynamic: Option<Entity>,
    dynamic_rows: Vec<Entity>,
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
    mut gui_edit: GuiMut,
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
            gui_edit.edit(menu, |object: &mut GuiObjectMut| object.set_visible(false));
            let owner = commands
                .spawn(
                    FairyGuiRoot::new(assets.load("ui/TreeView.fui"), "Main")
                        .with_position(Vec2::new(40., 30.)),
                )
                .id();

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
            gui_edit.edit(state.tree.unwrap(), |object: &mut GuiObjectMut| {
                object.collapse_tree()
            });
            next = true;
        }
        8 if elapsed >= 8 => {
            assert_eq!(
                gui.tree(state.tree.unwrap()).unwrap().visible_nodes(),
                [0, 5]
            );
            gui_edit.edit(state.tree.unwrap(), |object: &mut GuiObjectMut| {
                object.select_tree_node(3, true)
            });
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
            let dynamic = gui.child_by_name(state.owner.unwrap(), "tree2").unwrap();
            let Some(tree) = gui
                .tree(dynamic)
                .filter(|tree| tree.node_by_key("top").is_some())
            else {
                return;
            };
            assert_eq!(tree.nodes().len(), 16);
            assert_eq!(tree.visible_nodes(), [0, 15]);
            state.dynamic = Some(dynamic);
            state.dynamic_rows = tree
                .nodes()
                .iter()
                .map(|node| node.view().unwrap())
                .collect();
            assert_eq!(gui.text(state.dynamic_rows[0]), Some("I'm a top node"));
            assert_eq!(gui.text(state.dynamic_rows[15]), Some("I'm a top node too"));
            state.cursor = Some(center(state.dynamic_rows[0]));
            next = true;
        }
        11 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        12 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        13 if elapsed >= 8 => {
            let tree = gui.tree(state.dynamic.unwrap()).unwrap();
            assert_eq!(tree.visible_nodes().len(), 11);
            let folder = tree.node_by_key("folder").unwrap().view().unwrap();
            state.cursor = Some(center(gui.child_by_name(folder, "expandButton").unwrap()));
            next = true;
        }
        14 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        15 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        16 if elapsed >= 8 => {
            let tree = gui.tree(state.dynamic.unwrap()).unwrap();
            assert_eq!(tree.visible_nodes().len(), 16);
            let file = tree.node_by_key("good-2").unwrap().view().unwrap();
            assert_eq!(gui.text(file), Some("Good 2"));
            assert_eq!(gui.icon(file), Some("ui://TreeView/file"));
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-ecs-tree-content.png"));
            let mut updated = crate::tree_demo::content();
            updated.items[0].children[0].title = "Updated through ECS".into();
            updated.items[0]
                .children
                .push(GuiTreeItem::leaf("new", "Added through ECS"));
            commands.entity(state.dynamic.unwrap()).insert(updated);
            let mut settings = *gui.tree_settings(state.dynamic.unwrap()).unwrap();
            settings.indent = 27;
            commands.entity(state.dynamic.unwrap()).insert(settings);
            next = true;
        }
        17 if elapsed >= 8 => {
            let tree = gui.tree(state.dynamic.unwrap()).unwrap();
            assert_eq!(tree.nodes().len(), 17);
            assert!(tree.node_by_key("folder").unwrap().is_expanded());
            assert_eq!(
                tree.node_by_key("hello-0").unwrap().view(),
                Some(state.dynamic_rows[1])
            );
            assert_eq!(gui.text(state.dynamic_rows[1]), Some("Updated through ECS"));
            let new = tree.node_by_key("new").unwrap().view().unwrap();
            let old_indent = gui.child_by_name(state.dynamic_rows[1], "indent").unwrap();
            let new_indent = gui.child_by_name(new, "indent").unwrap();
            assert_eq!(gui.transform(old_indent).unwrap().size.x, 15.);
            assert_eq!(gui.transform(new_indent).unwrap().size.x, 27.);
            assert!(
                (nodes.get(new_indent).unwrap().0.size().x
                    / nodes.get(old_indent).unwrap().0.size().x
                    - 1.8)
                    .abs()
                    < 0.01
            );
            state.dynamic_rows.push(new);
            commands
                .entity(state.dynamic.unwrap())
                .insert(GuiTreeContent::default());
            next = true;
        }
        18 if elapsed >= 8 => {
            assert!(gui.tree(state.dynamic.unwrap()).unwrap().nodes().is_empty());
            assert!(
                state
                    .dynamic_rows
                    .iter()
                    .all(|&row| !existing.contains(row))
            );
            commands.entity(state.owner.unwrap()).despawn();
            next = true;
        }
        19 if elapsed >= 8 => {
            assert!(state.rows.iter().all(|&row| !existing.contains(row)));
            println!(
                "demo smoke PASSED: exported and keyed dynamic trees, actual row/arrow clicks, native collapse defaults, folder/file icons, keyed row reuse, live ECS indent with retained/new cells, data updates, growth and disposal"
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

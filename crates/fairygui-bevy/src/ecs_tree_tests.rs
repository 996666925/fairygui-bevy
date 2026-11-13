use super::*;
use crate::core::{package::FuiPackage, widget::ControlData};
use crate::{
    FairyGuiPlugin, FairyGuiRoot, FuiAsset, GuiControllers, GuiDisplay, GuiObjectAction,
    GuiObjectCommand, GuiQuery, GuiRootInstance, GuiText, GuiTransform,
};
use bevy::ecs::system::SystemState;
use bevy::picking::hover::Hovered;
use bevy::ui::{ComputedStackIndex, RelativeCursorPosition};
use bevy::window::PrimaryWindow;

fn fixture(loaded: bool) -> (App, Handle<FuiAsset>, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let handle = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
    if loaded {
        load(&mut app, &handle);
    }
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(handle.clone(), "Main"))
        .id();
    (app, handle, owner)
}
fn load(app: &mut App, handle: &Handle<FuiAsset>) {
    let package = FuiPackage::parse(
        "TreeView",
        include_bytes!("../../fairygui-demo/assets/ui/TreeView.fui"),
    )
    .unwrap();
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(
            handle.id(),
            FuiAsset {
                package,
                res_key: "ui/TreeView".into(),
            },
        )
        .unwrap();
}
fn tree(app: &mut App, owner: Entity) -> Entity {
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    query
        .get(app.world())
        .unwrap()
        .child_by_name(owner, "tree")
        .unwrap()
}
fn send(app: &mut App, target: Entity, action: GuiObjectAction) {
    app.world_mut()
        .write_message(GuiObjectCommand::new(target, action));
}

#[test]
fn exported_tree_materializes_titles_icons_folder_controls_hierarchy_and_stable_snapshots() {
    let (mut app, _, owner) = fixture(true);
    app.update();
    app.update();
    let tree = tree(&mut app, owner);
    let state = app.world().get::<GuiTreeState>(tree).unwrap();
    assert_eq!(state.nodes().len(), 7);
    assert_eq!(state.visible_nodes(), [0, 1, 2, 3, 4, 5, 6]);
    let titles = [
        "Folder 1", "Leaf 1", "Leaf 2", "Leaf 3", "Leaf 4", "Folder 2", "Leaf 1",
    ];
    let rows: Vec<_> = state
        .nodes()
        .iter()
        .map(|node| node.view().unwrap())
        .collect();
    let view_width = app
        .world()
        .get::<crate::GuiScrollState>(tree)
        .unwrap()
        .view_size()
        .x;
    for (index, &row) in rows.iter().enumerate() {
        assert_eq!(
            app.world().get::<GuiText>(row).unwrap().value(),
            titles[index]
        );
        assert_eq!(
            app.world().get::<GuiTransform>(row).unwrap().size().x,
            view_width
        );
        assert_eq!(app.world().get::<GuiTreeNode>(row).unwrap().index(), index);
        assert_eq!(app.world().get::<GuiTreeNode>(row).unwrap().tree(), tree);
        let controls = app.world().get::<GuiControllers>(row).unwrap();
        let folder = index == 0 || index == 5;
        assert_eq!(
            controls.get("leaf").unwrap().selected_index(),
            i32::from(!folder)
        );
        let state = app.world().get::<GuiTreeState>(tree).unwrap();
        assert_eq!(state.node(index).unwrap().is_folder(), folder);
        assert_eq!(
            state.node(index).unwrap().parent(),
            if folder {
                None
            } else if index == 6 {
                Some(5)
            } else {
                Some(0)
            }
        );
        let runtime = app.world().non_send::<GuiRuntime>();
        let id = app.world().get::<GObjectRef>(row).unwrap().0;
        let indent = runtime.store().child_by_name(id, "indent").unwrap();
        assert_eq!(
            runtime.store().object(indent).unwrap().width(),
            if folder { 0. } else { 15. }
        );
        assert_eq!(
            app.world().get::<GuiTransform>(row).unwrap().position().x,
            0.
        );
    }
    let leaf_icon = app.world().get::<crate::GuiIcon>(rows[1]).unwrap().value();
    assert_eq!(leaf_icon, "ui://5nx1f8vzua5o8");
    let tick = app
        .world()
        .entity(tree)
        .get_ref::<GuiTreeState>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .entity(tree)
            .get_ref::<GuiTreeState>()
            .unwrap()
            .last_changed(),
        tick
    );
    app.world_mut().despawn(owner);
    app.update();
    assert!(rows.iter().all(|&row| app.world().get_entity(row).is_err()));
}

#[test]
fn ecs_tree_commands_collapse_top_level_folders_reflow_retain_views_and_select_hidden_nodes() {
    let (mut app, _, owner) = fixture(true);
    app.update();
    let tree = tree(&mut app, owner);
    let rows: Vec<_> = app
        .world()
        .get::<GuiTreeState>(tree)
        .unwrap()
        .nodes()
        .iter()
        .map(|n| n.view().unwrap())
        .collect();
    send(&mut app, tree, GuiObjectAction::CollapseTree);
    app.update();
    let state = app.world().get::<GuiTreeState>(tree).unwrap();
    assert_eq!(state.visible_nodes(), [0, 5]);
    assert!(!state.node(0).unwrap().is_expanded());
    assert!(!state.node(5).unwrap().is_expanded());
    assert_eq!(
        app.world()
            .get::<GuiTransform>(rows[5])
            .unwrap()
            .position()
            .y,
        48.
    );
    assert!(!app.world().get::<GuiDisplay>(rows[1]).unwrap().visible());
    assert_eq!(state.node(1).unwrap().view(), Some(rows[1]));
    send(
        &mut app,
        tree,
        GuiObjectAction::SelectTreeNode {
            index: 3,
            scroll_to_view: true,
        },
    );
    app.update();
    let state = app.world().get::<GuiTreeState>(tree).unwrap();
    assert_eq!(state.visible_nodes(), [0, 1, 2, 3, 4, 5]);
    assert_eq!(state.selected_nodes(), [3]);
    send(&mut app, tree, GuiObjectAction::ToggleTreeNode(0));
    app.update();
    assert_eq!(
        app.world()
            .get::<GuiTreeState>(tree)
            .unwrap()
            .visible_nodes(),
        [0, 5]
    );
    assert!(
        app.world()
            .get::<GuiTreeState>(tree)
            .unwrap()
            .selected_nodes()
            .is_empty()
    );
    send(&mut app, tree, GuiObjectAction::ExpandTree);
    app.update();
    assert_eq!(
        app.world()
            .get::<GuiTreeState>(tree)
            .unwrap()
            .visible_nodes()
            .len(),
        7
    );
    assert!(
        app.world()
            .get::<GuiTreeState>(tree)
            .unwrap()
            .selected_nodes()
            .is_empty()
    );
    send(
        &mut app,
        tree,
        GuiObjectAction::SelectTreeNode {
            index: 3,
            scroll_to_view: false,
        },
    );
    app.update();
    let tick = app
        .world()
        .entity(tree)
        .get_ref::<GuiTreeState>()
        .unwrap()
        .last_changed();
    send(
        &mut app,
        tree,
        GuiObjectAction::SetTreeExpanded {
            index: 1,
            expanded: false,
        },
    );
    send(&mut app, tree, GuiObjectAction::ToggleTreeNode(500));
    send(
        &mut app,
        tree,
        GuiObjectAction::SelectTreeNode {
            index: 500,
            scroll_to_view: true,
        },
    );
    app.update();
    assert_eq!(
        app.world()
            .entity(tree)
            .get_ref::<GuiTreeState>()
            .unwrap()
            .last_changed(),
        tick
    );
    assert_eq!(
        app.world()
            .get::<GuiTreeState>(tree)
            .unwrap()
            .selected_nodes(),
        [3]
    );
}

#[test]
fn tree_commands_ignore_unrelated_pending_roots_and_expander_controller_updates_native_state() {
    let (mut app, handle, owner) = fixture(false);
    // Unrelated root controls safely ignore retained tree actions after loading.
    send(&mut app, owner, GuiObjectAction::CollapseTree);
    for _ in 0..4 {
        app.update();
    }
    load(&mut app, &handle);
    app.update();
    let tree = tree(&mut app, owner);
    let folder = app
        .world()
        .get::<GuiTreeState>(tree)
        .unwrap()
        .node(0)
        .unwrap()
        .view()
        .unwrap();
    send(
        &mut app,
        folder,
        GuiObjectAction::SetControllerPage {
            name: "expanded".into(),
            index: 0,
        },
    );
    app.update();
    assert_eq!(
        app.world()
            .get::<GuiTreeState>(tree)
            .unwrap()
            .visible_nodes(),
        [0, 5, 6]
    );
    send(
        &mut app,
        folder,
        GuiObjectAction::SetControllerPage {
            name: "expanded".into(),
            index: 1,
        },
    );
    app.update();
    assert_eq!(
        app.world()
            .get::<GuiTreeState>(tree)
            .unwrap()
            .visible_nodes()
            .len(),
        7
    );
    let root = app.world().get::<GuiRootInstance>(owner).unwrap().object;
    let runtime = app.world().non_send::<GuiRuntime>();
    assert!(matches!(
        runtime.store().object(root).unwrap().control(),
        ControlData::Component(_)
    ));
}

fn hover(app: &mut App, target: Entity) {
    let entities: Vec<_> = app
        .world_mut()
        .query::<(Entity, &GObjectRef)>()
        .iter(app.world())
        .map(|(e, _)| e)
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
}
fn click(app: &mut App, target: Entity) {
    hover(app, target);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
}

#[derive(Resource, Default)]
struct ClickSnapshots(Vec<bool>);
#[derive(Resource, Default)]
struct ItemClicks(Vec<(usize, String)>);
#[derive(Resource, Default)]
struct TypedItemClicks(Vec<(Entity, Entity, usize, String)>);
fn observe_typed_item_click(
    event: On<crate::GuiListItemClick>,
    gui: GuiQuery,
    mut clicks: ResMut<TypedItemClicks>,
) {
    clicks.0.push((
        event.entity,
        event.item,
        event.index,
        gui.text(event.item).unwrap().to_owned(),
    ));
}
fn observe_item_click(event: On<crate::GuiEvent>, gui: GuiQuery, mut clicks: ResMut<ItemClicks>) {
    if event.event.kind == crate::core::object::EventKind::ClickItem {
        let tree = gui.tree(event.entity).unwrap();
        let index = tree.selected_nodes()[0];
        clicks.0.push((
            index,
            gui.text(tree.node(index).unwrap().view().unwrap())
                .unwrap()
                .to_owned(),
        ));
    }
}
fn observe_folder_click(
    event: On<crate::GuiEvent>,
    nodes: Query<&GuiTreeNode>,
    trees: Query<&GuiTreeState>,
    mut snapshots: ResMut<ClickSnapshots>,
) {
    if event.event.kind == crate::core::object::EventKind::Click {
        let node = nodes.get(event.entity).unwrap();
        snapshots.0.push(
            trees
                .get(node.tree())
                .unwrap()
                .node(node.index())
                .unwrap()
                .is_expanded(),
        );
    }
}
#[test]
fn tree_body_and_expander_clicks_each_toggle_once_and_publish_state_before_observers() {
    let (mut app, _, owner) = fixture(true);
    let mut window = Window {
        focused: true,
        ..default()
    };
    window.set_cursor_position(Some(Vec2::splat(10.)));
    app.world_mut().spawn((window, PrimaryWindow));
    app.update();
    let tree = tree(&mut app, owner);
    let folder = app
        .world()
        .get::<GuiTreeState>(tree)
        .unwrap()
        .node(0)
        .unwrap()
        .view()
        .unwrap();
    app.init_resource::<ClickSnapshots>();
    app.init_resource::<ItemClicks>();
    app.init_resource::<TypedItemClicks>();
    app.world_mut().entity_mut(tree).observe(observe_item_click);
    app.world_mut()
        .entity_mut(tree)
        .observe(observe_typed_item_click);
    app.world_mut()
        .entity_mut(folder)
        .observe(observe_folder_click);
    click(&mut app, folder);
    assert!(
        !app.world()
            .get::<GuiTreeState>(tree)
            .unwrap()
            .node(0)
            .unwrap()
            .is_expanded()
    );
    let runtime = app.world().non_send::<GuiRuntime>();
    let id = app.world().get::<GObjectRef>(folder).unwrap().0;
    let arrow = runtime.store().child_by_name(id, "expandButton").unwrap();
    let arrow = runtime.entity_of(arrow).unwrap();
    click(&mut app, arrow);
    assert!(
        app.world()
            .get::<GuiTreeState>(tree)
            .unwrap()
            .node(0)
            .unwrap()
            .is_expanded()
    );
    click(&mut app, arrow);
    assert!(
        !app.world()
            .get::<GuiTreeState>(tree)
            .unwrap()
            .node(0)
            .unwrap()
            .is_expanded()
    );
    assert_eq!(
        app.world().resource::<ClickSnapshots>().0,
        [false, true, false]
    );
    assert_eq!(
        app.world().resource::<ItemClicks>().0,
        vec![(0, "Folder 1".into()); 3]
    );
    assert_eq!(
        app.world().resource::<TypedItemClicks>().0,
        vec![(tree, folder, 0, "Folder 1".into()); 3]
    );
}

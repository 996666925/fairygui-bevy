use super::*;
use crate::core::package::FuiPackage;
use crate::{
    FairyGuiPlugin, FairyGuiRoot, FuiAsset, GuiQuery, GuiRootInstance, GuiText, GuiWorldExt,
};
use bevy::ecs::system::SystemState;

fn fixture() -> (App, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let package = FuiPackage::parse(
        "TreeView",
        include_bytes!("../../fairygui-demo/assets/ui/TreeView.fui"),
    )
    .unwrap();
    let handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package,
            res_key: "ui/TreeView".into(),
        });
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(handle, "Main"))
        .id();
    app.update();
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    let tree = query
        .get(app.world())
        .unwrap()
        .child_by_name(owner, "tree2")
        .unwrap();
    (app, owner, tree)
}
fn config() -> GuiTreeContent {
    let mut file = GuiTreeItem::leaf("file", "A file");
    file.icon = Some("ui://TreeView/file".into());
    GuiTreeContent::new(vec![
        GuiTreeItem::folder("folder", "A folder", vec![file]),
        GuiTreeItem::leaf("other", "Other"),
    ])
}
fn view(app: &App, tree: Entity, key: &str) -> Entity {
    app.world()
        .get::<GuiTreeState>(tree)
        .unwrap()
        .node_by_key(key)
        .unwrap()
        .view()
        .unwrap()
}
fn send(app: &mut App, target: Entity, action: impl FnOnce(&mut crate::GuiObjectMut)) {
    action(
        &mut app
            .world_mut()
            .gui_mut(target)
            .expect("GUI object must be ready"),
    );
}

#[test]
fn keyed_tree_reconciles_hierarchy_preserves_rows_selection_expansion_and_reclaims_removed_branches()
 {
    let (mut app, owner, tree) = fixture();
    app.world_mut().entity_mut(tree).insert(config());
    app.update();
    assert!(
        app.world()
            .get::<GuiTreeContentState>(tree)
            .unwrap()
            .error()
            .is_none()
    );
    let folder = view(&app, tree, "folder");
    let file = view(&app, tree, "file");
    let other = view(&app, tree, "other");
    assert_eq!(
        app.world()
            .get::<GuiTreeState>(tree)
            .unwrap()
            .visible_nodes(),
        [0, 2]
    );
    send(&mut app, tree, |object: &mut crate::GuiObjectMut| {
        object.select_tree_node(1, false)
    });
    app.update();
    assert!(
        app.world()
            .get::<GuiTreeState>(tree)
            .unwrap()
            .node(0)
            .unwrap()
            .is_expanded()
    );
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
    let mut next = config();
    next.items.swap(0, 1);
    next.items[1].children[0].title = "Renamed".into();
    app.world_mut().entity_mut(tree).insert(next);
    app.update();
    assert_eq!(view(&app, tree, "folder"), folder);
    assert_eq!(view(&app, tree, "file"), file);
    assert_eq!(view(&app, tree, "other"), other);
    let state = app.world().get::<GuiTreeState>(tree).unwrap();
    assert_eq!(state.selected_nodes(), [2]);
    assert!(state.node_by_key("folder").unwrap().is_expanded());
    assert_eq!(state.node_by_key("file").unwrap().parent(), Some(1));
    assert_eq!(app.world().get::<GuiText>(file).unwrap().value(), "Renamed");
    app.world_mut()
        .entity_mut(tree)
        .insert(GuiTreeContent::new(vec![GuiTreeItem::leaf(
            "other",
            "Still here",
        )]));
    app.update();
    assert_eq!(view(&app, tree, "other"), other);
    assert!(app.world().get_entity(file).is_err() && app.world().get_entity(folder).is_err());
    app.world_mut().despawn(owner);
    app.update();
    assert!(app.world().get_entity(other).is_err());
}

#[test]
fn keyed_tree_configuration_errors_leave_the_previous_tree_and_allow_retry() {
    let (mut app, _, tree) = fixture();
    app.world_mut().entity_mut(tree).insert(config());
    app.update();
    let old = app.world().get::<GuiTreeState>(tree).unwrap().clone();
    let folder = view(&app, tree, "folder");
    let objects_before = app.world().non_send::<GuiRuntime>().store().len();
    for error in 0..5 {
        let mut next = config();
        match error {
            0 => {
                next.items[1].key = "folder".into();
            }
            1 => {
                next.items[1].template = Some("Missing".into());
            }
            2 => {
                next.items[1].children = vec![GuiTreeItem::leaf("bad", "Invalid")];
            }
            3 => {
                next.items[1].key.clear();
            }
            _ => {
                next.items.insert(
                    0,
                    GuiTreeItem::leaf("staged", "Prepared before invalid template"),
                );
                next.items.last_mut().unwrap().template = Some("ui://TreeView/file".into());
            }
        }
        app.world_mut().entity_mut(tree).insert(next);
        app.update();
        assert!(
            app.world()
                .get::<GuiTreeContentState>(tree)
                .unwrap()
                .error()
                .is_some()
        );
        assert_eq!(app.world().get::<GuiTreeState>(tree).unwrap(), &old);
        assert_eq!(view(&app, tree, "folder"), folder);
        assert_eq!(
            app.world().non_send::<GuiRuntime>().store().len(),
            objects_before
        );
    }
    app.world_mut().entity_mut(tree).insert(config());
    app.update();
    assert!(
        app.world()
            .get::<GuiTreeContentState>(tree)
            .unwrap()
            .error()
            .is_none()
    );
    assert_eq!(view(&app, tree, "folder"), folder);
}

#[test]
fn keyed_tree_component_removal_retains_keys_and_views_reinsertion_reuses_and_empty_disposes_content()
 {
    let (mut app, _, tree) = fixture();
    app.world_mut().entity_mut(tree).insert(config());
    app.update();
    let folder = view(&app, tree, "folder");
    app.world_mut().entity_mut(tree).remove::<GuiTreeContent>();
    app.update();
    assert!(app.world().get::<GuiTreeContentState>(tree).is_none());
    assert_eq!(view(&app, tree, "folder"), folder);
    send(&mut app, tree, |object: &mut crate::GuiObjectMut| {
        object.set_tree_expanded(0, true)
    });
    app.update();
    app.world_mut().entity_mut(tree).insert(config());
    app.update();
    assert_eq!(view(&app, tree, "folder"), folder);
    assert!(
        app.world()
            .get::<GuiTreeState>(tree)
            .unwrap()
            .node(0)
            .unwrap()
            .is_expanded()
    );
    let mut forced = config();
    forced.items[0].expanded = Some(false);
    app.world_mut().entity_mut(tree).insert(forced);
    app.update();
    assert_eq!(
        app.world()
            .get::<GuiTreeState>(tree)
            .unwrap()
            .visible_nodes(),
        [0, 2]
    );
    app.world_mut()
        .entity_mut(tree)
        .insert(GuiTreeContent::default());
    app.update();
    assert!(
        app.world()
            .get::<GuiTreeState>(tree)
            .unwrap()
            .nodes()
            .is_empty()
    );
    assert!(app.world().get_entity(folder).is_err());
}

#[test]
fn keyed_tree_rejects_non_tree_and_excessive_depth_and_replaces_changed_node_kind() {
    let (mut app, owner, tree) = fixture();
    let root = app.world().get::<GuiRootInstance>(owner).unwrap().object;
    let root = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(root)
        .unwrap();
    app.world_mut().entity_mut(root).insert(config());
    app.update();
    assert_eq!(
        app.world()
            .get::<GuiTreeContentState>(root)
            .unwrap()
            .error(),
        Some(&GuiTreeContentError::NotTree)
    );
    let mut node = GuiTreeItem::leaf("deep", "Deep");
    for index in 0..256 {
        node = GuiTreeItem::folder(index.to_string(), "Nested", vec![node]);
    }
    app.world_mut()
        .entity_mut(tree)
        .insert(GuiTreeContent::new(vec![node]));
    app.update();
    assert_eq!(
        app.world()
            .get::<GuiTreeContentState>(tree)
            .unwrap()
            .error(),
        Some(&GuiTreeContentError::TooDeep)
    );
    app.world_mut().entity_mut(tree).insert(config());
    app.update();
    let old = view(&app, tree, "other");
    let mut next = config();
    next.items[1] = GuiTreeItem::folder("other", "Changed", vec![]);
    app.world_mut().entity_mut(tree).insert(next);
    app.update();
    assert_ne!(view(&app, tree, "other"), old);
    assert!(app.world().get_entity(old).is_err());
}

//! Dynamic TreeView example configured through keyed ECS content.
use bevy::prelude::*;
use fairygui_bevy::prelude::*;

#[derive(Component)]
pub(crate) struct Bound;

pub(crate) fn content() -> GuiTreeContent {
    let file = |key: String, title: String| {
        let mut node = GuiTreeItem::leaf(key, title);
        node.icon = Some("ui://TreeView/file".into());
        node
    };
    let mut children: Vec<_> = (0..5)
        .map(|index| file(format!("hello-{index}"), format!("Hello {index}")))
        .collect();
    children.push(GuiTreeItem::folder(
        "folder",
        "A folder node",
        (0..5)
            .map(|index| file(format!("good-{index}"), format!("Good {index}")))
            .collect(),
    ));
    children.extend((0..3).map(|index| file(format!("world-{index}"), format!("World {index}"))));
    let mut heart = GuiTreeItem::leaf("heart", "I'm a top node too");
    heart.icon = Some("ui://TreeView/heart".into());
    GuiTreeContent::new(vec![
        GuiTreeItem::folder("top", "I'm a top node", children),
        heart,
    ])
}

pub fn bind(
    mut commands: Commands,
    pages: Query<(Entity, &FairyGuiRoot), Without<Bound>>,
    gui: GuiQuery,
) {
    for (owner, root) in &pages {
        if root.item_name != "Main"
            || !root
                .asset
                .path()
                .is_some_and(|path| path.path().ends_with("TreeView.fui"))
        {
            continue;
        }
        let Some(tree) = gui.child_by_name(owner, "tree") else {
            continue;
        };
        let Some(dynamic) = gui.child_by_name(owner, "tree2") else {
            continue;
        };
        commands.entity(dynamic).insert(content());
        commands.entity(tree).observe(click_node);
        commands.entity(dynamic).observe(click_node);
        commands.entity(owner).insert(Bound);
    }
}
fn click_node(event: On<GuiEvent>, gui: GuiQuery) {
    if event.event.kind != fairygui_bevy::core::object::EventKind::ClickItem {
        return;
    }
    let Some(tree) = gui.tree(event.entity) else {
        return;
    };
    if let Some(index) = tree.selected_nodes().first()
        && let Some(row) = tree.node(*index).and_then(|node| node.view())
    {
        info!("tree node: {}", gui.text(row).unwrap_or_default());
    }
}

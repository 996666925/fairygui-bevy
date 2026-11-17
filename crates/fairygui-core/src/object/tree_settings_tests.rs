use super::*;
use crate::{package::ObjectType, widget::TreeNodeData};

fn cell(store: &mut ObjectStore, tree: GObjectId) -> (GObjectId, GObjectId) {
    let row = store.create(ObjectType::Component);
    store.set_size(row, 100., 20.);
    store.set_parent(row, Some(tree));
    let part = store.create(ObjectType::Graph);
    store.set_name(part, "indent");
    store.set_parent(part, Some(row));
    (row, part)
}

#[test]
fn tree_settings_match_native_assignment_cell_creation_and_click_policy() {
    let (mut indents, mut clicks) = (0, 0);
    for line in include_str!("../../tests/fixtures/tree-settings-reference.txt")
        .lines()
        .filter(|line| !line.starts_with('#'))
    {
        let mut values = line.split_whitespace();
        let tag = values.next().unwrap();
        let v: Vec<i32> = values.map(|s| s.parse().unwrap()).collect();
        let mut store = ObjectStore::new();
        let tree = store.create(ObjectType::Tree);
        store.set_size(tree, 100., 200.);
        if tag == "indent" {
            let level = v[2] as u8;
            store.set_tree_settings(
                tree,
                TreeSettings {
                    indent: v[0],
                    click_to_expand: 0,
                },
            );
            let mut parts = Vec::new();
            for _ in 0..=level {
                parts.push(cell(&mut store, tree).1);
            }
            let ControlData::Tree(data) = &mut store.objects[tree].control else {
                unreachable!()
            };
            data.nodes = (0..=level)
                .map(|level| TreeNodeData {
                    level,
                    is_folder: true,
                    ..Default::default()
                })
                .collect();
            store.initialize_tree_cells(tree);
            let retained = parts[level as usize];
            store.set_tree_settings(
                tree,
                TreeSettings {
                    indent: v[1],
                    click_to_expand: 0,
                },
            );
            store.layout_list(tree);
            assert_eq!(
                store.object(retained).unwrap().width(),
                v[3] as f32,
                "{line}"
            );
            let fresh = cell(&mut store, tree).1;
            let ControlData::Tree(data) = &mut store.objects[tree].control else {
                unreachable!()
            };
            data.nodes.push(TreeNodeData {
                level,
                is_folder: false,
                ..Default::default()
            });
            store.initialize_tree_cells(tree);
            assert_eq!(store.object(fresh).unwrap().width(), v[4] as f32, "{line}");
            assert_eq!(
                store.object(retained).unwrap().width(),
                v[3] as f32,
                "{line}"
            );
            indents += 1;
        } else {
            let row = cell(&mut store, tree).0;
            let ControlData::Tree(data) = &mut store.objects[tree].control else {
                unreachable!()
            };
            data.nodes.push(TreeNodeData {
                is_folder: v[1] != 0,
                ..Default::default()
            });
            data.expanded.push(v[2] != 0);
            store.set_tree_settings(
                tree,
                TreeSettings {
                    indent: 15,
                    click_to_expand: v[0],
                },
            );
            store.record_tree_pointer_down(row, None);
            store.toggle_clicked_tree_node(row);
            let ControlData::Tree(data) = store.object(tree).unwrap().control() else {
                unreachable!()
            };
            // Leaves expose true in this runtime; compare the native backing
            // flag for inert leaf clicks independently of that API convention.
            assert_eq!(data.expanded[0], v[3] != 0, "{line}");
            clicks += 1;
        }
    }
    assert_eq!((indents, clicks), (27, 20));
}

#[test]
fn tree_visible_indices_map_to_stable_nodes_and_skip_collapsed_descendants() {
    let mut store = ObjectStore::new();
    let tree = store.create(ObjectType::Tree);
    let ControlData::Tree(data) = &mut store.objects[tree].control else {
        unreachable!()
    };
    data.nodes = vec![
        TreeNodeData {
            is_folder: true,
            level: 0,
            ..Default::default()
        },
        TreeNodeData {
            level: 1,
            ..Default::default()
        },
        TreeNodeData {
            is_folder: true,
            level: 0,
            ..Default::default()
        },
        TreeNodeData {
            level: 1,
            ..Default::default()
        },
    ];
    data.expanded = vec![false, true, true, true];
    for _ in 0..4 {
        let row = store.create(ObjectType::Component);
        store.set_parent(row, Some(tree));
    }

    assert_eq!(store.tree_node_at_visible_index(tree, 0), Some(0));
    assert_eq!(store.tree_node_at_visible_index(tree, 1), Some(2));
    assert_eq!(store.tree_node_at_visible_index(tree, 2), Some(3));
    assert_eq!(store.tree_node_at_visible_index(tree, 3), None);
    assert_eq!(store.tree_visible_index_of_node(tree, 0), Some(0));
    assert_eq!(store.tree_visible_index_of_node(tree, 1), None);
    assert_eq!(store.tree_visible_index_of_node(tree, 2), Some(1));
    assert_eq!(store.tree_visible_index_of_node(tree, 3), Some(2));
    assert_eq!(store.list_child_index_to_item_index(tree, 1), None);
    assert_eq!(store.list_child_index_to_item_index(tree, 2), Some(1));
    assert_eq!(store.list_item_index_to_child_index(tree, 1), Some(2));
    assert_eq!(store.list_item_index_to_child_index(tree, 2), Some(3));
}

#[test]
fn tree_selection_uses_visible_indices_and_shift_ranges_skip_collapsed_rows() {
    let mut store = ObjectStore::new();
    let tree = store.create(ObjectType::Tree);
    let ControlData::Tree(data) = &mut store.objects[tree].control else {
        unreachable!()
    };
    data.nodes = vec![
        TreeNodeData {
            is_folder: true,
            level: 0,
            ..Default::default()
        },
        TreeNodeData {
            level: 1,
            ..Default::default()
        },
        TreeNodeData {
            is_folder: true,
            level: 0,
            ..Default::default()
        },
        TreeNodeData {
            level: 1,
            ..Default::default()
        },
    ];
    data.expanded = vec![false, true, true, true];
    data.list.selection_mode = crate::widget::ListSelectionMode::Multiple;
    let rows: Vec<_> = (0..4)
        .map(|_| {
            let row = store.create(ObjectType::Button);
            store.object_mut(row).unwrap().control =
                ControlData::Button(Box::new(crate::widget::ButtonData {
                    mode: crate::widget::ButtonMode::Check,
                    ..Default::default()
                }));
            store.set_parent(row, Some(tree));
            row
        })
        .collect();
    store.initialize_tree_cells(tree);

    assert_eq!(store.list_num_items(tree), 3);
    assert!(store.add_list_selection(tree, 1));
    assert_eq!(store.list_selections(tree), Some(vec![1]));
    assert_eq!(store.button_selected(rows[2]), Some(true));
    assert_eq!(store.button_selected(rows[1]), Some(false));

    assert!(store.select_clicked_item_with_modifiers(rows[3], true, false));
    assert_eq!(store.list_selections(tree), Some(vec![1, 2]));
    assert_eq!(store.button_selected(rows[2]), Some(true));
    assert_eq!(store.button_selected(rows[3]), Some(true));
    assert_eq!(store.button_selected(rows[1]), Some(false));

    assert!(store.set_tree_node_expanded(tree, 2, false));
    assert_eq!(store.list_num_items(tree), 2);
    assert_eq!(store.list_selections(tree), Some(vec![1]));
    assert_eq!(store.button_selected(rows[3]), Some(false));
}

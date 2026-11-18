use super::*;
use crate::{
    package::ObjectType,
    widget::{Controller, ListItemData, TreeNodeData},
};

const NODES: [(u8, bool); 13] = [
    (0, true),
    (1, false),
    (1, true),
    (2, true),
    (3, false),
    (2, false),
    (1, true),
    (1, false),
    (0, true),
    (1, true),
    (2, false),
    (1, false),
    (0, false),
];

fn fixture(mask: u32) -> (ObjectStore, GObjectId, Vec<GObjectId>) {
    let mut store = ObjectStore::new();
    let tree = store.create(ObjectType::Tree);
    store.set_size(tree, 120., 300.);
    let rows = NODES
        .iter()
        .map(|_| {
            let row = store.create(ObjectType::Component);
            store.set_size(row, 100., 20.);
            store.set_parent(row, Some(tree));
            store.objects[row]
                .control
                .component_mut()
                .unwrap()
                .controllers
                .push(Controller {
                    name: "expanded".into(),
                    page_ids: vec!["closed".into(), "open".into()],
                    page_names: vec!["closed".into(), "open".into()],
                    ..Default::default()
                });
            row
        })
        .collect();
    let mut bit = 0;
    let expanded = NODES
        .iter()
        .map(|(_, folder)| {
            if *folder {
                let expanded = mask & (1 << bit) != 0;
                bit += 1;
                expanded
            } else {
                true
            }
        })
        .collect();
    assert!(
        store.replace_tree_nodes(
            tree,
            NODES
                .iter()
                .map(|(level, is_folder)| TreeNodeData {
                    level: *level,
                    is_folder: *is_folder,
                    ..Default::default()
                })
                .collect(),
            vec![ListItemData::default(); NODES.len()],
            expanded,
        )
    );
    store.initialize_tree_cells(tree);
    (store, tree, rows)
}

fn flags(store: &ObjectStore, tree: GObjectId) -> Vec<bool> {
    let ControlData::Tree(data) = store.object(tree).unwrap().control() else {
        unreachable!()
    };
    (0..data.nodes.len())
        .map(|index| data.is_expanded(index))
        .collect()
}

#[test]
fn tree_branch_operations_match_64_native_scenarios_and_960_snapshots() {
    let mut current = None;
    let (mut cases, mut snapshots) = (0, 0);
    for line in include_str!("../../tests/fixtures/tree-branch-reference.txt").lines() {
        let values: Vec<_> = line.split_whitespace().collect();
        if values.is_empty() || values[0].starts_with('#') {
            continue;
        }
        if values[0] == "case" {
            current = Some(fixture(values[1].parse().unwrap()));
            cases += 1;
            continue;
        }
        let (store, tree, rows) = current.as_mut().unwrap();
        let tree = *tree;
        let index: i32 = values[1].parse().unwrap();
        let expanded = values[2] == "1";
        let changed = if index < 0 {
            let before = flags(store, tree);
            if expanded {
                store.expand_tree(tree);
            } else {
                store.collapse_tree(tree);
            }
            before != flags(store, tree)
        } else {
            store.set_tree_branch_expanded(tree, index as usize, expanded)
        };
        assert_eq!(changed, values[3] == "1", "case {}: {line}", cases - 1);
        let expected: Vec<_> = values[4].split(',').map(|value| value == "1").collect();
        assert_eq!(flags(store, tree), expected, "case {}: {line}", cases - 1);
        let visible: Vec<usize> = values[5]
            .split(',')
            .map(|value| value.parse().unwrap())
            .collect();
        let ControlData::Tree(data) = store.object(tree).unwrap().control() else {
            unreachable!()
        };
        assert_eq!(data.visible_nodes(), visible, "case {}: {line}", cases - 1);
        for (index, &row) in rows.iter().enumerate() {
            let object = store.object(row).unwrap();
            assert_eq!(object.visible(), visible.contains(&index), "{line}");
            assert_eq!(
                object
                    .control()
                    .component()
                    .unwrap()
                    .controller("expanded")
                    .unwrap()
                    .selected_index,
                i32::from(expected[index])
            );
            if let Some(position) = visible.iter().position(|&node| node == index) {
                assert_eq!(object.y(), position as f32 * 20., "{line}");
            }
        }
        snapshots += 1;
    }
    assert_eq!((cases, snapshots), (64, 960));
}

#[test]
fn tree_branch_rejects_leaves_invalid_indices_non_trees_and_disposed_objects() {
    let (mut store, tree, _) = fixture(0);
    let initial = flags(&store, tree);
    for index in [1, 4, NODES.len(), usize::MAX] {
        assert!(!store.set_tree_branch_expanded(tree, index, true));
        assert_eq!(flags(&store, tree), initial);
    }
    let list = store.create(ObjectType::List);
    assert!(!store.set_tree_branch_expanded(list, 0, true));
    store.remove(tree);
    assert!(!store.set_tree_branch_expanded(tree, 0, false));
}

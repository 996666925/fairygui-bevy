use super::*;
use crate::{
    package::ObjectType,
    widget::{ListItemData, TreeNodeData},
};

fn fixture(
    mode: i32,
    first: bool,
    second: bool,
) -> (ObjectStore, GObjectId, [GObjectId; 3], [GObjectId; 3]) {
    let mut store = ObjectStore::new();
    let tree = store.create(ObjectType::Tree);
    store.set_size(tree, 200., 200.);
    let rows = std::array::from_fn(|_| {
        let row = store.create(ObjectType::Component);
        store.set_size(row, 100., 20.);
        store.set_parent(row, Some(tree));
        row
    });
    let parts = rows.map(|row| {
        let part = store.create(ObjectType::Component);
        store.set_parent(part, Some(row));
        part
    });
    let ControlData::Tree(data) = &mut store.objects[tree].control else {
        unreachable!();
    };
    data.nodes = (0..3)
        .map(|index| TreeNodeData {
            is_folder: index != 2,
            ..Default::default()
        })
        .collect();
    data.list.items = vec![ListItemData::default(); 3];
    data.expanded = vec![first, second, true];
    data.click_to_expand = mode;
    store.initialize_tree_cells(tree);
    (store, tree, rows, parts)
}

#[test]
fn folder_clicks_match_200_native_press_snapshot_scenarios_and_540_operations() {
    let mut state = None;
    let mut events = Vec::new();
    let (mut cases, mut operations) = (0, 0);
    for line in include_str!("../../tests/fixtures/tree-click-reference.txt").lines() {
        let v: Vec<_> = line.split_whitespace().collect();
        if v.is_empty() || v[0].starts_with('#') {
            continue;
        }
        if v[0] == "case" {
            state = Some(fixture(v[1].parse().unwrap(), v[2] == "1", v[3] == "1"));
            events.clear();
            cases += 1;
            continue;
        }
        let (store, tree, rows, parts) = state.as_mut().unwrap();
        let index: usize = v[1].parse().unwrap();
        match v[0] {
            "down" => {
                store.record_tree_pointer_down(rows[index], None);
            }
            "blocked" => {
                assert!(!store.record_tree_pointer_down(parts[index], Some(parts[index])));
            }
            "flip" => {
                store.toggle_tree_node(*tree, index);
            }
            "mode" => {
                let mut settings = store.tree_settings(*tree).unwrap();
                settings.click_to_expand = index as i32;
                store.set_tree_settings(*tree, settings);
            }
            "click" => {
                store.toggle_clicked_tree_node(parts[index]);
                let ControlData::Tree(data) = store.object(*tree).unwrap().control() else {
                    unreachable!();
                };
                events.push(data.is_expanded(index));
            }
            _ => unreachable!(),
        }
        let ControlData::Tree(data) = store.object(*tree).unwrap().control() else {
            unreachable!();
        };
        assert_eq!(
            data.pressed_expanded.map_or(-1, i32::from),
            v[3].parse::<i32>().unwrap(),
            "snapshot {line}"
        );
        assert_eq!(
            (data.is_expanded(0), data.is_expanded(1)),
            (v[4] == "1", v[5] == "1"),
            "{line}"
        );
        let expected: Vec<_> = if v[6] == "_" {
            vec![]
        } else {
            v[6].split(',').map(|flag| flag == "1").collect()
        };
        assert_eq!(events, expected, "item dispatch {line}");
        operations += 1;
    }
    assert_eq!((cases, operations), (200, 540));
}

#[test]
fn tree_press_snapshots_belong_to_each_tree_and_only_folder_down_updates_them() {
    let (mut store, first, rows, parts) = fixture(1, true, false);
    assert!(
        !store.toggle_clicked_tree_node(rows[0]),
        "click without MouseDown has no native sample"
    );
    assert!(store.record_tree_pointer_down(parts[0], Some(rows[0])));
    assert!(!store.record_tree_pointer_down(rows[2], None));
    assert!(store.toggle_clicked_tree_node(parts[0]));
    assert!(
        !store.toggle_clicked_tree_node(parts[0]),
        "the shared snapshot is retained, not resampled on click"
    );
    let second = store.create(ObjectType::Tree);
    let row = store.create(ObjectType::Component);
    store.set_parent(row, Some(second));
    let ControlData::Tree(data) = &mut store.objects[second].control else {
        unreachable!();
    };
    data.nodes = vec![TreeNodeData {
        is_folder: true,
        ..Default::default()
    }];
    data.expanded = vec![false];
    data.click_to_expand = 1;
    assert!(store.record_tree_pointer_down(row, None));
    assert!(store.toggle_clicked_tree_node(row));
    let ControlData::Tree(data) = store.object(first).unwrap().control() else {
        unreachable!();
    };
    assert_eq!(data.pressed_expanded, Some(true));
    assert!(
        !store.toggle_clicked_tree_node(rows[0]),
        "another tree cannot replace the original snapshot"
    );
    store.remove(first);
    assert!(!store.record_tree_pointer_down(parts[0], None));
}

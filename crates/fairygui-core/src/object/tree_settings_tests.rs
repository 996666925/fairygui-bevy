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

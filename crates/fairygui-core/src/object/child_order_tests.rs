use super::*;
use crate::widget::ControlData;

#[test]
fn child_order_matches_513_native_snapshots_across_reorders_priorities_and_reparenting() {
    let mut store = ObjectStore::new();
    let parents: Vec<_> = (0..3)
        .map(|_| store.create(ObjectType::Component))
        .collect();
    let children: Vec<_> = (0..7)
        .map(|_| store.create(ObjectType::Component))
        .collect();
    for &child in &children {
        store.set_parent(child, Some(parents[0]));
    }
    let mut count = 0;
    for row in include_str!("../../tests/fixtures/child-order-reference.txt")
        .lines()
        .filter(|l| !l.starts_with('#'))
    {
        let fields: Vec<_> = row.split('|').collect();
        let op: Vec<_> = fields[0].split_whitespace().collect();
        let child = |name: &str| children[(name.as_bytes()[0] - b'a') as usize];
        match op[0] {
            "init" => {}
            "order" => {
                store.set_sorting_order(child(op[1]), op[2].parse().unwrap());
            }
            "index" => {
                store.set_child_index(child(op[1]), op[2].parse().unwrap());
            }
            "before" => {
                store.set_child_index_before(child(op[1]), op[2].parse().unwrap());
            }
            "add" => {
                store.add_child_at(
                    parents[(op[2].as_bytes()[0] - b'p') as usize],
                    child(op[1]),
                    op[3].parse().unwrap(),
                );
            }
            "swap" => {
                store.swap_children(child(op[1]), child(op[2]));
            }
            "detach" => {
                store.detach_from_parent(child(op[1]));
            }
            _ => panic!("unknown native operation: {row}"),
        }
        for (i, &parent) in parents.iter().enumerate() {
            let expected: Vec<_> = fields[i + 1]
                .bytes()
                .map(|name| children[(name - b'a') as usize])
                .collect();
            assert_eq!(
                store.object(parent).unwrap().children(),
                expected,
                "trace {count}: {row}"
            );
            for (index, &id) in expected.iter().enumerate() {
                assert_eq!(store.object(id).unwrap().parent(), Some(parent));
                assert_eq!(store.child_index(id), Some(index));
            }
        }
        for (&id, expected) in children.iter().zip(fields[4].split(',')) {
            assert_eq!(
                store.object(id).unwrap().sorting_order(),
                expected.parse::<i32>().unwrap(),
                "trace {count}: {row}"
            );
        }
        count += 1;
    }
    assert_eq!(count, 513);
}

#[test]
fn sorted_boundaries_before_indices_and_equal_priorities_follow_native_rules() {
    let mut store = ObjectStore::new();
    let parent = store.create(ObjectType::Component);
    let rows: Vec<_> = (0..5)
        .map(|_| store.create(ObjectType::Component))
        .collect();
    for &row in &rows {
        store.set_parent(row, Some(parent));
    }
    store.set_sorting_order(rows[4], 1);
    assert_eq!(store.set_child_index_before(rows[0], usize::MAX), Some(2));
    assert_eq!(
        store.object(parent).unwrap().children(),
        &[rows[1], rows[2], rows[0], rows[3], rows[4]]
    );
    assert_eq!(store.set_child_index(rows[0], usize::MAX), Some(3));
    store.set_sorting_order(rows[2], 1);
    assert_eq!(
        store.object(parent).unwrap().children(),
        &[rows[1], rows[3], rows[0], rows[4], rows[2]]
    );
    assert!(!store.set_sorting_order(rows[4], 1));
    assert_eq!(store.set_child_index(rows[4], 0), Some(3));
    store.set_sorting_order(rows[2], -1);
    assert_eq!(
        store.object(parent).unwrap().children(),
        &[rows[1], rows[3], rows[0], rows[2], rows[4]]
    );
}

#[test]
fn reordering_keeps_selected_objects_and_does_not_replay_stage_lifecycle() {
    let mut store = ObjectStore::new();
    let list = store.create(ObjectType::List);
    let rows: Vec<_> = (0..3).map(|_| store.create(ObjectType::Button)).collect();
    for &row in &rows {
        if let ControlData::Button(data) = &mut store.objects[row].control {
            data.mode = crate::widget::ButtonMode::Check;
        }
        store.set_parent(row, Some(list));
    }
    store.attach_to_stage(list);
    store.set_list_selection(list, 0);
    store.set_sorting_order(rows[0], 2);
    assert_eq!(store.list_selections(list), Some(vec![2]));
    assert_eq!(store.button_selected(rows[0]), Some(true));
    assert!(rows.iter().all(|&row| store.is_on_stage(row)));
    assert!(store.stage_events.is_empty());
    assert!(!store.swap_children(list, rows[0]));
    store.remove(rows[0]);
    assert_eq!(store.set_child_index(rows[0], 0), None);
    assert!(!store.set_sorting_order(rows[0], 1));
    assert_eq!(store.set_child_index(list, 0), None);
}

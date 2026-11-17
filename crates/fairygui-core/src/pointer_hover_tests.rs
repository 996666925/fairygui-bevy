use super::*;
use crate::package::ObjectType;

#[test]
fn hover_common_ancestors_current_chains_and_stage_filters_match_640_native_traces() {
    let mut count = 0;
    for line in include_str!("../tests/fixtures/pointer-hover-reference.txt").lines() {
        let v: Vec<_> = line.split_whitespace().collect();
        if v.first() != Some(&"case") {
            continue;
        }
        let number = |i: usize| v[i].parse::<usize>().unwrap();
        let mut store = ObjectStore::new();
        let ids: Vec<_> = (0..7)
            .map(|_| store.create(ObjectType::Component))
            .collect();
        for (index, parent) in [0, 1, 1, 2, 2, 3, 0].into_iter().enumerate() {
            if parent > 0 {
                store.set_parent(ids[index], Some(ids[parent - 1]));
            }
        }
        store.attach_to_stage(ids[0]);
        let previous = number(1).checked_sub(1).map(|i| ids[i]);
        let next = number(2).checked_sub(1).map(|i| ids[i]);
        match v[3] {
            "none" => {}
            "detach-old" | "detach-new" => {
                if let Some(id) = if v[3] == "detach-old" { previous } else { next } {
                    store.detach_from_stage(id);
                    store.detach_from_parent(id);
                }
            }
            "remove-old" => {
                if let Some(id) = previous {
                    store.remove(id);
                }
            }
            "remove-parent" => {
                if let Some(id) = previous.and_then(|id| store.object(id).unwrap().parent()) {
                    store.remove(id);
                }
            }
            "reparent-old" => {
                if let Some(id) = previous.filter(|_| number(1) != 1 && number(1) != 3) {
                    store.set_parent(id, Some(ids[2]));
                }
            }
            "reparent-new" => {
                if let Some(id) = next.filter(|_| number(2) != 1 && number(2) != 2) {
                    store.set_parent(id, Some(ids[1]));
                }
            }
            "hide-old" => {
                if let Some(id) = previous {
                    store.set_visible(id, false);
                }
            }
            "untouch-new" => {
                if let Some(id) = next {
                    store.set_touchable(id, false);
                }
            }
            "detach-root" => {
                store.detach_from_stage(ids[0]);
            }
            _ => unreachable!(),
        }
        let change = HoverChange::between(&store, previous, next);
        let mut actual = Vec::new();
        for (kind, targets) in [("out", change.exited), ("over", change.entered)] {
            for target in targets {
                let ordinal = ids.iter().position(|&id| id == target).unwrap() + 1;
                actual.push(format!("{kind}:{ordinal}:{ordinal}"));
            }
        }
        let expected: Vec<_> = if v[4] == "_" {
            vec![]
        } else {
            v[4].split(',').map(str::to_owned).collect()
        };
        assert_eq!(actual, expected, "{line}");
        count += 1;
    }
    assert_eq!(count, 640);
}

#[test]
fn a_disposed_hover_target_does_not_retarget_reused_slots() {
    let mut store = ObjectStore::new();
    let root = store.create(ObjectType::Component);
    let old = store.create(ObjectType::Component);
    store.set_parent(old, Some(root));
    store.attach_to_stage(root);
    store.remove(old);
    let next = store.create(ObjectType::Component);
    store.set_parent(next, Some(root));
    assert_ne!(old, next);
    assert_eq!(
        HoverChange::between(&store, Some(old), Some(next)),
        HoverChange {
            exited: vec![],
            entered: vec![next, root]
        }
    );
}

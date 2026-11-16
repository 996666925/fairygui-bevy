use super::*;
use crate::package::ObjectType;

#[test]
fn saved_press_paths_match_native_click_test_after_tree_and_policy_changes() {
    let mut count = 0;
    for line in include_str!("../tests/fixtures/click-target-reference.txt").lines() {
        let v: Vec<_> = line.split_whitespace().collect();
        if v.first() != Some(&"case") {
            continue;
        }
        let number = |i: usize| v[i].parse::<usize>().unwrap();
        let mut store = ObjectStore::new();
        let nodes: Vec<_> = (0..7)
            .map(|_| store.create(ObjectType::Component))
            .collect();
        for (index, parent) in [0, 1, 1, 2, 2, 3, 0].into_iter().enumerate() {
            if parent > 0 {
                store.set_parent(nodes[index], Some(nodes[parent - 1]));
            }
        }
        store.attach_to_stage(nodes[0]);
        let target = nodes[number(1) - 1];
        let release = number(2).checked_sub(1).map(|i| nodes[i]);
        let stop = number(4).checked_sub(1).map(|i| nodes[i]);
        let mut press = ClickTargets::begin(&store, target, stop);
        match v[3] {
            "none" => {}
            "hide" => store.set_visible(target, false),
            "untouch" => store.set_touchable(target, false),
            "detach" => {
                store.detach_from_stage(target);
                store.detach_from_parent(target);
            }
            "remove" => {
                store.remove(target);
            }
            "detach-parent" => {
                if let Some(parent) = store.object(target).unwrap().parent() {
                    store.detach_from_stage(parent);
                    store.detach_from_parent(parent);
                }
            }
            "reparent" => {
                if number(1) != 1 && number(1) != 3 {
                    store.set_parent(target, Some(nodes[2]));
                }
            }
            "reparent-release" => {
                if number(2) >= 4 {
                    store.set_parent(release.unwrap(), Some(nodes[2]));
                }
            }
            "detach-root" => {
                store.detach_from_stage(nodes[0]);
            }
            "stage-alone" => {
                store.attach_to_stage(target);
            }
            _ => unreachable!(),
        }
        let actual = press.resolve(&store, release, v[5] == "1");
        assert_eq!(actual, number(6).checked_sub(1).map(|i| nodes[i]), "{line}");
        assert_eq!(
            usize::from(!press.is_empty()),
            number(7),
            "path consumed {line}"
        );
        assert_eq!(
            press.resolve(&store, release, false),
            None,
            "one-shot {line}"
        );
        count += 1;
    }
    assert_eq!(count, 1920);
}

#[test]
fn disposal_does_not_redirect_clicks_to_reused_object_slots() {
    let mut store = ObjectStore::new();
    let root = store.create(ObjectType::Component);
    let child = store.create(ObjectType::Component);
    store.set_parent(child, Some(root));
    store.attach_to_stage(root);
    let mut press = ClickTargets::begin(&store, child, None);
    store.remove(child);
    let replacement = store.create(ObjectType::Component);
    store.set_parent(replacement, Some(root));
    assert_ne!(replacement, child);
    assert_eq!(press.resolve(&store, Some(replacement), false), Some(root));
    let mut empty = ClickTargets::begin(&store, child, None);
    assert!(empty.is_empty());
    assert_eq!(empty.resolve(&store, Some(replacement), false), None);
}

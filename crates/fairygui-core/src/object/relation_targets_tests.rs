use super::*;
use crate::widget::relation::{RelationDef, RelationItem};

fn relation(
    store: &mut ObjectStore,
    owner: GObjectId,
    target: Option<usize>,
    child: bool,
    kind: u8,
) {
    store
        .object_mut(owner)
        .unwrap()
        .relations
        .items
        .push(RelationItem {
            to_parent: target.is_none(),
            to_child: child,
            target_index: target,
            binding: None,
            defs: vec![RelationDef {
                relation_type: kind,
                percent: false,
                axis: u8::from(kind >= 7 && kind != 14),
            }],
        });
}
fn fixture() -> (ObjectStore, GObjectId, GObjectId, GObjectId, GObjectId) {
    let mut store = ObjectStore::new();
    let parent = store.create(ObjectType::Component);
    let a = store.create(ObjectType::Component);
    let b = store.create(ObjectType::Component);
    let owner = store.create(ObjectType::Component);
    for id in [a, b, owner] {
        store.set_parent(id, Some(parent));
        store.set_size(id, 50., 30.);
    }
    store.set_xy(owner, 100., 80.);
    (store, parent, a, b, owner)
}
#[test]
fn sibling_relations_keep_the_original_target_across_reorder_priority_and_index_reuse() {
    let (mut store, parent, target, other, owner) = fixture();
    relation(&mut store, owner, Some(0), false, 0);
    relation(&mut store, owner, Some(0), false, 14);
    store.set_child_index(target, 2);
    store.set_xy(other, 20., 0.);
    store.set_size(other, 90., 30.);
    assert_eq!(
        store.object(owner).unwrap().position(),
        Point::new(100., 80.)
    );
    assert_eq!(store.object(owner).unwrap().width(), 50.);
    store.set_xy(target, 30., 0.);
    store.set_size(target, 80., 30.);
    assert_eq!(
        store.object(owner).unwrap().position(),
        Point::new(130., 80.)
    );
    assert_eq!(store.object(owner).unwrap().width(), 80.);
    store.set_sorting_order(target, 3);
    store.remove(other);
    store.set_xy(target, 40., 0.);
    assert_eq!(store.object(owner).unwrap().x(), 140.);
    store.remove(target);
    let replacement = store.create(ObjectType::Component);
    store.add_child_at(parent, replacement, 0);
    store.set_xy(replacement, 100., 0.);
    store.set_size(replacement, 500., 200.);
    assert_eq!(
        store.object(owner).unwrap().position(),
        Point::new(140., 80.)
    );
    assert_eq!(store.object(owner).unwrap().width(), 80.);
}
#[test]
fn target_and_owner_can_move_between_containers_without_rebinding() {
    let (mut store, _, target, _, owner) = fixture();
    relation(&mut store, owner, Some(0), false, 0);
    relation(&mut store, owner, Some(0), false, 14);
    let new_parent = store.create(ObjectType::Component);
    store.set_parent(target, Some(new_parent));
    store.set_xy(target, 10., 0.);
    store.set_size(target, 70., 30.);
    assert_eq!(store.object(owner).unwrap().x(), 110.);
    assert_eq!(store.object(owner).unwrap().width(), 70.);
    store.detach_from_parent(owner);
    store.set_xy(target, 20., 0.);
    assert_eq!(store.object(owner).unwrap().x(), 120.);
    store.set_parent(owner, Some(new_parent));
    store.set_xy(target, 30., 0.);
    assert_eq!(store.object(owner).unwrap().x(), 130.);
}
#[test]
fn parent_relation_retains_original_parent_size_subscription_after_reparenting() {
    let (mut store, parent, _, _, owner) = fixture();
    relation(&mut store, owner, None, false, 14);
    let new_parent = store.create(ObjectType::Component);
    store.set_parent(owner, Some(new_parent));
    store.set_size(new_parent, 300., 100.);
    assert_eq!(store.object(owner).unwrap().width(), 50.);
    store.set_size(parent, 100., 50.);
    assert_eq!(store.object(owner).unwrap().width(), 150.);
    store.set_xy(parent, 20., 10.);
    assert_eq!(
        store.object(owner).unwrap().position(),
        Point::new(100., 80.)
    );
}
#[test]
fn component_relation_retains_child_target_when_the_child_is_reordered_or_detached() {
    let (mut store, parent, target, other, _) = fixture();
    relation(&mut store, parent, Some(0), true, 15);
    store.swap_children(target, other);
    store.set_size(other, 50., 90.);
    assert_eq!(store.object(parent).unwrap().height(), 0.);
    store.detach_from_parent(target);
    store.set_size(target, 50., 70.);
    assert_eq!(store.object(parent).unwrap().height(), 40.);
}

#[test]
fn retained_target_notifications_match_original_relation_item_lifecycle_traces() {
    let mut store = ObjectStore::new();
    let mut ids = std::collections::HashMap::new();
    let mut owner = None;
    let mut states = 0;
    for line in include_str!("../../tests/fixtures/relation-target-reference.txt").lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let (command, expected) = line.split_once(" | ").unwrap_or((line, ""));
        let parts: Vec<_> = command.split_whitespace().collect();
        if parts[0] == "case" {
            store = ObjectStore::new();
            ids.clear();
            for (name, width, height) in [
                ("p", 400., 200.),
                ("q", 600., 300.),
                ("a", 50., 30.),
                ("b", 50., 30.),
                ("o", 100., 60.),
            ] {
                let id = store.create(ObjectType::Component);
                store.set_size(id, width, height);
                store.set_source_size(id, width, height);
                store.set_init_size(id, width, height);
                ids.insert(name, id);
            }
            for name in ["a", "b", "o"] {
                store.set_parent(ids[name], Some(ids["p"]));
            }
            store.set_xy(ids["o"], 100., 80.);
            let related_owner = if parts[1] == "child" {
                ids["p"]
            } else {
                ids["o"]
            };
            let kind: u8 = parts[2].parse().unwrap();
            for kind in if kind == 24 { vec![14, 15] } else { vec![kind] } {
                relation(
                    &mut store,
                    related_owner,
                    if parts[1] == "parent" { None } else { Some(0) },
                    parts[1] == "child",
                    kind,
                );
            }
            store.bind_relation_targets(related_owner);
            owner = Some(related_owner);
        } else {
            let id = ids[parts[1]];
            match parts[0] {
                "index" => {
                    store.set_child_index(id, parts[2].parse().unwrap());
                }
                "xy" => store.set_xy(id, parts[2].parse().unwrap(), parts[3].parse().unwrap()),
                "size" => store.set_size(id, parts[2].parse().unwrap(), parts[3].parse().unwrap()),
                "parent" => {
                    store.set_parent(id, Some(ids[parts[2]]));
                }
                "detach" => {
                    store.detach_from_parent(id);
                }
                "dispose" => {
                    store.remove(id);
                }
                other => panic!("{other}"),
            }
            let expected: Vec<f32> = expected
                .split_whitespace()
                .map(|v| v.parse().unwrap())
                .collect();
            let o = store.object(owner.unwrap()).unwrap();
            assert_eq!(
                [o.x(), o.y(), o.width(), o.height()],
                expected[..],
                "{line}"
            );
            states += 1;
        }
    }
    assert_eq!(states, 360);
}

#[test]
fn exported_forward_references_bind_only_after_all_siblings_exist() {
    let package =
        crate::FuiPackage::parse("Chat", include_bytes!("../../tests/fixtures/Chat.fui")).unwrap();
    let mut store = ObjectStore::new();
    let root = crate::widget::create_from_package(
        &mut store,
        &package,
        package.item_index_by_name("chatLeft").unwrap(),
        &crate::package::LoadContext::none(),
    )
    .unwrap();
    let bubble = store.child_by_name(root, "n0").unwrap();
    let msg = store.child_by_name(root, "msg").unwrap();
    let bubble_size = store.object(bubble).unwrap().size();
    let message_size = store.object(msg).unwrap().size();
    store.set_size(msg, message_size.width + 100., message_size.height + 20.);
    assert_eq!(
        store.object(bubble).unwrap().size(),
        Size::new(bubble_size.width + 100., bubble_size.height + 20.)
    );
    store.set_child_index(msg, 0);
    store.set_size(msg, message_size.width + 160., message_size.height + 40.);
    assert_eq!(
        store.object(bubble).unwrap().size(),
        Size::new(bubble_size.width + 160., bubble_size.height + 40.)
    );
}

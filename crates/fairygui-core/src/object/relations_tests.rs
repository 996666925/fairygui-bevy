use super::*;

const TYPES: [RelationType; 25] = [
    RelationType::LeftLeft,
    RelationType::LeftCenter,
    RelationType::LeftRight,
    RelationType::CenterCenter,
    RelationType::RightLeft,
    RelationType::RightCenter,
    RelationType::RightRight,
    RelationType::TopTop,
    RelationType::TopMiddle,
    RelationType::TopBottom,
    RelationType::MiddleMiddle,
    RelationType::BottomTop,
    RelationType::BottomMiddle,
    RelationType::BottomBottom,
    RelationType::Width,
    RelationType::Height,
    RelationType::LeftExtLeft,
    RelationType::LeftExtRight,
    RelationType::RightExtLeft,
    RelationType::RightExtRight,
    RelationType::TopExtTop,
    RelationType::TopExtBottom,
    RelationType::BottomExtTop,
    RelationType::BottomExtBottom,
    RelationType::Size,
];

fn fixture() -> (ObjectStore, GObjectId, GObjectId, GObjectId) {
    let mut store = ObjectStore::new();
    let parent = store.create(ObjectType::Component);
    store.set_size(parent, 400., 300.);
    let target = store.create(ObjectType::Component);
    let owner = store.create(ObjectType::Component);
    store.set_parent(target, Some(parent));
    store.set_parent(owner, Some(parent));
    store.set_xy(target, 40., 30.);
    store.set_size(target, 100., 60.);
    store.set_xy(owner, 180., 100.);
    store.set_size(owner, 200., 160.);
    (store, parent, target, owner)
}

#[test]
fn typed_add_remove_clear_and_duplicate_percent_policy_match_native_runtime_traces() {
    let (mut store, _, mut target, mut owner) = fixture();
    let mut kind = RelationType::Size;
    let mut states = 0;
    for line in include_str!("../../tests/fixtures/relation-api-reference.txt").lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let (command, expected) = line.split_once(" | ").unwrap_or((line, ""));
        let words: Vec<_> = command.split_whitespace().collect();
        if words[0] == "case" {
            (store, _, target, owner) = fixture();
            kind = TYPES[words[1].parse::<usize>().unwrap()];
            assert_eq!(kind as usize, words[1].parse::<usize>().unwrap());
            continue;
        }
        let changed = match words[0] {
            "add" => store.add_relation(owner, target, kind, words[1] == "1"),
            "remove" => store.remove_relation(owner, target, kind),
            "clearTo" => store.clear_relations_to(owner, target),
            "clearAll" => store.clear_relations(owner),
            "size" => {
                store.set_size(target, words[1].parse().unwrap(), words[2].parse().unwrap());
                false
            }
            "xy" => {
                store.set_xy(target, words[1].parse().unwrap(), words[2].parse().unwrap());
                false
            }
            _ => unreachable!(),
        };
        let expected: Vec<f32> = expected
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        let o = store.object(owner).unwrap();
        for (actual, expected) in [o.x(), o.y(), o.width(), o.height()]
            .into_iter()
            .zip(&expected)
        {
            assert!(
                (actual - expected).abs() < 0.001,
                "state {states}: {command}, kind={kind:?}: {actual} != {expected}"
            );
        }
        assert_eq!(
            store.has_relation(owner, target, kind),
            expected[4] == 1.,
            "{line}"
        );
        assert_eq!(changed, expected[5] == 1., "{line}");
        states += 1;
    }
    assert_eq!(states, 750);
}

#[test]
fn size_expansion_keeps_existing_percent_and_removes_only_requested_definitions() {
    let (mut store, _, target, owner) = fixture();
    assert!(store.add_relation(owner, target, RelationType::Width, true));
    assert!(store.add_relation(owner, target, RelationType::Size, false));
    assert!(!store.add_relation(owner, target, RelationType::Size, true));
    assert!(store.has_relation(owner, target, RelationType::Size));
    store.set_size(target, 200., 120.);
    assert_eq!(store.object(owner).unwrap().size(), Size::new(400., 220.));
    assert!(store.remove_relation(owner, target, RelationType::Width));
    assert!(!store.has_relation(owner, target, RelationType::Size));
    assert!(store.has_relation(owner, target, RelationType::Height));
    assert_eq!(store.relation_dependents[&target], vec![owner]);
    store.set_size(target, 300., 150.);
    assert_eq!(store.object(owner).unwrap().size(), Size::new(400., 250.));
    assert!(store.remove_relation(owner, target, RelationType::Size));
    assert!(!store.relation_dependents.contains_key(&target));
    store.set_size(target, 400., 200.);
    assert_eq!(store.object(owner).unwrap().size(), Size::new(400., 250.));
}

#[test]
fn clear_one_target_preserves_others_and_clear_all_stops_self_alignment() {
    let (mut store, parent, target, owner) = fixture();
    store.add_relation(owner, target, RelationType::LeftLeft, false);
    store.add_relation(owner, parent, RelationType::CenterCenter, false);
    assert!(store.clear_relations_to(owner, target));
    assert!(!store.relation_dependents.contains_key(&target));
    assert_eq!(store.relation_dependents[&parent], vec![owner]);
    store.set_xy(target, 100., 30.);
    assert_eq!(store.object(owner).unwrap().x(), 180.);
    store.set_size(owner, 240., 160.);
    assert_eq!(store.object(owner).unwrap().x(), 160.);
    assert!(store.clear_relations(owner));
    assert!(!store.clear_relations(owner));
    assert!(!store.relation_dependents.contains_key(&parent));
    store.set_size(owner, 300., 160.);
    assert_eq!(store.object(owner).unwrap().x(), 160.);
}

#[test]
fn exported_relations_can_be_removed_without_rebinding_or_reconstruction() {
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
    let size = store.object(bubble).unwrap().size();
    assert!(store.has_relation(bubble, msg, RelationType::Size));
    assert!(!store.add_relation(bubble, msg, RelationType::Width, true));
    assert!(store.remove_relation(bubble, msg, RelationType::Size));
    store.set_size(msg, 400., 200.);
    assert_eq!(store.object(bubble).unwrap().size(), size);
}

#[test]
fn runtime_targets_survive_reparenting_and_disposal_without_following_reused_slots() {
    let (mut store, parent, target, owner) = fixture();
    store.add_relation(owner, target, RelationType::LeftLeft, false);
    let other = store.create(ObjectType::Component);
    store.set_parent(target, Some(other));
    store.set_xy(target, 50., 30.);
    assert_eq!(store.object(owner).unwrap().x(), 190.);
    store.set_parent(owner, Some(other));
    store.set_xy(target, 60., 30.);
    assert_eq!(store.object(owner).unwrap().x(), 200.);
    store.remove(target);
    let replacement = store.create(ObjectType::Component);
    store.set_parent(replacement, Some(parent));
    store.set_xy(replacement, 500., 500.);
    assert_eq!(store.object(owner).unwrap().x(), 200.);
    assert!(!store.add_relation(owner, target, RelationType::Size, false));
    assert!(!store.add_relation(owner, owner, RelationType::Size, false));
    assert!(store.clear_relations_to(owner, target));
    assert!(store.objects[owner].relations.is_empty());
    store.add_relation(owner, replacement, RelationType::LeftLeft, false);
    store.add_relation(replacement, owner, RelationType::LeftLeft, false);
    store.set_xy(replacement, 510., 500.);
    assert!(store.relation_handling.is_empty());
    store.remove(owner);
    assert!(
        store
            .relation_dependents
            .values()
            .all(|owners| !owners.contains(&owner))
    );
}

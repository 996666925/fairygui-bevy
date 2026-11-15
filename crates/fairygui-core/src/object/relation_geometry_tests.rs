use super::*;

#[test]
fn constrained_pivoted_relations_match_native_parent_sibling_and_child_targets() {
    let mut store = ObjectStore::new();
    let mut owner = None;
    let mut target = None;
    let mut states = 0;
    let mut case = "";
    for line in include_str!("../../tests/fixtures/relation-geometry-reference.txt").lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let (command, expected) = line.split_once(" | ").unwrap_or((line, ""));
        let words: Vec<_> = command.split_whitespace().collect();
        if words[0] == "case" {
            case = line;
            store = ObjectStore::new();
            let root = store.create(ObjectType::Component);
            let parent = store.create(ObjectType::Component);
            store.set_parent(parent, Some(root));
            let t = if words[1] == "parent" {
                parent
            } else {
                store.create(ObjectType::Component)
            };
            let o = if words[1] == "child" {
                parent
            } else {
                store.create(ObjectType::Component)
            };
            for (id, x, y, w, h) in [
                (root, 0., 0., 800., 600.),
                (parent, 180., 100., 200., 160.),
                (t, 40., 30., 100., 60.),
                (o, 180., 100., 200., 160.),
            ] {
                store.set_xy(id, x, y);
                store.set_size(id, w, h);
                store.set_source_size(id, w, h);
                store.set_init_size(id, w, h);
            }
            if t != parent {
                store.set_parent(t, Some(parent));
            }
            if o != parent {
                store.set_parent(o, Some(parent));
            }
            store.set_pivot(o, 0.5, 0.5, words[4] == "1");
            store.set_pivot(t, 0.25, 0.75, true);
            if words[5] == "1" {
                store.set_size_constraints(
                    o,
                    SizeConstraints {
                        min: Size::new(70., 50.),
                        max: Size::new(240., 190.),
                    },
                );
            }
            let kind = match words[2].parse::<u8>().unwrap() {
                0 => RelationType::LeftLeft,
                1 => RelationType::LeftCenter,
                2 => RelationType::LeftRight,
                3 => RelationType::CenterCenter,
                4 => RelationType::RightLeft,
                5 => RelationType::RightCenter,
                6 => RelationType::RightRight,
                7 => RelationType::TopTop,
                8 => RelationType::TopMiddle,
                9 => RelationType::TopBottom,
                10 => RelationType::MiddleMiddle,
                11 => RelationType::BottomTop,
                12 => RelationType::BottomMiddle,
                13 => RelationType::BottomBottom,
                14 => RelationType::Width,
                15 => RelationType::Height,
                16 => RelationType::LeftExtLeft,
                17 => RelationType::LeftExtRight,
                18 => RelationType::RightExtLeft,
                19 => RelationType::RightExtRight,
                20 => RelationType::TopExtTop,
                21 => RelationType::TopExtBottom,
                22 => RelationType::BottomExtTop,
                23 => RelationType::BottomExtBottom,
                24 => RelationType::Size,
                _ => unreachable!(),
            };
            store.add_relation(o, t, kind, words[3] == "1");
            owner = Some(o);
            target = Some(t);
        } else {
            let x = words[1].parse().unwrap();
            let y = words[2].parse().unwrap();
            if words[0] == "size" {
                store.set_size(target.unwrap(), x, y);
            } else {
                store.set_xy(target.unwrap(), x, y);
            }
            let o = store.object(owner.unwrap()).unwrap();
            let raw = o.requested_size();
            let expected: Vec<f32> = expected
                .split_whitespace()
                .map(|v| v.parse().unwrap())
                .collect();
            let actual = [o.x(), o.y(), o.width(), o.height(), raw.width, raw.height];
            for (index, (actual, expected)) in actual.into_iter().zip(expected).enumerate() {
                assert!(
                    (actual - expected).abs() < 0.01,
                    "{case}, state {states}: {line}, axis {index}: {actual} != {expected}"
                );
            }
            states += 1;
        }
    }
    assert_eq!(states, 3600);
}

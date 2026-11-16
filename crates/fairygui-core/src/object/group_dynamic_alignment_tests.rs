use super::*;

fn layout(code: u8) -> GroupLayoutType {
    match code {
        0 => GroupLayoutType::None,
        1 => GroupLayoutType::Horizontal,
        2 => GroupLayoutType::Vertical,
        _ => panic!("invalid layout {code}"),
    }
}

fn scene(config: &[u8]) -> (ObjectStore, Vec<GObjectId>) {
    let mut store = ObjectStore::new();
    let mut objects = Vec::new();
    for (kind, x, y, w, h) in [
        (ObjectType::Component, 0., 0., 300., 200.),
        (ObjectType::Group, 100., 70., 100., 60.),
        (ObjectType::Group, 100., 70., 60., 60.),
        (ObjectType::Component, 100., 70., 30., 20.),
        (ObjectType::Component, 100., 90., 40., 40.),
        (ObjectType::Component, 160., 70., 40., 60.),
    ] {
        let id = store.create(kind);
        if let Some(&parent) = objects.first() {
            store.set_parent(id, Some(parent));
        }
        store.set_size(id, w, h);
        store.set_xy(id, x, y);
        objects.push(id);
    }
    for (member, group) in [(2, 1), (3, 2), (4, 2), (5, 1)] {
        store.set_group(objects[member], Some(objects[group]));
    }
    for (index, fixed) in [(1, config[2] == 1), (2, false)] {
        store.set_group_layout_settings(
            objects[index],
            GroupLayoutSettings {
                layout: layout(config[index - 1]),
                auto_size_disabled: fixed,
                ..Default::default()
            },
        );
    }
    let (horizontal, vertical) = match config[3] {
        0 => (RelationType::LeftLeft, RelationType::TopTop),
        3 => (RelationType::CenterCenter, RelationType::MiddleMiddle),
        6 => (RelationType::RightRight, RelationType::BottomBottom),
        code => panic!("invalid alignment {code}"),
    };
    store.add_relation(objects[1], objects[0], horizontal, false);
    store.add_relation(objects[1], objects[0], vertical, false);
    store.add_relation(
        objects[5],
        objects[2],
        if config[0] == 1 {
            RelationType::MiddleMiddle
        } else {
            RelationType::CenterCenter
        },
        false,
    );
    (store, objects)
}

#[test]
fn nested_group_dynamic_alignment_matches_original_runtime() {
    let mut store = ObjectStore::new();
    let mut objects = Vec::new();
    let mut case = "";
    let mut states = 0;
    for line in include_str!("../../tests/fixtures/group-dynamic-alignment-reference.txt").lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some(config) = line.strip_prefix("case ") {
            case = config;
            let config: Vec<u8> = config
                .split_whitespace()
                .map(|v| v.parse().unwrap())
                .collect();
            (store, objects) = scene(&config);
            continue;
        }
        let [parent, outer, _inner, first, second, _peer] = objects[..] else {
            panic!("missing scene")
        };
        let (operation, expected) = line.split_once(" | ").unwrap();
        match operation {
            "initial" => {}
            "parent" => store.set_size(parent, 400., 240.),
            "batch" => {
                // Pending inner layout must not leave outer alignment using
                // the previous frame's content rectangle.
                store.set_size(first, 75., 35.);
                store.set_size(parent, 500., 300.);
            }
            "group" => store.set_size(outer, 220., 130.),
            "members" => {
                store.set_size(first, 30., 20.);
                store.set_size(second, 80., 50.);
            }
            "move_group" | "move_member" => {
                let (object, dx, dy) = if operation == "move_group" {
                    (outer, 17., -9.)
                } else {
                    (first, 10., -5.)
                };
                let p = store.position_of(object).unwrap();
                store.set_xy(object, p.x + dx, p.y + dy);
            }
            _ => panic!("invalid operation {operation}"),
        }
        store.flush_group_layouts();
        let expected: Vec<f32> = expected
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        let actual: Vec<f32> = objects[1..]
            .iter()
            .flat_map(|&id| {
                let o = store.object(id).unwrap();
                [o.x, o.y, o.width, o.height]
            })
            .collect();
        assert_eq!(actual, expected, "case {case}: {operation}");
        assert_eq!(store.flush_group_layouts(), 0, "case {case}: {operation}");
        states += 1;
    }
    assert_eq!(states, 378);
}

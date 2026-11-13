use super::*;
use crate::{
    package::ObjectType,
    widget::{Controller, Gear, GearKind, GearValue},
};

#[test]
fn flip_scale_position_and_coordinate_sequences_match_original_gimage_methods() {
    let mut store = ObjectStore::new();
    let mut id = store.create(ObjectType::Image);
    let mut states = 0;
    for line in include_str!("../../tests/fixtures/image-flip-reference.txt").lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.is_empty() || fields[0].starts_with('#') {
            continue;
        }
        if fields[0] == "new" {
            store = ObjectStore::new();
            id = store.create(ObjectType::Image);
            continue;
        }
        let v: Vec<f32> = fields[1..].iter().map(|v| v.parse().unwrap()).collect();
        match fields[0] {
            "size" => store.set_size(id, v[0], v[1]),
            "xy" => store.set_xy(id, v[0], v[1]),
            "pivot" => store.set_pivot(id, v[0], v[1], v[2] != 0.),
            "scale" => store.set_scale(id, v[0], v[1]),
            "flip" => store.set_image_flip(id, FlipType::from_code(v[0] as u8)),
            "rotation" => store.set_rotation(id, v[0]),
            "skew" => store.set_skew(id, v[0], v[1]),
            "state" => {
                let o = store.object(id).unwrap();
                let actual = [
                    o.control().as_image().unwrap().flip as u8 as f32,
                    o.x(),
                    o.y(),
                    o.width(),
                    o.height(),
                    o.scale_x(),
                    o.scale_y(),
                ];
                for (a, b) in actual.iter().zip(&v) {
                    assert!((a - b).abs() < 0.0002, "{line}: {actual:?}");
                }
                if let Some(p) = o.image_display_position() {
                    assert!(
                        (p.x - v[7]).abs() < 0.0002 && (p.y - v[8]).abs() < 0.0002,
                        "{line}: {p:?}"
                    );
                }
                states += 1;
            }
            "point" => {
                let p = store.local_to_global(id, Point::new(v[0], v[1])).unwrap();
                assert!(
                    (p.x - v[2]).abs() < 0.0003 && (p.y - v[3]).abs() < 0.0003,
                    "{line}: {p:?}"
                );
                let local = store.global_to_local(id, p).unwrap();
                assert!(
                    (local.x - v[0]).abs() < 0.0003 && (local.y - v[1]).abs() < 0.0003,
                    "{line}: {local:?}"
                );
            }
            other => panic!("{other}"),
        }
    }
    assert_eq!(states, 87);
}

#[test]
fn changed_flip_captures_size_gear_and_unchanged_flip_keeps_custom_scale_and_dirty_state() {
    let mut store = ObjectStore::new();
    let root = store.create(ObjectType::Component);
    store
        .object_mut(root)
        .unwrap()
        .control
        .component_mut()
        .unwrap()
        .controllers
        .push(Controller {
            page_ids: vec!["a".into(), "b".into()],
            page_names: vec!["A".into(), "B".into()],
            selected_index: 0,
            ..Default::default()
        });
    let image = store.create(ObjectType::Image);
    store.set_size(image, 100., 60.);
    store.set_parent(image, Some(root));
    let mut gear = Gear::new(GearKind::Size);
    gear.controller_index = 0;
    gear.controller_owner = Some(root);
    gear.values.insert(
        None,
        GearValue::Size {
            width: 100.,
            height: 60.,
            scale_x: 1.,
            scale_y: 1.,
        },
    );
    store.object_mut(image).unwrap().gears.push(gear);
    store.set_image_flip(image, FlipType::Horizontal);
    assert_eq!(
        store.object(image).unwrap().gears[0].values[&Some("a".into())],
        GearValue::Size {
            width: 100.,
            height: 60.,
            scale_x: -1.,
            scale_y: 1.
        }
    );
    store.set_scale(image, 2., 0.5);
    store.take_dirty();
    store.set_image_flip(image, FlipType::Horizontal);
    assert_eq!(
        (
            store.object(image).unwrap().scale_x(),
            store.object(image).unwrap().scale_y()
        ),
        (2., 0.5)
    );
    assert!(store.take_dirty().is_empty());
    store.set_controller_page_index(root, "", 1);
    store.set_controller_page_index(root, "", 0);
    assert_eq!(
        (
            store.object(image).unwrap().scale_x(),
            store.object(image).unwrap().scale_y()
        ),
        (2., 0.5)
    );
    assert_eq!(store.image_flip(image), Some(FlipType::Horizontal));
    store.set_image_flip(root, FlipType::Both);
    assert_eq!(store.image_flip(root), None);
}

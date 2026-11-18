use super::*;
use crate::{package::ObjectType, widget::FlipType};

#[test]
fn snapping_matches_native_rounding_order_anchor_and_cached_image_refreshes() {
    let (mut cases, mut snapshots) = (0, 0);
    let mut current = None;
    for line in include_str!("../../tests/fixtures/pixel-snapping-reference.txt").lines() {
        if line.starts_with('#') {
            continue;
        }
        let words: Vec<_> = line.split_whitespace().collect();
        if words[0] == "case" {
            let v: Vec<f32> = words[1..].iter().map(|s| s.parse().unwrap()).collect();
            let mut store = ObjectStore::new();
            let id = store.create(if v[0] == 1. {
                ObjectType::Image
            } else {
                ObjectType::Component
            });
            store.set_size(id, 99., 61.);
            store.set_xy(id, 0.25, -0.5);
            store.set_pivot(id, 0.25, 0.75, v[1] == 1.);
            if v[0] == 1. {
                store.set_image_flip(id, FlipType::Horizontal);
            }
            store.set_scale(id, v[2], v[3]);
            store.set_rotation(id, v[4]);
            store.set_skew(id, v[5], v[6]);
            current = Some((store, id));
            cases += 1;
            continue;
        }
        let (command, expected) = line.split_once(':').unwrap();
        let command: Vec<_> = command.split_whitespace().collect();
        let v: Vec<f32> = command[1..].iter().map(|s| s.parse().unwrap()).collect();
        let (store, id) = current.as_mut().unwrap();
        let id = *id;
        match command[0] {
            "initial" => (),
            "snap" => {
                store.set_pixel_snapping(id, v[0] == 1.);
            }
            "xy" => store.set_xy(id, v[0], v[1]),
            "pivot" => store.set_pivot(id, v[0], v[1], v[2] == 1.),
            "size" => store.set_size(id, v[0], v[1]),
            "scale" => store.set_scale(id, v[0], v[1]),
            _ => panic!("{line}"),
        }
        let o = store.object(id).unwrap();
        let p = o.display_position().unwrap_or_else(|| {
            let p = o.display_base_position();
            let (px, py) = (
                f64::from(o.pivot().x * o.width()),
                f64::from(o.pivot().y * o.height()),
            );
            let [a, b, c, d] = o.transform_axes();
            Point::new(
                (f64::from(p.x) + px - a * px - c * py) as f32,
                (f64::from(p.y) + py - b * px - d * py) as f32,
            )
        });
        let mut actual = vec![
            f64::from(u8::from(o.pixel_snapping())),
            f64::from(o.x()),
            f64::from(o.y()),
            f64::from(o.width()),
            f64::from(o.height()),
            f64::from(p.x),
            f64::from(p.y),
        ];
        for p in [
            Point::ZERO,
            Point::new(13., 9.),
            Point::new(o.width(), o.height()),
        ] {
            let p = store.local_to_global(id, p).unwrap();
            actual.extend([f64::from(p.x), f64::from(p.y)]);
        }
        let expected: Vec<f64> = expected
            .split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect();
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!(
                (actual - expected).abs() < 0.00004,
                "case {cases} {line}: {actual} != {expected}"
            );
        }
        snapshots += 1;
    }
    assert_eq!((cases, snapshots), (12, 252));
}

#[test]
fn pixel_snapping_changes_display_leaving_logical_and_child_geometry_intact() {
    let mut store = ObjectStore::new();
    let owner = store.create(ObjectType::Component);
    let other = store.create(ObjectType::Component);
    store.set_size(owner, 31.25, 19.75);
    store.set_xy(owner, -0.5, -1.5);
    store.set_xy(other, 17.25, 20.5);
    store.set_parent(other, Some(owner));
    store.take_dirty();
    assert!(store.set_pixel_snapping(owner, true));
    assert_eq!(
        store.object(owner).unwrap().position(),
        Point::new(-0.5, -1.5)
    );
    assert_eq!(
        store.object(owner).unwrap().size(),
        crate::Size::new(31.25, 19.75)
    );
    assert_eq!(
        store.object(other).unwrap().position(),
        Point::new(17.25, 20.5)
    );
    assert_eq!(
        store.local_to_global(owner, Point::ZERO),
        Some(Point::new(0., -1.))
    );
    let dirty = store.take_dirty();
    assert_eq!(dirty, [(owner, super::dirty::LOOK)]);
    assert!(!store.set_pixel_snapping(owner, true));
    assert!(store.take_dirty().is_empty());
}

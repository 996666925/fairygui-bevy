use super::*;
use crate::widget::relation::{RelationDef, RelationItem};

#[test]
fn size_limits_pivots_and_self_relations_match_original_runtime_methods() {
    let mut store = ObjectStore::new();
    let mut id = None;
    let mut states = 0;
    for line in include_str!("../../tests/fixtures/size-constraints-reference.txt").lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let values: Vec<f32> = line
            .split_whitespace()
            .skip(1)
            .map(|v| v.parse().unwrap())
            .collect();
        if line.starts_with("case ") {
            store = ObjectStore::new();
            let parent = store.create(ObjectType::Component);
            let child = store.create(ObjectType::Component);
            store.set_parent(child, Some(parent));
            store.set_xy(child, 120., 80.);
            store.set_pivot(child, values[1], values[1], values[0] == 1.);
            let o = store.object_mut(child).unwrap();
            if values[3] == 1. {
                (o.min_width, o.min_height, o.max_width, o.max_height) = (30., 20., 180., 90.);
            }
            let kind = values[2] as u8;
            if kind != 0 {
                o.relations.items.push(RelationItem {
                    to_parent: true,
                    to_child: false,
                    target_index: None,
                    binding: None,
                    defs: vec![
                        RelationDef {
                            relation_type: kind,
                            percent: false,
                            axis: 0,
                        },
                        RelationDef {
                            relation_type: if kind == 3 { 10 } else { 13 },
                            percent: false,
                            axis: 1,
                        },
                    ],
                });
            }
            id = Some(child);
        } else {
            let id = id.unwrap();
            match line.split_whitespace().next().unwrap() {
                "width" => store.set_width(id, values[0]),
                "height" => store.set_height(id, values[0]),
                _ => store.set_size_internal(id, values[0], values[1], values[2] == 1., false),
            }
            let o = store.object(id).unwrap();
            let raw = o.requested_size();
            assert_eq!(
                [o.x(), o.y(), o.width(), o.height(), raw.width, raw.height],
                values[3..],
                "{line}"
            );
            states += 1;
        }
    }
    assert_eq!(states, 396);
}

#[test]
fn updating_constraints_restores_the_request_and_notifies_dependents_with_effective_deltas() {
    let mut store = ObjectStore::new();
    let root = store.create(ObjectType::Component);
    store.set_size(root, 100., 60.);
    let child = store.create(ObjectType::Component);
    store.set_parent(child, Some(root));
    store.set_xy(child, 80., 40.);
    store
        .object_mut(child)
        .unwrap()
        .relations
        .items
        .push(RelationItem {
            to_parent: true,
            to_child: false,
            target_index: None,
            binding: None,
            defs: vec![RelationDef {
                relation_type: 6,
                percent: false,
                axis: 0,
            }],
        });
    let limits = SizeConstraints {
        min: Size::new(50., 30.),
        max: Size::new(200., 100.),
    };
    assert!(store.set_size_constraints(root, limits));
    store.set_size(root, 300., 150.);
    assert_eq!(store.object(root).unwrap().size(), Size::new(200., 100.));
    assert_eq!(store.object(child).unwrap().x(), 180.);
    store.set_width(root, 400.);
    assert_eq!(
        store.object(root).unwrap().requested_size(),
        Size::new(400., 150.)
    );
    assert_eq!(store.object(child).unwrap().x(), 180.);
    assert!(store.set_size_constraints(root, SizeConstraints::default()));
    assert_eq!(store.object(root).unwrap().size(), Size::new(400., 150.));
    assert_eq!(store.object(child).unwrap().x(), 380.);
    store.take_dirty();
    store.set_size(root, 400., 150.);
    assert!(store.take_dirty().is_empty());
}

#[test]
fn invalid_constraint_and_size_writes_leave_finite_geometry_unchanged() {
    let mut store = ObjectStore::new();
    let id = store.create(ObjectType::Graph);
    store.set_size(id, 100., 60.);
    for min in [
        Size::new(-1., 0.),
        Size::new(f32::NAN, 0.),
        Size::new(f32::INFINITY, 0.),
    ] {
        assert!(!store.set_size_constraints(
            id,
            SizeConstraints {
                min,
                ..Default::default()
            }
        ));
    }
    assert!(!store.set_size_constraints(
        id,
        SizeConstraints {
            min: Size::new(100., 0.),
            max: Size::new(50., 0.)
        }
    ));
    store.set_size(id, f32::NAN, 70.);
    store.set_height(id, f32::INFINITY);
    assert_eq!(store.object(id).unwrap().size(), Size::new(100., 60.));
    store.set_size(id, -10., -5.);
    assert_eq!(store.object(id).unwrap().size(), Size::ZERO);
    assert_eq!(
        store.object(id).unwrap().requested_size(),
        Size::new(-10., -5.)
    );
}

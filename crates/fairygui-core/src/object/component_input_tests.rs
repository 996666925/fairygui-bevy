use super::*;
use crate::{
    Point,
    package::ObjectType,
    widget::{GraphData, HitTestArea},
};

#[test]
fn opaque_mask_and_same_value_sequences_match_original_component_and_input_methods() {
    let mut store = ObjectStore::new();
    let mut owner = store.create(ObjectType::Component);
    let mut mask = store.create(ObjectType::Graph);
    let mut states = 0;
    let mut hits = 0;
    for line in include_str!("../../tests/fixtures/opaque-reference.txt").lines() {
        let p: Vec<_> = line.split_whitespace().collect();
        if p.is_empty() || p[0].starts_with('#') {
            continue;
        }
        match p[0] {
            "new" => {
                store = ObjectStore::new();
                owner = store.create(ObjectType::Component);
                mask = store.create(ObjectType::Graph);
                store.set_size(owner, 100., 80.);
                store.set_size(mask, 40., 30.);
                store.set_xy(mask, 10., 15.);
                store.set_graph(
                    mask,
                    GraphData {
                        graph_type: 1,
                        ..Default::default()
                    },
                );
            }
            "opaque" => {
                store.set_component_opaque(owner, p[1] == "1");
            }
            "mask" => {
                store.set_component_mask(owner, Some(mask), p[1] == "1");
            }
            "clear" => {
                store.set_component_mask(owner, None, false);
            }
            "state" => {
                let o = store.object(owner).unwrap();
                assert_eq!(
                    o.control().component().unwrap().opaque,
                    p[1] == "1",
                    "{line}"
                );
                assert_eq!(o.component_mouse_through(), Some(p[2] == "1"), "{line}");
                assert_eq!(o.component_hit_test_prior(), p[3] == "1", "{line}");
                states += 1;
            }
            "hit" => {
                let x: f32 = p[1].parse().unwrap();
                let y: f32 = p[2].parse().unwrap();
                let o = store.object(owner).unwrap();
                let c = o.control().component().unwrap();
                let actual = if let Some(HitTestArea::MaskChild { reversed, .. }) = &c.hit_test {
                    let data = store.object(mask).unwrap().control();
                    let crate::widget::ControlData::Graph(data) = data else {
                        panic!()
                    };
                    crate::graph::graph_contains(
                        data,
                        store.object(mask).unwrap().size(),
                        Point::new(x - 10., y - 15.),
                    ) != *reversed
                } else {
                    o.component_has_self_hit() == Some(true)
                        && x >= 0.
                        && y >= 0.
                        && x < o.width()
                        && y < o.height()
                };
                assert_eq!(actual, p[3] == "1", "{line}, {o:?}");
                hits += 1;
            }
            _ => panic!("{line}"),
        }
    }
    assert_eq!((states, hits), (24, 120));
}

#[test]
fn opaque_only_changes_input_state_and_preserves_children_alpha_and_custom_areas() {
    let mut store = ObjectStore::new();
    let owner = store.create(ObjectType::Component);
    let child = store.create(ObjectType::Graph);
    store.set_parent(child, Some(owner));
    store.set_alpha(owner, 0.3);
    store.take_dirty();
    assert!(!store.set_component_opaque(owner, false));
    assert!(store.take_dirty().is_empty());
    assert!(store.set_component_opaque(owner, true));
    assert_eq!(store.take_dirty(), vec![(owner, dirty::LOOK)]);
    assert_eq!(store.object(owner).unwrap().alpha(), 0.3);
    assert!(store.object(child).unwrap().touchable());
    assert_eq!(store.object(child).unwrap().parent(), Some(owner));
    store
        .object_mut(owner)
        .unwrap()
        .control
        .component_mut()
        .unwrap()
        .hit_test = Some(HitTestArea::Pixel {
        item_id: "shape".into(),
        offset_x: 3,
        offset_y: 4,
    });
    assert!(store.set_component_opaque(owner, false));
    assert!(matches!(
        store
            .object(owner)
            .unwrap()
            .control()
            .component()
            .unwrap()
            .hit_test,
        Some(HitTestArea::Pixel { .. })
    ));
    assert_eq!(
        store.object(owner).unwrap().component_has_self_hit(),
        Some(true)
    );
    assert!(!store.set_component_opaque(child, true));
}

#[test]
fn clearing_exported_child_hit_area_retains_prior_and_non_through_flags() {
    let mut store = ObjectStore::new();
    let owner = store.create(ObjectType::Component);
    store
        .object_mut(owner)
        .unwrap()
        .control
        .component_mut()
        .unwrap()
        .hit_test = Some(HitTestArea::Child { child_index: 0 });
    assert_eq!(
        store.object(owner).unwrap().component_mouse_through(),
        Some(false)
    );
    store.set_component_mask(owner, None, false);
    let object = store.object(owner).unwrap();
    assert!(object.control().component().unwrap().hit_test.is_none());
    assert_eq!(object.component_mouse_through(), Some(false));
    assert!(object.component_hit_test_prior());
    assert!(!store.set_component_opaque(owner, false));
    assert_eq!(
        store.object(owner).unwrap().component_has_self_hit(),
        Some(true)
    );
}

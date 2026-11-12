use super::*;

#[test]
fn set_mask_matches_native_cache_blend_parent_and_hit_area_side_effects() {
    let mut state = None;
    let mut count = 0;
    let mode = |s: &str| match s {
        "add" => BlendMode::Add,
        "destinationOut" => BlendMode::Erase,
        _ => BlendMode::Normal,
    };
    for line in include_str!("../../tests/fixtures/mask-reference.txt").lines() {
        let p: Vec<_> = line.split_whitespace().collect();
        match p.first().copied() {
            Some("case") => {
                let mut store = ObjectStore::new();
                let owner = store.create(ObjectType::Component);
                let a = store.create(if p[1] == "1" {
                    ObjectType::Graph
                } else {
                    ObjectType::Image
                });
                let b = store.create(if p[2] == "1" {
                    ObjectType::Graph
                } else {
                    ObjectType::Image
                });
                for id in [a, b] {
                    store.set_parent(id, Some(owner));
                }
                store.set_blend_mode(a, mode(p[3]));
                store.set_blend_mode(b, BlendMode::Add);
                state = Some((store, owner, a, b));
            }
            Some("step") => {
                let (store, owner, a, b) = state.as_mut().unwrap();
                let id = |s: &str| match s {
                    "a" => Some(*a),
                    "b" => Some(*b),
                    _ => None,
                };
                assert!(store.set_component_mask(*owner, id(p[1]), p[2] == "1"));
                assert_eq!(
                    store.component_mask(*owner).map(|v| v.0),
                    id(p[3]),
                    "{line}"
                );
                let actual = store.component_mask(*owner).filter(|v| !v.1).map(|v| v.0);
                assert_eq!(actual, id(p[4]), "{line}");
                assert_eq!(
                    store.object(*owner).unwrap().cache_as_bitmap(),
                    p[5] == "bitmap"
                );
                for (child, start) in [(*a, 6), (*b, 9)] {
                    let object = store.object(child).unwrap();
                    assert_eq!(object.blend_mode_is_set(), p[start] != "inherit", "{line}");
                    assert_eq!(object.blend_mode(), mode(p[start]), "{line}");
                    assert_eq!(object.cache_as_bitmap(), p[start + 1] == "bitmap", "{line}");
                    assert_eq!(
                        object.mask_parent(),
                        (p[start + 2] == "owner").then_some(*owner),
                        "{line}"
                    );
                }
                let area = store
                    .object(*owner)
                    .unwrap()
                    .control()
                    .component()
                    .unwrap()
                    .hit_test
                    .as_ref();
                let actual = match area {
                    Some(HitTestArea::MaskChild { object, reversed }) => Some((*object, *reversed)),
                    _ => None,
                };
                assert_eq!(actual, id(p[12]).map(|id| (id, p[13] == "1")), "{line}");
                count += 1;
            }
            _ => (),
        }
    }
    assert_eq!(count, 132);
}

#[test]
fn exported_mask_reverse_byte_precedes_pixel_hit_fields_and_resolves_stable_children() {
    let package =
        crate::FuiPackage::parse("Masks", include_bytes!("../../tests/fixtures/Masks.fui"))
            .unwrap();
    for (name, reversed) in [("Demo_Graph", false), ("Demo_Image", true)] {
        let mut store = ObjectStore::new();
        let index = package.item_index_by_name(name).unwrap();
        let root = crate::widget::create_from_package(
            &mut store,
            &package,
            index,
            &crate::package::LoadContext::default(),
        )
        .unwrap();
        let first = store.object(root).unwrap().children()[0];
        assert_eq!(store.component_mask(root), Some((first, reversed)));
        let data = store.object(root).unwrap().control().component().unwrap();
        assert!(
            matches!(&data.hit_test,Some(HitTestArea::Pixel {item_id,offset_x:17,offset_y:-23})if item_id=="t9fj54")
        );
        store.add_child_at(root, first, usize::MAX);
        assert_eq!(store.component_mask(root), Some((first, reversed)));
        store.set_parent(first, None);
        assert_eq!(store.component_mask(root), Some((first, reversed)));
        store.remove(first);
        // Native retains a destroyed mask reference (empty graphics), until the
        // caller clears it. Generational handles cannot alias a replacement.
        assert_eq!(store.component_mask(root), Some((first, reversed)));
        assert!(!store.contains(first));
        assert!(store.set_component_mask(root, None, false));
        assert_eq!(store.component_mask(root), None);
    }
}

#[test]
fn invalid_mask_assignment_is_atomic_and_bitmap_cache_survives_reverse_clear() {
    let mut store = ObjectStore::new();
    let root = store.create(ObjectType::Component);
    let child = store.create(ObjectType::Component);
    let mask = store.create(ObjectType::Graph);
    store.set_parent(child, Some(root));
    store.set_parent(mask, Some(child));
    assert!(store.set_component_mask(child, Some(mask), true));
    assert!(!store.set_component_mask(child, Some(root), false));
    assert!(!store.set_component_mask(child, Some(child), false));
    assert_eq!(store.component_mask(child), Some((mask, true)));
    store.set_component_mask(child, None, false);
    assert!(store.object(child).unwrap().cache_as_bitmap());
    assert!(!store.object(mask).unwrap().blend_mode_is_set());
    store.set_cache_as_bitmap(child, false);
    assert!(!store.object(child).unwrap().cache_as_bitmap());
}

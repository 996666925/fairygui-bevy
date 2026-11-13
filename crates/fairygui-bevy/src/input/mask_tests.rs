use super::*;
use bevy::math::{Affine2, Mat2};
use fairygui_core::{Size, package::ObjectType};

#[test]
fn graphic_hit_gate_matches_native_child_hit_area_including_unsupported_paths() {
    let mut data = GraphData::default();
    let mut size = Size::default();
    let mut origin = Vec2::ZERO;
    let mut reversed = false;
    let mut cases = 0;
    let mut hits = 0;
    for line in include_str!("../../../fairygui-core/tests/fixtures/mask-hit-reference.txt").lines()
    {
        let p: Vec<_> = line.split_whitespace().collect();
        match p.first().copied() {
            Some("case") => {
                data = GraphData {
                    graph_type: p[1].parse().unwrap(),
                    corner_radius: match p[2] {
                        "0" => None,
                        "1" => Some([5.; 4]),
                        _ => Some([0.; 4]),
                    },
                    polygon_points: Some(vec![
                        0., 0., 80., 0., 80., 15., 25., 15., 25., 40., 0., 40.,
                    ]),
                    sides: 5,
                    ..default()
                };
                size = Size::new(p[3].parse().unwrap(), p[4].parse().unwrap());
                reversed = p[5] == "1";
                cases += 1;
            }
            Some("origin") => {
                origin = Vec2::new(p[1].parse().unwrap(), p[2].parse().unwrap());
            }
            Some("hit") => {
                let point = Vec2::new(p[1].parse().unwrap(), p[2].parse().unwrap());
                assert_eq!(
                    graph_contains(&data, size, point - origin) != reversed,
                    p[3] == "1",
                    "{line}, graph={data:?} origin={origin:?}"
                );
                hits += 1;
            }
            _ => (),
        }
    }
    assert_eq!((cases, hits), (36, 1764));
}

#[test]
fn mask_prior_gate_uses_parent_coordinates_and_retains_graph_hit_when_replaced_by_image() {
    for dpi in [1., 1.75, 2.] {
        let mut gui = GuiRuntime::default();
        let owner = gui.store_mut().create(ObjectType::Component);
        let mask = gui.store_mut().create(ObjectType::Graph);
        let image = gui.store_mut().create(ObjectType::Image);
        let content = gui.store_mut().create(ObjectType::Graph);
        for id in [mask, image, content] {
            gui.store_mut().set_parent(id, Some(owner));
        }
        gui.store_mut().set_size(owner, 180., 110.);
        gui.store_mut().set_size(mask, 80., 40.);
        gui.store_mut().set_graph(
            mask,
            GraphData {
                graph_type: 1,
                ..default()
            },
        );
        gui.store_mut().set_component_mask(owner, Some(mask), false);
        let owner_matrix = Mat2::from_angle(0.3) * Mat2::from_diagonal(Vec2::new(1.2, 0.8));
        let owner_origin = Vec2::new(100., 70.) * dpi;
        let node = |size: Vec2| ComputedNode {
            size: size * dpi,
            inverse_scale_factor: 1. / dpi,
            ..default()
        };
        let owner_n = node(Vec2::new(180., 110.));
        let owner_t = UiGlobalTransform::from(Affine2 {
            matrix2: owner_matrix,
            translation: owner_origin + owner_matrix * (owner_n.size * 0.5),
        });
        // Native ChildHitArea uses only toParentPoint(0,0). Scale/rotation affect
        // that origin, without rotating or scaling the hit graphics themselves.
        let mask_matrix = Mat2::from_angle(0.7) * Mat2::from_diagonal(Vec2::new(0.5, 2.));
        let mask_origin = Vec2::new(30., 20.);
        let mask_n = node(Vec2::new(80., 40.));
        let mask_t = UiGlobalTransform::from(Affine2 {
            matrix2: owner_matrix * mask_matrix,
            translation: owner_origin
                + owner_matrix * (mask_origin * dpi + mask_matrix * (mask_n.size * 0.5)),
        });
        let geometry = |id| {
            if id == owner {
                Some((&owner_t, &owner_n))
            } else if id == mask {
                Some((&mask_t, &mask_n))
            } else {
                None
            }
        };
        for (local, inside) in [
            (Vec2::new(40., 35.), true),
            (Vec2::new(10., 10.), false),
            (Vec2::new(100., 50.), true),
            (Vec2::new(120., 65.), false),
        ] {
            let world = owner_origin + owner_matrix * (local * dpi);
            assert_eq!(allowed_geometry(&gui, content, world, geometry), inside);
        }
        gui.store_mut()
            .set_component_mask(owner, Some(image), false);
        gui.store_mut().set_visible(mask, false);
        assert!(!allowed_geometry(
            &gui,
            content,
            owner_origin + owner_matrix * (Vec2::new(10., 10.) * dpi),
            geometry
        ));
        gui.store_mut().set_component_mask(owner, None, false);
        assert!(allowed_geometry(
            &gui,
            content,
            owner_origin + owner_matrix * (Vec2::new(10., 10.) * dpi),
            geometry
        ));
        // Clear retains hitTestPrior and mouseThrough=false, now testing the
        // owner's rectangle before descendants, including children outside it.
        let outside = owner_origin + owner_matrix * (Vec2::new(200., 50.) * dpi);
        assert!(!allowed_geometry(&gui, content, outside, geometry));
        gui.store_mut().set_component_opaque(owner, true);
        gui.store_mut().set_component_opaque(owner, false);
        assert!(allowed_geometry(&gui, content, outside, geometry));
        gui.store_mut().set_component_mask(owner, Some(mask), false);
        gui.store_mut().set_component_opaque(owner, true);
        gui.store_mut().set_component_opaque(owner, false);
        let cutout = owner_origin + owner_matrix * (Vec2::new(10., 10.) * dpi);
        assert!(allowed_geometry(&gui, content, cutout, geometry));
        assert!(!allowed_geometry(&gui, owner, cutout, geometry));
    }
}

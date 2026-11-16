use super::*;
use bevy::{
    ecs::entity::EntityIndexMap,
    math::Affine2,
    sprite::BorderRect,
    text::RunGeometry,
    ui::{CalculatedClip, ResolvedBorderRadius},
    ui_render::NodeType,
};

fn rectangle(position: Vec2, size: Vec2, z_order: f32, base: Affine2) -> ExtractedUiNode {
    ExtractedUiNode {
        z_order,
        image: AssetId::default(),
        clip: Some(CalculatedClip::default()),
        transform: base * Affine2::from_translation(position),
        item: ExtractedUiItem::Node {
            color: LinearRgba::new(0.1, 0.7, 0.2, 0.3),
            rect: Rect::from_corners(Vec2::ZERO, size),
            atlas_scaling: None,
            flip_x: false,
            flip_y: false,
            border: BorderRect::ZERO,
            border_radius: ResolvedBorderRadius::ZERO,
            node_type: NodeType::Rect,
        },
    }
}

fn rect(node: &ExtractedUiNode) -> Rect {
    let ExtractedUiItem::Node { rect, .. } = &node.item else {
        panic!("rectangle");
    };
    *rect
}

#[test]
fn projected_draw_rectangles_match_108_unmodified_native_command_pairs_at_fractional_scales() {
    let mut cases = 0;
    for line in include_str!("../tests/fixtures/text-decoration-reference.txt")
        .lines()
        .filter(|line| !line.starts_with('#'))
    {
        let values: Vec<f32> = line
            .split_whitespace()
            .map(|n| n.parse().unwrap())
            .collect();
        let [x, y, width, height, size, ux, uy, uw, uh, sx, sy, sw, sh] = values.as_slice() else {
            panic!("fixture record");
        };
        for scale in [0.7, 1., 1.25, 1.3, 1.75, 2.] {
            let original = Rect::new(
                x * scale,
                y * scale,
                (x + width) * scale,
                (y + height) * scale,
            );
            let mut run = RunGeometry {
                bounds: original,
                ..default()
            };
            project_geometry(&mut run, *y, *height, *size, scale);
            assert_eq!(
                run.bounds, original,
                "paint cannot expand links/backgrounds"
            );
            let under = Rect::from_center_size(run.underline_position(), run.underline_size());
            assert!(
                under.min.abs_diff_eq(Vec2::new(*ux, *uy) * scale, 0.001),
                "{line}: {under:?}"
            );
            assert!(under.size().abs_diff_eq(Vec2::new(*uw, *uh) * scale, 0.001));
            let mut items = EntityIndexMap::default();
            let id = Entity::from_raw_u32(1).unwrap();
            items.insert(
                id,
                rectangle(
                    run.strikethrough_position(),
                    run.strikethrough_size(),
                    2.07,
                    Affine2::IDENTITY,
                ),
            );
            extend_strike(&mut items, &run, Affine2::IDENTITY, 2.07, scale);
            let node = &items[&id];
            let actual = Rect::from_center_size(node.transform.translation, rect(node).size());
            assert!(
                actual.min.abs_diff_eq(Vec2::new(*sx, *sy) * scale, 0.001),
                "{line}: {actual:?}"
            );
            assert!(
                actual
                    .size()
                    .abs_diff_eq(Vec2::new(*sw, *sh) * scale, 0.001)
            );
        }
        cases += 1;
    }
    assert_eq!(cases, 108);
}

#[test]
fn extension_preserves_shadow_background_underline_color_clip_transform_and_duplicate_commands() {
    for scale in [0.7, 1.3, 2.] {
        let transform =
            Affine2::from_scale_angle_translation(Vec2::new(-1.2, 0.7), 0.6, Vec2::new(120., 90.));
        let mut run = RunGeometry {
            bounds: Rect::new(3. * scale, 0., 43. * scale, 29. * scale),
            ..default()
        };
        project_geometry(&mut run, 0., 29., 24., scale);
        let z = 4.07;
        let mut world = World::new();
        let ids: Vec<_> = (0..5).map(|_| world.spawn_empty().id()).collect();
        let mut items = EntityIndexMap::default();
        // Two identical commands still represent two native drawLine calls.
        for &id in &ids[..2] {
            items.insert(
                id,
                rectangle(
                    run.strikethrough_position(),
                    run.strikethrough_size(),
                    z,
                    transform,
                ),
            );
        }
        items.insert(
            ids[2],
            rectangle(
                run.strikethrough_position(),
                run.strikethrough_size(),
                4.02,
                transform,
            ),
        );
        items.insert(
            ids[3],
            rectangle(run.bounds.center(), run.bounds.size(), 4.06, transform),
        );
        items.insert(
            ids[4],
            rectangle(run.underline_position(), run.underline_size(), z, transform),
        );
        let snapshots: Vec<_> = ids
            .iter()
            .map(|id| (items[id].transform, rect(&items[id]), items[id].z_order))
            .collect();
        for _ in 0..2 {
            extend_strike(&mut items, &run, transform, z, scale);
        }
        for (i, id) in ids.iter().enumerate() {
            let node = &items[id];
            assert_eq!(node.transform, snapshots[i].0);
            assert_eq!(node.z_order, snapshots[i].2);
            assert!(node.clip.is_some());
            let ExtractedUiItem::Node { color, .. } = node.item else {
                panic!("rectangle");
            };
            assert_eq!(color, LinearRgba::new(0.1, 0.7, 0.2, 0.3));
            let expected = snapshots[i].1.size()
                + if i < 2 {
                    Vec2::new(8. * scale, 0.)
                } else {
                    Vec2::ZERO
                };
            assert!(rect(node).size().abs_diff_eq(expected, 0.001));
        }
    }
}

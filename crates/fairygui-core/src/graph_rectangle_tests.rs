use super::*;
use crate::color::Color;

#[test]
fn rectangle_fill_and_ordered_stroke_quads_match_original_graphics_commands() {
    let mut cases = 0;
    for line in include_str!("../tests/fixtures/rectangle-reference.txt").lines() {
        if line.starts_with('#') {
            continue;
        }
        let (case, expected) = line.split_once(" |").unwrap();
        let v: Vec<f32> = case
            .split_whitespace()
            .skip(1)
            .map(|v| v.parse().unwrap())
            .collect();
        let drawing = rectangle_drawing(
            &GraphData {
                graph_type: 1,
                line_size: v[2] as i32,
                fill_color: if v[3] == 1. {
                    Color::WHITE
                } else {
                    Color::TRANSPARENT
                },
                line_color: Color::new(1., 1., 1., v[4]),
                ..Default::default()
            },
            Size::new(v[0], v[1]),
        );
        let mut actual = Vec::new();
        if let Some(drawing) = drawing {
            if let Some(fill) = drawing.fill {
                actual.extend([0., fill.x, fill.y, fill.width, fill.height]);
            }
            if let Some(stroke) = drawing.stroke {
                for q in stroke {
                    actual.extend([1., q.x, q.y, q.width, q.height]);
                }
            }
        }
        let expected: Vec<f32> = expected
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        assert_eq!(actual, expected, "{case}");
        cases += 1;
    }
    assert_eq!(cases, 100);
}

#[test]
fn rounded_rectangles_keep_the_draw_path_policy() {
    assert!(
        rectangle_drawing(
            &GraphData {
                graph_type: 1,
                corner_radius: Some([0.; 4]),
                ..Default::default()
            },
            Size::new(100., 80.)
        )
        .is_none()
    );
}

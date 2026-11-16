use super::{PolygonPoint, polygon_draw_offset, polygon_points};
use crate::{Size, widget::GraphData};

/// DrawPolyCmd's drawing path after pixel alignment and explicit closure.
/// HitArea uses the original unshifted vertices instead of this drawing path.
#[derive(Debug, Clone, PartialEq)]
pub struct PolygonDrawing {
    pub points: Vec<PolygonPoint>,
    pub convex: bool,
}

pub fn polygon_drawing(data: &GraphData, size: Size) -> Option<PolygonDrawing> {
    let mut points = polygon_points(data, size);
    if points.is_empty() {
        return None;
    }
    let convex = points.len() <= 3;
    let offset = f64::from(polygon_draw_offset(data));
    for p in &mut points {
        p.x += offset;
        p.y += offset;
    }
    if points.len() > 2 && points.last() != points.first() {
        points.push(points[0]);
    }
    Some(PolygonDrawing { points, convex })
}

/// GraphicsRunner.fill uses a triangle fan for convex paths, including circles
/// and DrawPoly commands with at most three original vertices. Other paths use
/// Earcut. Non-convex paths with only three vertices are skipped by the renderer.
pub fn path_fill(points: &[PolygonPoint], convex: bool) -> Vec<usize> {
    if points.len() < 3
        || (points.len() == 3 && !convex)
        || points.iter().any(|p| !p.x.is_finite() || !p.y.is_finite())
    {
        return Vec::new();
    }
    if convex {
        return (1..points.len() - 1).flat_map(|i| [0, i, i + 1]).collect();
    }
    let coordinates: Vec<f64> = points.iter().flat_map(|p| [p.x, p.y]).collect();
    earcutr::earcut(&coordinates, &[], 2).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Point,
        color::Color,
        graph::{graph_contains, polygon_stroke},
    };

    fn layers(points: &[PolygonPoint], indices: &[usize], p: PolygonPoint) -> usize {
        indices
            .as_chunks::<3>()
            .0
            .iter()
            .filter(|t| {
                let (a, b, c) = (points[t[0]], points[t[1]], points[t[2]]);
                let cross = |a: PolygonPoint, b: PolygonPoint, c: PolygonPoint| {
                    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
                };
                if cross(a, b, c).abs() < 1e-12 {
                    return false;
                }
                let (x, y, z) = (cross(a, b, p), cross(b, c, p), cross(c, a, p));
                (x >= 0. && y >= 0. && z >= 0.) || (x <= 0. && y <= 0. && z <= 0.)
            })
            .count()
    }

    #[test]
    fn drawing_paths_fills_strokes_and_hits_match_original_polygon_commands() {
        let mut data = GraphData::default();
        let mut size = Size::ZERO;
        let mut name = "";
        let mut drawing = None;
        let mut cases = 0;
        let mut differing_fill_hits = 0;
        for line in include_str!("../../tests/fixtures/polygon-fill-reference.txt").lines() {
            if line.starts_with('#') {
                continue;
            }
            let mut parts = line.split_whitespace();
            let kind = parts.next().unwrap();
            if kind == "case" {
                name = parts.next().unwrap();
                let v: Vec<f32> = parts.map(|v| v.parse().unwrap()).collect();
                size = Size::new(v[1], v[2]);
                data = GraphData {
                    graph_type: v[0] as i32,
                    line_size: v[3] as i32,
                    sides: v[4] as i32,
                    start_angle: v[5],
                    fill_color: if v[6] != 0. {
                        Color::WHITE
                    } else {
                        Color::TRANSPARENT
                    },
                    line_color: Color::new(1., 1., 1., v[7]),
                    ..Default::default()
                };
                cases += 1;
                continue;
            }
            let v: Vec<f64> = parts.map(|v| v.parse().unwrap()).collect();
            match kind {
                "raw" => data.polygon_points = Some(v.iter().map(|v| *v as f32).collect()),
                "dist" => {
                    data.distances = (!v.is_empty()).then(|| v.iter().map(|v| *v as f32).collect())
                }
                "points" => {
                    drawing = polygon_drawing(&data, size);
                    let actual: Vec<f64> = drawing.as_ref().map_or_else(Vec::new, |d| {
                        d.points.iter().flat_map(|p| [p.x, p.y]).collect()
                    });
                    assert_eq!(actual.len(), v.len(), "{name}: points");
                    for (a, b) in actual.iter().zip(v) {
                        assert!((a - b).abs() < 1e-6, "{name}: {a} != {b}");
                    }
                }
                "convex" => assert_eq!(
                    drawing.as_ref().is_some_and(|d| d.convex),
                    v[0] != 0.,
                    "{name}: fan policy"
                ),
                "fill_vertices" => {
                    let actual: Vec<f64> = drawing
                        .as_ref()
                        .filter(|_| data.has_fill())
                        .map_or_else(Vec::new, |d| {
                            d.points.iter().flat_map(|p| [p.x, p.y]).collect()
                        });
                    if !v.is_empty() {
                        assert_eq!(actual.len(), v.len(), "{name}: fill vertices");
                        for (a, b) in actual.iter().zip(v) {
                            assert!((a - b).abs() < 1e-6, "{name}: fill {a} != {b}");
                        }
                    }
                }
                "fill_indices" => {
                    let Some(d) = drawing.as_ref() else {
                        assert!(v.is_empty());
                        continue;
                    };
                    let actual = if data.has_fill() {
                        path_fill(&d.points, d.convex)
                    } else {
                        Vec::new()
                    };
                    let expected: Vec<usize> = v.iter().map(|v| *v as usize).collect();
                    if d.convex {
                        assert_eq!(actual, expected, "{name}: fan indices");
                    }
                    if expected.is_empty() {
                        assert!(actual.is_empty(), "{name}: empty fill");
                        continue;
                    }
                    let (mut min, mut max) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
                    for p in &d.points {
                        min[0] = min[0].min(p.x);
                        min[1] = min[1].min(p.y);
                        max[0] = max[0].max(p.x);
                        max[1] = max[1].max(p.y);
                    }
                    for x in 0..23 {
                        for y in 0..23 {
                            let p = PolygonPoint::new(
                                min[0] + (max[0] - min[0]) * (x as f64 + 0.371) / 23.,
                                min[1] + (max[1] - min[1]) * (y as f64 + 0.619) / 23.,
                            );
                            let expected_layers = layers(&d.points, &expected, p);
                            let painted = expected_layers > 0;
                            assert_eq!(
                                layers(&d.points, &actual, p),
                                expected_layers,
                                "{name}: fill coverage and overdraw at {p:?}"
                            );
                            if painted
                                != graph_contains(&data, size, Point::new(p.x as f32, p.y as f32))
                            {
                                differing_fill_hits += 1;
                            }
                        }
                    }
                }
                "stroke_vertices" => {
                    let actual = drawing
                        .as_ref()
                        .filter(|_| data.line_size > 0)
                        .map_or_else(Vec::new, |d| {
                            polygon_stroke(&d.points, data.line_size as f32).vertices
                        });
                    assert_eq!(actual.len() * 2, v.len(), "{name}: stroke vertices");
                    for (a, b) in actual.iter().flat_map(|p| [p.x, p.y]).zip(v) {
                        assert!((f64::from(a) - b).abs() < 1e-4, "{name}: stroke {a} != {b}");
                    }
                }
                "stroke_indices" => {
                    let actual = drawing
                        .as_ref()
                        .filter(|_| data.line_size > 0)
                        .map_or_else(Vec::new, |d| {
                            polygon_stroke(&d.points, data.line_size as f32).indices
                        });
                    assert_eq!(
                        actual,
                        v.iter().map(|v| *v as usize).collect::<Vec<_>>(),
                        "{name}: stroke indices"
                    );
                }
                "hit" => assert_eq!(
                    graph_contains(&data, size, Point::new(v[0] as f32, v[1] as f32)),
                    v[2] != 0.,
                    "{name}: hit"
                ),
                _ => panic!("unknown native record {kind}"),
            }
        }
        assert_eq!(cases, 22);
        assert!(
            differing_fill_hits > 0,
            "drawing and HitArea must remain independent"
        );
    }
}

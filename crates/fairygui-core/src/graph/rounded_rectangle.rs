use super::PolygonPoint;
use crate::{
    Size,
    widget::{GraphData, GraphType},
};

/// GGraph's DrawPath rectangle. Radii keep their declared values, including
/// oversized and negative values; the native path does not apply CSS clamping.
/// DrawPath strokes are centered on this path and do not inset the fill.
pub fn rounded_rectangle_points(data: &GraphData, size: Size) -> Vec<PolygonPoint> {
    let Some(radii) = data
        .corner_radius
        .filter(|r| r.iter().all(|v| v.is_finite()))
    else {
        return Vec::new();
    };
    if data.graph_kind() != GraphType::Rect
        || !size.width.is_finite()
        || !size.height.is_finite()
        || size.width <= 0.
        || size.height <= 0.
    {
        return Vec::new();
    }
    let [tl, tr, bl, br] = radii.map(f64::from);
    let (w, h) = (f64::from(size.width), f64::from(size.height));
    let mut path = Path {
        points: vec![PolygonPoint::new(tl, 0.)],
        last: PolygonPoint::new(tl, 0.),
    };
    path.line_to(w - tr, 0.);
    path.arc_to(w, 0., w, tr, tr);
    path.line_to(w, h - br);
    path.arc_to(w, h, w - br, h, br);
    path.line_to(bl, h);
    path.arc_to(0., h, 0., h - bl, bl);
    path.line_to(0., tl);
    path.arc_to(0., 0., tl, 0., tl);
    if path
        .points
        .iter()
        .all(|p| p.x.is_finite() && p.y.is_finite())
    {
        path.points
    } else {
        Vec::new()
    }
}

struct Path {
    points: Vec<PolygonPoint>,
    last: PolygonPoint,
}
impl Path {
    fn push(&mut self, x: f64, y: f64) {
        self.last = PolygonPoint::new(x, y);
        self.points.push(self.last);
    }
    fn line_to(&mut self, x: f64, y: f64) {
        if (x - self.last.x).abs() < 1e-3 && (y - self.last.y).abs() < 1e-3 {
            return;
        }
        self.push(x, y);
    }

    // GraphicsRunner.arcTo, including its fixed 32 rotations and the 0.1 pixel
    // emission threshold. Unlike arc(), this tessellation is scale independent.
    fn arc_to(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, radius: f64) {
        let (dx, dy) = (self.last.x - x1, self.last.y - y1);
        let len1 = (dx * dx + dy * dy).sqrt();
        if len1 <= 0.000001 {
            return;
        }
        let (nx, ny) = (dx / len1, dy / len1);
        let (dx2, dy2) = (x2 - x1, y2 - y1);
        let len2 = (dx2 * dx2 + dy2 * dy2).sqrt();
        if len2 <= 0.000001 {
            return;
        }
        let (nx2, ny2) = (dx2 / len2, dy2 / len2);
        let (ox, oy) = (nx + nx2, ny + ny2);
        let olen = (ox * ox + oy * oy).sqrt();
        if olen <= 0.000001 {
            return;
        }
        let (nox, noy) = (ox / olen, oy / olen);
        let alpha = (nox * nx + noy * ny).acos();
        let half_angle = std::f64::consts::FRAC_PI_2 - alpha;
        let tangent = radius * half_angle.tan();
        let (px, py) = (tangent * nx + x1, tangent * ny + y1);
        let center_distance = (tangent * tangent + radius * radius).sqrt();
        let (cx, cy) = (x1 + nox * center_distance, y1 + noy * center_distance);
        let direction = nx * ny2 - ny * nx2;
        let angle = if direction >= 0. {
            half_angle * 2.
        } else {
            -half_angle * 2.
        } / 32.;
        let (sin, cos) = (angle.sin(), angle.cos());
        if (px - self.last.x).abs() > 0.1 || (py - self.last.y).abs() > 0.1 {
            self.push(px, py);
        }
        let (mut vx, mut vy) = (px - cx, py - cy);
        for _ in 0..32 {
            let (next_x, next_y) = (vx * cos + vy * sin, -vx * sin + vy * cos);
            let (x, y) = (next_x + cx, next_y + cy);
            if (self.last.x - x).abs() > 0.1 || (self.last.y - y).abs() > 0.1 {
                self.push(x, y);
            }
            (vx, vy) = (next_x, next_y);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{graph_contains, path_fill, polygon_stroke};

    #[test]
    fn rounded_paths_and_centered_strokes_match_original_draw_path_commands() {
        let mut points = Vec::new();
        let mut width = 0;
        let mut filled = false;
        let mut cases = 0;
        for line in include_str!("../../tests/fixtures/rounded-rectangle-reference.txt").lines() {
            if line.starts_with('#') {
                continue;
            }
            let (kind, values) = line.split_once(' ').unwrap_or((line, ""));
            let expected: Vec<f64> = values
                .split_whitespace()
                .map(|s| s.parse().unwrap())
                .collect();
            match kind {
                "case" => {
                    width = expected[2] as i32;
                    filled = expected[7] != 0.;
                    let data = GraphData {
                        graph_type: 1,
                        line_size: width,
                        corner_radius: Some([
                            expected[3] as f32,
                            expected[4] as f32,
                            expected[5] as f32,
                            expected[6] as f32,
                        ]),
                        ..Default::default()
                    };
                    let size = Size::new(expected[0] as f32, expected[1] as f32);
                    points = rounded_rectangle_points(&data, size);
                    assert!(
                        !graph_contains(
                            &data,
                            size,
                            crate::Point::new(size.width * 0.5, size.height * 0.5)
                        ),
                        "rounded DrawPath has no native hit branch"
                    );
                    cases += 1;
                }
                "points" => {
                    let actual: Vec<f64> = points.iter().flat_map(|p| [p.x, p.y]).collect();
                    assert_eq!(actual.len(), expected.len(), "case {cases}: points");
                    for (a, b) in actual.iter().zip(expected) {
                        assert!((a - b).abs() < 1e-6, "case {cases}: {a} != {b}");
                    }
                }
                "vertices" => {
                    let mesh = polygon_stroke(&points, width as f32);
                    assert_eq!(
                        mesh.vertices.len() * 2,
                        expected.len(),
                        "case {cases}: stroke"
                    );
                    for (a, b) in mesh.vertices.iter().flat_map(|p| [p.x, p.y]).zip(expected) {
                        assert!((f64::from(a) - b).abs() < 1e-4, "case {cases}: {a} != {b}");
                    }
                }
                "indices" => assert_eq!(
                    polygon_stroke(&points, width as f32).indices,
                    expected.iter().map(|v| *v as usize).collect::<Vec<_>>(),
                    "case {cases}: indices"
                ),
                "fill" => {
                    let actual = if filled {
                        path_fill(&points, false)
                    } else {
                        Vec::new()
                    };
                    let expected: Vec<usize> = expected.iter().map(|v| *v as usize).collect();
                    if expected.is_empty() {
                        assert!(actual.is_empty(), "case {cases}: empty fill");
                        continue;
                    }
                    assert!(actual.iter().all(|&index| index < points.len()));
                    let (mut min, mut max) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
                    for p in &points {
                        min[0] = min[0].min(p.x);
                        min[1] = min[1].min(p.y);
                        max[0] = max[0].max(p.x);
                        max[1] = max[1].max(p.y);
                    }
                    let covered = |indices: &[usize], p: PolygonPoint| {
                        indices.as_chunks::<3>().0.iter().any(|t| {
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
                    };
                    // Earcut versions can choose different valid diagonals.
                    // Compare sampled coverage to the original mesh, including
                    // oversized/negative radii and their self-intersections.
                    for x in 0..23 {
                        for y in 0..23 {
                            let p = PolygonPoint::new(
                                min[0] + (max[0] - min[0]) * (x as f64 + 0.371) / 23.,
                                min[1] + (max[1] - min[1]) * (y as f64 + 0.619) / 23.,
                            );
                            assert_eq!(
                                covered(&actual, p),
                                covered(&expected, p),
                                "case {cases}: fill at {p:?}"
                            );
                        }
                    }
                }
                _ => panic!("unknown native record {kind}"),
            }
        }
        assert_eq!(cases, 18);
    }
}

use super::{PolygonPoint, circle_bounds};
use crate::{
    Size,
    widget::{GraphData, GraphType},
};

/// Native DrawCircleCmd path after its line-width inset. A negative radius is
/// intentional: Laya's arc command keeps it when the stroke exceeds the width.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CircleDrawing {
    pub center: PolygonPoint,
    pub radius: f64,
    pub segments: usize,
}

pub fn circle_drawing(data: &GraphData, size: Size, global_scale: f64) -> Option<CircleDrawing> {
    if data.graph_kind() != GraphType::Ellipse || !global_scale.is_finite() || global_scale < 0. {
        return None;
    }
    circle_bounds(size)?;
    let radius = (f64::from(size.width) - f64::from(data.line_size.max(0))) * 0.5;
    // GraphicsRunner.arc uses at least 40 divisions for a circle and the global
    // matrix's largest axis length. Match its JS ToInt32 conversion as well.
    let divisions = (std::f64::consts::TAU * radius * global_scale / 5.).max(40.);
    let segments = (divisions % 4_294_967_296.) as u32 as i32;
    Some(CircleDrawing {
        center: PolygonPoint::new(f64::from(size.width) * 0.5, f64::from(size.height) * 0.5),
        radius,
        segments: segments.max(0) as usize,
    })
}

impl CircleDrawing {
    /// GraphicsRunner.arc -> Path.addPoint, retaining the duplicate first point
    /// and floating-point closing point before BasePoly's own cleanup.
    pub fn points(self) -> Vec<PolygonPoint> {
        let point = |angle: f64| {
            PolygonPoint::new(
                self.center.x + angle.cos() * self.radius,
                self.center.y + angle.sin() * self.radius,
            )
        };
        let mut points = vec![point(0.)];
        let step = std::f64::consts::TAU / self.segments as f64;
        let mut angle = 0.;
        while std::f64::consts::TAU - angle >= step {
            points.push(point(angle));
            angle += step;
        }
        points.push(point(std::f64::consts::TAU));
        points
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::polygon_stroke;

    #[test]
    fn circle_paths_and_stroke_meshes_match_native_commands_at_multiple_scales() {
        let mut drawing = None;
        let mut width = 0;
        let mut points = Vec::new();
        let mut cases = 0;
        for line in include_str!("../../tests/fixtures/circle-reference.txt").lines() {
            if line.starts_with('#') {
                continue;
            }
            let (kind, values) = line.split_once(' ').unwrap_or((line, ""));
            let expected: Vec<f64> = values
                .split_whitespace()
                .map(|v| v.parse().unwrap())
                .collect();
            match kind {
                "case" => {
                    width = expected[2] as i32;
                    drawing = circle_drawing(
                        &GraphData {
                            graph_type: 2,
                            line_size: width,
                            ..Default::default()
                        },
                        Size::new(expected[0] as f32, expected[1] as f32),
                        expected[3],
                    );
                    points = drawing.map_or_else(Vec::new, CircleDrawing::points);
                    cases += 1;
                }
                "points" => {
                    let actual: Vec<f64> = points.iter().flat_map(|p| [p.x, p.y]).collect();
                    assert_eq!(actual.len(), expected.len(), "case {cases}: points");
                    for (a, b) in actual.iter().zip(expected) {
                        assert!((a - b).abs() < 1e-8, "case {cases}: {a} != {b}");
                    }
                }
                "vertices" => {
                    let actual = polygon_stroke(&points, width as f32);
                    if expected.iter().any(|v| !v.is_finite()) {
                        assert!(
                            actual.vertices.is_empty(),
                            "degenerate native path has no finite drawing"
                        );
                    } else {
                        assert_eq!(
                            actual.vertices.len() * 2,
                            expected.len(),
                            "case {cases}: stroke"
                        );
                        for (a, b) in actual
                            .vertices
                            .iter()
                            .flat_map(|p| [p.x, p.y])
                            .zip(expected)
                        {
                            assert!((f64::from(a) - b).abs() < 1e-4, "case {cases}: {a} != {b}");
                        }
                    }
                }
                "indices" => {
                    if drawing.is_some_and(|p| p.radius == 0.) {
                        continue;
                    }
                    assert_eq!(
                        polygon_stroke(&points, width as f32).indices,
                        expected.iter().map(|v| *v as usize).collect::<Vec<_>>(),
                        "case {cases}: indices"
                    );
                }
                _ => panic!("unknown native record {kind}"),
            }
        }
        assert_eq!(cases, 18);
    }
}

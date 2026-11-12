//! Native GGraph polygon coordinates and LayaAir 3.3 closed-line geometry.
use crate::{
    Point, Size,
    widget::{GraphData, GraphType},
};

/// Keep JS Number precision until stroke joins are constructed. Rounding regular
/// vertices to f32 earlier can change native near-parallel/intersection branches.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PolygonPoint {
    pub x: f64,
    pub y: f64,
}
impl PolygonPoint {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// Explicit polygon coordinates stay in object space when the object resizes.
/// Regular polygons use min(width,height)/2 for both radius and center, as GGraph.
pub fn polygon_points(data: &GraphData, size: Size) -> Vec<PolygonPoint> {
    if size.width <= 0.0 || size.height <= 0.0 {
        return Vec::new();
    }
    let points = match data.graph_kind() {
        GraphType::Polygon => data
            .polygon_points
            .as_deref()
            .unwrap_or_default()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| PolygonPoint::new(f64::from(p[0]), f64::from(p[1])))
            .collect(),
        GraphType::RegularPolygon if data.sides > 0 => {
            let radius = f64::from(size.width.min(size.height)) / 2.0;
            let mut angle = f64::from(data.start_angle).to_radians();
            let step = std::f64::consts::TAU / f64::from(data.sides);
            (0..data.sides as usize)
                .map(|i| {
                    let distance = data
                        .distances
                        .as_ref()
                        .and_then(|d| d.get(i))
                        .copied()
                        .filter(|d| !d.is_nan())
                        .unwrap_or(1.0);
                    let p = PolygonPoint::new(
                        radius + radius * f64::from(distance) * angle.cos(),
                        radius + radius * f64::from(distance) * angle.sin(),
                    );
                    angle += step;
                    p
                })
                .collect()
        }
        _ => Vec::new(),
    };
    if points.iter().all(|p| p.x.is_finite() && p.y.is_finite()) {
        points
    } else {
        Vec::new()
    }
}

/// The native DrawPoly command offsets odd positive line widths by half a pixel.
/// HitArea deliberately uses the unshifted command points, independent of color.
pub fn polygon_draw_offset(data: &GraphData) -> f32 {
    if data.line_size > 0 && data.line_size % 2 != 0 {
        0.5
    } else {
        0.0
    }
}

pub fn polygon_contains(points: &[PolygonPoint], point: Point) -> bool {
    let point = PolygonPoint::new(f64::from(point.x), f64::from(point.y));
    let mut inside = false;
    let Some(mut previous) = points.last().copied() else {
        return false;
    };
    for &current in points {
        if (current.y > point.y) != (previous.y > point.y)
            && point.x
                < (previous.x - current.x) * (point.y - current.y) / (previous.y - current.y)
                    + current.x
        {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

#[derive(Debug, Default)]
pub struct StrokeMesh {
    pub vertices: Vec<Point>,
    pub indices: Vec<usize>,
}

type P = [f64; 2];
fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1]]
}
fn add(a: P, b: P) -> P {
    [a[0] + b[0], a[1] + b[1]]
}
fn normal(a: P, b: P, width: f64) -> P {
    let delta = sub(b, a);
    let length = delta[0].hypot(delta[1]);
    [delta[1] / length * width, -delta[0] / length * width]
}
enum Cross {
    Parallel,
    Outside,
    Point(P),
}
fn cross(a: P, b: P, c: P, d: P, segment: bool) -> Cross {
    let u = sub(b, a);
    let v = sub(d, c);
    let delta = sub(c, a);
    let denominator = u[0] * v[1] - v[0] * u[1];
    if denominator.abs() < 1e-10 {
        return Cross::Parallel;
    }
    let t = (delta[0] * v[1] - delta[1] * v[0]) / denominator;
    let s = (delta[0] * u[1] - delta[1] * u[0]) / denominator;
    if segment && (!(0.0..=1.0).contains(&t) || !(0.0..=1.0).contains(&s)) {
        return Cross::Outside;
    }
    Cross::Point([a[0] + t * u[0], a[1] + t * u[1]])
}

// Port of BasePoly.checkCrossPoint: a short outer miter is kept, a long one
// becomes a bevel. Parallel/reversing segments use the native simple fallback.
#[allow(clippy::too_many_arguments)] // Keep native BasePoly argument correspondence.
fn join_side(
    a: P,
    b: P,
    c: P,
    d: P,
    other_a: P,
    other_d: P,
    center: P,
    half: f64,
    vertices: &mut Vec<P>,
) -> Option<usize> {
    match cross(a, b, c, d, true) {
        Cross::Point(p) => {
            vertices.push(p);
            return Some(1);
        }
        Cross::Parallel => return None,
        Cross::Outside => {}
    }
    if matches!(cross(other_a, a, c, d, true), Cross::Point(_))
        || matches!(cross(a, b, d, other_d, true), Cross::Point(_))
    {
        return None;
    }
    if let Cross::Point(p) = cross(a, b, c, d, false) {
        let distance = sub(p, center);
        if distance[0].hypot(distance[1]) > half * 1.5 {
            vertices.extend([b, c]);
            return Some(2);
        }
        vertices.push(p);
        return Some(1);
    }
    None
}

#[allow(clippy::too_many_arguments)] // Keep native BasePoly argument correspondence.
fn simple_join(
    a: P,
    b: P,
    c: P,
    left: P,
    right: P,
    vertices: &mut Vec<P>,
    indices: &mut Vec<usize>,
    index: usize,
) -> usize {
    let u = sub(a, b);
    let v = sub(c, b);
    let cosine =
        ((u[0] * v[0] + u[1] * v[1]) / (u[0].hypot(u[1]) * v[0].hypot(v[1]))).clamp(-1.0, 1.0);
    if cosine.acos() > std::f64::consts::FRAC_PI_2 {
        vertices.extend([left, right]);
        indices.extend([index, index + 1, index + 2, index + 1, index + 2, index + 3]);
    } else {
        vertices.extend([right, left]);
        indices.extend([index, index + 1, index + 3, index + 1, index + 3, index + 2]);
    }
    index + 2
}

fn middle(
    a: P,
    b: P,
    c: P,
    half: f64,
    vertices: &mut Vec<P>,
    indices: &mut Vec<usize>,
    index: usize,
) -> usize {
    let n1 = normal(a, b, half);
    let n2 = normal(b, c, half);
    let left = [sub(a, n1), sub(b, n1), sub(b, n2), sub(c, n2)];
    let right = [add(a, n1), add(b, n1), add(b, n2), add(c, n2)];
    let Some(left_count) = join_side(
        left[0], left[1], left[2], left[3], right[0], right[3], b, half, vertices,
    ) else {
        return simple_join(a, b, c, left[1], right[1], vertices, indices, index);
    };
    let right_count = join_side(
        right[0], right[1], right[2], right[3], left[0], left[3], b, half, vertices,
    );
    if right_count.is_none() {
        vertices.truncate(vertices.len() - left_count);
        return simple_join(a, b, c, left[1], right[1], vertices, indices, index);
    }
    if right_count == Some(2) {
        let count = vertices.len();
        vertices.swap(count - 2, count - 3);
        indices.extend([
            index,
            index + 1,
            index + 3,
            index + 1,
            index + 3,
            index + 2,
            index + 2,
            index + 3,
            index + 4,
        ]);
        index + 3
    } else if left_count == 2 {
        indices.extend([
            index,
            index + 1,
            index + 2,
            index + 1,
            index + 2,
            index + 4,
            index + 2,
            index + 3,
            index + 4,
        ]);
        index + 3
    } else {
        indices.extend([index, index + 1, index + 2, index + 1, index + 2, index + 3]);
        index + 2
    }
}

/// Closed line strip using unchanged LayaAir 3.3 BasePoly.createLine2 branches.
/// Invalid or wholly coincident paths produce no renderable triangles.
pub fn polygon_stroke(input: &[PolygonPoint], width: f32) -> StrokeMesh {
    if input.len() < 2 || width <= 0.0 || !width.is_finite() {
        return StrokeMesh::default();
    }
    let mut points = vec![[input[0].x, input[0].y]];
    for pair in input.windows(2) {
        if (pair[1].x - pair[0].x).abs() + (pair[1].y - pair[0].y).abs() > 0.01 {
            points.push([pair[1].x, pair[1].y]);
        }
    }
    if points.len() < 2 {
        return StrokeMesh::default();
    }
    let first = points[0];
    let last = *points.last().unwrap();
    let delta = (first[0] - last[0]).abs() + (first[1] - last[1]).abs();
    if delta > 1e-13 {
        points.push(first);
    } else {
        *points.last_mut().unwrap() = first;
    }
    let half = f64::from(width) / 2.0;
    let n = normal(points[0], points[1], half);
    let mut vertices = vec![sub(first, n), add(first, n)];
    let mut indices = Vec::new();
    let mut index = 0;
    for triple in points.windows(3) {
        index = middle(
            triple[0],
            triple[1],
            triple[2],
            half,
            &mut vertices,
            &mut indices,
            index,
        );
    }
    let count = points.len();
    middle(
        points[count - 2],
        points[count - 1],
        points[1],
        half,
        &mut vertices,
        &mut indices,
        index,
    );
    let count = vertices.len();
    if count < 2
        || vertices
            .iter()
            .any(|p| !p[0].is_finite() || !p[1].is_finite())
    {
        return StrokeMesh::default();
    }
    vertices[0] = vertices[count - 2];
    vertices[1] = vertices[count - 1];
    // Native's last join may introduce one or two additional vertices.
    let end = count - 2;
    indices.extend([end, end + 1, 1, end + 1, 1, 0]);
    if indices.iter().any(|i| *i >= vertices.len()) {
        return StrokeMesh::default();
    }
    StrokeMesh {
        vertices: vertices
            .into_iter()
            .map(|p| Point::new(p[0] as f32, p[1] as f32))
            .collect(),
        indices,
    }
}

#[cfg(test)]
#[path = "graph_tests.rs"]
mod tests;

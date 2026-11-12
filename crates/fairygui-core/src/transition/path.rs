//! 运动路径。
//!
//! 路径由若干锚点构成，相邻两点之间是一段曲线。曲线类型记在**起点**上，
//! 描述「从这一点出发的那一段」如何求值。连续的样条点会被合并成一段。
//!
//! 复刻时有几处与直觉相反的地方，都已按官方实现保留：
//!
//! - `Straight` 的编号是 `3` 而不是 `0`；
//! - 每段内部点的平铺顺序是「起点、终点、控制点…」，`points[start + 1]` 是终点；
//! - 段长度是**近似值**（直线段用两锚点直线距离，样条段用控制多边形折线长度），
//!   不是真实弧长。这会影响「比例 → 位置」的映射，要完全对齐官方结果就不能改。

use crate::geom::{Point, Size};

/// 曲线类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CurveType {
    /// 均匀 Catmull-Rom 样条。连续多个该类型的点会合并为一段。
    #[default]
    CatmullRom,
    /// 二次贝塞尔，带一个控制点。
    Bezier,
    /// 三次贝塞尔，带两个控制点。
    CubicBezier,
    /// 直线。
    Straight,
}

impl CurveType {
    /// 由描述中的字节编号构造。
    pub const fn from_code(code: u8) -> Self {
        match code {
            1 => Self::Bezier,
            2 => Self::CubicBezier,
            3 => Self::Straight,
            _ => Self::CatmullRom,
        }
    }
}

/// 路径上的一个锚点。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PathPoint {
    /// 锚点坐标。
    pub x: f32,
    pub y: f32,
    /// 从本点出发的那一段的第一个控制点。
    pub control1: Point,
    /// 从本点出发的那一段的第二个控制点。
    pub control2: Point,
    /// 从本点出发的那一段的曲线类型。
    pub curve_type: CurveType,
}

impl PathPoint {
    /// 直线段的锚点。
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            x,
            y,
            ..Default::default()
        }
    }

    /// 二次贝塞尔段的锚点。
    pub fn bezier(x: f32, y: f32, control1: Point) -> Self {
        Self {
            x,
            y,
            control1,
            control2: Point::ZERO,
            curve_type: CurveType::Bezier,
        }
    }

    /// 三次贝塞尔段的锚点。
    pub fn cubic_bezier(x: f32, y: f32, control1: Point, control2: Point) -> Self {
        Self {
            x,
            y,
            control1,
            control2,
            curve_type: CurveType::CubicBezier,
        }
    }

    /// 直线段的锚点。
    ///
    /// 曲线类型记在**起点**上，因此要开一段直线，起点必须用它构造。
    pub fn straight(x: f32, y: f32) -> Self {
        Self {
            x,
            y,
            curve_type: CurveType::Straight,
            ..Default::default()
        }
    }

    /// 样条段的锚点。
    pub fn spline(x: f32, y: f32) -> Self {
        Self {
            x,
            y,
            curve_type: CurveType::CatmullRom,
            ..Default::default()
        }
    }

    fn position(&self) -> Point {
        Point::new(self.x, self.y)
    }
}

/// 一段曲线。
#[derive(Debug, Clone, PartialEq)]
struct Segment {
    curve_type: CurveType,
    /// 近似长度：直线段取锚点距离，样条段取控制多边形折线长度。
    length: f32,
    /// 在展平点表中的起始下标。
    point_start: usize,
    /// 该段占用的点数。
    point_count: usize,
}

/// 运动路径。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GPath {
    segments: Vec<Segment>,
    /// 所有段的点展开后平铺于此。
    points: Vec<Point>,
    full_length: f32,
}

impl GPath {
    /// 用一串锚点创建路径。
    pub fn new(points: &[PathPoint]) -> Self {
        let mut path = Self::default();
        path.build(points);
        path
    }

    /// 路径总长度（近似）。
    pub fn length(&self) -> f32 {
        self.full_length
    }

    /// 段数。
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// 按**整条路径的比例**取点。`t` 会被夹到 `0..=1`。
    ///
    /// 注意参数是比例而不是「段号 + 段内比例」。
    pub fn point_at(&self, t: f32) -> Point {
        let t = t.clamp(0.0, 1.0);
        if self.segments.is_empty() {
            return Point::ZERO;
        }

        let last = self.segments.len() - 1;

        // 终点单独处理，避免落到最后一段之外。
        if t >= 1.0 {
            return self.evaluate(&self.segments[last], 1.0);
        }

        let mut remaining = t * self.full_length;
        for segment in &self.segments {
            remaining -= segment.length;
            if remaining < 0.0 {
                let local = if segment.length > 0.0 {
                    1.0 + remaining / segment.length
                } else {
                    0.0
                };
                return self.evaluate(segment, local.clamp(0.0, 1.0));
            }
        }

        self.evaluate(&self.segments[last], 1.0)
    }

    /// 求某一小段在比例 `t` 处的位置。
    fn evaluate(&self, segment: &Segment, t: f32) -> Point {
        let start = segment.point_start;

        match segment.curve_type {
            CurveType::Straight => {
                let from = self.points[start];
                let to = self.points[start + 1];
                lerp(from, to, t)
            }
            CurveType::Bezier => {
                // 平铺顺序是「起点、终点、控制点」。
                let p0 = self.points[start];
                let p1 = self.points[start + 1];
                let c0 = self.points[start + 2];

                let t2 = 1.0 - t;
                Point::new(
                    t2 * t2 * p0.x + 2.0 * t2 * t * c0.x + t * t * p1.x,
                    t2 * t2 * p0.y + 2.0 * t2 * t * c0.y + t * t * p1.y,
                )
            }
            CurveType::CubicBezier => {
                let p0 = self.points[start];
                let p1 = self.points[start + 1];
                let c0 = self.points[start + 2];
                let c1 = self.points[start + 3];

                let t2 = 1.0 - t;
                Point::new(
                    t2 * t2 * t2 * p0.x
                        + 3.0 * t2 * t2 * t * c0.x
                        + 3.0 * t2 * t * t * c1.x
                        + t * t * t * p1.x,
                    t2 * t2 * t2 * p0.y
                        + 3.0 * t2 * t2 * t * c0.y
                        + 3.0 * t2 * t * t * c1.y
                        + t * t * t * p1.y,
                )
            }
            CurveType::CatmullRom => self.evaluate_spline(segment, t),
        }
    }

    /// 均匀 Catmull-Rom 样条（张力 0.5）。
    fn evaluate_spline(&self, segment: &Segment, t: f32) -> Point {
        // 每四个相邻点决定一条子曲线。
        let sub_curves = segment.point_count.saturating_sub(4);
        if sub_curves == 0 {
            let start = segment.point_start;
            return lerp(self.points[start], self.points[start + 1], t);
        }

        let scaled = t * sub_curves as f32;
        let base = scaled.floor() as usize;
        // 比例正好为 1 时小数部分会变成 0，需要特判成 1。
        let local = if t >= 1.0 { 1.0 } else { scaled - base as f32 };

        let index = segment.point_start + base;
        let p0 = self.points[index];
        let p1 = self.points[index + 1];
        let p2 = self.points[index + 2];
        let p3 = self.points[index + 3];

        let u = local;
        let c0 = -0.5 * u * u * u + u * u - 0.5 * u;
        let c1 = 1.5 * u * u * u - 2.5 * u * u + 1.0;
        let c2 = -1.5 * u * u * u + 2.0 * u * u + 0.5 * u;
        let c3 = 0.5 * u * u * u - 0.5 * u * u;

        Point::new(
            p0.x * c0 + p1.x * c1 + p2.x * c2 + p3.x * c3,
            p0.y * c0 + p1.y * c1 + p2.y * c2 + p3.y * c3,
        )
    }

    /// 展开为段与点表。
    fn build(&mut self, points: &[PathPoint]) {
        self.segments.clear();
        self.points.clear();
        self.full_length = 0.0;

        if points.is_empty() {
            return;
        }

        // 累积的样条点，连续的样条锚点会被收束成一段。
        let mut spline: Vec<Point> = Vec::new();
        let mut previous = points[0];

        if previous.curve_type == CurveType::CatmullRom {
            spline.push(previous.position());
        }

        for current in &points[1..] {
            if previous.curve_type != CurveType::CatmullRom {
                self.push_curve_segment(&previous, current);
            }

            if current.curve_type == CurveType::CatmullRom {
                spline.push(current.position());
            } else if spline.len() > 1 {
                spline.push(current.position());
                self.push_spline_segment(&mut spline);
            }

            previous = *current;
        }

        if spline.len() > 1 {
            self.push_spline_segment(&mut spline);
        }
    }

    /// 直线与贝塞尔段：直接按「起点、终点、控制点…」平铺。
    fn push_curve_segment(&mut self, from: &PathPoint, to: &PathPoint) {
        let start = self.points.len();
        let count = match from.curve_type {
            CurveType::Straight => 2,
            CurveType::Bezier => 3,
            CurveType::CubicBezier => 4,
            CurveType::CatmullRom => return,
        };

        self.points.push(from.position());
        self.points.push(to.position());
        if count >= 3 {
            self.points.push(from.control1);
        }
        if count >= 4 {
            self.points.push(from.control2);
        }

        // 长度一律取两锚点的直线距离。
        let length = distance(from.position(), to.position());
        self.full_length += length;
        self.segments.push(Segment {
            curve_type: from.curve_type,
            length,
            point_start: start,
            point_count: count,
        });
    }

    /// 样条段：首点重复一次、末点重复两次后整段平铺。
    fn push_spline_segment(&mut self, spline: &mut Vec<Point>) {
        if spline.len() < 2 {
            spline.clear();
            return;
        }

        // 两端补齐，让首尾子曲线也有完整的四个控制点。
        let first = spline[0];
        let last = *spline.last().unwrap_or(&first);
        spline.insert(0, first);
        spline.push(last);
        spline.push(last);

        let start = self.points.len();
        let count = spline.len();

        // 长度取控制多边形的折线长度（补齐点贡献 0）。
        let mut length = 0.0f32;
        for pair in spline.windows(2) {
            length += distance(pair[0], pair[1]);
        }

        self.points.extend_from_slice(spline);
        self.full_length += length;
        self.segments.push(Segment {
            curve_type: CurveType::CatmullRom,
            length,
            point_start: start,
            point_count: count,
        });

        spline.clear();
    }
}

fn lerp(from: Point, to: Point, t: f32) -> Point {
    Point::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t)
}

fn distance(from: Point, to: Point) -> f32 {
    let dx = from.x - to.x;
    let dy = from.y - to.y;
    (dx * dx + dy * dy).sqrt()
}

/// 路径的包围盒，供渲染层预留空间。
pub fn bounding_box(path: &GPath) -> Size {
    if path.points.is_empty() {
        return Size::ZERO;
    }

    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    for point in &path.points {
        min_x = min_x.min(point.x);
        min_y = min_y.min(point.y);
        max_x = max_x.max(point.x);
        max_y = max_y.max(point.y);
    }

    Size::new(max_x - min_x, max_y - min_y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn curve_codes_match_official_table() {
        assert_eq!(CurveType::from_code(0), CurveType::CatmullRom);
        assert_eq!(CurveType::from_code(1), CurveType::Bezier);
        assert_eq!(CurveType::from_code(2), CurveType::CubicBezier);
        assert_eq!(
            CurveType::from_code(3),
            CurveType::Straight,
            "直线的编号是 3，不是 0"
        );
    }

    #[test]
    fn straight_segment_interpolates_linearly() {
        let path = GPath::new(&[
            PathPoint::straight(0.0, 0.0),
            PathPoint::straight(100.0, 0.0),
        ]);
        assert_eq!(path.segment_count(), 1);
        assert!(approx(path.length(), 100.0));

        let mid = path.point_at(0.5);
        assert!(approx(mid.x, 50.0) && approx(mid.y, 0.0), "实际 {mid:?}");
        assert!(approx(path.point_at(0.0).x, 0.0));
        assert!(approx(path.point_at(1.0).x, 100.0));
    }

    #[test]
    fn quadratic_bezier_bends_towards_the_control_point() {
        let path = GPath::new(&[
            PathPoint::bezier(0.0, 0.0, Point::new(0.0, 100.0)),
            PathPoint::straight(100.0, 0.0),
        ]);

        let mid = path.point_at(0.5);
        // 控制点把曲线拉向上方：横坐标只走到四分之一处，纵坐标到一半。
        assert!(approx(mid.x, 25.0), "实际 {mid:?}");
        assert!(approx(mid.y, 50.0), "实际 {mid:?}");
    }

    #[test]
    fn cubic_bezier_uses_both_control_points() {
        let path = GPath::new(&[
            PathPoint::cubic_bezier(0.0, 0.0, Point::new(0.0, 100.0), Point::new(100.0, 100.0)),
            PathPoint::straight(100.0, 0.0),
        ]);

        let mid = path.point_at(0.5);
        // 三次贝塞尔在 t=0.5 时，两个控制点各贡献 3/8。
        assert!(approx(mid.x, 50.0), "实际 {mid:?}");
        assert!(approx(mid.y, 75.0), "实际 {mid:?}");
    }

    #[test]
    fn spline_passes_through_its_anchors() {
        // 三个样条点会合并成一段，并经过补点处理。
        let path = GPath::new(&[
            PathPoint::spline(0.0, 0.0),
            PathPoint::spline(50.0, 50.0),
            PathPoint::spline(100.0, 0.0),
        ]);

        assert_eq!(path.segment_count(), 1, "连续样条点应当合并为一段");
        assert!(path.length() > 0.0);

        // 中点应当落在第二个锚点附近。
        let mid = path.point_at(0.5);
        assert!(approx(mid.x, 50.0), "实际 {mid:?}");
        assert!(approx(mid.y, 50.0), "实际 {mid:?}");
    }

    #[test]
    fn mixed_path_produces_one_segment_per_curve() {
        let path = GPath::new(&[
            PathPoint::straight(0.0, 0.0),
            PathPoint::straight(100.0, 0.0),
            PathPoint::bezier(200.0, 0.0, Point::new(150.0, 50.0)),
            PathPoint::straight(300.0, 0.0),
        ]);

        assert_eq!(path.segment_count(), 3);
        // 段长取的是两锚点的直线距离，因此三个 100 像素的锚点间距正好是 300。
        assert!(approx(path.length(), 300.0), "实际 {}", path.length());
    }

    #[test]
    fn ratio_beyond_range_is_clamped() {
        let path = GPath::new(&[
            PathPoint::straight(0.0, 0.0),
            PathPoint::straight(10.0, 0.0),
        ]);

        assert!(approx(path.point_at(-5.0).x, 0.0));
        assert!(approx(path.point_at(5.0).x, 10.0));
    }

    #[test]
    fn empty_path_is_safe() {
        let path = GPath::default();
        assert!(path.is_empty());
        assert_eq!(path.point_at(0.5), Point::ZERO);
        assert_eq!(bounding_box(&path), Size::ZERO);
    }

    #[test]
    fn single_point_path_has_no_segments() {
        let path = GPath::new(&[PathPoint::new(5.0, 5.0)]);
        assert!(path.is_empty());
    }

    #[test]
    fn bounding_box_covers_all_points() {
        let path = GPath::new(&[
            PathPoint::straight(10.0, 20.0),
            PathPoint::straight(40.0, 5.0),
        ]);

        let size = bounding_box(&path);
        assert!(approx(size.width, 30.0));
        assert!(approx(size.height, 15.0));
    }
}

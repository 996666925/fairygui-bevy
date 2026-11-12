//! 核心层的几何类型。
//!
//! 与渲染引擎的类型刻意分离：核心层用浮点坐标与「左上角为原点、y 轴向下」的
//! 约定，这类语义在 FairyGUI 里贯穿始终（滚动、九宫格、轴心换算都依赖它）。

/// 二维点 / 向量。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// 落在原点。
    pub fn is_zero(&self) -> bool {
        self.x == 0.0 && self.y == 0.0
    }

    pub fn offset(&mut self, dx: f32, dy: f32) {
        self.x += dx;
        self.y += dy;
    }

    pub fn set(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
    }
}

/// 宽高尺寸。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
    };

    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    pub fn is_zero(&self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }

    pub fn set(&mut self, width: f32, height: f32) {
        self.width = width;
        self.height = height;
    }
}

/// 矩形区域，`x`/`y` 为左上角坐标。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };

    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// 以左上角与尺寸构造。
    pub const fn from_size(origin: Point, size: Size) -> Self {
        Self::new(origin.x, origin.y, size.width, size.height)
    }

    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    pub fn position(&self) -> Point {
        Point::new(self.x, self.y)
    }

    pub fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }

    pub fn is_empty(&self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }

    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.x && point.y >= self.y && point.x < self.right() && point.y < self.bottom()
    }

    pub fn intersects(&self, other: &Self) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }

    pub fn intersection(&self, other: &Self) -> Self {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        Self::new(x, y, (right - x).max(0.0), (bottom - y).max(0.0))
    }

    /// 同时包含两者的最小矩形。
    pub fn union(&self, other: &Self) -> Self {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        Self::new(x, y, right - x, bottom - y)
    }

    /// 向外扩张（用于适配缩放与滤镜留白）。
    pub fn inflate(&mut self, dx: f32, dy: f32) {
        self.x -= dx;
        self.y -= dy;
        self.width += dx * 2.0;
        self.height += dy * 2.0;
    }
}

/// 四边距，语义与 FairyGUI 的 `Margin` 一致（不含中线偏移）。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Margin {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Margin {
    pub const ZERO: Self = Self {
        left: 0.0,
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
    };

    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub const fn all(value: f32) -> Self {
        Self::new(value, value, value, value)
    }

    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }

    pub fn top_left(&self) -> Point {
        Point::new(self.left, self.top)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_contains_uses_half_open_interval() {
        let rect = Rect::new(10.0, 20.0, 30.0, 40.0);
        assert!(rect.contains(Point::new(10.0, 20.0)));
        assert!(rect.contains(Point::new(39.9, 59.9)));
        assert!(!rect.contains(Point::new(40.0, 60.0)));
        assert!(!rect.contains(Point::new(9.9, 20.0)));
    }

    #[test]
    fn rect_intersection_clamps_to_zero_size() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(20.0, 20.0, 10.0, 10.0);
        assert!(!a.intersects(&b));
        let hit = a.intersection(&b);
        assert_eq!(hit.width, 0.0);
        assert_eq!(hit.height, 0.0);
    }

    #[test]
    fn rect_union_handles_empty_operands() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let empty = Rect::ZERO;
        assert_eq!(a.union(&empty), a);
        assert_eq!(empty.union(&a), a);
    }

    #[test]
    fn margin_sums_opposite_edges() {
        let margin = Margin::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(margin.horizontal(), 4.0);
        assert_eq!(margin.vertical(), 6.0);
        assert_eq!(margin.top_left(), Point::new(1.0, 2.0));
    }
}

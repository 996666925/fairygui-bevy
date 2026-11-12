//! Cached native GList virtual geometry, independent of item object creation.
use super::{ListData, ListLayoutType};
use crate::geom::{Point, Rect, Size};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VirtualLayout {
    pub layout: ListLayoutType,
    pub view: Size,
    pub auto_resize: bool,
    pub columns: i32,
    pub rows: i32,
    pub column_gap: i32,
    pub line_gap: i32,
    pub align: i32,
    pub vertical_align: i32,
}
impl VirtualLayout {
    pub(crate) fn valid(self, item: Size) -> bool {
        if ![self.view.width, self.view.height, item.width, item.height]
            .into_iter()
            .all(|v| v.is_finite() && v > 0.)
            || item.width + self.column_gap as f32 <= 0.
            || item.height + self.line_gap as f32 <= 0.
        {
            return false;
        }
        let (columns, rows) = self.counts(item);
        columns > 0
            && rows > 0
            && columns
                .checked_mul(rows)
                .is_some_and(|n| n <= i32::MAX as usize)
    }
    pub fn from_list(data: &ListData, view: Size) -> Self {
        Self {
            layout: data.layout,
            view,
            auto_resize: data.auto_resize_item,
            columns: data.column_count,
            rows: data.line_count,
            column_gap: data.column_gap,
            line_gap: data.line_gap,
            align: data.align,
            vertical_align: data.vertical_align,
        }
    }
    pub fn vertical(self) -> bool {
        matches!(
            self.layout,
            ListLayoutType::SingleColumn | ListLayoutType::FlowHorizontal
        )
    }
    pub fn gap(self) -> f32 {
        if self.vertical() {
            self.line_gap as f32
        } else {
            self.column_gap as f32
        }
    }
    pub fn counts(self, item: Size) -> (usize, usize) {
        let count = |explicit: i32, view: f32, size: f32, gap: i32| {
            if explicit > 0 {
                explicit as usize
            } else {
                ((f64::from(view) + f64::from(gap)) / (f64::from(size) + f64::from(gap)))
                    .floor()
                    .max(1.) as usize
            }
        };
        match self.layout {
            ListLayoutType::SingleColumn | ListLayoutType::SingleRow => (1, 1),
            ListLayoutType::FlowHorizontal => (
                count(self.columns, self.view.width, item.width, self.column_gap),
                1,
            ),
            ListLayoutType::FlowVertical => (
                count(self.rows, self.view.height, item.height, self.line_gap),
                1,
            ),
            ListLayoutType::Pagination => (
                count(self.columns, self.view.width, item.width, self.column_gap),
                count(self.rows, self.view.height, item.height, self.line_gap),
            ),
        }
    }
    pub fn alignment(self, content: Size) -> Point {
        let align = |view: f32, content: f32, mode: i32| {
            if content >= view {
                0.
            } else {
                match mode {
                    1 => ((view - content) / 2.).floor(),
                    2 => view - content,
                    _ => 0.,
                }
            }
        };
        Point::new(
            align(self.view.width, content.width, self.align),
            align(self.view.height, content.height, self.vertical_align),
        )
    }
}

#[derive(Debug, Clone)]
pub struct VirtualGeometry {
    pub config: VirtualLayout,
    pub item_size: Size,
    pub sizes: Vec<Size>,
    pub count: usize,
    pub line_items: usize,
    pub page_rows: usize,
    pub first: usize,
    pub first_position: Option<Point>,
}
impl VirtualGeometry {
    pub fn content_size(&self) -> Size {
        if self.count == 0 {
            return Size::ZERO;
        }
        let c = self.config;
        if c.layout == ListLayoutType::Pagination {
            return Size::new(
                self.count.div_ceil(self.line_items * self.page_rows) as f32 * c.view.width,
                c.view.height,
            );
        }
        let mut main = 0_f64;
        for i in (0..self.count).step_by(self.line_items) {
            main += f64::from(self.extent(i)) + f64::from(c.gap());
        }
        if main > 0. {
            main -= f64::from(c.gap());
        }
        let cross = if c.auto_resize {
            if c.vertical() {
                c.view.width
            } else {
                c.view.height
            }
        } else {
            let gap = if c.vertical() {
                c.column_gap
            } else {
                c.line_gap
            };
            let mut cross = 0_f64;
            for i in 0..self.line_items.min(self.count) {
                cross += f64::from(if c.vertical() {
                    self.sizes[i].width
                } else {
                    self.sizes[i].height
                }) + f64::from(gap);
            }
            if cross > 0. {
                cross -= f64::from(gap);
            }
            cross as f32
        };
        if c.vertical() {
            Size::new(cross, main as f32)
        } else {
            Size::new(main as f32, cross)
        }
    }
    pub fn extent(&self, i: usize) -> f32 {
        if self.config.vertical() {
            self.sizes[i].height
        } else {
            self.sizes[i].width
        }
    }

    /// Native getIndexOnPos1/2's retained-child anchor and strict trailing test.
    pub fn index_on_position(&self, value: f32, force: bool) -> (usize, f32) {
        if self.count < self.line_items {
            return (0, 0.);
        }
        if self.config.layout == ListLayoutType::Pagination {
            let page = (value / self.config.view.width).floor().max(0.) as usize;
            let start = page * self.line_items * self.page_rows;
            let mut pos = page as f64 * f64::from(self.config.view.width);
            for i in 0..self.line_items {
                let width = self
                    .sizes
                    .get(start + i)
                    .map_or(self.item_size.width, |s| s.width);
                let next = pos + f64::from(width) + f64::from(self.config.column_gap);
                if next > f64::from(value) {
                    return (start + i, pos as f32);
                }
                pos = next;
            }
            return (start + self.line_items - 1, pos as f32);
        }
        let gap = f64::from(self.config.gap());
        if !force && let Some(first) = self.first_position {
            let mut pos = f64::from(if self.config.vertical() {
                first.y
            } else {
                first.x
            });
            if pos > f64::from(value) {
                let mut i = self.first;
                while let Some(previous) = i.checked_sub(self.line_items) {
                    i = previous;
                    pos -= f64::from(self.extent(i)) + gap;
                    if pos <= f64::from(value) {
                        return (i, pos as f32);
                    }
                }
                return (0, 0.);
            }
            for i in (self.first..self.count).step_by(self.line_items) {
                let next = pos + f64::from(self.extent(i)) + gap;
                if next > f64::from(value) {
                    return (i, pos as f32);
                }
                pos = next;
            }
            return (self.count - self.line_items, pos as f32);
        }
        let mut pos = 0_f64;
        for i in (0..self.count).step_by(self.line_items) {
            let next = pos + f64::from(self.extent(i)) + gap;
            if next > f64::from(value) {
                return (i, pos as f32);
            }
            pos = next;
        }
        (self.count - self.line_items, pos as f32)
    }
    pub fn snapping_position(&self, mut value: Point, direction: Point) -> Point {
        let vertical = self.config.vertical();
        let (saved, dir) = if vertical {
            (value.y, direction.y)
        } else {
            (value.x, direction.x)
        };
        let (index, mut pos) = self.index_on_position(saved, false);
        if index < self.count {
            let extent = if vertical {
                self.sizes[index].height
            } else {
                self.sizes[index].width
            };
            let threshold = if dir < 0. {
                0.1
            } else if dir > 0. {
                1. - 0.1
            } else {
                0.5
            };
            if f64::from(saved - pos) > threshold * f64::from(extent) {
                pos += extent + self.config.gap();
            }
        }
        if vertical {
            value.y = pos;
        } else {
            value.x = pos;
        }
        value
    }
    pub fn item_rect(&self, index: usize) -> Option<Rect> {
        let size = *self.sizes.get(index)?;
        let c = self.config;
        if c.layout == ListLayoutType::Pagination {
            // Preserve native divisions here, including fractional page indices.
            let page = index as f32 / (self.line_items * self.page_rows) as f32;
            return Some(Rect::new(
                page * c.view.width
                    + (index % self.line_items) as f32 * (size.width + c.column_gap as f32),
                (index as f32 / self.line_items as f32) % self.page_rows as f32
                    * (size.height + c.line_gap as f32),
                size.width,
                size.height,
            ));
        }
        let mut pos = 0_f64;
        for i in (self.line_items - 1..index).step_by(self.line_items) {
            pos += f64::from(self.extent(i)) + f64::from(c.gap());
        }
        Some(if c.vertical() {
            Rect::new(0., pos as f32, self.item_size.width, size.height)
        } else {
            Rect::new(pos as f32, 0., size.width, self.item_size.height)
        })
    }
}

//! Materialized GList pagination before container alignment.
use super::ListData;
use crate::geom::{Point, Size};

pub(crate) struct PaginationLayout {
    pub positions: Vec<Point>,
    pub sizes: Vec<Size>,
    pub content: Size,
}

impl ListData {
    pub(crate) fn pagination_layout(
        &self,
        sizes: &[Size],
        sources: &[Size],
        view: Size,
    ) -> PaginationLayout {
        let mut result = PaginationLayout {
            positions: vec![Point::ZERO; sizes.len()],
            sizes: sizes.to_vec(),
            content: Size::ZERO,
        };
        let (mut x, mut y, mut max_height, mut max_width) = (0_f32, 0_f32, 0_f32, 0_f32);
        let (mut page, mut column, mut row) = (0, 0, 0);
        let gx = self.column_gap as f32;
        let gy = self.line_gap as f32;
        let each_height = if self.auto_resize_item && self.line_count > 0 {
            ((view.height - (self.line_count - 1) as f32 * gy) / self.line_count as f32).floor()
        } else {
            0.
        };
        if self.auto_resize_item && self.column_count > 0 {
            for start in (0..sizes.len()).step_by(self.column_count as usize) {
                if (self.line_count != 0 && row >= self.line_count)
                    || (self.line_count == 0 && y + sizes[start].height > view.height)
                {
                    page += 1;
                    y = 0.;
                    row = 0;
                }
                let end = (start + self.column_count as usize).min(sizes.len());
                let line_size: f32 = sources[start..end].iter().map(|s| s.width).sum();
                let ratio = (view.width - line_size - (end - start - 1) as f32 * gx) / line_size;
                x = 0.;
                for i in start..end {
                    result.positions[i] = Point::new(page as f32 * view.width + x, y);
                    let width = if i + 1 < end {
                        sources[i].width + (sources[i].width * ratio + 0.5).floor()
                    } else {
                        view.width - x
                    };
                    let height = if self.line_count > 0 {
                        each_height
                    } else {
                        sizes[i].height
                    };
                    result.sizes[i] = Size::new(width, height);
                    if i + 1 < end {
                        x += width.ceil() + gx;
                    }
                    max_height = max_height.max(height);
                }
                y += max_height.ceil() + gy;
                max_height = 0.;
                row += 1;
            }
        } else {
            for (i, size) in sizes.iter().enumerate() {
                if x != 0. {
                    x += gx;
                }
                let height = if self.auto_resize_item && self.line_count > 0 {
                    each_height
                } else {
                    size.height
                };
                result.sizes[i].height = height;
                if (self.column_count != 0 && column >= self.column_count)
                    || (self.column_count == 0 && x + size.width > view.width && max_height != 0.)
                {
                    x = 0.;
                    y += max_height.ceil() + gy;
                    max_height = 0.;
                    column = 0;
                    row += 1;
                    if (self.line_count != 0 && row >= self.line_count)
                        || (self.line_count == 0 && y + height > view.height && max_width != 0.)
                    {
                        page += 1;
                        y = 0.;
                        row = 0;
                    }
                }
                result.positions[i] = Point::new(page as f32 * view.width + x, y);
                x += size.width.ceil();
                max_width = max_width.max(x);
                max_height = max_height.max(height);
                column += 1;
            }
        }
        result.content = Size::new(
            (page + 1) as f32 * view.width,
            if page > 0 {
                view.height
            } else {
                y + max_height.ceil()
            },
        );
        result
    }
}

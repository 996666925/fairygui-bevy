//! Materialized native GList bounds before its container alignment.
use super::{ListData, ListLayoutType};
use crate::geom::{Point, Size};

pub(crate) struct ListLayout {
    pub positions: Vec<Point>,
    pub sizes: Vec<Size>,
    pub content: Size,
}

impl ListData {
    pub(crate) fn materialized_layout(
        &self,
        sizes: &[Size],
        sources: &[Size],
        positions: &[Point],
        view: Size,
        flush_last: bool,
    ) -> ListLayout {
        if self.layout == ListLayoutType::Pagination {
            let n = if self.auto_resize_item && self.column_count > 0 && !flush_last {
                sizes.len() / self.column_count as usize * self.column_count as usize
            } else {
                sizes.len()
            };
            let mut result = self.pagination_layout(&sizes[..n], &sources[..n], view);
            result.positions.extend_from_slice(&positions[n..]);
            result.sizes.extend_from_slice(&sizes[n..]);
            return ListLayout {
                positions: result.positions,
                sizes: result.sizes,
                content: result.content,
            };
        }
        let mut result = ListLayout {
            positions: positions.to_vec(),
            sizes: sizes.to_vec(),
            content: Size::ZERO,
        };
        let vertical = matches!(
            self.layout,
            ListLayoutType::SingleColumn | ListLayoutType::FlowVertical
        );
        let single = matches!(
            self.layout,
            ListLayoutType::SingleColumn | ListLayoutType::SingleRow
        );
        let gap = if vertical {
            self.line_gap
        } else {
            self.column_gap
        } as f32;
        let cross_gap = if vertical {
            self.column_gap
        } else {
            self.line_gap
        } as f32;
        let limit = if vertical { view.height } else { view.width };
        let cross_limit = if vertical { view.width } else { view.height };
        let count = if vertical {
            self.line_count
        } else {
            self.column_count
        };
        let axes = |s: Size| {
            if vertical {
                (s.height, s.width)
            } else {
                (s.width, s.height)
            }
        };
        let size = |main, cross| {
            if vertical {
                Size::new(cross, main)
            } else {
                Size::new(main, cross)
            }
        };
        let point = |main, cross| {
            if vertical {
                Point::new(cross, main)
            } else {
                Point::new(main, cross)
            }
        };
        let (mut main, mut cross, mut max_main, mut max_cross) = (0_f32, 0_f32, 0_f32, 0_f32);
        if single {
            for (i, original) in sizes.iter().copied().enumerate() {
                if main != 0. {
                    main += gap;
                }
                let (length, mut width) = axes(original);
                let old_cross = if vertical {
                    positions[i].x
                } else {
                    positions[i].y
                };
                result.positions[i] = point(main, old_cross);
                if self.auto_resize_item {
                    width = cross_limit;
                }
                result.sizes[i] = size(length, width);
                main += length.ceil();
                max_cross = max_cross.max(width);
            }
            result.content = size(main, max_cross.ceil());
        } else if self.auto_resize_item && count > 0 {
            for start in (0..sizes.len()).step_by(count as usize) {
                let end = (start + count as usize).min(sizes.len());
                if end - start < count as usize && !flush_last {
                    break;
                }
                let total: f32 = sources[start..end].iter().map(|s| axes(*s).0).sum();
                let ratio = (limit - total - (end - start - 1) as f32 * gap) / total;
                main = 0.;
                for i in start..end {
                    let (_, width) = axes(sizes[i]);
                    let source = axes(sources[i]).0;
                    let length = if i + 1 < end {
                        source + (source * ratio + 0.5).floor()
                    } else {
                        limit - main
                    };
                    result.positions[i] = point(main, cross);
                    result.sizes[i] = size(length, width);
                    if i + 1 < end {
                        main += length.ceil() + gap;
                    }
                    max_cross = max_cross.max(width);
                }
                cross += max_cross.ceil() + cross_gap;
                max_cross = 0.;
            }
            // Native includes the final line gap for auto-sized flow rows.
            result.content = size(limit, cross + max_cross.ceil());
        } else {
            let mut n = 0;
            for (i, original) in sizes.iter().copied().enumerate() {
                let (length, width) = axes(original);
                if main != 0. {
                    main += gap;
                }
                if count != 0 && n >= count
                    || count == 0 && main + length > limit && max_cross != 0.
                {
                    main = 0.;
                    cross += max_cross.ceil() + cross_gap;
                    max_cross = 0.;
                    n = 0;
                }
                result.positions[i] = point(main, cross);
                main += length.ceil();
                max_main = max_main.max(main);
                max_cross = max_cross.max(width);
                n += 1;
            }
            result.content = size(max_main.ceil(), cross + max_cross.ceil());
        }
        result
    }
}

#[cfg(test)]
#[path = "list_layout_tests.rs"]
mod tests;

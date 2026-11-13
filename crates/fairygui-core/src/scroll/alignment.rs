//! Native refresh-time alignment for programmatic commands (not release inertia).
use super::{Point, Rect, ScrollAnimation, ScrollPane};

#[cfg(test)]
#[path = "alignment_tests.rs"]
mod tests;

/// GComponent's logical child-order search, also used by nonvirtual GList.
/// Rect dimensions are actualWidth/actualHeight, including absolute scale.
pub(crate) fn snapping_position(items: &[Rect], mut value: Point) -> Point {
    if items.is_empty() {
        return Point::ZERO;
    }
    let mut i = 0;
    if value.y != 0. {
        while i < items.len() {
            if value.y < items[i].y {
                value.y = if i == 0 {
                    0.
                } else {
                    let prev = items[i - 1];
                    if value.y < prev.y + prev.height / 2. {
                        prev.y
                    } else {
                        items[i].y
                    }
                };
                break;
            }
            i += 1;
        }
        if i == items.len() {
            value.y = items[i - 1].y;
        }
    }
    if value.x != 0. {
        i = i.saturating_sub(1);
        while i < items.len() {
            if value.x < items[i].x {
                value.x = if i == 0 {
                    0.
                } else {
                    let prev = items[i - 1];
                    if value.x < prev.x + prev.width / 2. {
                        prev.x
                    } else {
                        items[i].x
                    }
                };
                break;
            }
            i += 1;
        }
        if i == items.len() {
            value.x = items[i - 1].x;
        }
    }
    value
}

impl ScrollPane {
    /// Geometry for standalone panes. ObjectStore refreshes it from current
    /// logical children before native commands; registered bars are excluded.
    pub fn set_snap_items(&mut self, items: impl Into<Box<[Rect]>>) {
        self.snap_items = items.into();
        self.motion.virtual_snap = None;
    }

    pub fn set_virtual_snap_geometry(
        &mut self,
        geometry: std::sync::Arc<crate::widget::VirtualGeometry>,
    ) {
        self.snap_items = Box::default();
        self.motion.virtual_snap = Some(geometry);
    }

    fn item_snap_position(&self, value: Point, direction: Point) -> Point {
        self.motion.virtual_snap.as_ref().map_or_else(
            || snapping_position(&self.snap_items, value),
            |geometry| geometry.snapping_position(value, direction),
        )
    }

    fn aligned_position(&self, value: Point) -> Point {
        let max = self.max_scroll_pos();
        if self.page_mode {
            Point::new(
                align_page(
                    value.x,
                    self.display_pos.x,
                    self.view_size.width,
                    self.content_size.width,
                    max.x,
                    self.motion.paging_threshold,
                ),
                align_page(
                    value.y,
                    self.display_pos.y,
                    self.view_size.height,
                    self.content_size.height,
                    max.y,
                    self.motion.paging_threshold,
                ),
            )
        } else if self.snap_to_item {
            let snap = self.item_snap_position(value, Point::ZERO);
            Point::new(
                if value.x > 0. && value.x < max.x {
                    snap.x
                } else {
                    value.x
                },
                if value.y > 0. && value.y < max.y {
                    snap.y
                } else {
                    value.y
                },
            )
        } else {
            value
        }
    }

    pub(super) fn commit_command_position(&mut self, request: Point, animated: bool) -> bool {
        // Native setters compare their clamped request before deferred alignment.
        // A different request may align back to the same endpoint and refresh it.
        if request == self.pos {
            return false;
        }
        let next = self.aligned_position(request);
        if !next.x.is_finite() || !next.y.is_finite() {
            return false;
        }
        self.pos = next;
        self.raw_bounce = false;
        self.motion.tween = None;
        self.motion.pending_release = None;
        if animated && !self.dragging {
            let end = Point::new(next.x.floor(), next.y.floor());
            self.animation = (end != self.display_pos).then_some(ScrollAnimation {
                start: self.display_pos,
                end,
                elapsed: 0.,
            });
        } else {
            self.animation = None;
            self.display_pos = Point::new(next.x.ceil(), next.y.ceil());
            self.loop_check_current();
        }
        self.note_scroll();
        self.sync_bar_activity();
        true
    }
}

impl ScrollPane {
    pub(super) fn aligned_release_position(
        &self,
        mut target: [f64; 2],
        origin: [f64; 2],
        start: [f64; 2],
    ) -> [f64; 2] {
        let max = self.max_scroll_pos();
        if self.page_mode {
            let geometry = [
                (self.view_size.width, self.content_size.width, max.x),
                (self.view_size.height, self.content_size.height, max.y),
            ];
            for i in 0..2 {
                let (view, content, max) = geometry[i];
                if view <= 0. || !view.is_finite() {
                    continue;
                }
                let (view, content, max) = (f64::from(view), f64::from(content), f64::from(max));
                let last_page = (content / view).ceil() - 1.;
                let page_of = |value: f64| {
                    if value > 0. {
                        0.
                    } else if value < -max {
                        last_page
                    } else {
                        (-value / view).floor()
                    }
                };
                let mut page = page_of(target[i]);
                if target[i] <= 0. && target[i] >= -max {
                    let change = target[i] - origin[i];
                    let size = view.min(content - (page + 1.) * view);
                    let threshold = if change.abs() > view {
                        0.5
                    } else if change < 0. {
                        self.motion.paging_threshold
                    } else {
                        1. - self.motion.paging_threshold
                    };
                    if -target[i] - page * view > size * threshold {
                        page += 1.;
                    }
                    target[i] = -(page * view).min(max);
                }
                let old_page = page_of(start[i]);
                let first_page = (-origin[i] / view).floor();
                if (page - first_page).abs() > 1. && (old_page - first_page).abs() <= 1. {
                    page = if page > first_page {
                        first_page + 1.
                    } else {
                        first_page - 1.
                    };
                    target[i] = -page * view;
                }
            }
        } else if self.snap_to_item {
            let snap = self.item_snap_position(
                Point::new(-target[0] as f32, -target[1] as f32),
                Point::new(
                    (target[0] - origin[0]) as f32,
                    (target[1] - origin[1]) as f32,
                ),
            );
            if target[0] < 0. && target[0] > -f64::from(max.x) {
                target[0] = -f64::from(snap.x);
            }
            if target[1] < 0. && target[1] > -f64::from(max.y) {
                target[1] = -f64::from(snap.y);
            }
        }
        target
    }
}

fn align_page(
    value: f32,
    displayed: f32,
    view: f32,
    content: f32,
    overlap: f32,
    paging_threshold: f64,
) -> f32 {
    if !view.is_finite() || view <= 0. || value < 0. || value > overlap {
        return value;
    }
    // Inputs are model f32; native threshold arithmetic is JavaScript double.
    let (value, displayed, view, content) = (
        f64::from(value),
        f64::from(displayed),
        f64::from(view),
        f64::from(content),
    );
    let mut page = (value / view).floor();
    let change = displayed - value;
    let page_size = view.min(content - (page + 1.) * view);
    let delta = value - page * view;
    let threshold = if change.abs() > view {
        0.5
    } else if change < 0. {
        paging_threshold
    } else {
        1. - paging_threshold
    };
    if delta > page_size * threshold {
        page += 1.;
    }
    (page * view).min(f64::from(overlap)) as f32
}

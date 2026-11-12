//! Pull-refresh bounds, display geometry and release-time lock policy.
use super::{Point, ScrollPane};
use crate::geom::Size;
#[cfg(test)]
#[path = "refresh_tests.rs"]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullRelease {
    Down,
    Up,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RefreshBarLayout {
    pub visible: bool,
    pub position: Point,
    pub size: Size,
}

#[derive(Debug, Default, Clone)]
pub(super) struct RefreshState {
    header: Option<crate::object::GObjectId>,
    footer: Option<crate::object::GObjectId>,
    pub header_max: Option<Size>,
    pub footer_max: Option<Size>,
    header_locked: f32,
    footer_locked: f32,
}

impl ScrollPane {
    pub fn header(&self) -> Option<crate::object::GObjectId> {
        self.motion.refresh.header
    }
    pub fn footer(&self) -> Option<crate::object::GObjectId> {
        self.motion.refresh.footer
    }
    pub(crate) fn set_refresh_part(
        &mut self,
        header: bool,
        part: Option<crate::object::GObjectId>,
    ) {
        if header {
            self.motion.refresh.header = part;
        } else {
            self.motion.refresh.footer = part;
        }
    }
    pub(super) fn clamp_refresh_range(&mut self) {
        if self.refresh_axis().is_none() {
            return;
        }
        let max = self.max_scroll_pos();
        self.pos.x = self.pos.x.clamp(0., max.x);
        self.pos.y = self.pos.y.clamp(0., max.y);
        let bounds = self.motion_bounds();
        self.display_pos.x = self
            .display_pos
            .x
            .clamp(-bounds[0].1 as f32, -bounds[0].0 as f32);
        self.display_pos.y = self
            .display_pos
            .y
            .clamp(-bounds[1].1 as f32, -bounds[1].0 as f32);
    }
    /// Standalone panes supply resource presence and native maximum sizes.
    /// Zero maxima use the native half-viewport pull limit.
    pub fn set_refresh_max_sizes(&mut self, header: Option<Size>, footer: Option<Size>) -> bool {
        if [header, footer]
            .into_iter()
            .flatten()
            .any(|s| !s.width.is_finite() || !s.height.is_finite() || s.width < 0. || s.height < 0.)
        {
            return false;
        }
        if self.motion.refresh.header_max == header && self.motion.refresh.footer_max == footer {
            return false;
        }
        self.motion.refresh.header_max = header;
        self.motion.refresh.footer_max = footer;
        true
    }

    /// Both and Vertical use Y; Horizontal uses X. No refresh resource means
    /// native refreshBarAxis is absent, including for lock calls.
    pub fn refresh_axis(&self) -> Option<bool> {
        (self.motion.refresh.header_max.is_some()
            || self.motion.refresh.footer_max.is_some()
            || self.header().is_some()
            || self.footer().is_some()
            || self.header_res.is_some()
            || self.footer_res.is_some())
        .then_some(!matches!(self.scroll_type, super::ScrollType::Horizontal))
    }

    pub fn header_locked_size(&self) -> f32 {
        self.motion.refresh.header_locked
    }
    pub fn footer_locked_size(&self) -> f32 {
        self.motion.refresh.footer_locked
    }

    pub(super) fn footer_end(&self) -> f64 {
        let vertical = self.refresh_axis() == Some(true);
        let max = self.max_scroll_pos();
        let (max, content, view) = if vertical {
            (max.y, self.content_size.height, self.view_size.height)
        } else {
            (max.x, self.content_size.width, self.view_size.width)
        };
        if max == 0. {
            (f64::from(content) + f64::from(self.motion.refresh.footer_locked) - f64::from(view))
                .max(0.)
        } else {
            f64::from(max) + f64::from(self.motion.refresh.footer_locked)
        }
    }

    pub(super) fn motion_bounds(&self) -> [(f64, f64); 2] {
        let max = self.max_scroll_pos();
        let mut bounds = [(-f64::from(max.x), 0.), (-f64::from(max.y), 0.)];
        if let Some(vertical) = self.refresh_axis() {
            bounds[usize::from(vertical)] = (
                -self.footer_end(),
                f64::from(self.motion.refresh.header_locked),
            );
        }
        bounds
    }

    pub fn lock_header(&mut self, size: f32) -> bool {
        self.lock_refresh(size, true)
    }
    pub fn lock_footer(&mut self, size: f32) -> bool {
        self.lock_refresh(size, false)
    }

    fn lock_refresh(&mut self, size: f32, header: bool) -> bool {
        if !size.is_finite() || size < 0. {
            return false;
        }
        let current = if header {
            &mut self.motion.refresh.header_locked
        } else {
            &mut self.motion.refresh.footer_locked
        };
        if *current == size {
            return false;
        }
        *current = size;
        let Some(vertical) = self.refresh_axis() else {
            return true;
        };
        if self.motion.pending_release.is_some() {
            return true;
        }
        let i = usize::from(vertical);
        let start = [
            -f64::from(self.display_pos.x),
            -f64::from(self.display_pos.y),
        ];
        let max = self.max_scroll_pos();
        let overlap = if vertical { max.y } else { max.x };
        if header && start[i] >= 0. || !header && start[i] <= -f64::from(overlap) {
            let mut target = start;
            target[i] = if header {
                f64::from(size)
            } else {
                -self.footer_end()
            };
            self.start_release_tween(start, target, [0.3; 2], true);
        }
        true
    }

    /// checkRefreshBar changes only the scrolling dimension and the footer's
    /// position. Cross-axis sizing belongs to native handleSizeChanged.
    pub fn refresh_bar_layout(
        &self,
        header: bool,
        mut position: Point,
        mut size: Size,
    ) -> Option<RefreshBarLayout> {
        let vertical = self.refresh_axis()?;
        let max = self.max_scroll_pos();
        let (display, overlap, view, content) = if vertical {
            (
                self.display_pos.y,
                max.y,
                self.view_size.height,
                self.content_size.height,
            )
        } else {
            (
                self.display_pos.x,
                max.x,
                self.view_size.width,
                self.content_size.width,
            )
        };
        let visible = if header {
            display < 0.
        } else {
            display > overlap || overlap == 0. && self.motion.refresh.footer_locked > 0.
        };
        if visible {
            let extent = if header {
                -display
            } else {
                let pos = if overlap > 0. {
                    content - display
                } else {
                    (view - display)
                        .min(view - self.motion.refresh.footer_locked)
                        .max(view - content)
                };
                if vertical {
                    position.y = pos;
                } else {
                    position.x = pos;
                }
                if overlap > 0. {
                    display - overlap
                } else {
                    view - pos
                }
            };
            if vertical {
                size.height = extent;
            } else {
                size.width = extent;
            }
        }
        Some(RefreshBarLayout {
            visible,
            position,
            size,
        })
    }
}

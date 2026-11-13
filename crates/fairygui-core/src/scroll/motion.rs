//! Native accepted-drag sampling, release planning and type-2 cubic tweens.
use super::{Point, ScrollPane};

#[cfg(test)]
#[path = "motion_tests.rs"]
mod tests;

/// Native timing/velocity environment, independent of the rendering backend.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollPhysics {
    pub slow_frame_rate: bool,
    /// `Some(max(stage_width, stage_height))` enables native mobile thresholds.
    pub mobile_screen_extent: Option<f32>,
    pub deceleration_rate: f64,
}

impl Default for ScrollPhysics {
    fn default() -> Self {
        Self {
            slow_frame_rate: false,
            mobile_screen_extent: None,
            deceleration_rate: 0.967,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct Motion {
    pub scroll_revision: u64,
    pub paging_threshold: f64,
    pub default_scroll_step: f32,
    pub looping: super::looping::LoopState,
    pub virtual_snap: Option<std::sync::Arc<crate::widget::VirtualGeometry>>,
    pub refresh: super::refresh::RefreshState,
    pub physics: ScrollPhysics,
    pub tween: Option<ReleaseTween>,
    pub pending_release: Option<PendingRelease>,
    origin: [f64; 2],
    delta: [f64; 2],
    velocity: [f64; 2],
    velocity_scale: f64,
    idle: f64,
    moved: bool,
}

impl Default for Motion {
    fn default() -> Self {
        Self {
            scroll_revision: 0,
            paging_threshold: 0.3,
            default_scroll_step: super::SCROLL_STEP,
            looping: Default::default(),
            virtual_snap: None,
            refresh: Default::default(),
            physics: ScrollPhysics::default(),
            tween: None,
            pending_release: None,
            origin: [0.0; 2],
            delta: [0.0; 2],
            velocity: [0.0; 2],
            velocity_scale: 0.0,
            idle: 0.0,
            moved: false,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct PendingRelease {
    start: [f64; 2],
    target: [f64; 2],
    idle: f32,
}

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct AxisTween {
    start: f64,
    change: f64,
    elapsed: f64,
    duration: f64,
}

#[derive(Debug, Default, Clone)]
pub(super) struct ReleaseTween(pub [AxisTween; 2]);

impl AxisTween {
    fn advance(&mut self, current: f64, lower: f64, upper: f64, dt: f64, bounce: bool) -> f64 {
        if self.change == 0. {
            return current;
        }
        self.elapsed += dt;
        let mut value = if self.elapsed >= self.duration {
            let end = self.start + self.change;
            self.change = 0.;
            end
        } else {
            let t = self.elapsed / self.duration - 1.;
            self.start + (self.change * (t * t * t + 1.)).floor()
        };
        if bounce {
            let boundary = if value > upper + 20. && self.change > 0.
                || value > upper && self.change == 0.
            {
                Some(upper)
            } else if value < lower - 20. && self.change < 0. || value < lower && self.change == 0.
            {
                Some(lower)
            } else {
                None
            };
            if let Some(boundary) = boundary {
                self.elapsed = 0.;
                self.duration = 0.3;
                self.change = boundary - value;
                self.start = value;
            }
        } else if value > upper || value < lower {
            value = value.clamp(lower, upper);
            self.change = 0.;
        }
        value
    }
}

impl ScrollPane {
    pub(super) fn translate_release_origin(&mut self, offset: Point) {
        if let Some(tween) = &mut self.motion.tween {
            tween.0[0].start -= f64::from(offset.x);
            tween.0[1].start -= f64::from(offset.y);
        }
    }
    pub(super) fn translate_drag_origin(&mut self, offset: Point) {
        self.motion.origin[0] -= f64::from(offset.x);
        self.motion.origin[1] -= f64::from(offset.y);
    }
    pub fn physics(&self) -> ScrollPhysics {
        self.motion.physics
    }

    /// Invalid rates/extents leave the current environment intact.
    pub fn set_physics(&mut self, physics: ScrollPhysics) -> bool {
        if !physics.deceleration_rate.is_finite()
            || physics.deceleration_rate <= 0.
            || physics.deceleration_rate >= 1.
            || physics
                .mobile_screen_extent
                .is_some_and(|v| !v.is_finite() || v <= 0.)
            || physics == self.motion.physics
        {
            return false;
        }
        self.motion.physics = physics;
        true
    }

    /// Begin an accepted content gesture. Type-1 cancellation commits its end;
    /// cancelling a type-2 release holds the current display, as killTween does.
    pub fn begin_drag(&mut self) {
        if !self.touch_effect {
            return;
        }
        if let Some(animation) = self.animation.take() {
            self.display_pos = animation.end;
            self.note_scroll();
        }
        self.motion.tween = None;
        self.motion.pending_release = None;
        self.motion.origin = [
            -f64::from(self.display_pos.x),
            -f64::from(self.display_pos.y),
        ];
        self.motion.delta = [0.; 2];
        self.motion.velocity = [0.; 2];
        self.motion.velocity_scale = 1.;
        self.motion.idle = 0.;
        self.motion.moved = false;
        self.raw_bounce = false;
        self.dragging = true;
        self.sync_bar_activity();
    }

    /// Local pointer displacement and time since the preceding sample. Zero
    /// displacement represents a stationary hold; it ages velocity at release.
    pub fn drag(&mut self, dx: f32, dy: f32, dt: f32) {
        self.drag_with_scale(dx, dy, dt, Point::new(1., 1.));
    }

    /// `scale` is local-to-stage scale, used by native screen-speed thresholds.
    pub fn drag_with_scale(&mut self, dx: f32, dy: f32, dt: f32, scale: Point) {
        if !self.touch_effect
            || ![dx, dy, dt, scale.x, scale.y]
                .into_iter()
                .all(f32::is_finite)
            || dt < 0.
        {
            return;
        }
        if !self.dragging {
            self.begin_drag();
        }
        if dx == 0. && dy == 0. {
            self.motion.idle += f64::from(dt);
            return;
        }
        let max = self.max_scroll_pos();
        let frame_rate = if self.motion.physics.slow_frame_rate {
            30.
        } else {
            60.
        };
        let dt = (f64::from(dt) + self.motion.idle).max(1. / frame_rate);
        let elapsed = dt * frame_rate - 1.;
        let axes = [
            (
                dx,
                self.scroll_type.allows_horizontal(),
                self.view_size.width,
                max.x,
            ),
            (
                dy,
                self.scroll_type.allows_vertical(),
                self.view_size.height,
                max.y,
            ),
        ];
        let mut display = [self.display_pos.x, self.display_pos.y];
        for (i, (delta, enabled, view, max)) in axes.into_iter().enumerate() {
            let delta = if enabled { f64::from(delta) } else { 0. };
            self.motion.delta[i] += delta;
            let mut container = (self.motion.origin[i] + self.motion.delta[i]).floor();
            let (max, view) = (f64::from(max), f64::from(view));
            let cap = |size: Option<crate::geom::Size>| {
                let limit = size.map_or(0., |s| if i == 0 { s.width } else { s.height });
                if limit > 0. {
                    f64::from(limit)
                } else {
                    view * 0.5
                }
            };
            if container > 0. {
                container = if self.bounceback_effect {
                    (container * 0.5)
                        .min(cap(self.motion.refresh.header_max))
                        .floor()
                } else {
                    0.
                };
            } else if container < -max {
                container = if self.bounceback_effect {
                    (((container + max) * 0.5).max(-cap(self.motion.refresh.footer_max)) - max)
                        .floor()
                } else {
                    -max
                };
            }
            if enabled {
                display[i] = -container as f32;
            }
            let v = &mut self.motion.velocity[i];
            if elapsed > 1. {
                *v *= 0.833_f64.powf(elapsed);
            }
            *v += dt * 10. * (delta * 60. / frame_rate / dt - *v);
        }
        if dx != 0. && self.scroll_type.allows_horizontal() {
            self.motion.velocity_scale = f64::from(scale.x).abs();
        } else if dy != 0. && self.scroll_type.allows_vertical() {
            self.motion.velocity_scale = f64::from(scale.y).abs();
        }
        self.motion.idle = 0.;
        self.motion.moved = true;
        self.display_pos = Point::new(display[0], display[1]);
        self.sync_release_position();
        let before_loop = self.display_pos;
        if self.loop_check_current() {
            self.translate_drag_origin(Point::new(
                self.display_pos.x - before_loop.x,
                self.display_pos.y - before_loop.y,
            ));
        }
        self.note_scroll();
        self.sync_bar_activity();
    }

    pub fn end_drag(&mut self) {
        self.end_drag_after(0.);
    }

    /// Release after an additional stationary interval since the last sample.
    pub fn end_drag_after(&mut self, idle: f32) {
        self.defer_drag_release(idle);
        self.complete_drag_release();
    }

    /// Queue-aware consumers publish this target-only pull notification, allow
    /// responses to assign locks, then complete before advancing the tween.
    pub fn defer_drag_release(&mut self, idle: f32) -> Option<super::PullRelease> {
        if !self.dragging || !idle.is_finite() || idle < 0. {
            return None;
        }
        self.dragging = false;
        if !self.touch_effect || !self.motion.moved {
            self.sync_bar_activity();
            return None;
        }
        let max = self.max_scroll_pos();
        let start = [
            -f64::from(self.display_pos.x),
            -f64::from(self.display_pos.y),
        ];
        let bounds = [f64::from(max.x), f64::from(max.y)];
        let target = std::array::from_fn(|i| start[i].clamp(-bounds[i], 0.));
        self.motion.pending_release = Some(PendingRelease {
            start,
            target,
            idle,
        });
        let change = [target[0] - start[0], target[1] - start[1]];
        let event = if change.into_iter().any(|v| v < -10.) {
            Some(super::PullRelease::Down)
        } else if change.into_iter().any(|v| v > 10.) {
            Some(super::PullRelease::Up)
        } else {
            None
        };
        self.sync_bar_activity();
        event
    }

    pub fn complete_drag_release(&mut self) -> bool {
        let Some(pending) = self.motion.pending_release.take() else {
            return false;
        };
        let mut start = pending.start;
        let idle = pending.idle;
        let outside = pending.target != start;
        let mut target = if outside { pending.target } else { start };
        let mut duration = [0.3; 2];
        if outside {
            if let Some(vertical) = self.refresh_axis() {
                let i = usize::from(vertical);
                let max = self.max_scroll_pos();
                let overlap = if vertical { max.y } else { max.x };
                if self.header_locked_size() > 0. && target[i] == 0. {
                    target[i] = f64::from(self.header_locked_size());
                } else if self.footer_locked_size() > 0. && target[i] == -f64::from(overlap) {
                    target[i] = -self.footer_end();
                }
            }
        } else {
            if !self.inertia_disabled {
                let rate = if self.motion.physics.slow_frame_rate {
                    30.
                } else {
                    60.
                };
                let elapsed = (self.motion.idle + f64::from(idle)) * rate - 1.;
                for i in 0..2 {
                    let mut v = self.motion.velocity[i];
                    if elapsed > 1. {
                        v *= 0.833_f64.powf(elapsed);
                    }
                    let mut screen_speed = v.abs() * self.motion.velocity_scale;
                    if let Some(extent) = self.motion.physics.mobile_screen_extent {
                        screen_speed *= 1136. / f64::from(extent);
                    }
                    let threshold =
                        if self.page_mode || self.motion.physics.mobile_screen_extent.is_none() {
                            500.
                        } else {
                            1000.
                        };
                    if screen_speed > threshold {
                        let ratio = ((screen_speed - threshold) / threshold).powi(2).min(1.);
                        screen_speed *= ratio;
                        v *= ratio;
                        let time = (60. / screen_speed).ln()
                            / self.motion.physics.deceleration_rate.ln()
                            / 60.;
                        target[i] += (v * time * 0.4).floor();
                        duration[i] = time.max(0.3);
                    }
                }
            }
            let old_target = target;
            let old_start = start;
            self.loop_check_target(&mut start, &mut target);
            target = self.aligned_release_position(target, self.motion.origin, start);
            for i in 0..2 {
                let change = target[i] - start[i];
                let old_change = old_target[i] - old_start[i];
                if (self.page_mode || self.snap_to_item)
                    && change != 0.
                    && change.abs() < old_change.abs()
                {
                    duration[i] = ((change / old_change).abs() * duration[i]).max(0.3);
                }
            }
        }
        self.start_release_tween(start, target, duration, false);
        true
    }

    pub(super) fn start_release_tween(
        &mut self,
        start: [f64; 2],
        target: [f64; 2],
        duration: [f64; 2],
        force: bool,
    ) {
        self.animation = None;
        self.raw_bounce = false;
        self.motion.tween = (force || target != start).then(|| {
            ReleaseTween(std::array::from_fn(|i| AxisTween {
                start: start[i],
                change: target[i] - start[i],
                elapsed: 0.,
                duration: duration[i],
            }))
        });
        self.sync_bar_activity();
    }

    fn sync_release_position(&mut self) {
        let max = self.max_scroll_pos();
        if max.x > 0. {
            self.pos.x = self.display_pos.x.clamp(0., max.x);
        }
        if max.y > 0. {
            self.pos.y = self.display_pos.y.clamp(0., max.y);
        }
    }

    pub(super) fn advance_release(&mut self, dt: f32) -> bool {
        if self.motion.tween.is_some() {
            self.note_scroll();
        }
        let bounds = self.motion_bounds();
        let Some(tween) = &mut self.motion.tween else {
            return false;
        };
        let before = self.display_pos;
        self.display_pos = Point::new(
            -tween.0[0].advance(
                -f64::from(before.x),
                bounds[0].0,
                bounds[0].1,
                f64::from(dt),
                self.bounceback_effect,
            ) as f32,
            -tween.0[1].advance(
                -f64::from(before.y),
                bounds[1].0,
                bounds[1].1,
                f64::from(dt),
                self.bounceback_effect,
            ) as f32,
        );
        if tween.0.iter().all(|axis| axis.change == 0.) {
            self.motion.tween = None;
        }
        self.sync_release_position();
        if self.motion.tween.is_none() {
            self.loop_check_current();
        }
        self.sync_bar_activity();
        before != self.display_pos
    }
}

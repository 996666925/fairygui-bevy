//! 滚动容器。
//!
//! 配置来自组件描述块 7：两个字节的方向与显示方式、一个 32 位标志位、
//! 可选的滚动条边距，以及四个资源地址（纵横滚动条、页头、页脚）。

use super::visibility::BarVisibility;
#[path = "alignment.rs"]
mod alignment;
#[path = "motion.rs"]
mod motion;
pub use motion::ScrollPhysics;
#[path = "refresh.rs"]
mod refresh;
pub use refresh::{PullRelease, RefreshBarLayout};
#[path = "pages.rs"]
mod pages;
pub(crate) use alignment::snapping_position;
#[path = "looping.rs"]
mod looping;
#[path = "resize.rs"]
mod resize;
pub use looping::ScrollLoop;
#[cfg(test)]
#[path = "visibility_tests.rs"]
mod visibility_tests;
#[path = "wheel.rs"]
mod wheel;
use crate::binary::{ByteBuffer, StringTable};
use crate::error::Result;
use crate::geom::{Margin, Point, Rect, Size};
use crate::object::GObjectId;

/// 滚动方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollType {
    /// 只允许横向滚动。
    Horizontal,
    /// 只允许纵向滚动。
    #[default]
    Vertical,
    /// 两个方向都可以滚动。
    Both,
}

impl ScrollType {
    /// 由描述中的字节编号构造。
    pub const fn from_code(code: i8) -> Self {
        match code {
            0 => Self::Horizontal,
            2 => Self::Both,
            _ => Self::Vertical,
        }
    }

    /// 是否允许横向滚动。
    pub const fn allows_horizontal(self) -> bool {
        matches!(self, Self::Horizontal | Self::Both)
    }

    /// 是否允许纵向滚动。
    pub const fn allows_vertical(self) -> bool {
        matches!(self, Self::Vertical | Self::Both)
    }
}

/// 滚动条的显示方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollBarDisplayType {
    /// 交给全局默认值决定。
    #[default]
    Default,
    /// 始终显示。
    Visible,
    /// 需要时显示。
    Auto,
    /// 不显示。
    Hidden,
}

impl ScrollBarDisplayType {
    /// 由描述中的字节编号构造。
    pub const fn from_code(code: i8) -> Self {
        match code {
            1 => Self::Visible,
            2 => Self::Auto,
            3 => Self::Hidden,
            _ => Self::Default,
        }
    }
}

/// 滚动容器。
#[derive(Debug, Clone)]
pub struct ScrollPane {
    /// 滚动方向。
    pub scroll_type: ScrollType,
    /// 滚动条显示方式。
    pub bar_display: ScrollBarDisplayType,

    /// 滚动条是否放在左侧或上侧。
    pub display_on_left: bool,
    /// 停止滚动时是否吸附到最近的列表项。
    pub snap_to_item: bool,
    /// 内容放得下时是否隐藏对应轴的整条滚动条。
    pub display_in_demand: bool,
    /// 是否按页滚动。
    pub page_mode: bool,
    /// 是否允许拖拽滚动。
    pub touch_effect: bool,
    /// 越界后是否回弹。
    pub bounceback_effect: bool,
    /// 是否禁用惯性。
    pub inertia_disabled: bool,
    /// 是否裁剪超出范围的内容。
    pub clip: bool,
    /// 滚动条是否浮在内容之上。
    pub floating: bool,
    /// 是否不裁剪边距内的内容。
    pub dont_clip_margin: bool,
    mouse_wheel_enabled: bool,
    scroll_step: f32,

    /// 滚动条边距。
    pub scroll_bar_margin: Margin,
    /// 纵向滚动条的资源地址。
    pub vertical_bar_res: Option<String>,
    /// 横向滚动条的资源地址。
    pub horizontal_bar_res: Option<String>,
    /// 页头资源地址。
    pub header_res: Option<String>,
    /// 页脚资源地址。
    pub footer_res: Option<String>,

    /// 已实例化的纵向滚动条对象。
    pub vt_bar: Option<GObjectId>,
    /// 已实例化的横向滚动条对象。
    pub hz_bar: Option<GObjectId>,

    /// 当前滚动位置。
    pos: Point,
    /// Display offset while a programmatic scroll is approaching pos.
    display_pos: Point,
    animation: Option<ScrollAnimation>,
    /// 视口尺寸。
    view_size: Size,
    /// 内容尺寸。
    content_size: Size,
    motion: Box<motion::Motion>,
    /// 是否正在被拖动。
    dragging: bool,
    raw_bounce: bool,
    snap_items: Box<[Rect]>,
    // Keep optional pane state from inflating every ComponentData/ControlData.
    bars: Box<[BarVisibility; 2]>,
    bar_mode: ScrollBarDisplayType,
    bar_moving: bool,
}

#[derive(Debug, Clone)]
struct ScrollAnimation {
    start: Point,
    // Native start + remaining tweenChange. A boundary clears that axis's
    // change, so killTween restores its start while another axis is active.
    end: Point,
    elapsed: f32,
}

impl Default for ScrollPane {
    fn default() -> Self {
        Self {
            scroll_type: ScrollType::Vertical,
            bar_display: ScrollBarDisplayType::Default,
            display_on_left: false,
            snap_to_item: false,
            display_in_demand: false,
            page_mode: false,
            touch_effect: true,
            bounceback_effect: true,
            inertia_disabled: false,
            clip: true,
            floating: false,
            dont_clip_margin: false,
            mouse_wheel_enabled: true,
            scroll_step: SCROLL_STEP,
            scroll_bar_margin: Margin::ZERO,
            vertical_bar_res: None,
            horizontal_bar_res: None,
            header_res: None,
            footer_res: None,
            vt_bar: None,
            hz_bar: None,
            pos: Point::ZERO,
            display_pos: Point::ZERO,
            animation: None,
            view_size: Size::ZERO,
            content_size: Size::ZERO,
            motion: Box::default(),
            dragging: false,
            raw_bounce: false,
            snap_items: Box::default(),
            bars: Box::new([BarVisibility::default(); 2]),
            bar_mode: ScrollBarDisplayType::Default,
            bar_moving: false,
        }
    }
}

/// 回弹时每帧回归边界的比例。
const BOUNCE_RATE: f32 = 0.5;
/// 非页模式下一次滚动箭头的步长（像素），与官方一致。
pub const SCROLL_STEP: f32 = 25.0;

impl ScrollPane {
    pub(crate) fn scroll_revision(&self) -> u64 {
        self.motion.scroll_revision
    }

    pub(super) fn note_scroll(&mut self) {
        self.motion.scroll_revision = self.motion.scroll_revision.wrapping_add(1);
    }
    /// 读取滚动配置。`buffer` 的位置应当是块 7 的起点。
    pub fn setup(&mut self, buffer: &mut ByteBuffer<'_>, table: &StringTable) -> Result<()> {
        self.scroll_type = ScrollType::from_code(buffer.read_i8()?);
        self.bar_display = ScrollBarDisplayType::from_code(buffer.read_i8()?);
        self.mouse_wheel_enabled = self.bar_display != ScrollBarDisplayType::Hidden;

        let flags = buffer.read_i32()? as u32;

        if buffer.read_bool()? {
            self.scroll_bar_margin.top = buffer.read_i32()? as f32;
            self.scroll_bar_margin.bottom = buffer.read_i32()? as f32;
            self.scroll_bar_margin.left = buffer.read_i32()? as f32;
            self.scroll_bar_margin.right = buffer.read_i32()? as f32;
        }

        self.vertical_bar_res = buffer.read_s(table)?;
        self.horizontal_bar_res = buffer.read_s(table)?;
        self.header_res = buffer.read_s(table)?;
        self.footer_res = buffer.read_s(table)?;

        self.display_on_left = flags & 1 != 0;
        self.snap_to_item = flags & 2 != 0;
        self.display_in_demand = flags & 4 != 0;
        self.page_mode = flags & 8 != 0;

        // 触摸效果：两个位分表示「强制开」「强制关」，都没设时用全局默认。
        if flags & 16 != 0 {
            self.touch_effect = true;
        } else if flags & 32 != 0 {
            self.touch_effect = false;
        }

        // 回弹同理。
        if flags & 64 != 0 {
            self.bounceback_effect = true;
        } else if flags & 128 != 0 {
            self.bounceback_effect = false;
        }

        self.inertia_disabled = flags & 256 != 0;
        // 该位为 0 时才设置裁剪区域。
        self.clip = flags & 512 == 0;
        self.floating = flags & 1024 != 0;
        self.dont_clip_margin = flags & 2048 != 0;

        // 按需显示模式下初始是隐藏的——此时还没有滚动过。与官方一致：
        // 它在创建滚动条后立刻把两条都设为不可见。
        self.sync_bar_mode();

        Ok(())
    }

    /// 当前滚动位置。
    pub fn pos(&self) -> Point {
        self.pos
    }

    /// Current content offset. pos() is the requested endpoint during a tween.
    pub fn display_pos(&self) -> Point {
        self.display_pos
    }

    pub fn is_animating(&self) -> bool {
        self.animation.is_some()
            || self.motion.tween.is_some()
            || self.motion.pending_release.is_some()
    }

    /// Native ScrollPane.scrollToView rectangle policy, in content coordinates.
    /// Small items entering at the trailing edge leave one item of extra room.
    pub fn scroll_to_view(&mut self, rect: Rect, animated: bool, set_first: bool) -> bool {
        if ![rect.x, rect.y, rect.width, rect.height]
            .into_iter()
            .all(f32::is_finite)
        {
            return false;
        }
        let max = self.max_scroll_pos();
        let axis = |start: f32, extent: f32, pos: f32, view: f32, overlap: f32| {
            if overlap <= 0. || view <= 0. {
                return pos;
            }
            if set_first || start <= pos || extent >= view {
                if self.page_mode {
                    (start / view).floor() * view
                } else {
                    start
                }
            } else if start + extent > pos + view {
                if self.page_mode {
                    (start / view).floor() * view
                } else if extent <= view / 2. {
                    start + extent * 2. - view
                } else {
                    start + extent - view
                }
            } else {
                pos
            }
        };
        self.set_pos_animated(
            axis(rect.x, rect.width, self.pos.x, self.view_size.width, max.x),
            axis(
                rect.y,
                rect.height,
                self.pos.y,
                self.view_size.height,
                max.y,
            ),
            animated,
        )
    }

    /// Programmatic scroll with native half-second cubic-out timing. A repeated
    /// endpoint preserves progress. Dragging uses the immediate setter instead.
    pub fn set_pos_animated(&mut self, x: f32, y: f32, animated: bool) -> bool {
        if !x.is_finite() || !y.is_finite() {
            return false;
        }
        let max = self.max_scroll_pos();
        let x = self.loop_check_new_pos(x, false);
        let y = self.loop_check_new_pos(y, true);
        let next = Point::new(
            if self.scroll_type.allows_horizontal() {
                x.clamp(0., max.x)
            } else {
                self.pos.x
            },
            if self.scroll_type.allows_vertical() {
                y.clamp(0., max.y)
            } else {
                self.pos.y
            },
        );
        self.commit_command_position(next, animated)
    }

    /// 视口尺寸。
    pub fn view_size(&self) -> Size {
        self.view_size
    }

    /// 内容尺寸。
    pub fn content_size(&self) -> Size {
        self.content_size
    }

    /// Native isChildInView uses unscaled bounds and includes exact edges.
    pub fn is_child_in_view(&self, rect: Rect) -> bool {
        let max = self.max_scroll_pos();
        !(max.y > 0.
            && (rect.y - self.display_pos.y < -rect.height
                || rect.y - self.display_pos.y > self.view_size.height)
            || max.x > 0.
                && (rect.x - self.display_pos.x < -rect.width
                    || rect.x - self.display_pos.x > self.view_size.width))
    }

    /// 视口尺寸变化。
    pub fn set_view_size(&mut self, size: Size) {
        if self.view_size != size {
            self.view_size = size;
            self.clamp_refresh_range();
            self.refresh_bar_visibility();
        }
    }

    /// 内容尺寸变化。如果当前位置越界则收回到边界内。
    pub fn set_content_size(&mut self, size: Size) {
        if self.content_size != size {
            self.content_size = size;
            if self.refresh_axis().is_some() {
                self.clamp_refresh_range();
            } else {
                self.clamp_pos();
            }
            self.refresh_bar_visibility();
        }
    }

    /// 可滚动的最大位置。内容小于视口时为 0。
    pub fn max_scroll_pos(&self) -> Point {
        Point::new(
            if self.scroll_type.allows_horizontal() {
                (self.content_size.width - self.view_size.width)
                    .max(0.)
                    .ceil()
            } else {
                0.
            },
            if self.scroll_type.allows_vertical() {
                (self.content_size.height - self.view_size.height)
                    .max(0.)
                    .ceil()
            } else {
                0.
            },
        )
    }

    /// 是否可滚动（内容超出视口）。
    pub fn is_scrollable(&self) -> bool {
        let max = self.max_scroll_pos();
        (self.scroll_type.allows_horizontal() && max.x > 0.0)
            || (self.scroll_type.allows_vertical() && max.y > 0.0)
    }

    /// 设置滚动位置。
    ///
    /// `allow_bounce` 为真时允许暂时越界（拖拽中或惯性未收敛时会用到）。
    /// 返回位置是否发生变化。
    pub fn set_pos(&mut self, x: f32, y: f32, allow_bounce: bool) -> bool {
        if !x.is_finite() || !y.is_finite() {
            return false;
        }
        let (mut x, mut y) = (x, y);

        if !self.scroll_type.allows_horizontal() {
            x = self.pos.x;
        }
        if !self.scroll_type.allows_vertical() {
            y = self.pos.y;
        }

        if !allow_bounce {
            let max = self.max_scroll_pos();
            x = x.clamp(0.0, max.x);
            y = y.clamp(0.0, max.y);
        }

        if x == self.pos.x && y == self.pos.y {
            return false;
        }
        self.pos = Point::new(x, y);
        self.raw_bounce = allow_bounce;
        self.display_pos = self.pos;
        self.animation = None;
        self.motion.tween = None;
        self.motion.pending_release = None;
        self.note_scroll();
        self.sync_bar_activity();
        true
    }

    /// 把位置收回合法范围。
    pub fn clamp_pos(&mut self) {
        let current = self.pos;
        self.set_pos(current.x, current.y, false);
    }

    /// 停止滚动。
    pub fn stop(&mut self) {
        if let Some(animation) = self.animation.take() {
            self.display_pos = animation.end;
            self.note_scroll();
        }
        self.motion.tween = None;
        self.motion.pending_release = None;
        self.dragging = false;
        self.sync_bar_activity();
    }

    /// 每帧推进：先按惯性滑动，再处理越界回弹。
    ///
    /// 返回滚动位置是否发生变化。
    pub fn advance(&mut self, dt: f32) -> bool {
        if !dt.is_finite() || dt <= 0.0 {
            return false;
        }

        self.complete_drag_release();

        if self.motion.tween.is_some() {
            return self.advance_release(dt);
        }

        let bounds = self.motion_bounds();
        if self.animation.is_some() {
            self.note_scroll();
        }
        if let Some(animation) = &mut self.animation {
            let before = self.display_pos;
            animation.elapsed += dt;
            let t = animation.elapsed / 0.5 - 1.;
            let ratio = t * t * t + 1.;
            let axis = |start: f32, end: f32, current: f32| {
                if start == end {
                    current
                } else if animation.elapsed >= 0.5 {
                    end
                } else {
                    start - ((start - end) * ratio).floor()
                }
            };
            // Native zero-change axes retain their current container position.
            self.display_pos = Point::new(
                axis(animation.start.x, animation.end.x, before.x),
                axis(animation.start.y, animation.end.y, before.y),
            );
            // runTween(type=1) stops an axis as soon as it hits a boundary,
            // even if snapping left the logical target beyond that boundary.
            let clamped = Point::new(
                self.display_pos
                    .x
                    .clamp(-bounds[0].1 as f32, -bounds[0].0 as f32),
                self.display_pos
                    .y
                    .clamp(-bounds[1].1 as f32, -bounds[1].0 as f32),
            );
            if clamped.x != self.display_pos.x {
                animation.end.x = animation.start.x;
            }
            if clamped.y != self.display_pos.y {
                animation.end.y = animation.start.y;
            }
            self.display_pos = clamped;
            if animation.elapsed >= 0.5 || animation.start == animation.end {
                self.animation = None;
                self.loop_check_current();
            }
            self.sync_bar_activity();
            return self.display_pos != before;
        }

        let before = self.pos;

        // 越界回弹
        if !self.dragging && self.bounceback_effect && self.raw_bounce {
            let max = self.max_scroll_pos();
            let blend = 1.0 - BOUNCE_RATE.powf(dt * 60.0);

            if self.pos.x < 0.0 {
                self.pos.x += (0.0 - self.pos.x) * blend;
            } else if self.pos.x > max.x {
                self.pos.x += (max.x - self.pos.x) * blend;
            }

            if self.pos.y < 0.0 {
                self.pos.y += (0.0 - self.pos.y) * blend;
            } else if self.pos.y > max.y {
                self.pos.y += (max.y - self.pos.y) * blend;
            }
        }

        if self.pos != before {
            self.display_pos = self.pos;
        }
        self.sync_bar_activity();
        self.pos.x != before.x || self.pos.y != before.y
    }

    fn bars_are_moving(&self) -> bool {
        let max = self.max_scroll_pos();
        self.dragging
            || self.is_animating()
            || (self.raw_bounce
                && self.bounceback_effect
                && (self.pos.x < 0. || self.pos.y < 0. || self.pos.x > max.x || self.pos.y > max.y))
    }

    fn sync_bar_mode(&mut self) {
        if self.bar_mode != self.bar_display {
            self.bar_mode = self.bar_display;
            let auto = self.bar_display == ScrollBarDisplayType::Auto;
            let moving = self.bars_are_moving();
            for bar in self.bars.iter_mut() {
                bar.reset(auto, moving);
            }
        }
    }

    fn sync_bar_activity(&mut self) {
        self.sync_bar_mode();
        let moving = self.bars_are_moving();
        if moving != self.bar_moving {
            self.bar_moving = moving;
            self.refresh_bar_visibility();
        }
    }

    /// Re-evaluate the native visibility policy at a size/drag/tween transition.
    /// Repeated idle refreshes restart an existing fade, as GTween.kill/to does.
    pub fn refresh_bar_visibility(&mut self) {
        self.sync_bar_mode();
        let moving = self.bars_are_moving();
        self.bar_moving = moving;
        let auto = self.bar_display == ScrollBarDisplayType::Auto;
        for bar in self.bars.iter_mut() {
            bar.refresh(auto, moving);
        }
    }

    pub(crate) fn set_bar_available(&mut self, vertical: bool, available: bool) {
        self.sync_bar_mode();
        let i = usize::from(vertical);
        if self.bars[i].available != available {
            self.bars[i].available = available;
            let auto = self.bar_display == ScrollBarDisplayType::Auto;
            let moving = self.bars_are_moving();
            self.bars[i].refresh(auto, moving);
        }
    }

    pub(crate) fn reset_bar_visibility(&mut self, vertical: bool) {
        self.sync_bar_mode();
        let moving = self.bars_are_moving();
        let auto = self.bar_display == ScrollBarDisplayType::Auto;
        let bar = &mut self.bars[usize::from(vertical)];
        bar.grip_dragging = false;
        bar.reset(auto, moving);
    }

    pub(crate) fn override_bar_visible(&mut self, vertical: bool, visible: bool) {
        self.sync_bar_mode();
        self.bars[usize::from(vertical)].visible = visible;
    }

    pub(crate) fn override_bar_alpha(&mut self, vertical: bool, alpha: f32) {
        self.sync_bar_mode();
        self.bars[usize::from(vertical)].alpha = alpha;
    }

    pub fn set_grip_dragging(&mut self, vertical: bool, dragging: bool) -> bool {
        self.sync_bar_mode();
        let bar = &mut self.bars[usize::from(vertical)];
        if bar.grip_dragging == dragging {
            return false;
        }
        bar.grip_dragging = dragging;
        self.refresh_bar_visibility();
        true
    }

    pub fn grip_dragging(&self, vertical: bool) -> bool {
        self.bars[usize::from(vertical)].grip_dragging
    }

    /// Explicitly reveal both bars, then apply the native idle fade policy.
    /// Immediate position changes alone do not reveal a hidden native auto bar.
    pub fn touch_bars(&mut self) {
        self.sync_bar_mode();
        for bar in self.bars.iter_mut() {
            bar.alpha = 1.;
            bar.visible = bar.available;
        }
        self.refresh_bar_visibility();
    }

    /// Alpha of the primary axis (vertical except on horizontal-only panes).
    pub fn bar_alpha(&self) -> f32 {
        self.bar_alpha_for(self.scroll_type != ScrollType::Horizontal)
    }

    pub fn bar_alpha_for(&self, vertical: bool) -> f32 {
        if self.bar_mode != self.bar_display {
            1.
        } else {
            self.bars[usize::from(vertical)].alpha
        }
    }

    pub fn bar_visible_for(&self, vertical: bool) -> bool {
        let bar = &self.bars[usize::from(vertical)];
        if self.bar_mode != self.bar_display {
            bar.available
                && (self.bar_display != ScrollBarDisplayType::Auto
                    || self.bars_are_moving()
                    || bar.grip_dragging)
        } else {
            bar.visible
        }
    }

    /// Native half-second delay followed by a half-second QuadOut alpha tween.
    /// Fade completion hides the bar and restores alpha to one.
    pub fn advance_bar_fade(&mut self, dt: f32) -> bool {
        if !dt.is_finite() || dt <= 0. {
            return false;
        }
        let before = (*self.bars).map(|bar| (bar.visible, bar.alpha));
        self.sync_bar_activity();
        if self.bar_display == ScrollBarDisplayType::Auto {
            for bar in self.bars.iter_mut() {
                bar.advance(dt);
            }
        }
        before != (*self.bars).map(|bar| (bar.visible, bar.alpha))
    }

    pub fn bars_visible(&self) -> bool {
        (self.scroll_type.allows_vertical() && self.bar_visible_for(true))
            || (self.scroll_type.allows_horizontal() && self.bar_visible_for(false))
    }

    /// 横向滚动比例：0 在起点，1 在终点。
    pub fn perc_x(&self) -> f32 {
        let max = self.max_scroll_pos().x;
        if max <= 0.0 {
            0.0
        } else {
            (self.pos.x / max).clamp(0.0, 1.0)
        }
    }

    pub fn display_perc_x(&self) -> f32 {
        let max = self.max_scroll_pos().x;
        if max <= 0. {
            0.
        } else {
            (self.display_pos.x / max).clamp(0., 1.)
        }
    }

    pub fn display_perc_y(&self) -> f32 {
        let max = self.max_scroll_pos().y;
        if max <= 0. {
            0.
        } else {
            (self.display_pos.y / max).clamp(0., 1.)
        }
    }

    /// 纵向滚动比例。
    pub fn perc_y(&self) -> f32 {
        let max = self.max_scroll_pos().y;
        if max <= 0.0 {
            0.0
        } else {
            (self.pos.y / max).clamp(0.0, 1.0)
        }
    }

    /// 按比例设置横向滚动位置。
    pub fn set_perc_x(&mut self, perc: f32) -> bool {
        let x = self.max_scroll_pos().x * perc.clamp(0.0, 1.0);
        if self.page_mode || self.snap_to_item || self.loop_mode() != ScrollLoop::None {
            return self.apply_input_position(Point::new(x, self.pos.y), Some(true));
        }
        self.set_pos(x, self.pos.y, false)
    }

    /// 按比例设置纵向滚动位置。
    pub fn set_perc_y(&mut self, perc: f32) -> bool {
        let y = self.max_scroll_pos().y * perc.clamp(0.0, 1.0);
        if self.page_mode || self.snap_to_item || self.loop_mode() != ScrollLoop::None {
            return self.apply_input_position(Point::new(self.pos.x, y), Some(false));
        }
        self.set_pos(self.pos.x, y, false)
    }

    /// 按步长滚动。页模式下滚动的是一屏，否则是固定步长。
    ///
    /// `direction` 为 1 表示朝终点，-1 表示朝起点。
    pub fn scroll_by(&mut self, horizontal: bool, direction: f32) -> bool {
        let step = if self.page_mode {
            if horizontal {
                self.view_size.width
            } else {
                self.view_size.height
            }
        } else {
            self.scroll_step
        };

        if !step.is_finite() || !direction.is_finite() {
            return false;
        }
        let old = if horizontal { self.pos.x } else { self.pos.y };
        let value = (f64::from(old) + f64::from(step) * f64::from(direction))
            .clamp(-f64::from(f32::MAX), f64::from(f32::MAX)) as f32;
        let next = if horizontal {
            Point::new(value, self.pos.y)
        } else {
            Point::new(self.pos.x, value)
        };
        self.apply_input_position(next, Some(horizontal))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane_with(view: Size, content: Size) -> ScrollPane {
        let mut pane = ScrollPane {
            // 默认只允许纵向滚动，测试里两个方向都要覆盖到。
            scroll_type: ScrollType::Both,
            ..Default::default()
        };
        pane.set_view_size(view);
        pane.set_content_size(content);
        pane
    }

    #[test]
    fn scroll_type_decoding() {
        assert_eq!(ScrollType::from_code(0), ScrollType::Horizontal);
        assert_eq!(ScrollType::from_code(1), ScrollType::Vertical);
        assert_eq!(ScrollType::from_code(2), ScrollType::Both);
        assert_eq!(
            ScrollBarDisplayType::from_code(3),
            ScrollBarDisplayType::Hidden
        );
    }

    #[test]
    fn max_scroll_pos_never_goes_negative() {
        let pane = pane_with(Size::new(100.0, 100.0), Size::new(40.0, 30.0));
        assert_eq!(pane.max_scroll_pos(), Point::ZERO);
        assert!(!pane.is_scrollable());
    }

    #[test]
    fn position_is_clamped_without_bounce() {
        let mut pane = pane_with(Size::new(100.0, 100.0), Size::new(300.0, 250.0));
        assert!(pane.set_pos(500.0, 500.0, false));
        assert_eq!(pane.pos(), Point::new(200.0, 150.0));

        pane.set_pos(-50.0, -50.0, false);
        assert_eq!(pane.pos(), Point::ZERO);
    }

    #[test]
    fn bounce_allows_temporary_overscroll() {
        let mut pane = pane_with(Size::new(100.0, 100.0), Size::new(300.0, 250.0));
        assert!(pane.set_pos(260.0, 0.0, true));
        assert_eq!(pane.pos().x, 260.0, "允许越界时不应被夹住");

        pane.clamp_pos();
        assert_eq!(pane.pos().x, 200.0, "收回后才落到边界上");
    }

    #[test]
    fn direction_restricts_movement_on_the_other_axis() {
        let mut pane = pane_with(Size::new(100.0, 100.0), Size::new(300.0, 250.0));
        pane.scroll_type = ScrollType::Vertical;
        pane.set_pos(120.0, 80.0, false);
        assert_eq!(
            pane.pos(),
            Point::new(0.0, 80.0),
            "纵向滚动不应改动横向位置"
        );
    }

    #[test]
    fn content_shrink_clamps_position() {
        let mut pane = pane_with(Size::new(100.0, 100.0), Size::new(300.0, 300.0));
        pane.set_pos(200.0, 200.0, false);
        pane.set_content_size(Size::new(120.0, 120.0));
        assert_eq!(pane.pos(), Point::new(20.0, 20.0));
    }

    #[test]
    fn inertia_decays_and_stops() {
        let mut pane = pane_with(Size::new(100.0, 100.0), Size::new(1000.0, 1000.0));
        pane.begin_drag();
        pane.drag(-100., 0., 1. / 60.);
        pane.end_drag();
        assert!(pane.advance(1.0 / 60.0));
        assert!(pane.pos().x > 100.);

        // 推进足够多的帧后应当停下。
        for _ in 0..600 {
            pane.advance(1.0 / 60.0);
        }
        assert!(!pane.is_animating());
    }

    #[test]
    fn bounce_pulls_back_into_range() {
        let mut pane = pane_with(Size::new(100.0, 100.0), Size::new(300.0, 100.0));
        pane.set_pos(-50.0, 0.0, true);
        assert!(pane.pos().x < 0.0);

        for _ in 0..120 {
            pane.advance(1.0 / 60.0);
        }
        assert!(pane.pos().x.abs() < 1.0, "越界后应当被拉回边界");
    }

    #[test]
    fn on_demand_bars_fade_after_scrolling_stops() {
        let mut pane = pane_with(Size::new(100.0, 100.0), Size::new(300.0, 300.0));
        pane.bar_display = ScrollBarDisplayType::Auto;

        // 滚动让滚动条显形。
        pane.set_perc_x(0.5);
        assert!(
            !pane.bars_visible(),
            "immediate positions do not wake hidden native auto bars"
        );
        pane.begin_drag();
        pane.end_drag();
        assert_eq!(pane.bar_alpha(), 1.0);
        assert!(pane.bars_visible());

        // 延迟期间保持不透明。
        pane.advance_bar_fade(0.4);
        assert_eq!(pane.bar_alpha(), 1.0, "延迟期间不应开始淡出");

        // 过了延迟才开始淡出。
        pane.advance_bar_fade(0.2);
        assert!(pane.bar_alpha() < 1.0, "延迟过后应当开始淡出");

        // 再推进一会儿就完全消失。
        for _ in 0..60 {
            pane.advance_bar_fade(1.0 / 60.0);
        }
        assert_eq!(
            pane.bar_alpha(),
            1.0,
            "native completion restores alpha before hiding"
        );
        assert!(!pane.bars_visible(), "淡出后滚动条应当不再可见");
    }

    #[test]
    fn animated_scrolling_again_brings_the_bars_back() {
        let mut pane = pane_with(Size::new(100.0, 100.0), Size::new(300.0, 300.0));
        pane.bar_display = ScrollBarDisplayType::Auto;

        pane.set_perc_x(0.5);
        // 推进到淡出一半的位置：延迟 0.5 秒走完，淡出到一半。
        pane.touch_bars();
        for _ in 0..45 {
            pane.advance_bar_fade(1.0 / 60.0);
        }
        assert!(pane.bar_alpha() < 1.0, "此时应当已经淡了一部分");

        // 再次滚动立刻拉回不透明。
        pane.set_pos_animated(200., 0., true);
        assert_eq!(pane.bar_alpha(), 1.0);
        assert!(pane.bars_visible());
    }

    #[test]
    fn always_visible_mode_never_fades() {
        let mut pane = pane_with(Size::new(100.0, 100.0), Size::new(300.0, 300.0));
        pane.bar_display = ScrollBarDisplayType::Visible;

        for _ in 0..300 {
            assert!(!pane.advance_bar_fade(1.0 / 60.0), "非按需模式不推进淡出");
        }
        assert_eq!(pane.bar_alpha(), 1.0);
        assert!(pane.bars_visible());
    }

    #[test]
    fn perc_roundtrips() {
        let mut pane = pane_with(Size::new(100.0, 100.0), Size::new(300.0, 300.0));

        assert!(pane.set_perc_y(0.5));
        assert!((pane.perc_y() - 0.5).abs() < 1e-6);
        assert_eq!(pane.pos().y, 100.0);

        // 越界的比例会被夹住。
        pane.set_perc_x(5.0);
        assert_eq!(pane.perc_x(), 1.0);
        assert_eq!(pane.pos().x, 200.0);
    }

    #[test]
    fn perc_is_zero_when_content_fits() {
        let pane = pane_with(Size::new(100.0, 100.0), Size::new(40.0, 30.0));
        assert_eq!(pane.perc_x(), 0.0);
        assert_eq!(pane.perc_y(), 0.0);
    }

    #[test]
    fn scroll_by_moves_one_step() {
        let mut pane = pane_with(Size::new(100.0, 100.0), Size::new(100.0, 1000.0));

        assert!(pane.scroll_by(false, 1.0));
        assert_eq!(pane.pos().y, SCROLL_STEP);

        assert!(pane.scroll_by(false, -1.0));
        assert_eq!(pane.pos().y, 0.0);

        // 已经在起点，再往回滚不会产生变化。
        assert!(!pane.scroll_by(false, -1.0));
    }

    #[test]
    fn page_mode_scrolls_a_whole_view() {
        let mut pane = pane_with(Size::new(100.0, 100.0), Size::new(100.0, 1000.0));
        pane.page_mode = true;

        pane.scroll_by(false, 1.0);
        assert_eq!(pane.pos().y, 100.0, "页模式下一次滚动一屏");
    }

    #[test]
    fn dragging_updates_position_and_slow_release_stays_put() {
        let mut pane = pane_with(Size::new(100.0, 100.0), Size::new(300.0, 300.0));
        pane.begin_drag();
        pane.drag(0.0, -30.0, 1.0 / 60.0);

        assert_eq!(pane.pos().y, 30.0, "向上拖动内容应下移视口");
        pane.end_drag();
        assert!(!pane.is_animating(), "低速松手不应产生额外惯性");
    }
}

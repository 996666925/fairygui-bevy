//! 通用补间。
//!
//! 与过渡动画里的补间不同，这里是可独立使用的补间器：给定起止值与时长，
//! 由调度器逐帧推进。官方的推进算法有几处需要照搬：
//!
//! - 循环时先把已过去的时间折回单次时长内，**往复模式再按轮次奇偶把时间镜像**
//!   （`duration - tt`），而不是直接反向插值；
//! - 缓动作用在镜像后的时间上；
//! - 结束标志是三态：`0` 未结束、`1` 正常结束、`2` 到达断点。

use crate::geom::Point;
use crate::object::{GObjectId, ObjectStore};
use crate::transition::ease::{EaseType, evaluate};
use crate::transition::path::GPath;

/// 补间值：四个浮点分量。
///
/// 四个分量可以按需解释为坐标、尺寸、颜色通道等。颜色按 ARGB 打包。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TweenValue {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl TweenValue {
    /// 当前位置。
    pub fn position(&self) -> Point {
        Point::new(self.x, self.y)
    }

    /// 按 `0xAARRGGBB` 组合。
    pub fn color(&self) -> u32 {
        ((self.w as u32) << 24) | ((self.x as u32) << 16) | ((self.y as u32) << 8) | (self.z as u32)
    }

    /// 由 `0xAARRGGBB` 拆出四个分量。
    pub fn set_color(&mut self, value: u32) {
        self.x = ((value & 0x00FF_0000) >> 16) as f32;
        self.y = ((value & 0x0000_FF00) >> 8) as f32;
        self.z = (value & 0x0000_00FF) as f32;
        self.w = ((value & 0xFF00_0000) >> 24) as f32;
    }

    /// 按序号取分量。越界返回 0。
    pub fn field(&self, index: usize) -> f32 {
        match index {
            0 => self.x,
            1 => self.y,
            2 => self.z,
            3 => self.w,
            _ => 0.0,
        }
    }

    /// 按序号写分量。越界忽略。
    pub fn set_field(&mut self, index: usize, value: f32) {
        match index {
            0 => self.x = value,
            1 => self.y = value,
            2 => self.z = value,
            3 => self.w = value,
            _ => {}
        }
    }
}

/// 补间作用的属性。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TweenProperty {
    /// 位置。
    Xy,
    /// 尺寸。
    Size,
    /// 缩放。
    Scale,
    /// 不透明度。
    Alpha,
    /// 旋转角度。
    Rotation,
}

impl TweenProperty {
    /// 该属性需要几个分量。
    pub const fn value_size(self) -> u8 {
        match self {
            Self::Xy | Self::Size | Self::Scale => 2,
            Self::Alpha | Self::Rotation => 1,
        }
    }
}

/// 一个补间。
#[derive(Debug, Clone)]
pub struct Tween {
    /// 作用的属性与对象。
    target: Option<(GObjectId, TweenProperty)>,
    /// 起始值。
    start_value: TweenValue,
    /// 目标值。
    end_value: TweenValue,
    /// 当前值。
    value: TweenValue,
    /// 延迟（秒）。
    delay: f32,
    /// 单次时长（秒）。
    duration: f32,
    /// 缓动类型。
    ease_type: EaseType,
    /// 重复次数，`0` 表示不重复。
    repeat: i32,
    /// 是否往复。
    yoyo: bool,
    /// 结果是否取整。
    snapping: bool,
    /// 时间缩放。
    time_scale: f32,
    /// 分量数。
    value_size: u8,
    /// 运动路径。设置了它就不再走线性插值。
    path: Option<GPath>,

    elapsed: f32,
    started: bool,
    ended: u8,
    killed: bool,
    paused: bool,
}

impl Default for Tween {
    fn default() -> Self {
        Self {
            target: None,
            start_value: TweenValue::default(),
            end_value: TweenValue::default(),
            value: TweenValue::default(),
            delay: 0.0,
            duration: 0.0,
            // 官方默认缓动是 QuadOut。
            ease_type: 5,
            repeat: 0,
            yoyo: false,
            snapping: false,
            time_scale: 1.0,
            value_size: 0,
            path: None,
            elapsed: 0.0,
            started: false,
            ended: 0,
            killed: false,
            paused: false,
        }
    }
}

impl Tween {
    /// 新建一个作用在对象属性上的补间。
    pub fn new(owner: GObjectId, property: TweenProperty) -> Self {
        Self {
            target: Some((owner, property)),
            value_size: property.value_size(),
            ..Default::default()
        }
    }

    /// 设置起止值。
    pub fn from_to(mut self, start: TweenValue, end: TweenValue) -> Self {
        self.start_value = start;
        self.end_value = end;
        self
    }

    /// 设置时长。
    pub fn with_duration(mut self, duration: f32) -> Self {
        self.duration = duration.max(0.0);
        self
    }

    /// 设置延迟。
    pub fn with_delay(mut self, delay: f32) -> Self {
        self.delay = delay.max(0.0);
        self
    }

    /// 设置缓动类型。
    pub fn with_ease(mut self, ease_type: EaseType) -> Self {
        self.ease_type = ease_type;
        self
    }

    /// 设置重复与往复。
    pub fn with_repeat(mut self, repeat: i32, yoyo: bool) -> Self {
        self.repeat = repeat;
        self.yoyo = yoyo;
        self
    }

    /// 结果是否取整。
    pub fn with_snapping(mut self, snapping: bool) -> Self {
        self.snapping = snapping;
        self
    }

    /// 设置时间缩放。
    pub fn with_time_scale(mut self, scale: f32) -> Self {
        self.time_scale = scale;
        self
    }

    /// 沿路径运动。设置后不再按数值插值。
    pub fn along_path(mut self, path: GPath) -> Self {
        self.value_size = 2;
        self.path = Some(path);
        self
    }

    /// 当前值。
    pub fn value(&self) -> TweenValue {
        self.value
    }

    /// 是否已结束。
    pub fn is_completed(&self) -> bool {
        self.ended != 0
    }

    /// 是否已被回收。
    pub fn is_killed(&self) -> bool {
        self.killed
    }

    /// 暂停或继续。
    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    /// 立即结束（可选是否把值落到终点）。
    pub fn kill(&mut self, complete: bool, store: &mut ObjectStore) {
        if !self.killed && complete {
            self.elapsed = self.delay + self.duration;
            self.update(store);
        }
        self.killed = true;
    }

    /// 直接跳到指定时刻。
    pub fn seek(&mut self, time: f32, store: &mut ObjectStore) {
        if time < self.delay && !self.started {
            return;
        }
        self.elapsed = time;
        self.update(store);
    }

    /// 推进一帧。返回是否仍在进行。
    pub fn advance(&mut self, store: &mut ObjectStore, dt: f32) -> bool {
        if self.killed {
            return false;
        }
        if self.paused {
            return true;
        }

        let dt = dt * self.time_scale;
        if dt == 0.0 {
            return true;
        }

        self.elapsed += dt;
        self.update(store);

        if self.ended != 0 {
            self.killed = true;
        }
        !self.killed
    }

    fn update(&mut self, store: &mut ObjectStore) {
        self.ended = 0;

        // 纯延时（没有分量）只需要判断时间到没到。
        if self.value_size == 0 {
            if self.elapsed >= self.delay + self.duration {
                self.ended = 1;
            }
            return;
        }

        if !self.started {
            if self.elapsed < self.delay {
                return;
            }
            self.started = true;
        }

        let mut time = self.elapsed - self.delay;
        let mut reversed = false;

        if self.repeat != 0 && self.duration > 0.0 {
            let round = (time / self.duration).floor();
            time -= self.duration * round;

            if self.yoyo {
                reversed = (round as i32) % 2 == 1;
            }
            if self.repeat > 0 && self.repeat as f32 - round < 0.0 {
                if self.yoyo {
                    reversed = self.repeat % 2 == 1;
                }
                time = self.duration;
                self.ended = 1;
            }
        } else if time >= self.duration {
            time = self.duration;
            self.ended = 1;
        }

        // 往复时把时间镜像，使缓动曲线反向走一遍。
        let progress = evaluate(
            self.ease_type,
            if reversed { self.duration - time } else { time },
            self.duration,
        );

        let previous = self.value;
        self.value = TweenValue::default();

        if let Some(path) = &self.path {
            let point = path.point_at(progress);
            let (x, y) = if self.snapping {
                (point.x.round(), point.y.round())
            } else {
                (point.x, point.y)
            };
            self.value.x = x;
            self.value.y = y;
            let _ = previous;
            self.apply(store);
            return;
        }

        let count = self.value_size.min(4) as usize;
        for index in 0..count {
            let start = self.start_value.field(index);
            let end = self.end_value.field(index);
            let mut field = start + (end - start) * progress;
            if self.snapping {
                field = field.round();
            }
            self.value.set_field(index, field);
        }

        self.apply(store);
    }

    fn apply(&self, store: &mut ObjectStore) {
        let Some((owner, property)) = self.target else {
            return;
        };
        let value = self.value;

        match property {
            TweenProperty::Xy => store.set_xy(owner, value.x, value.y),
            TweenProperty::Size => store.set_size(owner, value.x, value.y),
            TweenProperty::Scale => store.set_scale(owner, value.x, value.y),
            TweenProperty::Alpha => store.set_alpha(owner, value.x),
            TweenProperty::Rotation => store.set_rotation(owner, value.x),
        }
    }
}

/// 补间调度器。
///
/// 与官方的做法一致：已结束的补间在推进时被就地回收。
#[derive(Debug, Default)]
pub struct TweenManager {
    tweens: Vec<Tween>,
}

impl TweenManager {
    /// 空调度器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 活跃补间数量。
    pub fn len(&self) -> usize {
        self.tweens.len()
    }

    /// 是否没有活跃补间。
    pub fn is_empty(&self) -> bool {
        self.tweens.is_empty()
    }

    /// 加入一个补间。
    pub fn add(&mut self, tween: Tween) {
        self.tweens.push(tween);
    }

    /// 推进全部补间，并回收已结束的。
    pub fn advance(&mut self, store: &mut ObjectStore, dt: f32) {
        if self.tweens.is_empty() {
            return;
        }

        let mut alive: Vec<Tween> = Vec::with_capacity(self.tweens.len());
        for mut tween in std::mem::take(&mut self.tweens) {
            if tween.advance(store, dt) {
                alive.push(tween);
            }
        }
        self.tweens = alive;
    }

    /// 终止全部补间。`complete` 为真时先把值落到终点。
    pub fn kill_all(&mut self, store: &mut ObjectStore, complete: bool) {
        for tween in &mut self.tweens {
            tween.kill(complete, store);
        }
        self.tweens.clear();
    }

    /// 终止作用在某个对象上的补间。
    pub fn kill_for(&mut self, store: &mut ObjectStore, owner: GObjectId) {
        let mut remaining = Vec::with_capacity(self.tweens.len());
        for mut tween in std::mem::take(&mut self.tweens) {
            if tween.target.is_some_and(|(id, _)| id == owner) {
                tween.kill(false, store);
            } else {
                remaining.push(tween);
            }
        }
        self.tweens = remaining;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::ObjectType;

    fn store_with_object() -> (ObjectStore, GObjectId) {
        let mut store = ObjectStore::new();
        let id = store.create(ObjectType::Component);
        (store, id)
    }

    #[test]
    fn color_packs_and_unpacks() {
        let mut value = TweenValue::default();
        value.set_color(0x80112233);

        assert_eq!(value.x, 0x11 as f32);
        assert_eq!(value.y, 0x22 as f32);
        assert_eq!(value.z, 0x33 as f32);
        assert_eq!(value.w, 0x80 as f32);
        assert_eq!(value.color(), 0x80112233);
    }

    #[test]
    fn fields_are_indexed_in_order() {
        let mut value = TweenValue::default();
        for index in 0..4 {
            value.set_field(index, index as f32 + 1.0);
        }
        assert_eq!(value.x, 1.0);
        assert_eq!(value.y, 2.0);
        assert_eq!(value.z, 3.0);
        assert_eq!(value.w, 4.0);

        // 越界读写不应panic。
        assert_eq!(value.field(9), 0.0);
        value.set_field(9, 1.0);
    }

    #[test]
    fn alpha_tween_reaches_its_target() {
        let (mut store, id) = store_with_object();
        let mut tween = Tween::new(id, TweenProperty::Alpha)
            .from_to(
                TweenValue {
                    x: 0.0,
                    ..Default::default()
                },
                TweenValue {
                    x: 1.0,
                    ..Default::default()
                },
            )
            .with_duration(1.0)
            .with_ease(0);

        // 推进一半应当到中间值。
        assert!(tween.advance(&mut store, 0.5));
        assert!((store.object(id).unwrap().alpha() - 0.5).abs() < 0.01);

        // 再推进一半应当结束并落到终点。
        assert!(!tween.advance(&mut store, 0.6));
        assert!((store.object(id).unwrap().alpha() - 1.0).abs() < 0.01);
        assert!(tween.is_completed());
    }

    #[test]
    fn delay_defers_the_start() {
        let (mut store, id) = store_with_object();
        store.set_alpha(id, 0.0);

        let mut tween = Tween::new(id, TweenProperty::Alpha)
            .from_to(
                TweenValue::default(),
                TweenValue {
                    x: 1.0,
                    ..Default::default()
                },
            )
            .with_duration(1.0)
            .with_delay(1.0);

        assert!(tween.advance(&mut store, 0.5));
        assert_eq!(
            store.object(id).unwrap().alpha(),
            0.0,
            "延迟期内不应改动属性"
        );
    }

    #[test]
    fn yoyo_returns_to_the_start_on_odd_rounds() {
        let (mut store, id) = store_with_object();
        let mut tween = Tween::new(id, TweenProperty::Alpha)
            .from_to(
                TweenValue::default(),
                TweenValue {
                    x: 1.0,
                    ..Default::default()
                },
            )
            .with_duration(1.0)
            .with_ease(0)
            .with_repeat(2, true);

        // 第一轮走完，值在终点。
        tween.advance(&mut store, 1.0);
        assert!((store.object(id).unwrap().alpha() - 1.0).abs() < 0.01);

        // 第二轮是往复，走到一半时应当回到中间。
        tween.advance(&mut store, 0.5);
        assert!((store.object(id).unwrap().alpha() - 0.5).abs() < 0.01);
    }

    #[test]
    fn zero_repeat_means_single_run() {
        let (mut store, id) = store_with_object();
        let mut tween = Tween::new(id, TweenProperty::Alpha)
            .from_to(
                TweenValue::default(),
                TweenValue {
                    x: 1.0,
                    ..Default::default()
                },
            )
            .with_duration(0.5);

        assert!(tween.advance(&mut store, 0.3));
        assert!(!tween.advance(&mut store, 0.3));
    }

    #[test]
    fn infinite_repeat_keeps_running() {
        let (mut store, id) = store_with_object();
        let mut tween = Tween::new(id, TweenProperty::Alpha)
            .from_to(
                TweenValue::default(),
                TweenValue {
                    x: 1.0,
                    ..Default::default()
                },
            )
            .with_duration(0.5)
            .with_repeat(-1, false);

        for _ in 0..20 {
            assert!(tween.advance(&mut store, 0.3));
        }
    }

    #[test]
    fn snapping_rounds_the_result() {
        let (mut store, id) = store_with_object();
        let mut tween = Tween::new(id, TweenProperty::Xy)
            .from_to(
                TweenValue::default(),
                TweenValue {
                    x: 10.0,
                    y: 10.0,
                    ..Default::default()
                },
            )
            .with_duration(1.0)
            .with_ease(0)
            .with_snapping(true);

        tween.advance(&mut store, 0.33);
        let position = store.object(id).unwrap().position();
        assert_eq!(position.x.fract(), 0.0, "取整后不应有小数");
        assert_eq!(position.y.fract(), 0.0);
    }

    #[test]
    fn path_tween_follows_the_curve() {
        let (mut store, id) = store_with_object();
        let path = GPath::new(&[
            crate::transition::PathPoint::straight(0.0, 0.0),
            crate::transition::PathPoint::straight(100.0, 0.0),
        ]);

        let mut tween = Tween::new(id, TweenProperty::Xy)
            .along_path(path)
            .with_duration(1.0)
            .with_ease(0);

        tween.advance(&mut store, 0.5);
        assert!((store.object(id).unwrap().position().x - 50.0).abs() < 0.5);
    }

    #[test]
    fn manager_reclaims_finished_tweens() {
        let (mut store, id) = store_with_object();
        let mut manager = TweenManager::new();

        manager.add(
            Tween::new(id, TweenProperty::Alpha)
                .from_to(
                    TweenValue::default(),
                    TweenValue {
                        x: 1.0,
                        ..Default::default()
                    },
                )
                .with_duration(0.5),
        );
        assert_eq!(manager.len(), 1);

        manager.advance(&mut store, 1.0);
        assert!(manager.is_empty(), "结束的补间应当被回收");
    }

    #[test]
    fn kill_all_stops_everything() {
        let (mut store, id) = store_with_object();
        let mut manager = TweenManager::new();
        manager.add(
            Tween::new(id, TweenProperty::Alpha)
                .from_to(
                    TweenValue::default(),
                    TweenValue {
                        x: 1.0,
                        ..Default::default()
                    },
                )
                .with_duration(5.0),
        );

        manager.kill_all(&mut store, false);
        assert!(manager.is_empty());
        manager.advance(&mut store, 10.0);
        assert_eq!(
            store.object(id).unwrap().alpha(),
            1.0,
            "未完成就终止的补间不应改动属性"
        );
    }

    #[test]
    fn kill_for_only_touches_the_given_object() {
        let (mut store, first) = store_with_object();
        let second = store.create(ObjectType::Component);

        let mut manager = TweenManager::new();
        manager.add(
            Tween::new(first, TweenProperty::Alpha)
                .from_to(
                    TweenValue::default(),
                    TweenValue {
                        x: 1.0,
                        ..Default::default()
                    },
                )
                .with_duration(1.0),
        );
        manager.add(
            Tween::new(second, TweenProperty::Alpha)
                .from_to(
                    TweenValue::default(),
                    TweenValue {
                        x: 1.0,
                        ..Default::default()
                    },
                )
                .with_duration(1.0),
        );

        manager.kill_for(&mut store, first);
        assert_eq!(manager.len(), 1, "只应移除目标对象上的补间");
    }

    #[test]
    fn paused_tween_does_not_advance() {
        let (mut store, id) = store_with_object();
        let mut tween = Tween::new(id, TweenProperty::Alpha)
            .from_to(
                TweenValue::default(),
                TweenValue {
                    x: 1.0,
                    ..Default::default()
                },
            )
            .with_duration(1.0);

        tween.set_paused(true);
        assert!(tween.advance(&mut store, 0.5));
        assert_eq!(
            store.object(id).unwrap().alpha(),
            1.0,
            "暂停期间不应改动属性"
        );
    }
}

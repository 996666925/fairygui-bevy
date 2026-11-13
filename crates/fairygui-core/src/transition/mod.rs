//! 过渡动画。
//!
//! 过渡由若干项组成：每项在指定时刻开始作用，带补间的项会在时长内插值，
//! 不带补间的项直接跳到目标取值。推进由上层逐帧驱动，核心层不依赖引擎时钟。
//!
//! 描述里每一项自成一个小型「索引表 + 数据块」结构：块 0 是类型与时刻，
//! 块 1 是补间配置，块 2 与块 3 分别是起止取值。

mod ease;
mod edit;
mod item;
mod path;
mod playback;
#[cfg(test)]
mod playback_tests;
mod tween;

pub use ease::{EaseType, evaluate};
pub use edit::{
    TransitionCallback, TransitionEditError, TransitionEditValue, TransitionListenerId,
};
pub use item::{ActionType, TransitionItem, TransitionValue, TweenConfig};
pub use path::{CurveType, GPath, PathPoint, bounding_box};
pub use playback::{TransitionComplete, TransitionHook};
pub use tween::{Tween, TweenManager, TweenProperty, TweenValue};

use crate::binary::{ByteBuffer, StringTable};
use crate::error::Result;
use crate::object::GObjectId;

/// 一次过渡播放的时间区间（秒）。结束时间为空表示不截断。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionPlayOptions {
    pub times: i32,
    pub delay: f32,
    pub start_time: f32,
    pub end_time: Option<f32>,
    pub reversed: bool,
}

impl Default for TransitionPlayOptions {
    fn default() -> Self {
        Self {
            times: 1,
            delay: 0.0,
            start_time: 0.0,
            end_time: None,
            reversed: false,
        }
    }
}

impl TransitionPlayOptions {
    pub fn is_valid(&self) -> bool {
        self.delay.is_finite()
            && self.delay >= 0.0
            && self.start_time.is_finite()
            && self.start_time >= 0.0
            && self
                .end_time
                .is_none_or(|end| end.is_finite() && end >= 0.0)
    }
}

/// 过渡动画。
#[derive(Debug, Clone, Default)]
pub struct Transition {
    /// 名称。
    pub name: String,
    /// 选项位。
    pub options: i32,
    /// 是否自动播放。
    pub auto_play: bool,
    /// 自动播放次数，负数表示无限；0 与 1 均播放一轮。
    pub auto_play_times: i32,
    /// 自动播放前的延迟（秒）。
    pub auto_play_delay: f32,
    /// 过渡项。
    pub items: Vec<TransitionItem>,
    /// 总时长（秒）。
    pub total_duration: f32,

    playing: bool,
    time: f32,
    play_times: i32,
    remaining_times: i32,
    delay: f32,
    generation: u64,
    end_time: Option<f32>,
    on_complete: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    zero_yield: bool,
    paused: bool,
    time_scale: Option<f32>,
    reversed: bool,
    start_time: f32,
    bound_targets: Option<Vec<Option<GObjectId>>>,
    callbacks: std::collections::HashMap<(usize, bool), TransitionListenerId>,
    cached_sounds: std::collections::HashMap<usize, String>,
    runtime: playback::PlaybackState,
}

impl Transition {
    /// 读取过渡描述。
    ///
    /// `buffer` 的位置应当是该项在块 5 中的起点。
    pub fn setup(&mut self, buffer: &mut ByteBuffer<'_>, table: &StringTable) -> Result<()> {
        self.name = buffer.read_s(table)?.unwrap_or_default();
        self.options = buffer.read_i32()?;
        self.auto_play = buffer.read_bool()?;
        self.auto_play_times = buffer.read_i32()?;
        self.auto_play_delay = buffer.read_f32()?;

        let count = buffer.read_i16()?.max(0) as usize;

        for _ in 0..count {
            let data_len = buffer.read_i16()?;
            let item_start = buffer.position();

            let mut transition_item = TransitionItem {
                action: ActionType::Unknown,
                time: 0.0,
                target_index: -1,
                label: None,
                value: TransitionValue::None,
                tween: None,
            };

            // ---- 块 0：类型、时刻与目标 ----
            if buffer.seek(item_start, 0)? {
                transition_item.action = ActionType::from_code(buffer.read_u8()?);
                transition_item.time = buffer.read_f32()?;
                // 负数表示作用在容器自身。
                transition_item.target_index = buffer.read_i16()?;
                // 标签用于按名字跳转，不能丢。
                transition_item.label = buffer.read_s(table)?;

                if buffer.read_bool()? {
                    // ---- 块 1：补间配置 ----
                    buffer.seek(item_start, 1)?;
                    let duration = buffer.read_f32()?;
                    let ease_type = i32::from(buffer.read_u8()?);
                    let repeat = buffer.read_i32()?;
                    let yoyo = buffer.read_bool()?;
                    let end_label = buffer.read_s(table)?;

                    // ---- 块 2 / 3：起止取值 ----
                    buffer.seek(item_start, 2)?;
                    let start_value = item::decode_value(transition_item.action, buffer, table)?;

                    buffer.seek(item_start, 3)?;
                    let end_value = item::decode_value(transition_item.action, buffer, table)?;

                    // 路径点在格式 2 及以上才有。曲线类型决定后面跟几个浮点。
                    let mut path = None;
                    if buffer.version() >= 2 {
                        let path_len = buffer.read_i32()?.max(0) as usize;
                        let mut points = Vec::with_capacity(path_len);
                        for _ in 0..path_len {
                            let curve_type = CurveType::from_code(buffer.read_u8()?);
                            let x = buffer.read_f32()?;
                            let y = buffer.read_f32()?;
                            let point = match curve_type {
                                CurveType::Bezier => PathPoint::bezier(
                                    x,
                                    y,
                                    crate::geom::Point::new(buffer.read_f32()?, buffer.read_f32()?),
                                ),
                                CurveType::CubicBezier => PathPoint::cubic_bezier(
                                    x,
                                    y,
                                    crate::geom::Point::new(buffer.read_f32()?, buffer.read_f32()?),
                                    crate::geom::Point::new(buffer.read_f32()?, buffer.read_f32()?),
                                ),
                                _ => PathPoint {
                                    x,
                                    y,
                                    curve_type,
                                    ..Default::default()
                                },
                            };
                            points.push(point);
                        }
                        if !points.is_empty() {
                            path = Some(GPath::new(&points));
                        }
                    }

                    transition_item.tween = Some(TweenConfig {
                        duration,
                        ease_type,
                        repeat,
                        yoyo,
                        end_label,
                        path,
                        start_value,
                        end_value,
                    });
                } else {
                    // 无补间：取值直接放在块 2。
                    buffer.seek(item_start, 2)?;
                    transition_item.value =
                        item::decode_value(transition_item.action, buffer, table)?;
                }
            }

            self.total_duration = self.total_duration.max(transition_item.end_time());
            self.items.push(transition_item);

            if data_len > 0 {
                let next = item_start.saturating_add(data_len.max(0) as usize);
                buffer.set_position(next);
            }
        }

        Ok(())
    }

    /// 是否正在播放。
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// 当前播放进度（秒）。
    pub fn time(&self) -> f32 {
        self.time
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::ObjectStore;
    use crate::package::ObjectType;

    fn fade_in_transition(duration: f32) -> Transition {
        Transition {
            name: "fade".to_string(),
            items: vec![TransitionItem {
                action: ActionType::Alpha,
                time: 0.0,
                target_index: -1,
                label: None,
                value: TransitionValue::None,
                tween: Some(TweenConfig {
                    duration,
                    ease_type: 0,
                    repeat: 0,
                    yoyo: false,
                    end_label: None,
                    path: None,
                    start_value: TransitionValue::Scalar(0.0),
                    end_value: TransitionValue::Scalar(1.0),
                }),
            }],
            total_duration: duration,
            ..Default::default()
        }
    }

    #[test]
    fn advancing_interpolates_alpha() {
        let mut store = ObjectStore::new();
        let owner = store.create(ObjectType::Component);
        store.set_alpha(owner, 0.0);
        store.take_dirty();

        let mut transition = fade_in_transition(1.0);
        transition.play(1, 0.0);

        // 推进一半时长，透明度应当到中间。
        transition.advance(&mut store, owner, 0.5);
        let alpha = store.object(owner).unwrap().alpha();
        assert!((alpha - 0.5).abs() < 1e-3, "实际为 {alpha}");

        // 走完剩余时长。
        transition.advance(&mut store, owner, 0.6);
        assert!((store.object(owner).unwrap().alpha() - 1.0).abs() < 1e-3);
    }

    #[test]
    fn single_shot_stops_after_the_duration() {
        let mut store = ObjectStore::new();
        let owner = store.create(ObjectType::Component);

        let mut transition = fade_in_transition(0.5);
        transition.play(1, 0.0);

        assert!(transition.advance(&mut store, owner, 0.3));
        assert!(!transition.advance(&mut store, owner, 0.3));
        assert!(!transition.is_playing());
    }

    #[test]
    fn natural_completion_is_once_per_playback_after_repeats_and_excludes_stops() {
        let mut store = ObjectStore::new();
        let owner = store.create(ObjectType::Component);
        let mut transition = fade_in_transition(0.5);
        transition.play(2, 0.0);
        let first = transition.play_generation();
        transition.advance(&mut store, owner, 0.75);
        assert!(store.take_transition_completions().is_empty());
        transition.advance(&mut store, owner, 0.25);
        let complete = store.take_transition_completions();
        assert_eq!(complete.len(), 1);
        assert_eq!(complete[0].owner, owner);
        assert_eq!(complete[0].play_generation, first);
        transition.advance(&mut store, owner, 1.0);
        assert!(store.take_transition_completions().is_empty());

        transition.play(1, 0.0);
        transition.advance(&mut store, owner, 0.1);
        transition.stop_with_callback(&mut store, owner, true, true);
        assert!(store.take_transition_completions().is_empty());
        transition.play(-1, 0.0);
        transition.advance(&mut store, owner, 10.0);
        assert!(store.take_transition_completions().is_empty());
        transition.play(1, 0.0);
        transition.advance(&mut store, owner, 0.5);
        let complete = store.take_transition_completions();
        assert_eq!(complete.len(), 1);
        assert!(complete[0].play_generation > first);

        let mut instant = Transition::default();
        instant.play(1, 0.0);
        instant.advance(&mut store, owner, 0.0);
        assert_eq!(store.take_transition_completions().len(), 1);
        instant.play(1, 0.0);
        instant.advance(&mut store, owner, 0.0);
        store.remove(owner);
        assert!(store.take_transition_completions().is_empty());
    }

    #[test]
    fn infinite_mode_keeps_looping() {
        let mut store = ObjectStore::new();
        let owner = store.create(ObjectType::Component);

        let mut transition = fade_in_transition(0.5);
        transition.play(-1, 0.0);

        for _ in 0..10 {
            assert!(transition.advance(&mut store, owner, 0.3));
        }
        assert!(transition.is_playing());
    }

    #[test]
    fn delay_defers_the_first_application() {
        let mut store = ObjectStore::new();
        let owner = store.create(ObjectType::Component);
        store.set_alpha(owner, 0.0);

        let mut transition = fade_in_transition(1.0);
        transition.play(1, 1.0);

        // 延迟期内不应改动属性。
        assert!(transition.advance(&mut store, owner, 0.5));
        assert_eq!(store.object(owner).unwrap().alpha(), 0.0);
        assert_eq!(transition.time(), 0.0);
    }

    #[test]
    fn targets_are_resolved_by_child_index() {
        let mut store = ObjectStore::new();
        let owner = store.create(ObjectType::Component);
        let child = store.create(ObjectType::Image);
        store.set_parent(child, Some(owner));
        store.set_alpha(child, 0.0);

        let mut transition = Transition {
            items: vec![TransitionItem {
                action: ActionType::Alpha,
                time: 0.0,
                target_index: 0,
                label: None,
                value: TransitionValue::None,
                tween: Some(TweenConfig {
                    duration: 1.0,
                    ease_type: 0,
                    repeat: 0,
                    yoyo: false,
                    end_label: None,
                    path: None,
                    start_value: TransitionValue::Scalar(0.0),
                    end_value: TransitionValue::Scalar(1.0),
                }),
            }],
            total_duration: 1.0,
            ..Default::default()
        };
        transition.play(1, 0.0);
        transition.advance(&mut store, owner, 2.0);

        assert!(
            (store.object(child).unwrap().alpha() - 1.0).abs() < 1e-3,
            "应当作用在按编号找到的子对象上"
        );
        assert_eq!(
            store.object(owner).unwrap().alpha(),
            1.0,
            "容器自身不应被改动"
        );
    }

    #[test]
    fn stopping_halts_further_advance() {
        let mut store = ObjectStore::new();
        let owner = store.create(ObjectType::Component);

        let mut transition = fade_in_transition(1.0);
        transition.play(1, 0.0);
        transition.stop();

        assert!(!transition.advance(&mut store, owner, 0.5));
    }
}

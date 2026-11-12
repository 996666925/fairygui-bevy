//! 过渡项：一次属性变化的描述。
//!
//! 每一项由「属性类型 + 起始时刻 + 目标对象」构成；带补间时另有起止取值与
//! 缓动配置，不带补间时只有一个目标取值。取值按属性类型分派解码，
//! 坐标额外带一位「是否按比例」，旋转与 GObject.rotation 一样使用角度。

use crate::binary::{ByteBuffer, StringTable};
use crate::color::Color;
use crate::error::Result;
use crate::object::{GObjectId, ObjectStore};
use crate::transition::ease::{EaseType, evaluate};

/// 属性类型编号，与官方 `ActionType` 一致。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionType {
    /// 位置。
    Xy,
    /// 尺寸。
    Size,
    /// 缩放。
    Scale,
    /// 轴心。
    Pivot,
    /// 不透明度。
    Alpha,
    /// 旋转（角度）。
    Rotation,
    /// 颜色。
    Color,
    /// 播放状态与帧。
    Animation,
    /// 可见性。
    Visible,
    /// 声音。
    Sound,
    /// 播放另一个过渡。
    Transition,
    /// 抖动。
    Shake,
    /// 颜色滤镜。
    ColorFilter,
    /// 斜切。
    Skew,
    /// 文本。
    Text,
    /// 图标。
    Icon,
    /// 未知类型，不参与变化。
    Unknown,
}

impl ActionType {
    /// 由描述中的字节编号构造。
    pub const fn from_code(code: u8) -> Self {
        match code {
            0 => Self::Xy,
            1 => Self::Size,
            2 => Self::Scale,
            3 => Self::Pivot,
            4 => Self::Alpha,
            5 => Self::Rotation,
            6 => Self::Color,
            7 => Self::Animation,
            8 => Self::Visible,
            9 => Self::Sound,
            10 => Self::Transition,
            11 => Self::Shake,
            12 => Self::ColorFilter,
            13 => Self::Skew,
            14 => Self::Text,
            15 => Self::Icon,
            _ => Self::Unknown,
        }
    }
}

/// 一个属性取值。
#[derive(Debug, Clone, PartialEq, Default)]
pub enum TransitionValue {
    /// 一对数值。两个「是否参与」标志指出该分量要不要改动。
    Pair {
        use_x: bool,
        use_y: bool,
        x: f32,
        y: f32,
    },
    /// 相对所属容器宽高的坐标。
    PercentPosition {
        use_x: bool,
        use_y: bool,
        x: f32,
        y: f32,
    },
    /// 单个数值。
    Scalar(f32),
    /// 两个数值。
    Vector2 { x: f32, y: f32 },
    /// 颜色。
    Color(Color),
    /// 播放状态与帧。
    Animation { playing: bool, frame: i32 },
    /// 可见性。
    Visible(bool),
    /// 声音。
    Sound { sound: Option<String>, volume: f32 },
    /// 播放另一个过渡。
    Transition { name: Option<String>, times: i32 },
    /// 抖动。
    Shake { amplitude: f32, duration: f32 },
    /// 颜色滤镜的四个分量。
    ColorFilter([f32; 4]),
    /// 文本或图标地址。
    Text(Option<String>),
    /// 不参与变化。
    #[default]
    None,
}

impl TransitionValue {
    /// 把取值套用到目标对象上。
    ///
    /// 取值本身不区分「位置」与「尺寸」这类同构的属性，因此需要属性类型参与判断。
    pub(crate) fn apply(&self, store: &mut ObjectStore, target: GObjectId, action: ActionType) {
        store.with_gear_lock(target, |store| self.apply_locked(store, target, action));
    }

    fn apply_locked(&self, store: &mut ObjectStore, target: GObjectId, action: ActionType) {
        match self {
            Self::PercentPosition { use_x, use_y, x, y } => {
                let size = store
                    .object(target)
                    .and_then(|o| o.parent())
                    .and_then(|id| store.object(id))
                    .map(|o| o.size())
                    .unwrap_or_default();
                Self::Pair {
                    use_x: *use_x,
                    use_y: *use_y,
                    x: x * size.width,
                    y: y * size.height,
                }
                .apply(store, target, action);
            }
            Self::Pair { use_x, use_y, x, y } => {
                let Some(object) = store.object(target) else {
                    return;
                };
                let (current_x, current_y) = (object.x(), object.y());
                let (current_w, current_h) = (object.width(), object.height());
                let current_pivot = object.pivot();
                let (current_skew_x, current_skew_y) = (object.skew_x(), object.skew_y());

                let new_x = if *use_x { *x } else { current_x };
                let new_y = if *use_y { *y } else { current_y };

                match action {
                    ActionType::Xy => store.set_xy(target, new_x, new_y),
                    ActionType::Size => store.set_size(
                        target,
                        if *use_x { *x } else { current_w },
                        if *use_y { *y } else { current_h },
                    ),
                    ActionType::Pivot => store.set_pivot(
                        target,
                        if *use_x { *x } else { current_pivot.x },
                        if *use_y { *y } else { current_pivot.y },
                        object.pivot_as_anchor(),
                    ),
                    ActionType::Skew => store.set_skew(
                        target,
                        if *use_x { *x } else { current_skew_x },
                        if *use_y { *y } else { current_skew_y },
                    ),
                    _ => {}
                }
            }
            Self::Scalar(value) => match action {
                ActionType::Alpha => store.set_alpha(target, *value),
                ActionType::Rotation => store.set_rotation(target, *value),
                _ => {}
            },
            Self::Vector2 { x, y } => store.set_scale(target, *x, *y),
            Self::Visible(visible) => store.set_visible(target, *visible),
            Self::Color(color) => apply_color(store, target, *color),
            Self::Text(text) => {
                let text = text.as_deref().unwrap_or("");
                if action == ActionType::Icon {
                    store.apply_icon_to(target, text);
                } else {
                    store.set_object_text(target, text);
                }
            }
            Self::ColorFilter(params) => store.set_color_filter(
                target,
                Some(crate::color_matrix::ColorFilterParams {
                    brightness: params[0],
                    contrast: params[1],
                    saturation: params[2],
                    hue: params[3],
                }),
            ),
            Self::Animation { playing, frame } => apply_animation(store, target, *playing, *frame),
            // 音效不由核心层播放：它只入队，由使用方取走交给音频系统。
            Self::Sound { sound, volume } => {
                if let Some(sound) = sound {
                    store.queue_sound(target, sound.clone(), *volume);
                }
            }
            // 抖动与嵌套过渡需要跨帧驱动：前者要逐帧随机位移，后者要驱动
            // 目标对象上的另一条过渡，两者都不是「套一个取值」能表达的。
            Self::Shake { .. } | Self::Transition { .. } => {}
            Self::None => {}
        }
    }

    /// 与另一个同构取值之间按进度插值。
    pub fn lerp(&self, other: &Self, progress: f32) -> Self {
        match (self, other) {
            (
                Self::PercentPosition {
                    use_x,
                    use_y,
                    x: sx,
                    y: sy,
                },
                Self::PercentPosition { x: ex, y: ey, .. },
            ) => Self::PercentPosition {
                use_x: *use_x,
                use_y: *use_y,
                x: sx + (ex - sx) * progress,
                y: sy + (ey - sy) * progress,
            },
            (
                Self::Pair {
                    use_x,
                    use_y,
                    x: sx,
                    y: sy,
                },
                Self::Pair { x: ex, y: ey, .. },
            ) => Self::Pair {
                use_x: *use_x,
                use_y: *use_y,
                x: sx + (ex - sx) * progress,
                y: sy + (ey - sy) * progress,
            },
            (Self::Scalar(start), Self::Scalar(end)) => {
                Self::Scalar(start + (end - start) * progress)
            }
            (Self::Vector2 { x: sx, y: sy }, Self::Vector2 { x: ex, y: ey }) => Self::Vector2 {
                x: sx + (ex - sx) * progress,
                y: sy + (ey - sy) * progress,
            },
            (Self::Color(start), Self::Color(end)) => Self::Color(Color::new(
                start.r + (end.r - start.r) * progress,
                start.g + (end.g - start.g) * progress,
                start.b + (end.b - start.b) * progress,
                start.a + (end.a - start.a) * progress,
            )),
            (Self::ColorFilter(start), Self::ColorFilter(end)) => {
                let mut out = [0.0f32; 4];
                for (index, slot) in out.iter_mut().enumerate() {
                    *slot = start[index] + (end[index] - start[index]) * progress;
                }
                Self::ColorFilter(out)
            }
            // 其余类型无法插值，直接跳到终点。
            _ => other.clone(),
        }
    }
}

/// 补间配置。
#[derive(Debug, Clone, PartialEq)]
pub struct TweenConfig {
    /// 时长（秒）。
    pub duration: f32,
    /// 缓动类型。
    pub ease_type: EaseType,
    /// 重复次数，`0` 表示不重复。
    pub repeat: i32,
    /// 是否往复播放。
    pub yoyo: bool,
    /// 补间结束钩子的名称，不改变时间轴。
    pub end_label: Option<String>,
    /// 导出的运动路径；坐标相对补间起点。
    pub path: Option<super::GPath>,
    /// 起始取值。
    pub start_value: TransitionValue,
    /// 结束取值。
    pub end_value: TransitionValue,
}

impl TweenConfig {
    /// 包含重复的实际播放时长；负数 repeat 表示无限。
    pub fn playback_duration(&self) -> f32 {
        if self.duration <= 0.0 {
            0.0
        } else if self.repeat < 0 {
            f32::INFINITY
        } else {
            self.duration * (self.repeat as f32 + 1.0)
        }
    }

    pub(crate) fn progress_at(&self, elapsed: f32) -> f32 {
        if self.duration <= 0.0 || elapsed >= self.playback_duration() {
            return if self.yoyo && self.repeat > 0 && self.repeat % 2 != 0 {
                0.0
            } else {
                1.0
            };
        }
        let elapsed = elapsed.max(0.0);
        let cycle = (elapsed / self.duration).floor();
        let time = elapsed % self.duration;
        evaluate(
            self.ease_type,
            if self.yoyo && cycle % 2.0 != 0.0 {
                self.duration - time
            } else {
                time
            },
            self.duration,
        )
    }
    /// 按进度插值出当前取值。
    pub fn value_at(&self, progress: f32) -> TransitionValue {
        if let Some(path) = &self.path
            && let TransitionValue::Pair { x, y, .. } = &self.start_value
        {
            let point = path.point_at(progress);
            return TransitionValue::Pair {
                use_x: true,
                use_y: true,
                x: x + point.x,
                y: y + point.y,
            };
        }
        self.start_value.lerp(&self.end_value, progress)
    }
}

/// 一个过渡项。
#[derive(Debug, Clone, PartialEq)]
pub struct TransitionItem {
    /// 属性类型。
    pub action: ActionType,
    /// 该项开始作用的时刻（秒）。
    pub time: f32,
    /// 目标对象在容器内的编号。`-1` 表示过渡所属的容器自身。
    pub target_index: i16,
    /// 该项的标签。过渡可以按标签跳转到指定时刻。
    pub label: Option<String>,
    /// 未配置补间时的目标取值。
    pub value: TransitionValue,
    /// 补间配置。
    pub tween: Option<TweenConfig>,
}

impl TransitionItem {
    /// 按已过去的时长把取值套用到目标上。
    pub fn apply_elapsed(&self, store: &mut ObjectStore, target: GObjectId, elapsed: f32) {
        match &self.tween {
            None => self.value.apply(store, target, self.action),
            Some(tween) => {
                let progress = tween.progress_at(elapsed);
                tween.value_at(progress).apply(store, target, self.action);
            }
        }
    }

    /// 该项的结束时刻。
    pub fn end_time(&self) -> f32 {
        match &self.tween {
            Some(tween) => self.time + tween.duration,
            None => self.time,
        }
    }
}

/// 按属性类型解码一个取值。
pub(crate) fn decode_value(
    action: ActionType,
    buffer: &mut ByteBuffer<'_>,
    table: &StringTable,
) -> Result<TransitionValue> {
    let version = buffer.version();

    Ok(match action {
        ActionType::Xy | ActionType::Size | ActionType::Pivot | ActionType::Skew => {
            let use_x = buffer.read_bool()?;
            let use_y = buffer.read_bool()?;
            let x = buffer.read_f32()?;
            let y = buffer.read_f32()?;

            // 坐标额外带一位「是否按比例」，尺寸与斜切没有。
            if version >= 2 && action == ActionType::Xy && buffer.read_bool()? {
                return Ok(TransitionValue::PercentPosition { use_x, use_y, x, y });
            }

            TransitionValue::Pair { use_x, use_y, x, y }
        }
        ActionType::Alpha | ActionType::Rotation => TransitionValue::Scalar(buffer.read_f32()?),
        ActionType::Scale => TransitionValue::Vector2 {
            x: buffer.read_f32()?,
            y: buffer.read_f32()?,
        },
        ActionType::Color => TransitionValue::Color(read_color(buffer)?),
        // 注意顺序：先播放状态，再帧号。
        ActionType::Animation => TransitionValue::Animation {
            playing: buffer.read_bool()?,
            frame: buffer.read_i32()?,
        },
        ActionType::Visible => TransitionValue::Visible(buffer.read_bool()?),
        ActionType::Sound => TransitionValue::Sound {
            sound: buffer.read_s(table)?,
            volume: buffer.read_f32()?,
        },
        ActionType::Transition => TransitionValue::Transition {
            name: buffer.read_s(table)?,
            times: buffer.read_i32()?,
        },
        ActionType::Shake => TransitionValue::Shake {
            amplitude: buffer.read_f32()?,
            duration: buffer.read_f32()?,
        },
        ActionType::ColorFilter => TransitionValue::ColorFilter([
            buffer.read_f32()?,
            buffer.read_f32()?,
            buffer.read_f32()?,
            buffer.read_f32()?,
        ]),
        ActionType::Text | ActionType::Icon => TransitionValue::Text(buffer.read_s(table)?),
        ActionType::Unknown => TransitionValue::None,
    })
}

fn read_color(buffer: &mut ByteBuffer<'_>) -> Result<Color> {
    let r = buffer.read_u8()?;
    let g = buffer.read_u8()?;
    let b = buffer.read_u8()?;
    let a = buffer.read_u8()?;
    Ok(Color::from_rgba8(r, g, b, a))
}

fn apply_color(store: &mut ObjectStore, target: GObjectId, color: Color) {
    store.set_control_color(target, color, None);
}

/// 套用序列帧的播放状态。
///
/// 帧号为负表示「只改播放状态，不动当前帧」——描述里不指定帧时就是这个形状，
/// 强行归零会让影片剪辑每次被过渡碰到都跳回开头。
fn apply_animation(store: &mut ObjectStore, target: GObjectId, playing: bool, frame: i32) {
    store.set_animation_playing(target, playing);
    if frame >= 0 {
        store.set_animation_frame(target, frame);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_codes_match_official_table() {
        assert_eq!(ActionType::from_code(0), ActionType::Xy);
        assert_eq!(ActionType::from_code(4), ActionType::Alpha);
        assert_eq!(ActionType::from_code(5), ActionType::Rotation);
        assert_eq!(ActionType::from_code(7), ActionType::Animation);
        assert_eq!(ActionType::from_code(15), ActionType::Icon);
        assert_eq!(ActionType::from_code(99), ActionType::Unknown);
    }

    #[test]
    fn scalar_interpolates_linearly() {
        let start = TransitionValue::Scalar(0.0);
        let end = TransitionValue::Scalar(1.0);
        assert_eq!(start.lerp(&end, 0.25), TransitionValue::Scalar(0.25));
    }

    #[test]
    fn pair_keeps_the_use_flags_from_the_start() {
        let start = TransitionValue::Pair {
            use_x: true,
            use_y: false,
            x: 0.0,
            y: 0.0,
        };
        let end = TransitionValue::Pair {
            use_x: false,
            use_y: true,
            x: 100.0,
            y: 50.0,
        };

        let mid = start.lerp(&end, 0.5);
        match mid {
            TransitionValue::Pair { use_x, use_y, x, y } => {
                assert!(use_x);
                assert!(!use_y, "是否参与变化应当沿用起点的那一份");
                assert!((x - 50.0).abs() < 1e-6);
                assert!((y - 25.0).abs() < 1e-6);
            }
            other => panic!("类型不符：{other:?}"),
        }
    }

    #[test]
    fn color_interpolates_each_channel() {
        let start = TransitionValue::Color(Color::from_rgba8(0, 0, 0, 255));
        let end = TransitionValue::Color(Color::from_rgba8(255, 100, 50, 255));

        let mid = start.lerp(&end, 0.5);
        match mid {
            TransitionValue::Color(color) => {
                assert!((color.r - 0.5).abs() < 1e-3);
                assert!((color.g - 100.0 / 255.0 / 2.0).abs() < 1e-3);
            }
            other => panic!("类型不符：{other:?}"),
        }
    }

    #[test]
    fn non_interpolatable_values_jump_to_the_end() {
        let start = TransitionValue::Visible(true);
        let end = TransitionValue::Visible(false);
        assert_eq!(start.lerp(&end, 0.5), TransitionValue::Visible(false));
    }

    #[test]
    fn color_filter_value_lands_on_the_object() {
        let mut store = ObjectStore::new();
        let object = store.create(crate::package::ObjectType::Component);

        TransitionValue::ColorFilter([0.5, 1.2, 0.8, 30.0]).apply(
            &mut store,
            object,
            ActionType::ColorFilter,
        );

        let filter = store
            .object(object)
            .expect("object")
            .color_filter()
            .expect("应当套上了滤镜");
        assert!((filter.brightness - 0.5).abs() < 1e-6);
        assert!((filter.contrast - 1.2).abs() < 1e-6);
        assert!((filter.saturation - 0.8).abs() < 1e-6);
        assert!((filter.hue - 30.0).abs() < 1e-6);
    }

    #[test]
    fn animation_value_sets_playback_state() {
        let mut store = ObjectStore::new();
        let object = store.create(crate::package::ObjectType::MovieClip);

        TransitionValue::Animation {
            playing: true,
            frame: 3,
        }
        .apply(&mut store, object, ActionType::Animation);

        match store.object(object).expect("object").control() {
            crate::widget::ControlData::MovieClip(data) => {
                assert!(data.playing);
                assert_eq!(data.frame, 3);
            }
            other => panic!("类型不符：{other:?}"),
        }
    }

    #[test]
    fn negative_frame_leaves_the_current_frame_alone() {
        let mut store = ObjectStore::new();
        let object = store.create(crate::package::ObjectType::MovieClip);

        // 先把帧推到一个非零位置。
        TransitionValue::Animation {
            playing: false,
            frame: 5,
        }
        .apply(&mut store, object, ActionType::Animation);

        // 负帧号表示「只改播放状态」。
        TransitionValue::Animation {
            playing: true,
            frame: -1,
        }
        .apply(&mut store, object, ActionType::Animation);

        match store.object(object).expect("object").control() {
            crate::widget::ControlData::MovieClip(data) => {
                assert!(data.playing, "播放状态应当被改掉");
                assert_eq!(data.frame, 5, "负帧号不该动当前帧");
            }
            other => panic!("类型不符：{other:?}"),
        }
    }

    #[test]
    fn sound_value_is_queued_rather_than_played() {
        let mut store = ObjectStore::new();
        let object = store.create(crate::package::ObjectType::Component);

        TransitionValue::Sound {
            sound: Some("ui://pkg/click".to_string()),
            volume: 0.7,
        }
        .apply(&mut store, object, ActionType::Sound);

        let queued = store.take_queued_sounds();
        assert_eq!(queued.len(), 1, "音效应当排进队列");
        assert_eq!(queued[0].0, object);
        assert_eq!(queued[0].1, "ui://pkg/click");
        assert!((queued[0].2 - 0.7).abs() < 1e-6);
        assert_eq!(store.queued_sound_count(), 0, "取走后队列应当清空");
    }

    #[test]
    fn empty_sound_name_is_not_queued() {
        let mut store = ObjectStore::new();
        let object = store.create(crate::package::ObjectType::Component);

        TransitionValue::Sound {
            sound: None,
            volume: 1.0,
        }
        .apply(&mut store, object, ActionType::Sound);

        assert_eq!(store.queued_sound_count(), 0, "没指定音效时不该入队");
    }

    #[test]
    fn item_end_time_accounts_for_tween_duration() {
        let item = TransitionItem {
            action: ActionType::Alpha,
            time: 0.5,
            target_index: -1,
            label: None,
            value: TransitionValue::None,
            tween: None,
        };
        assert_eq!(item.end_time(), 0.5);

        let mut with_tween = item.clone();
        with_tween.tween = Some(TweenConfig {
            duration: 0.25,
            ease_type: 0,
            repeat: 0,
            yoyo: false,
            end_label: None,
            path: None,
            start_value: TransitionValue::Scalar(0.0),
            end_value: TransitionValue::Scalar(1.0),
        });
        assert_eq!(with_tween.end_time(), 0.75);
    }
}

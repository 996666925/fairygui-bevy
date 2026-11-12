use super::{ActionType, Transition, TransitionHook, TransitionValue};
use crate::color::Color;
use crate::object::{GObjectId, ObjectStore};

/// 用户钩子的路由编号；回调由消费方处理，核心不持有闭包。
pub type TransitionListenerId = u64;

#[derive(Debug, Clone, PartialEq)]
pub struct TransitionCallback {
    pub listener: TransitionListenerId,
    pub hook: TransitionHook,
}

/// Rust 类型化的 setValue 参数。可选字段为空时保留导出值。
#[derive(Debug, Clone, PartialEq)]
pub enum TransitionEditValue {
    Pair {
        x: f32,
        y: f32,
    },
    Scalar(f32),
    Color(Color),
    Animation {
        frame: i32,
        playing: Option<bool>,
    },
    Visible(bool),
    Sound {
        sound: Option<String>,
        volume: Option<f32>,
    },
    Transition {
        name: Option<String>,
        times: Option<i32>,
    },
    Shake {
        amplitude: f32,
        duration: Option<f32>,
    },
    ColorFilter([f32; 4]),
    Text(Option<String>),
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TransitionEditError {
    #[error("transition not found: {0}")]
    TransitionNotFound(String),
    #[error("transition is active on the driver stack")]
    TransitionBusy,
    #[error("transition label not found: {0}")]
    LabelNotFound(String),
    #[error("value does not match action {action:?} at label {label}")]
    IncompatibleValue { label: String, action: ActionType },
    #[error("transition value must be finite and durations must be nonnegative")]
    InvalidValue,
    #[error("transition target no longer exists")]
    InvalidTarget,
}

impl TransitionEditValue {
    fn valid(&self) -> bool {
        match self {
            Self::Pair { x, y } => x.is_finite() && y.is_finite(),
            Self::Scalar(value) => value.is_finite(),
            Self::Color(color) => [color.r, color.g, color.b, color.a]
                .into_iter()
                .all(f32::is_finite),
            Self::Sound { volume, .. } => volume.is_none_or(f32::is_finite),
            Self::Shake {
                amplitude,
                duration,
            } => {
                amplitude.is_finite()
                    && duration.is_none_or(|value| value.is_finite() && value >= 0.0)
            }
            Self::ColorFilter(values) => values.iter().all(|value| value.is_finite()),
            _ => true,
        }
    }

    fn apply(&self, action: ActionType, value: &TransitionValue) -> Option<TransitionValue> {
        Some(match (self, action, value) {
            (
                Self::Pair { x, y },
                ActionType::Xy | ActionType::Size | ActionType::Pivot | ActionType::Skew,
                TransitionValue::PercentPosition { .. },
            ) => TransitionValue::PercentPosition {
                use_x: true,
                use_y: true,
                x: *x,
                y: *y,
            },
            (
                Self::Pair { x, y },
                ActionType::Xy | ActionType::Size | ActionType::Pivot | ActionType::Skew,
                _,
            ) => TransitionValue::Pair {
                use_x: true,
                use_y: true,
                x: *x,
                y: *y,
            },
            (Self::Pair { x, y }, ActionType::Scale, _) => {
                TransitionValue::Vector2 { x: *x, y: *y }
            }
            (Self::Scalar(value), ActionType::Alpha | ActionType::Rotation, _) => {
                TransitionValue::Scalar(*value)
            }
            (Self::Color(value), ActionType::Color, _) => TransitionValue::Color(*value),
            (
                Self::Animation { frame, playing },
                ActionType::Animation,
                TransitionValue::Animation { playing: old, .. },
            ) => TransitionValue::Animation {
                frame: *frame,
                playing: playing.unwrap_or(*old),
            },
            (Self::Visible(value), ActionType::Visible, _) => TransitionValue::Visible(*value),
            (
                Self::Sound { sound, volume },
                ActionType::Sound,
                TransitionValue::Sound { volume: old, .. },
            ) => TransitionValue::Sound {
                sound: sound.clone(),
                volume: volume.unwrap_or(*old),
            },
            (
                Self::Transition { name, times },
                ActionType::Transition,
                TransitionValue::Transition { times: old, .. },
            ) => TransitionValue::Transition {
                name: name.clone(),
                times: times.unwrap_or(*old),
            },
            (
                Self::Shake {
                    amplitude,
                    duration,
                },
                ActionType::Shake,
                TransitionValue::Shake { duration: old, .. },
            ) => TransitionValue::Shake {
                amplitude: *amplitude,
                duration: duration.unwrap_or(*old),
            },
            (Self::ColorFilter(values), ActionType::ColorFilter, _) => {
                TransitionValue::ColorFilter(*values)
            }
            (Self::Text(text), ActionType::Text | ActionType::Icon, _) => {
                TransitionValue::Text(text.clone())
            }
            _ => return None,
        })
    }
}

impl Transition {
    /// 更新所有同名起点/终点；起点优先。已创建的补间参数留到下一轮生效。
    pub fn set_value(
        &mut self,
        label: &str,
        patch: TransitionEditValue,
    ) -> Result<usize, TransitionEditError> {
        if !patch.valid() {
            return Err(TransitionEditError::InvalidValue);
        }
        let mut updates = Vec::new();
        for (index, item) in self.items.iter().enumerate() {
            let at_end = if item.label.as_deref() == Some(label) {
                false
            } else if item
                .tween
                .as_ref()
                .is_some_and(|tween| tween.end_label.as_deref() == Some(label))
            {
                true
            } else {
                continue;
            };
            let old = item.tween.as_ref().map_or(&item.value, |tween| {
                if at_end {
                    &tween.end_value
                } else {
                    &tween.start_value
                }
            });
            let value = patch.apply(item.action, old).ok_or_else(|| {
                TransitionEditError::IncompatibleValue {
                    label: label.into(),
                    action: item.action,
                }
            })?;
            updates.push((index, at_end, value));
        }
        if updates.is_empty() {
            return Err(TransitionEditError::LabelNotFound(label.into()));
        }
        let count = updates.len();
        for (index, at_end, value) in updates {
            let item = &mut self.items[index];
            if let Some(tween) = &mut item.tween {
                if at_end {
                    tween.end_value = value;
                } else {
                    tween.start_value = value;
                }
            } else {
                item.value = value;
            }
        }
        Ok(count)
    }

    /// 只改同名起点的补间时长，不修改 total_duration 或当前轮已创建的任务。
    pub fn set_duration(
        &mut self,
        label: &str,
        duration: f32,
    ) -> Result<usize, TransitionEditError> {
        if !duration.is_finite() || duration < 0.0 {
            return Err(TransitionEditError::InvalidValue);
        }
        let mut count = 0;
        for item in &mut self.items {
            if item.label.as_deref() == Some(label)
                && let Some(tween) = &mut item.tween
            {
                tween.duration = duration;
                count += 1;
            }
        }
        if count == 0 {
            Err(TransitionEditError::LabelNotFound(label.into()))
        } else {
            Ok(count)
        }
    }

    /// 按源码顺序返回第一个同名起点/终点的导出时间；重复和反向不改变标签时间。
    pub fn label_time(&self, label: &str) -> Option<f32> {
        self.items.iter().find_map(|item| {
            if item.label.as_deref() == Some(label) {
                Some(item.time)
            } else {
                item.tween
                    .as_ref()
                    .filter(|tween| tween.end_label.as_deref() == Some(label))
                    .map(|tween| item.time + tween.duration)
            }
        })
    }

    /// 仅绑定第一个匹配项；None 清除该项的用户钩子。自动标签通知保持不变。
    pub fn set_hook(
        &mut self,
        label: &str,
        listener: Option<TransitionListenerId>,
    ) -> Result<(), TransitionEditError> {
        let key = self
            .items
            .iter()
            .enumerate()
            .find_map(|(index, item)| {
                if item.label.as_deref() == Some(label) {
                    Some((index, false))
                } else {
                    item.tween
                        .as_ref()
                        .filter(|tween| tween.end_label.as_deref() == Some(label))
                        .map(|_| (index, true))
                }
            })
            .ok_or_else(|| TransitionEditError::LabelNotFound(label.into()))?;
        if let Some(listener) = listener {
            self.callbacks.insert(key, listener);
        } else {
            self.callbacks.remove(&key);
        }
        Ok(())
    }

    pub fn clear_hooks(&mut self) {
        self.callbacks.clear();
    }

    /// 修改所有同名起点的目标；None/owner 表示容器本身，其他目标必须仍存在。
    pub fn set_target(
        &mut self,
        store: &ObjectStore,
        owner: GObjectId,
        label: &str,
        target: Option<GObjectId>,
    ) -> Result<usize, TransitionEditError> {
        let target = target.unwrap_or(owner);
        if !store.contains(target) {
            return Err(TransitionEditError::InvalidTarget);
        }
        let indices: Vec<_> = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| (item.label.as_deref() == Some(label)).then_some(index))
            .collect();
        if indices.is_empty() {
            return Err(TransitionEditError::LabelNotFound(label.into()));
        }
        if self.bound_targets.is_none() {
            self.bind_targets(store, owner);
        }
        let targets = self.bound_targets.as_mut().unwrap();
        for index in &indices {
            targets[*index] = Some(target);
        }
        Ok(indices.len())
    }

    /// 关联布局移动只平移绝对 XY 定义；百分比坐标与当前轮数值副本不变。
    pub fn update_from_relations(
        &mut self,
        store: &ObjectStore,
        owner: GObjectId,
        target: GObjectId,
        dx: f32,
        dy: f32,
    ) {
        if !dx.is_finite() || !dy.is_finite() {
            return;
        }
        if self.bound_targets.is_none() {
            self.bind_targets(store, owner);
        }
        let targets = self.bound_targets.as_ref().unwrap();
        let shift = |value: &mut TransitionValue| {
            if let TransitionValue::Pair { x, y, .. } = value {
                *x += dx;
                *y += dy;
            }
        };
        for (index, item) in self.items.iter_mut().enumerate() {
            if item.action == ActionType::Xy
                && targets.get(index).copied().flatten() == Some(target)
            {
                if let Some(tween) = &mut item.tween {
                    shift(&mut tween.start_value);
                    shift(&mut tween.end_value);
                } else {
                    shift(&mut item.value);
                }
            }
        }
    }
}

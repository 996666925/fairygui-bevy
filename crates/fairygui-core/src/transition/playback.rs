use super::{
    ActionType, Transition, TransitionPlayOptions, TransitionValue, TweenConfig, evaluate,
};
use crate::geom::Point;
use crate::object::{DisplayLockToken, GObjectId, NestedHandle, ObjectStore, TransitionId};
use std::collections::{HashMap, HashSet};

/// 时间轴经过具名起点或补间终点时产生的通知。
#[derive(Debug, Clone, PartialEq)]
pub struct TransitionHook {
    pub owner: GObjectId,
    pub transition: String,
    pub label: String,
    pub at_end: bool,
}

/// One naturally completed playback, after all repetitions and nested tasks.
/// Stopping, replacement, detachment and disposal do not synthesize completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionComplete {
    pub owner: GObjectId,
    pub transition: String,
    pub play_generation: u64,
}

#[derive(Debug, Clone, Default)]
pub(super) struct PlaybackState {
    reset: bool,
    initialized: bool,
    base: Point,
    started: HashSet<usize>,
    completed: HashSet<usize>,
    tweens: HashMap<usize, TweenConfig>,
    schedule: HashMap<usize, ScheduledItem>,
    offsets: HashMap<usize, (GObjectId, Point)>,
    locks: Vec<(GObjectId, DisplayLockToken)>,
    locks_acquired: bool,
    random: u64,
    plans_prepared: bool,
    plans: Vec<NestedPlan>,
    nested: Vec<NestedHandle>,
    sampled_time: Option<f32>,
    paused_movies: Vec<(GObjectId, bool)>,
}

#[derive(Debug, Clone)]
struct NestedPlan {
    index: usize,
    id: TransitionId,
    times: i32,
    end_time: Option<f32>,
}

#[derive(Debug, Clone)]
struct ScheduledItem {
    time: f32,
    end: f32,
    tween: Option<TweenConfig>,
    shake: Option<(f32, f32)>,
}

impl Transition {
    pub(crate) fn prepared_nested_ids(&self) -> Vec<TransitionId> {
        self.runtime.plans.iter().map(|plan| plan.id).collect()
    }

    pub(crate) fn apply_pending_relations(&mut self, store: &mut ObjectStore, owner: GObjectId) {
        for (target, dx, dy) in store.take_transition_relation_updates() {
            self.update_from_relations(store, owner, target, dx, dy);
        }
    }

    fn apply_pending_stage(&mut self, store: &mut ObjectStore, owner: GObjectId) {
        for added in store.take_transition_stage_updates() {
            if !added && self.options & 2 == 0 {
                self.stop_with_callback(store, owner, self.options & 4 != 0, false);
            } else if added && self.auto_play && !self.playing {
                self.play(self.auto_play_times, self.auto_play_delay);
            }
        }
    }
    pub fn is_paused(&self) -> bool {
        self.paused
    }
    pub fn time_scale(&self) -> f32 {
        self.time_scale.unwrap_or(1.0)
    }
    pub fn is_reversed(&self) -> bool {
        self.reversed
    }

    pub fn set_paused(&mut self, store: &mut ObjectStore, owner: GObjectId, paused: bool) -> bool {
        if !self.playing || self.paused == paused {
            return false;
        }
        self.paused = paused;
        for plan in &self.runtime.plans {
            store.pause_nested_transition(plan.id, paused);
        }
        if paused {
            for index in 0..self.items.len() {
                if self.items[index].action == ActionType::Animation
                    && let Some(target) = self.target(store, owner, index)
                    && let Some((playing, _)) = store.animation_state(target)
                {
                    self.runtime.paused_movies.push((target, playing));
                    store.set_animation_playing(target, false);
                }
            }
        } else {
            for (target, playing) in self.runtime.paused_movies.drain(..) {
                store.set_animation_playing(target, playing);
            }
        }
        true
    }

    pub fn set_time_scale(
        &mut self,
        store: &mut ObjectStore,
        owner: GObjectId,
        scale: f32,
    ) -> bool {
        if !scale.is_finite() || scale < 0.0 || self.time_scale() == scale {
            return false;
        }
        self.time_scale = Some(scale);
        if self.playing {
            for plan in &self.runtime.plans {
                if !self.runtime.initialized || self.runtime.started.contains(&plan.index) {
                    store.scale_nested_transition(plan.id, scale);
                }
            }
            for index in 0..self.items.len() {
                if self.items[index].action == ActionType::Animation
                    && (!self.runtime.initialized || self.runtime.started.contains(&index))
                    && let Some(target) = self.target(store, owner, index)
                {
                    store.set_animation_time_scale(target, scale);
                }
            }
        }
        true
    }

    fn item_time(&self, index: usize) -> f32 {
        if let Some(item) = self.runtime.schedule.get(&index) {
            return item.time;
        }
        let item = &self.items[index];
        if !self.reversed {
            return item.time;
        }
        let duration = if let Some(tween) = &item.tween {
            tween.duration
        } else if let TransitionValue::Shake { duration, .. } = &item.value {
            *duration
        } else {
            0.0
        };
        self.total_duration - item.time - duration
    }

    fn item_end(&self, index: usize) -> f32 {
        if let Some(item) = self.runtime.schedule.get(&index) {
            return item.end;
        }
        self.item_time(index)
            + if let Some(tween) = &self.items[index].tween {
                tween.playback_duration()
            } else if let TransitionValue::Shake { duration, .. } = &self.items[index].value {
                duration.max(0.0)
            } else {
                0.0
            }
    }
    pub(crate) fn begin_frame(&mut self) {
        self.zero_yield = false;
    }
    pub(crate) fn yielded(&self) -> bool {
        self.zero_yield
    }
    pub(crate) fn has_nested_notification(&self) -> bool {
        self.playing
            && self
                .runtime
                .nested
                .iter()
                .any(|handle| handle.completed.load(std::sync::atomic::Ordering::Relaxed))
    }
    /// Increments on each new playback; identifies queued completion records.
    pub fn play_generation(&self) -> u64 {
        self.generation
    }
    pub(crate) fn set_completion_token(
        &mut self,
        token: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) {
        self.on_complete = Some(token);
    }

    pub(crate) fn stop_for_replay(&mut self, store: &mut ObjectStore, owner: GObjectId) {
        self.stop_with_callback(store, owner, true, true);
    }

    pub(crate) fn stop_with_callback(
        &mut self,
        store: &mut ObjectStore,
        owner: GObjectId,
        complete: bool,
        process_callback: bool,
    ) {
        let callback = self.on_complete.take();
        if complete {
            self.stop_with_completion(store, owner);
        } else {
            self.stop_and_restore(store);
        }
        if process_callback && let Some(callback) = callback {
            callback.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    pub(crate) fn nested_calls(
        &self,
        store: &ObjectStore,
        owner: GObjectId,
    ) -> Vec<(usize, GObjectId, String, i32)> {
        self.items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                let TransitionValue::Transition {
                    name: Some(name),
                    times,
                } = &item.value
                else {
                    return None;
                };
                let target = if self.bound_targets.is_some() {
                    self.target(store, owner, index)
                } else if item.target_index < 0 {
                    Some(owner)
                } else {
                    store.child_at(owner, item.target_index as usize)
                }?;
                Some((index, target, name.clone(), *times))
            })
            .collect()
    }

    fn prepare_nested(&mut self, store: &ObjectStore, owner: GObjectId) {
        if self.runtime.plans_prepared {
            return;
        }
        self.runtime.plans_prepared = true;
        for (index, target, name, times) in self.nested_calls(store, owner) {
            let Some(id) = store.find_transition_id(target, &name) else {
                continue;
            };
            if !store.nested_allowed(id) {
                continue;
            }
            if times == 0
                && let Some(previous) = self
                    .runtime
                    .plans
                    .iter_mut()
                    .rev()
                    .find(|plan| plan.id == id)
            {
                previous.end_time =
                    Some((self.items[index].time - self.items[previous.index].time).max(0.0));
                continue;
            }
            self.runtime.plans.push(NestedPlan {
                index,
                id,
                times: if times == 0 { 1 } else { times },
                end_time: (times == 0).then_some(0.0),
            });
        }
    }
    /// 构建时固定对象句柄。运行时调整子对象顺序不会改变动画的目标。
    pub(crate) fn bind_targets(&mut self, store: &ObjectStore, owner: GObjectId) {
        self.bound_targets = Some(
            self.items
                .iter()
                .map(|item| {
                    if item.target_index < 0 {
                        Some(owner)
                    } else {
                        store.child_at(owner, item.target_index as usize)
                    }
                })
                .collect(),
        );
    }

    fn target(&self, store: &ObjectStore, owner: GObjectId, index: usize) -> Option<GObjectId> {
        let id = self.bound_targets.as_ref()?.get(index).copied().flatten()?;
        let object = store.object(id)?;
        (id == owner || object.parent() == Some(owner)).then_some(id)
    }

    /// 开始播放。负数 times 无限循环；0 与 1 均播放一轮，delay 以秒计。
    /// 独立使用时，起始值在第一次 advance 时生效。
    pub fn play(&mut self, times: i32, delay: f32) {
        self.play_with_options(TransitionPlayOptions {
            times,
            delay: delay.max(0.0),
            ..Default::default()
        });
    }

    /// 直接采样 start_time，保留起点之前的属性，跳过历史音效和起始标签。
    /// 非法区间参数返回 false，且不打断现有播放。
    pub fn play_with_options(&mut self, options: TransitionPlayOptions) -> bool {
        if !options.is_valid() {
            return false;
        }
        self.runtime.reset = true;
        self.generation = self.generation.wrapping_add(1);
        self.zero_yield = false;
        self.end_time = options.end_time;
        self.start_time = options.start_time;
        self.playing = true;
        self.paused = false;
        self.reversed = options.reversed;
        self.time = 0.0;
        self.delay = options.delay;
        self.play_times = options.times;
        self.remaining_times = options.times;
        true
    }

    /// 与官方 playReverse 一样播放一轮；重复子过渡仍使用其导出次数。
    pub fn play_reverse(&mut self, delay: f32) {
        self.play(1, delay);
        self.reversed = true;
    }

    /// 停止推进。独立使用时，下次 advance 会清理抖动与显示锁。
    /// 需要立即归位时使用 stop_and_restore；ObjectStore 会自动调用后者。
    pub fn stop(&mut self) {
        self.playing = false;
        self.on_complete = None;
        self.runtime.reset = true;
    }

    pub fn stop_and_restore(&mut self, store: &mut ObjectStore) {
        self.stop_nested_plans(store, false);
        self.stop();
        self.clear_runtime(store);
    }

    pub fn change_play_times(&mut self, times: i32) {
        self.play_times = times;
        self.remaining_times = times;
    }

    pub(crate) fn stop_with_completion(&mut self, store: &mut ObjectStore, owner: GObjectId) {
        if self.playing && self.delay <= 0.0 {
            if self.bound_targets.is_none() {
                self.bind_targets(store, owner);
            }
            self.initialize_runtime(store, owner);
        }
        self.restore_offsets(store);
        if self.playing && self.delay <= 0.0 {
            if self.bound_targets.is_none() {
                self.bind_targets(store, owner);
            }
            for ordinal in 0..self.items.len() {
                let index = if self.reversed {
                    self.items.len() - 1 - ordinal
                } else {
                    ordinal
                };
                if self.runtime.completed.contains(&index) {
                    continue;
                }
                let Some(target) = self.target(store, owner, index) else {
                    continue;
                };
                let item = &self.items[index];
                if self.end_time.is_some_and(|end| self.item_time(index) > end) {
                    continue;
                }
                let action = item.action;
                if let Some(tween) = self
                    .runtime
                    .schedule
                    .get(&index)
                    .and_then(|item| item.tween.as_ref())
                {
                    let tween =
                        self.runtime.tweens.get(&index).cloned().unwrap_or_else(|| {
                            self.resolve_tween(store, owner, target, index, tween)
                        });
                    let elapsed = self
                        .end_time
                        .map(|end| (end - self.item_time(index)).max(0.0))
                        .unwrap_or_else(|| {
                            if tween.repeat < 0 {
                                tween.duration * 2.0
                            } else {
                                tween.playback_duration()
                            }
                        });
                    let progress = tween.progress_at(elapsed);
                    if !self.runtime.started.contains(&index)
                        && item.time >= self.start_time
                        && let Some(label) = item.label.clone()
                    {
                        self.hook(store, owner, index, label, false);
                    }
                    self.apply_sample(store, owner, target, action, &tween.value_at(progress));
                    if elapsed >= tween.playback_duration()
                        && let Some(label) = tween.end_label.clone()
                    {
                        self.hook(store, owner, index, label, true);
                    }
                } else if !self.runtime.started.contains(&index)
                    && !matches!(
                        action,
                        ActionType::Sound | ActionType::Shake | ActionType::Transition
                    )
                {
                    self.apply_sample(store, owner, target, action, &item.value);
                }
                self.apply_pending_relations(store, owner);
            }
        }
        self.stop_nested_plans(store, true);
        self.stop();
        self.clear_runtime(store);
    }

    /// 标签是时间点。结束标签指向补间结束时刻，不会让播放自动跳转。
    pub fn seek_to_label(&mut self, label: &str) -> bool {
        let time = self.label_time(label);
        if let Some(time) = time {
            self.time = time;
            self.start_time = time;
            self.runtime.reset = true;
            true
        } else {
            false
        }
    }

    /// 包括补间重复、往返与抖动的实际时长。
    pub fn playback_duration(&self) -> f32 {
        (0..self.items.len())
            .map(|index| self.item_end(index))
            .fold(0.0, f32::max)
    }

    fn scheduled(&self, index: usize) -> bool {
        if self.runtime.initialized {
            return self.runtime.schedule.contains_key(&index);
        }
        let item = &self.items[index];
        item.action == ActionType::Shake
            || self.end_time.is_none_or(|end| self.item_time(index) <= end)
            || (item.tween.is_none()
                && item.action != ActionType::Shake
                && self.item_time(index) <= self.start_time)
    }

    fn shake_breakpoint(&self, index: usize) -> Option<f32> {
        // The reference deliberately subtracts original item.time even when
        // reversed. Negative breakpoints disable truncation in GTweener.
        self.end_time
            .map(|end| end - self.items[index].time)
            .filter(|end| *end >= 0.0)
    }

    fn task_end(&self, index: usize) -> f32 {
        if self.items[index].action == ActionType::Shake {
            self.shake_breakpoint(index)
                .map_or(self.item_end(index), |end| {
                    self.item_end(index).min(self.item_time(index) + end)
                })
        } else {
            self.end_time
                .map_or(self.item_end(index), |end| self.item_end(index).min(end))
        }
    }

    fn pass_end(&self) -> f32 {
        (0..self.items.len())
            .filter(|index| self.scheduled(*index))
            .map(|index| self.task_end(index))
            .fold(self.start_time, f32::max)
    }

    /// 按事件边界推进，长帧也保留跨循环的剩余时间。
    pub fn advance(&mut self, store: &mut ObjectStore, owner: GObjectId, dt: f32) -> bool {
        store.advance_standalone_transition(self, owner, dt)
    }

    pub(crate) fn advance_driven(
        &mut self,
        store: &mut ObjectStore,
        owner: GObjectId,
        dt: f32,
    ) -> bool {
        if self.runtime.reset {
            self.clear_runtime(store);
        }
        if !self.playing {
            return false;
        }
        if !dt.is_finite() || dt < 0.0 {
            return true;
        }
        let mut remaining = dt;
        if self.bound_targets.is_none() {
            self.bind_targets(store, owner);
        }
        self.prepare_nested(store, owner);
        if self.delay > 0.0 {
            if self.paused {
                return true;
            }
            if remaining < self.delay {
                self.delay -= remaining;
                store.elapse_transition_clock(remaining);
                return true;
            }
            remaining -= self.delay;
            store.elapse_transition_clock(self.delay);
            self.delay = 0.0;
        }
        if self.paused && !self.runtime.initialized {
            return true;
        }
        loop {
            if !self.paused || self.runtime.initialized {
                self.initialize_runtime(store, owner);
                self.sample(store, owner);
            }
            if !self.playing {
                return false;
            }
            self.runtime
                .nested
                .retain(|handle| !handle.completed.load(std::sync::atomic::Ordering::Relaxed));
            let duration = self.pass_end();
            if self.time >= duration && self.runtime.nested.is_empty() {
                if self.remaining_times > 0 {
                    self.remaining_times -= 1;
                }
                if self.play_times >= 0 && self.remaining_times == 0 {
                    self.playing = false;
                    store.queue_transition_completion(TransitionComplete {
                        owner,
                        transition: self.name.clone(),
                        play_generation: self.generation,
                    });
                    if let Some(callback) = self.on_complete.take() {
                        callback.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                    self.clear_runtime(store);
                    return false;
                }
                let pass_duration = self.time - self.start_time;
                let plans = self.runtime.plans.clone();
                let locks = std::mem::take(&mut self.runtime.locks);
                let locks_acquired = self.runtime.locks_acquired;
                self.clear_runtime(store);
                self.runtime.plans = plans;
                self.runtime.plans_prepared = true;
                self.runtime.locks = locks;
                self.runtime.locks_acquired = locks_acquired;
                self.time = self.start_time;
                // A zero-duration infinite transition can only run once per tick.
                if pass_duration <= 0.0 {
                    self.zero_yield = true;
                    return true;
                }
                if remaining <= 0.0 {
                    self.initialize_runtime(store, owner);
                    self.sample(store, owner);
                    return true;
                }
                continue;
            }
            if remaining <= 0.0 {
                return true;
            }
            let step = remaining.min(self.next_step(store));
            if step <= 0.0 {
                return true;
            }
            let local_boundary = self.next_local_boundary();
            let mut next_time = self.time
                + if self.paused {
                    0.0
                } else {
                    step * self.time_scale()
                };
            if !self.paused
                && self.time_scale() > 0.0
                && local_boundary.is_finite()
                && (local_boundary - next_time).abs() <= 4.0 * f32::EPSILON * local_boundary.abs()
            {
                next_time = local_boundary;
            }
            remaining = (remaining - step).max(0.0);
            let nested = self.runtime.nested.clone();
            store.elapse_transition_clock(step);
            self.time = next_time;
            self.sample(store, owner);
            if !self.playing {
                return false;
            }
            let clock = store.current_transition_clock();
            for handle in nested {
                store.advance_nested_transition(&handle, clock);
            }
        }
    }

    fn next_local_boundary(&self) -> f32 {
        let duration = self.pass_end();
        let next = (0..self.items.len())
            .filter(|index| self.scheduled(*index))
            .flat_map(|index| [self.item_time(index), self.task_end(index)])
            .filter(|time| *time > self.time)
            .fold(f32::INFINITY, f32::min);
        if duration > self.time {
            next.min(duration)
        } else {
            next
        }
    }

    pub(crate) fn next_step(&self, store: &ObjectStore) -> f32 {
        if !self.playing || self.zero_yield {
            return f32::INFINITY;
        }
        if self.delay > 0.0 {
            return if self.paused {
                f32::INFINITY
            } else {
                self.delay
            };
        }
        let own = if self.paused || self.time_scale() == 0.0 {
            f32::INFINITY
        } else {
            (self.next_local_boundary() - self.time) / self.time_scale()
        };
        self.runtime
            .nested
            .iter()
            .filter(|handle| store.nested_playing(handle))
            .map(|handle| store.nested_next_step(handle))
            .fold(own, f32::min)
    }

    fn initialize_runtime(&mut self, store: &mut ObjectStore, owner: GObjectId) {
        self.prepare_nested(store, owner);
        if self.runtime.initialized {
            return;
        }
        for index in 0..self.items.len() {
            if self.scheduled(index) && self.target(store, owner, index).is_some() {
                let shake = if let TransitionValue::Shake {
                    amplitude,
                    duration,
                } = self.items[index].value
                {
                    Some((amplitude, duration))
                } else {
                    None
                };
                let scheduled = ScheduledItem {
                    time: self.item_time(index),
                    end: self.item_end(index),
                    tween: self.items[index].tween.clone(),
                    shake,
                };
                self.runtime.schedule.insert(index, scheduled);
            }
        }
        self.runtime.initialized = true;
        self.time = self.time.max(self.start_time);
        self.runtime.base = store
            .object(owner)
            .map(|o| o.position())
            .unwrap_or_default();
    }

    fn acquire_display_locks(&mut self, store: &mut ObjectStore, owner: GObjectId) {
        if self.options & 1 != 0
            && !self.runtime.locks_acquired
            && (self.pass_end() > self.time || !self.runtime.nested.is_empty())
        {
            self.runtime.locks_acquired = true;
            for index in 0..self.items.len() {
                if let Some(target) = self.target(store, owner, index).filter(|id| *id != owner)
                    && let Some(token) = store.add_display_lock(target)
                {
                    self.runtime.locks.push((target, token));
                }
            }
        }
    }

    fn sample(&mut self, store: &mut ObjectStore, owner: GObjectId) {
        self.apply_pending_stage(store, owner);
        if !self.playing {
            return;
        }
        if self.runtime.sampled_time == Some(self.time) {
            return;
        }
        let initial = self.runtime.sampled_time.is_none();
        let catch_up = initial && !self.reversed && self.start_time > 0.0;
        self.runtime.sampled_time = Some(self.time);
        for ordinal in 0..self.items.len() {
            let index = if self.reversed {
                self.items.len() - 1 - ordinal
            } else {
                ordinal
            };
            let item = &self.items[index];
            if initial && let Some(scheduled) = self.runtime.schedule.get_mut(&index) {
                // playItem creates and seeks each tweener in source order.
                // Earlier immediate Size actions can update later XY definitions.
                scheduled.tween = item.tween.clone();
            }
            if (catch_up && item.action == ActionType::Animation && item.time <= self.start_time)
                || !self.scheduled(index)
                || self.item_time(index) > self.time
                || self.runtime.completed.contains(&index)
            {
                continue;
            }
            let Some(target) = self.target(store, owner, index) else {
                continue;
            };
            let elapsed =
                self.end_time.map_or(self.time, |end| self.time.min(end)) - self.item_time(index);
            let action = item.action;
            let start = self.runtime.started.insert(index);
            if start {
                if action != ActionType::Shake
                    && item.time >= self.start_time
                    && let Some(label) = item.label.clone()
                {
                    self.hook(store, owner, index, label, false);
                }
                if let Some(tween) = self
                    .runtime
                    .schedule
                    .get(&index)
                    .and_then(|item| item.tween.as_ref())
                {
                    let resolved = self.resolve_tween(store, owner, target, index, tween);
                    self.runtime.tweens.insert(index, resolved);
                }
            }
            if let Some(tween) = self.runtime.tweens.get(&index) {
                let value = tween.value_at(tween.progress_at(elapsed));
                let completed = elapsed >= tween.playback_duration();
                let clipped = self.end_time.is_some_and(|end| self.time >= end);
                self.apply_sample(store, owner, target, action, &value);
                if completed || clipped {
                    if completed
                        && let Some(label) = self.items[index]
                            .tween
                            .as_ref()
                            .and_then(|t| t.end_label.clone())
                    {
                        self.hook(store, owner, index, label, true);
                    }
                    self.runtime.completed.insert(index);
                }
            } else if let Some((amplitude, duration)) = self
                .runtime
                .schedule
                .get(&index)
                .and_then(|item| item.shake)
            {
                let elapsed = self.time - self.item_time(index);
                let completed = elapsed >= duration
                    || self
                        .shake_breakpoint(index)
                        .is_some_and(|end| elapsed >= end);
                let radius = if completed {
                    0.0
                } else {
                    amplitude * (1.0 - evaluate(5, elapsed, duration))
                };
                let offset = Point::new(radius * self.random_sign(), radius * self.random_sign());
                self.set_offset(store, index, target, offset);
                if completed {
                    self.runtime.completed.insert(index);
                }
            } else if start {
                if let Some(plan) = self.runtime.plans.iter().find(|plan| plan.index == index) {
                    let parent_limit = self
                        .end_time
                        .map(|end| end - self.items[index].time)
                        .filter(|end| *end >= 0.0);
                    let end = match (parent_limit, plan.end_time) {
                        (Some(a), Some(b)) => Some(a.min(b)),
                        (a, b) => a.or(b),
                    };
                    if let Some(handle) = store.play_nested_transition(
                        plan.id,
                        TransitionPlayOptions {
                            times: match &self.items[index].value {
                                TransitionValue::Transition { times, .. } => *times,
                                _ => plan.times,
                            },
                            start_time: (self.start_time - self.items[index].time).max(0.0),
                            end_time: end,
                            reversed: self.reversed,
                            ..Default::default()
                        },
                        self.time_scale(),
                    ) {
                        self.runtime.nested.push(handle);
                    }
                }
                if action != ActionType::Sound || self.items[index].time >= self.start_time {
                    if action == ActionType::Sound {
                        if !self.cached_sounds.contains_key(&index)
                            && let TransitionValue::Sound {
                                sound: Some(sound), ..
                            } = &self.items[index].value
                        {
                            self.cached_sounds.insert(index, sound.clone());
                        }
                        if let Some(sound) = self.cached_sounds.get(&index)
                            && !sound.is_empty()
                            && let TransitionValue::Sound { volume, .. } = self.items[index].value
                        {
                            TransitionValue::Sound {
                                sound: Some(sound.clone()),
                                volume,
                            }
                            .apply(store, target, action);
                        }
                    } else {
                        self.apply_sample(store, owner, target, action, &self.items[index].value);
                    }
                }
                self.runtime.completed.insert(index);
            }
            self.apply_pending_relations(store, owner);
            self.apply_pending_stage(store, owner);
            if !self.playing {
                return;
            }
        }
        if catch_up {
            self.skip_animations(store, owner);
        }
        // Official onDelayedPlay acquires locks after internalPlay. A nested
        // transition has already begun when the resulting stage-entry arrives.
        self.acquire_display_locks(store, owner);
        self.apply_pending_stage(store, owner);
    }

    fn skip_animations(&mut self, store: &mut ObjectStore, owner: GObjectId) {
        let mut targets = HashSet::new();
        for index in 0..self.items.len() {
            if self.items[index].action != ActionType::Animation
                || self.items[index].time > self.start_time
            {
                continue;
            }
            let Some(target) = self.target(store, owner, index) else {
                continue;
            };
            if !targets.insert(target) {
                continue;
            }
            let Some((playing, _)) = store.animation_state(target) else {
                continue;
            };
            let mut play_start = playing.then_some(0.0);
            let mut total = 0.0;
            let mut explicit_frame = None;
            for i in index..self.items.len() {
                let item = &self.items[i];
                if item.action != ActionType::Animation
                    || item.time > self.start_time
                    || self.target(store, owner, i) != Some(target)
                {
                    continue;
                }
                let TransitionValue::Animation { playing, frame } = item.value else {
                    continue;
                };
                if frame != -1 {
                    explicit_frame = Some(frame);
                    total = 0.0;
                    play_start = playing.then_some(item.time);
                } else if playing {
                    play_start.get_or_insert(item.time);
                } else if let Some(start) = play_start.take() {
                    total += item.time - start;
                }
                if item.time >= self.start_time
                    && let Some(label) = item.label.clone()
                {
                    self.hook(store, owner, i, label, false);
                }
                self.runtime.started.insert(i);
                self.runtime.completed.insert(i);
            }
            if let Some(start) = play_start {
                total += self.start_time - start;
            }
            store.set_animation_playing(target, play_start.is_some());
            // With no explicit frame cue, the adapter retains its live frame,
            // including earlier queued catch-up operations.
            if let Some(frame) = explicit_frame {
                store.set_animation_frame(target, frame);
            }
            if total > 0.0 {
                store.advance_animation(target, total);
            }
        }
    }

    fn resolve_tween(
        &self,
        store: &ObjectStore,
        owner: GObjectId,
        target: GObjectId,
        index: usize,
        source: &TweenConfig,
    ) -> TweenConfig {
        let action = self.items[index].action;
        let mut tween = source.clone();
        if self.reversed {
            std::mem::swap(&mut tween.start_value, &mut tween.end_value);
        }
        if !matches!(action, ActionType::Xy | ActionType::Size) {
            return tween;
        }
        let Some(object) = store.object(target) else {
            return tween;
        };
        let pair = |value: &TransitionValue| match value {
            TransitionValue::Pair { use_x, use_y, x, y } => Some((*use_x, *use_y, *x, *y, false)),
            TransitionValue::PercentPosition { use_x, use_y, x, y } => {
                Some((*use_x, *use_y, *x, *y, true))
            }
            _ => None,
        };
        if let (Some((mask_sx, mask_sy, x0, y0, _)), Some((mask_ex, mask_ey, x1, y1, _))) =
            (pair(&tween.start_value), pair(&tween.end_value))
        {
            let live = self.items[index].tween.as_ref().unwrap_or(source);
            let (live_start, live_end) = if self.reversed {
                (&live.end_value, &live.start_value)
            } else {
                (&live.start_value, &live.end_value)
            };
            let (sx, sy, lx0, ly0, percent_start) = pair(live_start).unwrap();
            let (ex, ey, lx1, ly1, percent_end) = pair(live_end).unwrap();
            let size = store.object(owner).unwrap().size();
            let percent_start = action == ActionType::Xy && target != owner && percent_start;
            let percent_end = action == ActionType::Xy && target != owner && percent_end;
            let current = if action == ActionType::Size {
                Point::new(object.width(), object.height())
            } else if target == owner {
                Point::new(
                    object.x() - self.runtime.base.x,
                    object.y() - self.runtime.base.y,
                )
            } else {
                object.position()
            };
            let start_x = if !sx {
                current.x
            } else if percent_start {
                lx0 * size.width
            } else {
                x0
            };
            let start_y = if !sy {
                current.y
            } else if percent_start {
                ly0 * size.height
            } else {
                y0
            };
            tween.start_value = TransitionValue::Pair {
                use_x: mask_sx || mask_ex,
                use_y: mask_sy || mask_ey,
                x: start_x,
                y: start_y,
            };
            tween.end_value = TransitionValue::Pair {
                use_x: mask_sx || mask_ex,
                use_y: mask_sy || mask_ey,
                x: if !ex {
                    start_x
                } else if percent_end {
                    lx1 * size.width
                } else {
                    x1
                },
                y: if !ey {
                    start_y
                } else if percent_end {
                    ly1 * size.height
                } else {
                    y1
                },
            };
        }
        tween
    }

    fn apply_sample(
        &self,
        store: &mut ObjectStore,
        owner: GObjectId,
        target: GObjectId,
        action: ActionType,
        value: &TransitionValue,
    ) {
        if action == ActionType::Xy {
            match value {
                TransitionValue::Pair { use_x, use_y, x, y } if target == owner => {
                    TransitionValue::Pair {
                        use_x: *use_x,
                        use_y: *use_y,
                        x: x + self.runtime.base.x,
                        y: y + self.runtime.base.y,
                    }
                    .apply(store, target, action);
                    return;
                }
                TransitionValue::PercentPosition { use_x, use_y, x, y } => {
                    let size = store.object(owner).unwrap().size();
                    let (x, y) = if target == owner {
                        (x + self.runtime.base.x, y + self.runtime.base.y)
                    } else {
                        (x * size.width, y * size.height)
                    };
                    TransitionValue::Pair {
                        use_x: *use_x,
                        use_y: *use_y,
                        x,
                        y,
                    }
                    .apply(store, target, action);
                    return;
                }
                _ => {}
            }
        }
        value.apply(store, target, action);
        if action == ActionType::Animation {
            store.set_animation_time_scale(target, self.time_scale());
        }
    }

    fn hook(
        &self,
        store: &mut ObjectStore,
        owner: GObjectId,
        index: usize,
        label: String,
        at_end: bool,
    ) {
        let hook = TransitionHook {
            owner,
            transition: self.name.clone(),
            label,
            at_end,
        };
        if let Some(listener) = self.callbacks.get(&(index, at_end)) {
            store.queue_transition_callback(super::TransitionCallback {
                listener: *listener,
                hook: hook.clone(),
            });
        }
        store.queue_transition_hook(hook);
    }

    fn random_sign(&mut self) -> f32 {
        if self.runtime.random == 0 {
            static SEED: std::sync::atomic::AtomicU64 =
                std::sync::atomic::AtomicU64::new(0x9e3779b97f4a7c15);
            self.runtime.random =
                SEED.fetch_add(0x9e3779b97f4a7c15, std::sync::atomic::Ordering::Relaxed);
        }
        let state = &mut self.runtime.random;
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        if *state & 1 == 0 { -1.0 } else { 1.0 }
    }

    fn set_offset(
        &mut self,
        store: &mut ObjectStore,
        index: usize,
        target: GObjectId,
        offset: Point,
    ) {
        let old = self
            .runtime
            .offsets
            .insert(index, (target, offset))
            .map_or(Point::default(), |(_, offset)| offset);
        if let Some(object) = store.object(target) {
            let (x, y) = (object.x() - old.x + offset.x, object.y() - old.y + offset.y);
            store.with_gear_lock(target, |store| store.set_xy(target, x, y));
        }
    }

    fn restore_offsets(&mut self, store: &mut ObjectStore) {
        for (_, (target, offset)) in self.runtime.offsets.drain() {
            if let Some(object) = store.object(target) {
                let (x, y) = (object.x() - offset.x, object.y() - offset.y);
                store.with_gear_lock(target, |store| store.set_xy(target, x, y));
            }
        }
    }

    fn clear_runtime(&mut self, store: &mut ObjectStore) {
        for handle in self.runtime.nested.drain(..) {
            if store.nested_playing(&handle) {
                store.stop_nested_transition(handle.id, false);
            }
        }
        self.restore_offsets(store);
        for (target, token) in self.runtime.locks.drain(..) {
            store.release_display_lock(target, token);
        }
        self.runtime = PlaybackState::default();
    }

    fn stop_nested_plans(&mut self, store: &mut ObjectStore, complete: bool) {
        for plan in &self.runtime.plans {
            store.stop_nested_transition(plan.id, complete);
        }
    }
}

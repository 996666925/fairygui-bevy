use super::*;
use crate::transition::{Transition, TransitionPlayOptions};
use std::collections::{HashMap, VecDeque};

#[cfg(test)]
#[path = "transition_edit_tests.rs"]
mod edit_tests;
#[path = "transition_edits.rs"]
mod editing;
#[path = "transition_stage.rs"]
mod stage;
#[cfg(test)]
#[path = "transition_stage_tests.rs"]
mod stage_tests;

#[cfg(test)]
#[path = "transition_control_tests.rs"]
mod control_tests;
#[cfg(test)]
#[path = "transition_interval_tests.rs"]
mod interval_tests;
#[cfg(test)]
#[path = "nested_transition_tests.rs"]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct TransitionId {
    pub owner: GObjectId,
    pub index: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct NestedHandle {
    pub id: TransitionId,
    pub generation: u64,
    pub completed: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

/// One graph for the duration of a playback operation. Removing only the active
/// animation permits same-owner calls without aliasing the object store.
#[derive(Debug, Default)]
pub(super) struct TransitionDriver {
    bank: HashMap<TransitionId, Transition>,
    names: HashMap<(GObjectId, String), TransitionId>,
    owners: Vec<(GObjectId, usize)>,
    order: Vec<TransitionId>,
    stack: Vec<TransitionId>,
    clocks: HashMap<TransitionId, f32>,
    relation_updates: HashMap<TransitionId, Vec<(GObjectId, f32, f32)>>,
    stage_updates: HashMap<TransitionId, Vec<bool>>,
}

impl ObjectStore {
    fn with_transition_driver<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let outer = self.transition_driver.is_none();
        if outer {
            let mut driver = TransitionDriver::default();
            for (owner, object) in &mut self.objects {
                let Some(component) = object.control.component_mut() else {
                    continue;
                };
                if component.transitions.is_empty() {
                    continue;
                }
                let transitions = std::mem::take(&mut component.transitions);
                driver.owners.push((owner, transitions.len()));
                for (index, mut transition) in transitions.into_iter().enumerate() {
                    transition.begin_frame();
                    let id = TransitionId { owner, index };
                    driver
                        .names
                        .entry((owner, transition.name.clone()))
                        .or_insert(id);
                    driver.order.push(id);
                    driver.bank.insert(id, transition);
                }
            }
            self.transition_driver = Some(driver);
        }
        let result = f(self);
        if outer {
            loop {
                let updates =
                    std::mem::take(&mut self.transition_driver.as_mut().unwrap().stage_updates);
                for (id, updates) in updates {
                    for added in updates {
                        self.dispatch_stage_for_id(id, added);
                    }
                }
                if let Some(id) = self
                    .transition_driver
                    .as_ref()
                    .unwrap()
                    .order
                    .iter()
                    .copied()
                    .find(|id| {
                        self.transition_driver
                            .as_ref()
                            .unwrap()
                            .bank
                            .get(id)
                            .is_some_and(|transition| {
                                transition.has_nested_notification() && !transition.yielded()
                            })
                    })
                {
                    let clock = self
                        .transition_driver
                        .as_ref()
                        .unwrap()
                        .clocks
                        .get(&id)
                        .copied()
                        .unwrap_or(0.0);
                    self.advance_transition_to(id, clock);
                } else if self
                    .transition_driver
                    .as_ref()
                    .unwrap()
                    .stage_updates
                    .is_empty()
                {
                    break;
                }
            }
            let mut driver = self.transition_driver.take().unwrap();
            for (id, updates) in driver.relation_updates.drain() {
                if let Some(transition) = driver.bank.get_mut(&id) {
                    for (target, dx, dy) in updates {
                        transition.update_from_relations(self, id.owner, target, dx, dy);
                    }
                }
            }
            for (owner, count) in driver.owners {
                if let Some(component) = self
                    .objects
                    .get_mut(owner)
                    .and_then(|o| o.control.component_mut())
                {
                    component.transitions = (0..count)
                        .filter_map(|index| driver.bank.remove(&TransitionId { owner, index }))
                        .collect();
                }
            }
        }
        result
    }

    pub(crate) fn find_transition_id(&self, owner: GObjectId, name: &str) -> Option<TransitionId> {
        if let Some(driver) = &self.transition_driver {
            driver.names.get(&(owner, name.to_string())).copied()
        } else {
            self.objects
                .get(owner)?
                .control
                .component()?
                .transitions
                .iter()
                .position(|transition| transition.name == name)
                .map(|index| TransitionId { owner, index })
        }
    }

    fn driven_transition(&self, id: TransitionId) -> Option<&Transition> {
        if let Some(driver) = &self.transition_driver {
            driver.bank.get(&id)
        } else {
            self.objects
                .get(id.owner)?
                .control
                .component()?
                .transitions
                .get(id.index)
        }
    }

    pub(crate) fn current_transition_clock(&self) -> f32 {
        self.transition_driver
            .as_ref()
            .and_then(|driver| driver.stack.last().and_then(|id| driver.clocks.get(id)))
            .copied()
            .unwrap_or(0.0)
    }

    pub(crate) fn elapse_transition_clock(&mut self, dt: f32) {
        if let Some(driver) = &mut self.transition_driver
            && let Some(id) = driver.stack.last()
        {
            *driver.clocks.entry(*id).or_default() += dt;
        }
    }

    pub(crate) fn nested_allowed(&self, id: TransitionId) -> bool {
        self.transition_driver
            .as_ref()
            .is_none_or(|driver| !driver.stack.contains(&id))
    }

    fn advance_transition_to(&mut self, id: TransitionId, clock: f32) {
        let driver = self.transition_driver.as_mut().unwrap();
        if driver.stack.contains(&id) {
            return;
        }
        let from = driver.clocks.get(&id).copied().unwrap_or(0.0);
        if clock < from {
            return;
        }
        let Some(mut transition) = driver.bank.remove(&id) else {
            return;
        };
        if transition.yielded() {
            driver.bank.insert(id, transition);
            driver.clocks.insert(id, clock);
            return;
        }
        driver.stack.push(id);
        driver.clocks.insert(id, from);
        transition.advance_driven(self, id.owner, clock - from);
        let driver = self.transition_driver.as_mut().unwrap();
        driver.clocks.insert(id, clock);
        driver.stack.pop();
        driver.bank.insert(id, transition);
    }

    fn play_transition_id(
        &mut self,
        id: TransitionId,
        options: TransitionPlayOptions,
        clock: f32,
        inherited_scale: Option<f32>,
    ) -> Option<NestedHandle> {
        self.advance_transition_to(id, clock);
        let driver = self.transition_driver.as_mut().unwrap();
        if driver.stack.contains(&id) {
            return None;
        }
        let mut transition = driver.bank.remove(&id)?;
        driver.stack.push(id);
        driver.clocks.insert(id, clock);
        if let Some(scale) = inherited_scale {
            transition.set_time_scale(self, id.owner, scale);
        }
        transition.stop_for_replay(self, id.owner);
        transition.play_with_options(options);
        let handle = NestedHandle {
            id,
            generation: transition.play_generation(),
            completed: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };
        transition.set_completion_token(handle.completed.clone());
        transition.advance_driven(self, id.owner, 0.0);
        let driver = self.transition_driver.as_mut().unwrap();
        driver.stack.pop();
        driver.bank.insert(id, transition);
        Some(handle)
    }

    pub(crate) fn play_nested_transition(
        &mut self,
        id: TransitionId,
        options: TransitionPlayOptions,
        time_scale: f32,
    ) -> Option<NestedHandle> {
        let clock = self.current_transition_clock();
        self.play_transition_id(id, options, clock, Some(time_scale))
    }

    pub(crate) fn nested_playing(&self, handle: &NestedHandle) -> bool {
        self.driven_transition(handle.id).is_some_and(|transition| {
            transition.is_playing() && transition.play_generation() == handle.generation
        })
    }

    pub(crate) fn nested_next_step(&self, handle: &NestedHandle) -> f32 {
        self.driven_transition(handle.id)
            .filter(|transition| transition.play_generation() == handle.generation)
            .map_or(f32::INFINITY, |transition| transition.next_step(self))
    }

    pub(crate) fn advance_nested_transition(&mut self, handle: &NestedHandle, clock: f32) {
        if self.nested_playing(handle) {
            self.advance_transition_to(handle.id, clock);
        }
    }

    pub(crate) fn stop_nested_transition(&mut self, id: TransitionId, complete: bool) {
        self.with_transition_driver(|store| {
            let driver = store.transition_driver.as_mut().unwrap();
            if driver.stack.contains(&id) {
                return;
            }
            let Some(mut transition) = driver.bank.remove(&id) else {
                return;
            };
            driver.stack.push(id);
            if complete {
                transition.stop_with_completion(store, id.owner);
            } else {
                transition.stop_and_restore(store);
            }
            let driver = store.transition_driver.as_mut().unwrap();
            driver.stack.pop();
            driver.bank.insert(id, transition);
        });
    }

    pub(crate) fn advance_standalone_transition(
        &mut self,
        transition: &mut Transition,
        owner: GObjectId,
        dt: f32,
    ) -> bool {
        self.with_transition_driver(|store| {
            let id = TransitionId {
                owner,
                index: usize::MAX,
            };
            transition.begin_frame();
            let driver = store.transition_driver.as_mut().unwrap();
            let previous = driver.names.insert((owner, transition.name.clone()), id);
            driver.stack.push(id);
            let playing = transition.advance_driven(store, owner, dt);
            transition.apply_pending_relations(store, owner);
            let driver = store.transition_driver.as_mut().unwrap();
            driver.stack.pop();
            driver.names.remove(&(owner, transition.name.clone()));
            if let Some(previous) = previous {
                driver
                    .names
                    .insert((owner, transition.name.clone()), previous);
            }
            playing
        })
    }

    /// Advance potential parents before independent children. Per-transition
    /// clocks then consume only the remaining portion of this frame.
    fn transition_drive_order(&self) -> Vec<TransitionId> {
        let driver = self.transition_driver.as_ref().unwrap();
        let mut edges: HashMap<TransitionId, Vec<TransitionId>> = HashMap::new();
        let mut incoming: HashMap<_, usize> = driver.order.iter().map(|id| (*id, 0)).collect();
        for id in &driver.order {
            let children = edges.entry(*id).or_default();
            for (_, owner, name, _) in driver.bank[id].nested_calls(self, id.owner) {
                if let Some(child) = self.find_transition_id(owner, &name)
                    && child != *id
                    && !children.contains(&child)
                {
                    children.push(child);
                    *incoming.entry(child).or_default() += 1;
                }
            }
            for child in driver.bank[id].prepared_nested_ids() {
                if child != *id && !children.contains(&child) {
                    children.push(child);
                    *incoming.entry(child).or_default() += 1;
                }
            }
        }
        let mut ready: VecDeque<_> = driver
            .order
            .iter()
            .copied()
            .filter(|id| incoming[id] == 0)
            .collect();
        let mut order = Vec::new();
        while let Some(id) = ready.pop_front() {
            order.push(id);
            for child in edges.get(&id).into_iter().flatten() {
                let count = incoming.get_mut(child).unwrap();
                *count -= 1;
                if *count == 0 {
                    ready.push_back(*child);
                }
            }
        }
        // Cyclic definitions are handled by the active-call guard, so unrelated
        // properties still run and malformed graphs cannot recurse forever.
        for id in &driver.order {
            if !order.contains(id) {
                order.push(*id);
            }
        }
        order
    }

    /// 每帧推进全部过渡。同一帧嵌套调用与独立播放共用时间，不重复推进。
    pub fn advance_transitions(&mut self, dt: f32) {
        if !dt.is_finite() || dt < 0.0 {
            return;
        }
        self.with_transition_driver(|store| {
            let order = store.transition_drive_order();
            let mut clock = 0.0;
            loop {
                for id in &order {
                    store.advance_transition_to(*id, clock);
                }
                if clock >= dt {
                    break;
                }
                let step = store
                    .transition_driver
                    .as_ref()
                    .unwrap()
                    .bank
                    .values()
                    .map(|transition| transition.next_step(store))
                    .filter(|step| *step > 0.0)
                    .fold(dt - clock, f32::min);
                let next = (clock + step).min(dt);
                if next <= clock {
                    break;
                }
                for id in &order {
                    store.advance_transition_to(*id, next);
                }
                clock = next;
            }
        });
    }

    pub fn start_auto_transitions(&mut self) {
        self.start_auto_transitions_for(None);
    }

    /// 新增界面时只启动该子树的自动过渡，避免重启其他已存在的界面。
    pub fn start_auto_transitions_in(&mut self, owner: GObjectId) {
        self.start_auto_transitions_for(Some(owner));
    }

    fn start_auto_transitions_for(&mut self, owner: Option<GObjectId>) {
        self.with_transition_driver(|store| {
            for id in store.transition_drive_order() {
                if owner
                    .is_some_and(|owner| id.owner != owner && !store.is_ancestor(id.owner, owner))
                {
                    continue;
                }
                let transition = &store.transition_driver.as_ref().unwrap().bank[&id];
                if transition.auto_play {
                    let (times, delay) = (transition.auto_play_times, transition.auto_play_delay);
                    store.play_transition_id(
                        id,
                        TransitionPlayOptions {
                            times,
                            delay,
                            ..Default::default()
                        },
                        0.0,
                        None,
                    );
                }
            }
        });
    }

    pub fn play_transition(&mut self, owner: GObjectId, name: &str, times: i32) -> bool {
        self.play_transition_delayed(owner, name, times, 0.0)
    }

    pub fn play_transition_delayed(
        &mut self,
        owner: GObjectId,
        name: &str,
        times: i32,
        delay: f32,
    ) -> bool {
        self.play_transition_with_options(
            owner,
            name,
            TransitionPlayOptions {
                times,
                delay: delay.max(0.0),
                ..Default::default()
            },
        )
    }

    /// 按指定时间区间播放；起始时刻直接重建属性，历史音效和起始标签不再触发。
    pub fn play_transition_with_options(
        &mut self,
        owner: GObjectId,
        name: &str,
        options: TransitionPlayOptions,
    ) -> bool {
        if !options.is_valid() {
            return false;
        }
        self.with_transition_driver(|store| {
            let Some(id) = store.find_transition_id(owner, name) else {
                return false;
            };
            let clock = store.current_transition_clock();
            store.play_transition_id(id, options, clock, None).is_some()
        })
    }

    pub fn stop_transition(&mut self, owner: GObjectId, name: &str) -> bool {
        self.stop_transition_with_completion(owner, name, true)
    }

    /// 官方 playReverse 的一次反向播放，保留该过渡的 timeScale。
    pub fn play_transition_reverse(&mut self, owner: GObjectId, name: &str, delay: f32) -> bool {
        self.play_transition_with_options(
            owner,
            name,
            TransitionPlayOptions {
                reversed: true,
                delay: delay.max(0.0),
                ..Default::default()
            },
        )
    }

    fn edit_transition<R>(
        &mut self,
        id: TransitionId,
        f: impl FnOnce(&mut Transition, &mut Self) -> R,
    ) -> Option<R> {
        self.with_transition_driver(|store| {
            let driver = store.transition_driver.as_mut().unwrap();
            if driver.stack.contains(&id) {
                return None;
            }
            let mut transition = driver.bank.remove(&id)?;
            driver.stack.push(id);
            let result = f(&mut transition, store);
            let driver = store.transition_driver.as_mut().unwrap();
            driver.stack.pop();
            driver.bank.insert(id, transition);
            Some(result)
        })
    }

    pub(crate) fn pause_nested_transition(&mut self, id: TransitionId, paused: bool) -> bool {
        self.edit_transition(id, |transition, store| {
            transition.set_paused(store, id.owner, paused)
        })
        .unwrap_or(false)
    }
    pub(crate) fn scale_nested_transition(&mut self, id: TransitionId, scale: f32) -> bool {
        self.edit_transition(id, |transition, store| {
            transition.set_time_scale(store, id.owner, scale)
        })
        .unwrap_or(false)
    }

    pub fn set_transition_paused(&mut self, owner: GObjectId, name: &str, paused: bool) -> bool {
        let Some(id) = self.find_transition_id(owner, name) else {
            return false;
        };
        self.pause_nested_transition(id, paused)
    }
    pub fn set_transition_time_scale(&mut self, owner: GObjectId, name: &str, scale: f32) -> bool {
        let Some(id) = self.find_transition_id(owner, name) else {
            return false;
        };
        self.scale_nested_transition(id, scale)
    }
    pub fn transition_paused(&self, owner: GObjectId, name: &str) -> Option<bool> {
        Some(
            self.driven_transition(self.find_transition_id(owner, name)?)?
                .is_paused(),
        )
    }
    pub fn transition_time_scale(&self, owner: GObjectId, name: &str) -> Option<f32> {
        Some(
            self.driven_transition(self.find_transition_id(owner, name)?)?
                .time_scale(),
        )
    }
    pub fn transition_reversed(&self, owner: GObjectId, name: &str) -> Option<bool> {
        Some(
            self.driven_transition(self.find_transition_id(owner, name)?)?
                .is_reversed(),
        )
    }

    pub fn stop_transition_with_completion(
        &mut self,
        owner: GObjectId,
        name: &str,
        complete: bool,
    ) -> bool {
        self.stop_transition_with_options(owner, name, complete, false)
    }

    /// process_callback=true 同时发出该次播放的完成通知，允许等待它的父过渡结束。
    pub fn stop_transition_with_options(
        &mut self,
        owner: GObjectId,
        name: &str,
        complete: bool,
        process_callback: bool,
    ) -> bool {
        self.with_transition_driver(|store| {
            let Some(id) = store.find_transition_id(owner, name) else {
                return false;
            };
            let driver = store.transition_driver.as_mut().unwrap();
            if driver.stack.contains(&id) {
                return false;
            }
            let Some(mut transition) = driver.bank.remove(&id) else {
                return false;
            };
            driver.stack.push(id);
            transition.stop_with_callback(store, owner, complete, process_callback);
            let driver = store.transition_driver.as_mut().unwrap();
            driver.stack.pop();
            driver.bank.insert(id, transition);
            true
        })
    }

    pub(super) fn change_transition_play_times(
        &mut self,
        owner: GObjectId,
        name: &str,
        times: i32,
    ) -> bool {
        self.with_transition_driver(|store| {
            let Some(id) = store.find_transition_id(owner, name) else {
                return false;
            };
            let Some(transition) = store.transition_driver.as_mut().unwrap().bank.get_mut(&id)
            else {
                return false;
            };
            transition.change_play_times(times);
            true
        })
    }

    pub fn seek_transition(&mut self, owner: GObjectId, name: &str, label: &str) -> bool {
        self.with_transition_driver(|store| {
            let Some(id) = store.find_transition_id(owner, name) else {
                return false;
            };
            store
                .transition_driver
                .as_mut()
                .unwrap()
                .bank
                .get_mut(&id)
                .is_some_and(|transition| transition.seek_to_label(label))
        })
    }

    pub fn transition_state(&self, owner: GObjectId, name: &str) -> Option<(bool, f32)> {
        let transition = self.driven_transition(self.find_transition_id(owner, name)?)?;
        Some((transition.is_playing(), transition.time()))
    }

    pub fn transition_names(&self, owner: GObjectId) -> Vec<String> {
        if let Some(driver) = &self.transition_driver {
            driver
                .order
                .iter()
                .filter(|id| id.owner == owner)
                .filter_map(|id| {
                    driver
                        .bank
                        .get(id)
                        .map(|transition| transition.name.clone())
                })
                .collect()
        } else {
            self.objects
                .get(owner)
                .and_then(|object| object.control.component())
                .map(|component| {
                    component
                        .transitions
                        .iter()
                        .map(|transition| transition.name.clone())
                        .collect()
                })
                .unwrap_or_default()
        }
    }
}

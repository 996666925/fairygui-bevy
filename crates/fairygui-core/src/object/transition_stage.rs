use super::*;
use crate::transition::TransitionEditError;

impl ObjectStore {
    pub fn set_transition_auto_play(
        &mut self,
        owner: GObjectId,
        name: &str,
        enabled: bool,
    ) -> Result<bool, TransitionEditError> {
        self.set_transition_auto_play_with_options(owner, name, enabled, -1, 0.0)
    }

    /// 同一 enabled 值不更新次数/延迟；禁用时只在离开舞台的对象上停止并通知等待者。
    pub fn set_transition_auto_play_with_options(
        &mut self,
        owner: GObjectId,
        name: &str,
        enabled: bool,
        times: i32,
        delay: f32,
    ) -> Result<bool, TransitionEditError> {
        if !delay.is_finite() || delay < 0.0 {
            return Err(TransitionEditError::InvalidValue);
        }
        let id = self
            .find_transition_id(owner, name)
            .ok_or_else(|| TransitionEditError::TransitionNotFound(name.into()))?;
        self.with_transition_driver(|store| {
            let changed = store
                .edit_transition(id, |transition, _| {
                    if transition.auto_play == enabled {
                        return false;
                    }
                    transition.auto_play = enabled;
                    transition.auto_play_times = times;
                    transition.auto_play_delay = delay;
                    true
                })
                .ok_or(TransitionEditError::TransitionBusy)?;
            if changed {
                if enabled && store.is_on_stage(owner) {
                    store.play_transition_id(
                        id,
                        TransitionPlayOptions {
                            times,
                            delay,
                            ..Default::default()
                        },
                        store.current_transition_clock(),
                        None,
                    );
                } else if !enabled && !store.is_on_stage(owner) {
                    store.stop_transition_with_options(owner, name, false, true);
                }
            }
            Ok(changed)
        })
    }

    pub(crate) fn dispatch_transition_stage(&mut self, owner: GObjectId, added: bool) {
        self.with_transition_driver(|store| {
            let ids: Vec<_> = store
                .transition_driver
                .as_ref()
                .unwrap()
                .order
                .iter()
                .copied()
                .filter(|id| id.owner == owner)
                .collect();
            for id in ids {
                if store
                    .transition_driver
                    .as_ref()
                    .unwrap()
                    .stack
                    .contains(&id)
                {
                    store
                        .transition_driver
                        .as_mut()
                        .unwrap()
                        .stage_updates
                        .entry(id)
                        .or_default()
                        .push(added);
                } else {
                    store.dispatch_stage_for_id(id, added);
                }
            }
        });
    }

    pub(super) fn dispatch_stage_for_id(&mut self, id: TransitionId, added: bool) {
        let Some(transition) = self.driven_transition(id) else {
            return;
        };
        if added {
            if transition.auto_play && !transition.is_playing() {
                let options = TransitionPlayOptions {
                    times: transition.auto_play_times,
                    delay: transition.auto_play_delay,
                    ..Default::default()
                };
                self.play_transition_id(id, options, self.current_transition_clock(), None);
            }
        } else if transition.options & 2 == 0 {
            let complete = transition.options & 4 != 0;
            let name = transition.name.clone();
            self.stop_transition_with_options(id.owner, &name, complete, false);
        }
    }

    pub(crate) fn take_transition_stage_updates(&mut self) -> Vec<bool> {
        let Some(driver) = &mut self.transition_driver else {
            return Vec::new();
        };
        let Some(id) = driver.stack.last() else {
            return Vec::new();
        };
        driver.stage_updates.remove(id).unwrap_or_default()
    }

    pub(crate) fn dispose_transitions_in(&mut self, owner: GObjectId) {
        self.with_transition_driver(|store| {
            let ids: Vec<_> = store
                .transition_driver
                .as_ref()
                .unwrap()
                .order
                .iter()
                .copied()
                .filter(|id| id.owner == owner || store.is_ancestor(id.owner, owner))
                .collect();
            for id in ids {
                store.stop_nested_transition(id, false);
            }
        });
    }
}

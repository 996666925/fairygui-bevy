use super::*;
use crate::transition::{
    TransitionCallback, TransitionEditError, TransitionEditValue, TransitionListenerId,
};

impl ObjectStore {
    fn edit_named_transition<R>(
        &mut self,
        owner: GObjectId,
        name: &str,
        f: impl FnOnce(&mut Transition, &mut Self) -> Result<R, TransitionEditError>,
    ) -> Result<R, TransitionEditError> {
        let id = self
            .find_transition_id(owner, name)
            .ok_or_else(|| TransitionEditError::TransitionNotFound(name.into()))?;
        self.edit_transition(id, f)
            .ok_or(TransitionEditError::TransitionBusy)?
    }

    pub fn set_transition_value(
        &mut self,
        owner: GObjectId,
        name: &str,
        label: &str,
        value: TransitionEditValue,
    ) -> Result<usize, TransitionEditError> {
        self.edit_named_transition(owner, name, |transition, _| {
            transition.set_value(label, value)
        })
    }

    pub fn set_transition_duration(
        &mut self,
        owner: GObjectId,
        name: &str,
        label: &str,
        duration: f32,
    ) -> Result<usize, TransitionEditError> {
        self.edit_named_transition(owner, name, |transition, _| {
            transition.set_duration(label, duration)
        })
    }

    pub fn set_transition_target(
        &mut self,
        owner: GObjectId,
        name: &str,
        label: &str,
        target: Option<GObjectId>,
    ) -> Result<usize, TransitionEditError> {
        self.edit_named_transition(owner, name, |transition, store| {
            transition.set_target(store, owner, label, target)
        })
    }

    pub fn set_transition_hook(
        &mut self,
        owner: GObjectId,
        name: &str,
        label: &str,
        listener: Option<TransitionListenerId>,
    ) -> Result<(), TransitionEditError> {
        self.edit_named_transition(owner, name, |transition, _| {
            transition.set_hook(label, listener)
        })
    }

    pub fn clear_transition_hooks(
        &mut self,
        owner: GObjectId,
        name: &str,
    ) -> Result<(), TransitionEditError> {
        self.edit_named_transition(owner, name, |transition, _| {
            transition.clear_hooks();
            Ok(())
        })
    }

    pub fn transition_label_time(&self, owner: GObjectId, name: &str, label: &str) -> Option<f32> {
        self.driven_transition(self.find_transition_id(owner, name)?)?
            .label_time(label)
    }

    pub(crate) fn queue_transition_callback(&mut self, callback: TransitionCallback) {
        self.queued_transition_callbacks.push(callback);
    }

    pub fn take_transition_callbacks(&mut self) -> Vec<TransitionCallback> {
        std::mem::take(&mut self.queued_transition_callbacks)
    }

    pub(crate) fn update_transition_relations(
        &mut self,
        owner: GObjectId,
        target: GObjectId,
        dx: f32,
        dy: f32,
    ) {
        self.with_transition_driver(|store| {
            let driver = store.transition_driver.as_ref().unwrap();
            let mut ids: Vec<_> = driver
                .order
                .iter()
                .chain(&driver.stack)
                .copied()
                .filter(|id| id.owner == owner)
                .collect();
            ids.sort_by_key(|id| id.index);
            ids.dedup();
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
                        .relation_updates
                        .entry(id)
                        .or_default()
                        .push((target, dx, dy));
                } else {
                    store.edit_transition(id, |transition, store| {
                        transition.update_from_relations(store, owner, target, dx, dy)
                    });
                }
            }
        });
    }

    pub(crate) fn take_transition_relation_updates(&mut self) -> Vec<(GObjectId, f32, f32)> {
        let Some(driver) = &mut self.transition_driver else {
            return Vec::new();
        };
        let Some(id) = driver.stack.last() else {
            return Vec::new();
        };
        driver.relation_updates.remove(id).unwrap_or_default()
    }
}

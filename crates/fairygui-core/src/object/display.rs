use super::*;
use crate::object::object::DisplayGearState;
use crate::widget::GearKind;

/// A display lock belongs to one object and one application of its display gear.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayLockToken {
    owner: GObjectId,
    generation: u64,
}

impl ObjectStore {
    /// Local drawing state including FairyGUI groups (groups are siblings,
    /// not ancestors in the rendered entity tree).
    pub fn display_visible(&self, owner: GObjectId) -> bool {
        let Some(object) = self.objects.get(owner) else {
            return false;
        };
        if !object.display_visible() || !self.refresh_overlay_attached(owner) {
            return false;
        }
        if object.group.is_none() {
            return true;
        }
        let mut current = Some(owner);
        let mut visited = HashSet::new();
        while let Some(id) = current {
            if !visited.insert(id) {
                return false;
            }
            let Some(object) = self.objects.get(id) else {
                return false;
            };
            if !object.display_visible() {
                return false;
            }
            current = object.group;
        }
        true
    }

    pub(crate) fn touch_display(&mut self, owner: GObjectId) {
        let mut pending = vec![owner];
        let mut visited = HashSet::new();
        while let Some(id) = pending.pop() {
            if !visited.insert(id) {
                continue;
            }
            self.touch(id, dirty::LOOK);
            if !self
                .objects
                .get(id)
                .is_some_and(|object| object.kind == ObjectType::Group)
            {
                continue;
            }
            pending.extend(
                self.objects
                    .iter()
                    .filter_map(|(member, object)| (object.group == Some(id)).then_some(member)),
            );
        }
        self.refresh_stage();
    }

    pub(crate) fn apply_display_gear(
        &mut self,
        owner: GObjectId,
        kind: GearKind,
        controller_index: i32,
        visible: bool,
        condition: i32,
    ) {
        let Some(object) = self.objects.get_mut(owner) else {
            return;
        };
        match kind {
            GearKind::Display => {
                let generation = object
                    .display_gear
                    .as_ref()
                    .map_or(1, |gear| gear.generation.wrapping_add(1));
                object.display_gear = Some(DisplayGearState {
                    controller_index,
                    generation,
                    visible,
                    locks: 0,
                });
            }
            GearKind::Display2 => object.display_gear2 = Some((visible, condition)),
            _ => return,
        }
        self.touch_display(owner);
    }

    /// Keep the primary display gear connected until this lock is released.
    /// Explicit visibility and Display2 still apply. Objects without a bound
    /// primary display gear do not need a lock.
    pub fn add_display_lock(&mut self, owner: GObjectId) -> Option<DisplayLockToken> {
        let gear = self.objects.get_mut(owner)?.display_gear.as_mut()?;
        if gear.controller_index < 0 {
            return None;
        }
        gear.locks += 1;
        let token = DisplayLockToken {
            owner,
            generation: gear.generation,
        };
        self.touch_display(owner);
        Some(token)
    }

    /// Old tokens cannot disconnect a newly applied display gear.
    pub fn release_display_lock(&mut self, owner: GObjectId, token: DisplayLockToken) -> bool {
        let Some(gear) = self
            .objects
            .get_mut(owner)
            .and_then(|object| object.display_gear.as_mut())
        else {
            return false;
        };
        if token.owner != owner || token.generation != gear.generation || gear.locks == 0 {
            return false;
        }
        gear.locks -= 1;
        self.touch_display(owner);
        true
    }

    pub(crate) fn lock_gear_display(
        &mut self,
        owner: GObjectId,
        controller_index: i32,
    ) -> Option<DisplayLockToken> {
        let gear = self.objects.get(owner)?.display_gear.as_ref()?;
        if gear.controller_index != controller_index {
            return None;
        }
        self.add_display_lock(owner)
    }
}

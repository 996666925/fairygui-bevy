//! Sprite mask/cache side effects, following GComponent.setMask.
use super::*;
use crate::widget::{ControlData, HitTestArea};

#[cfg(test)]
#[path = "mask_tests.rs"]
mod tests;

impl ObjectStore {
    /// Current mask reference and reverse flag, independent of child ordering.
    pub fn component_mask(&self, owner: GObjectId) -> Option<(GObjectId, bool)> {
        let object = self.object(owner)?;
        let data = object.control().component()?;
        Some((object.mask?, data.mask_reversed))
    }

    /// Set an ordinary/reversed Sprite mask. Self/ancestor masks and missing
    /// handles are rejected before changing state. Reordering or detaching the
    /// referenced child does not silently rebind the mask to another child.
    pub fn set_component_mask(
        &mut self,
        owner: GObjectId,
        mask: Option<GObjectId>,
        reversed: bool,
    ) -> bool {
        let Some(data) = self.object(owner).and_then(|o| o.control().component()) else {
            return false;
        };
        if let Some(mask) = mask
            && (mask == owner || !self.contains(mask) || self.is_ancestor(owner, mask))
        {
            return false;
        }
        let old = self.object(owner).unwrap().mask;
        let old_normal = old.filter(|_| !data.mask_reversed);
        if old != mask
            && let Some(old) = old
            && self
                .object(old)
                .is_some_and(|o| o.blend_mode() == BlendMode::Erase)
        {
            self.reset_blend_mode(old);
        }
        let normal = mask.filter(|_| !reversed);
        let same_normal = normal.is_some()
            && normal == old_normal
            && self
                .object(normal.unwrap())
                .is_some_and(|o| o.mask_parent == Some(owner));
        if !same_normal {
            if let Some(old) = old_normal {
                if let Some(object) = self.objects.get_mut(old) {
                    object.mask_parent = None;
                }
                self.set_cache_as_bitmap(old, false);
            }
            if let Some(mask) = normal {
                self.objects.get_mut(mask).unwrap().mask_parent = Some(owner);
                self.set_cache_as_bitmap(mask, true);
            }
        }
        let has_hit = mask.and_then(|id| self.object(id)).is_some_and(|o| {
            matches!(o.control(), ControlData::Graph(_))
                || o.control()
                    .component()
                    .is_some_and(|c| c.hit_test.is_some())
        });
        self.objects.get_mut(owner).unwrap().mask = mask;
        let data = self
            .objects
            .get_mut(owner)
            .unwrap()
            .control
            .component_mut()
            .unwrap();
        data.mask_reversed = reversed;
        if let Some(mask) = mask {
            if has_hit {
                data.hit_test = Some(HitTestArea::MaskChild {
                    object: mask,
                    reversed,
                });
            }
        } else if matches!(
            data.hit_test,
            Some(HitTestArea::Child { .. } | HitTestArea::MaskChild { .. })
        ) {
            data.hit_test = None;
        }
        if reversed && let Some(mask) = mask {
            self.set_cache_as_bitmap(owner, true);
            self.set_blend_mode(mask, BlendMode::Erase);
        }
        self.touch(owner, dirty::LOOK);
        true
    }

    /// Native bitmap isolation. Clearing a reversed mask deliberately leaves
    /// this independent Sprite property enabled.
    pub fn set_cache_as_bitmap(&mut self, id: GObjectId, value: bool) {
        if let Some(object) = self.objects.get_mut(id)
            && object.cache_as_bitmap != value
        {
            object.cache_as_bitmap = value;
            self.touch(id, dirty::LOOK);
        }
    }
}

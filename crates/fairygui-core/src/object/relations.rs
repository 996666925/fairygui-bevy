use super::*;
use crate::widget::relation::{RelationBinding, RelationDef, RelationItem};

/// FairyGUI relation between an owner's edge/size and a target's edge/size.
/// Adding a relation preserves the current offset; it does not snap either object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RelationType {
    LeftLeft = 0,
    LeftCenter = 1,
    LeftRight = 2,
    CenterCenter = 3,
    RightLeft = 4,
    RightCenter = 5,
    RightRight = 6,
    TopTop = 7,
    TopMiddle = 8,
    TopBottom = 9,
    MiddleMiddle = 10,
    BottomTop = 11,
    BottomMiddle = 12,
    BottomBottom = 13,
    Width = 14,
    Height = 15,
    LeftExtLeft = 16,
    LeftExtRight = 17,
    RightExtLeft = 18,
    RightExtRight = 19,
    TopExtTop = 20,
    TopExtBottom = 21,
    BottomExtTop = 22,
    BottomExtBottom = 23,
    /// Adds/removes Width and Height together; presence requires both.
    Size = 24,
}

impl RelationType {
    fn codes(self) -> impl Iterator<Item = u8> {
        let first = if self == Self::Size { 14 } else { self as u8 };
        [Some(first), (self == Self::Size).then_some(15)]
            .into_iter()
            .flatten()
    }
}

impl ObjectStore {
    /// Retain a live target by identity. Duplicate types keep their original
    /// percent policy. Returns whether any definition was added; self/dead
    /// targets and missing owners return false without changing relations.
    pub fn add_relation(
        &mut self,
        owner: GObjectId,
        target: GObjectId,
        relation: RelationType,
        percent: bool,
    ) -> bool {
        if owner == target || !self.contains(owner) || !self.contains(target) {
            return false;
        }
        self.bind_relation_targets(owner);
        let parent = self.objects[owner].parent;
        let items = &mut self.objects[owner].relations.items;
        let index = if let Some(index) = items.iter().position(|item| {
            item.binding
                .is_some_and(|binding| binding.target == Some(target))
        }) {
            index
        } else {
            items.push(RelationItem {
                to_parent: parent == Some(target),
                to_child: false,
                target_index: None,
                binding: Some(RelationBinding {
                    target: Some(target),
                    track_position: parent != Some(target),
                }),
                defs: Vec::new(),
            });
            items.len() - 1
        };
        let mut added = false;
        for code in relation.codes() {
            if items.iter().any(|item| {
                item.binding
                    .is_some_and(|binding| binding.target == Some(target))
                    && item.defs.iter().any(|def| def.relation_type == code)
            }) {
                continue;
            }
            items[index].defs.push(RelationDef {
                relation_type: code,
                percent,
                axis: u8::from(!(code <= 6 || code == 14 || (16..=19).contains(&code))),
            });
            added = true;
        }
        if added {
            let owners = self.relation_dependents.entry(target).or_default();
            if !owners.contains(&owner) {
                owners.push(owner);
            }
        }
        added
    }

    /// Remove only the selected type(s). Other definitions keep their original
    /// target subscription. Returns whether any definition was removed.
    pub fn remove_relation(
        &mut self,
        owner: GObjectId,
        target: GObjectId,
        relation: RelationType,
    ) -> bool {
        self.bind_relation_targets(owner);
        let Some(object) = self.objects.get_mut(owner) else {
            return false;
        };
        let mut removed = false;
        object.relations.items.retain_mut(|item| {
            if !item
                .binding
                .is_some_and(|binding| binding.target == Some(target))
            {
                return true;
            }
            let before = item.defs.len();
            item.defs
                .retain(|def| !relation.codes().any(|code| code == def.relation_type));
            removed |= before != item.defs.len();
            !item.defs.is_empty()
        });
        if removed && !self.relations_target(owner, target) {
            self.unsubscribe_relation(owner, target);
        }
        removed
    }

    /// Clear all definitions for one target, including a previously disposed target.
    pub fn clear_relations_to(&mut self, owner: GObjectId, target: GObjectId) -> bool {
        self.bind_relation_targets(owner);
        let Some(object) = self.objects.get_mut(owner) else {
            return false;
        };
        let before = object.relations.items.len();
        object.relations.items.retain(|item| {
            !item
                .binding
                .is_some_and(|binding| binding.target == Some(target))
        });
        let removed = before != object.relations.items.len();
        if removed {
            self.unsubscribe_relation(owner, target);
        }
        removed
    }

    /// Clear exported and runtime definitions and unsubscribe from all targets.
    pub fn clear_relations(&mut self, owner: GObjectId) -> bool {
        let Some(object) = self.objects.get_mut(owner) else {
            return false;
        };
        let items = std::mem::take(&mut object.relations.items);
        let removed = !items.is_empty();
        for item in items {
            if let Some(target) = item.binding.and_then(|binding| binding.target) {
                self.unsubscribe_relation(owner, target);
            }
        }
        removed
    }

    /// Check a retained definition. Size is present only when both Width and
    /// Height are present. Resolves exported construction metadata if necessary.
    pub fn has_relation(
        &mut self,
        owner: GObjectId,
        target: GObjectId,
        relation: RelationType,
    ) -> bool {
        self.bind_relation_targets(owner);
        let Some(object) = self.objects.get(owner) else {
            return false;
        };
        relation.codes().all(|code| {
            object.relations.items.iter().any(|item| {
                item.binding
                    .is_some_and(|binding| binding.target == Some(target))
                    && item.defs.iter().any(|def| def.relation_type == code)
            })
        })
    }

    fn relations_target(&self, owner: GObjectId, target: GObjectId) -> bool {
        self.objects[owner].relations.items.iter().any(|item| {
            item.binding
                .is_some_and(|binding| binding.target == Some(target))
                && !item.defs.is_empty()
        })
    }

    fn unsubscribe_relation(&mut self, owner: GObjectId, target: GObjectId) {
        if let std::collections::hash_map::Entry::Occupied(mut entry) =
            self.relation_dependents.entry(target)
        {
            entry.get_mut().retain(|&id| id != owner);
            if entry.get().is_empty() {
                entry.remove();
            }
        }
    }
}

#[cfg(test)]
#[path = "relations_tests.rs"]
mod tests;

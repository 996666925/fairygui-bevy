use super::*;
use crate::widget::relation::RelationBinding;

impl ObjectStore {
    /// The export index is construction metadata. Once resolved, retain the
    /// object identity across reorders, reparenting, detach and target disposal.
    pub(crate) fn bind_exported_relation_targets(
        &mut self,
        owner: GObjectId,
        siblings: &[GObjectId],
        children: &[GObjectId],
    ) {
        let Some(object) = self.objects.get(owner) else {
            return;
        };
        let parent = object.parent;
        let bindings: Vec<_> = object
            .relations
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                if item.binding.is_some() {
                    return None;
                }
                if item.to_parent && parent.is_none() {
                    return None;
                }
                let target = if item.to_parent {
                    parent
                } else if item.to_child {
                    item.target_index
                        .and_then(|index| children.get(index))
                        .copied()
                } else {
                    item.target_index
                        .and_then(|index| siblings.get(index))
                        .copied()
                };
                Some((
                    index,
                    RelationBinding {
                        target,
                        track_position: target != parent,
                    },
                ))
            })
            .collect();
        for (index, binding) in bindings {
            self.objects[owner].relations.items[index].binding = Some(binding);
            if let Some(target) = binding.target {
                let owners = self.relation_dependents.entry(target).or_default();
                if !owners.contains(&owner) {
                    owners.push(owner);
                }
            }
        }
    }

    pub(super) fn bind_relation_targets(&mut self, owner: GObjectId) {
        let Some(object) = self.objects.get(owner) else {
            return;
        };
        if object.under_construct
            || object
                .parent
                .and_then(|id| self.objects.get(id))
                .is_some_and(|parent| parent.under_construct)
            || object
                .relations
                .items
                .iter()
                .all(|item| item.binding.is_some())
        {
            return;
        }
        let children = object.children.clone();
        let siblings = object
            .parent
            .and_then(|id| self.objects.get(id))
            .map(|o| o.children.clone())
            .unwrap_or_default();
        self.bind_exported_relation_targets(owner, &siblings, &children);
    }

    pub(super) fn bind_container_relations(&mut self, container: GObjectId) {
        self.bind_relation_targets(container);
        let children = self
            .objects
            .get(container)
            .map(|o| o.children.clone())
            .unwrap_or_default();
        for child in children {
            self.bind_relation_targets(child);
        }
    }

    pub(super) fn related_owners(&mut self, target: GObjectId) -> Vec<GObjectId> {
        // Resolve runtime-created local relations before the first notification.
        // Exported relations are already bound before controllers/layout run.
        self.bind_container_relations(target);
        if let Some(parent) = self.objects.get(target).and_then(|o| o.parent) {
            self.bind_container_relations(parent);
        }
        self.relation_dependents
            .get(&target)
            .cloned()
            .unwrap_or_default()
    }
}

#[cfg(test)]
#[path = "relation_targets_tests.rs"]
mod tests;

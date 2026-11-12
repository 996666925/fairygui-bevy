use super::*;

impl ObjectStore {
    /// Group members are siblings, never render-tree children of the group.
    pub(crate) fn group_members(&self, group: GObjectId) -> Vec<GObjectId> {
        let Some(object) = self.objects.get(group) else {
            return Vec::new();
        };
        if object.kind != ObjectType::Group {
            return Vec::new();
        }
        let Some(parent) = object.parent.and_then(|parent| self.objects.get(parent)) else {
            return Vec::new();
        };
        parent
            .children
            .iter()
            .copied()
            .filter(|id| {
                self.objects
                    .get(*id)
                    .is_some_and(|object| object.group == Some(group))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (ObjectStore, GObjectId, GObjectId, GObjectId) {
        let mut store = ObjectStore::new();
        let parent = store.create(ObjectType::Component);
        let group = store.create(ObjectType::Group);
        let member = store.create(ObjectType::Image);
        store.set_parent(group, Some(parent));
        store.set_parent(member, Some(parent));
        store.set_group(member, Some(group));
        store.set_xy(member, 100.0, 200.0);
        (store, parent, group, member)
    }

    #[test]
    fn moving_groups_moves_hidden_members_by_the_same_delta_and_marks_views_dirty() {
        let (mut store, parent, group, member) = fixture();
        let unrelated = store.create(ObjectType::Image);
        store.set_parent(unrelated, Some(parent));
        store.set_visible(member, false);
        store.take_dirty();
        store.set_xy(group, 50.0, -20.0);
        assert_eq!(store.position_of(member), Some(Point::new(150.0, 180.0)));
        assert_eq!(store.position_of(unrelated), Some(Point::ZERO));
        assert!(
            store
                .take_dirty()
                .iter()
                .any(|&(id, flags)| id == member && flags & dirty::XY != 0)
        );
        store.set_xy(group, 50.0, -20.0);
        assert_eq!(store.position_of(member), Some(Point::new(150.0, 180.0)));
    }

    #[test]
    fn nested_groups_move_members_once_and_reject_membership_cycles() {
        let (mut store, parent, outer, member) = fixture();
        let inner = store.create(ObjectType::Group);
        store.set_parent(inner, Some(parent));
        store.set_group(inner, Some(outer));
        store.set_group(member, Some(inner));
        store.set_group(outer, Some(inner));
        assert_eq!(store.object(outer).unwrap().group(), None);
        store.set_xy(outer, 20.0, 30.0);
        assert_eq!(store.position_of(inner), Some(Point::new(20.0, 30.0)));
        assert_eq!(store.position_of(member), Some(Point::new(120.0, 230.0)));
    }

    #[test]
    fn group_alpha_assigns_member_alpha_instead_of_multiplying_it_again() {
        let (mut store, parent, group, member) = fixture();
        let descendant = store.create(ObjectType::Image);
        store.set_parent(descendant, Some(member));
        store.set_alpha(parent, 0.5);
        store.set_alpha(member, 0.25);
        store.set_alpha(group, 0.5);
        assert_eq!(store.object(member).unwrap().alpha(), 0.5);
        assert_eq!(store.effective_alpha(member), 0.25);
        assert_eq!(store.effective_alpha(descendant), 0.25);
        store.set_alpha(member, 0.75);
        store.set_alpha(group, 0.5); // Unchanged group value must not overwrite later edits.
        assert_eq!(store.object(member).unwrap().alpha(), 0.75);
        store.set_alpha(group, 0.25);
        assert_eq!(store.object(member).unwrap().alpha(), 0.25);
    }

    #[test]
    fn reordering_preserves_group_membership_but_reparenting_clears_it() {
        let (mut store, parent, group, member) = fixture();
        store.add_child_at(parent, member, 0);
        assert_eq!(store.child_at(parent, 0), Some(member));
        assert_eq!(store.object(member).unwrap().group(), Some(group));
        store.add_child_at(parent, member, usize::MAX);
        assert_eq!(store.object(member).unwrap().group(), Some(group));
        let other = store.create(ObjectType::Component);
        store.set_parent(member, Some(other));
        assert_eq!(store.object(member).unwrap().group(), None);
        store.set_xy(group, 20.0, 30.0);
        assert_eq!(store.position_of(member), Some(Point::new(100.0, 200.0)));
    }
}

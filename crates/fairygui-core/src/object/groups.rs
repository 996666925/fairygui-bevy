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

    pub(super) fn invalidate_member_group(&mut self, member: GObjectId) {
        if let Some(group) = self.objects.get(member).and_then(|o| o.group)
            && !self.updating_groups.contains(&group)
        {
            self.pending_group_layouts.insert(group);
        }
    }

    pub(super) fn move_group_members(&mut self, group: GObjectId, delta: Point) {
        let members = self.group_members(group);
        if members.is_empty() || !self.moving_groups.insert(group) {
            return;
        }
        let was_updating = !self.updating_groups.insert(group);
        for member in members {
            if let Some(position) = self.position_of(member) {
                self.set_xy(member, position.x + delta.x, position.y + delta.y);
            }
        }
        self.moving_groups.remove(&group);
        if !was_updating {
            self.updating_groups.remove(&group);
        }
    }

    /// Finish groups affected by member position, size, visibility or membership
    /// changes before projecting geometry. Nested groups settle inside out.
    pub fn flush_group_layouts(&mut self) -> usize {
        let mut count = 0;
        while let Some(&group) = self.pending_group_layouts.iter().next() {
            self.layout_group(group);
            count += 1;
        }
        count
    }

    /// Apply all group layouts and refresh their member bounds.
    pub fn layout_all_groups(&mut self) -> usize {
        let groups: Vec<_> = self
            .objects
            .iter()
            .filter_map(|(id, o)| (o.kind == ObjectType::Group).then_some(id))
            .collect();
        groups
            .into_iter()
            .map(|group| self.layout_group(group))
            .sum()
    }

    /// Match GGroup.handleLayout: only write the layout axis. The perpendicular
    /// coordinates retain the editor's left/center/right or top/middle/bottom
    /// alignment, including offsets maintained by relations.
    /// Returns the number of members moved (bounds changes are not counted).
    pub fn layout_group(&mut self, group: GObjectId) -> usize {
        self.pending_group_layouts.remove(&group);
        let Some(object) = self.objects.get(group) else {
            return 0;
        };
        let crate::widget::ControlData::Group(data) = &object.control else {
            return 0;
        };
        if object.parent.is_none() || !self.updating_groups.insert(group) {
            return 0;
        }
        let (layout, gap, exclude, fixed_size) = (
            data.layout,
            if data.layout == 1 {
                data.column_gap
            } else {
                data.line_gap
            } as f32,
            data.exclude_invisibles,
            data.auto_size_disabled,
        );
        let members = self.group_members(group);
        for &member in &members {
            if self.pending_group_layouts.contains(&member) {
                self.layout_group(member);
            }
        }
        self.moving_groups.insert(group);
        let mut cursor = if layout == 1 {
            self.objects[group].x
        } else {
            self.objects[group].y
        };
        let mut applied = 0;
        for &member in &members {
            let object = &self.objects[member];
            if exclude && !object.display_visible() {
                continue;
            }
            let (mut x, mut y) = (object.x, object.y);
            let extent = if layout == 1 {
                object.width
            } else {
                object.height
            };
            match layout {
                1 => {
                    x = cursor
                        + if object.pivot_as_anchor {
                            object.width * object.pivot_x
                        } else {
                            0.
                        }
                }
                2 => {
                    y = cursor
                        + if object.pivot_as_anchor {
                            object.height * object.pivot_y
                        } else {
                            0.
                        }
                }
                _ => continue,
            }
            if (x, y) != (object.x, object.y) {
                self.set_xy(member, x, y);
                applied += 1;
            }
            if extent != 0. {
                cursor += extent + gap;
            }
        }
        if !fixed_size || layout == 0 {
            // Updating the group rectangle must notify relations, but must not
            // translate its members a second time (GGroup._updating bit 1).
            let mut bounds: Option<(f32, f32, f32, f32)> = None;
            for member in members {
                let object = &self.objects[member];
                if exclude && !object.display_visible() {
                    continue;
                }
                let x = object.x
                    - if object.pivot_as_anchor {
                        object.width * object.pivot_x
                    } else {
                        0.
                    };
                let y = object.y
                    - if object.pivot_as_anchor {
                        object.height * object.pivot_y
                    } else {
                        0.
                    };
                let right = x + object.width;
                let bottom = y + object.height;
                bounds = Some(match bounds {
                    Some((left, top, r, b)) => {
                        (left.min(x), top.min(y), r.max(right), b.max(bottom))
                    }
                    None => (x, y, right, bottom),
                });
            }
            if let Some((x, y, right, bottom)) = bounds {
                self.set_xy(group, x, y);
                self.moving_groups.remove(&group);
                self.set_size(group, right - x, bottom - y);
            } else {
                self.moving_groups.remove(&group);
                self.set_size(group, 0., 0.);
            }
        }
        self.moving_groups.remove(&group);
        self.updating_groups.remove(&group);
        applied
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn fixture() -> (ObjectStore, GObjectId, GObjectId, GObjectId) {
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

#[cfg(test)]
mod layout_tests {
    use super::*;
    use crate::widget::relation::{RelationDef, RelationItem};

    fn config(store: &mut ObjectStore, group: GObjectId, layout: i32, exclude: bool) {
        let crate::widget::ControlData::Group(data) = &mut store.object_mut(group).unwrap().control
        else {
            unreachable!()
        };
        data.layout = layout;
        data.column_gap = 7;
        data.line_gap = 9;
        data.exclude_invisibles = exclude;
    }

    #[test]
    fn alignment_pivots_zero_extents_and_dynamic_bounds_match_native_group_traces() {
        let mut store = ObjectStore::new();
        let mut group = None;
        let mut members = Vec::new();
        let mut states = 0;
        for line in include_str!("../../tests/fixtures/group-reference.txt").lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let v: Vec<f32> = line
                .split_whitespace()
                .skip(1)
                .map(|v| v.parse().unwrap())
                .collect();
            if line.starts_with("case ") {
                store = ObjectStore::new();
                members.clear();
                let parent = store.create(ObjectType::Component);
                let id = store.create(ObjectType::Group);
                store.set_parent(id, Some(parent));
                store.set_xy(id, 40., 60.);
                config(&mut store, id, v[0] as i32, v[3] == 1.);
                group = Some(id);
                for i in 0..3 {
                    let member = store.create(ObjectType::Component);
                    store.set_parent(member, Some(parent));
                    store.set_group(member, Some(id));
                    let zero = i == 1 && v[4] == 1.;
                    let width = if zero { 0. } else { 40. + i as f32 * 20. };
                    let height = if zero { 0. } else { 20. + i as f32 * 10. };
                    store.set_size(member, width, height);
                    store.set_pivot(member, 0.5, 0.5, v[2] == 1.);
                    let left = 40.
                        + if v[0] == 1. {
                            i as f32 * 100.
                        } else {
                            (120. - width) * v[1] / 2.
                        };
                    let top = 60.
                        + if v[0] == 1. {
                            (80. - height) * v[1] / 2.
                        } else {
                            i as f32 * 70.
                        };
                    store.set_xy(member, left + width * v[2] / 2., top + height * v[2] / 2.);
                    members.push(member);
                }
            } else {
                match v[0] as u8 {
                    1 => store.set_size(members[0], 75., 45.),
                    2 => store.set_visible(members[1], false),
                    _ => {}
                }
                store.flush_group_layouts();
                for (index, id) in std::iter::once(group.unwrap())
                    .chain(members.iter().copied())
                    .enumerate()
                {
                    let o = store.object(id).unwrap();
                    assert_eq!(
                        [o.x, o.y, o.width, o.height],
                        v[1 + index * 4..5 + index * 4],
                        "{line}, object {index}"
                    );
                }
                assert_eq!(
                    store.flush_group_layouts(),
                    0,
                    "layout must settle in the same batch"
                );
                states += 1;
            }
        }
        assert_eq!(states, 216);
    }

    fn relation(store: &mut ObjectStore, owner: GObjectId, target: Option<usize>, kind: u8) {
        store
            .object_mut(owner)
            .unwrap()
            .relations
            .items
            .push(RelationItem {
                to_parent: target.is_none(),
                to_child: false,
                target_index: target,
                defs: vec![RelationDef {
                    relation_type: kind,
                    percent: false,
                    axis: 0,
                }],
            });
    }

    #[test]
    fn centered_and_right_aligned_groups_move_members_when_content_width_changes() {
        for (kind, initial_x, expected_x) in [(3, 200., 175.), (6, 400., 350.)] {
            let mut store = ObjectStore::new();
            let parent = store.create(ObjectType::Component);
            store.set_size(parent, 500., 300.);
            let group = store.create(ObjectType::Group);
            store.set_parent(group, Some(parent));
            store.set_xy(group, initial_x, 30.);
            config(&mut store, group, 1, false);
            if let crate::widget::ControlData::Group(data) =
                &mut store.object_mut(group).unwrap().control
            {
                data.column_gap = 0;
            }
            let mut members = Vec::new();
            for i in 0..2 {
                let member = store.create(ObjectType::Component);
                store.set_parent(member, Some(parent));
                store.set_group(member, Some(group));
                store.set_size(member, 50., 20.);
                store.set_xy(member, initial_x + i as f32 * 50., 30. + i as f32 * 10.);
                members.push(member);
            }
            store.flush_group_layouts();
            relation(&mut store, group, None, kind);
            store.set_size(members[1], 100., 20.);
            store.flush_group_layouts();
            assert_eq!(store.position_of(group), Some(Point::new(expected_x, 30.)));
            assert_eq!(
                store.position_of(members[0]),
                Some(Point::new(expected_x, 30.))
            );
            assert_eq!(
                store.position_of(members[1]),
                Some(Point::new(expected_x + 50., 40.))
            );
            assert_eq!(store.object(group).unwrap().width(), 150.);
            assert_eq!(store.flush_group_layouts(), 0);
        }
    }

    #[test]
    fn moving_a_group_does_not_apply_member_position_relations_twice() {
        let (mut store, parent, group, first) = super::tests::fixture();
        let second = store.create(ObjectType::Component);
        store.set_parent(second, Some(parent));
        store.set_xy(second, 150., 210.);
        store.set_group(second, Some(group));
        let index = store.child_index(first);
        relation(&mut store, second, index, 0);
        store.set_xy(group, 20., 30.);
        assert_eq!(store.position_of(first), Some(Point::new(120., 230.)));
        assert_eq!(store.position_of(second), Some(Point::new(170., 240.)));
    }

    #[test]
    fn unarranged_nested_group_bounds_refresh_after_removing_the_last_member() {
        let (mut store, parent, outer, member) = super::tests::fixture();
        let inner = store.create(ObjectType::Group);
        store.set_parent(inner, Some(parent));
        store.set_group(inner, Some(outer));
        store.set_group(member, Some(inner));
        store.set_size(member, 80., 30.);
        store.flush_group_layouts();
        for id in [inner, outer] {
            assert_eq!(store.position_of(id), Some(Point::new(100., 200.)));
            assert_eq!(store.object(id).unwrap().size(), Size::new(80., 30.));
        }
        store.remove(member);
        store.flush_group_layouts();
        for id in [inner, outer] {
            assert_eq!(store.object(id).unwrap().size(), Size::ZERO);
        }
    }
}

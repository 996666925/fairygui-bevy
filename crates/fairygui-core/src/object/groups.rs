use super::*;

/// Native group layout. Cross-axis alignment is retained in member coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GroupLayoutType {
    #[default]
    None,
    Horizontal,
    Vertical,
}
impl GroupLayoutType {
    fn code(self) -> i32 {
        match self {
            Self::None => 0,
            Self::Horizontal => 1,
            Self::Vertical => 2,
        }
    }
}

/// Layout policies of a GGroup; members remain siblings in the display tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroupLayoutSettings {
    pub layout: GroupLayoutType,
    pub line_gap: i32,
    pub column_gap: i32,
    pub exclude_invisibles: bool,
    /// Keep the group's dimensions and allocate that space to its members.
    pub auto_size_disabled: bool,
    /// Ordinal among all group members, including excluded invisible members.
    pub main_grid_index: Option<usize>,
    pub main_grid_min_size: f32,
}
impl Default for GroupLayoutSettings {
    fn default() -> Self {
        Self {
            layout: GroupLayoutType::None,
            line_gap: 0,
            column_gap: 0,
            exclude_invisibles: false,
            auto_size_disabled: false,
            main_grid_index: None,
            main_grid_min_size: 50.,
        }
    }
}

#[derive(Debug)]
pub(super) struct GroupPercentages {
    members: Vec<(GObjectId, f64, bool)>,
    count: usize,
    total: f64,
    main: Option<GObjectId>,
}

impl ObjectStore {
    pub fn group_layout_settings(&self, group: GObjectId) -> Option<GroupLayoutSettings> {
        let crate::widget::ControlData::Group(data) = &self.objects.get(group)?.control else {
            return None;
        };
        Some(GroupLayoutSettings {
            layout: match data.layout {
                1 => GroupLayoutType::Horizontal,
                2 => GroupLayoutType::Vertical,
                _ => GroupLayoutType::None,
            },
            line_gap: data.line_gap,
            column_gap: data.column_gap,
            exclude_invisibles: data.exclude_invisibles,
            auto_size_disabled: data.auto_size_disabled,
            main_grid_index: usize::try_from(data.main_grid_index).ok(),
            main_grid_min_size: data.main_grid_min_size,
        })
    }

    /// Queue one layout for a batch of policy edits. Negative gaps are valid;
    /// the main-grid minimum must be finite and nonnegative.
    pub fn set_group_layout_settings(
        &mut self,
        group: GObjectId,
        next: GroupLayoutSettings,
    ) -> bool {
        let Some(old) = self.group_layout_settings(group) else {
            return false;
        };
        if old == next
            || !next.main_grid_min_size.is_finite()
            || next.main_grid_min_size < 0.
            || next
                .main_grid_index
                .is_some_and(|i| i32::try_from(i).is_err())
        {
            return false;
        }
        let crate::widget::ControlData::Group(data) = &mut self.objects[group].control else {
            unreachable!()
        };
        data.layout = next.layout.code();
        data.line_gap = next.line_gap;
        data.column_gap = next.column_gap;
        data.exclude_invisibles = next.exclude_invisibles;
        data.auto_size_disabled = next.auto_size_disabled;
        data.main_grid_index = next.main_grid_index.map_or(-1, |i| i as i32);
        data.main_grid_min_size = next.main_grid_min_size;
        let reset = old.layout != next.layout
            || old.exclude_invisibles != next.exclude_invisibles
            || old.main_grid_index != next.main_grid_index
            || old.main_grid_min_size != next.main_grid_min_size;
        self.invalidate_group(group, reset);
        true
    }

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
                    .is_some_and(|o| o.group == Some(group))
            })
            .collect()
    }

    fn invalidate_group(&mut self, group: GObjectId, reset_percentages: bool) {
        if !self.updating_groups.contains(&group) {
            self.pending_group_layouts.insert(group);
            if reset_percentages {
                self.group_percentages.remove(&group);
            }
        }
    }

    pub(super) fn invalidate_member_group(&mut self, member: GObjectId) {
        if let Some(group) = self.objects.get(member).and_then(|o| o.group) {
            self.invalidate_group(group, true);
        }
    }

    pub(super) fn invalidate_group_geometry(&mut self, member: GObjectId, flags: u32) {
        if flags & (dirty::XY | dirty::SIZE) != 0
            && let Some(group) = self.objects.get(member).and_then(|o| o.group)
        {
            // Position/appearance changes must not replace the original size
            // percentages with rounded allocations from an earlier resize.
            self.invalidate_group(group, flags & dirty::SIZE != 0);
        }
    }

    pub(super) fn invalidate_group_member_visibility(&mut self, member: GObjectId) {
        if let Some(group) = self.objects.get(member).and_then(|o| o.group)
            && self
                .group_layout_settings(group)
                .is_some_and(|s| s.exclude_invisibles)
        {
            self.invalidate_group(group, true);
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

    /// Finish affected groups before projecting geometry. Nested groups settle
    /// inside out, including groups without an automatic arrangement.
    pub fn flush_group_layouts(&mut self) -> usize {
        let mut count = 0;
        while let Some(&group) = self.pending_group_layouts.iter().next() {
            self.layout_group(group);
            count += 1;
        }
        count
    }

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

    fn member_bounds(&self, members: &[GObjectId], exclude: bool) -> Option<(f32, f32, f32, f32)> {
        let mut bounds: Option<(f32, f32, f32, f32)> = None;
        for &member in members {
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
                Some((left, top, r, b)) => (left.min(x), top.min(y), r.max(right), b.max(bottom)),
                None => (x, y, right, bottom),
            });
        }
        bounds
    }

    fn place_group_member(&mut self, member: GObjectId, horizontal: bool, cursor: f32) -> bool {
        let object = &self.objects[member];
        let x = if horizontal {
            cursor
                + if object.pivot_as_anchor {
                    object.width * object.pivot_x
                } else {
                    0.
                }
        } else {
            object.x
        };
        let y = if !horizontal {
            cursor
                + if object.pivot_as_anchor {
                    object.height * object.pivot_y
                } else {
                    0.
                }
        } else {
            object.y
        };
        let changed = (x, y) != (object.x, object.y);
        if changed {
            self.set_xy(member, x, y);
        }
        changed
    }

    /// Arrange only the layout axis, preserving cross-axis alignment. Fixed
    /// groups resize members; automatic groups adopt their member bounds.
    /// Returns the number of members whose layout changed.
    pub fn layout_group(&mut self, group: GObjectId) -> usize {
        self.pending_group_layouts.remove(&group);
        let Some(settings) = self.group_layout_settings(group) else {
            return 0;
        };
        if self.objects[group].parent.is_none() || !self.updating_groups.insert(group) {
            return 0;
        }
        let members = self.group_members(group);
        for &member in &members {
            if self.pending_group_layouts.contains(&member) {
                self.layout_group(member);
            }
        }
        let mut applied = 0;
        let horizontal = settings.layout == GroupLayoutType::Horizontal;
        if settings.layout != GroupLayoutType::None && settings.auto_size_disabled {
            applied = self.resize_group_members(group, 0., 0.);
        } else {
            self.moving_groups.insert(group);
            if settings.layout != GroupLayoutType::None {
                let mut cursor = if horizontal {
                    self.objects[group].x
                } else {
                    self.objects[group].y
                };
                let gap = if horizontal {
                    settings.column_gap
                } else {
                    settings.line_gap
                } as f32;
                for &member in &members {
                    let object = &self.objects[member];
                    if settings.exclude_invisibles && !object.display_visible() {
                        continue;
                    }
                    let extent = if horizontal {
                        object.width
                    } else {
                        object.height
                    };
                    applied += usize::from(self.place_group_member(member, horizontal, cursor));
                    if extent != 0. {
                        cursor += extent + gap;
                    }
                }
            }
            let size = if let Some((x, y, r, b)) =
                self.member_bounds(&members, settings.exclude_invisibles)
            {
                // Refresh the rectangle without translating members twice.
                self.set_xy(group, x, y);
                Size::new(r - x, b - y)
            } else {
                Size::ZERO
            };
            self.moving_groups.remove(&group);
            // Auto-size notifications can move a centered/right-aligned group,
            // but must not proportionally resize its already measured content.
            self.resizing_groups.insert(group);
            self.set_size(group, size.width, size.height);
            self.resizing_groups.remove(&group);
        }
        self.updating_groups.remove(&group);
        applied
    }

    fn group_percentages(
        &self,
        members: &[GObjectId],
        settings: GroupLayoutSettings,
    ) -> GroupPercentages {
        let horizontal = settings.layout == GroupLayoutType::Horizontal;
        let extent = |id| {
            let o = &self.objects[id];
            f64::from(if horizontal { o.width } else { o.height })
        };
        let visible = |id| !settings.exclude_invisibles || self.objects[id].display_visible();
        let included: Vec<_> = members.iter().copied().filter(|&id| visible(id)).collect();
        let main = settings
            .main_grid_index
            .and_then(|index| members.get(index))
            .copied()
            .filter(|&id| visible(id));
        let mut total: f64 = included.iter().map(|&id| extent(id)).sum();
        if let Some(main) = main {
            total += f64::from(settings.main_grid_min_size) - extent(main);
        }
        GroupPercentages {
            members: members
                .iter()
                .map(|&id| {
                    let value = if Some(id) == main {
                        f64::from(settings.main_grid_min_size)
                    } else {
                        extent(id)
                    };
                    (id, if total > 0. { value / total } else { 0. }, visible(id))
                })
                .collect(),
            count: included.len(),
            total,
            main,
        }
    }

    fn resize_group_member(
        &mut self,
        member: GObjectId,
        horizontal: bool,
        extent: f32,
        cross_delta: f32,
    ) {
        let object = &self.objects[member];
        let raw = object.requested_size();
        let requested = if horizontal {
            Size::new(extent, raw.height + cross_delta)
        } else {
            Size::new(raw.width + cross_delta, extent)
        };
        self.set_size_internal(member, requested.width, requested.height, true, false);
    }

    pub(super) fn resize_group_members(
        &mut self,
        group: GObjectId,
        mut dw: f32,
        mut dh: f32,
    ) -> usize {
        let Some(settings) = self.group_layout_settings(group) else {
            return 0;
        };
        if settings.layout == GroupLayoutType::None
            || self.objects[group].parent.is_none()
            || !self.resizing_groups.insert(group)
        {
            return 0;
        }
        let was_updating = !self.updating_groups.insert(group);
        let members = self.group_members(group);
        if self.pending_group_layouts.remove(&group) && !settings.auto_size_disabled {
            for &member in &members {
                if self.pending_group_layouts.contains(&member) {
                    self.layout_group(member);
                }
            }
            let actual = if let Some((x, y, r, b)) =
                self.member_bounds(&members, settings.exclude_invisibles)
            {
                let was_moving = !self.moving_groups.insert(group);
                self.set_xy(group, x, y);
                if !was_moving {
                    self.moving_groups.remove(&group);
                }
                Size::new(r - x, b - y)
            } else {
                Size::ZERO
            };
            dw = self.objects[group].width - actual.width;
            dh = self.objects[group].height - actual.height;
        }
        let shares = self
            .group_percentages
            .remove(&group)
            // Reparenting a group changes which siblings belong to its layout.
            .filter(|cached| {
                cached
                    .members
                    .iter()
                    .map(|(id, _, _)| *id)
                    .eq(members.iter().copied())
            })
            .unwrap_or_else(|| self.group_percentages(&members, settings));
        let horizontal = settings.layout == GroupLayoutType::Horizontal;
        let gap = if horizontal {
            settings.column_gap
        } else {
            settings.line_gap
        };
        let dimension = if horizontal {
            self.objects[group].width
        } else {
            self.objects[group].height
        };
        let mut remaining = f64::from(dimension) - (shares.count as f64 - 1.) * f64::from(gap);
        let mut remaining_percent = 1.;
        let cross_delta = if horizontal { dh } else { dw };
        let extent = |store: &Self, id| {
            let o = &store.objects[id];
            f64::from(if horizontal { o.width } else { o.height })
        };
        let previous: Vec<_> = shares
            .members
            .iter()
            .map(|(id, _, _)| {
                let o = &self.objects[*id];
                (o.x, o.y, o.width, o.height)
            })
            .collect();
        let handled_main = shares.main.filter(|_| remaining >= shares.total);
        if let Some(main) = handled_main {
            self.resize_group_member(
                main,
                horizontal,
                (remaining - (shares.total - f64::from(settings.main_grid_min_size))) as f32,
                cross_delta,
            );
            remaining -= extent(self, main);
            remaining_percent -= shares
                .members
                .iter()
                .find(|(id, _, _)| *id == main)
                .unwrap()
                .1;
        }
        let mut cursor = if horizontal {
            self.objects[group].x
        } else {
            self.objects[group].y
        };
        let mut changed = 0;
        for (&(member, percent, included), before) in shares.members.iter().zip(previous) {
            if !included {
                let object = &self.objects[member];
                let raw = object.requested_size();
                self.resize_group_member(
                    member,
                    horizontal,
                    if horizontal { raw.width } else { raw.height },
                    cross_delta,
                );
            } else {
                if Some(member) != handled_main {
                    // JavaScript Math.round rounds negative ties toward +infinity.
                    let value = if percent == 0. || remaining_percent == 0. {
                        0.
                    } else {
                        (percent / remaining_percent * remaining + 0.5).floor()
                    };
                    self.resize_group_member(member, horizontal, value as f32, cross_delta);
                    remaining -= extent(self, member);
                    remaining_percent -= percent;
                }
                self.place_group_member(member, horizontal, cursor);
                let size = extent(self, member) as f32;
                if size != 0. {
                    cursor += size + gap as f32;
                }
            }
            let after = &self.objects[member];
            changed += usize::from(before != (after.x, after.y, after.width, after.height));
        }
        self.group_percentages.insert(group, shares);
        self.resizing_groups.remove(&group);
        if !was_updating {
            self.updating_groups.remove(&group);
        }
        changed
    }
}

#[cfg(test)]
#[path = "group_resize_tests.rs"]
mod resize_tests;

#[cfg(test)]
#[path = "group_dynamic_alignment_tests.rs"]
mod dynamic_alignment_tests;

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
                    // This fixture executes GGroup against a geometry-only host.
                    // Pivot side effects of GObject.setSize are covered by the
                    // separate size-constraints-reference fixture.
                    1 => store.set_size_internal(members[0], 75., 45., true, false),
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
                binding: None,
                defs: vec![RelationDef {
                    relation_type: kind,
                    percent: false,
                    axis: u8::from(!(kind <= 6 || kind == 14 || (16..=19).contains(&kind))),
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
    fn parent_width_relation_preserves_left_alignment_of_a_group_with_a_pivot() {
        let mut store = ObjectStore::new();
        let parent = store.create(ObjectType::Component);
        store.set_size(parent, 300., 200.);
        let group = store.create(ObjectType::Group);
        store.set_parent(group, Some(parent));
        store.set_size(group, 100., 40.);
        store.set_xy(group, 25., 30.);
        store.set_pivot(group, 0.5, 0.5, false);
        store.set_group_layout_settings(
            group,
            GroupLayoutSettings {
                layout: GroupLayoutType::Horizontal,
                auto_size_disabled: true,
                ..Default::default()
            },
        );
        let member = store.create(ObjectType::Component);
        store.set_parent(member, Some(parent));
        store.set_size(member, 100., 40.);
        store.set_xy(member, 25., 30.);
        store.set_group(member, Some(group));
        store.flush_group_layouts();
        relation(&mut store, group, None, 0);
        relation(&mut store, group, None, 14);
        store.set_size(parent, 400., 200.);
        store.flush_group_layouts();
        assert_eq!(store.position_of(group), Some(Point::new(25., 30.)));
        assert_eq!(store.position_of(member), Some(Point::new(25., 30.)));
        assert_eq!(store.object(group).unwrap().width(), 200.);
    }

    #[test]
    fn group_size_relations_and_pivots_match_original_alignment_traces() {
        let mut store = ObjectStore::new();
        let mut objects = Vec::new();
        let mut target = None;
        let mut states = 0;
        for line in include_str!("../../tests/fixtures/group-pivot-alignment-reference.txt").lines()
        {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let (command, expected) = line.split_once(" | ").unwrap_or((line, ""));
            let values: Vec<f32> = command
                .split_whitespace()
                .skip(1)
                .map(|v| v.parse().unwrap())
                .collect();
            if line.starts_with("case ") {
                store = ObjectStore::new();
                objects.clear();
                let parent = store.create(ObjectType::Component);
                store.set_size(parent, 300., 200.);
                let t = if values[0] == 1. {
                    parent
                } else {
                    let id = store.create(ObjectType::Component);
                    store.set_parent(id, Some(parent));
                    store.set_size(id, 300., 200.);
                    id
                };
                target = Some(t);
                let group = store.create(ObjectType::Group);
                store.set_parent(group, Some(parent));
                store.set_size(group, 100., 60.);
                store.set_xy(group, 25., 30.);
                store.set_pivot(group, values[1], values[1], values[3] == 1.);
                store.set_group_layout_settings(
                    group,
                    GroupLayoutSettings {
                        layout: GroupLayoutType::Horizontal,
                        auto_size_disabled: true,
                        ..Default::default()
                    },
                );
                objects.push(group);
                for (x, y, w, h) in [(25., 30., 40., 20.), (65., 40., 60., 30.)] {
                    let id = store.create(ObjectType::Component);
                    store.set_parent(id, Some(parent));
                    store.set_size(id, w, h);
                    store.set_xy(id, x, y);
                    store.set_group(id, Some(group));
                    objects.push(id);
                }
                // The native capture starts with unmodified exported geometry.
                store.pending_group_layouts.clear();
                let index = if t == parent {
                    None
                } else {
                    store.child_index(t)
                };
                let align = values[2] as u8;
                for kind in [
                    align,
                    if align == 0 {
                        7
                    } else if align == 3 {
                        10
                    } else {
                        13
                    },
                    14,
                    15,
                ] {
                    relation(&mut store, group, index, kind);
                }
            } else {
                store.set_size(target.unwrap(), values[0], values[1]);
                store.flush_group_layouts();
                let expected: Vec<f32> = expected
                    .split_whitespace()
                    .map(|v| v.parse().unwrap())
                    .collect();
                let actual: Vec<f32> = objects
                    .iter()
                    .flat_map(|&id| {
                        let o = store.object(id).unwrap();
                        [o.x(), o.y(), o.width(), o.height()]
                    })
                    .collect();
                assert_eq!(actual, expected, "{command} in state {states}");
                assert_eq!(store.flush_group_layouts(), 0);
                states += 1;
            }
        }
        assert_eq!(states, 108);
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

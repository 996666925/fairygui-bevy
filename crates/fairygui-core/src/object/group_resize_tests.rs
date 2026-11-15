use super::*;

#[test]
fn fixed_and_auto_group_resizing_matches_native_allocations_and_cached_ratios() {
    let mut store = ObjectStore::new();
    let mut group = None;
    let mut members = Vec::new();
    let mut cases = 0;
    let mut states = 0;
    for line in include_str!("../../tests/fixtures/group-resize-reference.txt").lines() {
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
            store.set_size(id, 260., 160.);
            store.set_group_layout_settings(
                id,
                GroupLayoutSettings {
                    layout: if v[0] == 1. {
                        GroupLayoutType::Horizontal
                    } else {
                        GroupLayoutType::Vertical
                    },
                    auto_size_disabled: v[1] == 1.,
                    main_grid_index: if v[2] < 0. { None } else { Some(v[2] as usize) },
                    exclude_invisibles: v[3] == 1.,
                    line_gap: 9,
                    column_gap: 7,
                    ..Default::default()
                },
            );
            group = Some(id);
            for i in 0..3 {
                let member = store.create(ObjectType::Component);
                store.set_parent(member, Some(parent));
                let zero = i == 1 && v[6] == 1.;
                let width = if zero { 0. } else { 40. + i as f32 * 20. };
                let height = if zero { 0. } else { 20. + i as f32 * 10. };
                store.set_size(member, width, height);
                store.set_pivot(member, v[4] * 0.5, v[4] * 0.5, v[4] == 1.);
                store.set_xy(
                    member,
                    40. + i as f32 * 100. + width * v[4] * 0.5,
                    60. + i as f32 * 70. + height * v[4] * 0.5,
                );
                if v[5] == 1. {
                    let limits = [
                        (18., 100., 15., 100.),
                        (10., 65., 8., 70.),
                        (25., 130., 12., 85.),
                    ][i];
                    let o = store.object_mut(member).unwrap();
                    (o.min_width, o.max_width, o.min_height, o.max_height) = limits;
                }
                store.set_group(member, Some(id));
                members.push(member);
            }
            cases += 1;
        } else {
            let group = group.unwrap();
            match v[0] as u8 {
                1 => {
                    let size = store.object(group).unwrap().size();
                    store.set_size(group, size.width + 73., size.height + 17.);
                }
                2 => store.set_size(group, 319., 211.),
                3 => {
                    let p = store.position_of(group).unwrap();
                    store.set_xy(group, p.x + 13., p.y - 5.);
                    store.set_alpha(members[0], 0.5);
                    store.set_size(group, 260., 180.);
                }
                4 => {
                    store.set_visible(members[1], false);
                    store.flush_group_layouts();
                    store.set_size(group, 271., 203.);
                }
                5 => {
                    store.set_visible(members[1], true);
                    store.set_size(members[0], 55., 31.);
                    store.flush_group_layouts();
                    store.set_size(group, 342., 225.);
                }
                6 => {
                    store.set_group(members[2], None);
                    store.flush_group_layouts();
                    store.set_size(group, 278., 184.);
                }
                7 | 8 => {
                    let mut settings = store.group_layout_settings(group).unwrap();
                    if v[0] == 7. {
                        settings.column_gap = 12;
                        settings.line_gap = 3;
                    } else {
                        settings.main_grid_index = Some(2);
                        settings.main_grid_min_size = 65.;
                    }
                    store.set_group_layout_settings(group, settings);
                    store.flush_group_layouts();
                    let (w, h) = if v[0] == 7. {
                        (323., 217.)
                    } else {
                        (352., 229.)
                    };
                    store.set_size(group, w, h);
                }
                _ => {}
            }
            store.flush_group_layouts();
            for (index, id) in std::iter::once(group)
                .chain(members.iter().copied())
                .enumerate()
            {
                let o = store.object(id).unwrap();
                assert_eq!(
                    [o.x, o.y, o.width, o.height],
                    v[1 + index * 4..5 + index * 4],
                    "case {cases}: {line}, object {index}"
                );
            }
            assert_eq!(store.flush_group_layouts(), 0);
            states += 1;
        }
    }
    assert_eq!((cases, states), (192, 1728));
}

#[test]
fn zero_percent_members_stay_finite_when_the_main_grid_takes_all_available_space() {
    for main_min in [0., 50.] {
        let mut store = ObjectStore::new();
        let parent = store.create(ObjectType::Component);
        let group = store.create(ObjectType::Group);
        store.set_parent(group, Some(parent));
        store.set_size(group, 100., 40.);
        store.set_group_layout_settings(
            group,
            GroupLayoutSettings {
                layout: GroupLayoutType::Horizontal,
                auto_size_disabled: true,
                main_grid_index: Some(0),
                main_grid_min_size: main_min,
                column_gap: 7,
                ..Default::default()
            },
        );
        let members: Vec<_> = (0..2)
            .map(|_| {
                let id = store.create(ObjectType::Component);
                store.set_parent(id, Some(parent));
                store.set_group(id, Some(group));
                id
            })
            .collect();
        store.flush_group_layouts();
        for width in [96., 100., 147., 100.] {
            store.set_size(group, width, 40.);
            assert_eq!(store.object(members[0]).unwrap().width(), width - 7.);
            assert_eq!(store.object(members[1]).unwrap().width(), 0.);
            assert_eq!(store.object(members[1]).unwrap().x(), width);
            assert_eq!(store.flush_group_layouts(), 0);
        }
    }
}

#[test]
fn resizing_an_outer_group_resizes_nested_group_members_once() {
    let mut store = ObjectStore::new();
    let parent = store.create(ObjectType::Component);
    let outer = store.create(ObjectType::Group);
    let inner = store.create(ObjectType::Group);
    let leaf = store.create(ObjectType::Component);
    for id in [outer, inner, leaf] {
        store.set_parent(id, Some(parent));
    }
    store.set_size(outer, 300., 100.);
    store.set_size(inner, 100., 100.);
    store.set_size(leaf, 100., 100.);
    store.set_xy(inner, 10., 10.);
    store.set_xy(leaf, 130., 20.);
    store.set_group_layout_settings(
        inner,
        GroupLayoutSettings {
            layout: GroupLayoutType::Vertical,
            auto_size_disabled: true,
            line_gap: 5,
            ..Default::default()
        },
    );
    let mut cells = Vec::new();
    for (height, y) in [(20., 10.), (30., 35.)] {
        let cell = store.create(ObjectType::Component);
        store.set_parent(cell, Some(parent));
        store.set_size(cell, 80., height);
        store.set_xy(cell, 10., y);
        store.set_group(cell, Some(inner));
        cells.push(cell);
    }
    store.flush_group_layouts();
    store.set_group(inner, Some(outer));
    store.set_group(leaf, Some(outer));
    store.set_group_layout_settings(
        outer,
        GroupLayoutSettings {
            layout: GroupLayoutType::Horizontal,
            auto_size_disabled: true,
            main_grid_index: Some(0),
            column_gap: 10,
            ..Default::default()
        },
    );
    store.flush_group_layouts();
    assert_eq!(store.object(inner).unwrap().size(), Size::new(190., 100.));
    assert_eq!(
        store.object(leaf).unwrap().position(),
        Point::new(200., 20.)
    );
    assert_eq!(store.object(cells[0]).unwrap().size(), Size::new(170., 38.));
    assert_eq!(store.object(cells[1]).unwrap().size(), Size::new(170., 57.));
    assert_eq!(
        store.object(cells[0]).unwrap().position(),
        Point::new(0., 10.)
    );
    assert_eq!(
        store.object(cells[1]).unwrap().position(),
        Point::new(0., 53.)
    );
    assert_eq!(store.flush_group_layouts(), 0);
}

#[test]
fn groups_default_to_no_main_grid_and_reject_invalid_minimums() {
    let mut store = ObjectStore::new();
    let group = store.create(ObjectType::Group);
    let original = store.group_layout_settings(group).unwrap();
    assert_eq!(original, GroupLayoutSettings::default());
    for minimum in [-1., f32::NAN, f32::INFINITY] {
        assert!(!store.set_group_layout_settings(
            group,
            GroupLayoutSettings {
                main_grid_min_size: minimum,
                ..original
            }
        ));
        assert_eq!(store.group_layout_settings(group), Some(original));
    }
}

#[test]
fn reparented_groups_never_resize_cached_members_from_the_previous_container() {
    let mut store = ObjectStore::new();
    let previous_parent = store.create(ObjectType::Component);
    let new_parent = store.create(ObjectType::Component);
    let group = store.create(ObjectType::Group);
    store.set_parent(group, Some(previous_parent));
    store.set_size(group, 200., 50.);
    store.set_group_layout_settings(
        group,
        GroupLayoutSettings {
            layout: GroupLayoutType::Horizontal,
            auto_size_disabled: true,
            ..Default::default()
        },
    );
    let members: Vec<_> = [40., 60.]
        .into_iter()
        .map(|width| {
            let id = store.create(ObjectType::Component);
            store.set_parent(id, Some(previous_parent));
            store.set_size(id, width, 20.);
            store.set_group(id, Some(group));
            id
        })
        .collect();
    store.flush_group_layouts();
    let previous: Vec<_> = members
        .iter()
        .map(|&id| {
            let o = store.object(id).unwrap();
            (o.position(), o.size())
        })
        .collect();
    store.set_parent(group, Some(new_parent));
    store.set_size(group, 300., 50.);
    assert!(store.group_members(group).is_empty());
    assert_eq!(
        members
            .iter()
            .map(|&id| {
                let o = store.object(id).unwrap();
                (o.position(), o.size())
            })
            .collect::<Vec<_>>(),
        previous
    );
}

use super::*;
use crate::{
    package::ObjectType,
    scroll::{ScrollPane, ScrollType},
    widget::{ButtonMode, Controller, ListSelectionMode, OverflowType},
};
use std::cell::{Cell, RefCell};

pub(super) fn fixture(
    layout: ListLayoutType,
    auto: bool,
    explicit: bool,
) -> (ObjectStore, GObjectId, GObjectId) {
    let mut store = ObjectStore::new();
    let parent = store.create(ObjectType::Component);
    store.objects[parent]
        .control
        .component_mut()
        .unwrap()
        .controllers
        .push(Controller {
            name: "selection".into(),
            selected_index: 0,
            page_ids: (0..2000).map(|i| i.to_string()).collect(),
            page_names: (0..2000).map(|i| i.to_string()).collect(),
            ..Default::default()
        });
    let list = store.create(ObjectType::List);
    store.set_parent(list, Some(parent));
    store.set_size(list, 120., 80.);
    let ControlData::List(d) = &mut store.objects[list].control else {
        unreachable!()
    };
    d.layout = layout;
    d.auto_resize_item = auto;
    d.column_count = if explicit { 3 } else { 0 };
    d.line_count = if explicit { 2 } else { 0 };
    d.column_gap = 3;
    d.line_gap = 2;
    d.align = 1;
    d.vertical_align = 1;
    d.children_render_order = 2;
    d.selection_mode = ListSelectionMode::Multiple;
    d.component.overflow = OverflowType::Scroll;
    let mut p = ScrollPane::default();
    p.scroll_type = if matches!(
        layout,
        ListLayoutType::SingleColumn | ListLayoutType::FlowHorizontal
    ) {
        ScrollType::Vertical
    } else {
        ScrollType::Horizontal
    };
    p.set_view_size(Size::new(120., 80.));
    d.component.scroll_pane = Some(p);
    store.set_list_selection_controller(list, Some("selection"));
    (store, parent, list)
}

pub(super) fn button(store: &mut ObjectStore, width: f32, height: f32) -> GObjectId {
    let id = store.create(ObjectType::Button);
    let ControlData::Button(d) = &mut store.objects[id].control else {
        unreachable!()
    };
    d.mode = ButtonMode::Check;
    store.set_size(id, width, height);
    id
}

#[test]
fn virtual_rows_match_native_layout_pool_identity_measurement_and_selection_traces() {
    let lines: Vec<_> = include_str!("../../tests/fixtures/virtual-list-reference.txt")
        .lines()
        .filter(|l| !l.starts_with('#'))
        .collect();
    let mut cases = 0;
    let mut snapshots = 0;
    for (header, line) in lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with("case "))
    {
        let end = lines[header + 1..]
            .iter()
            .position(|l| l.starts_with("case "))
            .map_or(lines.len(), |n| header + 1 + n);
        let chunk = &lines[header + 1..end];
        let case: Vec<_> = line.split_whitespace().collect();
        let layout = ListLayoutType::from_code(case[2].parse().unwrap());
        let vertical = matches!(
            layout,
            ListLayoutType::SingleColumn | ListLayoutType::FlowHorizontal
        );
        let variable = case[4] == "1";
        let mixed = case[5] == "1";
        let (mut store, parent, list) = fixture(layout, case[3] == "1", case[6] == "1");
        let ids = RefCell::new(Vec::new());
        let rendered = Cell::new(0);
        let mut create = |store: &mut ObjectStore, _: &str| {
            let id = button(store, 24.25, 19.5);
            ids.borrow_mut().push(id);
            Some(id)
        };
        let mut provider = |i| (mixed && i % 3 == 1).then(|| "b".into());
        let mut render = |store: &mut ObjectStore, i: usize, id| {
            rendered.set(rendered.get() + 1);
            if variable {
                let s = store.object(id).unwrap().size();
                store.set_size(
                    id,
                    if vertical {
                        s.width
                    } else {
                        22.25 + (i % 4) as f32 * 3.
                    },
                    if vertical {
                        13.5 + (i % 5) as f32 * 4.
                    } else {
                        s.height
                    },
                );
            }
        };
        store.enable_virtual_list(list, "a", &mut create).unwrap();
        for line in chunk {
            let v: Vec<_> = line.split_whitespace().collect();
            if v[0] == "probe" {
                let values: Vec<f32> = v[1..].iter().map(|v| v.parse().unwrap()).collect();
                let g = &store.virtual_states[&list].geometry;
                let rect = g.item_rect(values[0] as usize).unwrap();
                let snap =
                    g.snapping_position(Point::new(73., 91.), Point::new(values[5], values[5]));
                for (actual, expected) in [rect.x, rect.y, rect.width, rect.height, snap.x, snap.y]
                    .into_iter()
                    .zip([
                        values[1], values[2], values[3], values[4], values[6], values[7],
                    ])
                {
                    assert!(
                        (actual - expected).abs() < 0.01,
                        "probe case {} {actual} != {expected}: {line}",
                        case[1]
                    );
                }
                continue;
            }
            let a: f32 = v[2].parse().unwrap();
            let b: f32 = v[3].parse().unwrap();
            match v[1] {
                "count" => {
                    store.set_virtual_item_count(list, a as usize).unwrap();
                }
                "move" => {
                    let max = store.scroll_pane(list).unwrap().max_scroll_pos();
                    store.set_scroll_pos(
                        list,
                        if vertical { 0. } else { (max.x * a).round() },
                        if vertical { (max.y * a).round() } else { 0. },
                        false,
                    );
                }
                "add" => {
                    store.add_list_selection(list, a as usize);
                }
                "click" => {
                    let row = store.virtual_item_object(list, a as usize).unwrap();
                    store.select_clicked_item_with_modifiers(row, true, false);
                }
                "force" => {
                    store.invalidate_virtual_list(list, false);
                }
                "all" => {
                    store.select_all_list_items(list);
                }
                "reverse" => {
                    store.reverse_list_selection(list);
                }
                "clear" => {
                    store.clear_list_selection(list);
                }
                "resize" => {
                    store.set_size(list, a, b);
                }
                _ => panic!("{line}"),
            }
            store
                .refresh_virtual_list(list, &mut create, &mut provider, &mut render)
                .unwrap();
            let info = store.virtual_list_info(list).unwrap();
            let pane = store.scroll_pane(list).unwrap();
            let ControlData::List(d) = store.object(list).unwrap().control() else {
                unreachable!()
            };
            let controller = store.objects[parent]
                .control
                .component()
                .unwrap()
                .controllers[0]
                .selected_index;
            let values = [
                info.item_count as f32,
                info.first_index as f32,
                pane.content_size().width,
                pane.content_size().height,
                pane.pos().x,
                pane.pos().y,
                pane.display_pos().x,
                pane.display_pos().y,
                info.alignment.x,
                info.alignment.y,
                d.apex_index as f32,
                info.pooled_count as f32,
                ids.borrow().len() as f32,
                rendered.get() as f32,
                controller as f32,
            ];
            for (index, value) in values.into_iter().enumerate() {
                let expected: f32 = v[index + 4].parse().unwrap();
                assert!(
                    (value - expected).abs() < 0.002,
                    "case {} field {index}: {value} != {expected}; values {values:?}; {line}",
                    case[1]
                );
            }
            let expected: Vec<usize> = if v[19] == "_" {
                Vec::new()
            } else {
                v[19].split(',').map(|s| s.parse().unwrap()).collect()
            };
            assert_eq!(
                store.list_selections(list).unwrap(),
                expected,
                "case {} {line}",
                case[1]
            );
            let rows = store.list_items(list).unwrap();
            assert_eq!(
                rows.len(),
                v[20].parse::<usize>().unwrap(),
                "case {} {line}",
                case[1]
            );
            let (expected_rows, remainder) = v[21..].as_chunks::<8>();
            assert!(remainder.is_empty());
            for (&id, row) in rows.iter().zip(expected_rows) {
                assert_eq!(
                    store.virtual_states[&list].entries[row[0].parse::<usize>().unwrap()].url,
                    row[2]
                );
                assert_eq!(
                    store.virtual_item_index(list, id).unwrap(),
                    row[0].parse::<usize>().unwrap(),
                    "case {} {line}",
                    case[1]
                );
                assert_eq!(
                    ids.borrow().iter().position(|&o| o == id).unwrap(),
                    row[1].parse::<usize>().unwrap(),
                    "identity case {} {line}",
                    case[1]
                );
                let object = store.object(id).unwrap();
                for (value, expected) in [object.x(), object.y(), object.width(), object.height()]
                    .into_iter()
                    .zip(&row[3..7])
                {
                    assert!(
                        (value - expected.parse::<f32>().unwrap()).abs() < 0.002,
                        "geometry case {} {line}",
                        case[1]
                    );
                }
                assert_eq!(
                    store.button_selected(id),
                    Some(row[7] == "1"),
                    "case {} {line}",
                    case[1]
                );
            }
            snapshots += 1;
        }
        cases += 1;
    }
    assert_eq!((cases, snapshots), (80, 1760));
}

#[test]
fn failed_virtual_enable_preserves_authored_rows_and_disposes_only_new_invalid_sample() {
    let (mut store, _, list) = fixture(ListLayoutType::SingleColumn, true, false);
    let row = button(&mut store, 0., 0.);
    store.set_parent(row, Some(list));
    assert_eq!(
        store.enable_virtual_list(list, "a", &mut |_, _| None),
        Err(VirtualListError::InvalidGeometry)
    );
    assert_eq!(store.list_items(list), Some(vec![row]));
    assert!(!store.is_virtual_list(list));
    store.set_size(row, 10., 20.);
    store
        .enable_virtual_list(list, "a", &mut |_, _| None)
        .unwrap();
    let removed = store.remove(list);
    assert!(removed.contains(&row));
    assert!(!store.contains(row));
}

#[test]
fn failed_factory_during_a_jump_can_retry_without_leaking_the_old_window() {
    let (mut store, _, list) = fixture(ListLayoutType::SingleColumn, true, false);
    let mut create = |store: &mut ObjectStore, _: &str| Some(button(store, 24., 20.));
    store.enable_virtual_list(list, "a", &mut create).unwrap();
    store.set_virtual_item_count(list, 1000).unwrap();
    store
        .refresh_virtual_list(list, &mut create, &mut |_| None, &mut |_, _, _| {})
        .unwrap();
    store.set_scroll_pos(list, 0., 500., false);
    let mut attempts = 0;
    let result = store.refresh_virtual_list(
        list,
        &mut |store, url| {
            attempts += 1;
            if attempts >= 2 {
                None
            } else {
                create(store, url)
            }
        },
        &mut |_| Some("b".into()),
        &mut |_, _, _| {},
    );
    assert_eq!(result, Err(VirtualListError::MissingResource("b".into())));
    assert_eq!(store.list_item_count(list), 0);
    store
        .refresh_virtual_list(
            list,
            &mut create,
            &mut |_| Some("b".into()),
            &mut |_, _, _| {},
        )
        .unwrap();
    let info = store.virtual_list_info(list).unwrap();
    assert!(info.first_index > 0 && info.active_count < 10);
    assert_eq!(store.list_item_count(list), info.active_count);
    for row in store.list_items(list).unwrap() {
        assert!(store.virtual_item_index(list, row).is_some());
    }
    let count = store.len();
    store.remove(list);
    assert_eq!(store.len(), 1, "all {count} active/pooled rows disposed");
}

#[test]
fn renderer_may_remove_the_list_without_panicking_or_retaining_pool_objects() {
    let (mut store, _, list) = fixture(ListLayoutType::SingleColumn, true, false);
    let mut create = |store: &mut ObjectStore, _: &str| Some(button(store, 24., 20.));
    store.enable_virtual_list(list, "a", &mut create).unwrap();
    store.set_virtual_item_count(list, 1000).unwrap();
    assert_eq!(
        store.refresh_virtual_list(list, &mut create, &mut |_| None, &mut |store, _, _| {
            store.remove(list);
        }),
        Err(VirtualListError::Removed)
    );
    assert_eq!(store.len(), 1);
}

#[test]
fn renderer_cannot_recursively_refresh_the_same_virtual_list() {
    let (mut store, _, list) = fixture(ListLayoutType::SingleColumn, true, false);
    let mut create = |store: &mut ObjectStore, _: &str| Some(button(store, 24., 20.));
    store.enable_virtual_list(list, "a", &mut create).unwrap();
    store.set_virtual_item_count(list, 1000).unwrap();
    let nested = RefCell::new(None);

    let rendered = store
        .refresh_virtual_list(list, &mut create, &mut |_| None, &mut |store, _, _| {
            if nested.borrow().is_none() {
                *nested.borrow_mut() = Some(store.refresh_virtual_list(
                    list,
                    &mut |store, _| Some(button(store, 24., 20.)),
                    &mut |_| None,
                    &mut |_, _, _| {},
                ));
            }
        })
        .unwrap();

    assert!(rendered > 0);
    assert_eq!(
        *nested.borrow(),
        Some(Err(VirtualListError::ReentrantRefresh))
    );
    assert!(store.virtual_list_info(list).unwrap().active_count > 0);

    store.invalidate_virtual_list(list, false);
    assert!(
        store
            .refresh_virtual_list(list, &mut create, &mut |_| None, &mut |_, _, _| {},)
            .is_ok(),
        "the guard must be released after the outer refresh"
    );
}

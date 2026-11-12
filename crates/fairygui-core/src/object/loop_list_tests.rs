use super::tests::{button, fixture};
use super::*;
use std::cell::{Cell, RefCell};

#[test]
fn loop_lists_match_native_sixfold_records_commands_drag_tweens_and_selection() {
    let lines: Vec<_> = include_str!("../../tests/fixtures/loop-list-reference.txt")
        .lines()
        .filter(|l| !l.starts_with('#'))
        .collect();
    let (mut cases, mut snapshots, mut probes, mut native_errors) = (0, 0, 0, 0);
    for (header, line) in lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with("case "))
    {
        let end = lines[header + 1..]
            .iter()
            .position(|l| l.starts_with("case "))
            .map_or(lines.len(), |i| header + 1 + i);
        let case: Vec<_> = line.split_whitespace().collect();
        let layout = ListLayoutType::from_code(case[2].parse().unwrap());
        let vertical = layout == ListLayoutType::SingleColumn;
        let variable = case[4] == "1";
        let mixed = case[5] == "1";
        let (mut store, parent, list) = fixture(layout, case[3] == "1", false);
        store.set_scroll_snap_to_item(list, case[6] == "1");
        let ids = RefCell::new(Vec::new());
        let rendered = Cell::new(0);
        let mut factory = |store: &mut ObjectStore, _: &str| {
            let id = button(store, 24.25, 19.5);
            ids.borrow_mut().push(id);
            Some(id)
        };
        let mut provider = |i| (mixed && i % 3 == 1).then(|| "b".into());
        let mut render = |store: &mut ObjectStore, i: usize, row| {
            rendered.set(rendered.get() + 1);
            if variable {
                let s = store.object(row).unwrap().size();
                store.set_size(
                    row,
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
        store
            .enable_loop_virtual_list(list, "a", &mut factory)
            .unwrap();
        for line in &lines[header + 1..end] {
            let v: Vec<_> = line.split_whitespace().collect();
            if v[0] == "view" {
                let expected: i32 = v[1].parse().unwrap();
                assert_eq!(
                    store.list_first_item_in_view(list),
                    usize::try_from(expected).ok(),
                    "view case {}",
                    case[1]
                );
                continue;
            }
            if v[0] == "native_error" {
                assert_eq!(v[1], "pagination_snap_outside_cached_pages");
                // Native has no output here. Verify the bounded Rust behavior
                // separately, without treating it as an oracle equality claim.
                assert_eq!(layout, ListLayoutType::Pagination);
                store.end_scroll_drag_after(list, 0.0125);
                for _ in 0..8 {
                    store.advance_scrolls(0.5);
                    store
                        .refresh_virtual_list(list, &mut factory, &mut provider, &mut render)
                        .unwrap();
                }
                let p = store.scroll_pane(list).unwrap();
                assert!(p.pos().x.is_finite() && p.display_pos().x.is_finite());
                assert!(store.list_item_count(list) > 0);
                native_errors += 1;
                break;
            }
            if v[0] == "probe" {
                let p: Vec<f32> = v[1..].iter().map(|v| v.parse().unwrap()).collect();
                let s = &store.virtual_states[&list];
                let index = s.geometry.first / s.num_items * s.num_items + p[0] as usize;
                let rect = s.geometry.item_rect(index).unwrap();
                let snap = s
                    .geometry
                    .snapping_position(Point::new(73., 91.), Point::new(p[5], p[5]));
                for (actual, expected) in [rect.x, rect.y, rect.width, rect.height, snap.x, snap.y]
                    .into_iter()
                    .zip([p[1], p[2], p[3], p[4], p[6], p[7]])
                {
                    assert!(
                        (actual - expected).abs() < 0.02,
                        "case {} probe {actual}!={expected}: {line}",
                        case[1]
                    );
                }
                let child = p[8] as i32;
                let expected = (child >= 0 && (child as usize) < store.list_item_count(list))
                    .then_some(child as usize);
                assert_eq!(
                    store.list_item_index_to_child_index(list, p[0] as usize),
                    expected,
                    "case {} {line}",
                    case[1]
                );
                probes += 1;
                continue;
            }
            let a: f32 = v[2].parse().unwrap();
            let b: f32 = v[3].parse().unwrap();
            let c: f32 = v[4].parse().unwrap();
            let dt: f32 = v[5].parse().unwrap();
            match v[1] {
                "count" => {
                    store.set_virtual_item_count(list, a as usize).unwrap();
                }
                "go" => {
                    store.set_scroll_pos_animated(list, a, b, c != 0.);
                }
                "wheel" => {
                    store.scroll_mouse_wheel(list, a);
                }
                "step" => {
                    store.scroll_container_by(list, !vertical, a);
                }
                "perc" => {
                    store.set_scroll_perc(list, vertical, a);
                }
                "tick" => {
                    store.advance_scrolls(dt);
                }
                "add" => {
                    store.add_list_selection(list, a as usize);
                }
                "click" => {
                    let row = store.list_item_at(list, a as usize).unwrap();
                    store.select_clicked_item_with_modifiers(row, c != 0., b != 0.);
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
                "force" => {
                    store.invalidate_virtual_list(list, false);
                }
                "kill" => {
                    store.objects[list]
                        .control
                        .component_mut()
                        .unwrap()
                        .scroll_pane
                        .as_mut()
                        .unwrap()
                        .stop();
                }
                "down" => {
                    store.begin_scroll_drag(list);
                }
                "move" => {
                    store.drag_scroll_by(list, a, b, dt);
                }
                "up" => {
                    store.end_scroll_drag_after(list, dt);
                }
                _ => panic!("{line}"),
            }
            store
                .refresh_virtual_list(list, &mut factory, &mut provider, &mut render)
                .unwrap();
            let info = store.virtual_list_info(list).unwrap();
            let p = store.scroll_pane(list).unwrap();
            let ControlData::List(d) = store.object(list).unwrap().control() else {
                unreachable!()
            };
            let controller = store
                .object(parent)
                .unwrap()
                .control()
                .component()
                .unwrap()
                .controllers[0]
                .selected_index;
            let actual = [
                info.item_count as f32,
                info.real_item_count as f32,
                info.first_index as f32,
                p.content_size().width,
                p.content_size().height,
                p.pos().x,
                p.pos().y,
                p.display_pos().x,
                p.display_pos().y,
                info.alignment.x,
                info.alignment.y,
                d.apex_index as f32,
                info.pooled_count as f32,
                ids.borrow().len() as f32,
                rendered.get() as f32,
                controller as f32,
            ];
            for (i, actual) in actual.into_iter().enumerate() {
                let expected: f32 = v[6 + i].parse().unwrap();
                assert!(
                    (actual - expected).abs() < 0.02,
                    "case {} field {i}: {actual}!={expected}; state {info:?} pane {:?} step {:?}: {line}",
                    case[1],
                    p.pos(),
                    p.scroll_step()
                );
            }
            let selected: Vec<usize> = if v[22] == "_" {
                Vec::new()
            } else {
                v[22].split(',').map(|v| v.parse().unwrap()).collect()
            };
            assert_eq!(
                store.list_selections(list).unwrap(),
                selected,
                "case {} {line}",
                case[1]
            );
            let rows = store.list_items(list).unwrap();
            assert_eq!(
                rows.len(),
                v[23].parse::<usize>().unwrap(),
                "case {} {line}",
                case[1]
            );
            let (expected, remainder) = v[24..].as_chunks::<9>();
            assert!(remainder.is_empty());
            for (&row, r) in rows.iter().zip(expected) {
                assert_eq!(
                    store.virtual_item_record_index(list, row).unwrap(),
                    r[0].parse::<usize>().unwrap(),
                    "case {} {line}",
                    case[1]
                );
                assert_eq!(
                    store.virtual_item_index(list, row).unwrap(),
                    r[1].parse::<usize>().unwrap(),
                    "index case {} {line}",
                    case[1]
                );
                assert_eq!(
                    ids.borrow().iter().position(|&id| id == row).unwrap(),
                    r[2].parse::<usize>().unwrap(),
                    "identity case {} {line}",
                    case[1]
                );
                assert_eq!(
                    store.virtual_states[&list].entries[r[0].parse::<usize>().unwrap()].url,
                    r[3]
                );
                let o = store.object(row).unwrap();
                for (a, b) in [o.x(), o.y(), o.width(), o.height()]
                    .into_iter()
                    .zip(&r[4..8])
                {
                    assert!(
                        (a - b.parse::<f32>().unwrap()).abs() < 0.02,
                        "geometry case {} {line}",
                        case[1]
                    );
                }
                assert_eq!(store.button_selected(row), Some(r[8] == "1"));
            }
            snapshots += 1;
        }
        cases += 1;
    }
    assert_eq!(
        (cases, snapshots, probes, native_errors),
        (96, 3472, 10176, 16)
    );
}

#[test]
fn loop_enable_rejects_flow_layouts_without_modifying_the_original_list() {
    for layout in [ListLayoutType::FlowHorizontal, ListLayoutType::FlowVertical] {
        let (mut store, _, list) = fixture(layout, true, false);
        let row = button(&mut store, 20., 20.);
        store.set_parent(row, Some(list));
        assert_eq!(
            store.enable_loop_virtual_list(list, "a", &mut |_, _| None),
            Err(VirtualListError::UnsupportedLoopLayout)
        );
        assert_eq!(store.list_items(list), Some(vec![row]));
        assert!(!store.is_virtual_list(list));
        assert!(store.scroll_pane(list).unwrap().bounceback_effect);
    }
}

use super::*;
use crate::{
    geom::Size,
    package::ObjectType,
    scroll::{ScrollPane, ScrollType},
    widget::{ButtonMode, ListItemData, ListSelectionMode},
};

pub(crate) fn bars(store: &mut ObjectStore, list: GObjectId) -> [GObjectId; 2] {
    let mut pane = ScrollPane::default();
    pane.scroll_type = ScrollType::Both;
    pane.vertical_bar_res = Some("vertical".into());
    pane.horizontal_bar_res = Some("horizontal".into());
    store.objects[list]
        .control
        .component_mut()
        .unwrap()
        .scroll_pane = Some(pane);
    assert_eq!(
        store.attach_scroll_bars(list, &mut |store, name| {
            let bar = store.create(ObjectType::ScrollBar);
            store.set_size(
                bar,
                if name == "vertical" { 12. } else { 120. },
                if name == "vertical" { 100. } else { 10. },
            );
            Some(bar)
        }),
        2
    );
    let pane = store.scroll_pane(list).unwrap();
    [pane.vt_bar.unwrap(), pane.hz_bar.unwrap()]
}

#[test]
fn pane_bar_ownership_matches_36_native_attachment_and_disposal_cases() {
    use crate::scroll::ScrollBarDisplayType;
    let mut cases = 0;
    for line in include_str!("../../tests/fixtures/list-bar-reference.txt")
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
    {
        let fields: Vec<_> = line.split_whitespace().collect();
        let number = |i: usize| fields[i].parse::<usize>().unwrap();
        let mut store = ObjectStore::new();
        let list = store.create(ObjectType::List);
        store.set_list_selection_mode(list, ListSelectionMode::Multiple);
        let rows: Vec<_> = (0..number(2))
            .map(|i| {
                let row = store.create(ObjectType::Button);
                if let ControlData::Button(data) = &mut store.objects[row].control {
                    data.mode = ButtonMode::Check;
                }
                store.set_parent(row, Some(list));
                store.set_button_selected(row, i % 2 == 0);
                row
            })
            .collect();
        let mut pane = ScrollPane::default();
        pane.scroll_type = ScrollType::from_code(number(0) as i8);
        pane.bar_display = ScrollBarDisplayType::from_code(number(1) as i8);
        pane.vertical_bar_res = Some("vertical".into());
        pane.horizontal_bar_res = Some("horizontal".into());
        store.objects[list]
            .control
            .component_mut()
            .unwrap()
            .scroll_pane = Some(pane);
        assert_eq!(
            store.attach_scroll_bars(list, &mut |store, _| Some(
                store.create(ObjectType::ScrollBar)
            )),
            number(5),
            "{line}"
        );
        let pane = store.scroll_pane(list).unwrap();
        let bars: Vec<_> = [pane.vt_bar, pane.hz_bar].into_iter().flatten().collect();
        for &bar in &bars {
            store.add_child_at(list, bar, 0);
        }
        assert_eq!(store.list_item_count(list), number(3), "{line}");
        let selected = store.list_selections(list).unwrap();
        let expected = if fields[4] == "_" {
            vec![]
        } else {
            fields[4]
                .split(',')
                .map(|s| s.parse::<usize>().unwrap())
                .collect()
        };
        assert_eq!(selected, expected, "{line}");
        // Native pane disposal releases its bars while the owner and rows live.
        // Exercise that ownership boundary using ObjectStore's removal API.
        let removed: Vec<_> = bars.into_iter().flat_map(|bar| store.remove(bar)).collect();
        assert_eq!(removed.len(), number(7), "{line}");
        assert_eq!(
            rows.iter().filter(|&&row| !store.contains(row)).count(),
            number(6)
        );
        assert_eq!(store.list_items(list), Some(rows), "{line}");
        assert!(store.scroll_pane(list).unwrap().vt_bar.is_none());
        assert!(store.scroll_pane(list).unwrap().hz_bar.is_none());
        cases += 1;
    }
    assert_eq!(cases, 36);
}

fn fixture(tree: bool) -> (ObjectStore, GObjectId, Vec<GObjectId>, [GObjectId; 2]) {
    let mut store = ObjectStore::new();
    let list = store.create(if tree {
        ObjectType::Tree
    } else {
        ObjectType::List
    });
    store.set_size(list, 120., 100.);
    let rows = (0..4)
        .map(|_| {
            let row = store.create(ObjectType::Button);
            if let ControlData::Button(data) = &mut store.objects[row].control {
                data.mode = ButtonMode::Check;
            }
            store.set_parent(row, Some(list));
            store.set_size(row, 80., 40.);
            row
        })
        .collect::<Vec<_>>();
    let bars = bars(&mut store, list);
    store.add_child_at(list, bars[0], 0);
    store.add_child_at(list, bars[1], 3);
    store.layout_scroll_bars(list);
    (store, list, rows, bars)
}

#[test]
fn pane_bars_do_not_occupy_row_indices_layout_extents_or_item_data() {
    let (mut store, list, rows, bars) = fixture(false);
    assert_eq!(store.object(list).unwrap().children().len(), 6);
    assert_eq!(store.list_item_count(list), 4);
    assert_eq!(store.list_items(list), Some(rows.clone()));
    for (i, &row) in rows.iter().enumerate() {
        assert_eq!(store.list_item_index(list, row), Some(i));
        assert_eq!(store.list_item_at(list, i), Some(row));
    }
    assert!(store.list_item_at(list, 4).is_none());
    store.layout_scroll_bars(list);
    let bar_bounds: Vec<_> = bars
        .iter()
        .map(|&id| (store.objects[id].position(), store.objects[id].size()))
        .collect();
    store.layout_list(list);
    assert_eq!(
        store.scroll_pane(list).unwrap().content_size(),
        Size::new(80., 160.)
    );
    for (i, &row) in rows.iter().enumerate() {
        assert_eq!(store.objects[row].y(), i as f32 * 40.);
    }
    for (&bar, &(pos, size)) in bars.iter().zip(&bar_bounds) {
        assert_eq!(
            (store.objects[bar].position(), store.objects[bar].size()),
            (pos, size)
        );
        assert!(store.list_item_of(bar).is_none());
    }
    if let ControlData::List(data) = &mut store.objects[list].control {
        data.items = (0..4)
            .map(|i| ListItemData {
                title: Some(format!("row{i}")),
                ..Default::default()
            })
            .collect();
    }
    store.apply_all_list_items();
    for (i, &row) in rows.iter().enumerate() {
        let ControlData::Button(data) = store.objects[row].control() else {
            panic!("row")
        };
        assert_eq!(data.title, format!("row{i}"));
    }
    store.set_list_selection(list, 2);
    assert_eq!(store.list_selections(list), Some(vec![2]));
    store.scroll_list_item_to_view(list, 3, false, true);
    assert!(store.scroll_pane(list).unwrap().pos().y > 0.);
}

#[test]
fn recycling_and_tree_visibility_keep_pane_owned_bars_outside_item_operations() {
    for tree in [false, true] {
        let (mut store, list, rows, bars) = fixture(tree);
        if tree {
            store.apply_tree_visibility(list, &[0, 2]);
            assert!(!store.objects[rows[1]].visible());
            assert!(bars.iter().all(|&id| store.objects[id].visible()));
        }
        let mut pool = crate::pool::ObjectPool::new();
        assert_eq!(
            store.recycle_items_outside(list, 1..3, &mut pool, "rows"),
            2
        );
        assert_eq!(store.list_items(list), Some(rows[1..3].to_vec()));
        assert!(
            bars.iter()
                .all(|&id| store.objects[id].parent() == Some(list))
        );
        assert_eq!(pool.len(), 2);
        assert!(pool.acquire("rows").is_some_and(|id| rows.contains(&id)));
        let removed = store.remove(list);
        assert!(bars.iter().all(|id| removed.contains(id)));
    }
}

#[test]
fn bar_attachment_is_idempotent_and_detaching_clears_registration_without_guessing_by_kind() {
    let (mut store, list, rows, bars) = fixture(false);
    let len = store.len();
    assert_eq!(
        store.attach_scroll_bars(list, &mut |_, _| panic!("must reuse valid bars")),
        0
    );
    assert_eq!(store.len(), len);
    store.detach_from_parent(bars[0]);
    assert_eq!(store.scroll_pane(list).unwrap().vt_bar, None);
    assert!(!store.is_scroll_bar_overlay(bars[0]));
    assert_eq!(store.list_items(list), Some(rows.clone()));
    store.set_parent(bars[0], Some(list));
    assert_eq!(
        store.list_items(list).unwrap().last(),
        Some(&bars[0]),
        "unregistered ScrollBar can be a row"
    );
    assert_eq!(store.list_item_count(list), 5);
    store.remove(bars[1]);
    assert_eq!(store.scroll_pane(list).unwrap().hz_bar, None);
    assert_eq!(
        store.attach_scroll_bars(list, &mut |store, _| Some(
            store.create(ObjectType::ScrollBar)
        )),
        2
    );
    assert_eq!(store.list_item_count(list), 5);
    store.set_list_selection_mode(list, ListSelectionMode::Multiple);
    store.select_all_list_items(list);
    assert_eq!(store.list_selections(list), Some(vec![0, 1, 2, 3]));
}

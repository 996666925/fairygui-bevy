use super::*;
use crate::{
    geom::{Margin, Size},
    object::{ListLayoutSettings, SizeConstraints},
    package::ObjectType,
    scroll::ScrollPane,
    widget::{ListItemData, TreeNodeData},
};

fn list(layout: ListLayoutType) -> (ObjectStore, GObjectId, Vec<GObjectId>) {
    let mut store = ObjectStore::new();
    let list = store.create(ObjectType::List);
    store.set_size(list, 200., 100.);
    store.set_list_layout_settings(
        list,
        ListLayoutSettings {
            layout,
            line_count: 2,
            column_count: 2,
            line_gap: 2,
            column_gap: 3,
            auto_resize_item: false,
            ..Default::default()
        },
    );
    let rows = [(40., 10.), (80., 30.), (25., 20.), (50., 15.)]
        .into_iter()
        .map(|(width, height)| {
            let row = store.create(ObjectType::Component);
            store.set_size(row, width, height);
            store.set_parent(row, Some(list));
            row
        })
        .collect();
    (store, list, rows)
}

#[test]
fn resize_to_fit_uses_the_last_included_edge_for_all_five_layouts() {
    // GList.ts resizeToFit uses the last row, not the complete line's maximum
    // extent or the content bounds (including in Pagination).
    for (layout, expected) in [
        (ListLayoutType::SingleColumn, Size::new(200., 64.)),
        (ListLayoutType::SingleRow, Size::new(151., 100.)),
        (ListLayoutType::FlowHorizontal, Size::new(200., 52.)),
        (ListLayoutType::FlowVertical, Size::new(108., 100.)),
        (ListLayoutType::Pagination, Size::new(25., 100.)),
    ] {
        let (mut store, list, rows) = list(layout);
        // Fitting must flush edits, not rely on last frame's row positions.
        store.set_xy(rows[2], 500., 500.);
        assert!(store.resize_list_to_fit(list, Some(3), 0.), "{layout:?}");
        assert_eq!(store.object(list).unwrap().size(), expected, "{layout:?}");
    }
    let (mut store, list, _) = list(ListLayoutType::FlowHorizontal);
    assert!(store.resize_list_to_fit(list, Some(1), 0.));
    assert_eq!(store.object(list).unwrap().height(), 10.);
}

#[test]
fn resize_to_fit_counts_hidden_items_but_folds_the_trailing_invisible_rows() {
    let (mut store, list, rows) = list(ListLayoutType::SingleColumn);
    store.set_list_fold_invisible_items(list, true);
    store.set_visible(rows[1], false);
    store.set_visible(rows[2], false);
    assert!(store.resize_list_to_fit(list, Some(3), 28.));
    assert_eq!(store.object(list).unwrap().height(), 28.);
    assert!(store.resize_list_to_fit(list, Some(3), 0.));
    assert_eq!(store.object(list).unwrap().height(), 10.);
    store.set_visible(rows[0], false);
    assert!(store.resize_list_to_fit(list, None, 0.));
    assert_eq!(store.object(list).unwrap().height(), 15.);
    store.set_visible(rows[3], false);
    assert!(store.resize_list_to_fit(list, None, 7.));
    assert_eq!(store.object(list).unwrap().height(), 7.);
    store.set_list_fold_invisible_items(list, false);
    assert!(store.resize_list_to_fit(list, Some(3), 0.));
    assert_eq!(store.object(list).unwrap().height(), 64.);
    assert!(store.resize_list_to_fit(list, Some(0), 9.));
    assert_eq!(store.object(list).unwrap().height(), 9.);
    assert!(!store.resize_list_to_fit(list, Some(0), 9.));
}

#[test]
fn resize_to_fit_adds_margins_and_nonfloating_gutters_and_excludes_overlays() {
    for vertical in [false, true] {
        for floating in [false, true] {
            let layout = if vertical {
                ListLayoutType::SingleColumn
            } else {
                ListLayoutType::SingleRow
            };
            let (mut store, list, rows) = list(layout);
            let vt = store.create(ObjectType::ScrollBar);
            store.set_size(vt, 11., 1000.);
            let hz = store.create(ObjectType::ScrollBar);
            store.set_size(hz, 1000., 13.);
            let mut pane = ScrollPane::default();
            pane.vt_bar = Some(vt);
            pane.hz_bar = Some(hz);
            pane.floating = floating;
            let data = store.list_data_mut(list).unwrap();
            data.component.margin = Margin::new(5., 2., 7., 3.);
            data.item_margin = data.component.margin;
            data.component.scroll_pane = Some(pane);
            store.set_parent(vt, Some(list));
            store.set_parent(hz, Some(list));
            store.set_visible(rows[1], false);
            store.set_scale(rows[1], 3., 4.);
            assert_eq!(store.list_max_item_width(list), Some(80.));
            assert!(store.resize_list_to_fit(list, Some(999), 0.));
            let pane = store.scroll_pane(list).unwrap();
            if vertical {
                // 10 + 2 + 30 + 2 + 20 + 2 + 15 = 81 logical pixels.
                assert_eq!(pane.view_size().height, 81.);
                assert_eq!(
                    store.object(list).unwrap().height(),
                    86. + if floating { 0. } else { 13. }
                );
            } else {
                assert_eq!(pane.view_size().width, 204.);
                assert_eq!(
                    store.object(list).unwrap().width(),
                    216. + if floating { 0. } else { 11. }
                );
            }
        }
    }
}

#[test]
fn resize_to_fit_preserves_constraints_and_rejects_invalid_targets() {
    let (mut store, list, _) = list(ListLayoutType::SingleColumn);
    store.set_size_constraints(
        list,
        SizeConstraints {
            min: Size::new(0., 25.),
            max: Size::new(150., 0.),
        },
    );
    assert!(store.resize_list_to_fit(list, Some(0), 10.));
    assert_eq!(store.object(list).unwrap().size(), Size::new(150., 25.));
    store.set_size_constraints(list, SizeConstraints::default());
    assert_eq!(store.object(list).unwrap().size(), Size::new(200., 10.));
    for minimum in [f32::NAN, f32::INFINITY, -1.] {
        assert!(!store.resize_list_to_fit(list, Some(3), minimum));
        assert_eq!(store.object(list).unwrap().size(), Size::new(200., 10.));
    }
    let component = store.create(ObjectType::Component);
    assert!(!store.resize_list_to_fit(component, None, 0.));
    assert_eq!(store.list_max_item_width(component), None);
    store.remove(list);
    assert!(!store.resize_list_to_fit(list, None, 0.));
    assert_eq!(store.list_max_item_width(list), None);
}

#[test]
fn resize_to_fit_and_max_width_use_visible_tree_child_order() {
    let mut store = ObjectStore::new();
    let tree = store.create(ObjectType::Tree);
    store.set_size(tree, 200., 100.);
    store.set_list_layout_settings(
        tree,
        ListLayoutSettings {
            auto_resize_item: false,
            ..Default::default()
        },
    );
    for width in [40., 500., 60.] {
        let row = store.create(ObjectType::Component);
        store.set_size(row, width, 20.);
        store.set_parent(row, Some(tree));
    }
    assert!(store.replace_tree_nodes(
        tree,
        vec![
            TreeNodeData {
                is_folder: true,
                ..Default::default()
            },
            TreeNodeData {
                level: 1,
                ..Default::default()
            },
            TreeNodeData::default(),
        ],
        vec![ListItemData::default(); 3],
        vec![false; 3],
    ));
    store.initialize_tree_cells(tree);
    assert_eq!(store.list_max_item_width(tree), Some(60.));
    assert!(store.resize_list_to_fit(tree, Some(2), 0.));
    assert_eq!(store.object(tree).unwrap().height(), 40.);
    store.set_tree_node_expanded(tree, 0, true);
    assert_eq!(store.list_max_item_width(tree), Some(500.));
    assert!(store.resize_list_to_fit(tree, None, 0.));
    assert_eq!(store.object(tree).unwrap().height(), 60.);
}

#[test]
fn virtual_resize_to_fit_uses_template_lines_not_measured_rows_or_minimum() {
    for (layout, expected) in [
        (ListLayoutType::SingleColumn, Size::new(120., 108.)),
        (ListLayoutType::SingleRow, Size::new(132., 80.)),
        (ListLayoutType::FlowHorizontal, Size::new(120., 42.)),
        (ListLayoutType::FlowVertical, Size::new(78., 80.)),
        (ListLayoutType::Pagination, Size::new(51., 80.)),
    ] {
        let mut store = ObjectStore::new();
        let list = store.create(ObjectType::List);
        store.set_size(list, 120., 80.);
        store.set_list_layout_settings(
            list,
            ListLayoutSettings {
                layout,
                column_count: 3,
                line_count: 2,
                column_gap: 3,
                line_gap: 2,
                auto_resize_item: false,
                ..Default::default()
            },
        );
        let mut pane = ScrollPane::default();
        pane.set_view_size(Size::new(120., 80.));
        store.list_data_mut(list).unwrap().component.scroll_pane = Some(pane);
        let mut create = |store: &mut ObjectStore, _: &str| {
            let row = store.create(ObjectType::Component);
            store.set_size(row, 24., 20.);
            Some(row)
        };
        store.enable_virtual_list(list, "a", &mut create).unwrap();
        store.set_virtual_item_count(list, 50).unwrap();
        store
            .refresh_virtual_list(list, &mut create, &mut |_| None, &mut |store, _, row| {
                store.set_size(row, 48., 40.)
            })
            .unwrap();
        assert_eq!(store.list_max_item_width(list), Some(48.));
        assert!(store.resize_list_to_fit(list, Some(5), 1000.), "{layout:?}");
        assert_eq!(store.object(list).unwrap().size(), expected, "{layout:?}");
        store.set_virtual_item_count(list, 0).unwrap();
        assert!(store.resize_list_to_fit(list, None, 1000.));
        let size = store.object(list).unwrap().size();
        assert_eq!(
            if matches!(
                layout,
                ListLayoutType::SingleColumn | ListLayoutType::FlowHorizontal
            ) {
                size.height
            } else {
                size.width
            },
            0.
        );
        // Native ScrollPane clamps the actual viewport to at least one pixel.
        let view = store.scroll_pane(list).unwrap().view_size();
        assert!(view.width >= 1. && view.height >= 1.);
    }
}

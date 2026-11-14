use super::*;
use crate::package::ObjectType;

#[test]
fn nested_lists_reflow_after_viewport_and_membership_changes_without_self_invalidation() {
    let mut store = ObjectStore::new();
    let outer = store.create(ObjectType::List);
    let inner = store.create(ObjectType::List);
    let other = store.create(ObjectType::List);
    store.set_size(outer, 200., 100.);
    store.set_size(inner, 100., 60.);
    store.set_size(other, 90., 100.);
    for list in [outer, inner, other] {
        let mut settings = store.list_layout_settings(list).unwrap();
        settings.auto_resize_item = true;
        store.set_list_layout_settings(list, settings);
    }
    let mut rows = Vec::new();
    for _ in 0..2 {
        let row = store.create(ObjectType::Component);
        store.set_size(row, 30., 20.);
        store.set_parent(row, Some(inner));
        rows.push(row);
    }
    store.set_parent(inner, Some(outer));
    store.flush_list_layouts();
    assert_eq!(store.object(inner).unwrap().width(), 200.);
    assert_eq!(store.object(rows[0]).unwrap().width(), 200.);
    assert_eq!(store.object(rows[1]).unwrap().y(), 20.);

    store.set_size(outer, 150., 100.);
    store.set_size(rows[0], 200., 35.);
    store.flush_list_layouts();
    assert_eq!(store.object(rows[0]).unwrap().width(), 150.);
    assert_eq!(store.object(rows[1]).unwrap().y(), 35.);
    store.set_parent(inner, Some(other));
    store.flush_list_layouts();
    assert_eq!(store.object(rows[0]).unwrap().width(), 90.);
    assert_eq!(store.list_alignment(outer), crate::Point::ZERO);
    assert_eq!(store.list_bounds[&outer], crate::Size::ZERO);
    assert_eq!(store.flush_list_layouts(), 0);
    store.take_dirty();
    store.set_alpha(rows[0], 0.5);
    assert_eq!(
        store.flush_list_layouts(),
        0,
        "appearance does not affect row geometry"
    );
    store.take_dirty();
    assert!(store.take_dirty().is_empty());

    store.set_size(inner, 80., 60.);
    store.remove(other);
    assert_eq!(
        store.flush_list_layouts(),
        0,
        "disposed subtrees leave no pending layout"
    );
}

#[test]
fn folding_tracks_later_visibility_edits_and_can_restore_all_rows() {
    let mut store = ObjectStore::new();
    let list = store.create(ObjectType::List);
    store.set_size(list, 100., 80.);
    let mut rows = Vec::new();
    for _ in 0..3 {
        let row = store.create(ObjectType::Component);
        store.set_size(row, 30., 20.);
        store.set_parent(row, Some(list));
        rows.push(row);
    }
    store.set_list_fold_invisible_items(list, true);
    store.set_visible(rows[1], false);
    store.flush_list_layouts();
    assert_eq!(store.object(rows[2]).unwrap().y(), 20.);
    assert_eq!(store.list_bounds[&list].height, 40.);
    store.set_visible(rows[1], true);
    store.set_size(rows[0], 100., 35.);
    store.flush_list_layouts();
    assert_eq!(store.object(rows[1]).unwrap().y(), 35.);
    assert_eq!(store.object(rows[2]).unwrap().y(), 55.);
    assert_eq!(store.list_bounds[&list].height, 75.);
    assert_eq!(store.list_items(list).unwrap(), rows);
    for row in rows {
        store.set_visible(row, false);
    }
    store.flush_list_layouts();
    assert_eq!(store.list_bounds[&list], crate::Size::ZERO);
    assert_eq!(store.flush_list_layouts(), 0);
}

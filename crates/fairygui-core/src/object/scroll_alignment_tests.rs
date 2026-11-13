use super::*;
use crate::{
    geom::Size,
    package::ObjectType,
    scroll::{ScrollPane, ScrollType},
};

#[test]
fn ordinary_component_scroll_bounds_include_hidden_scaled_children_and_exclude_overlays() {
    let mut store = ObjectStore::new();
    let owner = store.create(ObjectType::Button);
    store.set_size(owner, 100., 80.);
    let mut pane = ScrollPane::default();
    pane.scroll_type = ScrollType::Both;
    store.objects[owner]
        .control
        .component_mut()
        .unwrap()
        .scroll_pane = Some(pane);
    let child = store.create(ObjectType::Component);
    store.set_parent(child, Some(owner));
    store.set_xy(child, 20.25, 10.75);
    store.set_size(child, 120., 60.);
    store.set_scale(child, -2., 3.);
    store.set_visible(child, false);
    store.set_rotation(child, 45.);
    let bar = store.create(ObjectType::ScrollBar);
    store.set_parent(bar, Some(owner));
    store.set_xy(bar, 9000., 9000.);
    store.objects[owner]
        .control
        .component_mut()
        .unwrap()
        .scroll_pane
        .as_mut()
        .unwrap()
        .vt_bar = Some(bar);
    assert!(store.layout_component_scroll_content(owner));
    assert_eq!(
        store.scroll_pane(owner).unwrap().content_size(),
        Size::new(260., 191.)
    );
    assert!(!store.layout_component_scroll_content(owner));
    store.set_xy(child, -100., -100.);
    assert!(store.layout_component_scroll_content(owner));
    assert_eq!(
        store.scroll_pane(owner).unwrap().content_size(),
        Size::new(140., 80.)
    );
    store.remove(child);
    assert!(store.layout_component_scroll_content(owner));
    assert_eq!(store.scroll_pane(owner).unwrap().content_size(), Size::ZERO);
    let list = store.create(ObjectType::List);
    let mut pane = ScrollPane::default();
    pane.set_content_size(Size::new(100., 5000.));
    store.objects[list]
        .control
        .component_mut()
        .unwrap()
        .scroll_pane = Some(pane);
    assert!(!store.layout_component_scroll_content(list));
    assert_eq!(
        store.scroll_pane(list).unwrap().content_size(),
        Size::new(100., 5000.)
    );
}

#[test]
fn releasing_a_drag_refreshes_snap_geometry_after_child_changes() {
    let mut store = ObjectStore::new();
    let owner = store.create(ObjectType::List);
    let mut pane = ScrollPane::default();
    pane.snap_to_item = true;
    pane.set_view_size(Size::new(100., 80.));
    pane.set_content_size(Size::new(100., 500.));
    store.objects[owner]
        .control
        .component_mut()
        .unwrap()
        .scroll_pane = Some(pane);
    let mut rows = Vec::new();
    for y in [0., 60., 150., 300.] {
        let row = store.create(ObjectType::Component);
        store.set_parent(row, Some(owner));
        store.set_xy(row, 0., y);
        store.set_size(row, 100., 40.);
        rows.push(row);
    }
    store.begin_scroll_drag(owner);
    store.drag_scroll_by(owner, 0., -90., 0.1);
    store.set_size(rows[1], 100., 100.);
    store.end_scroll_drag_after(owner, 1.);
    assert_eq!(store.scroll_pane(owner).unwrap().pos().y, 90.);
    store.advance_scrolls(0.3);
    assert_eq!(store.scroll_pane(owner).unwrap().pos().y, 60.);
}

#[test]
fn materialized_snap_geometry_tracks_dynamic_scaled_children_excludes_bars_and_keeps_hidden_items()
{
    let mut store = ObjectStore::new();
    let owner = store.create(ObjectType::List);
    let mut pane = ScrollPane::default();
    pane.scroll_type = ScrollType::Vertical;
    pane.snap_to_item = true;
    pane.set_view_size(Size::new(100., 80.));
    pane.set_content_size(Size::new(100., 500.));
    store.objects[owner]
        .control
        .component_mut()
        .unwrap()
        .scroll_pane = Some(pane);
    let mut rows = Vec::new();
    for y in [0., 60., 150., 300.] {
        let row = store.create(ObjectType::Component);
        store.set_parent(row, Some(owner));
        store.set_xy(row, 0., y);
        store.set_size(row, 100., 40.);
        rows.push(row);
    }
    let bar = store.create(ObjectType::ScrollBar);
    store.set_parent(bar, Some(owner));
    store.set_xy(bar, 0., 500.);
    store.objects[owner]
        .control
        .component_mut()
        .unwrap()
        .scroll_pane
        .as_mut()
        .unwrap()
        .vt_bar = Some(bar);
    assert_eq!(
        store.snapping_position(owner, Point::new(0., 400.)),
        Some(Point::new(0., 300.))
    );
    store.set_visible(rows[1], false);
    store.set_scroll_pos_animated(owner, 0., 90., false);
    assert_eq!(store.scroll_pane(owner).unwrap().pos().y, 150.);
    store.set_scale(rows[1], 1., -2.);
    store.set_scroll_pos_animated(owner, 0., 90., false);
    assert_eq!(store.scroll_pane(owner).unwrap().pos().y, 60.);
    store.set_xy(rows[2], 0., 180.);
    store.scroll_mouse_wheel(owner, 1.);
    assert_eq!(store.scroll_pane(owner).unwrap().pos().y, 180.);
    store.scroll_container_by(owner, false, -1.);
    assert_eq!(
        store.scroll_pane(owner).unwrap().pos().y,
        180.,
        "step can align back to the same item"
    );
    store.set_scroll_perc(owner, true, 0.15);
    assert_eq!(store.scroll_pane(owner).unwrap().pos().y, 60.);
    store.scroll_mouse_wheel_pixels(owner, 0., 0.25);
    assert_eq!(
        store.scroll_pane(owner).unwrap().pos().y,
        60.25,
        "precise Pixel remains a backend extension"
    );
    assert_eq!(store.list_item_count(owner), 4);
}

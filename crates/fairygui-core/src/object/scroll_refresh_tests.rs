use super::*;
use crate::{
    geom::Point,
    package::{FuiPackage, LoadContext, ObjectType},
    widget::create_from_package,
};

fn fixture() -> (ObjectStore, GObjectId, GObjectId, GObjectId) {
    let package = FuiPackage::parse(
        "PullToRefresh",
        include_bytes!("../../../fairygui-demo/assets/ui/PullToRefresh.fui"),
    )
    .unwrap();
    let mut store = ObjectStore::new();
    let root = create_from_package(
        &mut store,
        &package,
        package.item_index_by_name("Main").unwrap(),
        &LoadContext::none(),
    )
    .unwrap();
    let first = store.child_by_name(root, "list1").unwrap();
    let second = store.child_by_name(root, "list2").unwrap();
    for list in [first, second] {
        assert_eq!(
            store.attach_scroll_refresh_bars(list, &mut |s, url| create_from_package(
                s,
                &package,
                package.resolve_local_url(url)?,
                &LoadContext::none()
            )
            .ok()),
            1
        );
        assert_eq!(
            store.attach_scroll_refresh_bars(list, &mut |_, _| panic!(
                "existing refresh resources must be retained"
            )),
            0
        );
        store.layout_scroll_bars(list);
    }
    store.attach_to_stage(root);
    (store, root, first, second)
}

#[test]
fn real_refresh_resources_are_owned_separately_from_rows_and_follow_display_stage_lifetime() {
    let (mut s, root, first, second) = fixture();
    let header = s.scroll_header(first).unwrap();
    let footer = s.scroll_footer(second).unwrap();
    assert_eq!(s.object(header).unwrap().source_size().height, 71.);
    assert_eq!(s.list_item_count(first), 0);
    assert!(!s.is_on_stage(header));
    assert!(!s.is_on_stage(footer));
    assert!(s.object(header).unwrap().visible());
    assert!(s.is_scroll_refresh_overlay(header));
    assert!(!s.is_scroll_bar_overlay(header));
    let row = s.create(ObjectType::Component);
    s.set_parent(row, Some(first));
    s.set_size(row, 100., 40.);
    assert_eq!(s.list_items(first), Some(vec![row]));
    assert_eq!(s.list_item_index(first, header), None);
    s.begin_scroll_drag(first);
    assert!(s.drag_scroll_by(first, 0., 180., 0.1));
    assert_eq!(s.scroll_pane(first).unwrap().display_pos().y, -90.);
    assert_eq!(s.object(header).unwrap().height(), 90.);
    assert!(s.is_on_stage(header));
    assert!(s.display_visible(header));
    assert!(s.transition_state(header, "t0").unwrap().0);
    s.end_scroll_drag(first);
    assert_eq!(s.drain_pull_releases(), vec![(first, PullRelease::Down)]);
    s.lock_scroll_header(first, 71.);
    s.advance_scrolls(0.3);
    assert_eq!(
        s.scroll_pane(first).unwrap().display_pos(),
        Point::new(0., -71.)
    );
    assert_eq!(s.scroll_pane(first).unwrap().pos(), Point::ZERO);
    assert_eq!(s.object(header).unwrap().height(), 71.);
    s.lock_scroll_header(first, 35.);
    s.advance_scrolls(0.3);
    assert_eq!(s.object(header).unwrap().height(), 35.);
    s.lock_scroll_header(first, 0.);
    s.advance_scrolls(0.3);
    assert!(!s.is_on_stage(header));
    assert!(!s.display_visible(header));
    assert!(s.object(header).unwrap().visible());
    assert!(!s.transition_state(header, "t0").unwrap().0);
    assert_eq!(s.scroll_pane(first).unwrap().display_pos(), Point::ZERO);
    s.remove(root);
    assert!(!s.contains(header));
    assert!(!s.contains(footer));
    assert!(!s.contains(row));
    assert!(s.drain_pull_releases().is_empty());
}

#[test]
fn footer_locks_fit_content_and_overflow_use_native_distinct_extents() {
    let (mut s, _, _, list) = fixture();
    let footer = s.scroll_footer(list).unwrap();
    let view = s.scroll_pane(list).unwrap().view_size();
    s.update_scroll_content_size(list, Size::new(view.width, view.height * 0.5));
    s.lock_scroll_footer(list, 71.);
    s.advance_scrolls(0.3);
    assert_eq!(s.scroll_pane(list).unwrap().display_pos(), Point::ZERO);
    assert!(s.is_on_stage(footer));
    assert_eq!(s.object(footer).unwrap().y(), view.height - 71.);
    assert_eq!(s.object(footer).unwrap().height(), 71.);
    assert_eq!(
        s.object(footer).unwrap().width(),
        400.,
        "native footer cross-axis sizing retains authored width"
    );
    s.update_scroll_content_size(list, Size::new(view.width, view.height * 2.));
    assert!(!s.is_on_stage(footer));
    let max = s.scroll_pane(list).unwrap().max_scroll_pos().y;
    s.set_scroll_pos(list, 0., max, false);
    s.begin_scroll_drag(list);
    s.drag_scroll_by(list, 0., -100., 0.1);
    s.end_scroll_drag(list);
    assert_eq!(s.drain_pull_releases(), vec![(list, PullRelease::Up)]);
    s.advance_scrolls(0.3);
    assert_eq!(s.scroll_pane(list).unwrap().display_pos().y, max + 71.);
    assert_eq!(s.scroll_pane(list).unwrap().pos().y, max);
    assert_eq!(s.object(footer).unwrap().height(), 71.);
    s.lock_scroll_footer(list, 0.);
    s.advance_scrolls(0.3);
    assert_eq!(s.scroll_pane(list).unwrap().display_pos().y, max);
    assert!(!s.is_on_stage(footer));
}

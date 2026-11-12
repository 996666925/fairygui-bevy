use super::*;
use crate::{
    geom::Size,
    package::ObjectType,
    scroll::{ScrollPane, ScrollType},
    widget::ListSelectionMode,
};

const REFERENCE: &str = include_str!("../../tests/fixtures/scroll-view-reference.txt");

fn pane(kind: ScrollType) -> ScrollPane {
    let mut p = ScrollPane::default();
    p.scroll_type = kind;
    p.set_view_size(Size::new(100., 100.));
    p.set_content_size(Size::new(500., 500.));
    p
}

#[test]
fn scroll_view_rectangle_policy_matches_324_native_branches() {
    let mut count = 0;
    for line in REFERENCE.lines().filter(|l| l.starts_with("rect ")) {
        let v: Vec<f32> = line
            .split_whitespace()
            .skip(1)
            .map(|v| v.parse().unwrap())
            .collect();
        let mut p = pane(ScrollType::from_code(v[0] as i8));
        p.page_mode = v[1] == 1.;
        p.set_pos(v[3], v[3], false);
        p.scroll_to_view(Rect::new(v[4], v[5], v[6], v[7]), false, v[2] == 1.);
        assert_eq!(p.pos(), Point::new(v[8], v[9]), "{line}");
        count += 1;
    }
    assert_eq!(count, 324);
}

#[test]
fn exported_scrollable_lists_keep_their_instance_overflow_on_the_component() {
    let package =
        crate::FuiPackage::parse("Basics", include_bytes!("../../tests/fixtures/Basics.fui"))
            .unwrap();
    let mut store = ObjectStore::new();
    let root = crate::widget::create_from_package(
        &mut store,
        &package,
        package.item_index_by_name("Demo_List").unwrap(),
        &crate::package::LoadContext::none(),
    )
    .unwrap();
    let mut count = 0;
    store.for_each_descendant(root, &mut |id| {
        if let ControlData::List(data) = store.object(id).unwrap().control()
            && data.component.scroll_pane.is_some()
        {
            assert_eq!(data.component.overflow, crate::widget::OverflowType::Scroll);
            count += 1;
        }
    });
    assert!(count > 0);
}

#[test]
fn scroll_view_tween_matches_native_logical_and_display_positions_with_retargets() {
    let mut p = pane(ScrollType::Both);
    p.set_content_size(Size::new(600., 600.));
    let mut count = 0;
    for line in REFERENCE.lines().filter(|l| l.starts_with("tween ")) {
        let (command, expected) = line.split_once(':').unwrap();
        let command: Vec<_> = command.split_whitespace().collect();
        let expected: Vec<f32> = expected
            .split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect();
        if command[1] == "set" {
            p.set_pos_animated(
                command[2].parse().unwrap(),
                command[3].parse().unwrap(),
                command[4] == "1",
            );
        } else {
            p.advance(command[2].parse().unwrap());
        }
        assert_eq!(
            p.pos(),
            Point::new(expected[0], expected[1]),
            "logical {line}"
        );
        assert_eq!(
            p.display_pos(),
            Point::new(expected[2], expected[3]),
            "display {line}"
        );
        assert_eq!(p.is_animating(), expected[4] == 1., "state {line}");
        count += 1;
    }
    assert_eq!(count, 13);
}

fn scroll_container(
    store: &mut ObjectStore,
    kind: ObjectType,
    width: f32,
    height: f32,
) -> GObjectId {
    let id = store.create(kind);
    store.set_size(id, width, height);
    store.objects[id]
        .control
        .component_mut()
        .unwrap()
        .scroll_pane = Some(pane(ScrollType::Both));
    id
}

#[test]
fn scroll_view_nested_rectangles_match_native_rotation_scale_pivot_and_skew() {
    let mut count = 0;
    for line in REFERENCE.lines().filter(|l| l.starts_with("geometry ")) {
        let v: Vec<f32> = line
            .split_whitespace()
            .skip(1)
            .map(|s| s.parse().unwrap())
            .collect();
        let mut store = ObjectStore::new();
        let root = store.create(ObjectType::Component);
        store.set_xy(root, 35., 45.);
        store.set_size(root, 900., 900.);
        store.set_scale(root, 1.2, 0.9);
        store.set_rotation(root, 12.);
        let owner = scroll_container(&mut store, ObjectType::Component, 112., 114.);
        store.set_parent(owner, Some(root));
        store.set_xy(owner, 70., 80.);
        store.set_scale(owner, 1.1, 1.3);
        store.set_rotation(owner, 8.);
        // Match the native owner's 100x100 pivot dimensions before margins.
        store.set_size(owner, 100., 100.);
        store.set_pivot(owner, 0.2, 0.3, true);
        {
            let c = store.objects[owner].control.component_mut().unwrap();
            c.margin.left = 5.;
            c.margin.top = 7.;
            c.margin.right = -5.;
            c.margin.bottom = -7.;
            c.scroll_pane
                .as_mut()
                .unwrap()
                .set_content_size(Size::new(1000., 1000.));
        }
        store.set_scroll_pos(owner, 20., 30., false);
        let wrapper = store.create(ObjectType::Component);
        store.set_parent(wrapper, Some(owner));
        store.set_xy(wrapper, 110., 160.);
        store.set_size(wrapper, 200., 120.);
        store.set_scale(wrapper, v[0], v[1]);
        store.set_rotation(wrapper, v[2]);
        store.set_pivot(wrapper, v[4], v[5], v[3] == 1.);
        store.objects[wrapper].skew_x = v[6];
        store.objects[wrapper].skew_y = v[7];
        let row = store.create(ObjectType::Button);
        store.set_parent(row, Some(wrapper));
        store.set_xy(row, 15., 25.);
        store.set_size(row, 30., 40.);
        store.set_scale(row, 5., 5.);
        store.set_rotation(row, 45.);
        let actual = store
            .scroll_relative_rect(wrapper, owner, Rect::new(15., 25., 30., 40.))
            .unwrap();
        for (a, e) in [actual.x, actual.y, actual.width, actual.height]
            .into_iter()
            .zip(&v[8..12])
        {
            assert!((a - e).abs() < 0.001, "rectangle {line}: {actual:?}");
        }
        store.scroll_object_to_view(owner, row, false, true);
        let pos = store.scroll_pane(owner).unwrap().pos();
        assert!(
            (pos.x - v[12]).abs() < 0.001 && (pos.y - v[13]).abs() < 0.001,
            "endpoint {line}: {pos:?}"
        );
        count += 1;
    }
    assert_eq!(count, 7);
}

#[test]
fn scroll_view_list_fallback_click_flag_and_selection_none_follow_native_routes() {
    let mut store = ObjectStore::new();
    let parent = scroll_container(&mut store, ObjectType::Component, 100., 100.);
    let list = store.create(ObjectType::List);
    store.set_parent(list, Some(parent));
    store.set_xy(list, 20., 150.);
    store.set_size(list, 80., 300.);
    let rows: Vec<_> = (0..8)
        .map(|_| {
            let row = store.create(ObjectType::Component);
            store.set_parent(row, Some(list));
            store.set_size(row, 80., 40.);
            row
        })
        .collect();
    assert!(store.scroll_list_item_to_view(list, 2, false, true));
    assert_eq!(store.scroll_pane(parent).unwrap().pos().y, 230.);
    assert!(
        !store.scroll_clicked_list_item_to_view(rows[5]),
        "click does not use the parent's pane"
    );
    store.objects[list]
        .control
        .component_mut()
        .unwrap()
        .scroll_pane = Some(pane(ScrollType::Vertical));
    store.set_size(list, 80., 100.);
    store.set_list_selection_mode(list, ListSelectionMode::None);
    assert!(
        store.scroll_clicked_list_item_to_view(rows[5]),
        "non-button None-mode row still scrolls"
    );
    let p = store.scroll_pane(list).unwrap();
    assert_eq!(p.pos().y, 180.);
    assert_eq!(p.display_pos().y, 0.);
    assert!(p.is_animating());
    store.set_list_scroll_item_to_view_on_click(list, false);
    assert!(!store.scroll_clicked_list_item_to_view(rows[7]));
    assert_eq!(store.scroll_pane(list).unwrap().pos().y, 180.);
    assert!(!store.scroll_list_item_to_view(list, 100, false, true));
}

#[test]
fn scroll_view_animation_interruptions_resize_and_invalid_inputs_preserve_finite_state() {
    let mut p = pane(ScrollType::Both);
    assert!(p.set_pos_animated(350., 300., true));
    p.advance(0.125);
    assert!(p.display_pos().x > 0. && p.display_pos().x < p.pos().x);
    let before = p.display_pos();
    assert!(p.display_perc_x() > 0. && p.display_perc_x() < p.perc_x());
    assert!(!p.set_pos_animated(350., 300., true));
    assert_eq!(p.display_pos(), before);
    assert!(!p.advance(f32::NAN));
    assert!(!p.scroll_to_view(Rect::new(f32::INFINITY, 0., 1., 1.), true, false));
    p.begin_drag();
    assert!(!p.is_animating());
    assert_eq!(p.display_pos(), p.pos());
    p.end_drag();
    p.set_pos_animated(100., 150., true);
    p.stop();
    assert_eq!(p.display_pos(), p.pos());
    p.set_pos_animated(350., 300., true);
    p.set_content_size(Size::new(120., 130.));
    assert!(!p.is_animating());
    assert_eq!(p.display_pos(), Point::new(20., 30.));
    p.set_content_size(Size::new(500., 500.));
    p.set_pos_animated(200.5, 200.5, true);
    p.advance(0.125);
    p.begin_drag();
    p.drag(0., 10., 1. / 60.);
    assert_eq!(
        p.pos().y,
        190.,
        "drag starts at the displayed endpoint after native tween cancellation"
    );
    p.end_drag();
    p.set_pos_animated(10., 10., false);
    p.advance(0.125);
    assert_eq!(
        p.pos(),
        Point::new(10., 10.),
        "explicit reveal cancels prior inertia"
    );
}

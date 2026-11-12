use super::*;
use crate::{
    geom::Size,
    package::ObjectType,
    scroll::{BAR_NAME, ScrollPane, ScrollType},
};

struct Fixture {
    store: ObjectStore,
    list: GObjectId,
    bar: GObjectId,
    track: GObjectId,
    grip: GObjectId,
    decor: GObjectId,
    arrow1: GObjectId,
    arrow2: GObjectId,
}

#[test]
fn registered_grip_holds_one_axis_and_stale_release_does_not_cancel_a_replacement() {
    let mut store = ObjectStore::new();
    let list = store.create(ObjectType::List);
    store.set_size(list, 300., 300.);
    let [vertical, horizontal] = super::super::list_items::tests::bars(&mut store, list);
    store.layout_scroll_bars(list);
    store.update_scroll_content_size(list, Size::new(1000., 1000.));
    store.set_scroll_bar_display(list, crate::scroll::ScrollBarDisplayType::Auto);
    store.sync_scroll_bars(list);
    assert!(!store.objects[vertical].visible());
    store.begin_scroll_drag(list);
    store.end_scroll_drag(list);
    assert!(store.set_scroll_bar_grip_dragging(vertical, true));
    store.advance_scrolls(0.75);
    store.sync_scroll_bars(list);
    assert_eq!(store.objects[vertical].alpha(), 1.);
    assert_eq!(store.objects[horizontal].alpha(), 0.25);
    store.advance_scrolls(0.25);
    store.sync_scroll_bars(list);
    assert!(store.objects[vertical].visible());
    assert!(!store.objects[horizontal].visible());
    assert_eq!(store.objects[horizontal].alpha(), 1.);
    // External display writes stay authoritative at the next native refresh.
    store.set_visible(vertical, false);
    store.set_scroll_bar_grip_dragging(vertical, false);
    assert!(!store.objects[vertical].visible());
    store.set_scroll_bar_grip_dragging(vertical, true);
    assert!(store.objects[vertical].visible());
    store.detach_from_parent(vertical);
    assert!(!store.scroll_pane(list).unwrap().grip_dragging(true));
    assert_eq!(
        store.attach_scroll_bars(list, &mut |store, _| Some(
            store.create(ObjectType::ScrollBar)
        )),
        1
    );
    let replacement = store.scroll_pane(list).unwrap().vt_bar.unwrap();
    assert!(store.set_scroll_bar_grip_dragging(replacement, true));
    assert!(!store.set_scroll_bar_grip_dragging(vertical, false));
    assert!(store.scroll_pane(list).unwrap().grip_dragging(true));
    store.remove(list);
    assert!(!store.set_scroll_bar_grip_dragging(replacement, false));
}

#[test]
fn demand_flag_and_arrow_minimum_size_control_availability_independently_of_grip_size() {
    for demand in [false, true] {
        for fits in [false, true] {
            let mut f = fixture(true, false, 0.);
            f.store.objects[f.list]
                .control
                .component_mut()
                .unwrap()
                .scroll_pane
                .as_mut()
                .unwrap()
                .display_in_demand = demand;
            f.store
                .update_scroll_content_size(f.list, Size::new(100., if fits { 50. } else { 300. }));
            f.store.sync_scroll_bars(f.list);
            assert_eq!(f.store.objects[f.bar].visible(), !demand || !fits);
            let pane = f.store.objects[f.list]
                .control
                .component_mut()
                .unwrap()
                .scroll_pane
                .as_mut()
                .unwrap();
            pane.set_view_size(Size::new(20., 20.));
            f.store.sync_scroll_bars(f.list);
            assert!(
                !f.store.objects[f.bar].visible(),
                "view == two arrow heights"
            );
            f.store.objects[f.list]
                .control
                .component_mut()
                .unwrap()
                .scroll_pane
                .as_mut()
                .unwrap()
                .set_view_size(Size::new(20., 21.));
            f.store.sync_scroll_bars(f.list);
            assert!(
                f.store.objects[f.bar].visible(),
                "content now exceeds 21 and arrows fit"
            );
        }
    }
}

fn fixture(vertical: bool, page: bool, initial: f32) -> Fixture {
    let mut store = ObjectStore::new();
    let list = store.create(ObjectType::List);
    let mut pane = ScrollPane::default();
    pane.scroll_type = if vertical {
        ScrollType::Vertical
    } else {
        ScrollType::Horizontal
    };
    pane.page_mode = page;
    pane.vertical_bar_res = Some("bar".into());
    pane.horizontal_bar_res = Some("bar".into());
    pane.set_view_size(Size::new(100., 100.));
    pane.set_content_size(Size::new(320., 320.));
    pane.set_pos(
        if vertical { 0. } else { initial },
        if vertical { initial } else { 0. },
        false,
    );
    store.objects[list]
        .control
        .component_mut()
        .unwrap()
        .scroll_pane = Some(pane);
    store.attach_scroll_bars(list, &mut |store, _| {
        Some(store.create(ObjectType::ScrollBar))
    });
    let pane = store.scroll_pane(list).unwrap();
    let bar = if vertical {
        pane.vt_bar.unwrap()
    } else {
        pane.hz_bar.unwrap()
    };
    store.set_xy(bar, 30., 40.);
    store.set_size(
        bar,
        if vertical { 10. } else { 120. },
        if vertical { 120. } else { 10. },
    );
    let mut part = |name, parent, axis, length| {
        let id = store.create(ObjectType::Component);
        store.set_name(id, name);
        store.set_parent(id, Some(parent));
        store.set_xy(
            id,
            if vertical { 0. } else { axis },
            if vertical { axis } else { 0. },
        );
        store.set_size(
            id,
            if vertical { 10. } else { length },
            if vertical { length } else { 10. },
        );
        id
    };
    let track = part(BAR_NAME, bar, 10., 100.);
    let grip = part(GRIP_NAME, bar, 10. + 69. * initial / 220., 31.);
    let arrow1 = part(ARROW1_NAME, bar, 0., 10.);
    let arrow2 = part(ARROW2_NAME, bar, 110., 10.);
    let decor = part("decor", track, 4., 80.);
    Fixture {
        store,
        list,
        bar,
        track,
        grip,
        decor,
        arrow1,
        arrow2,
    }
}

#[test]
fn scrollbar_presses_match_216_native_down_and_up_cases() {
    let mut count = 0;
    for line in include_str!("../../tests/fixtures/scroll-bar-input-reference.txt")
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
    {
        let fields: Vec<_> = line.split_whitespace().collect();
        let number = |i: usize| fields[i].parse::<f32>().unwrap();
        let vertical = fields[0] == "1";
        let mut f = fixture(vertical, fields[1] == "1", number(2));
        let target = match fields[3] {
            "root" => f.bar,
            "track" => f.track,
            "decor" => f.decor,
            "grip" => f.grip,
            "arrow1" => f.arrow1,
            "arrow2" => f.arrow2,
            _ => panic!("{line}"),
        };
        assert_eq!(f.store.registered_scroll_bar_of(target), Some(f.bar));
        assert_eq!(
            f.store.scroll_bar_pointer_down_stop(target).is_some(),
            fields[9] == "1",
            "{line}"
        );
        assert!(
            f.store
                .press_scroll_bar(target, Some(Point::new(number(5), number(6))))
        );
        let pos = f.store.scroll_pane(f.list).unwrap().pos();
        assert_eq!(if vertical { pos.y } else { pos.x }, number(7), "{line}");
        assert_eq!(number(7), number(8), "native up adds no action: {line}");
        assert_eq!(fields[10] == "1", target == f.grip);
        assert_eq!(fields[11], "0");
        count += 1;
    }
    assert_eq!(count, 216);
}

#[test]
fn background_direction_uses_grip_local_origin_including_rotation_reflection_and_nested_hits() {
    for vertical in [false, true] {
        let mut f = fixture(vertical, false, 110.);
        // Track 100, grip 31: origin is 34.5% of the track at 50% scroll.
        // A point at 35.5% is after the grip origin, despite being below 50%.
        let point = if vertical {
            Point::new(5., 35.5)
        } else {
            Point::new(35.5, 5.)
        };
        assert!(f.store.press_scroll_bar(f.track, Some(point)));
        let pos = f.store.scroll_pane(f.list).unwrap().pos();
        assert_eq!(if vertical { pos.y } else { pos.x }, 210.);
    }
    for rotate in [false, true] {
        let mut f = fixture(true, false, 110.);
        f.store.set_xy(f.grip, 0., 40.);
        if rotate {
            f.store.set_rotation(f.grip, 90.);
        } else {
            f.store.set_scale(f.grip, 1., -1.);
        }
        assert!(f.store.press_scroll_bar(f.bar, Some(Point::new(5., 45.))));
        assert_eq!(f.store.scroll_pane(f.list).unwrap().pos().y, 10.);
    }
    let mut f = fixture(true, false, 110.);
    f.store.set_xy(f.grip, 0., 40.);
    f.store.set_xy(f.decor, 0., 6.);
    f.store.set_scale(f.decor, 1., -2.);
    assert!(
        f.store
            .press_scroll_bar(f.decor, Some(Point::new(5., -20.)))
    );
    assert_eq!(f.store.scroll_pane(f.list).unwrap().pos().y, 210.);
}

#[test]
fn invalid_geometry_stale_handles_and_unregistered_bars_do_not_scroll() {
    let mut f = fixture(true, false, 110.);
    for point in [
        None,
        Some(Point::new(f32::NAN, 0.)),
        Some(Point::new(0., f32::INFINITY)),
    ] {
        assert!(f.store.press_scroll_bar(f.track, point));
        assert_eq!(f.store.scroll_pane(f.list).unwrap().pos().y, 110.);
    }
    f.store.set_scale(f.grip, 0., 0.);
    assert!(f.store.press_scroll_bar(f.track, Some(Point::new(5., 90.))));
    assert_eq!(f.store.scroll_pane(f.list).unwrap().pos().y, 110.);
    assert!(f.store.press_scroll_bar(f.arrow1, None));
    assert_eq!(f.store.scroll_pane(f.list).unwrap().pos().y, 85.);
    f.store.remove(f.grip);
    assert!(f.store.press_scroll_bar(f.track, Some(Point::new(5., 90.))));
    assert_eq!(f.store.scroll_pane(f.list).unwrap().pos().y, 85.);
    f.store.detach_from_parent(f.bar);
    assert!(f.store.registered_scroll_bar_of(f.track).is_none());
    assert!(!f.store.press_scroll_bar(f.track, Some(Point::new(5., 90.))));
    f.store.set_parent(f.bar, Some(f.list));
    assert!(
        !f.store.press_scroll_bar(f.arrow2, None),
        "an unregistered row is not a pane bar"
    );
    f.store.remove(f.bar);
    assert!(!f.store.press_scroll_bar(f.track, Some(Point::ZERO)));
}

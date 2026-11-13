use super::*;
use crate::{FuiPackage, package::LoadContext, widget::create_from_package};

fn fixture(name: &str) -> (ObjectStore, GObjectId) {
    let package =
        FuiPackage::parse("Basics", include_bytes!("../../tests/fixtures/Basics.fui")).unwrap();
    let mut store = ObjectStore::new();
    let owner = create_from_package(
        &mut store,
        &package,
        package.item_index_by_name(name).unwrap(),
        &LoadContext::none(),
    )
    .unwrap();
    (store, owner)
}
fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 0.001, "{actual} != {expected}");
}

#[test]
fn exported_horizontal_and_vertical_bars_fill_and_grips_follow_relations_before_first_view() {
    for (name, vertical) in [("Slider_HZ", false), ("Slider_VT", true)] {
        let (mut store, owner) = fixture(name);
        let bar = store
            .child_by_name(owner, if vertical { "bar_v" } else { "bar" })
            .unwrap();
        let grip = store.child_by_name(owner, "grip").unwrap();
        let start = store.object(bar).unwrap().position();
        near(
            if vertical {
                store.object(bar).unwrap().height()
            } else {
                store.object(bar).unwrap().width()
            } as f64,
            50.,
        );
        let initial_grip = store.object(grip).unwrap().position();
        store.set_slider_range(owner, 0., 1.);
        for value in [0.25, 0., 1., 0.75, 0.25] {
            store.set_slider_value(owner, value);
            let object = store.object(bar).unwrap();
            assert_eq!(
                object.position(),
                start,
                "normal slider fills its bar instead of moving it"
            );
            near(
                if vertical {
                    object.height()
                } else {
                    object.width()
                } as f64,
                value * 100.,
            );
            let grip_pos = store.object(grip).unwrap().position();
            near(
                if vertical {
                    grip_pos.y - initial_grip.y
                } else {
                    grip_pos.x - initial_grip.x
                } as f64,
                (value - 0.5) * 100.,
            );
        }
        let size = store.object(owner).unwrap().size();
        store.set_size(
            owner,
            size.width + if vertical { 0. } else { 100. },
            size.height + if vertical { 100. } else { 0. },
        );
        near(
            if vertical {
                store.object(bar).unwrap().height()
            } else {
                store.object(bar).unwrap().width()
            } as f64,
            50.,
        );
        store.remove(owner);
        assert!(!store.slider_bars.contains_key(&owner));
    }
}

#[test]
fn fractional_and_out_of_range_programmatic_values_do_not_round_or_clamp() {
    let (mut store, owner) = fixture("Slider1");
    let title = store.child_by_name(owner, "title").unwrap();
    store.set_slider_range(owner, -2., 2.);
    store.set_slider_whole_numbers(owner, true);
    store.set_slider_title_type(owner, 1);
    store.set_slider_value(owner, -1.5);
    assert_eq!(store.slider_value(owner), Some(-1.5));
    assert_eq!(store.object_text(title), Some("-1.5/2"));
    store.set_slider_percent_from_input(owner, 0.125);
    assert_eq!(
        store.slider_value(owner),
        Some(-1.),
        "JS Math.round rounds negative halves toward positive infinity"
    );
    store.set_slider_whole_numbers(owner, false);
    store.set_slider_percent_from_input(owner, 0.1234);
    near(store.slider_value(owner).unwrap(), -1.5064);
    store.set_slider_value(owner, 12.345);
    assert_eq!(store.slider_percent(owner), Some(1.));
    assert_eq!(store.slider_value(owner), Some(12.345));
    assert!(!store.set_slider_value(owner, f64::NAN));
    assert!(!store.set_slider_range(owner, 10., -10.));
    assert_eq!(store.slider_value(owner), Some(12.345));
    store.set_slider_range(owner, 2., 2.);
    assert_eq!(store.slider_percent(owner), Some(0.));
}

#[test]
fn track_click_is_relative_to_the_actual_grip_and_obeys_change_on_click() {
    for (name, vertical) in [("Slider_HZ", false), ("Slider_VT", true)] {
        let (mut store, owner) = fixture(name);
        let grip = store.child_by_name(owner, "grip").unwrap();
        let position = store.object(grip).unwrap().position();
        let point = Point::new(
            position.x + if vertical { 0. } else { 25. },
            position.y + if vertical { 25. } else { 0. },
        );
        store.set_slider_change_on_click(owner, false);
        assert!(!store.click_slider_at(owner, point));
        assert_eq!(store.slider_value(owner), Some(50.));
        store.set_slider_change_on_click(owner, true);
        assert!(store.click_slider_at(owner, point));
        near(store.slider_value(owner).unwrap(), 75.);
        let ControlData::Slider(data) = &mut store.objects[owner].control else {
            unreachable!()
        };
        data.reverse = true;
        store.set_slider_value(owner, 25.);
        let bar = store
            .child_by_name(owner, if vertical { "bar_v" } else { "bar" })
            .unwrap();
        let object = store.object(bar).unwrap();
        near(
            if vertical {
                object.height()
            } else {
                object.width()
            } as f64,
            25.,
        );
        near(
            if vertical { object.y() } else { object.x() } as f64,
            if vertical { 82. } else { 80. },
        );
    }
}

#[test]
fn grip_press_stops_on_the_exported_part_but_tracks_and_detached_skins_do_not() {
    let (mut store, owner) = fixture("Slider_HZ");
    let grip = store.child_by_name(owner, "grip").unwrap();
    let bar = store.child_by_name(owner, "bar").unwrap();
    let skin = store.create(ObjectType::Component);
    store.set_parent(skin, Some(grip));
    assert_eq!(store.slider_pointer_down_stop(skin), Some(grip));
    assert_eq!(store.slider_pointer_down_stop(grip), Some(grip));
    assert_eq!(store.slider_pointer_down_stop(bar), None);
    assert_eq!(store.slider_pointer_down_stop(owner), None);
    store.set_parent(skin, None);
    assert_eq!(store.slider_pointer_down_stop(skin), None);
}

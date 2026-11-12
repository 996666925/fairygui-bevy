use super::*;
use crate::{geom::Size, package::ObjectType, scroll::ScrollType};

fn pane(
    store: &mut ObjectStore,
    parent: Option<GObjectId>,
    horizontal: bool,
    enabled: bool,
    ratio: f32,
) -> GObjectId {
    let id = store.create(ObjectType::Component);
    store.set_parent(id, parent);
    let mut p = ScrollPane::default();
    p.scroll_type = if horizontal {
        ScrollType::Horizontal
    } else {
        ScrollType::Vertical
    };
    p.set_view_size(Size::new(100., 80.));
    p.set_content_size(Size::new(500., 480.));
    p.set_mouse_wheel_enabled(enabled);
    let range = p.max_scroll_pos();
    p.set_pos(range.x * ratio, range.y * ratio, false);
    store.objects[id]
        .control
        .component_mut()
        .unwrap()
        .scroll_pane = Some(p);
    id
}

#[test]
fn default_bubbling_matches_24_native_nested_pane_cases_including_disabled_and_boundaries() {
    let mut count = 0;
    for line in include_str!("../../tests/fixtures/scroll-wheel-reference.txt")
        .lines()
        .filter(|s| s.starts_with("bubble "))
    {
        let v: Vec<f32> = line
            .split_whitespace()
            .skip(1)
            .map(|s| s.parse().unwrap())
            .collect();
        let mut store = ObjectStore::new();
        let outer = pane(&mut store, None, false, v[1] != 0., 0.5);
        let inner = pane(
            &mut store,
            Some(outer),
            true,
            v[0] != 0.,
            if v[2] != 0. { 1. } else { 0.5 },
        );
        let leaf = store.create(ObjectType::Graph);
        store.set_parent(leaf, Some(inner));
        store.take_dirty();
        let moved = store.scroll_mouse_wheel(leaf, v[3] * 0.025);
        let a = store.scroll_pane(inner).unwrap().pos();
        let b = store.scroll_pane(outer).unwrap().pos();
        assert_eq!(a, crate::geom::Point::new(v[4], v[5]), "{line}");
        assert_eq!(b, crate::geom::Point::new(v[6], v[7]), "{line}");
        let dirty = store
            .take_dirty()
            .into_iter()
            .filter(|(_, f)| f & dirty::SCROLL != 0)
            .count();
        assert_eq!(dirty, moved, "each moved pane must be projected");
        count += 1;
    }
    assert_eq!(count, 24);
}

#[test]
fn runtime_settings_control_wheel_arrow_step_and_precise_pixel_bubbling() {
    let mut store = ObjectStore::new();
    let outer = pane(&mut store, None, false, true, 0.);
    let inner = pane(&mut store, Some(outer), true, true, 0.);
    assert_eq!(store.scroll_mouse_wheel_enabled(inner), Some(true));
    assert!(store.set_scroll_step(inner, 12.5));
    assert_eq!(store.scroll_step(inner), Some(12.5));
    assert!(store.scroll_container_by(inner, true, 1.));
    assert_eq!(store.scroll_pane(inner).unwrap().pos().x, 12.5);
    assert_eq!(store.scroll_pane(inner).unwrap().display_pos().x, 13.);
    assert_eq!(store.scroll_mouse_wheel(inner, 1.), 2);
    assert_eq!(store.scroll_pane(inner).unwrap().pos().x, 37.5);
    assert!(store.set_scroll_step(inner, 0.));
    assert_eq!(store.scroll_step(inner), Some(crate::scroll::SCROLL_STEP));
    assert!(store.set_scroll_mouse_wheel_enabled(inner, false));
    assert_eq!(store.scroll_mouse_wheel_pixels(inner, 0., 0.25), 1);
    assert_eq!(store.scroll_pane(outer).unwrap().pos().y, 50.25);
    assert_eq!(store.scroll_pane(outer).unwrap().display_pos().y, 50.25);
    store.set_scroll_mouse_wheel_enabled(inner, true);
    assert_eq!(store.scroll_mouse_wheel_pixels(inner, 0., 0.5), 2);
    assert_eq!(store.scroll_pane(inner).unwrap().pos().x, 38.);
    let missing = store.create(ObjectType::Graph);
    store.remove(missing);
    assert_eq!(store.scroll_mouse_wheel(missing, 1.), 0);
    assert_eq!(store.scroll_step(missing), None);
    assert!(!store.set_scroll_step(missing, 5.));
    assert_eq!(store.scroll_mouse_wheel_enabled(missing), None);
    assert!(!store.set_scroll_mouse_wheel_enabled(missing, true));
}

use super::*;
use crate::{
    binary::{ByteBuffer, StringTable},
    geom::Size,
    scroll::{SCROLL_STEP, ScrollBarDisplayType, ScrollDefaults, ScrollType},
};

pub(crate) fn pane(
    direction: i8,
    display: i8,
    page: bool,
    step: f32,
    cw: f32,
    ch: f32,
    ratio: f32,
) -> ScrollPane {
    let mut bytes = vec![direction as u8, display as u8];
    bytes.extend_from_slice(&(if page { 8_i32 } else { 0 }).to_be_bytes());
    bytes.push(0);
    for _ in 0..4 {
        bytes.extend_from_slice(&65534_u16.to_be_bytes());
    }
    let mut p = ScrollPane::default();
    p.setup(&mut ByteBuffer::new(&bytes), &StringTable::default())
        .unwrap();
    p.set_scroll_step(step);
    p.set_view_size(Size::new(100., 80.));
    p.set_content_size(Size::new(cw, ch));
    let range = p.max_scroll_pos();
    p.set_pos(range.x * ratio, range.y * ratio, false);
    p
}

#[test]
fn wheel_setup_steps_axes_pages_and_boundaries_match_1422_native_cases() {
    let mut count = 0;
    for line in include_str!("../../tests/fixtures/scroll-wheel-reference.txt")
        .lines()
        .filter(|s| s.starts_with("wheel "))
    {
        let v: Vec<f32> = line
            .split_whitespace()
            .skip(1)
            .map(|s| s.parse().unwrap())
            .collect();
        let mut p = pane(v[0] as i8, v[1] as i8, v[3] != 0., v[4], v[5], v[6], v[7]);
        assert_eq!(p.mouse_wheel_enabled(), v[1] != 3., "{line}");
        assert_eq!(p.scroll_step(), if v[4] == 0. { SCROLL_STEP } else { v[4] });
        if v[2] >= 0. {
            p.set_mouse_wheel_enabled(v[2] != 0.);
        }
        let changed = p.mouse_wheel(v[8] * 0.025);
        assert_eq!(changed, v[11] != 0., "{line}");
        assert_eq!(p.pos(), Point::new(v[9], v[10]), "{line}");
        assert_eq!(p.display_pos(), Point::new(v[12], v[13]), "{line}");
        count += 1;
    }
    assert_eq!(count, 1422);
}

#[test]
fn invalid_zero_and_boundary_wheel_preserve_animation_but_real_wheel_cancels_it() {
    let mut p = pane(1, 1, false, 25., 100., 180., 0.);
    p.set_pos_animated(0., 100., true);
    for delta in [0., f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 1.] {
        assert!(!p.mouse_wheel(delta));
        assert!(p.is_animating());
    }
    assert!(p.mouse_wheel(-1.));
    assert!(!p.is_animating());
    assert_eq!(p.pos().y, 50.);
    assert_eq!(p.display_pos().y, 50.);
    for step in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(!p.set_scroll_step(step));
    }
    assert_eq!(p.scroll_step(), 25.);
    let mut p = pane(2, 2, false, 25., 100.25, 80.75, 0.);
    assert_eq!(p.max_scroll_pos(), Point::new(1., 1.));
    p.set_pos(0.25, 0., false);
    assert!(p.mouse_wheel(1.));
    assert_eq!(p.pos(), Point::new(0.25, 1.));
    assert_eq!(p.display_pos(), Point::new(1., 1.));
    p.advance(0.1);
    assert_eq!(
        p.pos(),
        Point::new(0.25, 1.),
        "ceiled range must not bounce back"
    );
    assert!(
        !p.bars_visible(),
        "immediate wheel does not reveal hidden Auto bars"
    );
    assert!(p.set_scroll_step(f32::MAX));
    assert!(p.mouse_wheel(-1.));
    assert_eq!(p.pos().y, 0.);
}

#[test]
fn smooth_pixels_keep_precision_with_horizontal_fallback_and_native_page_policy() {
    let mut p = pane(2, 1, false, 25., 500., 480., 0.);
    assert!(p.mouse_wheel_pixels(0.25, 0.5));
    assert_eq!(p.pos(), Point::new(0.25, 0.5));
    assert_eq!(p.display_pos(), p.pos());
    p.set_mouse_wheel_enabled(false);
    assert!(!p.mouse_wheel_pixels(1., 1.));
    p.set_mouse_wheel_enabled(true);
    assert!(!p.mouse_wheel_pixels(f32::NAN, 1.));
    assert!(!p.mouse_wheel_pixels(0., 0.));
    let mut p = pane(0, 1, false, 25., 500., 80., 0.);
    assert!(p.mouse_wheel_pixels(0., 0.5));
    assert_eq!(p.pos().x, 0.5);
    assert!(p.mouse_wheel_pixels(0.25, 20.));
    assert_eq!(p.pos().x, 0.75);
    p.page_mode = true;
    assert!(p.mouse_wheel_pixels(0., 0.1));
    assert_eq!(
        p.pos().x,
        100.,
        "page-mode Pixel uses the complete native refresh alignment"
    );
    assert_eq!(p.scroll_type, ScrollType::Horizontal);
    assert_eq!(p.bar_display, ScrollBarDisplayType::Visible);
}

#[test]
fn scroll_setup_uses_global_defaults_only_when_the_export_keeps_defaults() {
    for default_display in [ScrollBarDisplayType::Auto, ScrollBarDisplayType::Hidden] {
        for exported_display in 0..=3 {
            for flags in [0_i32, 16 | 64, 32 | 128, 16 | 32 | 64 | 128] {
                for effects in [false, true] {
                    let defaults = ScrollDefaults {
                        bar_display: default_display,
                        touch_effect: effects,
                        bounceback_effect: effects,
                        scroll_step: 12.5,
                        paging_threshold: 0.45,
                    };
                    let mut bytes = vec![1, exported_display];
                    bytes.extend_from_slice(&flags.to_be_bytes());
                    bytes.push(0);
                    for _ in 0..4 {
                        bytes.extend_from_slice(&65534_u16.to_be_bytes());
                    }
                    let mut pane = ScrollPane::default();
                    pane.setup_with_defaults(
                        &mut ByteBuffer::new(&bytes),
                        &StringTable::default(),
                        defaults,
                    )
                    .unwrap();
                    let display = if exported_display == 0 {
                        default_display
                    } else {
                        ScrollBarDisplayType::from_code(exported_display as i8)
                    };
                    assert_eq!(pane.bar_display, display);
                    assert_eq!(
                        pane.mouse_wheel_enabled(),
                        display != ScrollBarDisplayType::Hidden
                    );
                    assert_eq!(
                        pane.touch_effect,
                        flags & 16 != 0 || (flags & 32 == 0 && effects)
                    );
                    assert_eq!(
                        pane.bounceback_effect,
                        flags & 64 != 0 || (flags & 128 == 0 && effects)
                    );
                    assert_eq!(pane.scroll_step(), 12.5);
                    assert_eq!(pane.paging_threshold(), 0.45);
                    assert!(pane.set_scroll_step(50.));
                    assert!(pane.set_scroll_step(0.));
                    assert_eq!(pane.scroll_step(), 12.5);
                    pane.set_view_size(Size::new(100., 80.));
                    pane.set_content_size(Size::new(100., 480.));
                    let moved = display != ScrollBarDisplayType::Hidden;
                    assert_eq!(pane.mouse_wheel(1.), moved);
                    assert_eq!(pane.pos().y, if moved { 25. } else { 0. });
                }
            }
        }
    }
}

#[test]
fn global_paging_threshold_changes_short_drag_page_snapping() {
    let mut pane = ScrollPane {
        page_mode: true,
        scroll_type: ScrollType::Both,
        ..ScrollPane::default()
    };
    pane.set_paging_threshold(0.45);
    pane.set_view_size(Size::new(100., 100.));
    pane.set_content_size(Size::new(300., 300.));
    assert!(pane.set_pos_animated(0., 40., false));
    assert_eq!(pane.pos().y, 0.);

    pane.set_paging_threshold(0.3);
    assert!(pane.set_pos_animated(0., 40., false));
    assert_eq!(pane.pos().y, 100.);
}

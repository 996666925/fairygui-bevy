use super::*;

#[test]
fn per_axis_visibility_matches_162_native_tween_and_handler_snapshots() {
    let mut current = None;
    let mut pane = ScrollPane::default();
    let mut count = 0;
    for line in include_str!("../../tests/fixtures/bar-visibility-reference.txt")
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
    {
        let fields: Vec<_> = line.split_whitespace().collect();
        let key = (fields[0], fields[1]);
        let value: f32 = fields[3].parse().unwrap();
        if current != Some(key) {
            pane = ScrollPane::default();
            pane.scroll_type = ScrollType::Both;
            pane.set_view_size(Size::new(100., 100.));
            pane.set_content_size(Size::new(320., 320.));
            pane.bar_display = if fields[1] == "1" {
                ScrollBarDisplayType::Auto
            } else {
                ScrollBarDisplayType::Visible
            };
            pane.sync_bar_activity();
            current = Some(key);
        }
        match fields[2] {
            "init" => {}
            "immediate" => {
                pane.set_pos(10., 10., false);
            }
            "tick" => {
                pane.advance_bar_fade(value);
            }
            "refresh" => pane.refresh_bar_visibility(),
            "drag" => {
                if value == 1. {
                    pane.begin_drag();
                } else {
                    pane.end_drag();
                }
            }
            "tween" => {
                pane.animation = (value != 0.).then_some(ScrollAnimation {
                    start: Point::ZERO,
                    end: Point::new(10., 10.),
                    elapsed: 0.,
                });
                pane.sync_bar_activity();
            }
            "hold_v" => {
                pane.set_grip_dragging(true, value != 0.);
            }
            "hold_h" => {
                pane.set_grip_dragging(false, value != 0.);
            }
            "available_v" | "available_h" => {
                pane.set_bar_available(fields[2] == "available_v", value != 0.);
                pane.refresh_bar_visibility();
            }
            _ => panic!("{line}"),
        }
        for (vertical, visible, alpha, grip) in [(true, 4, 5, 8), (false, 6, 7, 9)] {
            assert_eq!(
                pane.bar_visible_for(vertical),
                fields[visible] == "1",
                "{line}"
            );
            let expected: f32 = fields[alpha].parse().unwrap();
            assert!(
                (pane.bar_alpha_for(vertical) - expected).abs() < 1e-5,
                "{line}: axis={vertical} actual={}",
                pane.bar_alpha_for(vertical)
            );
            assert_eq!(pane.grip_dragging(vertical), fields[grip] == "1", "{line}");
        }
        count += 1;
    }
    assert_eq!(count, 162);
}

#[test]
fn invalid_fade_time_is_ignored_and_motion_keeps_both_bars_visible() {
    let mut pane = ScrollPane {
        bar_display: ScrollBarDisplayType::Auto,
        ..Default::default()
    };
    pane.touch_bars();
    for dt in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1., 0.] {
        assert!(!pane.advance_bar_fade(dt));
        assert_eq!(pane.bar_alpha_for(true), 1.);
        assert!(pane.bar_visible_for(true));
    }
    pane.begin_drag();
    pane.advance_bar_fade(5.);
    assert!(pane.bar_visible_for(true));
    assert_eq!(pane.bar_alpha_for(true), 1.);
    pane.end_drag();
    pane.advance_bar_fade(0.75);
    assert_eq!(pane.bar_alpha_for(true), 0.25);
    pane.advance_bar_fade(0.25);
    assert!(!pane.bar_visible_for(true));
    assert_eq!(pane.bar_alpha_for(true), 1.);
}

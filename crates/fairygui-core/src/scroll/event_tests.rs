use super::*;
use crate::geom::{Point, Size};

#[test]
fn scroll_event_lifecycles_match_native_refresh_drag_wheel_and_tween_traces() {
    let mut pane = ScrollPane::default();
    let mut context = "";
    let mut cases = 0;
    for line in include_str!("../../tests/fixtures/scroll-events-reference.txt").lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields[0] == "case" {
            context = line;
            pane = ScrollPane {
                bounceback_effect: fields[2] == "1",
                inertia_disabled: fields[3] == "1",
                ..Default::default()
            };
            pane.set_view_size(Size::new(100., 80.));
            pane.set_content_size(Size::new(100., 600.));
            pane.set_pos(0., fields[4].parse().unwrap(), false);
            pane.drain_scroll_events().for_each(drop);
            cases += 1;
            continue;
        }
        let n = |i: usize| fields[i].parse::<f32>().unwrap();
        match fields[0] {
            "command" => {
                pane.set_pos_animated(0., n(1), n(2) != 0.);
                continue; // Native setters defer the refresh notification.
            }
            "wheel" => {
                pane.mouse_wheel(n(1));
                continue;
            }
            "refresh" => {}
            "down" => pane.begin_drag(),
            "move" => pane.drag(0., n(1), n(2)),
            "wait" => pane.drag(0., 0., n(1)),
            "up" => pane.end_drag(),
            "kill" => pane.stop(),
            "tick" => {
                pane.advance(n(1));
            }
            "lock" => {
                pane.set_refresh_max_sizes(Some(Size::ZERO), None);
                pane.lock_header(n(1));
            }
            _ => panic!("{line}"),
        }
        let actual: String = pane
            .drain_scroll_events()
            .map(|event| match event {
                ScrollEvent::Refresh | ScrollEvent::Scroll => 'S',
                ScrollEvent::End => 'E',
            })
            .collect();
        let actual = if actual.is_empty() { "-" } else { &actual };
        assert_eq!(actual, *fields.last().unwrap(), "{context}\n{line}");
    }
    assert_eq!(cases, 17);
}

#[test]
fn geometry_rebasing_and_invalid_or_idle_updates_do_not_notify() {
    let mut pane = ScrollPane::default();
    pane.set_view_size(Size::new(100., 80.));
    pane.set_content_size(Size::new(100., 600.));
    pane.set_pos_animated(0., 400., false);
    assert_eq!(pane.drain_scroll_events().count(), 1);
    pane.set_content_size(Size::new(100., 120.));
    assert_eq!(pane.pos().y, 40.);
    pane.set_view_size(Size::new(100., 100.));
    pane.clamp_pos();
    pane.change_content_size_on_scrolling(Size::new(0., 40.), Point::new(0., 20.));
    pane.set_pos_animated(0., f32::NAN, true);
    pane.advance(f32::INFINITY);
    pane.advance(0.1);
    pane.stop();
    assert_eq!(pane.drain_scroll_events().count(), 0);
}

#[test]
fn animated_position_refresh_does_not_duplicate_the_first_tween_scroll() {
    let mut pane = ScrollPane::default();
    pane.set_view_size(Size::new(100., 80.));
    pane.set_content_size(Size::new(100., 600.));
    assert!(pane.set_pos_animated(0., 240., true));

    assert!(pane.advance(0.1));
    let first: Vec<_> = pane.drain_scroll_events().collect();
    assert_eq!(first, vec![ScrollEvent::Refresh]);
    assert!(pane.display_pos().y > 0. && pane.display_pos().y < 240.);
    assert!(pane.is_animating());

    assert!(pane.advance(0.1));
    let second: Vec<_> = pane.drain_scroll_events().collect();
    assert_eq!(second, vec![ScrollEvent::Scroll]);
    assert!(pane.display_pos().y > 0. && pane.display_pos().y < 240.);

    pane.set_pos_animated(0., 120., true);
    assert!(pane.advance(0.5));
    assert_eq!(
        pane.drain_scroll_events().collect::<Vec<_>>(),
        vec![ScrollEvent::Refresh, ScrollEvent::Scroll, ScrollEvent::End]
    );
}

use super::*;
use crate::{geom::Size, scroll::ScrollType};

pub(crate) fn items(profile: usize) -> Vec<Rect> {
    let values: &[[f32; 6]] = match profile {
        0 => &[],
        1 => &[
            [0., 0., 80., 20., 1., 1.],
            [0., 40., 100., 60., -1., 1.],
            [0., 110., 60., 70., 1., 0.5],
            [0., 260., 120., 100., 1., 1.],
        ],
        2 => &[
            [0., 0., 20., 80., 1., 1.],
            [40., 0., 60., 100., 1., -1.],
            [110., 0., 70., 60., 0.5, 1.],
            [260., 0., 100., 120., 1., 1.],
        ],
        3 => &[
            [0., 0., 30., 20., 1., 1.],
            [40., 0., 50., 20., 1., 1.],
            [0., 40., 30., 60., 1., 1.],
            [40., 40., 50., 60., 1., 1.],
        ],
        4 => &[
            [80., 120., 40., 30., 1., 1.],
            [0., 0., 80., 70., 1., 1.],
            [200., 50., 100., 60., -2., 0.5],
        ],
        _ => &[
            [0., 0., 10., 10., 1., 1.],
            [150., 140., 50., 100., 1., 1.],
            [350., 280., 20., 20., 1., 1.],
            [600., 500., 20., 20., 1., 1.],
        ],
    };
    values
        .iter()
        .map(|v| Rect::new(v[0], v[1], v[2] * v[4].abs(), v[3] * v[5].abs()))
        .collect()
}

#[test]
fn mid_tween_page_requests_follow_native_displayed_direction_and_restart_rules() {
    let mut p = ScrollPane {
        scroll_type: ScrollType::Vertical,
        page_mode: true,
        ..Default::default()
    };
    p.set_view_size(Size::new(100., 80.));
    p.set_content_size(Size::new(100., 400.));
    let mut count = 0;
    for line in include_str!("../../tests/fixtures/scroll-alignment-reference.txt")
        .lines()
        .filter(|s| s.starts_with("seq "))
    {
        let mut fields = line.split_whitespace().skip(1);
        let action = fields.next().unwrap();
        let v: Vec<f32> = fields.map(|s| s.parse().unwrap()).collect();
        assert_eq!(v.len(), 8);
        match action {
            "set" => assert_eq!(
                p.set_pos_animated(0., v[0], v[1] != 0.),
                v[2] != 0.,
                "accepted {line}"
            ),
            "tick" => {
                p.advance(v[0]);
            }
            _ => panic!("unknown sequence action: {line}"),
        }
        assert_eq!(p.pos(), Point::new(v[3], v[4]), "logical {line}");
        assert_eq!(p.display_pos(), Point::new(v[5], v[6]), "display {line}");
        assert_eq!(p.is_animating(), v[7] != 0., "tween {line}");
        count += 1;
    }
    assert_eq!(count, 8);
}

#[test]
fn stopping_after_one_axis_clamps_matches_native_remaining_tween_change() {
    let mut count = 0;
    for line in include_str!("../../tests/fixtures/scroll-alignment-reference.txt")
        .lines()
        .filter(|s| s.starts_with("stop "))
    {
        let v: Vec<f32> = line
            .split_whitespace()
            .skip(1)
            .map(|s| s.parse().unwrap())
            .collect();
        let mut p = ScrollPane {
            scroll_type: ScrollType::Both,
            snap_to_item: true,
            ..Default::default()
        };
        p.set_view_size(Size::new(100., 80.));
        p.set_content_size(Size::new(500., 400.));
        p.set_snap_items(items(5));
        p.set_pos_animated(v[0], v[1], true);
        p.advance(0.25);
        let (states, remainder) = v[2..].as_chunks::<5>();
        assert!(remainder.is_empty());
        assert_eq!(states.len(), 2);
        for (i, s) in states.iter().enumerate() {
            if i == 1 {
                p.stop();
            }
            assert_eq!(p.pos(), Point::new(s[0], s[1]), "logical {line}");
            assert_eq!(p.display_pos(), Point::new(s[2], s[3]), "display {line}");
            assert_eq!(p.is_animating(), s[4] != 0., "tween {line}");
        }
        count += 1;
    }
    assert_eq!(count, 3);
}

#[test]
fn aligned_requests_and_tween_boundaries_match_native_refresh() {
    let mut commands = 0;
    let mut snaps = 0;
    for line in include_str!("../../tests/fixtures/scroll-alignment-reference.txt").lines() {
        let Some((kind, rest)) = line.split_once(' ') else {
            continue;
        };
        if kind != "cmd" && kind != "snap" {
            continue;
        }
        let v: Vec<f32> = rest
            .split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect();
        if kind == "snap" {
            assert_eq!(
                snapping_position(&items(v[1] as usize), Point::new(v[2], v[3])),
                Point::new(v[4], v[5]),
                "{line}"
            );
            snaps += 1;
            continue;
        }
        let mut p = ScrollPane {
            scroll_type: ScrollType::from_code(v[0] as i8),
            page_mode: v[1] != 0.,
            snap_to_item: v[2] != 0.,
            ..Default::default()
        };
        p.set_view_size(Size::new(v[5], v[6]));
        p.set_content_size(Size::new(v[7], v[8]));
        p.set_pos(v[9], v[10], false);
        p.set_snap_items(items(v[4] as usize));
        assert_eq!(
            p.set_pos_animated(v[11], v[12], v[13] != 0.),
            v[14] != 0.,
            "accepted {line}"
        );
        let (states, remainder) = v[15..].as_chunks::<5>();
        assert!(remainder.is_empty());
        assert_eq!(states.len(), 4);
        for (i, state) in states.iter().enumerate() {
            if i > 0 {
                p.advance([0.125, 0.375, 0.1][i - 1]);
            }
            assert_eq!(
                p.pos(),
                Point::new(state[0], state[1]),
                "logical step={i}: {line}"
            );
            assert_eq!(
                p.display_pos(),
                Point::new(state[2], state[3]),
                "display step={i}: {line}"
            );
            assert_eq!(p.is_animating(), state[4] != 0., "tween step={i}: {line}");
        }
        commands += 1;
    }
    assert_eq!((commands, snaps), (2970, 120));
}

#[test]
fn repeated_raw_target_keeps_tween_but_a_distinct_request_snapping_back_restarts_it() {
    let mut p = ScrollPane {
        page_mode: true,
        ..Default::default()
    };
    p.set_view_size(Size::new(100., 100.));
    p.set_content_size(Size::new(100., 500.));
    p.set_pos_animated(0., 100., true);
    p.advance(0.125);
    let before = p.display_pos().y;
    assert!(!p.set_pos_animated(0., 100., true));
    p.advance(0.125);
    assert!(p.display_pos().y > before);
    assert!(p.set_pos_animated(0., 110., true));
    assert_eq!(p.pos().y, 100.);
    let at = p.display_pos().y;
    p.advance(0.125);
    assert!(p.display_pos().y > at);
    p.set_pos_animated(0., f32::NAN, false);
    assert!(p.is_animating());
}

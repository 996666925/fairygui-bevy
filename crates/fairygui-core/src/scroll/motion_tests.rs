use super::*;
use crate::{
    geom::{Rect, Size},
    scroll::ScrollType,
};

#[test]
fn accepted_drag_release_bounce_and_interruptions_match_native_mouse_and_tween_traces() {
    let mut pane = ScrollPane::default();
    let mut scale = 1.;
    let mut cases = 0;
    let mut snapshots = 0;
    let mut context = String::new();
    for line in include_str!("../../tests/fixtures/scroll-motion-reference.txt").lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let mut fields = line.split_whitespace();
        let kind = fields.next().unwrap();
        // Configuration rates retain JavaScript precision; model inputs are f32.
        let v: Vec<f64> = fields.map(|s| s.parse().unwrap()).collect();
        if kind == "case" {
            context = line.into();
            pane = ScrollPane {
                scroll_type: ScrollType::from_code(v[1] as i8),
                page_mode: v[2] != 0.,
                snap_to_item: v[3] != 0.,
                bounceback_effect: v[4] != 0.,
                inertia_disabled: v[5] != 0.,
                ..Default::default()
            };
            pane.set_physics(ScrollPhysics {
                slow_frame_rate: v[6] != 0.,
                mobile_screen_extent: (v[7] > 0.).then_some(v[7] as f32),
                deceleration_rate: v[9],
            });
            scale = v[8] as f32;
            pane.set_view_size(Size::new(v[10] as f32, v[11] as f32));
            pane.set_content_size(Size::new(v[12] as f32, v[13] as f32));
            pane.set_pos(v[14] as f32, v[15] as f32, false);
            pane.set_snap_items(vec![
                Rect::new(0., 0., 25., 20.),
                Rect::new(40., 40., 60., 60.),
                Rect::new(120., 110., 80., 70.),
                Rect::new(260., 260., 100., 100.),
                Rect::new(420., 420., 80., 80.),
            ]);
            cases += 1;
            continue;
        }
        assert_eq!(v.len(), 8, "{line}");
        match kind {
            "down" => pane.begin_drag(),
            "move" => pane.drag_with_scale(
                v[0] as f32,
                v[1] as f32,
                v[2] as f32,
                Point::new(scale, scale),
            ),
            "wait" => pane.drag(0., 0., v[2] as f32),
            "up" => pane.end_drag_after(v[2] as f32),
            "tick" => {
                pane.advance(v[2] as f32);
            }
            "kill" => pane.stop(),
            _ => panic!("unknown native action {line}"),
        }
        assert_eq!(
            pane.pos(),
            Point::new(v[3] as f32, v[4] as f32),
            "logical {context}\n{line}"
        );
        assert_eq!(
            pane.display_pos(),
            Point::new(v[5] as f32, v[6] as f32),
            "display {context}\n{line}"
        );
        assert_eq!(pane.is_animating(), v[7] != 0., "tween {context}\n{line}");
        snapshots += 1;
    }
    assert_eq!(cases, 1728);
    assert!(snapshots > 30000);
}

#[test]
fn invalid_motion_inputs_do_not_poison_an_active_release() {
    let mut p = ScrollPane::default();
    p.set_view_size(Size::new(100., 80.));
    p.set_content_size(Size::new(100., 600.));
    p.begin_drag();
    p.drag(0., -60., 0.016);
    p.end_drag();
    assert!(p.is_animating());
    let before = p.display_pos();
    p.drag(f32::NAN, 0., 0.1);
    assert_eq!(p.display_pos(), before);
    assert!(!p.set_physics(ScrollPhysics {
        deceleration_rate: 1.,
        ..Default::default()
    }));
    assert!(!p.set_physics(ScrollPhysics {
        mobile_screen_extent: Some(0.),
        ..Default::default()
    }));
    assert!(!p.advance(f32::INFINITY));
    p.advance(0.1);
    assert!(p.display_pos().y.is_finite());
}

use super::*;
use crate::scroll::ScrollType;

#[test]
fn native_pull_caps_release_callback_locks_and_resource_geometry_match() {
    let mut p = ScrollPane::default();
    let mut parts = 0;
    let mut cap = 0.;
    let mut policy = false;
    let mut positions = [Point::ZERO; 2];
    let mut sizes = [Size::ZERO; 2];
    let mut context = String::new();
    let mut cases = 0;
    let mut snapshots = 0;
    for line in include_str!("../../tests/fixtures/pull-refresh-reference.txt").lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let mut fields = line.split_whitespace();
        let kind = fields.next().unwrap();
        let v: Vec<f32> = fields.map(|s| s.parse().unwrap()).collect();
        let mut layout_cross = false;
        let mut event = 0.;
        if kind == "case" {
            context = line.into();
            parts = v[2] as u8;
            cap = v[3];
            policy = v[7] != 0.;
            p = ScrollPane {
                scroll_type: ScrollType::from_code(v[1] as i8),
                inertia_disabled: true,
                ..Default::default()
            };
            p.set_refresh_max_sizes(
                (parts & 1 != 0).then_some(Size::new(cap, cap)),
                (parts & 2 != 0).then_some(Size::new(cap, cap)),
            );
            p.set_view_size(Size::new(v[8], v[9]));
            p.set_content_size(Size::new(v[10], v[11]));
            p.set_pos(v[12], v[13], false);
            positions = [Point::new(3., 5.); 2];
            sizes = [Size::new(70., 30.); 2];
            layout_cross = true;
            cases += 1;
        } else {
            match kind {
                "head" => {
                    p.lock_header(v[0]);
                }
                "foot" => {
                    p.lock_footer(v[0]);
                }
                "down" => p.begin_drag(),
                "move" => p.drag(v[0], v[1], v[2]),
                "up" => {
                    let release = p.defer_drag_release(v[2]);
                    event = match release {
                        Some(PullRelease::Down) => 1.,
                        Some(PullRelease::Up) => 2.,
                        None => 0.,
                    };
                    if policy {
                        match release {
                            Some(PullRelease::Down) => {
                                p.lock_header(35.);
                            }
                            Some(PullRelease::Up) => {
                                p.lock_footer(35.);
                            }
                            None => {}
                        }
                    }
                    p.complete_drag_release();
                }
                "tick" => {
                    p.advance(v[2]);
                }
                "content" => {
                    p.set_content_size(Size::new(v[0], v[1]));
                    layout_cross = true;
                }
                _ => panic!("unknown action {line}"),
            }
        }
        let mut visible = [false; 2];
        for i in 0..2 {
            if parts & (1 << i) == 0 {
                continue;
            }
            if layout_cross {
                let view = p.view_size();
                let vertical = p.refresh_axis() == Some(true);
                if (i == 0 && !vertical) || (i == 1 && vertical) {
                    sizes[i].height = view.height;
                } else {
                    sizes[i].width = view.width;
                }
                if cap > 0. {
                    sizes[i].width = sizes[i].width.min(cap);
                    sizes[i].height = sizes[i].height.min(cap);
                }
            }
            let g = p
                .refresh_bar_layout(i == 0, positions[i], sizes[i])
                .unwrap();
            visible[i] = g.visible;
            if g.visible {
                positions[i] = g.position;
                sizes[i] = g.size;
                if cap > 0. {
                    sizes[i].width = sizes[i].width.min(cap);
                    sizes[i].height = sizes[i].height.min(cap);
                }
            }
        }
        if kind == "case" {
            continue;
        }
        let mut actual = vec![
            p.pos().x,
            p.pos().y,
            p.display_pos().x,
            p.display_pos().y,
            f32::from(p.is_animating()),
            p.header_locked_size(),
            p.footer_locked_size(),
            event,
        ];
        for i in 0..2 {
            if parts & (1 << i) == 0 {
                actual.extend([0.; 5]);
            } else {
                actual.extend([
                    f32::from(visible[i]),
                    positions[i].x,
                    positions[i].y,
                    sizes[i].width,
                    sizes[i].height,
                ]);
            }
        }
        assert_eq!(actual, &v[3..], "{context}\n{line}");
        snapshots += 1;
    }
    assert_eq!((cases, snapshots), (648, 12960));
}

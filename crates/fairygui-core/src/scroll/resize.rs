//! Native changeContentSizeOnScrolling, used by measured virtual rows.
use super::{Point, ScrollPane};
use crate::geom::Size;

impl ScrollPane {
    pub fn scrolling_pos(&self) -> Point {
        let max = self.max_scroll_pos();
        Point::new(
            self.display_pos.x.clamp(0., max.x),
            self.display_pos.y.clamp(0., max.y),
        )
    }

    pub fn change_content_size_on_scrolling(&mut self, delta: Size, offset: Point) -> bool {
        if ![delta.width, delta.height, offset.x, offset.y]
            .into_iter()
            .all(f32::is_finite)
        {
            return false;
        }
        let old_max = self.max_scroll_pos();
        let edges = [self.pos.x == old_max.x, self.pos.y == old_max.y];
        self.content_size = Size::new(
            self.content_size.width + delta.width,
            self.content_size.height + delta.height,
        );
        let max = self.max_scroll_pos();
        let bounds = self.motion_bounds();
        self.pos = Point::new(self.pos.x.clamp(0., max.x), self.pos.y.clamp(0., max.y));
        self.display_pos = Point::new(
            self.display_pos
                .x
                .clamp(-bounds[0].1 as f32, -bounds[0].0 as f32),
            self.display_pos
                .y
                .clamp(-bounds[1].1 as f32, -bounds[1].0 as f32),
        );
        if let Some(animation) = &mut self.animation {
            if delta.width != 0. && edges[0] && animation.end.x > animation.start.x {
                self.pos.x = max.x;
                animation.end.x = max.x;
            }
            if delta.height != 0. && edges[1] && animation.end.y > animation.start.y {
                self.pos.y = max.y;
                animation.end.y = max.y;
            }
        } else if self.motion.tween.is_some() {
            self.translate_release_origin(offset);
            if offset.x != 0. {
                self.display_pos.x += offset.x;
                self.pos.x = self.display_pos.x;
            }
            if offset.y != 0. {
                self.display_pos.y += offset.y;
                self.pos.y = self.display_pos.y;
            }
        } else if self.dragging {
            self.translate_drag_origin(offset);
            if offset.x != 0. {
                self.display_pos.x += offset.x;
                self.pos.x = self.display_pos.x;
            }
            if offset.y != 0. {
                self.display_pos.y += offset.y;
                self.pos.y = self.display_pos.y;
            }
        } else {
            if delta.width != 0. && edges[0] {
                self.pos.x = max.x;
                self.display_pos.x = max.x;
            }
            if delta.height != 0. && edges[1] {
                self.pos.y = max.y;
                self.display_pos.y = max.y;
            }
        }
        self.refresh_bar_visibility();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scroll::ScrollType;

    #[test]
    fn measured_deltas_match_native_idle_drag_and_both_tween_paths() {
        let mut pane = ScrollPane::default();
        let mut context = String::new();
        let (mut cases, mut snapshots) = (0, 0);
        for line in include_str!("../../tests/fixtures/virtual-resize-reference.txt")
            .lines()
            .filter(|l| !l.starts_with('#'))
        {
            let mut fields = line.split_whitespace();
            let kind = fields.next().unwrap();
            let v: Vec<f32> = fields.map(|v| v.parse().unwrap()).collect();
            if kind == "case" {
                context = line.into();
                pane = ScrollPane::default();
                pane.scroll_type = ScrollType::from_code(v[1] as i8);
                pane.set_view_size(Size::new(100., 80.));
                pane.set_content_size(Size::new(400., 350.));
                let max = pane.max_scroll_pos();
                pane.set_pos(max.x * v[2], max.y * v[2], false);
                cases += 1;
                continue;
            }
            match kind {
                "go" => {
                    pane.set_pos_animated(v[0], v[1], true);
                }
                "down" => pane.begin_drag(),
                "move" => pane.drag(v[0], v[1], v[4]),
                "up" => pane.end_drag_after(v[4]),
                "tick" => {
                    pane.advance(v[4]);
                }
                "resize" => {
                    pane.change_content_size_on_scrolling(
                        Size::new(v[0], v[1]),
                        Point::new(v[2], v[3]),
                    );
                }
                _ => panic!("{line}"),
            }
            let actual = [
                pane.pos().x,
                pane.pos().y,
                pane.display_pos().x,
                pane.display_pos().y,
                f32::from(pane.is_animating()),
                pane.content_size().width,
                pane.content_size().height,
            ];
            for (actual, expected) in actual.into_iter().zip(&v[5..]) {
                assert!(
                    (actual - expected).abs() < 0.002,
                    "{context}\n{line}\n{actual} != {expected}"
                );
            }
            snapshots += 1;
        }
        assert_eq!(cases, 36);
        assert!(snapshots > 300);
    }
}

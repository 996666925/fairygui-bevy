//! GScrollBar's press actions belong to MouseDown, independently of clicks.
use super::{GObjectId, ObjectStore};
use crate::{
    geom::{Point, Rect},
    scroll::{ARROW1_NAME, ARROW2_NAME, GRIP_NAME},
};

#[cfg(test)]
#[path = "scroll_bar_input_tests.rs"]
mod tests;

impl ObjectStore {
    /// Keep only this registered bar visible while its grip owns the pointer.
    /// Identity validation prevents a stale capture from releasing a replacement.
    pub fn set_scroll_bar_grip_dragging(&mut self, bar: GObjectId, dragging: bool) -> bool {
        let Some(vertical) = self.scroll_bar_axis(bar) else {
            return false;
        };
        let Some(container) = self.scroll_container_of_bar(bar) else {
            return false;
        };
        let changed = self
            .objects
            .get_mut(container)
            .and_then(|o| o.control.component_mut())
            .and_then(|c| c.scroll_pane.as_mut())
            .is_some_and(|pane| pane.set_grip_dragging(vertical, dragging));
        if changed {
            self.touch(container, super::dirty::SCROLL);
            self.sync_scroll_bars(container);
        }
        changed
    }

    /// Registered pane bar containing this hit, including nested skin graphics.
    pub fn registered_scroll_bar_of(&self, target: GObjectId) -> Option<GObjectId> {
        let mut current = Some(target);
        while let Some(id) = current {
            if self.is_scroll_bar_overlay(id) {
                return Some(id);
            }
            current = self.object(id)?.parent();
        }
        None
    }

    fn scroll_bar_hit_part(&self, target: GObjectId) -> Option<(GObjectId, GObjectId)> {
        let bar = self.registered_scroll_bar_of(target)?;
        let mut part = target;
        while part != bar && self.object(part)?.parent() != Some(bar) {
            part = self.object(part)?.parent()?;
        }
        Some((bar, part))
    }

    /// Native grip/arrow MouseDown handlers stop bubbling at their skin part.
    pub fn scroll_bar_pointer_down_stop(&self, target: GObjectId) -> Option<GObjectId> {
        let (bar, part) = self.scroll_bar_hit_part(target)?;
        (bar != part
            && matches!(
                self.object(part)?.name(),
                GRIP_NAME | ARROW1_NAME | ARROW2_NAME
            ))
        .then_some(part)
    }

    /// Apply one native press action. The point is in the hit object's local
    /// coordinates. Returns whether the registered bar consumes the action,
    /// including grips, boundaries and invalid/missing coordinates.
    /// Grip movement is handled separately by the adapter's captured drag.
    pub fn press_scroll_bar(&mut self, target: GObjectId, point: Option<Point>) -> bool {
        let Some((bar, part)) = self.scroll_bar_hit_part(target) else {
            return false;
        };
        let Some(vertical) = self.scroll_bar_axis(bar) else {
            return false;
        };
        let Some(container) = self.scroll_container_of_bar(bar) else {
            return false;
        };
        let name = if part == bar {
            ""
        } else {
            self.object(part).unwrap().name()
        };
        let amount = match name {
            GRIP_NAME => return true,
            ARROW1_NAME => -1.,
            ARROW2_NAME => 1.,
            _ => {
                let Some(point) = point.filter(|p| p.x.is_finite() && p.y.is_finite()) else {
                    return true;
                };
                let Some(grip) = self.child_by_name(bar, GRIP_NAME) else {
                    return true;
                };
                let Some(local) =
                    self.scroll_relative_rect(target, grip, Rect::new(point.x, point.y, 0., 0.))
                else {
                    return true;
                };
                let position = if vertical { local.y } else { local.x };
                if !position.is_finite() {
                    return true;
                }
                // Native compares the point to the grip's local origin, not
                // normalized track position versus the scroll percentage.
                if position < 0. { -4. } else { 4. }
            }
        };
        self.scroll_container_by(container, !vertical, amount);
        true
    }
}

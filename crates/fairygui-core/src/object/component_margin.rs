//! Retain the native content Sprite origin independently of the clip margin.
use super::{GObjectId, ObjectStore, dirty};
use crate::{
    geom::{Margin, Point},
    widget::OverflowType,
};

impl ObjectStore {
    pub fn component_margin(&self, id: GObjectId) -> Option<Margin> {
        Some(self.object(id)?.control().component()?.margin)
    }

    /// Assign local logical margins and invalidate layout without resizing the
    /// outer object. Negative finite margins are supported, as in FairyGUI.
    /// Visible overflow retains its content Sprite position until alignment
    /// changes; clipped components and panes update their origin immediately.
    pub fn set_component_margin(&mut self, id: GObjectId, margin: Margin) -> bool {
        if ![margin.left, margin.top, margin.right, margin.bottom]
            .into_iter()
            .all(f32::is_finite)
            || self.component_margin(id).is_none_or(|old| old == margin)
        {
            return false;
        }
        let origin = self.component_content_origin(id).unwrap();
        let component = self.objects[id].control.component_mut().unwrap();
        if component.scroll_pane.is_none() && component.overflow != OverflowType::Hidden {
            self.visible_content_origins.entry(id).or_insert(origin);
        }
        component.margin = margin;
        if let Some(list) = self.list_data_mut(id) {
            list.item_margin = margin;
        }
        // Do not flush pending resize/layout here: an intermediate alignment
        // would move a visible list that native deferred layout keeps stationary.
        self.invalidate_list_layout(id);
        self.invalidate_virtual_list(id, true);
        self.layout_scroll_bars(id);
        self.touch(id, dirty::LOOK | dirty::SCROLL);
        true
    }

    /// Content origin in the owner's local coordinates, before displayed scroll.
    /// Shared by coordinate conversion and renderer projection.
    pub fn component_content_origin(&self, id: GObjectId) -> Option<Point> {
        let component = self.object(id)?.control().component()?;
        let alignment = self
            .virtual_states
            .get(&id)
            .map(|s| s.alignment)
            .or_else(|| self.list_alignments.get(&id).copied())
            .unwrap_or(Point::ZERO);
        if component.scroll_pane.is_some() {
            let mut origin = component.margin.top_left();
            origin.x += self.scroll_left_gutter(id);
            origin.offset(alignment.x, alignment.y);
            Some(origin)
        } else if component.overflow == OverflowType::Hidden {
            Some(Point::new(
                component.margin.left + alignment.x,
                component.margin.top + alignment.y,
            ))
        } else {
            Some(
                self.visible_content_origins
                    .get(&id)
                    .copied()
                    .unwrap_or(Point::new(
                        component.margin.left + alignment.x,
                        component.margin.top + alignment.y,
                    )),
            )
        }
    }

    /// Native mask-container origin. Refresh bars live here, outside the
    /// scrolling/aligned content Sprite; ordinary scrollbars live on the owner.
    pub fn scroll_mask_origin(&self, id: GObjectId) -> Option<Point> {
        let component = self.object(id)?.control().component()?;
        let pane = component.scroll_pane.as_ref()?;
        let mut origin = if pane.dont_clip_margin {
            Point::ZERO
        } else {
            component.margin.top_left()
        };
        origin.x += self.scroll_left_gutter(id);
        Some(origin)
    }

    fn scroll_left_gutter(&self, id: GObjectId) -> f32 {
        self.scroll_pane(id)
            .filter(|p| p.display_on_left && !p.floating)
            .and_then(|p| p.vt_bar)
            .and_then(|bar| self.object(bar))
            .map_or(0., |bar| bar.width())
    }
}

#[cfg(test)]
#[path = "component_margin_tests.rs"]
mod tests;

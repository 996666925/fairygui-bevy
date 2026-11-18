//! Native component scrollRect and ScrollPane mask-container bounds.
use super::{GObjectId, ObjectStore};
use crate::{
    GObject,
    geom::{Point, Rect},
    widget::OverflowType,
};

impl GObject {
    /// Hidden GComponent's own Sprite scrollRect offset. Unlike a ScrollPane,
    /// it participates in the component's native local/global API coordinates.
    pub fn component_scroll_offset(&self) -> Point {
        self.control()
            .component()
            .filter(|c| c.scroll_pane.is_none() && c.overflow == OverflowType::Hidden)
            .map_or(Point::ZERO, |c| c.margin.top_left())
    }
}

impl ObjectStore {
    /// Native hard-clip bounds in the owner's layout frame. Hidden components
    /// clip themselves and descendants; pane bounds apply only to content and
    /// refresh bars, leaving owner graphics and ordinary bars outside the mask.
    pub fn component_clip_rect(&self, id: GObjectId) -> Option<Rect> {
        let object = self.object(id)?;
        let component = object.control().component()?;
        if let Some(pane) = component.scroll_pane.as_ref() {
            if !pane.clip {
                return None;
            }
            let origin = self.scroll_mask_origin(id)?;
            let mut size = pane.view_size();
            if pane.display_in_demand {
                if pane.content_size().height <= pane.view_size().height {
                    size.width += pane
                        .vt_bar
                        .and_then(|b| self.object(b))
                        .map_or(0., |b| b.width());
                }
                if pane.content_size().width <= pane.view_size().width {
                    size.height += pane
                        .hz_bar
                        .and_then(|b| self.object(b))
                        .map_or(0., |b| b.height());
                }
            }
            if pane.dont_clip_margin {
                size.width += component.margin.horizontal();
                size.height += component.margin.vertical();
            }
            Some(Rect::new(
                origin.x,
                origin.y,
                size.width.max(0.),
                size.height.max(0.),
            ))
        } else if component.overflow == OverflowType::Hidden {
            // updateMask uses width - right, not width - left - right.
            // ClipVertex shifts painting by -scrollRect.xy, leaving the visible
            // clip anchored at the owner's unscrolled layout origin.
            Some(Rect::new(
                0.,
                0.,
                (object.width() - component.margin.right).max(0.),
                (object.height() - component.margin.bottom).max(0.),
            ))
        } else {
            None
        }
    }
}

#[cfg(test)]
#[path = "component_clip_tests.rs"]
mod tests;

//! A pane's registered bars belong to the owner lifetime, not its list rows.
use super::{GObjectId, ObjectStore};
use crate::widget::ControlData;

#[cfg(test)]
#[path = "list_items_tests.rs"]
pub(crate) mod tests;

impl ObjectStore {
    /// Native GComponent.isChildInView: geometry only, inclusive edges, using
    /// unscaled child bounds and the pane's current display offset. Visibility,
    /// stage connection and ancestor clipping do not affect this query.
    pub fn is_child_in_view(&self, owner: GObjectId, child: GObjectId) -> Option<bool> {
        let parent = self.object(owner)?;
        let component = parent.control().component()?;
        let child = self.object(child)?;
        if child.parent() != Some(owner) {
            return None;
        }
        let rect = crate::geom::Rect::new(child.x(), child.y(), child.width(), child.height());
        Some(
            if component.overflow == crate::widget::OverflowType::Hidden {
                rect.x + rect.width >= 0.
                    && rect.x <= parent.width()
                    && rect.y + rect.height >= 0.
                    && rect.y <= parent.height()
            } else if let Some(pane) = component.scroll_pane.as_ref() {
                pane.is_child_in_view(rect)
            } else {
                true
            },
        )
    }

    /// True only for the current pane's registered bar directly owned here.
    /// A user-authored ScrollBar used as a row is still ordinary list content.
    pub fn is_scroll_bar_overlay(&self, object: GObjectId) -> bool {
        self.object(object)
            .and_then(|o| o.parent())
            .and_then(|parent| self.scroll_pane(parent))
            .is_some_and(|p| p.vt_bar == Some(object) || p.hz_bar == Some(object))
    }

    /// Materialized content rows, including invisible rows, excluding pane bars.
    /// General GObject.children still includes bars for ownership and disposal.
    pub fn list_items(&self, list: GObjectId) -> Option<Vec<GObjectId>> {
        let object = self.object(list)?;
        if !matches!(
            object.control(),
            ControlData::List(_) | ControlData::Tree(_)
        ) {
            return None;
        }
        self.content_children(list)
    }

    pub(super) fn content_children(&self, owner: GObjectId) -> Option<Vec<GObjectId>> {
        Some(
            self.object(owner)?
                .children()
                .iter()
                .copied()
                .filter(|&id| !self.is_scroll_overlay(id))
                .collect(),
        )
    }

    pub fn list_item_at(&self, list: GObjectId, index: usize) -> Option<GObjectId> {
        let object = self.object(list)?;
        if !matches!(
            object.control(),
            ControlData::List(_) | ControlData::Tree(_)
        ) {
            return None;
        }
        object
            .children()
            .iter()
            .copied()
            .filter(|&id| !self.is_scroll_overlay(id))
            .nth(index)
    }

    pub fn list_item_index(&self, list: GObjectId, item: GObjectId) -> Option<usize> {
        let object = self.object(list)?;
        if !matches!(
            object.control(),
            ControlData::List(_) | ControlData::Tree(_)
        ) {
            return None;
        }
        object
            .children()
            .iter()
            .copied()
            .filter(|&id| !self.is_scroll_overlay(id))
            .position(|id| id == item)
    }
}

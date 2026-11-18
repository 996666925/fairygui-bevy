//! GComponent.viewWidth/viewHeight delegate through ScrollPane when present.
use super::{GObjectId, ObjectStore};
use crate::geom::Size;

impl ObjectStore {
    /// Current unscaled content viewport, excluding margins and reserved bars.
    /// Plain components can have negative view dimensions; panes clamp to one.
    pub fn component_view_size(&self, id: GObjectId) -> Option<Size> {
        let object = self.object(id)?;
        let component = object.control().component()?;
        Some(component.scroll_pane.as_ref().map_or(
            Size::new(
                object.width() - component.margin.horizontal(),
                object.height() - component.margin.vertical(),
            ),
            |pane| pane.view_size(),
        ))
    }

    /// Native viewWidth setter. Add margins and the nonfloating vertical bar,
    /// then assign outer width through the usual size constraints and relations.
    /// Preserve the other axis's retained request, even if currently clamped.
    pub fn set_component_view_width(&mut self, id: GObjectId, width: f32) -> bool {
        self.set_component_view_axis(id, width, false)
    }

    /// Native viewHeight setter; only the horizontal bar reserves height.
    pub fn set_component_view_height(&mut self, id: GObjectId, height: f32) -> bool {
        self.set_component_view_axis(id, height, true)
    }

    /// Assign both native viewport axes, in width-then-height setter order.
    /// Nonfinite axes are ignored independently. Finite negative values follow
    /// native outer-size clamping; this is a one-time size edit, not a constraint.
    pub fn set_component_view_size(&mut self, id: GObjectId, size: Size) -> bool {
        let width = self.set_component_view_width(id, size.width);
        self.set_component_view_height(id, size.height) || width
    }

    fn set_component_view_axis(&mut self, id: GObjectId, value: f32, vertical: bool) -> bool {
        if !value.is_finite() {
            return false;
        }
        let Some(object) = self.object(id) else {
            return false;
        };
        let Some(component) = object.control().component() else {
            return false;
        };
        let margin = if vertical {
            component.margin.vertical()
        } else {
            component.margin.horizontal()
        };
        let bar = component
            .scroll_pane
            .as_ref()
            .filter(|p| !p.floating)
            .and_then(|p| if vertical { p.hz_bar } else { p.vt_bar })
            .and_then(|id| self.object(id));
        let gutter = bar.map_or(0., |bar| if vertical { bar.height() } else { bar.width() });
        let outer = value + margin + gutter;
        if !outer.is_finite() {
            return false;
        }
        let before = object.requested_size();
        if vertical {
            self.set_height(id, outer);
        } else {
            self.set_width(id, outer);
        }
        // ScrollPane.onOwnerSizeChanged runs synchronously in the native setter.
        // Materialized list rows still reflow after the application edit batch.
        self.layout_scroll_bars(id);
        self.object(id)
            .is_some_and(|o| o.requested_size() != before)
    }
}

#[cfg(test)]
#[path = "component_view_size_tests.rs"]
mod tests;

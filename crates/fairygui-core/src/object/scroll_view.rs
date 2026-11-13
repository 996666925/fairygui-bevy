//! Scroll-to-view commands and the materialized GList routing rules.
use super::{GObjectId, ObjectStore, dirty};
use crate::{
    geom::{Point, Rect},
    widget::ControlData,
};

#[cfg(test)]
#[path = "scroll_view_tests.rs"]
mod tests;

impl ObjectStore {
    /// Reveal a content-coordinate rectangle using FairyGUI's item/page policy.
    /// Animated calls set the logical endpoint now and advance display_pos later.
    pub fn scroll_rect_to_view(
        &mut self,
        container: GObjectId,
        rect: Rect,
        animated: bool,
        set_first: bool,
    ) -> bool {
        self.layout_scroll_bars(container);
        self.prepare_scroll_snapping(container);
        let changed = self
            .objects
            .get_mut(container)
            .and_then(|o| o.control.component_mut())
            .and_then(|c| c.scroll_pane.as_mut())
            .is_some_and(|pane| pane.scroll_to_view(rect, animated, set_first));
        if changed {
            self.touch(container, dirty::SCROLL);
            self.update_scroll_page_controller(container);
        }
        changed
    }

    /// Reveal an object. Direct children use their untransformed logical bounds,
    /// as native ScrollPane does. Nested objects use the parent's two-corner
    /// local/global rectangle conversion, not an axis-aligned four-corner bound.
    pub fn scroll_object_to_view(
        &mut self,
        container: GObjectId,
        target: GObjectId,
        animated: bool,
        set_first: bool,
    ) -> bool {
        let Some(object) = self.object(target) else {
            return false;
        };
        let rect = Rect::from_size(object.position(), object.size());
        let Some(parent) = object.parent() else {
            return false;
        };
        let rect = if parent == container {
            rect
        } else {
            let Some(rect) = self.scroll_relative_rect(parent, container, rect) else {
                return false;
            };
            rect
        };
        self.scroll_rect_to_view(container, rect, animated, set_first)
    }

    /// GList.scrollToView for materialized rows: prefer its own pane, otherwise
    /// use only the immediate parent's pane. Invalid indices have no effect.
    pub fn scroll_list_item_to_view(
        &mut self,
        list: GObjectId,
        index: usize,
        animated: bool,
        set_first: bool,
    ) -> bool {
        if !matches!(
            self.object(list).map(|o| o.control()),
            Some(ControlData::List(_) | ControlData::Tree(_))
        ) {
            return false;
        }
        if let Some(state) = self.virtual_states.get(&list) {
            if index >= state.num_items {
                return false;
            }
            let index = if state.looping && state.num_items > 0 {
                state.geometry.first / state.num_items * state.num_items + index
            } else {
                index
            };
            return state
                .geometry
                .item_rect(index)
                .is_some_and(|rect| self.scroll_rect_to_view(list, rect, animated, set_first));
        }
        let Some(item) = self.list_item_at(list, index) else {
            return false;
        };
        self.layout_list(list);
        let container = if self.scroll_pane(list).is_some() {
            Some(list)
        } else {
            self.object(list)
                .and_then(|o| o.parent())
                .filter(|&p| self.scroll_pane(p).is_some())
        };
        container.is_some_and(|c| self.scroll_object_to_view(c, item, animated, set_first))
    }

    pub fn set_list_scroll_item_to_view_on_click(
        &mut self,
        list: GObjectId,
        enabled: bool,
    ) -> bool {
        let data = match self.objects.get_mut(list).map(|o| &mut o.control) {
            Some(ControlData::List(data)) => data.as_mut(),
            Some(ControlData::Tree(data)) => &mut data.list,
            _ => return false,
        };
        if data.scroll_item_to_view_on_click == enabled {
            return false;
        }
        data.scroll_item_to_view_on_click = enabled;
        true
    }

    /// __clickItem scrolls only its own pane, even in None mode/non-button rows.
    /// Unlike explicit scrollToView, clicks do not delegate to a parent pane.
    pub fn scroll_clicked_list_item_to_view(&mut self, target: GObjectId) -> bool {
        let Some((list, item, _)) = self.list_item_of(target) else {
            return false;
        };
        let enabled = match self.object(list).map(|o| o.control()) {
            Some(ControlData::List(data)) => data.scroll_item_to_view_on_click,
            Some(ControlData::Tree(data)) => data.list.scroll_item_to_view_on_click,
            _ => false,
        };
        if !enabled || self.scroll_pane(list).is_none() {
            return false;
        }
        self.layout_list(list);
        self.scroll_object_to_view(list, item, true, false)
    }

    pub(crate) fn scroll_relative_rect(
        &self,
        source: GObjectId,
        owner: GObjectId,
        rect: Rect,
    ) -> Option<Rect> {
        let source_object = self.object(source)?;
        let owner_object = self.object(owner)?;
        let source_matrix = self.scroll_global_transform(source)?;
        let inverse = self.scroll_global_transform(owner)?.inverse()?;
        let anchor = |o: &crate::GObject| {
            if o.pivot_as_anchor() {
                Point::new(o.pivot().x * o.width(), o.pivot().y * o.height())
            } else {
                Point::ZERO
            }
        };
        let a = anchor(source_object);
        let b = anchor(owner_object);
        let convert = |x, y| {
            let global = source_matrix.point(Point::new(x + a.x, y + a.y));
            let mut local = inverse.point(global);
            local.offset(-b.x, -b.y);
            local
        };
        let low = convert(rect.x, rect.y);
        let high = convert(rect.right(), rect.bottom());
        Some(Rect::new(low.x, low.y, high.x - low.x, high.y - low.y))
    }

    fn scroll_global_transform(&self, id: GObjectId) -> Option<Transform> {
        let mut result = Transform::IDENTITY;
        let mut current = Some(id);
        while let Some(id) = current {
            let object = self.object(id)?;
            let mut local = Transform::object(object);
            if let Some(parent) = object.parent()
                && let Some(component) = self.object(parent)?.control().component()
            {
                let scroll = component
                    .scroll_pane
                    .as_ref()
                    .map_or(Point::ZERO, |p| p.display_pos());
                let align = self.virtual_list_alignment(parent);
                local.tx += f64::from(component.margin.left - scroll.x + align.x);
                local.ty += f64::from(component.margin.top - scroll.y + align.y);
            }
            result = local.mul(result);
            current = object.parent();
        }
        Some(result)
    }
}

#[derive(Clone, Copy)]
struct Transform {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    tx: f64,
    ty: f64,
}
impl Transform {
    const IDENTITY: Self = Self {
        a: 1.,
        b: 0.,
        c: 0.,
        d: 1.,
        tx: 0.,
        ty: 0.,
    };

    fn object(o: &crate::GObject) -> Self {
        // Sprite.transform uses this native degree conversion and skew signs.
        let x = f64::from(o.rotation() - o.skew_x()) * 0.0174532922222222;
        let y = f64::from(o.rotation() + o.skew_y()) * 0.0174532922222222;
        let (a, b) = (
            f64::from(o.scale_x()) * y.cos(),
            f64::from(o.scale_x()) * y.sin(),
        );
        let (c, d) = (
            -f64::from(o.scale_y()) * x.sin(),
            f64::from(o.scale_y()) * x.cos(),
        );
        let (px, py) = (
            f64::from(o.pivot().x * o.width()),
            f64::from(o.pivot().y * o.height()),
        );
        let (offset_x, offset_y) = if o.pivot_as_anchor() {
            (0., 0.)
        } else {
            (px, py)
        };
        Self {
            a,
            b,
            c,
            d,
            tx: f64::from(o.x()) + offset_x - a * px - c * py,
            ty: f64::from(o.y()) + offset_y - b * px - d * py,
        }
    }

    fn mul(self, r: Self) -> Self {
        Self {
            a: self.a * r.a + self.c * r.b,
            b: self.b * r.a + self.d * r.b,
            c: self.a * r.c + self.c * r.d,
            d: self.b * r.c + self.d * r.d,
            tx: self.a * r.tx + self.c * r.ty + self.tx,
            ty: self.b * r.tx + self.d * r.ty + self.ty,
        }
    }

    fn point(self, p: Point) -> Point {
        Point::new(
            (self.a * f64::from(p.x) + self.c * f64::from(p.y) + self.tx) as f32,
            (self.b * f64::from(p.x) + self.d * f64::from(p.y) + self.ty) as f32,
        )
    }

    fn inverse(self) -> Option<Self> {
        let det = self.a * self.d - self.b * self.c;
        if !det.is_finite() || det == 0. {
            return None;
        }
        let (a, b, c, d) = (self.d / det, -self.b / det, -self.c / det, self.a / det);
        Some(Self {
            a,
            b,
            c,
            d,
            tx: -a * self.tx - c * self.ty,
            ty: -b * self.tx - d * self.ty,
        })
    }
}

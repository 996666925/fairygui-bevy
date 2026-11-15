//! GComponent.opaque and Sprite mouseThrough are related but independent state.
use super::{ObjectStore, dirty};
use crate::{GObject, object::GObjectId};

impl GObject {
    pub fn component_hit_test_prior(&self) -> bool {
        self.component_hit_test_prior
            || self
                .control()
                .component()
                .is_some_and(|c| c.hit_test.is_some())
    }
    /// Native component Sprite policy, including setMask side effects. A
    /// custom hit area is tested even when mouseThrough has been enabled.
    pub fn component_mouse_through(&self) -> Option<bool> {
        let component = self.control().component()?;
        Some(
            self.component_mouse_through
                .unwrap_or(!component.opaque && component.hit_test.is_none()),
        )
    }
    /// Whether the component's own Sprite can hit. Children remain independent;
    /// mouseThrough uses this Sprite's graphics, never its children's bounds.
    pub fn component_has_self_hit(&self) -> Option<bool> {
        let component = self.control().component()?;
        Some(!self.component_mouse_through()? || component.hit_test.is_some())
    }
}
impl ObjectStore {
    /// Set component clip fade widths. It affects painting, not object geometry
    /// or input hit testing. Invalid widths and non-components are unchanged.
    pub fn set_clip_softness(&mut self, id: GObjectId, value: crate::geom::Point) -> bool {
        if !value.x.is_finite() || !value.y.is_finite() || value.x < 0. || value.y < 0. {
            return false;
        }
        let Some(component) = self
            .objects
            .get_mut(id)
            .and_then(|object| object.control.component_mut())
        else {
            return false;
        };
        if component.clip_softness == value {
            return false;
        }
        component.clip_softness = value;
        self.touch(id, dirty::LOOK);
        true
    }

    /// Toggle native blank-rectangle picking without changing opacity, child
    /// interaction or a custom pixel/child hit area. Same-value writes do nothing.
    pub fn set_component_opaque(&mut self, id: GObjectId, value: bool) -> bool {
        let Some(object) = self.objects.get_mut(id) else {
            return false;
        };
        let Some(component) = object.control.component_mut() else {
            return false;
        };
        if component.opaque == value {
            return false;
        }
        component.opaque = value;
        object.component_mouse_through = Some(!value);
        self.touch(id, dirty::LOOK);
        true
    }
}

#[cfg(test)]
#[path = "component_input_tests.rs"]
mod tests;

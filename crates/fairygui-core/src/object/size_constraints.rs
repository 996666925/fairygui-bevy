use super::*;

/// Effective layout dimensions are clamped between these bounds. A zero
/// maximum leaves the axis unbounded. Scaling is independent of these values.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct SizeConstraints {
    pub min: Size,
    pub max: Size,
}
impl ObjectStore {
    pub fn size_constraints(&self, id: GObjectId) -> Option<SizeConstraints> {
        let o = self.objects.get(id)?;
        Some(SizeConstraints {
            min: o.min_size(),
            max: o.max_size(),
        })
    }
    /// Apply new constraints and immediately reevaluate the last requested size.
    /// Removing an upper limit restores that request without another size write.
    pub fn set_size_constraints(&mut self, id: GObjectId, constraints: SizeConstraints) -> bool {
        let Some(old) = self.size_constraints(id) else {
            return false;
        };
        let SizeConstraints { min, max } = constraints;
        if old == constraints
            || [min.width, min.height, max.width, max.height]
                .into_iter()
                .any(|v| !v.is_finite() || v < 0.)
            || (max.width > 0. && max.width < min.width)
            || (max.height > 0. && max.height < min.height)
        {
            return false;
        }
        let o = self.objects.get_mut(id).unwrap();
        let request = o.requested_size();
        (o.min_width, o.min_height, o.max_width, o.max_height) =
            (min.width, min.height, max.width, max.height);
        self.set_size_internal(id, request.width, request.height, false, true);
        true
    }
    pub fn set_width(&mut self, id: GObjectId, width: f32) {
        if let Some(o) = self.objects.get(id) {
            self.set_size(id, width, o.requested_size().height);
        }
    }
    pub fn set_height(&mut self, id: GObjectId, height: f32) {
        if let Some(o) = self.objects.get(id) {
            self.set_size(id, o.requested_size().width, height);
        }
    }
}

#[cfg(test)]
#[path = "size_constraints_tests.rs"]
mod tests;

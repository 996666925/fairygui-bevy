//! GComponent's ChildHitArea is a graphics hit gate, not an alpha texture test.
use super::*;
use fairygui_core::{
    graph::{polygon_contains, polygon_points},
    widget::{ControlData, GraphData, GraphType, HitTestArea},
};

pub(super) fn graph_contains(data: &GraphData, size: fairygui_core::Size, p: Vec2) -> bool {
    match data.graph_kind() {
        GraphType::Rect if data.corner_radius.is_none() => {
            p.x >= 0. && p.y >= 0. && p.x < size.width && p.y < size.height
        }
        // GGraph emits DrawCircle even when its declared height differs.
        GraphType::Ellipse => {
            (p - Vec2::new(size.width * 0.5, size.height * 0.5)).length_squared()
                < (size.width * 0.5).powi(2)
        }
        GraphType::Polygon | GraphType::RegularPolygon => {
            polygon_contains(&polygon_points(data, size), Point::new(p.x, p.y))
        }
        // HitArea does not support the DrawPath command used by round rectangles.
        _ => false,
    }
}

pub(super) fn allowed(
    runtime: &GuiRuntime,
    target: GObjectId,
    point: Vec2,
    geometry: &Query<(&UiGlobalTransform, &ComputedNode)>,
) -> bool {
    allowed_geometry(runtime, target, point, |id| {
        runtime.entity_of(id).and_then(|e| geometry.get(e).ok())
    })
}

fn allowed_geometry<'a>(
    runtime: &GuiRuntime,
    target: GObjectId,
    point: Vec2,
    geometry: impl Fn(GObjectId) -> Option<(&'a UiGlobalTransform, &'a ComputedNode)>,
) -> bool {
    let mut current = Some(target);
    while let Some(id) = current {
        let Some(object) = runtime.store().object(id) else {
            return false;
        };
        if let Some(HitTestArea::MaskChild {
            object: mask,
            reversed,
        }) = object
            .control()
            .component()
            .and_then(|c| c.hit_test.as_ref())
        {
            let inside = (|| {
                let mask_object = runtime.store().object(*mask)?;
                let ControlData::Graph(data) = mask_object.control() else {
                    return Some(false);
                };
                let (owner_t, owner_n) = geometry(id)?;
                let local = (owner_t.try_inverse()?.transform_point2(point) + owner_n.size() * 0.5)
                    * owner_n.inverse_scale_factor();
                let (mask_t, mask_n) = geometry(*mask)?;
                let zero = mask_t.transform_point2(-mask_n.size() * 0.5);
                let origin = if let Some(parent) = mask_object.parent() {
                    let (t, n) = geometry(parent)?;
                    (t.try_inverse()?.transform_point2(zero) + n.size() * 0.5)
                        * n.inverse_scale_factor()
                } else {
                    zero * mask_n.inverse_scale_factor()
                };
                Some(graph_contains(data, mask_object.size(), local - origin))
            })()
            .unwrap_or(false);
            if inside == *reversed {
                return false;
            }
        }
        current = object.parent();
    }
    true
}

#[cfg(test)]
#[path = "mask_tests.rs"]
mod tests;

#[derive(Component, Debug)]
pub(super) struct MaskHitProjection;
#[allow(clippy::type_complexity)]
pub(super) fn sync(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    nodes: Query<(
        Entity,
        &GObjectRef,
        Option<&crate::runtime::HitAreaFromChild>,
        Option<&crate::runtime::PixelHitTest>,
        Option<&MaskHitProjection>,
    )>,
) {
    for (entity, object, child, pixel, old) in &nodes {
        let Some(data) = runtime
            .store()
            .object(object.0)
            .and_then(|o| o.control().component())
        else {
            continue;
        };
        if matches!(data.hit_test, Some(HitTestArea::MaskChild { .. })) {
            if child.is_some() {
                commands
                    .entity(entity)
                    .remove::<crate::runtime::HitAreaFromChild>();
            }
            if pixel.is_some() {
                commands
                    .entity(entity)
                    .remove::<crate::runtime::PixelHitTest>();
            }
            if old.is_none() {
                commands.entity(entity).insert(MaskHitProjection);
            }
        } else {
            if old.is_some() {
                commands.entity(entity).remove::<MaskHitProjection>();
            }
            if child.is_some() && !matches!(data.hit_test, Some(HitTestArea::Child { .. })) {
                commands
                    .entity(entity)
                    .remove::<crate::runtime::HitAreaFromChild>();
            }
        }
    }
}

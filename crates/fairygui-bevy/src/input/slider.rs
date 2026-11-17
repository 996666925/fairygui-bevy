//! Slider press actions bubble independently of exclusive drag ownership.
use super::*;
use fairygui_core::widget::ControlData;

impl Gesture {
    /// Sample through the current view transform. Sliders retain the local press
    /// point; scrollbars retain the offset from the grip origin at press.
    pub(super) fn sample_control_point(
        &mut self,
        runtime: &GuiRuntime,
        cursor: Vec2,
        targets: &PointerTargets,
        geometry: &Query<(&UiGlobalTransform, &ComputedNode)>,
        views: &PointerViews,
        scale: f32,
    ) {
        let owner = match self {
            Self::Slider { owner, .. } => *owner,
            Self::Grip { bar, .. } => *bar,
            _ => return,
        };
        let entity = runtime.entity_of(owner).or_else(|| {
            targets
                .iter()
                .find_map(|(entity, reference, ..)| (reference.0 == owner).then_some(entity))
        });
        let local = if let Some((entity, (transform, node))) =
            entity.and_then(|entity| geometry.get(entity).ok().map(|geometry| (entity, geometry)))
        {
            transform.try_inverse().map(|inverse| {
                let mut point = (inverse
                    .transform_point2(views.physical_position(entity, cursor, scale))
                    - node.border_box().min)
                    * node.inverse_scale_factor();
                if let Some(object) = runtime.store().object(owner)
                    && object.pivot_as_anchor()
                {
                    point -= Vec2::new(
                        object.pivot().x * object.width(),
                        object.pivot().y * object.height(),
                    );
                }
                point
            })
        } else {
            runtime
                .store()
                .global_to_local(owner, Point::new(cursor.x, cursor.y))
                .map(|point| Vec2::new(point.x, point.y))
        }
        .filter(|point| point.is_finite());
        match self {
            Self::Slider { origin, point, .. } => {
                *point = local;
                if origin.is_none() {
                    *origin = local;
                }
            }
            Self::Grip { offset, point, .. } => {
                *point = local;
                if offset.is_none() {
                    *offset = local
                        .zip(
                            runtime
                                .store()
                                .child_by_name(owner, fairygui_core::scroll::GRIP_NAME)
                                .and_then(|id| runtime.store().object(id)),
                        )
                        .map(|(point, grip)| point - Vec2::new(grip.x(), grip.y()));
                }
            }
            _ => unreachable!(),
        }
    }
}

pub(super) fn pointer_down_stop(runtime: &GuiRuntime, target: GObjectId) -> Option<GObjectId> {
    let store = runtime.store();
    let stops = [
        store.scroll_bar_pointer_down_stop(target),
        store.slider_pointer_down_stop(target),
    ];
    let mut current = Some(target);
    while let Some(id) = current {
        if stops.contains(&Some(id)) {
            return Some(id);
        }
        current = store.object(id)?.parent();
    }
    None
}

pub(super) fn press_slider(
    runtime: &mut GuiRuntime,
    target: GObjectId,
    cursor: Vec2,
    positions: &[(GObjectId, Vec2)],
    touch_id: Option<u64>,
) {
    let stop = pointer_down_stop(runtime, target);
    let mut current = Some(target);
    while let Some(id) = current {
        let Some(object) = runtime.store().object(id) else {
            break;
        };
        current = object.parent();
        if matches!(object.control(), ControlData::Slider(_)) {
            let local = positions
                .iter()
                .find(|(owner, _)| *owner == id)
                .map(|(_, n)| Point::new(n.x * object.width(), n.y * object.height()))
                .or_else(|| {
                    runtime
                        .store()
                        .global_to_local(id, Point::new(cursor.x, cursor.y))
                });
            if local.is_some_and(|point| runtime.store_mut().click_slider_at(id, point)) {
                let mut event = UiEvent::new(EventKind::StateChanged, id)
                    .with_pointer(Point::new(cursor.x, cursor.y));
                event.touch_id = touch_id;
                runtime.dispatch_event(event);
            }
        }
        if Some(id) == stop {
            break;
        }
    }
}

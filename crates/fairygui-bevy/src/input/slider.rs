//! Slider press actions bubble independently of exclusive drag ownership.
use super::*;
use fairygui_core::widget::ControlData;

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

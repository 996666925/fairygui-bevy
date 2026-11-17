//! Physical release delivery to the hit path and saved press recipients.
use super::*;
use fairygui_core::pointer::ClickTargets;

pub(super) enum Target {
    Hit(Option<GObjectId>),
    /// Local window/focus invalidation; OS touchcancel uses Hit(None).
    Cancelled,
}

pub(super) fn dispatch(
    runtime: &mut GuiRuntime,
    kind: EventKind,
    targets: &ClickTargets,
    hit: Option<GObjectId>,
    position: Vec2,
    touch: Option<u64>,
) -> GObjectId {
    let target = hit.unwrap_or_else(|| runtime.pointer_stage_target());
    let route = targets.release_route(runtime.store(), target);
    let mut event = UiEvent::new(kind, target).with_pointer(Point::new(position.x, position.y));
    event.touch_id = touch;
    runtime.dispatch_pointer_release(event, route);
    target
}

pub(super) fn drag_end(
    runtime: &mut GuiRuntime,
    targets: &ClickTargets,
    target: GObjectId,
    position: Vec2,
    touch: Option<u64>,
) {
    for &current_target in targets.targets() {
        if !runtime.store().contains(current_target) {
            continue;
        }
        let mut event = UiEvent::new(EventKind::MouseDragEnd, target)
            .with_pointer(Point::new(position.x, position.y));
        event.current_target = current_target;
        event.bubbling = current_target != target;
        event.touch_id = touch;
        runtime.dispatch_direct_event(event);
    }
}

//! Drag recipients come from the saved press path; the target is the current hit.
use super::*;
use fairygui_core::pointer::ClickTargets;

pub(super) fn drag(
    runtime: &mut GuiRuntime,
    targets: &ClickTargets,
    hit: Option<GObjectId>,
    position: Vec2,
    touch: Option<u64>,
) {
    let target = hit.unwrap_or_else(|| runtime.pointer_stage_target());
    let mut event =
        UiEvent::new(EventKind::MouseDrag, target).with_pointer(Point::new(position.x, position.y));
    event.touch_id = touch;
    runtime.dispatch_press_path(event, targets.targets());
}

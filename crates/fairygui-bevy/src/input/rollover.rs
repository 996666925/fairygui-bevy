//! Native direct enter/exit notifications below the first common ancestor.
use super::*;
use fairygui_core::pointer::HoverChange;

pub(super) fn dispatch(
    runtime: &mut GuiRuntime,
    previous: Option<GObjectId>,
    next: Option<GObjectId>,
    position: Vec2,
    touch: Option<u64>,
) {
    let change = HoverChange::between(runtime.store(), previous, next);
    for (kind, recipients) in [
        (EventKind::RollOut, change.exited),
        (EventKind::RollOver, change.entered),
    ] {
        for target in recipients {
            let mut event =
                UiEvent::new(kind.clone(), target).with_pointer(Point::new(position.x, position.y));
            event.touch_id = touch;
            runtime.dispatch_direct_event(event);
        }
    }
}

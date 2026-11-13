//! Entity snapshots keep application input queries independent of native IDs.
use super::*;
use bevy::window::PrimaryWindow;

/// Mouse buttons capture independently; touch IDs match GuiEvent.touch_id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GuiPointerId {
    Mouse(MouseButton),
    Touch(u64),
}

/// A live GUI capture, with positions in logical window pixels. MouseDrag may
/// start before the click-suppression threshold is crossed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GuiPointerCapture {
    pointer: GuiPointerId,
    window: Entity,
    target: Entity,
    hovered: Option<Entity>,
    start: Vec2,
    position: Vec2,
    dragged: bool,
    suppress_click: bool,
}
impl GuiPointerCapture {
    pub fn pointer(&self) -> GuiPointerId {
        self.pointer
    }
    pub fn window(&self) -> Entity {
        self.window
    }
    pub fn target(&self) -> Entity {
        self.target
    }
    pub fn hovered(&self) -> Option<Entity> {
        self.hovered
    }
    pub fn start_position(&self) -> Vec2 {
        self.start
    }
    pub fn position(&self) -> Vec2 {
        self.position
    }
    pub fn is_dragging(&self) -> bool {
        self.dragged
    }
    /// A threshold crossing or application drag has cancelled this press's
    /// native click. Other click rules, such as releasing over a different
    /// target, still apply independently.
    pub fn suppresses_click(&self) -> bool {
        self.suppress_click
    }
}

/// Read-only GUI ownership after Input, before PostLayout and at Sync end.
/// Captures are ordered left mouse, right mouse, then ascending touch ID. Stale,
/// hidden or modal-blocked views are excluded; unchanged snapshots retain their
/// change tick. Application code never needs to resolve a native object ID.
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct GuiInputState {
    hovered: Option<Entity>,
    captures: Vec<GuiPointerCapture>,
    focused: Option<Entity>,
    wants_pointer: bool,
}
impl GuiInputState {
    pub fn hovered(&self) -> Option<Entity> {
        self.hovered
    }
    pub fn captured(&self) -> Option<Entity> {
        self.captures.first().map(GuiPointerCapture::target)
    }
    pub fn captures(&self) -> &[GuiPointerCapture] {
        &self.captures
    }
    pub fn capture(&self, pointer: GuiPointerId) -> Option<&GuiPointerCapture> {
        self.captures
            .iter()
            .find(|capture| capture.pointer == pointer)
    }
    pub fn captured_touch(&self, id: u64) -> Option<Entity> {
        self.capture(GuiPointerId::Touch(id))
            .map(GuiPointerCapture::target)
    }
    pub fn hovered_touch(&self, id: u64) -> Option<Entity> {
        self.capture(GuiPointerId::Touch(id))
            .and_then(GuiPointerCapture::hovered)
    }
    /// Actual input view, including focus requested through a compound control.
    pub fn focused_entity(&self) -> Option<Entity> {
        self.focused
    }
    pub fn wants_pointer_input(&self) -> bool {
        self.wants_pointer
    }
    pub fn wants_keyboard_input(&self) -> bool {
        self.focused.is_some()
    }
}

pub(crate) fn sync_state(
    mut state: ResMut<GuiInputState>,
    router: Res<InputRouter>,
    runtime: NonSend<GuiRuntime>,
    views: Query<(Entity, &GObjectRef)>,
    settings: Res<GuiInputSettings>,
    windows: Query<(Entity, &Window, Has<PrimaryWindow>)>,
) {
    let window = windows
        .iter()
        .find(|(entity, _, primary)| settings.window.map_or(*primary, |chosen| chosen == *entity));
    let active = window.is_some_and(|(_, window, _)| window.focused);
    let mouse_active = active
        && window
            .is_some_and(|(_, window, _)| window.cursor_position().is_some_and(|p| p.is_finite()));
    let resolve = |object| {
        if !eligible(&runtime, object) {
            return None;
        }
        runtime
            .entity_of(object)
            .filter(|&entity| {
                views
                    .get(entity)
                    .is_ok_and(|(_, reference)| reference.0 == object)
            })
            // The input router can also be installed on manually materialized
            // views without FairyGuiPlugin's native-to-view map.
            .or_else(|| {
                views
                    .iter()
                    .filter(|(_, reference)| reference.0 == object)
                    .map(|(entity, _)| entity)
                    .min_by_key(|entity| entity.to_bits())
            })
    };
    let hovered = mouse_active
        .then_some(router.hovered)
        .flatten()
        .and_then(resolve);
    let mut next = GuiInputState {
        hovered,
        ..default()
    };
    next.wants_pointer = mouse_active
        && router
            .hovered
            .is_some_and(|object| eligible(&runtime, object));
    let mut push = |pointer, capture_window, object, hovered, start, position, dragged, moved| {
        // Retain arbitration even for a native logical recipient whose view
        // has not materialized yet. Only live entity targets enter captures.
        next.wants_pointer = true;
        if let Some(target) = resolve(object) {
            next.captures.push(GuiPointerCapture {
                pointer,
                window: capture_window,
                target,
                hovered,
                start,
                position,
                dragged,
                suppress_click: moved,
            });
        }
    };
    if mouse_active {
        if let Some(capture) = &router.capture
            && window.is_some_and(|(entity, _, _)| entity == capture.window)
            && eligible(&runtime, capture.target)
            && capture.gesture.valid(&runtime)
        {
            push(
                GuiPointerId::Mouse(MouseButton::Left),
                capture.window,
                capture.target,
                hovered,
                capture.start,
                capture.last,
                capture.dragged,
                capture.moved,
            );
        }
        if let Some(capture) = &router.right_capture
            && window.is_some_and(|(entity, _, _)| entity == capture.window)
            && eligible(&runtime, capture.target)
        {
            push(
                GuiPointerId::Mouse(MouseButton::Right),
                capture.window,
                capture.target,
                hovered,
                capture.start,
                capture.last,
                capture.dragged,
                capture.moved,
            );
        }
    }
    if active {
        let mut touches: Vec<_> = router.touches.iter().collect();
        touches.sort_by_key(|(id, _)| **id);
        for (&id, capture) in touches {
            if window.is_some_and(|(entity, _, _)| entity == capture.window)
                && eligible(&runtime, capture.target)
                && capture.gesture.valid(&runtime)
            {
                push(
                    GuiPointerId::Touch(id),
                    capture.window,
                    capture.target,
                    capture.hovered.and_then(resolve),
                    capture.start,
                    capture.last,
                    capture.dragged,
                    capture.moved,
                );
            }
        }
    }
    next.focused = router.focused_entity.filter(|&entity| {
        views
            .get(entity)
            .is_ok_and(|(_, object)| resolve(object.0) == Some(entity))
    });
    if *state != next {
        *state = next;
    }
}

//! Pointer targets, independent captures and exclusive widget gesture owners.
//! Read `GuiInputState` after `GuiSystems::Input` to arbitrate game input.
mod actions;
pub(crate) mod editing;
pub use editing::GuiInputCommand;
mod gestures;
mod masks;
mod touches;

use crate::runtime::{GObjectRef, GuiRuntime};
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::picking::pointer::PointerId;
use bevy::picking::{
    backend::HitData,
    hover::{HoverMap, Hovered},
};
use bevy::prelude::*;
use bevy::ui::{ComputedStackIndex, RelativeCursorPosition};
use fairygui_core::geom::Point;
use fairygui_core::object::{EventKind, GObjectId, UiEvent};
use gestures::{Gesture, PointerTransform};
use std::collections::HashMap;

#[derive(bevy::ecs::system::SystemParam)]
struct PointerHover<'w, 's> {
    map: Option<Res<'w, HoverMap>>,
    parents: Query<'w, 's, &'static ChildOf>,
}

impl PointerHover<'_, '_> {
    fn hits(&self, pointer: PointerId) -> Option<HashMap<Entity, (usize, &HitData)>> {
        let map = self.map.as_ref()?;
        let mut result = HashMap::new();
        if let Some(hits) = map.get(&pointer) {
            for (&entity, hit) in hits {
                let mut current = Some(entity);
                let mut distance = 0;
                while let Some(entity) = current {
                    // Text spans can be the picked entity while their owning UI
                    // node carries GObjectRef. Mirror Bevy's inherited hover per pointer.
                    let entry = result.entry(entity).or_insert((distance, hit));
                    if distance < entry.0 {
                        *entry = (distance, hit);
                    }
                    current = self.parents.get(entity).ok().map(ChildOf::parent);
                    distance += 1;
                }
            }
        }
        Some(result)
    }
}

type PointerTargets<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static GObjectRef,
        &'static Hovered,
        &'static ComputedNode,
        Option<&'static ComputedStackIndex>,
        Option<&'static RelativeCursorPosition>,
        Option<&'static crate::runtime::PixelHitTest>,
        Option<&'static crate::runtime::HitAreaFromChild>,
        Option<&'static crate::graph::PolygonSurface>,
    ),
>;

type PointerText<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static crate::runtime::RichTextBlock>,
        Option<&'static crate::runtime::BitmapRichText>,
        Option<&'static bevy::text::ComputedTextBlock>,
        Option<&'static bevy::text::TextLayoutInfo>,
    ),
>;

fn pointer_anchor(
    runtime: &GuiRuntime,
    target: GObjectId,
    targets: &PointerTargets,
    geometry: &Query<(&UiGlobalTransform, &ComputedNode)>,
    scale_factor: f32,
) -> Vec2 {
    targets
        .iter()
        .find(|(_, object, _, _, _, _, _, _, surface)| object.0 == target && surface.is_none())
        .and_then(|(entity, ..)| {
            let (global, computed) = geometry.get(entity).ok()?;
            let physical = global.transform_point2(computed.border_box().min);
            let root_geometry = runtime
                .root_container()
                .and_then(|id| runtime.entity_of(id))
                .and_then(|entity| geometry.get(entity).ok());
            Some(if let Some((root_global, root_computed)) = root_geometry {
                (root_global.try_inverse()?.transform_point2(physical)
                    - root_computed.content_box().min)
                    * root_computed.inverse_scale_factor()
            } else {
                physical / scale_factor
            })
        })
        .unwrap_or_else(|| {
            runtime
                .store()
                .object(target)
                .map_or(Vec2::ZERO, |object| Vec2::new(object.x(), object.y()))
        })
}

fn press_popup(
    runtime: &mut GuiRuntime,
    commands: &mut Commands,
    target: GObjectId,
    input_child: bool,
    anchor: Vec2,
    viewport: fairygui_core::Size,
    assets: &Assets<crate::asset::FuiAsset>,
) {
    let size = runtime
        .store()
        .object(target)
        .map_or(fairygui_core::Size::ZERO, |o| o.size());
    let anchor = fairygui_core::Rect::new(anchor.x, anchor.y, size.width, size.height);
    runtime.toggle_button_popup(target, anchor, viewport);
    let is_combo = matches!(
        runtime.store().object(target).map(|o| o.control()),
        Some(fairygui_core::widget::ControlData::ComboBox(_))
    );
    if is_combo && !input_child {
        runtime.ensure_root(viewport);
        if let Some(handle) = runtime.asset_handle_for(target)
            && let Some(asset) = assets.get(&handle)
        {
            runtime.show_combo_dropdown_at(commands, target, &asset.package, anchor);
        }
    }
}

/// Bevy normalizes around the node center; FairyGUI uses its top-left.
fn top_left_position(cursor: &RelativeCursorPosition) -> Option<Vec2> {
    cursor
        .normalized
        .map(|position| position + Vec2::splat(0.5))
}

fn graph_hit(
    runtime: &GuiRuntime,
    id: GObjectId,
    normalized: Option<Vec2>,
    surface: Option<&crate::graph::PolygonSurface>,
) -> bool {
    use fairygui_core::{
        graph::{polygon_contains, polygon_points},
        widget::{ControlData, GraphType},
    };
    let Some(object) = runtime.store().object(id) else {
        return false;
    };
    let ControlData::Graph(data) = object.control() else {
        return true;
    };
    match data.graph_kind() {
        // Exported scrollbar skins may use an invisible graph for the track.
        // Native GScrollBar handles its container background as a hit region.
        GraphType::Empty => {
            object.name() == fairygui_core::scroll::BAR_NAME
                && runtime
                    .store()
                    .scroll_bar_of_part(id)
                    .is_some_and(|bar| runtime.store().is_scroll_bar_overlay(bar))
        }
        GraphType::Polygon | GraphType::RegularPolygon => {
            let Some(n) = normalized else {
                return false;
            };
            let p = if let Some(surface) = surface {
                Point::new(
                    surface.canvas.x + n.x * surface.canvas.z,
                    surface.canvas.y + n.y * surface.canvas.w,
                )
            } else {
                Point::new(n.x * object.width(), n.y * object.height())
            };
            polygon_contains(&polygon_points(data, object.size()), p)
        }
        _ => true,
    }
}

/// Compatibility marker. Pointer history now belongs to `GuiInputState`.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct InteractionState;

/// Pointer settings. Distances use logical window pixels.
#[derive(Resource, Debug, Clone)]
pub struct GuiInputSettings {
    /// Input window; `None` selects the primary window.
    pub window: Option<Entity>,
    pub drag_threshold: f32,
}
impl Default for GuiInputSettings {
    fn default() -> Self {
        Self {
            window: None,
            drag_threshold: 6.0,
        }
    }
}

/// Application view of GUI pointer ownership.
#[derive(Resource, Debug, Default)]
pub struct GuiInputState {
    hovered: Option<GObjectId>,
    capture: Option<Capture>,
    right_capture: Option<RightCapture>,
    touches: HashMap<u64, TouchCapture>,
    last_position: Option<Vec2>,
    focused: Option<GObjectId>,
    focused_entity: Option<Entity>,
    text_press: Option<(Entity, Vec2, f64, u8)>,
}
impl GuiInputState {
    pub fn hovered(&self) -> Option<GObjectId> {
        self.hovered
    }
    pub fn captured(&self) -> Option<GObjectId> {
        self.capture
            .as_ref()
            .map(|c| c.target)
            .or_else(|| self.right_capture.as_ref().map(|c| c.target))
            .or_else(|| {
                self.touches
                    .iter()
                    .min_by_key(|(id, _)| *id)
                    .map(|(_, c)| c.target)
            })
    }
    /// The target captured by a particular active finger.
    pub fn captured_touch(&self, id: u64) -> Option<GObjectId> {
        self.touches.get(&id).map(|capture| capture.target)
    }
    /// The current eligible hit for a captured finger, which may leave its target.
    pub fn hovered_touch(&self, id: u64) -> Option<GObjectId> {
        self.touches.get(&id).and_then(|capture| capture.hovered)
    }
    pub fn wants_pointer_input(&self) -> bool {
        self.hovered.is_some()
            || self.capture.is_some()
            || self.right_capture.is_some()
            || !self.touches.is_empty()
    }
    pub fn focused_input(&self) -> Option<GObjectId> {
        self.focused
    }
    /// The actual focused input view, including when a compound label was focused.
    pub fn focused_entity(&self) -> Option<Entity> {
        self.focused_entity
    }
    pub fn wants_keyboard_input(&self) -> bool {
        self.focused.is_some()
    }
}

#[derive(Debug)]
struct Capture {
    target: GObjectId,
    start: Vec2,
    last: Vec2,
    moved: bool,
    dragged: bool,
    gesture: Gesture,
    link: Option<crate::runtime::LinkHit>,
    editing: Option<Entity>,
}

#[derive(Debug)]
struct RightCapture {
    target: GObjectId,
    start: Vec2,
    last: Vec2,
    moved: bool,
    dragged: bool,
}

#[derive(Debug)]
struct TouchCapture {
    target: GObjectId,
    window: Entity,
    start: Vec2,
    last: Vec2,
    moved: bool,
    dragged: bool,
    gesture: Gesture,
    link: Option<crate::runtime::LinkHit>,
    editing: Option<Entity>,
    hovered: Option<GObjectId>,
}

pub fn register(app: &mut App) {
    app.init_resource::<GuiInputSettings>()
        .init_resource::<GuiInputState>()
        .add_message::<bevy::input::touch::TouchInput>()
        .add_systems(
            Update,
            (
                crate::runtime::sync_text_inputs,
                crate::runtime::sync_input_policy,
                crate::runtime::apply_input_edits,
                route_pointer,
                touches::route_touches,
                editing::reconcile_focus,
            )
                .chain()
                .in_set(crate::GuiSystems::Input),
        );
    editing::register(app);
    app.add_systems(
        Update,
        masks::sync
            .in_set(crate::GuiSystems::Sync)
            .after(crate::runtime::sync_stage_hierarchy),
    );
}

/// Shared policy for clicks, wheel input, capture and drop targets.
pub(crate) fn eligible(runtime: &GuiRuntime, target: GObjectId) -> bool {
    // Plain GTextField keeps mouseEnabled=false. A picked button title can
    // still delegate to its logical control; standalone titles must pass through.
    // Markup fields retain the existing vector/bitmap link interaction path.
    if runtime.store().object(target).is_some_and(|object| {
        object.kind() == fairygui_core::package::ObjectType::Text
            && stable_target(runtime, target) == target
            && object
                .control()
                .as_text_field()
                .is_none_or(|text| !text.ubb_enabled && !text.html_enabled)
    }) {
        return false;
    }
    if !runtime.store().is_on_stage(target) {
        return false;
    }
    // GRoot is opaque=false. Its full-window layout box organizes popups and
    // windows; it must not cover independent GUI roots created before it.
    if runtime.root_container() == Some(target) {
        return false;
    }
    // Groups organize siblings; their bounding rectangle is not a hit surface.
    if runtime
        .store()
        .object(target)
        .is_some_and(|o| o.kind() == fairygui_core::package::ObjectType::Group)
    {
        return false;
    }
    if !runtime.is_in_top_modal(target) {
        return false;
    }
    let mut current = Some(target);
    while let Some(id) = current {
        let Some(object) = runtime.store().object(id) else {
            return false;
        };
        if !runtime.store().display_visible(id) || !object.touchable() {
            return false;
        }
        current = object.parent();
    }
    true
}

fn dispatch(runtime: &mut GuiRuntime, kind: EventKind, target: GObjectId, cursor: Vec2) {
    runtime.dispatch_event(UiEvent::new(kind, target).with_pointer(Point::new(cursor.x, cursor.y)));
}

fn update_button(
    runtime: &mut GuiRuntime,
    target: GObjectId,
    hovered: Option<bool>,
    pressed: Option<bool>,
) {
    let stop = (pressed == Some(true))
        .then(|| runtime.store().scroll_bar_pointer_down_stop(target))
        .flatten();
    let mut current = Some(target);
    while let Some(id) = current {
        let Some(object) = runtime.store().object(id) else {
            return;
        };
        if matches!(
            object.control(),
            fairygui_core::widget::ControlData::Button(_)
                | fairygui_core::widget::ControlData::ComboBox(_)
        ) {
            if let Some(hovered) = hovered {
                runtime.store_mut().set_button_hovered(id, hovered);
                runtime.store_mut().set_combo_hovered(id, hovered);
            }
            if let Some(pressed) = pressed {
                runtime.store_mut().set_button_pressed(id, pressed);
                runtime.store_mut().set_combo_pressed(id, pressed);
            }
            return;
        }
        if Some(id) == stop {
            return;
        }
        current = object.parent();
    }
}

fn update_button_from_pointers(runtime: &mut GuiRuntime, state: &GuiInputState, target: GObjectId) {
    let Some(button) = pointer_button(runtime, target, false) else {
        return;
    };
    let hovered = state
        .hovered
        .and_then(|id| pointer_button(runtime, id, false))
        == Some(button)
        || state.touches.values().any(|capture| {
            capture
                .hovered
                .and_then(|id| pointer_button(runtime, id, false))
                == Some(button)
        });
    let pressed = state
        .capture
        .as_ref()
        .is_some_and(|capture| pointer_button(runtime, capture.target, true) == Some(button))
        || state
            .touches
            .values()
            .any(|capture| pointer_button(runtime, capture.target, true) == Some(button));
    update_button(runtime, button, Some(hovered), Some(pressed));
}

fn pointer_button(runtime: &GuiRuntime, target: GObjectId, pressing: bool) -> Option<GObjectId> {
    let stop = pressing
        .then(|| runtime.store().scroll_bar_pointer_down_stop(target))
        .flatten();
    let mut current = Some(target);
    while let Some(id) = current {
        let object = runtime.store().object(id)?;
        if matches!(
            object.control(),
            fairygui_core::widget::ControlData::Button(_)
                | fairygui_core::widget::ControlData::ComboBox(_)
        ) {
            return Some(id);
        }
        if Some(id) == stop {
            return None;
        }
        current = object.parent();
    }
    None
}

// A button's controller swaps its background children on hover/down. Capture
// the button itself so that hiding the old background cannot cancel the press.
fn stable_target(runtime: &GuiRuntime, target: GObjectId) -> GObjectId {
    let mut current = Some(target);
    while let Some(id) = current {
        if runtime.store().is_scroll_overlay(id) {
            return target;
        }
        let Some(object) = runtime.store().object(id) else {
            break;
        };
        if matches!(
            object.control(),
            fairygui_core::widget::ControlData::Button(_)
                | fairygui_core::widget::ControlData::ComboBox(_)
        ) {
            return id;
        }
        current = object.parent();
    }
    target
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn route_pointer(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    settings: Res<GuiInputSettings>,
    mut state: ResMut<GuiInputState>,
    windows: Query<(Entity, &Window, Option<&bevy::window::PrimaryWindow>)>,
    targets: PointerTargets,
    rich_text: PointerText,
    geometry: Query<(&UiGlobalTransform, &ComputedNode)>,
    mut wheel: MessageReader<MouseWheel>,
    time: Res<Time>,
    assets: Res<Assets<crate::asset::FuiAsset>>,
    mut runtime: NonSendMut<GuiRuntime>,
    mut focus: ResMut<bevy::input_focus::InputFocus>,
    keys: Res<ButtonInput<bevy::input::keyboard::Key>>,
    mut editors: Query<&mut bevy::text::EditableText>,
    hover: PointerHover,
) {
    runtime
        .store_mut()
        .advance_button_feedback(time.delta_secs_f64());
    runtime.advance_menu_click(time.delta_secs());
    let window = windows.iter().find(|(entity, _, primary)| {
        settings
            .window
            .map_or(primary.is_some(), |chosen| chosen == *entity)
    });
    let cursor = window
        .filter(|(_, w, _)| w.focused)
        .and_then(|(_, w, _)| w.cursor_position());
    let mouse_hits = hover.hits(PointerId::Mouse);
    let raw_hit = cursor.and_then(|_| {
        targets
            .iter()
            .filter(
                |(entity, object, hovered, _, _, relative, pixel, child, surface)| {
                    mouse_hits
                        .as_ref()
                        .map_or(hovered.0, |hits| hits.contains_key(entity))
                        && child.is_none()
                        && eligible(&runtime, object.0)
                        && masks::allowed(
                            &runtime,
                            object.0,
                            cursor.unwrap() * window.map_or(1., |(_, w, _)| w.scale_factor()),
                            &geometry,
                        )
                        && relative.is_none_or(|r| r.cursor_over())
                        && graph_hit(
                            &runtime,
                            object.0,
                            relative.and_then(top_left_position),
                            *surface,
                        )
                        && pixel.is_none_or(|p| {
                            relative.and_then(top_left_position).is_some_and(|n| {
                                runtime.store().object(object.0).is_some_and(|o| {
                                    p.0.contains(n.x * o.width(), n.y * o.height())
                                })
                            })
                        })
                },
            )
            .max_by_key(|(entity, _, _, _, stack, _, _, _, _)| {
                (
                    stack.map_or(0, |s| s.0),
                    std::cmp::Reverse(
                        mouse_hits
                            .as_ref()
                            .and_then(|hits| hits.get(entity))
                            .map_or(0, |hit| hit.0),
                    ),
                    entity.to_bits(),
                )
            })
    });
    let hit = raw_hit.map(|(_, object, _, _, _, _, _, _, _)| stable_target(&runtime, object.0));
    let link = raw_hit.and_then(|(entity, object, _, computed, _, relative, _, _, _)| {
        let normalized = relative.and_then(top_left_position)?;
        let (rich, bitmap, block, layout) = rich_text.get(entity).ok()?;
        if let Some(bitmap) = bitmap {
            bitmap.link_at(&runtime, object.0, normalized, computed)
        } else {
            rich?.link_at(&runtime, object.0, normalized, computed, block?, layout?)
        }
    });
    let position = cursor.unwrap_or_else(|| {
        state
            .capture
            .as_ref()
            .map(|capture| capture.last)
            .or_else(|| state.right_capture.as_ref().map(|capture| capture.last))
            .unwrap_or(Vec2::ZERO)
    });
    let pointer_moved = cursor.is_some()
        && state
            .last_position
            .is_some_and(|previous| previous != position);
    state.last_position = cursor.map(|_| position);
    if hit != state.hovered {
        let old_hovered = state.hovered;
        if let Some(old) = old_hovered
            && runtime.store().object(old).is_some()
        {
            dispatch(&mut runtime, EventKind::RollOut, old, position);
        }
        if let Some(new) = hit {
            dispatch(&mut runtime, EventKind::RollOver, new, position);
        }
        state.hovered = hit;
        if let Some(old) = old_hovered {
            update_button_from_pointers(&mut runtime, &state, old);
        }
        if let Some(new) = hit {
            update_button_from_pointers(&mut runtime, &state, new);
        }
    }
    if pointer_moved && let Some(target) = hit {
        dispatch(&mut runtime, EventKind::MouseMove, target, position);
    }
    if let Some(mut capture) = state.capture.take() {
        if runtime.is_dragging() && runtime.drag_touch_id.is_none() {
            capture.gesture.finish(&mut runtime, time.delta_secs());
            capture.gesture = Gesture::None;
            capture.moved = true;
        }
        let valid = cursor.is_some()
            && eligible(&runtime, capture.target)
            && capture.gesture.valid(&runtime);
        capture.moved |= position.distance(capture.start) > settings.drag_threshold.max(0.0);
        if position != capture.last {
            capture.dragged = true;
            if runtime.store().object(capture.target).is_some() {
                dispatch(&mut runtime, EventKind::MouseDrag, capture.target, position);
            }
        }
        if valid && mouse.pressed(MouseButton::Left) {
            if let Some(entity) = capture.editing
                && capture.moved
                && let Ok((transform, node)) = geometry.get(entity)
                && let Some(inverse) = transform.try_inverse()
                && let Ok(mut editor) = editors.get_mut(entity)
                && !editor.is_composing()
            {
                let point = inverse
                    .transform_point2(position * window.map_or(1., |(_, w, _)| w.scale_factor()))
                    - node.content_box().min
                    + editor.viewport.offset;
                editor.queue_edit(bevy::text::TextEdit::ExtendSelectionToPoint(point));
            }
            let normalized = targets
                .iter()
                .filter_map(|(_, o, _, _, _, r, _, _, surface)| {
                    if surface.is_some() {
                        return None;
                    }
                    r.and_then(top_left_position).map(|n| (o.0, n))
                })
                .collect::<Vec<_>>();
            capture.gesture.update(
                &mut runtime,
                position,
                capture.last,
                capture.start,
                capture.moved,
                time.delta_secs(),
                &normalized,
            );
            capture.last = position;
            state.capture = Some(capture);
        } else {
            capture.gesture.finish(&mut runtime, time.delta_secs());
            if runtime.store().object(capture.target).is_some() {
                dispatch(&mut runtime, EventKind::MouseUp, capture.target, position);
                if capture.dragged {
                    dispatch(
                        &mut runtime,
                        EventKind::MouseDragEnd,
                        capture.target,
                        position,
                    );
                }
                update_button_from_pointers(&mut runtime, &state, capture.target);
            }
            if valid
                && mouse.just_released(MouseButton::Left)
                && !capture.moved
                && hit == Some(capture.target)
                && !runtime.is_dragging()
            {
                if let Some(link) = capture
                    .link
                    .filter(|pressed| Some(pressed) == link.as_ref())
                {
                    runtime.dispatch_event(
                        UiEvent::new(EventKind::Link, link.object)
                            .with_pointer(Point::new(position.x, position.y))
                            .with_data(fairygui_core::object::EventData::Text(link.href)),
                    );
                }
                dispatch(&mut runtime, EventKind::Click, capture.target, position);
                apply_click(
                    &mut runtime,
                    &mut commands,
                    capture.target,
                    position,
                    keys.pressed(bevy::input::keyboard::Key::Shift),
                    keys.pressed(bevy::input::keyboard::Key::Control),
                    None,
                );
            }
        }
    }
    if let Some(mut capture) = state.right_capture.take() {
        capture.moved |= position.distance(capture.start) > settings.drag_threshold.max(0.0);
        let valid = cursor.is_some() && eligible(&runtime, capture.target);
        if position != capture.last {
            capture.dragged = true;
            if runtime.store().object(capture.target).is_some() {
                dispatch(&mut runtime, EventKind::MouseDrag, capture.target, position);
            }
        }
        if valid && mouse.pressed(MouseButton::Right) {
            capture.last = position;
            state.right_capture = Some(capture);
        } else {
            if runtime.store().object(capture.target).is_some() {
                dispatch(
                    &mut runtime,
                    EventKind::RightMouseUp,
                    capture.target,
                    position,
                );
                if capture.dragged {
                    dispatch(
                        &mut runtime,
                        EventKind::MouseDragEnd,
                        capture.target,
                        position,
                    );
                }
            }
            if valid
                && mouse.just_released(MouseButton::Right)
                && !capture.moved
                && hit == Some(capture.target)
                && !runtime.is_dragging()
            {
                dispatch(
                    &mut runtime,
                    EventKind::RightClick,
                    capture.target,
                    position,
                );
            }
        }
    }
    if mouse.just_pressed(MouseButton::Right)
        && cursor.is_some()
        && let Some(target) = hit
    {
        let stop = runtime.store().scroll_bar_pointer_down_stop(target);
        runtime.dispatch_event_until(
            UiEvent::new(EventKind::RightMouseDown, target)
                .with_pointer(Point::new(position.x, position.y)),
            stop,
        );
        state.right_capture = Some(RightCapture {
            target,
            start: position,
            last: position,
            moved: false,
            dragged: false,
        });
    }
    if mouse.just_pressed(MouseButton::Left) && cursor.is_some() {
        let input = raw_hit
            .filter(|(entity, owner, ..)| {
                editors.contains(*entity)
                    && runtime.store().input_field_of(owner.0) == Some(owner.0)
            })
            .map(|(entity, ..)| entity);
        if let Some(entity) = input {
            focus.set(entity, bevy::input_focus::FocusCause::Pressed);
            if let Ok((transform, node)) = geometry.get(entity)
                && let Some(inverse) = transform.try_inverse()
                && let Ok(mut editor) = editors.get_mut(entity)
                && !editor.is_composing()
            {
                let point = inverse
                    .transform_point2(position * window.map_or(1., |(_, w, _)| w.scale_factor()))
                    - node.content_box().min
                    + editor.viewport.offset;
                let now = time.elapsed_secs_f64();
                let count = state
                    .text_press
                    .filter(|(old, xy, then, _)| {
                        *old == entity
                            && now - then <= 0.3
                            && xy.distance(position) <= settings.drag_threshold.max(0.)
                    })
                    .map_or(1, |(_, _, _, count)| count % 3 + 1);
                state.text_press = Some((entity, position, now, count));
                let edit = if keys.pressed(bevy::input::keyboard::Key::Shift) {
                    bevy::text::TextEdit::ShiftClickExtension(point)
                } else if count == 2 {
                    bevy::text::TextEdit::SelectWordAtPoint(point)
                } else if count == 3 {
                    bevy::text::TextEdit::SelectAll
                } else {
                    bevy::text::TextEdit::MoveToPoint(point)
                };
                editor.queue_edit(edit);
            }
        } else if runtime.focused_input().is_some() {
            focus.clear();
            state.text_press = None;
        }
        runtime.check_popups(raw_hit.map(|(_, object, ..)| object.0));
        if let Some(target) = hit {
            let stop = runtime.store().scroll_bar_pointer_down_stop(target);
            runtime.dispatch_event_until(
                UiEvent::new(EventKind::MouseDown, target)
                    .with_pointer(Point::new(position.x, position.y)),
                stop,
            );
            update_button(&mut runtime, target, None, Some(true));
            let local = targets
                .iter()
                .find(|(_, object, _, _, _, _, _, _, surface)| {
                    object.0 == target && surface.is_none()
                })
                .and_then(|(_, _, _, _, _, relative, _, _, _)| relative.and_then(top_left_position))
                .and_then(|p| {
                    runtime
                        .store()
                        .object(target)
                        .map(|o| Point::new(p.x * o.width(), p.y * o.height()))
                });
            runtime.store_mut().press_scroll_bar(target, local);
            let scale_factor = window.map_or(1.0, |(_, w, _)| w.scale_factor());
            let anchor = pointer_anchor(&runtime, target, &targets, &geometry, scale_factor);
            let size = runtime
                .store()
                .object(target)
                .map_or(fairygui_core::Size::ZERO, |o| o.size());
            let viewport = window.map_or(size, |(_, w, _)| {
                fairygui_core::Size::new(w.width(), w.height())
            });
            let input_child = raw_hit.is_some_and(|(_, object, ..)| {
                matches!(
                    runtime.store().object(object.0).map(|o| o.control()),
                    Some(fairygui_core::widget::ControlData::TextInput(_))
                )
            });
            press_popup(
                &mut runtime,
                &mut commands,
                target,
                input_child,
                anchor,
                viewport,
                &assets,
            );
            let window_scale_factor = window.map_or(1.0, |(_, window, _)| window.scale_factor());
            let gesture_transforms: Vec<_> = targets
                .iter()
                .filter_map(|(entity, object, ..)| {
                    let (transform, node) = geometry.get(entity).ok()?;
                    PointerTransform::from_view(transform, node, window_scale_factor)
                        .map(|transform| (object.0, transform))
                })
                .collect();
            let mut gesture = if input.is_some() {
                Gesture::None
            } else {
                let occupied: Vec<_> = state
                    .touches
                    .values()
                    .filter_map(|capture| capture.gesture.owner(&runtime))
                    .collect();
                Gesture::begin_excluding(&mut runtime, target, &gesture_transforms, &occupied)
            };
            let normalized = targets
                .iter()
                .filter_map(|(_, o, _, _, _, r, _, _, surface)| {
                    if surface.is_some() {
                        return None;
                    }
                    r.and_then(top_left_position).map(|n| (o.0, n))
                })
                .collect::<Vec<_>>();
            gesture.update(
                &mut runtime,
                position,
                position,
                position,
                false,
                0.,
                &normalized,
            );
            state.capture = Some(Capture {
                target,
                start: position,
                last: position,
                moved: false,
                dragged: false,
                gesture,
                link,
                editing: input,
            });
        }
    }
    for event in wheel.read() {
        if window.is_none_or(|(entity, _, _)| entity != event.window) {
            continue;
        }
        let Some((_, object, ..)) = raw_hit else {
            continue;
        };
        if !event.x.is_finite() || !event.y.is_finite() {
            continue;
        }
        let target = object.0;
        // A release or popup toggle above may have invalidated this frame's hit.
        if !eligible(&runtime, target) {
            continue;
        }
        runtime.dispatch_event(
            UiEvent::new(EventKind::MouseWheel, target)
                .with_pointer(Point::new(position.x, position.y))
                .with_data(fairygui_core::object::EventData::Delta(event.y)),
        );
        if event.unit == MouseScrollUnit::Line {
            // Bevy positive Y points up; native Laya delta points toward the end.
            runtime.store_mut().scroll_mouse_wheel(target, -event.y);
        } else {
            runtime
                .store_mut()
                .scroll_mouse_wheel_pixels(target, -event.x, -event.y);
        }
    }
    if runtime.is_dragging() && runtime.drag_touch_id.is_none() {
        runtime.update_drag(Point::new(position.x, position.y), hit);
        if cursor.is_none() || !mouse.pressed(MouseButton::Left) {
            if cursor.is_some() && mouse.just_released(MouseButton::Left) {
                runtime.finish_drag(&mut commands);
            } else {
                runtime.cancel_drag(&mut commands);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_click(
    runtime: &mut GuiRuntime,
    commands: &mut Commands,
    target: GObjectId,
    cursor: Vec2,
    shift: bool,
    ctrl: bool,
    touch_id: Option<u64>,
) {
    let mut button = Some(target);
    while let Some(id) = button {
        let Some(object) = runtime.store().object(id) else {
            break;
        };
        if matches!(
            object.control(),
            fairygui_core::widget::ControlData::Button(_)
                | fairygui_core::widget::ControlData::ComboBox(_)
        ) {
            if runtime.store_mut().click_button(id) {
                let mut event = UiEvent::new(EventKind::StateChanged, id)
                    .with_pointer(Point::new(cursor.x, cursor.y));
                event.touch_id = touch_id;
                runtime.dispatch_event(event);
            }
            break;
        }
        button = object.parent();
    }
    if runtime.store().registered_scroll_bar_of(target).is_some() {
        return;
    }
    runtime.store_mut().scroll_clicked_list_item_to_view(target);
    if let Some(index) = actions::close_button_window(runtime, target) {
        let object = runtime.window(index).unwrap().owner();
        if runtime.is_ecs_window_object(object) {
            runtime.hide_window(index);
        } else {
            runtime.close_window(commands, index);
        }
    } else if let Some(hit) = runtime.popup_menu_hit(target) {
        if hit.is_some() {
            runtime.activate_menu_item(target, Some(Point::new(cursor.x, cursor.y)));
        }
    } else if !actions::combo_click(runtime, target, cursor, touch_id) {
        runtime
            .store_mut()
            .select_clicked_item_with_modifiers(target, shift, ctrl);
        runtime.store_mut().toggle_clicked_tree_node(target);
        runtime.emit_list_item_click(
            target,
            Point::new(cursor.x, cursor.y),
            shift,
            ctrl,
            touch_id,
        );
    }
}

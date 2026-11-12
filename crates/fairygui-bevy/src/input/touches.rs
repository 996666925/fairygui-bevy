//! Independent touch capture using Bevy's per-pointer picking results.
use super::*;
use bevy::input::touch::{TouchInput, TouchPhase};
use fairygui_core::object::EventData;

#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct TouchViews<'w, 's> {
    targets: Query<'w, 's, &'static bevy::ui::ComputedUiTargetCamera>,
    cameras: Query<'w, 's, &'static Camera>,
}

impl TouchViews<'_, '_> {
    fn physical_position(&self, entity: Entity, position: Vec2, scale: f32) -> Vec2 {
        let viewport_origin = self
            .targets
            .get(entity)
            .ok()
            .and_then(|target| target.get())
            .and_then(|camera| self.cameras.get(camera).ok())
            .and_then(Camera::physical_viewport_rect)
            .map_or(Vec2::ZERO, |rect| rect.min.as_vec2());
        position * scale - viewport_origin
    }
}

fn touch_event(kind: EventKind, target: GObjectId, position: Vec2, id: u64) -> UiEvent {
    UiEvent::new(kind, target)
        .with_pointer(Point::new(position.x, position.y))
        .with_touch(id)
}

fn emit(runtime: &mut GuiRuntime, kind: EventKind, target: GObjectId, position: Vec2, id: u64) {
    runtime.dispatch_event(touch_event(kind, target, position, id));
}

fn finish(runtime: &mut GuiRuntime, capture: &TouchCapture, id: u64, dt: f32) {
    capture.gesture.finish(runtime, dt);
    if runtime.store().object(capture.target).is_some() {
        emit(
            runtime,
            EventKind::TouchEnd,
            capture.target,
            capture.last,
            id,
        );
        emit(
            runtime,
            EventKind::MouseUp,
            capture.target,
            capture.last,
            id,
        );
        if capture.dragged {
            emit(
                runtime,
                EventKind::MouseDragEnd,
                capture.target,
                capture.last,
                id,
            );
        }
    }
}

fn roll_out(runtime: &mut GuiRuntime, capture: &TouchCapture, id: u64) {
    if let Some(old) = capture.hovered
        && runtime.store().object(old).is_some()
    {
        emit(runtime, EventKind::RollOut, old, capture.last, id);
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn route_touches(
    mut commands: Commands,
    mut events: MessageReader<TouchInput>,
    settings: Res<GuiInputSettings>,
    mut state: ResMut<GuiInputState>,
    windows: Query<(Entity, &Window, Option<&bevy::window::PrimaryWindow>)>,
    targets: PointerTargets,
    rich_text: PointerText,
    geometry: Query<(&UiGlobalTransform, &ComputedNode)>,
    hover: PointerHover,
    time: Res<Time>,
    assets: Res<Assets<crate::asset::FuiAsset>>,
    mut runtime: NonSendMut<GuiRuntime>,
    mut focus: ResMut<bevy::input_focus::InputFocus>,
    keys: Res<ButtonInput<bevy::input::keyboard::Key>>,
    mut editors: Query<&mut bevy::text::EditableText>,
    views: TouchViews,
) {
    let window = windows.iter().find(|(entity, _, primary)| {
        settings
            .window
            .map_or(primary.is_some(), |chosen| chosen == *entity)
    });
    // Capture policy can change without a new OS touch message.
    let cancelled: Vec<_> = state
        .touches
        .iter()
        .filter_map(|(&id, c)| {
            (window.is_none_or(|(entity, w, _)| entity != c.window || !w.focused)
                || !eligible(&runtime, c.target)
                || !c.gesture.valid(&runtime))
            .then_some(id)
        })
        .collect();
    for id in cancelled {
        let capture = state.touches.remove(&id).unwrap();
        finish(&mut runtime, &capture, id, time.delta_secs());
        roll_out(&mut runtime, &capture, id);
        update_button_from_pointers(&mut runtime, &state, capture.target);
        if let Some(old) = capture.hovered {
            update_button_from_pointers(&mut runtime, &state, old);
        }
        if runtime.drag_touch_id == Some(id) {
            runtime.update_drag(Point::new(capture.last.x, capture.last.y), None);
            runtime.finish_drag(&mut commands);
        }
    }
    if runtime
        .drag_touch_id
        .is_some_and(|id| !state.touches.contains_key(&id))
    {
        runtime.update_drag(Point::ZERO, None);
        runtime.finish_drag(&mut commands);
    }
    let mut samples: Vec<_> = events.read().copied().collect();
    // Stationary fingers still accumulate gesture time and revalidate hover.
    for (&id, capture) in &state.touches {
        if !samples.iter().any(|event| event.id == id) {
            samples.push(TouchInput {
                id,
                window: capture.window,
                position: capture.last,
                phase: TouchPhase::Moved,
                force: None,
            });
        }
    }
    let mut counts = HashMap::<u64, usize>::new();
    for event in &samples {
        *counts.entry(event.id).or_default() += 1;
    }
    for event in samples {
        let Some((window_entity, window, _)) = window else {
            continue;
        };
        if event.window != window_entity || !window.focused || !event.position.is_finite() {
            continue;
        }
        let dt = time.delta_secs() / counts[&event.id] as f32;
        let position = event.position;
        let scale = window.scale_factor();
        let hits = hover.hits(PointerId::Touch(event.id));
        let normalized = |entity: Entity| -> Option<Vec2> {
            if let Ok((transform, node)) = geometry.get(entity)
                && node.size().min_element() > 0.0
            {
                return Some(
                    transform
                        .try_inverse()?
                        .transform_point2(views.physical_position(entity, position, scale))
                        / node.size()
                        + Vec2::splat(0.5),
                );
            }
            hits.as_ref()?
                .get(&entity)?
                .1
                .position
                .map(|p| p.truncate() + Vec2::splat(0.5))
        };
        let raw_hit = hits.as_ref().and_then(|hits| {
            targets
                .iter()
                .filter(|(entity, object, _, _, _, _, pixel, child, surface)| {
                    hits.contains_key(entity)
                        && child.is_none()
                        && eligible(&runtime, object.0)
                        && masks::allowed(
                            &runtime,
                            object.0,
                            views.physical_position(*entity, position, scale),
                            &geometry,
                        )
                        && graph_hit(&runtime, object.0, normalized(*entity), *surface)
                        && pixel.is_none_or(|p| {
                            normalized(*entity).is_some_and(|n| {
                                runtime.store().object(object.0).is_some_and(|o| {
                                    p.0.contains(n.x * o.width(), n.y * o.height())
                                })
                            })
                        })
                })
                .max_by_key(|(entity, _, _, _, stack, _, _, _, _)| {
                    (
                        stack.map_or(0, |s| s.0),
                        std::cmp::Reverse(hits.get(entity).map_or(0, |hit| hit.0)),
                        entity.to_bits(),
                    )
                })
        });
        let hit = raw_hit.map(|(_, owner, ..)| stable_target(&runtime, owner.0));
        let link = raw_hit.and_then(|(entity, owner, _, node, ..)| {
            let n = normalized(entity)?;
            let (rich, bitmap, block, layout) = rich_text.get(entity).ok()?;
            if let Some(bitmap) = bitmap {
                bitmap.link_at(&runtime, owner.0, n, node)
            } else {
                rich?.link_at(&runtime, owner.0, n, node, block?, layout?)
            }
        });
        let positions: Vec<_> = targets
            .iter()
            .filter_map(|(entity, owner, _, _, _, _, _, _, surface)| {
                surface
                    .is_none()
                    .then(|| normalized(entity).map(|p| (owner.0, p)))
                    .flatten()
            })
            .collect();
        if event.phase == TouchPhase::Started {
            if state.touches.contains_key(&event.id) {
                continue;
            }
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
                        .transform_point2(views.physical_position(entity, position, scale))
                        - node.content_box().min
                        + editor.viewport.offset;
                    editor.queue_edit(bevy::text::TextEdit::MoveToPoint(point));
                }
            } else if runtime.focused_input().is_some() {
                focus.clear();
                state.text_press = None;
            }
            runtime.check_popups(raw_hit.map(|(_, owner, ..)| owner.0));
            let Some(target) = hit.filter(|id| eligible(&runtime, *id)) else {
                continue;
            };
            emit(
                &mut runtime,
                EventKind::RollOver,
                target,
                position,
                event.id,
            );
            let stop = runtime.store().scroll_bar_pointer_down_stop(target);
            runtime.dispatch_event_until(
                touch_event(EventKind::TouchBegin, target, position, event.id),
                stop,
            );
            runtime.dispatch_event_until(
                touch_event(EventKind::MouseDown, target, position, event.id),
                stop,
            );
            update_button(&mut runtime, target, Some(true), Some(true));
            let local = positions
                .iter()
                .find(|(id, _)| *id == target)
                .and_then(|(_, n)| {
                    runtime
                        .store()
                        .object(target)
                        .map(|o| Point::new(n.x * o.width(), n.y * o.height()))
                });
            runtime.store_mut().press_scroll_bar(target, local);
            let anchor = pointer_anchor(&runtime, target, &targets, &geometry, scale);
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
                fairygui_core::Size::new(window.width(), window.height()),
                &assets,
            );
            let transforms: Vec<_> = targets
                .iter()
                .filter_map(|(entity, owner, ..)| {
                    let (transform, node) = geometry.get(entity).ok()?;
                    PointerTransform::from_view(transform, node, scale).map(|t| (owner.0, t))
                })
                .collect();
            let occupied: Vec<_> = state
                .touches
                .values()
                .map(|c| &c.gesture)
                .chain(state.capture.iter().map(|c| &c.gesture))
                .filter_map(|g| g.owner(&runtime))
                .collect();
            let mut gesture = if input.is_some() {
                Gesture::None
            } else {
                Gesture::begin_excluding(&mut runtime, target, &transforms, &occupied)
            };
            gesture.update(
                &mut runtime,
                position,
                position,
                position,
                false,
                0.0,
                &positions,
            );
            state.touches.insert(
                event.id,
                TouchCapture {
                    target,
                    window: event.window,
                    start: position,
                    last: position,
                    moved: false,
                    dragged: false,
                    gesture,
                    link,
                    editing: input,
                    hovered: hit,
                },
            );
            continue;
        }
        let Some(mut capture) = state.touches.remove(&event.id) else {
            continue;
        };
        if capture.window != event.window {
            state.touches.insert(event.id, capture);
            continue;
        }
        let previous_hover = capture.hovered;
        let pressed_target = capture.target;
        let hit = if event.phase == TouchPhase::Canceled {
            None
        } else {
            hit
        };
        let owns_drop = runtime.is_dragging() && runtime.drag_touch_id == Some(event.id);
        if owns_drop {
            capture.gesture.finish(&mut runtime, 0.0);
            capture.gesture = Gesture::None;
            capture.moved = true;
            runtime.update_drag(Point::new(position.x, position.y), hit);
        }
        if capture.hovered != hit {
            roll_out(&mut runtime, &capture, event.id);
            if let Some(target) = hit {
                emit(
                    &mut runtime,
                    EventKind::RollOver,
                    target,
                    position,
                    event.id,
                );
            }
            capture.hovered = hit;
        }
        capture.moved |= position.distance(capture.start) > settings.drag_threshold.max(0.0);
        let changed = position != capture.last;
        if changed && event.phase != TouchPhase::Canceled {
            capture.dragged = true;
            emit(
                &mut runtime,
                EventKind::TouchMove,
                capture.target,
                position,
                event.id,
            );
            if let Some(target) = hit {
                emit(
                    &mut runtime,
                    EventKind::MouseMove,
                    target,
                    position,
                    event.id,
                );
            }
            emit(
                &mut runtime,
                EventKind::MouseDrag,
                capture.target,
                position,
                event.id,
            );
        }
        if let Some(entity) = capture.editing
            && focus.get() == Some(entity)
            && capture.moved
            && event.phase != TouchPhase::Canceled
            && let Ok((transform, node)) = geometry.get(entity)
            && let Some(inverse) = transform.try_inverse()
            && let Ok(mut editor) = editors.get_mut(entity)
            && !editor.is_composing()
        {
            let point = inverse.transform_point2(views.physical_position(entity, position, scale))
                - node.content_box().min
                + editor.viewport.offset;
            editor.queue_edit(bevy::text::TextEdit::ExtendSelectionToPoint(point));
        }
        if event.phase != TouchPhase::Canceled {
            capture.gesture.update(
                &mut runtime,
                position,
                capture.last,
                capture.start,
                capture.moved,
                dt,
                &positions,
            );
        }
        capture.last = position;
        if matches!(event.phase, TouchPhase::Ended | TouchPhase::Canceled) {
            finish(&mut runtime, &capture, event.id, 0.0);
            if event.phase == TouchPhase::Ended
                && !capture.moved
                && hit == Some(capture.target)
                && eligible(&runtime, capture.target)
                && !runtime.is_dragging()
            {
                if let Some(link) = capture
                    .link
                    .as_ref()
                    .filter(|pressed| Some(*pressed) == link.as_ref())
                {
                    runtime.dispatch_event(
                        touch_event(EventKind::Link, link.object, position, event.id)
                            .with_data(EventData::Text(link.href.clone())),
                    );
                }
                emit(
                    &mut runtime,
                    EventKind::Click,
                    capture.target,
                    position,
                    event.id,
                );
                apply_click(
                    &mut runtime,
                    &mut commands,
                    capture.target,
                    position,
                    keys.pressed(bevy::input::keyboard::Key::Shift),
                    keys.pressed(bevy::input::keyboard::Key::Control),
                    Some(event.id),
                );
            }
            roll_out(&mut runtime, &capture, event.id);
            if owns_drop {
                runtime.finish_drag(&mut commands);
            }
        } else {
            state.touches.insert(event.id, capture);
        }
        if let Some(old) = previous_hover {
            update_button_from_pointers(&mut runtime, &state, old);
        }
        if let Some(target) = hit {
            update_button_from_pointers(&mut runtime, &state, target);
        }
        // The pressed target can differ from both hover recipients.
        update_button_from_pointers(&mut runtime, &state, pressed_target);
    }
}

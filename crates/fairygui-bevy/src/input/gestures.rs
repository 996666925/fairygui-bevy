//! Exclusive built-in gesture ownership and lifecycle.
use super::*;
use fairygui_core::widget::ControlData;

#[derive(Debug, Clone, Copy)]
pub(super) struct PointerTransform {
    screen_to_local: Mat2,
    local_to_screen_scale: Vec2,
}

impl PointerTransform {
    pub(super) fn from_view(
        transform: &UiGlobalTransform,
        node: &ComputedNode,
        window_scale_factor: f32,
    ) -> Option<Self> {
        let inverse = transform.try_inverse()?;
        let screen_to_local = inverse.matrix2 * (window_scale_factor * node.inverse_scale_factor);
        if !screen_to_local.x_axis.is_finite()
            || !screen_to_local.y_axis.is_finite()
            || screen_to_local.determinant().abs() <= f32::EPSILON
        {
            return None;
        }
        let local_to_screen = screen_to_local.inverse();
        let local_to_screen_scale = Vec2::new(
            local_to_screen.x_axis.length(),
            local_to_screen.y_axis.length(),
        );
        (local_to_screen_scale.is_finite() && local_to_screen_scale.min_element() > 0.0).then_some(
            Self {
                screen_to_local,
                local_to_screen_scale,
            },
        )
    }

    fn from_model_scale(scale: Vec2) -> Self {
        let scale = Vec2::new(
            if scale.x.abs() < f32::EPSILON {
                1.0
            } else {
                scale.x.abs()
            },
            if scale.y.abs() < f32::EPSILON {
                1.0
            } else {
                scale.y.abs()
            },
        );
        Self {
            screen_to_local: Mat2::from_diagonal(scale.recip()),
            local_to_screen_scale: scale,
        }
    }

    fn displacement(self, screen_delta: Vec2) -> Vec2 {
        self.screen_to_local * screen_delta
    }
}

#[derive(Debug)]
pub(super) enum Gesture {
    None,
    Slider {
        owner: GObjectId,
        grip: bool,
        initial: f64,
        transform: PointerTransform,
        applied: bool,
        touch_id: Option<u64>,
    },
    Object {
        owner: GObjectId,
        parent: GObjectId,
        motion: Option<fairygui_core::drag_drop::ObjectDrag>,
        touch_id: Option<u64>,
    },
    Grip {
        bar: GObjectId,
        container: GObjectId,
        vertical: bool,
        track: f32,
        initial: f32,
        transform: PointerTransform,
    },
    Window {
        owner: GObjectId,
        origin: Vec2,
        transform: PointerTransform,
    },
    Scroll {
        container: GObjectId,
        started: bool,
        transform: PointerTransform,
        elapsed: f32,
        candidates: Option<Vec<(GObjectId, PointerTransform)>>,
    },
}

impl Gesture {
    pub(super) fn owner(&self, runtime: &GuiRuntime) -> Option<GObjectId> {
        match self {
            Self::None => None,
            Self::Slider { owner, .. } => Some(*owner),
            Self::Object { owner, .. } => Some(*owner),
            Self::Grip { container, .. } => Some(*container),
            Self::Scroll {
                container,
                candidates,
                ..
            } => candidates
                .as_ref()
                .map_or(Some(*container), |panes| panes.first().map(|(id, _)| *id)),
            Self::Window { owner, .. } => runtime.window_index(*owner).map(|_| *owner),
        }
    }

    pub(super) fn begin_excluding(
        runtime: &mut GuiRuntime,
        target: GObjectId,
        transforms: &[(GObjectId, PointerTransform)],
        occupied: &[GObjectId],
    ) -> Self {
        if runtime.is_dragging() {
            return Self::None;
        }
        if runtime.store().is_scroll_bar_overlay(target) {
            return Self::None;
        }
        let mut current = Some(target);
        while let Some(id) = current {
            if runtime.store().is_scroll_bar_overlay(id) {
                return Self::None;
            }
            let Some(object) = runtime.store().object(id) else {
                break;
            };
            if matches!(object.control(), ControlData::Slider(_)) {
                if occupied.contains(&id) {
                    return Self::None;
                }
                let grip_object = runtime.store().child_by_name(id, "grip");
                let mut part = Some(target);
                let mut grip = false;
                while let Some(part_id) = part {
                    if Some(part_id) == grip_object {
                        grip = true;
                        break;
                    }
                    if part_id == id {
                        break;
                    }
                    part = runtime
                        .store()
                        .object(part_id)
                        .and_then(|object| object.parent());
                }
                let initial = runtime.store().slider_percent(id).unwrap_or_default();
                let transform = pointer_transform(runtime, id, transforms);
                // GSlider's grip-down handler explicitly enables this flag.
                if grip {
                    runtime.store_mut().set_slider_can_drag(id, true);
                }
                return Self::Slider {
                    owner: id,
                    grip,
                    initial,
                    transform,
                    applied: false,
                    touch_id: None,
                };
            }
            if let Some(bar) = runtime.store().scroll_bar_of_part(id) {
                // Arrows and track clicks never capture the grip or scroll content.
                if object.name() == fairygui_core::scroll::GRIP_NAME
                    && let Some(container) = runtime.store().scroll_container_of_bar(bar)
                    && let Some(vertical) = runtime.store().scroll_bar_axis(bar)
                    && let Some(track) = runtime.store().scroll_grip_track(bar)
                    && track > 0.0
                    && let Some(pane) = runtime.store().scroll_pane(container)
                {
                    if occupied.contains(&container) {
                        return Self::None;
                    }
                    let initial = if vertical {
                        pane.display_perc_y()
                    } else {
                        pane.display_perc_x()
                    };
                    let transform = pointer_transform(runtime, bar, transforms);
                    runtime.store_mut().set_scroll_bar_grip_dragging(bar, true);
                    return Self::Grip {
                        bar,
                        container,
                        vertical,
                        track,
                        initial,
                        transform,
                    };
                }
                // A nested graphic inside the grip can still resolve its parent.
                if matches!(
                    object.name(),
                    fairygui_core::scroll::ARROW1_NAME
                        | fairygui_core::scroll::ARROW2_NAME
                        | fairygui_core::scroll::BAR_NAME
                ) {
                    return Self::None;
                }
            }
            current = object.parent();
        }
        if runtime.store().scroll_bar_of_part(target).is_some() {
            return Self::None;
        }
        let mut current = Some(target);
        while let Some(owner) = current {
            // A configured proxy drag reserves the pointer before scrolling can
            // consume the threshold displacement on the same input frame.
            if runtime.drag_source_bindings.contains(&owner) {
                return Self::None;
            }
            if runtime.object_drag_eligible(owner) {
                if occupied.contains(&owner) {
                    return Self::None;
                }
                if let Some(parent) = runtime
                    .store()
                    .object(owner)
                    .and_then(|object| object.parent())
                {
                    return Self::Object {
                        owner,
                        parent,
                        motion: None,
                        touch_id: None,
                    };
                }
                return Self::None;
            }
            current = runtime
                .store()
                .object(owner)
                .and_then(|object| object.parent());
        }
        if let Some(index) = (0..runtime.window_count()).find(|i| {
            runtime.window(*i).is_some_and(|w| {
                w.is_showing() && w.is_draggable() && w.is_in_drag_area(runtime.store(), target)
            })
        }) {
            let owner = runtime.window(index).unwrap().owner();
            if occupied.contains(&owner) {
                return Self::None;
            }
            if let Some(object) = runtime.store().object(owner) {
                let origin = Vec2::new(object.x(), object.y());
                let transform = object.parent().map_or_else(
                    || PointerTransform::from_model_scale(Vec2::ONE),
                    |p| pointer_transform(runtime, p, transforms),
                );
                runtime.bring_window_to_front(index);
                return Self::Window {
                    owner,
                    origin,
                    transform,
                };
            }
        }
        let mut candidates = Vec::new();
        let mut current = Some(target);
        while let Some(id) = current {
            let Some(object) = runtime.store().object(id) else {
                break;
            };
            if runtime
                .store()
                .scroll_pane(id)
                .is_some_and(|pane| pane.touch_effect)
                && !occupied.contains(&id)
            {
                candidates.push((id, pointer_transform(runtime, id, transforms)));
            }
            current = object.parent();
        }
        if let Some(&(container, transform)) = candidates.first() {
            return Self::Scroll {
                container,
                started: false,
                transform,
                elapsed: 0.,
                candidates: Some(candidates),
            };
        }
        Self::None
    }

    /// Hold all eligible ancestor panes until a local axis exceeds sensitivity.
    /// Opposing nested axes use dominance; ties keep the nearest pane.
    pub(super) fn select_scroll(
        &mut self,
        runtime: &GuiRuntime,
        displacement: Vec2,
        sensitivity: f32,
        occupied: &[GObjectId],
    ) {
        let Self::Scroll {
            container,
            transform,
            candidates: Some(candidates),
            ..
        } = self
        else {
            return;
        };
        let available = |id: GObjectId| {
            !occupied.contains(&id)
                && eligible(runtime, id)
                && runtime
                    .store()
                    .scroll_pane(id)
                    .is_some_and(|pane| pane.touch_effect)
        };
        let horizontal = candidates.iter().any(|(id, _)| {
            available(*id)
                && runtime
                    .store()
                    .scroll_pane(*id)
                    .unwrap()
                    .scroll_type
                    .allows_horizontal()
        });
        let vertical = candidates.iter().any(|(id, _)| {
            available(*id)
                && runtime
                    .store()
                    .scroll_pane(*id)
                    .unwrap()
                    .scroll_type
                    .allows_vertical()
        });
        let selected = candidates.iter().copied().find(|(id, transform)| {
            if !available(*id) {
                return false;
            }
            let pane = runtime.store().scroll_pane(*id).unwrap();
            let delta = transform.displacement(displacement).abs();
            match pane.scroll_type {
                fairygui_core::scroll::ScrollType::Horizontal => {
                    delta.x >= sensitivity && (!vertical || delta.x >= delta.y)
                }
                fairygui_core::scroll::ScrollType::Vertical => {
                    delta.y >= sensitivity && (!horizontal || delta.y >= delta.x)
                }
                fairygui_core::scroll::ScrollType::Both => delta.max_element() >= sensitivity,
            }
        });
        if let Some((selected, selected_transform)) = selected {
            *container = selected;
            *transform = selected_transform;
            if let Self::Scroll { candidates, .. } = self {
                *candidates = None;
            }
        }
    }

    pub(super) fn cancel_scroll(&mut self, target: GObjectId) {
        if let Self::Scroll {
            container,
            candidates,
            ..
        } = self
        {
            if let Some(candidates) = candidates {
                candidates.retain(|(id, _)| *id != target);
                if !candidates.is_empty() {
                    return;
                }
            } else if *container != target {
                return;
            }
            *self = Self::None;
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn update(
        &mut self,
        runtime: &mut GuiRuntime,
        cursor: Vec2,
        last: Vec2,
        start: Vec2,
        moved: bool,
        dt: f32,
        positions: &[(GObjectId, Vec2)],
    ) {
        match self {
            Self::Object {
                owner,
                parent: _,
                motion,
                touch_id,
            } if moved => {
                if !runtime.object_drag_eligible(*owner) {
                    return;
                }
                let newly_started = motion.is_none();
                if newly_started {
                    *motion = fairygui_core::drag_drop::ObjectDrag::begin(
                        runtime.store(),
                        *owner,
                        Point::new(cursor.x, cursor.y),
                    );
                    if motion.is_none() {
                        return;
                    }
                    let active = crate::runtime::object_drag::Active {
                        touch_id: *touch_id,
                        position: cursor,
                    };
                    runtime.active_object_drags.insert(*owner, active);
                    runtime.object_drag_event(*owner, EventKind::DragStart, active);
                }
                if newly_started || cursor != last {
                    let bounds = runtime
                        .object_drag_bindings
                        .get(owner)
                        .and_then(|binding| binding.bounds);
                    let stage = runtime.root_container();
                    if motion.as_ref().unwrap().update(
                        runtime.store_mut(),
                        Point::new(cursor.x, cursor.y),
                        bounds,
                        stage,
                    ) {
                        let active = crate::runtime::object_drag::Active {
                            touch_id: *touch_id,
                            position: cursor,
                        };
                        runtime.active_object_drags.insert(*owner, active);
                        runtime.object_drag_event(*owner, EventKind::DragMove, active);
                    }
                }
            }
            Self::Slider {
                owner,
                grip,
                initial,
                transform,
                applied,
                touch_id,
            } => {
                let changed = if *grip {
                    // Native sliders update on pointer movement, not on press
                    // or idle frames (which would round/clamp raw setter values).
                    if cursor == last {
                        return;
                    }
                    let Some(ControlData::Slider(data)) = runtime
                        .store()
                        .object(*owner)
                        .map(|object| object.control())
                    else {
                        return;
                    };
                    if !data.can_drag {
                        return;
                    }
                    let reverse = data.reverse;
                    let Some((vertical, track)) = runtime
                        .store()
                        .slider_track(*owner)
                        .filter(|(_, track)| *track > 0.)
                    else {
                        return;
                    };
                    let delta = transform.displacement(cursor - start);
                    let delta =
                        f64::from(if vertical { delta.y } else { delta.x }) / f64::from(track);
                    runtime.store_mut().set_slider_percent_from_input(
                        *owner,
                        *initial + if reverse { -delta } else { delta },
                    )
                } else if !*applied {
                    *applied = true;
                    let Some(object) = runtime.store().object(*owner) else {
                        return;
                    };
                    let point = positions
                        .iter()
                        .find(|(id, _)| id == owner)
                        .map(|(_, n)| Point::new(n.x * object.width(), n.y * object.height()))
                        .or_else(|| {
                            runtime
                                .store()
                                .global_to_local(*owner, Point::new(cursor.x, cursor.y))
                        });
                    point.is_some_and(|point| runtime.store_mut().click_slider_at(*owner, point))
                } else {
                    false
                };
                if changed {
                    let mut event = UiEvent::new(EventKind::StateChanged, *owner)
                        .with_pointer(Point::new(cursor.x, cursor.y));
                    event.touch_id = *touch_id;
                    runtime.dispatch_event(event);
                }
            }
            Self::Grip {
                bar: _,
                container,
                vertical,
                track,
                initial,
                transform,
            } if moved => {
                let delta = transform.displacement(cursor - start);
                runtime.store_mut().set_scroll_perc(
                    *container,
                    *vertical,
                    *initial + (if *vertical { delta.y } else { delta.x }) / *track,
                );
            }
            Self::Window {
                owner,
                origin,
                transform,
            } if moved => {
                if let Some(index) = runtime.window_index(*owner)
                    && let Some(window) = runtime.window_mut(index)
                    && window.is_showing()
                    && window.is_draggable()
                {
                    window.set_dragging(true);
                    let owner = window.owner();
                    let at = *origin + transform.displacement(cursor - start);
                    runtime.store_mut().set_xy(owner, at.x, at.y);
                }
            }
            Self::Scroll {
                container,
                started,
                transform,
                elapsed,
                candidates,
            } => {
                *elapsed += dt;
                if !moved || candidates.is_some() {
                    return;
                }
                let delta = transform.displacement(cursor - if *started { last } else { start });
                if !*started {
                    runtime.store_mut().begin_scroll_drag(*container);
                    *started = true;
                }
                runtime.store_mut().drag_scroll_by_scaled(
                    *container,
                    delta.x,
                    delta.y,
                    *elapsed,
                    Point::new(
                        transform.local_to_screen_scale.x,
                        transform.local_to_screen_scale.y,
                    ),
                );
                *elapsed = 0.;
            }
            _ => {}
        }
    }

    pub(super) fn finish(&self, runtime: &mut GuiRuntime, dt: f32) {
        match self {
            Self::Object { owner, .. } => runtime.end_object_drag(*owner),
            Self::Grip { bar, .. } => {
                runtime
                    .store_mut()
                    .set_scroll_bar_grip_dragging(*bar, false);
            }
            Self::Scroll {
                container,
                started: true,
                ..
            } => {
                runtime.store_mut().end_scroll_drag_after(*container, dt);
            }
            Self::Window { owner, .. } => {
                if let Some(index) = runtime.window_index(*owner)
                    && let Some(w) = runtime.window_mut(index)
                {
                    w.end_drag();
                }
            }
            _ => {}
        }
    }

    pub(super) fn valid(&self, runtime: &GuiRuntime) -> bool {
        match self {
            Self::Object {
                owner,
                parent,
                motion,
                ..
            } => {
                runtime.object_drag_eligible(*owner)
                    && runtime
                        .store()
                        .object(*owner)
                        .is_some_and(|object| object.parent() == Some(*parent))
                    && (motion.is_none() || runtime.active_object_drags.contains_key(owner))
            }
            Self::Window { owner, .. } => runtime
                .window_index(*owner)
                .and_then(|index| runtime.window(index))
                .is_some_and(|window| window.is_showing() && window.is_draggable()),
            Self::Grip {
                bar,
                container,
                vertical,
                ..
            } => {
                runtime.store().scroll_container_of_bar(*bar) == Some(*container)
                    && runtime.store().scroll_bar_axis(*bar) == Some(*vertical)
            }
            Self::Scroll {
                container,
                candidates: None,
                ..
            } => {
                eligible(runtime, *container)
                    && runtime
                        .store()
                        .scroll_pane(*container)
                        .is_some_and(|pane| pane.touch_effect)
            }
            _ => true,
        }
    }

    pub(super) fn set_touch_id(&mut self, id: u64) {
        if let Self::Object { touch_id, .. } | Self::Slider { touch_id, .. } = self {
            *touch_id = Some(id);
        }
    }
}

fn pointer_transform(
    runtime: &GuiRuntime,
    target: GObjectId,
    transforms: &[(GObjectId, PointerTransform)],
) -> PointerTransform {
    transforms
        .iter()
        .find_map(|(object, transform)| (*object == target).then_some(*transform))
        .unwrap_or_else(|| PointerTransform::from_model_scale(world_scale(runtime, target)))
}

fn world_scale(runtime: &GuiRuntime, target: GObjectId) -> Vec2 {
    let mut scale = Vec2::ONE;
    let mut current = Some(target);
    while let Some(id) = current {
        let Some(object) = runtime.store().object(id) else {
            break;
        };
        scale *= Vec2::new(object.scale_x(), object.scale_y());
        current = object.parent();
    }
    // Degenerate transforms cannot provide usable pointer deltas.
    if scale.x.abs() < f32::EPSILON {
        scale.x = 1.0;
    }
    if scale.y.abs() < f32::EPSILON {
        scale.y = 1.0;
    }
    scale
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::Affine2;

    #[test]
    fn pointer_transform_converts_rotation_skew_reflection_and_dpi() {
        let cases = [
            Mat2::from_angle(0.61) * Mat2::from_diagonal(Vec2::new(1.4, 0.7)),
            Mat2::from_cols(Vec2::new(1.2, 0.25), Vec2::new(-0.4, 0.9)),
            Mat2::from_cols(Vec2::new(-1.1, 0.2), Vec2::new(0.35, 1.6)),
        ];
        let local = Vec2::new(13.0, -7.5);
        for dpi in [1.0, 1.75, 2.0] {
            for matrix in cases {
                let physical = matrix;
                let transform = UiGlobalTransform::from(Affine2 {
                    matrix2: physical,
                    translation: Vec2::new(120.0, 85.0) * dpi,
                });
                let node = ComputedNode {
                    inverse_scale_factor: 1.0 / dpi,
                    ..default()
                };
                let pointer = PointerTransform::from_view(&transform, &node, dpi).unwrap();
                let screen_delta = physical * local;
                assert!(
                    pointer
                        .displacement(screen_delta)
                        .abs_diff_eq(local, 0.0001)
                );
                let expected_scale = Vec2::new(physical.x_axis.length(), physical.y_axis.length());
                assert!(
                    pointer
                        .local_to_screen_scale
                        .abs_diff_eq(expected_scale, 0.0001)
                );
            }
        }
    }

    #[test]
    fn pointer_transform_rejects_degenerate_and_nonfinite_views() {
        let node = ComputedNode::default();
        for matrix in [
            Mat2::from_diagonal(Vec2::new(1.0, 0.0)),
            Mat2::from_diagonal(Vec2::new(f32::NAN, 1.0)),
        ] {
            let transform = UiGlobalTransform::from(Affine2 {
                matrix2: matrix,
                translation: Vec2::ZERO,
            });
            assert!(PointerTransform::from_view(&transform, &node, 1.0).is_none());
        }
    }
}

#[cfg(test)]
mod window_tests {
    use super::*;
    use bevy::ecs::system::SystemState;
    #[test]
    fn window_drag_keeps_its_object_after_other_windows_are_disposed() {
        let mut runtime = GuiRuntime::default();
        let package = fairygui_core::package::FuiPackage::parse(
            "Basics",
            include_bytes!("../../../fairygui-demo/assets/ui/Basics.fui"),
        )
        .unwrap();
        runtime.register_package(Handle::default(), &package);
        let size = fairygui_core::Size::new(1280., 720.);
        let first = runtime
            .show_window(&package, "WindowA", false, size)
            .unwrap();
        let second = runtime
            .show_window(&package, "WindowA", false, size)
            .unwrap();
        let owner = runtime.window(second).unwrap().owner();
        let frame = runtime.window(second).unwrap().drag_area().unwrap();
        let mut gesture = Gesture::begin_excluding(&mut runtime, frame, &[], &[]);
        assert_eq!(gesture.owner(&runtime), Some(owner));
        let initial = runtime.store().object(owner).unwrap().position();
        let mut world = World::new();
        let mut state = SystemState::<Commands>::new(&mut world);
        runtime.close_window(&mut state.get_mut(&mut world).unwrap(), first);
        state.apply(&mut world);
        gesture.update(
            &mut runtime,
            Vec2::new(60., 25.),
            Vec2::ZERO,
            Vec2::ZERO,
            true,
            0.1,
            &[],
        );
        assert_eq!(
            runtime.store().object(owner).unwrap().position(),
            Point::new(initial.x + 60., initial.y + 25.)
        );
        let index = runtime.window_index(owner).unwrap();
        assert!(runtime.window(index).unwrap().is_dragging());
        gesture.finish(&mut runtime, 0.1);
        assert!(!runtime.window(index).unwrap().is_dragging());
        runtime.set_window_draggable(index, false);
        gesture.update(
            &mut runtime,
            Vec2::new(120., 50.),
            Vec2::ZERO,
            Vec2::ZERO,
            true,
            0.1,
            &[],
        );
        assert_eq!(
            runtime.store().object(owner).unwrap().position(),
            Point::new(initial.x + 60., initial.y + 25.)
        );
    }
}

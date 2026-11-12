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
    Slider(GObjectId),
    Grip {
        bar: GObjectId,
        container: GObjectId,
        vertical: bool,
        track: f32,
        initial: f32,
        transform: PointerTransform,
    },
    Window {
        index: usize,
        origin: Vec2,
        transform: PointerTransform,
    },
    Scroll {
        container: GObjectId,
        started: bool,
        transform: PointerTransform,
        elapsed: f32,
    },
}

impl Gesture {
    pub(super) fn owner(&self, runtime: &GuiRuntime) -> Option<GObjectId> {
        match self {
            Self::None => None,
            Self::Slider(id) => Some(*id),
            Self::Grip { container, .. } | Self::Scroll { container, .. } => Some(*container),
            Self::Window { index, .. } => runtime.window(*index).map(|window| window.owner()),
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
                return Self::Slider(id);
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
                    index,
                    origin,
                    transform,
                };
            }
        }
        if let Some(container) = scrollable_ancestor(runtime, target) {
            if occupied.contains(&container) {
                return Self::None;
            }
            return Self::Scroll {
                container,
                started: false,
                transform: pointer_transform(runtime, container, transforms),
                elapsed: 0.,
            };
        }
        Self::None
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
            Self::Slider(id) => {
                if let Some((_, n)) = positions.iter().find(|(object, _)| object == id)
                    && let Some(object) = runtime.store().object(*id)
                {
                    let point = Point::new(n.x * object.width(), n.y * object.height());
                    runtime.store_mut().set_slider_value_at(*id, point);
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
                index,
                origin,
                transform,
            } if moved => {
                if let Some(window) = runtime.window_mut(*index) {
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
            } => {
                *elapsed += dt;
                if !moved {
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
            Self::Window { index, .. } => {
                if let Some(w) = runtime.window_mut(*index) {
                    w.end_drag();
                }
            }
            _ => {}
        }
    }

    pub(super) fn valid(&self, runtime: &GuiRuntime) -> bool {
        match self {
            Self::Grip {
                bar,
                container,
                vertical,
                ..
            } => {
                runtime.store().scroll_container_of_bar(*bar) == Some(*container)
                    && runtime.store().scroll_bar_axis(*bar) == Some(*vertical)
            }
            _ => true,
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

pub(super) fn scrollable_ancestor(runtime: &GuiRuntime, target: GObjectId) -> Option<GObjectId> {
    let mut current = Some(target);
    while let Some(id) = current {
        let object = runtime.store().object(id)?;
        if runtime.store().scroll_pane(id).is_some() {
            return Some(id);
        }
        current = object.parent();
    }
    None
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

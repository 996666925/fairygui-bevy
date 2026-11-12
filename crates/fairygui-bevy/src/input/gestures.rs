//! Exclusive built-in gesture ownership and lifecycle.
use super::*;
use fairygui_core::widget::ControlData;

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
        scale: Vec2,
    },
    Window {
        index: usize,
        origin: Vec2,
        scale: Vec2,
    },
    Scroll {
        container: GObjectId,
        started: bool,
        scale: Vec2,
        elapsed: f32,
    },
}

impl Gesture {
    pub(super) fn begin(runtime: &mut GuiRuntime, target: GObjectId) -> Self {
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
                    let initial = if vertical {
                        pane.display_perc_y()
                    } else {
                        pane.display_perc_x()
                    };
                    let scale = world_scale(runtime, bar);
                    runtime.store_mut().set_scroll_bar_grip_dragging(bar, true);
                    return Self::Grip {
                        bar,
                        container,
                        vertical,
                        track,
                        initial,
                        scale,
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
            if let Some(object) = runtime.store().object(owner) {
                let origin = Vec2::new(object.x(), object.y());
                let scale = object
                    .parent()
                    .map_or(Vec2::ONE, |p| world_scale(runtime, p));
                runtime.bring_window_to_front(index);
                return Self::Window {
                    index,
                    origin,
                    scale,
                };
            }
        }
        if let Some(container) = scrollable_ancestor(runtime, target) {
            return Self::Scroll {
                container,
                started: false,
                scale: world_scale(runtime, container),
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
                scale,
            } if moved => {
                let delta = (cursor - start) / *scale;
                runtime.store_mut().set_scroll_perc(
                    *container,
                    *vertical,
                    *initial + (if *vertical { delta.y } else { delta.x }) / *track,
                );
            }
            Self::Window {
                index,
                origin,
                scale,
            } if moved => {
                if let Some(window) = runtime.window_mut(*index) {
                    window.set_dragging(true);
                    let owner = window.owner();
                    let at = *origin + (cursor - start) / *scale;
                    runtime.store_mut().set_xy(owner, at.x, at.y);
                }
            }
            Self::Scroll {
                container,
                started,
                scale,
                elapsed,
            } => {
                *elapsed += dt;
                if !moved {
                    return;
                }
                let delta = (cursor - if *started { last } else { start }) / *scale;
                if !*started {
                    runtime.store_mut().begin_scroll_drag(*container);
                    *started = true;
                }
                runtime.store_mut().drag_scroll_by_scaled(
                    *container,
                    delta.x,
                    delta.y,
                    *elapsed,
                    Point::new(scale.x, scale.y),
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

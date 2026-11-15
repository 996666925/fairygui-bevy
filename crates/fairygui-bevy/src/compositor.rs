//! Native Sprite blend inheritance and isolated color-filter subtrees.
use crate::{
    GObjectRef, GuiRuntime,
    runtime::{InlineImageOwner, appearance::TextTintSource},
};
use bevy::prelude::*;
use fairygui_core::{GObjectId, color_matrix::ColorMatrix, object::BlendMode};
use std::collections::HashMap;

#[path = "compositor_gpu.rs"]
mod gpu;

#[cfg(test)]
#[path = "compositor_tests.rs"]
mod tests;

pub(crate) fn effect_matrix(object: &fairygui_core::GObject) -> Option<ColorMatrix> {
    if object.control().component().is_some() {
        // GComponent gray is handled by its grayed controller/child propagation,
        // independently of an explicit filter on the component's Sprite.
        object.color_filter().map(|filter| filter.to_matrix())
    } else if object.grayed() {
        Some(ColorMatrix::LAYA_GRAY)
    } else {
        object.color_filter().map(|filter| filter.to_matrix())
    }
}

fn isolated(object: &fairygui_core::GObject) -> bool {
    effect_matrix(object).is_some()
        || object.cache_as_bitmap()
        || object
            .control()
            .component()
            .is_some_and(|c| object.mask().is_some() && !c.mask_reversed)
}

/// Component-local rectangle and left/top/right/bottom fade widths. The render
/// extractor resolves physical screen coordinates from the owner's UI transform.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct SoftClip {
    pub rect: Rect,
    pub edges: Vec4,
    pub from_screen: [Vec4; 2],
}

fn soft_clip(object: &fairygui_core::GObject) -> Option<SoftClip> {
    use fairygui_core::widget::OverflowType;
    let component = object.control().component()?;
    let softness = component.clip_softness;
    if softness.x <= 0. && softness.y <= 0. {
        return None;
    }
    let margin = component.margin;
    let (min, size, edges) = if let Some(pane) = component.scroll_pane.as_ref() {
        if !pane.clip {
            return None;
        }
        let position = pane.display_pos();
        let max = pane.max_scroll_pos();
        let edges = Vec4::new(
            if pane.softness_on_top_left && position.x > 0. {
                softness.x
            } else {
                0.
            },
            if pane.softness_on_top_left && position.y > 0. {
                softness.y
            } else {
                0.
            },
            if position.x < max.x { softness.x } else { 0. },
            if position.y < max.y { softness.y } else { 0. },
        );
        let view = pane.view_size();
        if pane.dont_clip_margin {
            (
                Vec2::ZERO,
                Vec2::new(
                    view.width + margin.left + margin.right,
                    view.height + margin.top + margin.bottom,
                ),
                edges,
            )
        } else {
            (
                Vec2::new(margin.left, margin.top),
                Vec2::new(view.width, view.height),
                edges,
            )
        }
    } else if component.overflow == OverflowType::Hidden {
        (
            Vec2::new(margin.left, margin.top),
            Vec2::new(
                object.width() - margin.left - margin.right,
                object.height() - margin.top - margin.bottom,
            ),
            Vec4::new(softness.x, softness.y, softness.x, softness.y),
        )
    } else {
        return None;
    };
    Some(SoftClip {
        rect: Rect::from_corners(min, min + size.max(Vec2::ZERO)),
        edges,
        from_screen: [Vec4::ZERO; 2],
    })
}

impl GuiRuntime {
    pub(crate) fn has_composited_filter(&self, id: GObjectId) -> bool {
        if !self.compositing {
            return false;
        }
        let mut current = Some(id);
        while let Some(id) = current {
            let Some(object) = self.store().object(id) else {
                break;
            };
            if effect_matrix(object).is_some() {
                return true;
            }
            current = object.parent();
        }
        false
    }
    pub(crate) fn paint_matrix(&self, object: &fairygui_core::GObject) -> Option<ColorMatrix> {
        if self.compositing {
            None
        } else {
            crate::runtime::appearance::local_color_matrix(object)
        }
    }
    pub(crate) fn paint_alpha(&self, id: GObjectId) -> f32 {
        if !self.compositing {
            return self.store().effective_alpha(id);
        }
        let mut current = Some(id);
        let mut alpha = 1.;
        while let Some(id) = current {
            let Some(object) = self.store().object(id) else {
                break;
            };
            if isolated(object) {
                break;
            }
            alpha *= object.alpha();
            current = object.parent();
        }
        alpha
    }
    pub(crate) fn paint_tint(
        &self,
        object: &fairygui_core::GObject,
        color: Option<fairygui_core::color::Color>,
    ) -> Color {
        let mut color = crate::runtime::appearance::base_color(object, color);
        if let Some(matrix) = self.paint_matrix(object) {
            color = matrix.apply(color);
        }
        crate::backend::to_bevy_color(color)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Group {
    pub id: GObjectId,
    pub matrix: ColorMatrix,
    pub opacity: f32,
    pub blend: BlendMode,
    pub mask: Option<GObjectId>,
    pub clip: Option<SoftClip>,
}
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub(super) struct PaintPolicy {
    pub groups: Vec<Group>,
    pub blend: BlendMode,
    pub mask_for: Vec<GObjectId>,
}

fn policy(gui: &GuiRuntime, id: GObjectId) -> PaintPolicy {
    let mut chain = Vec::new();
    let mut current = Some(id);
    while let Some(id) = current {
        let Some(object) = gui.store().object(id) else {
            break;
        };
        chain.push((id, object));
        current = object.parent();
    }
    let mut result = PaintPolicy::default();
    let mut clips = Vec::new();
    // Mask coverage is combined with already faded content, so fading the
    // mask source as well would apply the viewport gradient twice.
    let mask_source = chain
        .windows(2)
        .any(|pair| pair[1].1.mask() == Some(pair[0].0));
    let mut alpha = 1.;
    chain.reverse();
    for (index, &(id, object)) in chain.iter().enumerate() {
        alpha *= object.alpha();
        if object.blend_mode_is_set() {
            result.blend = object.blend_mode();
        }
        if let Some(owner) = object.mask_parent() {
            result.mask_for.push(owner);
        }
        if isolated(object) {
            result.groups.push(Group {
                id,
                matrix: effect_matrix(object).unwrap_or_default(),
                opacity: alpha,
                blend: if object.mask_parent().is_some() {
                    BlendMode::Normal
                } else {
                    result.blend
                },
                mask: object
                    .control()
                    .component()
                    .and_then(|c| object.mask().filter(|_| !c.mask_reversed)),
                clip: None,
            });
            alpha = 1.;
            result.blend = BlendMode::Normal;
        }
        // Keep the owner's own graphics and registered scrollbar/refresh
        // subtrees outside the content fade.
        if !mask_source
            && index + 1 < chain.len()
            && !gui.store().is_scroll_overlay(chain[index + 1].0)
            && object.mask() != Some(chain[index + 1].0)
            && let Some(mut clip) = soft_clip(object)
        {
            if let Some(pane) = object
                .control()
                .component()
                .and_then(|c| c.scroll_pane.as_ref())
                && pane.display_on_left
                && !pane.floating
                && let Some(bar) = pane.vt_bar.and_then(|bar| gui.store().object(bar))
            {
                clip.rect.min.x += bar.width();
                clip.rect.max.x += bar.width();
            }
            clips.push(Group {
                id,
                matrix: ColorMatrix::IDENTITY,
                opacity: 1.,
                blend: BlendMode::Normal,
                mask: None,
                clip: Some(clip),
            });
        }
    }
    // Apply clipping per draw, preserving inherited blend factors and parent
    // opacity. Combining siblings first would change translucent overlaps.
    for (index, mut clip) in clips.into_iter().enumerate() {
        if index == 0 {
            clip.blend = result.blend;
        }
        result.groups.push(clip);
    }
    if result.groups.iter().any(|group| group.clip.is_some()) {
        result.blend = BlendMode::Normal;
    }
    result
}

#[allow(clippy::type_complexity)]
fn sync_policies(
    mut commands: Commands,
    gui: NonSend<GuiRuntime>,
    nodes: Query<
        (
            Entity,
            Option<&GObjectRef>,
            Option<&TextTintSource>,
            Option<&InlineImageOwner>,
            Option<&ChildOf>,
            Option<&PaintPolicy>,
        ),
        With<Node>,
    >,
) {
    let mut owners = HashMap::new();
    for (entity, object, source, _, _, _) in &nodes {
        if let Some(owner) = object.map(|o| o.0).or_else(|| source.map(|s| s.owner)) {
            owners.insert(entity, owner);
        }
    }
    let mut policies = HashMap::new();
    for (entity, _, _, inline, parent, old) in &nodes {
        let mut current = Some(inline.map_or(entity, |i| i.0));
        let mut owner = None;
        while let Some(id) = current {
            if let Some(found) = owners.get(&id) {
                owner = Some(*found);
                break;
            }
            current = nodes
                .get(id)
                .ok()
                .and_then(|(_, _, _, _, p, _)| p.map(ChildOf::parent));
            if id == entity && current.is_none() {
                current = parent.map(ChildOf::parent);
            }
        }
        let next = owner
            .map(|id| policies.entry(id).or_insert_with(|| policy(&gui, id)))
            .cloned()
            .unwrap_or_default();
        if old != Some(&next) {
            commands.entity(entity).insert(next);
        }
    }
}

pub(crate) fn install(app: &mut App) {
    if app.get_sub_app(bevy::render::RenderApp).is_none() {
        return;
    }
    app.world_mut().non_send_mut::<GuiRuntime>().compositing = true;
    app.add_systems(
        PostUpdate,
        sync_policies
            .after(bevy::ui::UiSystems::Layout)
            .after(crate::runtime::project_scroll_overflow),
    );
    gpu::install(app);
}

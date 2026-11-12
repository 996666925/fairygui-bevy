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
    let mut alpha = 1.;
    for (id, object) in chain.into_iter().rev() {
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
            });
            alpha = 1.;
            result.blend = BlendMode::Normal;
        }
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

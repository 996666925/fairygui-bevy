//! Keep native Sprite and mask-container clips in Bevy's shared paint/hit stack.
use crate::{GObjectRef, GuiRuntime};
use bevy::{
    prelude::*,
    ui::{CalculatedClip, FixedNode, OverrideClip, experimental::UiChildren},
};
use std::collections::HashMap;

type Geometry<'a> = (
    Entity,
    &'a Node,
    &'a ComputedNode,
    &'a UiGlobalTransform,
    Option<&'a GObjectRef>,
    Option<&'a CalculatedClip>,
    Has<OverrideClip>,
    Has<FixedNode>,
);

/// Rebuild only owned GUI subtrees after Bevy publishes inherited clips. A GUI
/// root keeps its external Bevy ancestors' clip; application UI descendants keep
/// ordinary overflow, FixedNode and OverrideClip behavior.
pub(crate) fn project(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    mut queries: ParamSet<(Query<Geometry>, Query<&mut CalculatedClip>)>,
    children: UiChildren,
) {
    let updates = {
        let geometry = queries.p0();
        let owners: HashMap<_, _> = geometry
            .iter()
            .filter_map(|(e, _, _, _, reference, _, _, _)| {
                let id = reference?.0;
                (runtime.entity_of(id) == Some(e)).then_some((e, id))
            })
            .collect();
        let mut updates = Vec::new();
        for &root in owners.keys() {
            let mut parent = children.get_parent(root);
            let mut nested = false;
            while let Some(p) = parent {
                if owners.contains_key(&p) {
                    nested = true;
                    break;
                }
                parent = children.get_parent(p);
            }
            if nested {
                continue;
            }
            let Ok((_, _, _, _, _, inherited, _, _)) = geometry.get(root) else {
                continue;
            };
            let mut pending = vec![(root, inherited.cloned())];
            while let Some((entity, mut clip)) = pending.pop() {
                let Ok((_, node, computed, transform, _, _, override_clip, fixed)) =
                    geometry.get(entity)
                else {
                    continue;
                };
                if override_clip || fixed {
                    clip = None;
                }
                if node.display == Display::None {
                    clip = Some(CalculatedClip::FullyClipped);
                }
                let native = owners
                    .get(&entity)
                    .and_then(|id| runtime.store().object(*id))
                    .and_then(|o| o.control().component());
                let rect = owners
                    .get(&entity)
                    .and_then(|id| runtime.store().component_clip_rect(*id));
                let pane = native.is_some_and(|c| c.scroll_pane.is_some());
                if native.is_some()
                    && !pane
                    && let Some(rect) = rect
                {
                    clip = Some(append(clip, rect, computed, transform));
                }
                updates.push((entity, clip.clone()));
                // Native components own their clip scope. Node.overflow continues
                // to drive Bevy's scroll layout, but must not clip their scrollbars.
                let inherited = if native.is_none() && !node.overflow.is_visible() {
                    Some(clip.unwrap_or_default().with_rect(
                        computed.resolve_clip_rect(node.overflow, node.overflow_clip_margin),
                        transform,
                    ))
                } else {
                    clip
                };
                for child in children.iter_ui_children(entity) {
                    let mut clip = inherited.clone();
                    if pane
                        && let (Some(&owner), Some(&id), Some(rect)) =
                            (owners.get(&entity), owners.get(&child), rect)
                        && runtime
                            .store()
                            .object(id)
                            .is_some_and(|o| o.parent() == Some(owner))
                        && !runtime.store().is_scroll_bar_overlay(id)
                    {
                        clip = Some(append(clip, rect, computed, transform));
                    }
                    pending.push((child, clip));
                }
            }
        }
        updates
    };
    let mut clips = queries.p1();
    for (entity, clip) in updates {
        match (clips.get_mut(entity).ok(), clip) {
            (Some(mut old), Some(new)) => {
                if *old != new {
                    *old = new;
                }
            }
            (None, Some(new)) => {
                commands.entity(entity).try_insert(new);
            }
            (Some(_), None) => {
                commands.entity(entity).remove::<CalculatedClip>();
            }
            (None, None) => (),
        }
    }
}

fn append(
    inherited: Option<CalculatedClip>,
    rect: crate::core::Rect,
    node: &ComputedNode,
    transform: &UiGlobalTransform,
) -> CalculatedClip {
    if rect.width <= 0. || rect.height <= 0. {
        return CalculatedClip::FullyClipped;
    }
    let scale = node.inverse_scale_factor().recip();
    let min = Vec2::new(rect.x, rect.y) * scale - node.size() * 0.5;
    let max = min + Vec2::new(rect.width, rect.height) * scale;
    if !min.is_finite() || !max.is_finite() {
        return CalculatedClip::FullyClipped;
    }
    inherited
        .unwrap_or_default()
        .with_rect(Rect::from_corners(min, max), transform)
}

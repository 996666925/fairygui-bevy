//! Bevy UiTransform has one rotation; FairyGUI gives its axes separate angles.
use crate::{GObjectRef, GuiRuntime, backend};
use bevy::{
    math::{Affine2, Mat2},
    prelude::*,
    ui::{
        ComputedUiRenderTargetInfo, FixedNode, IgnoreScroll, LayoutConfig,
        experimental::UiChildren, ui_surface::UiSurface,
    },
};
use std::collections::HashMap;

type Geometry<'a> = (
    Entity,
    &'a ComputedNode,
    &'a UiGlobalTransform,
    &'a UiTransform,
    &'a ComputedUiRenderTargetInfo,
    Option<&'a GObjectRef>,
    Option<&'a LayoutConfig>,
    Option<&'a IgnoreScroll>,
    Has<FixedNode>,
);

/// Replace the skewed local axes after layout and propagate them through paint
/// and application children before scrolling, text placement, clipping and hits.
/// Use actual Taffy positions instead of inverse matrices: zero scales and
/// degenerate skews remain finite and can be restored on the next frame.
pub(crate) fn project(
    runtime: NonSend<GuiRuntime>,
    surface: Option<ResMut<UiSurface>>,
    children: UiChildren,
    mut queries: ParamSet<(Query<Geometry>, Query<&mut UiGlobalTransform>)>,
) {
    // ECS-only applications can use the runtime without Bevy's UI layout plugin.
    let Some(mut surface) = surface else {
        return;
    };
    let updates = {
        let geometry = queries.p0();
        let skewed: HashMap<_, _> = geometry
            .iter()
            .filter_map(|(entity, _, _, _, _, reference, _, _, _)| {
                let id = reference?.0;
                let object = runtime.store().object(id)?;
                (runtime.entity_of(id) == Some(entity)
                    && (object.skew_x() != 0. || object.skew_y() != 0.))
                    .then_some((entity, id))
            })
            .collect();
        let mut updates = Vec::new();
        for (&root, &id) in &skewed {
            let Ok((_, node, global, transform, target, _, config, _, fixed)) = geometry.get(root)
            else {
                continue;
            };
            let mut parent = if fixed {
                None
            } else {
                children.get_parent(root)
            };
            let mut nested = false;
            let mut inherited_rounding = None;
            while let Some(entity) = parent {
                if skewed.contains_key(&entity) {
                    nested = true;
                    break;
                }
                if let Ok((_, _, _, _, _, _, config, _, fixed)) = geometry.get(entity) {
                    if inherited_rounding.is_none() {
                        inherited_rounding = config.map(|c| c.use_rounding);
                    }
                    if fixed {
                        break;
                    }
                }
                parent = children.get_parent(entity);
            }
            if nested {
                continue;
            }
            let use_rounding =
                config.map_or(inherited_rounding.unwrap_or(true), |c| c.use_rounding);
            let parent_matrix = if fixed {
                Mat2::IDENTITY
            } else {
                children
                    .get_parent(root)
                    .and_then(|p| geometry.get(p).ok())
                    .map_or(Mat2::IDENTITY, |(_, _, t, ..)| t.matrix2)
            };
            let object = runtime.store().object(id).unwrap();
            let baseline = transform.compute_affine(
                node.inverse_scale_factor().recip(),
                node.size(),
                target.physical_size().as_vec2(),
                node.em_size,
                node.rem_size,
            );
            let desired = local_skew(object, baseline, node.inverse_scale_factor().recip());
            let adjusted = Affine2 {
                matrix2: global.matrix2 + parent_matrix * (desired.matrix2 - baseline.matrix2),
                translation: global.translation
                    + parent_matrix * (desired.translation - baseline.translation),
            };
            if !adjusted.is_finite() {
                continue;
            }
            updates.push((root, adjusted));
            let mut pending: Vec<_> = children
                .iter_ui_children(root)
                .map(|child| {
                    (
                        child,
                        adjusted,
                        node.size(),
                        node.scroll_position,
                        use_rounding,
                    )
                })
                .collect();
            while let Some((entity, parent_transform, parent_size, parent_scroll, rounding)) =
                pending.pop()
            {
                let Ok((_, node, _, transform, target, reference, config, ignore, fixed)) =
                    geometry.get(entity)
                else {
                    continue;
                };
                if fixed {
                    continue;
                }
                let rounding = config.map_or(rounding, |c| c.use_rounding);
                let Ok((layout, _)) = surface.get_layout(entity, rounding) else {
                    continue;
                };
                let mut local = transform.compute_affine(
                    node.inverse_scale_factor().recip(),
                    node.size(),
                    target.physical_size().as_vec2(),
                    node.em_size,
                    node.rem_size,
                );
                if let Some(object) = reference
                    .filter(|r| runtime.entity_of(r.0) == Some(entity))
                    .and_then(|r| runtime.store().object(r.0))
                    && (object.skew_x() != 0. || object.skew_y() != 0.)
                {
                    local = local_skew(object, local, node.inverse_scale_factor().recip());
                }
                let scroll = ignore.map_or(parent_scroll, |i| parent_scroll * Vec2::from(!i.0));
                local.translation += Vec2::new(layout.location.x, layout.location.y) - scroll
                    + (node.size() - parent_size) * 0.5;
                let adjusted = parent_transform * local;
                if !adjusted.is_finite() {
                    continue;
                }
                updates.push((entity, adjusted));
                pending.extend(
                    children.iter_ui_children(entity).map(|child| {
                        (child, adjusted, node.size(), node.scroll_position, rounding)
                    }),
                );
            }
        }
        updates
    };
    let mut geometry = queries.p1();
    for (entity, affine) in updates {
        if let Ok(mut current) = geometry.get_mut(entity)
            && current.affine() != affine
        {
            *current = affine.into();
        }
    }
}

fn local_skew(object: &crate::core::GObject, mut baseline: Affine2, scale: f32) -> Affine2 {
    let [a, b, c, d] = object.transform_axes();
    let matrix = Mat2::from_cols(Vec2::new(a as f32, b as f32), Vec2::new(c as f32, d as f32));
    let plain = backend::transform_for(object);
    let unskewed = Mat2::from(plain.rotation) * Mat2::from_diagonal(plain.scale);
    let delta = matrix - unskewed;
    let center = Vec2::new(object.width(), object.height()) * 0.5;
    let pivot = Vec2::new(
        object.pivot().x * object.width(),
        object.pivot().y * object.height(),
    );
    baseline.matrix2 += delta;
    baseline.translation += delta
        * if object.display_position().is_some() {
            center
        } else {
            center - pivot
        }
        * scale;
    baseline
}

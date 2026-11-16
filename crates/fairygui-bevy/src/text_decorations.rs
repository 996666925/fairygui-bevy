//! Native strike extension affects paint only, preserving links and backgrounds.
use crate::runtime::text_measure::vertical::NativeTextLayout;
use bevy::{
    prelude::*,
    render::{Extract, ExtractSchedule, RenderApp},
    text::{ComputedTextBlock, Strikethrough, TextLayoutInfo},
    ui_render::{ExtractedUiItem, ExtractedUiNode, ExtractedUiNodes, RenderUiSystems},
};

#[cfg(test)]
#[path = "text_decoration_tests.rs"]
mod tests;

pub(crate) fn install(app: &mut App) {
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render.add_systems(
            ExtractSchedule,
            extend_strikethroughs
                .after(RenderUiSystems::ExtractTextBackgrounds)
                .before(RenderUiSystems::ExtractText),
        );
    }
}

pub(crate) fn project_geometry(
    run: &mut bevy::text::RunGeometry,
    top: f32,
    height: f32,
    size: f32,
    scale: f32,
) {
    let thickness = (size / 16.).max(1.);
    // Canvas drawLine's y is its center; Bevy stores its top edge.
    run.underline_y = (top + height - thickness * 1.5) * scale;
    run.underline_thickness = thickness * scale;
    run.strikethrough_y = ((top + height * 0.5 - thickness).trunc() - thickness * 0.5) * scale;
    run.strikethrough_thickness = thickness * scale;
}

/// Locate the exact rectangle emitted by Bevy for this strike. Shadow and
/// background rectangles use different z offsets; glyphs use a different item.
/// A new frame's changed nodes are removed/rebuilt by Bevy before this system.
fn extend_strike(
    items: &mut bevy::ecs::entity::EntityIndexMap<ExtractedUiNode>,
    run: &bevy::text::RunGeometry,
    transform: bevy::math::Affine2,
    z_order: f32,
    scale: f32,
) {
    if run.bounds.width() <= 0. || run.strikethrough_thickness <= 0. {
        return;
    }
    let expected = transform * bevy::math::Affine2::from_translation(run.strikethrough_position());
    for (_, node) in items.iter_mut() {
        if node.z_order != z_order || node.transform != expected {
            continue;
        }
        let ExtractedUiItem::Node { rect, .. } = &mut node.item else {
            continue;
        };
        if rect.min == Vec2::ZERO && rect.max == run.strikethrough_size() {
            // Rect UI items are centered by the renderer. Extending both ends
            // therefore changes width only; position/clip/color remain valid.
            rect.max.x += 8. * scale;
            break;
        }
    }
}

#[allow(clippy::type_complexity)]
fn extend_strikethroughs(
    mut nodes: ResMut<ExtractedUiNodes>,
    roots: Extract<
        Query<
            (
                &ComputedTextBlock,
                &TextLayoutInfo,
                &ComputedNode,
                &UiGlobalTransform,
                &bevy::ui::ComputedStackIndex,
            ),
            (With<NativeTextLayout>, Without<bevy::text::EditableText>),
        >,
    >,
    strikes: Extract<Query<(), (With<Strikethrough>, With<TextColor>)>>,
) {
    let nodes = &mut *nodes;
    for entity in &nodes.changed {
        let Ok((block, info, node, global, stack)) = roots.get(entity.entity()) else {
            continue;
        };
        let Some((_, items)) = nodes.uinodes.get_mut(entity) else {
            continue;
        };
        let transform = bevy::math::Affine2::from(global)
            * bevy::math::Affine2::from_translation(node.content_box().min);
        let z_order = stack.0 as f32 + bevy::ui_render::stack_z_offsets::TEXT_STRIKETHROUGH;
        for run in &info.run_geometry {
            if block
                .entities()
                .get(run.section_index as usize)
                .is_some_and(|section| strikes.contains(section.entity))
            {
                extend_strike(items, run, transform, z_order, info.scale_factor);
            }
        }
    }
}

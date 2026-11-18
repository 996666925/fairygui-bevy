//! Project native list offsets and preserve model overscroll after Bevy clamps it.
use super::*;

type PaneGeometry<'a> = (&'a GObjectRef, &'a ComputedNode, &'a UiGlobalTransform);

pub(crate) fn project_scroll_overflow(
    runtime: NonSend<GuiRuntime>,
    mut geometry: ParamSet<(Query<PaneGeometry>, Query<&mut UiGlobalTransform>)>,
    children: Query<&Children>,
    ignored: Query<&bevy::ui::IgnoreScroll>,
) {
    let mut translations = Vec::new();
    for (owner, node, transform) in &geometry.p0() {
        let Some(component) = runtime
            .store()
            .object(owner.0)
            .and_then(|o| o.control().component())
        else {
            continue;
        };
        let pane = component.scroll_pane.as_ref();
        let max = pane.map_or(fairygui_core::geom::Point::ZERO, |p| p.max_scroll_pos());
        let display = pane.map_or(fairygui_core::geom::Point::ZERO, |p| p.display_pos());
        let scale = node.inverse_scale_factor().recip();
        let origin = runtime
            .store()
            .component_content_origin(owner.0)
            .unwrap_or_default();
        let hidden_scroll = runtime
            .store()
            .object(owner.0)
            .unwrap()
            .component_scroll_offset();
        let offset = Vec2::new(
            if display.x < 0. || display.x > max.x {
                node.scroll_position.x - display.x * scale
            } else {
                0.
            },
            if display.y < 0. || display.y > max.y {
                node.scroll_position.y - display.y * scale
            } else {
                0.
            },
        ) + Vec2::new(origin.x - hidden_scroll.x, origin.y - hidden_scroll.y) * scale;
        // Content follows its retained origin; refresh bars follow the mask.
        // Ordinary scrollbars and the ECS-only extent stay on the owner.
        // All paint descendants receive each ancestor's offset once.
        for object in runtime
            .store()
            .object(owner.0)
            .map_or(&[][..], |object| object.children())
            .iter()
            .copied()
            .filter(|&id| !runtime.store().is_scroll_bar_overlay(id))
        {
            let Some(root) = runtime.entity_of(object) else {
                continue;
            };
            let effective = if runtime.store().is_scroll_refresh_overlay(object) {
                let origin = runtime
                    .store()
                    .scroll_mask_origin(owner.0)
                    .unwrap_or_default();
                Vec2::new(origin.x, origin.y) * scale
            } else {
                ignored
                    .get(root)
                    .map_or(offset, |ignore| offset * Vec2::from(!ignore.0))
            };
            if !effective.is_finite() || effective == Vec2::ZERO {
                continue;
            }
            let delta = transform.transform_vector2(effective);
            let mut pending = vec![root];
            while let Some(entity) = pending.pop() {
                translations.push((entity, delta));
                if let Ok(list) = children.get(entity) {
                    pending.extend(list.iter());
                }
            }
        }
    }
    for (entity, delta) in translations {
        if let Ok(mut transform) = geometry.p1().get_mut(entity) {
            let mut affine = transform.affine();
            affine.translation += delta;
            *transform = affine.into();
        }
    }
}

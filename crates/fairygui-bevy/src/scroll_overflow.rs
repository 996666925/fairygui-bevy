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
        let align = runtime.store().list_alignment(owner.0);
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
        ) + Vec2::new(
            align.x + component.margin.left,
            align.y + component.margin.top,
        ) * scale;
        if !offset.is_finite() || offset == Vec2::ZERO {
            continue;
        }
        // Only logical content moves. Registered bars and the ECS-only extent
        // stay in the pane coordinate system. All paint descendants move once.
        for object in runtime
            .store()
            .object(owner.0)
            .map_or(&[][..], |object| object.children())
            .iter()
            .copied()
            .filter(|&id| !runtime.store().is_scroll_overlay(id))
        {
            let Some(root) = runtime.entity_of(object) else {
                continue;
            };
            let effective = ignored
                .get(root)
                .map_or(offset, |ignore| offset * Vec2::from(!ignore.0));
            if effective == Vec2::ZERO {
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

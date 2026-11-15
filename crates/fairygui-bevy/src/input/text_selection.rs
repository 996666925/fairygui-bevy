//! Selection scrolling follows FairyGUI capture, including stationary touches.
use super::*;
use bevy::text::{EditableText, TextEdit, TextLayout, TextLayoutInfo};

fn scroll_axis(overflow: f32, size: f32, dt: f32) -> f32 {
    if size <= 0. || overflow == 0. {
        return 0.;
    }
    let ramp = (overflow.abs() / (size * 0.5)).min(1.);
    overflow.signum() * size * (0.75 + ramp * 1.25) * dt
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn autoscroll(
    state: Res<InputRouter>,
    focus: Res<bevy::input_focus::InputFocus>,
    mouse: Res<ButtonInput<MouseButton>>,
    time: Option<Res<Time<Real>>>,
    windows: Query<&Window>,
    views: PointerViews,
    runtime: NonSend<GuiRuntime>,
    mut editors: Query<(
        &mut EditableText,
        &ComputedNode,
        &UiGlobalTransform,
        &TextLayoutInfo,
        &TextLayout,
    )>,
) {
    let Some(entity) = focus.get() else { return };
    let pointer = state
        .capture
        .as_ref()
        .filter(|capture| {
            capture.editing == Some(entity) && capture.moved && mouse.pressed(MouseButton::Left)
        })
        .map(|capture| (capture.window, capture.last, capture.target))
        .or_else(|| {
            state
                .touches
                .values()
                .find(|capture| capture.editing == Some(entity) && capture.moved)
                .map(|capture| (capture.window, capture.last, capture.target))
        });
    let Some((window, position, owner)) = pointer else {
        return;
    };
    let Ok(window) = windows.get(window) else {
        return;
    };
    if !window.focused || !eligible(&runtime, owner) {
        return;
    }
    let Ok((mut editor, node, transform, info, layout)) = editors.get_mut(entity) else {
        return;
    };
    if editor.is_composing()
        || editor
            .pending_edits
            .iter()
            .any(|edit| matches!(edit,TextEdit::ImeSetCompose {value,..} if !value.is_empty()))
    {
        return;
    }
    let Some(inverse) = transform.try_inverse() else {
        return;
    };
    if editor.viewport.size.min_element() <= 0. {
        return;
    }
    let local =
        inverse.transform_point2(views.physical_position(entity, position, window.scale_factor()));
    let clamped = node.content_box().clamp_point(local);
    let overflow = local - clamped;
    let dt = time.as_ref().map_or(0., |time| time.delta_secs());
    let delta = Vec2::new(
        scroll_axis(overflow.x, editor.viewport.size.x, dt),
        scroll_axis(overflow.y, editor.viewport.size.y, dt),
    );
    let content = Vec2::new(
        bevy::text::scrollable_text_layout_width(
            layout.linebreak,
            info.size.x,
            editor.viewport.size.x,
            info.cursor.map(|(_, rect)| rect),
        ),
        info.size.y,
    );
    let offset = (editor.viewport.offset + delta)
        .clamp(Vec2::ZERO, (content - editor.viewport.size).max(Vec2::ZERO));
    // Bevy's generic widget autoscroll can also see picking drag state. Replace
    // its queued pointer scroll/selection with this capture's viewport-aware step.
    editor.pending_edits.retain(|edit| {
        !matches!(
            edit,
            TextEdit::ScrollBy(_) | TextEdit::ExtendSelectionToPoint(_)
        )
    });
    let displacement = offset - editor.viewport.offset;
    if displacement != Vec2::ZERO {
        editor.queue_edit(TextEdit::ScrollBy(displacement));
    }
    editor.queue_edit(TextEdit::ExtendSelectionToPoint(
        clamped - node.content_box().min + offset,
    ));
}

pub(super) fn install(app: &mut App) {
    app.add_systems(
        PostUpdate,
        (autoscroll, crate::runtime::apply_selection_edits)
            .chain()
            .in_set(bevy::text::EditableTextSystems)
            .after(crate::runtime::prepare_password_layout)
            .before(bevy::text::apply_text_edits),
    );
}

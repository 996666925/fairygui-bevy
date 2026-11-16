//! Selection scrolling follows FairyGUI capture, including stationary touches.
use super::*;
use bevy::text::{EditableText, TextEdit, TextLayout, TextLayoutInfo};

#[derive(Debug)]
pub(super) struct PendingPress {
    pub entity: Entity,
    pub window: Entity,
    pub position: Vec2,
    pub extend: bool,
    pub clicks: u8,
}

/// A bitmap/styled input first replaces its canvas renderer. Resolve the initial
/// press against that frame's editing viewport, never the old glyph geometry.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn apply_pending_press(
    mut state: ResMut<InputRouter>,
    focus: Res<bevy::input_focus::InputFocus>,
    runtime: NonSend<GuiRuntime>,
    views: PointerViews,
    windows: Query<&Window>,
    mut editors: Query<
        (
            &GObjectRef,
            &mut EditableText,
            &ComputedNode,
            &UiGlobalTransform,
            &TextLayout,
            Option<&crate::runtime::InputPasswordView>,
        ),
        With<crate::runtime::InputEditPolicy>,
    >,
    masks: Query<&EditableText, Without<crate::runtime::InputEditPolicy>>,
    fonts: Option<ResMut<bevy::text::FontCx>>,
    layouts: Option<ResMut<bevy::text::LayoutCx>>,
) {
    let Some(press) = state.pending_text_press.take() else {
        return;
    };
    if focus.get() != Some(press.entity) {
        return;
    }
    let Ok(window) = windows.get(press.window) else {
        return;
    };
    let Ok((owner, mut editor, node, transform, text_layout, password)) =
        editors.get_mut(press.entity)
    else {
        return;
    };
    if !window.focused || !eligible(&runtime, owner.0) || editor.is_composing() {
        return;
    }
    let Some(inverse) = transform.try_inverse() else {
        return;
    };
    let (Some(mut fonts), Some(mut layouts)) = (fonts, layouts) else {
        return;
    };
    editor.editor.refresh_layout(&mut fonts, &mut layouts.0);
    let display = password
        .and_then(|view| masks.get(view.0).ok())
        .unwrap_or(&editor);
    let Some(layout) = display.editor.try_layout() else {
        return;
    };
    let info = TextLayoutInfo {
        size: Vec2::new(layout.full_width(), layout.height()),
        scale_factor: layout.scale(),
        ..default()
    };
    let Some(object) = runtime.store().object(owner.0) else {
        return;
    };
    let Some(data) = object.control().as_text_field() else {
        return;
    };
    let gutter =
        if !matches!(data.auto_size, 3 | 4) && runtime.text_measurer.borrow().contains(owner.0) {
            2.
        } else {
            0.
        };
    // Center/right and vertical alignment change when raw markup replaces the
    // short canvas text. Predict the final inset from the new editor layout.
    let mut projected = *node;
    crate::backend::align_text_block(
        object,
        data,
        &info,
        text_layout,
        &mut Node::default(),
        &mut projected,
        gutter,
    );
    let size = projected.content_box().size();
    if editor.viewport.size != size {
        editor.viewport.size = size;
        editor.editor.set_width(Some(size.x));
    }
    let point = inverse.transform_point2(views.physical_position(
        press.entity,
        press.position,
        window.scale_factor(),
    )) - projected.content_box().min
        + editor.viewport.offset;
    editor.queue_edit(if press.extend {
        TextEdit::ShiftClickExtension(point)
    } else if press.clicks == 2 {
        TextEdit::SelectWordAtPoint(point)
    } else if press.clicks == 3 {
        TextEdit::SelectAll
    } else {
        TextEdit::MoveToPoint(point)
    });
}

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
        &GObjectRef,
        &mut EditableText,
        &ComputedNode,
        &UiGlobalTransform,
        &TextLayoutInfo,
        &TextLayout,
    )>,
) {
    let Some(entity) = focus.get() else { return };
    let Ok((reference, mut editor, node, transform, info, layout)) = editors.get_mut(entity) else {
        return;
    };
    if runtime.entity_of(reference.0) != Some(entity) {
        return;
    }
    // Generic picking can retain a drag after a direct selection cancels GUI
    // editing capture. Only GUI capture may enqueue these actions for our input.
    let generic_pointer_edit = |edit: &TextEdit| {
        matches!(
            edit,
            TextEdit::ScrollBy(_) | TextEdit::ExtendSelectionToPoint(_)
        )
    };
    if editor.pending_edits.iter().any(generic_pointer_edit) {
        editor
            .pending_edits
            .retain(|edit| !generic_pointer_edit(edit));
    }
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
        (
            apply_pending_press,
            autoscroll,
            crate::runtime::apply_selection_edits,
        )
            .chain()
            .in_set(bevy::text::EditableTextSystems)
            .after(crate::runtime::prepare_password_layout)
            .before(bevy::text::apply_text_edits),
    );
}

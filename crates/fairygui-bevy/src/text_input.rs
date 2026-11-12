//! Native user-edit validation around Bevy's editor and asynchronous clipboard.
use super::*;
use bevy::{
    clipboard::{Clipboard, ClipboardRead},
    text::{EditableText, FontCx, LayoutCx, TextEdit},
};
use fairygui_core::text::input::{InputRestriction, limit_input_insert};

#[cfg(test)]
#[path = "text_input_tests.rs"]
mod tests;

/// Bevy draws caret/selection only when TextCursorStyle is present. A native
/// one-logical-pixel caret scales with the window; foreground opacity also
/// applies to selection paint, which otherwise stays visible on transparent UI.
#[allow(clippy::type_complexity)]
pub(crate) fn sync_input_cursors(
    mut inputs: Query<
        (
            &TextColor,
            &TextFont,
            &bevy::ui::ComputedUiRenderTargetInfo,
            &mut EditableText,
            &mut bevy::text::TextCursorStyle,
        ),
        With<InputEditPolicy>,
    >,
    rem: Option<Res<bevy::text::RemSize>>,
) {
    use bevy::color::Alpha;
    let rem = rem.map(|r| *r).unwrap_or_default();
    for (color, font, target, mut editor, mut style) in &mut inputs {
        let width = target.scale_factor() / font.font_size.eval(target.logical_size(), rem).max(1.);
        if editor.cursor_width != width {
            editor.cursor_width = width;
        }
        let next = bevy::text::TextCursorStyle {
            color: color.0,
            selection_color: style.selection_color.with_alpha(color.0.alpha()),
            unfocused_selection_color: style.unfocused_selection_color.with_alpha(color.0.alpha()),
            selected_text_color: style
                .selected_text_color
                .map(|c| c.with_alpha(color.0.alpha())),
            ..*style
        };
        if *style != next {
            *style = next;
        }
    }
}

#[derive(Component, Debug)]
pub(crate) struct InputEditPolicy {
    restrict: Option<String>,
    multiline: bool,
    max: i32,
    editable: bool,
    password: bool,
    compiled: Option<InputRestriction>,
    paste: Option<ClipboardRead>,
    queued: std::collections::VecDeque<InputCommand>,
}

#[derive(Debug)]
enum InputCommand {
    Edit(TextEdit),
    Submit,
}

impl InputEditPolicy {
    pub(crate) fn queue_submit(&mut self, editor: &mut EditableText) {
        self.queued.extend(
            std::mem::take(&mut editor.pending_edits)
                .into_iter()
                .map(InputCommand::Edit),
        );
        self.queued.push_back(InputCommand::Submit);
    }
    pub(crate) fn cancel_session(&mut self, editor: &mut EditableText) {
        self.paste = None;
        self.queued.clear();
        editor.pending_edits.clear();
        editor.pending_paste = None;
        editor.queue_edit(TextEdit::clear_ime_compose());
        editor.queue_edit(TextEdit::CollapseSelection);
    }
    pub(super) fn new(data: &fairygui_core::widget::TextInputData) -> Self {
        let compiled = InputRestriction::new(data.restrict.as_deref());
        if let Err(error) = &compiled {
            tracing::warn!(%error, "Invalid input restriction; user insertions disabled");
        }
        Self {
            restrict: data.restrict.clone(),
            multiline: !data.text_field.single_line && !data.password,
            max: data.max_length,
            editable: data.editable,
            password: data.password,
            compiled: compiled.ok(),
            paste: None,
            queued: Default::default(),
        }
    }
}

pub(crate) fn sync_input_policy(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    mut inputs: Query<(
        Entity,
        &GObjectRef,
        &mut EditableText,
        &mut TextLayout,
        Option<&mut InputEditPolicy>,
    )>,
) {
    for (entity, owner, mut editor, mut layout, policy) in &mut inputs {
        let Some(ControlData::TextInput(data)) =
            runtime.store().object(owner.0).map(|o| o.control())
        else {
            continue;
        };
        // Native TextInputAdapter always selects its single-line password element.
        let multiline = !data.text_field.single_line && !data.password;
        let mut next_layout = backend::text_layout_for(&data.text_field);
        if data.password {
            next_layout.linebreak = bevy::text::LineBreak::NoWrap;
        }
        if layout.justify != next_layout.justify || layout.linebreak != next_layout.linebreak {
            *layout = next_layout;
        }
        if editor.allow_newlines != multiline {
            editor.allow_newlines = multiline;
        }
        // Our bridge clips UTF-16 insertions; Bevy's scalar-count whole-insert
        // rejection must not run a second time over the same editing operation.
        if editor.max_characters.is_some() {
            editor.max_characters = None;
        }
        commands.entity(entity).insert(if data.editable {
            bevy::text::TextReadWriteMode::Editable
        } else {
            bevy::text::TextReadWriteMode::ReadOnly
        });
        if let Some(mut policy) = policy {
            if policy.restrict != data.restrict {
                let compiled = InputRestriction::new(data.restrict.as_deref());
                if let Err(error) = &compiled {
                    tracing::warn!(%error, "Invalid input restriction; user insertions disabled");
                }
                policy.compiled = compiled.ok();
                policy.restrict.clone_from(&data.restrict);
            }
            policy.max = data.max_length;
            policy.multiline = multiline;
            policy.password = data.password;
            if policy.editable && !data.editable {
                policy.paste = None;
                editor.pending_paste = None;
                // Retain navigation/copy, discard queued mutations on lock.
                policy.queued.retain(|edit| match edit {
                    InputCommand::Edit(edit) => !edit.is_destructive(),
                    InputCommand::Submit => true,
                });
            }
            policy.editable = data.editable;
        } else {
            commands.entity(entity).insert(InputEditPolicy::new(data));
        }
    }
}

/// Run after Bevy has synchronized editor styles/viewports, before its normal
/// edit system. Only GUI editors with this policy have their queues consumed.
#[allow(clippy::type_complexity)]
pub(crate) fn apply_input_edits(
    mut inputs: Query<(
        &mut EditableText,
        &mut InputEditPolicy,
        Option<&input_password::InputPasswordView>,
        Option<&GObjectRef>,
        Option<&mut InputTextState>,
        Option<&mut Text>,
    )>,
    mut password_views: Query<&mut EditableText, Without<InputEditPolicy>>,
    fonts: Option<ResMut<FontCx>>,
    layouts: Option<ResMut<LayoutCx>>,
    clipboard: Option<ResMut<Clipboard>>,
    mut runtime: NonSendMut<GuiRuntime>,
) {
    let (Some(mut fonts), Some(mut layouts), Some(mut clipboard)) = (fonts, layouts, clipboard)
    else {
        return;
    };
    for (mut editable, mut policy, password_view, owner, mut input_state, mut text) in &mut inputs {
        if owner.is_some_and(|owner| {
            runtime
                .input_submit
                .is_some_and(|(pending, _)| pending == owner.0)
        }) {
            continue;
        }
        if !editable.pending_edits.is_empty()
            || editable.pending_paste.is_some()
            || !policy.queued.is_empty()
            || policy.paste.is_some()
        {
            editable.set_changed();
        }
        if !policy.editable && editable.is_composing() {
            editable
                .editor
                .driver(&mut fonts, &mut layouts.0)
                .clear_compose();
        }
        // Access without marking the editor changed merely by polling it.
        let editable = editable.bypass_change_detection();
        policy.queued.extend(
            std::mem::take(&mut editable.pending_edits)
                .into_iter()
                .map(InputCommand::Edit),
        );
        if policy.paste.is_none() {
            policy.paste = editable.pending_paste.take();
        }
        loop {
            let edit = if let Some(mut read) = policy.paste.take() {
                match read.poll_result() {
                    None => {
                        policy.paste = Some(read);
                        break;
                    }
                    Some(Ok(value)) => TextEdit::Insert(value.into()),
                    Some(Err(error)) => {
                        tracing::warn!(%error, "Failed to read input clipboard");
                        continue;
                    }
                }
            } else if let Some(command) = policy.queued.pop_front() {
                match command {
                    InputCommand::Edit(edit) => edit,
                    InputCommand::Submit => {
                        if let Some(owner) = owner {
                            runtime.emit_input_event(
                                fairygui_core::object::EventKind::Enter,
                                owner.0,
                                &editable.value().to_string(),
                            );
                            runtime.input_submit = Some((owner.0, false));
                        }
                        break;
                    }
                }
            } else {
                break;
            };
            if !policy.editable
                && (edit.is_destructive()
                    || matches!(
                        &edit,
                        TextEdit::ImeSetCompose { .. } | TextEdit::ImeCommit { .. }
                    ))
            {
                if matches!(
                    &edit,
                    TextEdit::ImeSetCompose { .. } | TextEdit::ImeCommit { .. }
                ) {
                    editable
                        .editor
                        .driver(&mut fonts, &mut layouts.0)
                        .clear_compose();
                }
                continue;
            }
            if matches!(edit, TextEdit::Paste) {
                policy.paste = Some(clipboard.fetch_text());
                continue;
            }
            let edit = if policy.password {
                if matches!(edit, TextEdit::Copy) {
                    continue;
                }
                if let Some(view) = password_view
                    && let Ok(mut target) = password_views.get_mut(view.0)
                    && input_password::apply_projected_edit(
                        &edit,
                        editable,
                        &mut target,
                        &mut fonts,
                        &mut layouts,
                        &mut clipboard,
                    )
                {
                    continue;
                }
                if matches!(edit, TextEdit::Cut) {
                    // Delete the selection without exporting the hidden value.
                    TextEdit::Insert("".into())
                } else {
                    edit
                }
            } else {
                edit
            };
            let mut driver = editable.editor.driver(&mut fonts, &mut layouts.0);
            let edit = match edit {
                TextEdit::Insert(value) => {
                    if policy.compiled.is_none() {
                        continue;
                    }
                    let current = driver.editor.text().to_string();
                    let selected = driver.editor.selected_text().unwrap_or_default();
                    TextEdit::Insert(
                        limit_input_insert(&current, selected, &value, policy.max).into(),
                    )
                }
                TextEdit::ImeCommit { value } => {
                    if policy.compiled.is_none() {
                        driver.clear_compose();
                        continue;
                    }
                    let current = driver.editor.text().to_string();
                    let selected = driver.editor.selected_text().unwrap_or_default();
                    TextEdit::ImeCommit {
                        value: limit_input_insert(&current, selected, &value, policy.max).into(),
                    }
                }
                other => other,
            };
            let committed = matches!(
                &edit,
                TextEdit::Insert(_)
                    | TextEdit::ImeCommit { .. }
                    | TextEdit::Backspace
                    | TextEdit::Delete
                    | TextEdit::BackspaceWord
                    | TextEdit::DeleteWord
                    | TextEdit::Cut
            );
            let before = committed.then(|| driver.editor.text().to_string());
            edit.apply(
                &mut driver,
                &mut editable.viewport,
                editable.cursor_margin,
                &mut clipboard,
                None,
                |_| true,
            );
            if committed
                && !driver.editor.is_composing()
                && let Some(restrict) = &policy.compiled
            {
                let current = driver.editor.text().to_string();
                if before.as_deref() == Some(current.as_str()) {
                    continue;
                }
                let validated = restrict.validate(&current, policy.multiline);
                if validated != current {
                    driver.editor.set_text(&validated);
                    driver.refresh_layout();
                }
            }
            if committed && !driver.editor.is_composing() {
                let value = driver.editor.text().to_string();
                if before.as_deref() != Some(&value)
                    && let Some(owner) = owner
                {
                    runtime.store_mut().set_object_text(owner.0, &value);
                    if let Some(state) = input_state.as_mut() {
                        state.source.clone_from(&value);
                        state.display.clone_from(&value);
                    }
                    if let Some(text) = text.as_mut() {
                        let display = runtime
                            .store()
                            .object(owner.0)
                            .map(|o| text_fonts::display_text(o, &value))
                            .unwrap_or_default();
                        if text.0 != display {
                            text.0 = display;
                        }
                    }
                    runtime.emit_input_event(
                        fairygui_core::object::EventKind::Input,
                        owner.0,
                        &value,
                    );
                }
            }
        }
    }
}

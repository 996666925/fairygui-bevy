//! Focused keyboard/IME input under the same hit and modal policy as GUI pointers.
use super::*;
use crate::runtime::{InputEditPolicy, InputPasswordView};
use bevy::input::{
    ButtonState,
    keyboard::{Key, KeyCode, KeyboardInput},
};
use bevy::input_focus::{FocusCause, FocusedInput, InputFocus, InputFocusSystems};
use bevy::text::{EditableText, TextEdit};
use bevy::window::{Ime, PrimaryWindow};

#[cfg(test)]
#[path = "editing_tests.rs"]
mod tests;

#[derive(Resource, Default)]
struct GuiImeWindow(Option<Entity>);

/// Entity-addressed input responses. Send in `GuiSystems::Respond` (or
/// PostLayout); commands execute in order after view/policy synchronization and
/// before focus reconciliation. Targets must already be materialized; requests
/// for missing entities or assets still loading are discarded.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiInputCommand {
    /// Focus an input view, or a compound control whose title is an input.
    /// Root/window owners are accepted too. Normal visibility/modal rules apply.
    Focus(Entity),
    /// Release GUI text focus without clearing another Bevy widget's focus.
    Blur,
    /// Keep the current input session open after its Enter notification.
    /// Send while handling `GuiTextInput` in Respond.
    PreventSubmit(Entity),
}

pub(crate) fn apply_input_commands(
    mut messages: MessageReader<GuiInputCommand>,
    objects: Query<&GObjectRef>,
    roots: Query<&crate::GuiRootInstance>,
    mut runtime: NonSendMut<GuiRuntime>,
) {
    for &message in messages.read() {
        let target = match message {
            GuiInputCommand::Focus(target) | GuiInputCommand::PreventSubmit(target) => target,
            GuiInputCommand::Blur => {
                runtime.request_input_focus(None);
                continue;
            }
        };
        let object = objects
            .get(target)
            .map(|reference| reference.0)
            .or_else(|_| roots.get(target).map(|root| root.object))
            .ok()
            .or_else(|| runtime.ecs_window_object(target));
        let Some(input) = object.and_then(|object| runtime.store().input_field_of(object)) else {
            continue;
        };
        if runtime.entity_of(input).is_none() {
            continue;
        }
        match message {
            GuiInputCommand::Focus(_) => {
                runtime.request_input_focus(Some(input));
            }
            GuiInputCommand::PreventSubmit(_) => {
                runtime.prevent_input_submit(input);
            }
            GuiInputCommand::Blur => unreachable!(),
        }
    }
}

pub(super) fn register(app: &mut App) {
    app.init_resource::<InputFocus>()
        .add_message::<GuiInputCommand>()
        .init_resource::<ButtonInput<Key>>()
        .init_resource::<GuiImeWindow>()
        .add_message::<Ime>()
        .add_observer(keyboard_input)
        .add_observer(keep_active_input_focus)
        .add_systems(
            PreUpdate,
            reconcile_focus
                .after(bevy::input::InputSystems)
                .before(InputFocusSystems::Dispatch),
        )
        .add_systems(PreUpdate, ime_input.after(InputFocusSystems::Dispatch))
        .add_systems(
            PostUpdate,
            update_ime_window
                .after(bevy::ui_widgets::ImeSystems::UpdatePosition)
                .after(crate::runtime::publish_password_layout),
        );
}

// Default PointerFocusPlugin clears focus at the window when no TabIndex claims
// the hit. Re-clicking our active editor must not end and restart its session;
// new GUI focus acquisition remains with the authoritative GUI pointer router.
fn keep_active_input_focus(
    mut event: On<bevy::input_focus::AcquireFocus>,
    focus: Res<InputFocus>,
    editors: Query<&GObjectRef, With<InputEditPolicy>>,
    runtime: NonSend<GuiRuntime>,
) {
    if focus.get() == Some(event.focused_entity)
        && let Ok(owner) = editors.get(event.focused_entity)
        && eligible(&runtime, owner.0)
    {
        event.propagate(false);
    }
}

fn selected_window(entity: Entity, primary: bool, settings: &GuiInputSettings) -> bool {
    settings.window.map_or(primary, |chosen| chosen == entity)
}

/// Observe external InputFocus changes and requests before input dispatch and
/// after GUI mutations. Native input notifications are target-only, not bubbled.
pub(crate) fn reconcile_focus(
    mut focus: ResMut<InputFocus>,
    mut state: ResMut<GuiInputState>,
    settings: Res<GuiInputSettings>,
    windows: Query<(Entity, &Window, Has<PrimaryWindow>)>,
    objects: Query<&GObjectRef>,
    mut editors: Query<(&mut EditableText, &mut InputEditPolicy)>,
    mut runtime: NonSendMut<GuiRuntime>,
) {
    let mut request = runtime.input_focus_request.take();
    if let Some(Some(owner)) = request
        && runtime.entity_of(owner).is_none()
        && runtime.store().object(owner).is_some()
    {
        runtime.input_focus_request = request;
        request = None;
    }
    if let Some(request) = request {
        if let Some(entity) = request.and_then(|owner| runtime.entity_of(owner)) {
            focus.set(entity, FocusCause::Navigated);
        } else if focus.get().is_some_and(|entity| {
            state.focused_entity == Some(entity)
                || objects
                    .get(entity)
                    .is_ok_and(|owner| runtime.store().input_field_of(owner.0) == Some(owner.0))
        }) {
            focus.clear();
        }
    }
    if let Some((owner, true)) = runtime.input_submit {
        runtime.input_submit = None;
        if runtime.focused_input == Some(owner) && focus.get() == runtime.entity_of(owner) {
            focus.clear();
        }
    }
    let window_active = windows
        .iter()
        .find(|(entity, _, primary)| selected_window(*entity, *primary, &settings))
        .is_none_or(|(_, window, _)| window.focused);
    let current = focus.get();
    let candidate = current
        .and_then(|entity| objects.get(entity).ok())
        .map(|o| o.0);
    let next = candidate.filter(|&owner| {
        window_active
            && eligible(&runtime, owner)
            && runtime
                .entity_of(owner)
                .is_some_and(|entity| editors.contains(entity))
            && runtime.store().input_field_of(owner) == Some(owner)
    });
    if next.is_none()
        && candidate.is_some_and(|owner| runtime.store().input_field_of(owner) == Some(owner))
    {
        focus.clear();
    }
    if runtime.focused_input != next {
        if let Some(old) = runtime.focused_input.take() {
            let mut value = runtime
                .store()
                .object(old)
                .and_then(|o| o.control().as_text_field())
                .map(|d| d.text.clone())
                .unwrap_or_default();
            if let Some(entity) = runtime.entity_of(old)
                && let Ok((mut editor, mut policy)) = editors.get_mut(entity)
            {
                value = editor.value().to_string();
                policy.cancel_session(&mut editor);
            }
            if let Some(object) = runtime.store().object(old) {
                let editable = matches!(object.control(), fairygui_core::widget::ControlData::TextInput(d) if d.editable);
                if editable {
                    runtime.emit_input_event(EventKind::Change, old, &value);
                }
                runtime.emit_input_event(EventKind::FocusOut, old, &value);
            }
            runtime.input_submit = None;
        }
        runtime.focused_input = next;
        if let Some(owner) = next
            && let Some(entity) = runtime.entity_of(owner)
            && let Ok((editor, _)) = editors.get(entity)
        {
            runtime.emit_input_event(EventKind::FocusIn, owner, &editor.value().to_string());
        }
    }
    state.focused = next;
    state.focused_entity = next.and_then(|owner| runtime.entity_of(owner));
}

fn shortcut(input: &KeyboardInput, letter: &str, code: KeyCode) -> bool {
    match &input.logical_key {
        Key::Character(c) if c.is_ascii() => c.eq_ignore_ascii_case(letter),
        _ => input.key_code == code,
    }
}

fn logical_key_name(key: &Key) -> String {
    match key {
        Key::Character(value) => value.to_string(),
        Key::Space => " ".to_owned(),
        Key::Dead(_) => "Dead".to_owned(),
        Key::Unidentified(_) => "Unidentified".to_owned(),
        _ => format!("{key:?}"),
    }
}

#[allow(clippy::too_many_arguments)]
fn keyboard_input(
    mut event: On<FocusedInput<KeyboardInput>>,
    mut editors: Query<(&GObjectRef, &mut EditableText, &mut InputEditPolicy)>,
    keys: Res<ButtonInput<Key>>,
    settings: Res<GuiInputSettings>,
    windows: Query<(&Window, Has<PrimaryWindow>)>,
    mut focus: ResMut<InputFocus>,
    mut runtime: NonSendMut<GuiRuntime>,
) {
    if event.original_event_target() != event.focused_entity {
        return;
    }
    let Ok((owner, mut editor, mut policy)) = editors.get_mut(event.focused_entity) else {
        return;
    };
    if !eligible(&runtime, owner.0)
        || !windows.get(event.input.window).is_ok_and(|(w, primary)| {
            w.focused && selected_window(event.input.window, primary, &settings)
        })
    {
        return;
    }
    let kind = if event.input.state == ButtonState::Pressed {
        EventKind::KeyDown
    } else {
        EventKind::KeyUp
    };
    runtime.dispatch_event(UiEvent::new(kind, owner.0).with_data(
        fairygui_core::object::EventData::Key(logical_key_name(&event.input.logical_key)),
    ));
    if editor.is_composing() {
        event.propagate(false);
        return;
    }
    let pressed = event.input.state == ButtonState::Pressed;
    let shift = keys.pressed(Key::Shift);
    let alt = keys.pressed(Key::Alt);
    let ctrl = keys.pressed(Key::Control);
    let super_key = keys.pressed(Key::Super);
    let mac = cfg!(target_os = "macos");
    let command = if mac { super_key } else { ctrl };
    let word = if mac { alt } else { ctrl };
    let key = &event.input.logical_key;
    let enter = matches!(key, Key::Enter) || event.input.key_code == KeyCode::NumpadEnter;
    if enter {
        event.propagate(false);
        if pressed {
            let single = runtime
                .store()
                .object(owner.0)
                .and_then(|o| o.control().as_text_field())
                .is_some_and(|d| d.single_line);
            if single {
                policy.queue_submit(&mut editor);
            } else if editor.allow_newlines && !command && !alt {
                editor.queue_edit(TextEdit::Insert("\n".into()));
            }
        }
        return;
    }
    let edit = if matches!(key, Key::Copy) || command && shortcut(&event.input, "c", KeyCode::KeyC)
    {
        Some(TextEdit::Copy)
    } else if matches!(key, Key::Cut)
        || command && shortcut(&event.input, "x", KeyCode::KeyX)
        || !mac && shift && matches!(key, Key::Delete)
    {
        Some(TextEdit::Cut)
    } else if matches!(key, Key::Paste) || command && shortcut(&event.input, "v", KeyCode::KeyV) {
        Some(TextEdit::Paste)
    } else if command && shortcut(&event.input, "a", KeyCode::KeyA) {
        Some(TextEdit::SelectAll)
    } else {
        match key {
            Key::ArrowLeft if mac && super_key => Some(TextEdit::HardLineStart(shift)),
            Key::ArrowRight if mac && super_key => Some(TextEdit::HardLineEnd(shift)),
            Key::ArrowLeft if word => Some(TextEdit::WordLeft(shift)),
            Key::ArrowRight if word => Some(TextEdit::WordRight(shift)),
            Key::ArrowLeft if !alt && !super_key => Some(TextEdit::Left(shift)),
            Key::ArrowRight if !alt && !super_key => Some(TextEdit::Right(shift)),
            Key::ArrowUp if command && mac => Some(TextEdit::TextStart(shift)),
            Key::ArrowDown if command && mac => Some(TextEdit::TextEnd(shift)),
            Key::ArrowUp if ctrl && !mac => Some(TextEdit::ScrollByLines(-1.)),
            Key::ArrowDown if ctrl && !mac => Some(TextEdit::ScrollByLines(1.)),
            Key::ArrowUp if !alt && !super_key => Some(TextEdit::Up(shift)),
            Key::ArrowDown if !alt && !super_key => Some(TextEdit::Down(shift)),
            Key::Home if command => Some(TextEdit::TextStart(shift)),
            Key::End if command => Some(TextEdit::TextEnd(shift)),
            Key::Home if alt && !mac => Some(TextEdit::HardLineStart(shift)),
            Key::End if alt && !mac => Some(TextEdit::HardLineEnd(shift)),
            Key::Home => Some(TextEdit::LineStart(shift)),
            Key::End => Some(TextEdit::LineEnd(shift)),
            Key::Backspace if word => Some(TextEdit::BackspaceWord),
            Key::Delete if word => Some(TextEdit::DeleteWord),
            Key::Backspace if !alt && !super_key => Some(TextEdit::Backspace),
            Key::Delete if !alt && !super_key => Some(TextEdit::Delete),
            Key::Escape => {
                if pressed {
                    focus.clear();
                }
                Some(TextEdit::CollapseSelection)
            }
            Key::Character(_) | Key::Space | Key::Unidentified(_)
                if !command && !alt && !super_key =>
            {
                event
                    .input
                    .text
                    .as_ref()
                    .filter(|s| !s.is_empty())
                    .map(|s| TextEdit::Insert(s.clone()))
            }
            _ => None,
        }
    };
    if let Some(edit) = edit {
        if !matches!(key, Key::Escape) {
            event.propagate(false);
        }
        if pressed {
            editor.queue_edit(edit);
        }
    }
}

fn ime_input(
    mut events: MessageReader<Ime>,
    focus: Res<InputFocus>,
    settings: Res<GuiInputSettings>,
    windows: Query<(&Window, Has<PrimaryWindow>)>,
    mut editors: Query<(&GObjectRef, &mut EditableText)>,
    runtime: NonSend<GuiRuntime>,
) {
    for event in events.read() {
        let (window, edit) = match event {
            Ime::Preedit {
                window,
                value,
                cursor,
            } => (
                *window,
                TextEdit::ImeSetCompose {
                    value: value.clone().into(),
                    cursor: cursor
                        .map(|(anchor, focus)| bevy::text::PreeditCursor { anchor, focus }),
                },
            ),
            Ime::Commit { window, value } => (
                *window,
                TextEdit::ImeCommit {
                    value: value.clone().into(),
                },
            ),
            Ime::Disabled { window } => (*window, TextEdit::clear_ime_compose()),
            Ime::Enabled { .. } => continue,
        };
        if !windows
            .get(window)
            .is_ok_and(|(w, primary)| w.focused && selected_window(window, primary, &settings))
        {
            continue;
        }
        let Some(entity) = focus.get() else {
            continue;
        };
        let Ok((owner, mut editor)) = editors.get_mut(entity) else {
            continue;
        };
        if eligible(&runtime, owner.0) && runtime.focused_input == Some(owner.0) {
            editor.queue_edit(edit);
        }
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn update_ime_window(
    focus: Res<InputFocus>,
    settings: Res<GuiInputSettings>,
    runtime: NonSend<GuiRuntime>,
    mut previous: ResMut<GuiImeWindow>,
    mut windows: Query<(Entity, &mut Window, Has<PrimaryWindow>)>,
    sources: Query<(
        &GObjectRef,
        &EditableText,
        &ComputedNode,
        &UiGlobalTransform,
        Option<&InputPasswordView>,
    )>,
    layouts: Query<&EditableText>,
    other_inputs: Query<(), With<bevy::ui_widgets::TextInput>>,
) {
    let input = focus
        .get()
        .and_then(|e| sources.get(e).ok())
        .filter(|(o, ..)| runtime.focused_input == Some(o.0) && eligible(&runtime, o.0));
    let active = input.is_some_and(|(o, ..)| matches!(runtime.store().object(o.0).map(|o| o.control()), Some(fairygui_core::widget::ControlData::TextInput(d)) if d.editable));
    for (entity, mut window, primary) in &mut windows {
        if !selected_window(entity, primary, &settings) {
            if previous.0 == Some(entity) {
                window.ime_enabled = false;
            }
            continue;
        }
        if input.is_some() || previous.0 == Some(entity) {
            if input.is_some() || !focus.get().is_some_and(|e| other_inputs.contains(e)) {
                window.ime_enabled = active && window.focused;
            }
            if let Some((_, editor, node, transform, view)) = input {
                let display = view
                    .and_then(|view| layouts.get(view.0).ok())
                    .unwrap_or(editor);
                if display.editor.try_layout().is_some() {
                    let area = display.editor.ime_cursor_area();
                    let local = Vec2::new(area.x0 as f32, area.y1 as f32) + node.content_box().min
                        - editor.viewport.offset;
                    window.ime_position = transform.transform_point2(local) / window.scale_factor();
                }
            }
        }
    }
    previous.0 = if input.is_some() {
        windows
            .iter()
            .find(|(e, _, p)| selected_window(*e, *p, &settings))
            .map(|(e, ..)| e)
    } else {
        None
    };
}

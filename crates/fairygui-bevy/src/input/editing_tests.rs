use super::*;
use crate::runtime;
use bevy::input_focus::{InputDispatchPlugin, InputFocusPlugin};
use fairygui_core::package::FuiPackage;

fn fixture() -> (App, GObjectId, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        bevy::input::InputPlugin,
        InputFocusPlugin,
        InputDispatchPlugin,
    ))
    .init_asset::<crate::FuiAsset>()
    .init_asset::<Image>()
    .init_asset::<Font>()
    .init_resource::<runtime::GuiFont>()
    .init_resource::<bevy::text::FontCx>()
    .init_resource::<bevy::text::LayoutCx>()
    .init_resource::<bevy::clipboard::Clipboard>()
    .init_resource::<bevy::ui::UiScale>()
    .init_resource::<bevy::picking::events::PointerState>()
    .add_message::<bevy::picking::events::PointerRelease>()
    .add_message::<crate::GuiTextInput>();
    app.add_plugins(bevy::ui_widgets::TextInputPlugin);
    let window = app
        .world_mut()
        .spawn((
            Window {
                focused: true,
                ..default()
            },
            PrimaryWindow,
        ))
        .id();
    let package = FuiPackage::parse(
        "Basics",
        include_bytes!("../../../fairygui-core/tests/fixtures/Basics.fui"),
    )
    .unwrap();
    let handle = app
        .world_mut()
        .resource_mut::<Assets<crate::FuiAsset>>()
        .add(crate::FuiAsset {
            package: package.clone(),
            res_key: "ui/Basics".into(),
        });
    let mut runtime = GuiRuntime::default();
    let index = runtime.create_root(handle, &package, "Demo_Text").unwrap();
    let root = runtime.root_object(index).unwrap();
    let owner = runtime.store().child_by_name(root, "n22").unwrap();
    runtime.store_mut().set_object_text(owner, "");
    runtime.store_mut().set_object_ubb_enabled(owner, false);
    runtime.store_mut().set_object_font(owner, None);
    runtime.store_mut().set_object_single_line(owner, true);
    app.insert_non_send(runtime)
        .configure_sets(
            Update,
            (
                crate::GuiSystems::Input,
                crate::GuiSystems::Publish,
                crate::GuiSystems::Respond,
                crate::GuiSystems::Sync,
            )
                .chain(),
        )
        .add_systems(
            Update,
            runtime::emit_text_input_messages.in_set(crate::GuiSystems::Publish),
        )
        .add_systems(
            Update,
            (
                runtime::spawn_pending_roots,
                runtime::spawn_pending_objects,
                runtime::sync_text_inputs,
                runtime::sync_dirty,
                runtime::sync_text_fonts,
                runtime::sync_input_policy,
                runtime::sync_password_views,
                runtime::sync_stage_hierarchy,
                reconcile_focus,
            )
                .chain()
                .in_set(crate::GuiSystems::Sync),
        )
        .add_systems(
            PostUpdate,
            (runtime::apply_input_edits, runtime::sync_input_cursors).chain(),
        );
    crate::input::register(&mut app);
    app.update();
    app.update();
    let entity = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(owner)
        .unwrap();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .drain_events()
        .for_each(drop);
    (app, owner, entity, window)
}
fn key(app: &mut App, window: Entity, key: Key, code: KeyCode, value: Option<&str>) {
    key_state(app, window, key, code, value, ButtonState::Pressed);
}

fn key_state(
    app: &mut App,
    window: Entity,
    key: Key,
    code: KeyCode,
    value: Option<&str>,
    state: ButtonState,
) {
    app.world_mut().write_message(KeyboardInput {
        key_code: code,
        logical_key: key,
        text: value.map(Into::into),
        state,
        repeat: false,
        window,
    });
    app.update();
}

#[test]
fn focused_keyboard_down_and_up_events_preserve_key_and_bubble_once() {
    let (mut app, owner, _entity, window) = fixture();
    let parent = app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .object(owner)
        .unwrap()
        .parent()
        .unwrap();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .request_input_focus(Some(owner));
    app.update();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .drain_events()
        .for_each(drop);

    key(
        &mut app,
        window,
        Key::Character("A".into()),
        KeyCode::KeyA,
        Some("A"),
    );
    let down: Vec<_> = app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .drain_events()
        .filter(|event| event.kind == EventKind::KeyDown)
        .collect();
    assert_eq!(down.len(), 2, "one target event and one parent bubble");
    assert_eq!(down[0].target, owner);
    assert_eq!(down[0].current_target, owner);
    assert!(!down[0].bubbling);
    assert_eq!(
        down[0].data,
        fairygui_core::object::EventData::Key("A".into())
    );
    assert_eq!(down[1].target, owner);
    assert_eq!(down[1].current_target, parent);
    assert!(down[1].bubbling);
    assert_eq!(down[1].data, down[0].data);

    key_state(
        &mut app,
        window,
        Key::Character("A".into()),
        KeyCode::KeyA,
        None,
        ButtonState::Released,
    );
    let up: Vec<_> = app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .drain_events()
        .filter(|event| event.kind == EventKind::KeyUp)
        .collect();
    assert_eq!(up.len(), 2, "one target event and one parent bubble");
    assert_eq!(
        up[0].data,
        fairygui_core::object::EventData::Key("A".into())
    );
    assert_eq!(up[1].current_target, parent);
}
fn events(app: &mut App) -> Vec<EventKind> {
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .drain_events()
        .filter(|e| {
            matches!(
                e.kind,
                EventKind::Input
                    | EventKind::Change
                    | EventKind::Enter
                    | EventKind::FocusIn
                    | EventKind::FocusOut
            )
        })
        .map(|e| {
            assert!(!e.bubbling);
            e.kind
        })
        .collect()
}
fn value(app: &App, entity: Entity) -> String {
    app.world()
        .get::<EditableText>(entity)
        .unwrap()
        .value()
        .to_string()
}

#[test]
fn actual_keyboard_dispatch_updates_real_value_and_native_input_enter_change_blur_order() {
    let (mut app, owner, entity, window) = fixture();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .request_input_focus(Some(owner));
    app.update();
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(entity));
    assert_eq!(events(&mut app), [EventKind::FocusIn]);
    assert!(
        app.world()
            .resource::<GuiInputState>()
            .wants_keyboard_input()
    );
    key(
        &mut app,
        window,
        Key::Character("A".into()),
        KeyCode::KeyA,
        Some("A"),
    );
    assert_eq!(value(&app, entity), "A");
    assert_eq!(events(&mut app), [EventKind::Input]);
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .object(owner)
            .unwrap()
            .control()
            .as_text_field()
            .unwrap()
            .text,
        "A"
    );
    key(&mut app, window, Key::Enter, KeyCode::Enter, Some("\r"));
    assert_eq!(
        events(&mut app),
        [EventKind::Enter, EventKind::Change, EventKind::FocusOut]
    );
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .focused_input()
            .is_none()
    );
    assert!(
        !app.world()
            .resource::<GuiInputState>()
            .wants_keyboard_input()
    );
    key(
        &mut app,
        window,
        Key::Character("B".into()),
        KeyCode::KeyB,
        Some("B"),
    );
    assert_eq!(value(&app, entity), "A");
    assert!(events(&mut app).is_empty());
}

#[test]
fn ime_messages_filter_commit_and_focus_loss_readonly_and_programmatic_changes_follow_native_events()
 {
    let (mut app, owner, entity, window) = fixture();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .request_input_focus(Some(owner));
    app.update();
    events(&mut app);
    app.world_mut().write_message(Ime::Preedit {
        window,
        value: "预编辑".into(),
        cursor: None,
    });
    app.update();
    assert!(
        app.world()
            .get::<EditableText>(entity)
            .unwrap()
            .is_composing()
    );
    assert_eq!(value(&app, entity), "");
    assert!(events(&mut app).is_empty());
    app.world_mut().write_message(Ime::Commit {
        window,
        value: "提交😀".into(),
    });
    app.update();
    assert_eq!(value(&app, entity), "提交😀");
    assert_eq!(events(&mut app), [EventKind::Input]);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text(owner, "Assigned");
    app.update();
    assert_eq!(events(&mut app), [EventKind::Change]);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_input_editable(owner, false);
    app.update();
    key(
        &mut app,
        window,
        Key::Character("X".into()),
        KeyCode::KeyX,
        Some("X"),
    );
    assert_eq!(value(&app, entity), "Assigned");
    assert!(events(&mut app).is_empty());
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .request_input_focus(None);
    app.update();
    assert_eq!(events(&mut app), [EventKind::FocusOut]);
}

fn prevent_submit(
    mut messages: MessageReader<crate::GuiTextInput>,
    mut runtime: NonSendMut<GuiRuntime>,
) {
    for message in messages.read() {
        if message.event.kind == EventKind::Enter {
            assert!(runtime.prevent_input_submit(message.event.target));
        }
    }
}

#[test]
fn executable_native_session_traces_match_real_keyboard_focus_and_edit_notifications() {
    let mut count = 0;
    for line in include_str!("../../../fairygui-core/tests/fixtures/input-events-reference.txt")
        .lines()
        .filter(|s| s.starts_with("case "))
    {
        let fields: Vec<_> = line.split_whitespace().collect();
        let name = fields[1];
        let (mut app, owner, entity, window) = fixture();
        {
            let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
            if name.starts_with("readonly") {
                runtime.store_mut().set_input_editable(owner, false);
            }
            if name == "rejected" {
                runtime
                    .store_mut()
                    .set_input_restrict(owner, Some("0-9"))
                    .unwrap();
            }
            if name == "multiline_enter" {
                runtime.store_mut().set_object_single_line(owner, false);
            }
            runtime.request_input_focus(Some(owner));
        }
        if name == "prevent_enter" {
            app.add_systems(Update, prevent_submit.in_set(crate::GuiSystems::Respond));
        }
        app.update();
        match name {
            "insert" | "duplicate_input" => {
                key(
                    &mut app,
                    window,
                    Key::Character("A".into()),
                    KeyCode::KeyA,
                    Some("A"),
                );
                if name == "duplicate_input" {
                    app.world_mut()
                        .get_mut::<EditableText>(entity)
                        .unwrap()
                        .queue_edit(TextEdit::SelectAll);
                    key(
                        &mut app,
                        window,
                        Key::Character("A".into()),
                        KeyCode::KeyA,
                        Some("A"),
                    );
                }
            }
            "rejected" => key(
                &mut app,
                window,
                Key::Character("abc".into()),
                KeyCode::KeyA,
                Some("abc"),
            ),
            "enter" | "readonly_enter" | "multiline_enter" | "prevent_enter" => {
                key(&mut app, window, Key::Enter, KeyCode::Enter, None)
            }
            "numpad" => key(&mut app, window, Key::Enter, KeyCode::NumpadEnter, None),
            _ => {}
        }
        if name != "prevent_enter" {
            app.world_mut()
                .non_send_mut::<GuiRuntime>()
                .request_input_focus(None);
            app.update();
        }
        let actual = events(&mut app)
            .iter()
            .map(|kind| match kind {
                EventKind::FocusIn => "focus",
                EventKind::FocusOut => "blur",
                EventKind::Input => "input",
                EventKind::Change => "change",
                EventKind::Enter => "enter",
                _ => unreachable!(),
            })
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(actual, fields[2], "{line}");
        let expected = if fields[3] == "_" {
            String::new()
        } else {
            String::from_utf8(
                fields[3]
                    .as_bytes()
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|v| u8::from_str_radix(std::str::from_utf8(v).unwrap(), 16).unwrap())
                    .collect(),
            )
            .unwrap()
        };
        assert_eq!(value(&app, entity), expected, "{line}");
        count += 1;
    }
    assert_eq!(count, 10);
}

#[test]
fn focus_policy_cancels_preedit_and_stale_window_input_and_hiding_clears_ime_once() {
    let (mut app, owner, entity, window) = fixture();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .request_input_focus(Some(owner));
    app.update();
    events(&mut app);
    assert!(app.world().get::<Window>(window).unwrap().ime_enabled);
    let foreign = app.world_mut().spawn(Window::default()).id();
    key(
        &mut app,
        foreign,
        Key::Character("X".into()),
        KeyCode::KeyX,
        Some("X"),
    );
    assert_eq!(value(&app, entity), "");
    app.world_mut().write_message(Ime::Preedit {
        window,
        value: "pending".into(),
        cursor: None,
    });
    app.update();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_visible(owner, false);
    key(
        &mut app,
        window,
        Key::Character("X".into()),
        KeyCode::KeyX,
        Some("X"),
    );
    assert_eq!(value(&app, entity), "");
    assert!(
        !app.world()
            .get::<EditableText>(entity)
            .unwrap()
            .is_composing()
    );
    assert_eq!(events(&mut app), [EventKind::Change, EventKind::FocusOut]);
    assert!(!app.world().get::<Window>(window).unwrap().ime_enabled);
    app.update();
    assert!(events(&mut app).is_empty());
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .request_input_focus(Some(owner));
    app.update();
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .focused_input()
            .is_none()
    );
}

#[test]
fn full_widget_requirements_do_not_turn_password_layout_children_into_ui_or_input_targets() {
    let (mut app, owner, entity, _) = fixture();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_input_password(owner, true);
    app.update();
    let child = app
        .world()
        .get::<runtime::InputPasswordView>(entity)
        .unwrap()
        .0;
    assert!(app.world().get::<Node>(child).is_none());
    assert!(app.world().get::<ComputedNode>(child).is_none());
    assert!(
        app.world()
            .get::<bevy::ui_widgets::TextInput>(child)
            .is_none()
    );
    assert_eq!(
        app.world().get::<bevy::picking::Pickable>(child),
        Some(&bevy::picking::Pickable::IGNORE)
    );
}

#[test]
fn clicking_active_input_keeps_its_session_and_window_blur_ends_it_without_stale_input() {
    let (mut app, owner, entity, window) = fixture();
    app.add_plugins(bevy::input_focus::pointer_focus::PointerFocusPlugin);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .request_input_focus(Some(owner));
    app.update();
    events(&mut app);
    app.world_mut().trigger(bevy::input_focus::AcquireFocus {
        focused_entity: entity,
        window,
    });
    app.update();
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(entity));
    assert!(events(&mut app).is_empty());
    app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
    key(
        &mut app,
        window,
        Key::Character("X".into()),
        KeyCode::KeyX,
        Some("X"),
    );
    assert_eq!(value(&app, entity), "");
    assert_eq!(events(&mut app), [EventKind::Change, EventKind::FocusOut]);
    assert!(!app.world().get::<Window>(window).unwrap().ime_enabled);
    app.world_mut().get_mut::<Window>(window).unwrap().focused = true;
    app.update();
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .focused_input()
            .is_none()
    );
}

#[test]
fn same_frame_policy_edits_apply_before_keyboard_or_clipboard_queues_are_consumed() {
    let (mut app, owner, entity, window) = fixture();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .request_input_focus(Some(owner));
    app.update();
    events(&mut app);
    {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        runtime.store_mut().set_input_max_length(owner, 6);
        runtime
            .store_mut()
            .set_input_restrict(owner, Some("0-9"))
            .unwrap();
    }
    app.world_mut()
        .resource_mut::<bevy::clipboard::Clipboard>()
        .set_text("a9b8C7")
        .unwrap();
    app.world_mut()
        .get_mut::<EditableText>(entity)
        .unwrap()
        .queue_edit(TextEdit::Paste);
    app.update();
    assert_eq!(value(&app, entity), "987");
    assert_eq!(events(&mut app), [EventKind::Input]);
    {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        runtime.store_mut().set_input_restrict(owner, None).unwrap();
        runtime.store_mut().set_input_max_length(owner, 0);
    }
    key(
        &mut app,
        window,
        Key::Character("X".into()),
        KeyCode::KeyX,
        Some("X"),
    );
    assert_eq!(value(&app, entity), "987X");
}

#[test]
fn clearing_unobserved_external_gui_focus_does_not_acquire_a_new_session_or_clear_other_controls() {
    let (mut app, _, entity, window) = fixture();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(entity, FocusCause::Navigated);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .request_input_focus(None);
    app.update();
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .focused_input()
            .is_none()
    );
    assert!(events(&mut app).is_empty());
    assert_ne!(app.world().resource::<InputFocus>().get(), Some(entity));
    let other = app.world_mut().spawn_empty().id();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(other, FocusCause::Navigated);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .request_input_focus(None);
    app.update();
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(other));
    assert_ne!(other, window);
}

#[test]
fn caret_and_selection_paint_follow_effective_text_color_opacity_and_logical_pixel_width() {
    use bevy::color::Alpha;
    let (mut app, _, entity, _) = fixture();
    let color = Color::srgba(0.2, 0.3, 0.5, 0.25);
    app.world_mut().get_mut::<TextColor>(entity).unwrap().0 = color;
    app.update();
    let style = app
        .world()
        .get::<bevy::text::TextCursorStyle>(entity)
        .unwrap();
    assert_eq!(style.color, color);
    assert_eq!(style.selection_color.alpha(), 0.25);
    assert_eq!(style.unfocused_selection_color.alpha(), 0.25);
    let size = app
        .world()
        .get::<TextFont>(entity)
        .unwrap()
        .font_size
        .eval(Vec2::ZERO, default());
    let width = app
        .world()
        .get::<EditableText>(entity)
        .unwrap()
        .cursor_width
        * size;
    assert!((width - 1.).abs() < 0.001);
    app.world_mut().get_mut::<TextColor>(entity).unwrap().0 = Color::NONE;
    app.update();
    assert_eq!(
        app.world()
            .get::<bevy::text::TextCursorStyle>(entity)
            .unwrap()
            .selection_color
            .alpha(),
        0.
    );
}

use super::*;
use crate::runtime;
use bevy::ecs::system::SystemState;
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
            (runtime::emit_gui_events, runtime::emit_text_input_messages)
                .chain()
                .in_set(crate::GuiSystems::Publish),
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
                // This fixture installs the focus bridge without FairyGuiPlugin;
                // mirror its end-of-Sync entity snapshot publication too.
                crate::input::sync_input_state,
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
fn touch_press_focuses_the_native_editor_without_a_mouse_cursor() {
    use bevy::input::touch::{TouchInput, TouchPhase};
    use bevy::picking::{backend::HitData, hover::HoverMap, pointer::PointerId};
    let (mut app, _, entity, window) = fixture();
    app.init_resource::<HoverMap>();
    app.world_mut().resource_mut::<HoverMap>().insert(
        PointerId::Touch(8),
        [(
            entity,
            HitData::new(Entity::PLACEHOLDER, 0.0, Some(Vec3::ZERO), None),
        )]
        .into_iter()
        .collect(),
    );
    app.world_mut().write_message(TouchInput {
        id: 8,
        phase: TouchPhase::Started,
        window,
        position: Vec2::splat(10.0),
        force: None,
    });
    app.update();
    assert_eq!(
        app.world().resource::<GuiInputState>().focused_entity(),
        Some(entity)
    );
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(entity));
    key(
        &mut app,
        window,
        Key::Character("A".into()),
        KeyCode::KeyA,
        Some("A"),
    );
    assert_eq!(value(&app, entity), "A");
    app.world_mut().write_message(TouchInput {
        id: 8,
        phase: TouchPhase::Ended,
        window,
        position: Vec2::splat(10.0),
        force: None,
    });
    app.update();
    assert_eq!(
        app.world().resource::<GuiInputState>().focused_entity(),
        Some(entity)
    );
    assert_eq!(
        app.world().resource::<GuiInputState>().captured_touch(8),
        None
    );
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

fn with_input(app: &mut App, edit: impl FnOnce(&mut GuiInput)) {
    let mut state = SystemState::<GuiInput>::new(app.world_mut());
    edit(&mut state.get_mut(app.world_mut()).unwrap());
}
fn focus(app: &mut App, entity: Entity) -> bool {
    let mut result = false;
    with_input(app, |input| result = input.focus(entity));
    result
}
fn blur(app: &mut App) {
    with_input(app, |input| input.blur());
}
fn prevent_submit(event: On<crate::GuiEnter>, mut input: GuiInput) {
    assert!(input.prevent_submit(event.entity));
}

#[test]
fn input_entity_observers_receive_committed_password_value_and_can_cancel_enter_before_sync() {
    #[derive(Resource, Default)]
    struct Seen(Vec<(Entity, EventKind, String)>);
    fn observe(event: On<crate::GuiTextInput>, mut seen: ResMut<Seen>) {
        seen.0
            .push((event.entity, event.event.kind.clone(), event.value.clone()));
    }
    let (mut app, owner, view, window) = fixture();
    app.init_resource::<Seen>();
    app.world_mut()
        .entity_mut(view)
        .observe(observe)
        .observe(prevent_submit);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_input_password(owner, true);
    focus(&mut app, view);
    app.update();
    key(
        &mut app,
        window,
        Key::Character("secret".into()),
        KeyCode::KeyS,
        Some("secret"),
    );
    app.update();
    key(&mut app, window, Key::Enter, KeyCode::Enter, None);
    app.update();
    assert!(
        app.world()
            .resource::<Seen>()
            .0
            .iter()
            .any(|(entity, kind, value)| *entity == view
                && *kind == EventKind::Enter
                && value == "secret")
    );
    assert_eq!(
        app.world().non_send::<GuiRuntime>().focused_input(),
        Some(owner)
    );
    assert!(
        !app.world()
            .resource::<Seen>()
            .0
            .iter()
            .any(|(_, kind, _)| *kind == EventKind::FocusOut)
    );
}

#[test]
fn direct_focus_and_blur_update_bevy_and_native_sessions_without_waiting_for_sync() {
    let (mut app, owner, view, _) = fixture();
    with_input(&mut app, |input| {
        assert!(input.focus(view));
        assert_eq!(input.focused_entity(), Some(view));
        assert!(!input.prevent_submit(view));
    });
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(view));
    assert_eq!(
        app.world().non_send::<GuiRuntime>().focused_input(),
        Some(owner)
    );
    assert_eq!(events(&mut app), [EventKind::FocusIn]);
    let unrelated = app.world_mut().spawn_empty().id();
    with_input(&mut app, |input| {
        assert!(!input.focus(unrelated));
        assert!(!input.cancel_scroll_drag(view));
        assert_eq!(input.focused_entity(), Some(view));
        input.blur();
        assert_eq!(input.focused_entity(), None);
    });
    assert_eq!(app.world().resource::<InputFocus>().get(), None);
    assert_eq!(app.world().non_send::<GuiRuntime>().focused_input(), None);
    assert_eq!(events(&mut app), [EventKind::Change, EventKind::FocusOut]);
    app.update();
    assert_eq!(
        app.world().resource::<GuiInputState>().focused_entity(),
        None
    );
}

#[test]
fn direct_focus_rejects_destroyed_or_unready_views_without_leaving_a_request() {
    let (mut app, _, view, _) = fixture();
    let pending = app
        .world()
        .resource::<Assets<crate::FuiAsset>>()
        .reserve_handle();
    let owner = app
        .world_mut()
        .spawn(crate::FairyGuiRoot::new(pending, "Input"))
        .id();
    with_input(&mut app, |input| {
        assert!(!input.focus(owner));
        assert_eq!(input.focused_entity(), None);
    });
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .input_focus_request
            .is_none()
    );
    app.world_mut().despawn(view);
    with_input(&mut app, |input| {
        assert!(!input.focus(view));
        assert!(!input.prevent_submit(view));
    });
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .input_focus_request
            .is_none()
    );
}

#[test]
fn direct_input_resolves_compound_views_and_preserves_other_bevy_focus() {
    let (mut app, input, view, _) = fixture();
    let (root, wrapper) = {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        let root = runtime.store().object(input).unwrap().parent().unwrap();
        let wrapper = runtime
            .store_mut()
            .create(fairygui_core::package::ObjectType::Label);
        runtime.store_mut().set_parent(wrapper, Some(root));
        runtime.store_mut().set_name(input, "title");
        runtime.store_mut().set_parent(input, Some(wrapper));
        runtime.queue_subtree(wrapper);
        (root, wrapper)
    };
    app.update();
    let owner = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(wrapper)
        .unwrap();
    focus(&mut app, owner);
    app.update();
    assert_eq!(
        app.world().resource::<GuiInputState>().focused_entity(),
        Some(view)
    );
    assert_eq!(events(&mut app), [EventKind::FocusIn]);
    app.update();
    let notifications: Vec<_> = app
        .world_mut()
        .resource_mut::<Messages<crate::GuiTextInput>>()
        .drain()
        .collect();
    assert!(notifications.iter().any(|event| event.entity == view
        && event.event.target == input
        && event.event.kind == EventKind::FocusIn));

    // A non-input target is discarded; it cannot replace an eligible request.
    let root_view = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(root)
        .unwrap();
    blur(&mut app);
    focus(&mut app, view);
    focus(&mut app, root_view);
    app.update();
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(view));
    assert_eq!(
        events(&mut app),
        [EventKind::Change, EventKind::FocusOut, EventKind::FocusIn]
    );
    focus(&mut app, owner);
    blur(&mut app);
    app.update();
    assert_eq!(
        app.world().resource::<GuiInputState>().focused_entity(),
        None
    );
    assert_eq!(events(&mut app), [EventKind::Change, EventKind::FocusOut]);
    let other = app.world_mut().spawn_empty().id();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(other, FocusCause::Navigated);
    blur(&mut app);
    app.update();
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(other));

    // External focus can move in Respond, before Sync has ended the old GUI
    // session. Blur must inspect current Bevy focus instead of the old session.
    focus(&mut app, view);
    app.update();
    events(&mut app);
    app.insert_resource(ExternalFocus(other));
    app.add_systems(Update, hand_off_focus.in_set(crate::GuiSystems::Respond));
    app.update();
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(other));
    assert_eq!(
        app.world().resource::<GuiInputState>().focused_entity(),
        None
    );
    assert_eq!(events(&mut app), [EventKind::Change, EventKind::FocusOut]);
}

#[derive(Resource)]
struct ExternalFocus(Entity);

fn hand_off_focus(target: Res<ExternalFocus>, mut input: ParamSet<(ResMut<InputFocus>, GuiInput)>) {
    input.p0().set(target.0, FocusCause::Navigated);
    input.p1().blur();
}

#[test]
fn direct_input_cancels_enter_and_ignores_missing_or_hidden_targets() {
    let (mut app, input, view, window) = fixture();
    app.add_observer(prevent_submit);
    focus(&mut app, view);
    app.update();
    events(&mut app);
    key(
        &mut app,
        window,
        Key::Character("ECS".into()),
        KeyCode::KeyE,
        Some("ECS"),
    );
    assert_eq!(value(&app, view), "ECS");
    events(&mut app);
    key(&mut app, window, Key::Enter, KeyCode::Enter, None);
    assert_eq!(events(&mut app), [EventKind::Enter]);
    assert_eq!(
        app.world().resource::<GuiInputState>().focused_entity(),
        Some(view)
    );
    app.update();
    assert_eq!(
        app.world().resource::<GuiInputState>().focused_entity(),
        Some(view)
    );
    let stale = app.world_mut().spawn_empty().id();
    app.world_mut().despawn(stale);
    focus(&mut app, stale);
    with_input(&mut app, |input| {
        assert!(!input.prevent_submit(stale));
    });
    app.update();
    assert_eq!(
        app.world().resource::<GuiInputState>().focused_entity(),
        Some(view)
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_visible(input, false);
    focus(&mut app, view);
    app.update();
    assert_eq!(
        app.world().resource::<GuiInputState>().focused_entity(),
        None
    );
    assert!(!app.world().get::<Window>(window).unwrap().ime_enabled);
    assert_eq!(events(&mut app), [EventKind::Change, EventKind::FocusOut]);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_visible(input, true);
    app.update();
    assert_eq!(
        app.world().resource::<GuiInputState>().focused_entity(),
        None
    );
}

#[test]
fn entity_input_focus_obeys_native_window_modality_and_does_not_retry_rejected_requests() {
    let (mut app, _, view, _) = fixture();
    let package = FuiPackage::parse(
        "Basics",
        include_bytes!("../../../fairygui-core/tests/fixtures/Basics.fui"),
    )
    .unwrap();
    let modal = app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .show_window(
            &package,
            "WindowA",
            true,
            fairygui_core::Size::new(1280., 720.),
        )
        .unwrap();
    focus(&mut app, view);
    app.update();
    assert_eq!(
        app.world().resource::<GuiInputState>().focused_entity(),
        None
    );
    assert!(events(&mut app).is_empty());
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .hide_window(modal);
    app.update();
    assert_eq!(
        app.world().resource::<GuiInputState>().focused_entity(),
        None
    );
    focus(&mut app, view);
    app.update();
    assert_eq!(
        app.world().resource::<GuiInputState>().focused_entity(),
        Some(view)
    );
    assert_eq!(events(&mut app), [EventKind::FocusIn]);
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
            app.add_observer(prevent_submit);
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

#[test]
fn mouse_drag_stops_editing_when_focus_moves_to_another_control() {
    use bevy::ecs::system::RunSystemOnce;
    let (mut app, owner, entity, window) = fixture();
    let other = app.world_mut().spawn_empty().id();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(other, FocusCause::Navigated);
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(Vec2::new(40., 20.)));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.world_mut().resource_mut::<InputRouter>().capture = Some(crate::input::Capture {
        window,
        target: owner,
        start: Vec2::ZERO,
        last: Vec2::ZERO,
        moved: true,
        dragged: true,
        drop_started: false,
        gesture: Gesture::None,
        link: None,
        editing: Some(entity),
    });
    app.world_mut()
        .get_mut::<EditableText>(entity)
        .unwrap()
        .pending_edits
        .clear();
    app.world_mut()
        .run_system_once(crate::input::route_pointer)
        .unwrap();
    assert!(
        app.world()
            .get::<EditableText>(entity)
            .unwrap()
            .pending_edits
            .is_empty(),
        "a captured mouse must not restore selection after focus was lost"
    );
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(other));
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(entity, FocusCause::Navigated);
    app.world_mut()
        .run_system_once(crate::input::route_pointer)
        .unwrap();
    assert!(
        app.world()
            .get::<EditableText>(entity)
            .unwrap()
            .pending_edits
            .is_empty(),
        "regaining focus must not restart an earlier pointer's drag selection"
    );
}

#[test]
fn explicit_blur_cancels_mouse_and_touch_selection_owners_before_refocus() {
    let (mut app, owner, entity, window) = fixture();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .request_input_focus(Some(owner));
    app.update();
    {
        let mut state = app.world_mut().resource_mut::<InputRouter>();
        state.capture = Some(crate::input::Capture {
            window,
            target: owner,
            start: Vec2::ZERO,
            last: Vec2::ZERO,
            moved: true,
            dragged: true,
            drop_started: false,
            gesture: Gesture::None,
            link: None,
            editing: Some(entity),
        });
        state.touches.insert(
            8,
            crate::input::TouchCapture {
                window,
                target: owner,
                start: Vec2::ZERO,
                last: Vec2::ZERO,
                moved: true,
                dragged: true,
                drop_started: false,
                gesture: Gesture::None,
                link: None,
                editing: Some(entity),
                hovered: Some(owner),
            },
        );
        state.text_press = Some((entity, Vec2::ZERO, 0., 2));
    }
    let mut services = SystemState::<GuiInput>::new(app.world_mut());
    services.get_mut(app.world_mut()).unwrap().blur();
    {
        let state = app.world().resource::<InputRouter>();
        assert!(state.capture.as_ref().unwrap().editing.is_none());
        assert!(state.touches[&8].editing.is_none());
        assert!(state.text_press.is_none());
    }
    assert!(services.get_mut(app.world_mut()).unwrap().focus(entity));
    let state = app.world().resource::<InputRouter>();
    assert!(state.capture.as_ref().unwrap().editing.is_none());
    assert!(state.touches[&8].editing.is_none());
}

#[test]
fn selection_autoscroll_uses_captured_mouse_or_touch_and_stops_at_bounds_and_reentry() {
    use bevy::ecs::system::RunSystemOnce;
    let (mut app, owner, entity, window) = fixture();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(entity, FocusCause::Navigated);
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .resolution
        .set_scale_factor_override(Some(2.));
    app.world_mut().entity_mut(entity).insert((
        ComputedNode {
            size: Vec2::new(200., 100.),
            ..default()
        },
        UiGlobalTransform::from(bevy::math::Affine2::from_translation(Vec2::new(300., 200.))),
        bevy::text::TextLayoutInfo {
            size: Vec2::new(1200., 800.),
            ..default()
        },
    ));
    {
        let mut editor = app.world_mut().get_mut::<EditableText>(entity).unwrap();
        editor.viewport.size = Vec2::new(200., 100.);
        editor.viewport.offset = Vec2::ZERO;
        editor.pending_edits.clear();
    }
    app.world_mut()
        .resource_mut::<Time<Real>>()
        .advance_by(std::time::Duration::from_secs_f32(0.1));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.world_mut().resource_mut::<InputRouter>().capture = Some(crate::input::Capture {
        window,
        target: owner,
        start: Vec2::ZERO,
        last: Vec2::new(250., 160.),
        moved: true,
        dragged: true,
        drop_started: false,
        gesture: Gesture::None,
        link: None,
        editing: Some(entity),
    });
    // A generic picking scroll queued earlier must not apply a second step.
    app.world_mut()
        .get_mut::<EditableText>(entity)
        .unwrap()
        .pending_edits
        .push(TextEdit::ScrollBy(Vec2::splat(999.)));
    app.world_mut()
        .run_system_once(crate::input::text_selection::autoscroll)
        .unwrap();
    let editor = app.world().get::<EditableText>(entity).unwrap();
    assert!(matches!(editor.pending_edits.as_slice(),[
        TextEdit::ScrollBy(delta),TextEdit::ExtendSelectionToPoint(point)
    ] if delta.abs_diff_eq(Vec2::new(40.,20.),0.001) && point.abs_diff_eq(Vec2::new(240.,120.),0.001)));
    // Reentry retains the current scroll and resolves inside the visible text.
    app.world_mut()
        .resource_mut::<InputRouter>()
        .capture
        .as_mut()
        .unwrap()
        .last = Vec2::new(150., 100.);
    app.world_mut()
        .get_mut::<EditableText>(entity)
        .unwrap()
        .viewport
        .offset = Vec2::new(40., 20.);
    app.world_mut()
        .run_system_once(crate::input::text_selection::autoscroll)
        .unwrap();
    assert!(
        matches!(app.world().get::<EditableText>(entity).unwrap().pending_edits.as_slice(),[
        TextEdit::ExtendSelectionToPoint(point)
    ] if point.abs_diff_eq(Vec2::new(140.,70.),0.001))
    );
    // A held touch continues scrolling even without new TouchInput messages.
    app.world_mut().resource_mut::<InputRouter>().capture = None;
    app.world_mut()
        .resource_mut::<InputRouter>()
        .touches
        .insert(
            5,
            crate::input::TouchCapture {
                window,
                target: owner,
                start: Vec2::ZERO,
                last: Vec2::new(50., 40.),
                moved: true,
                dragged: true,
                drop_started: false,
                gesture: Gesture::None,
                link: None,
                editing: Some(entity),
                hovered: None,
            },
        );
    app.world_mut()
        .run_system_once(crate::input::text_selection::autoscroll)
        .unwrap();
    assert!(
        matches!(app.world().get::<EditableText>(entity).unwrap().pending_edits.as_slice(),[
        TextEdit::ScrollBy(delta),TextEdit::ExtendSelectionToPoint(point)
    ] if delta.abs_diff_eq(Vec2::new(-40.,-20.),0.001) && *point==Vec2::ZERO)
    );
    app.world_mut()
        .get_mut::<EditableText>(entity)
        .unwrap()
        .viewport
        .offset = Vec2::ZERO;
    app.world_mut()
        .run_system_once(crate::input::text_selection::autoscroll)
        .unwrap();
    assert!(
        matches!(app.world().get::<EditableText>(entity).unwrap().pending_edits.as_slice(),[
        TextEdit::ExtendSelectionToPoint(point)
    ] if *point==Vec2::ZERO)
    );
    app.world_mut()
        .get_mut::<EditableText>(entity)
        .unwrap()
        .pending_edits
        .clear();
    app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
    app.world_mut()
        .run_system_once(crate::input::text_selection::autoscroll)
        .unwrap();
    assert!(
        app.world()
            .get::<EditableText>(entity)
            .unwrap()
            .pending_edits
            .is_empty()
    );
}

#[test]
fn direct_selection_uses_utf16_preserves_direction_and_syncs_new_model_text() {
    let (mut app, owner, view, _) = fixture();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text(owner, "A中😀e\u{301}Z");
    with_input(&mut app, |input| {
        assert!(input.set_selection(view, GuiTextSelection::new(4, 1)));
        assert_eq!(input.focused_entity(), Some(view));
        assert_eq!(input.selection(view), Some(GuiTextSelection::new(4, 1)));
        assert_eq!(input.selected_text(view), Some("中😀"));
        assert!(input.set_caret_position(view, 3));
        assert_eq!(input.selection(view), Some(GuiTextSelection::caret(2)));
        assert!(input.selected_text(view).is_none());
        assert!(input.select_all(view));
        assert_eq!(input.selection(view).unwrap().range(), 0..7);
        assert_eq!(input.selected_text(view), Some("A中😀e\u{301}Z"));
    });
    assert_eq!(events(&mut app), [EventKind::FocusIn]);
    for _ in 0..3 {
        app.update();
        with_input(&mut app, |input| {
            assert_eq!(input.selection(view), Some(GuiTextSelection::new(0, 7)))
        });
    }
    with_input(&mut app, |input| {
        input.blur();
        assert!(input.set_selection(view, GuiTextSelection::new(4, 1)));
    });
    app.update();
    with_input(&mut app, |input| {
        assert_eq!(input.selected_text(view), Some("中😀"))
    });
    assert_eq!(value(&app, view), "A中😀e\u{301}Z");
}

#[test]
fn direct_selection_supersedes_old_navigation_but_keeps_pending_text_replacement() {
    let (mut app, owner, view, _) = fixture();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text(owner, "abcde");
    with_input(&mut app, |input| assert!(input.select_all(view)));
    events(&mut app);
    app.world_mut()
        .get_mut::<EditableText>(view)
        .unwrap()
        .pending_edits
        .extend([
            TextEdit::TextStart(false),
            TextEdit::CollapseSelection,
            TextEdit::Insert("X".into()),
        ]);
    with_input(&mut app, |input| {
        assert!(input.set_selection(view, GuiTextSelection::new(1, 4)))
    });
    app.update();
    assert_eq!(value(&app, view), "aXe");
    with_input(&mut app, |input| {
        assert_eq!(input.selection(view), Some(GuiTextSelection::caret(2)))
    });
    assert_eq!(events(&mut app), [EventKind::Input]);
}

#[test]
fn direct_selection_preserves_active_and_pending_ime_and_rejects_ineligible_targets() {
    let (mut app, owner, view, window) = fixture();
    let missing = app.world_mut().spawn_empty().id();
    with_input(&mut app, |input| {
        assert!(!input.select_all(missing));
        assert!(input.selection(missing).is_none());
        assert!(input.select_all(view));
    });
    app.world_mut().write_message(Ime::Preedit {
        window,
        value: "中文".into(),
        cursor: Some((0, 6)),
    });
    app.update();
    let raw = app
        .world()
        .get::<EditableText>(view)
        .unwrap()
        .editor
        .raw_text()
        .to_owned();
    with_input(&mut app, |input| {
        assert!(input.selection(view).is_none());
        assert!(input.selected_text(view).is_none());
        assert!(!input.set_caret_position(view, 0));
        assert_eq!(input.focused_entity(), Some(view));
    });
    assert_eq!(
        app.world()
            .get::<EditableText>(view)
            .unwrap()
            .editor
            .raw_text(),
        raw
    );
    app.world_mut().write_message(Ime::Commit {
        window,
        value: "中文".into(),
    });
    app.update();
    assert_eq!(value(&app, view), "中文");
    app.world_mut()
        .get_mut::<EditableText>(view)
        .unwrap()
        .queue_edit(TextEdit::ImeSetCompose {
            value: "新".into(),
            cursor: None,
        });
    with_input(&mut app, |input| assert!(!input.select_all(view)));
    app.world_mut()
        .get_mut::<EditableText>(view)
        .unwrap()
        .pending_edits
        .clear();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_visible(owner, false);
    with_input(&mut app, |input| assert!(!input.select_all(view)));
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_visible(owner, true);
    app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
    with_input(&mut app, |input| assert!(!input.select_all(view)));
    app.world_mut().despawn(view);
    with_input(&mut app, |input| assert!(input.selection(view).is_none()));
}

#[test]
fn direct_selection_addresses_real_utf16_text_in_readonly_passwords_and_compound_titles() {
    let (mut app, owner, _, _) = fixture();
    let wrapper = {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        let parent = runtime.store().object(owner).unwrap().parent().unwrap();
        let wrapper = runtime
            .store_mut()
            .create(fairygui_core::package::ObjectType::Label);
        runtime.store_mut().set_parent(wrapper, Some(parent));
        runtime.store_mut().set_name(owner, "title");
        runtime.store_mut().set_parent(owner, Some(wrapper));
        runtime.store_mut().set_object_text(owner, "A中😀Z");
        runtime.store_mut().set_input_password(owner, true);
        runtime.store_mut().set_input_editable(owner, false);
        runtime.queue_subtree(wrapper);
        wrapper
    };
    app.update();
    let wrapper = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(wrapper)
        .unwrap();
    let view = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(owner)
        .unwrap();
    // Apply the application's new value after the compound view is materialized.
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text(owner, "A中😀Z");
    with_input(&mut app, |input| {
        assert!(input.set_selection(wrapper, GuiTextSelection::new(2, 4)));
        assert_eq!(input.selection(wrapper), Some(GuiTextSelection::new(2, 4)));
        assert_eq!(input.selected_text(wrapper), Some("😀"));
        assert_eq!(input.focused_entity(), Some(view));
    });
    app.update();
    assert_eq!(value(&app, view), "A中😀Z");
    with_input(&mut app, |input| {
        assert_eq!(input.selected_text(wrapper), Some("😀"))
    });
}

#[test]
fn direct_selection_rejects_old_picking_drag_edits_without_changing_other_bevy_editors() {
    use bevy::ecs::system::RunSystemOnce;
    let (mut app, owner, view, _) = fixture();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text(owner, "abcdef");
    with_input(&mut app, |input| assert!(input.set_caret_position(view, 0)));
    // The widget's generic autoscroll runs after Respond and can still see its
    // old picking drag. These are the exact two edits it produces outside bounds.
    app.world_mut()
        .get_mut::<EditableText>(view)
        .unwrap()
        .pending_edits
        .extend([
            TextEdit::ScrollBy(Vec2::splat(100.)),
            TextEdit::ExtendSelectionToPoint(Vec2::splat(100.)),
        ]);
    app.world_mut()
        .run_system_once(super::super::text_selection::autoscroll)
        .unwrap();
    assert!(
        app.world()
            .get::<EditableText>(view)
            .unwrap()
            .pending_edits
            .is_empty()
    );
    app.world_mut()
        .run_system_once(runtime::apply_selection_edits)
        .unwrap();
    with_input(&mut app, |input| {
        assert_eq!(input.selection(view), Some(GuiTextSelection::caret(0)))
    });
    let foreign = app
        .world_mut()
        .spawn(EditableText::new("other Bevy widget"))
        .id();
    app.world_mut()
        .get_mut::<EditableText>(foreign)
        .unwrap()
        .queue_edit(TextEdit::ScrollBy(Vec2::splat(10.)));
    let pending = app
        .world()
        .get::<EditableText>(foreign)
        .unwrap()
        .pending_edits
        .clone();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(foreign, FocusCause::Navigated);
    app.world_mut()
        .run_system_once(super::super::text_selection::autoscroll)
        .unwrap();
    assert_eq!(
        app.world()
            .get::<EditableText>(foreign)
            .unwrap()
            .pending_edits,
        pending
    );
}

#[test]
fn selection_consumes_same_frame_ecs_text_without_replaying_it_over_later_native_edits() {
    use crate::{GuiText, properties::sync_property};
    let (mut app, owner, view, _) = fixture();
    sync_property::<GuiText>(app.world_mut());
    app.world_mut()
        .get_mut::<GuiText>(view)
        .unwrap()
        .set("A中😀Z");
    with_input(&mut app, |input| {
        assert!(input.set_selection(view, GuiTextSelection::new(4, 1)));
        assert_eq!(input.selected_text(view), Some("中😀"));
    });
    assert_eq!(value(&app, view), "A中😀Z");
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .object_text(owner),
        Some("A中😀Z")
    );
    // The consumed ECS value must not be interpreted as another pending write
    // after a newer direct edit in this same response phase.
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text(owner, "newer native value");
    with_input(&mut app, |input| {
        assert!(input.select_all(view));
        assert_eq!(input.selected_text(view), Some("newer native value"));
    });
    sync_property::<GuiText>(app.world_mut());
    app.update();
    assert_eq!(value(&app, view), "newer native value");
    assert_eq!(
        app.world().get::<GuiText>(view).unwrap().value(),
        "newer native value"
    );
    with_input(&mut app, |input| {
        assert_eq!(input.selection(view).unwrap().range(), 0..18)
    });
}

#[test]
fn selection_consumes_nested_title_text_in_outer_to_inner_order_and_preserves_ime() {
    use crate::{GuiText, properties::sync_property};
    let (mut app, owner, _, window) = fixture();
    let (outer, inner) = {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        let parent = runtime.store().object(owner).unwrap().parent().unwrap();
        let store = runtime.store_mut();
        let outer = store.create(fairygui_core::package::ObjectType::Label);
        let inner = store.create(fairygui_core::package::ObjectType::Label);
        store.set_parent(outer, Some(parent));
        store.set_parent(inner, Some(outer));
        store.set_name(inner, "title");
        store.set_parent(owner, Some(inner));
        store.set_name(owner, "title");
        runtime.queue_subtree(outer);
        (outer, inner)
    };
    app.update();
    let (outer, inner, view) = {
        let runtime = app.world().non_send::<GuiRuntime>();
        (
            runtime.entity_of(outer).unwrap(),
            runtime.entity_of(inner).unwrap(),
            runtime.entity_of(owner).unwrap(),
        )
    };
    sync_property::<GuiText>(app.world_mut());
    // Calling the leaf also consumes an edited compound title above it.
    app.world_mut()
        .get_mut::<GuiText>(outer)
        .unwrap()
        .set("A中😀Z");
    with_input(&mut app, |input| {
        assert!(input.select_all(view));
        assert_eq!(input.selected_text(view), Some("A中😀Z"));
    });
    sync_property::<GuiText>(app.world_mut());
    app.world_mut()
        .get_mut::<GuiText>(outer)
        .unwrap()
        .set("outer");
    app.world_mut()
        .get_mut::<GuiText>(inner)
        .unwrap()
        .set("inner");
    app.world_mut()
        .get_mut::<GuiText>(view)
        .unwrap()
        .set("leaf😀");
    with_input(&mut app, |input| {
        assert!(input.select_all(outer));
        assert_eq!(input.selected_text(outer), Some("leaf😀"));
    });
    sync_property::<GuiText>(app.world_mut());
    app.update();
    assert_eq!(value(&app, view), "leaf😀");
    with_input(&mut app, |input| {
        assert!(input.set_caret_position(view, usize::MAX))
    });
    app.world_mut().write_message(Ime::Preedit {
        window,
        value: "预编辑".into(),
        cursor: None,
    });
    app.update();
    app.world_mut()
        .get_mut::<GuiText>(view)
        .unwrap()
        .set("pending ECS text");
    with_input(&mut app, |input| assert!(!input.select_all(view)));
    assert!(
        app.world()
            .get::<EditableText>(view)
            .unwrap()
            .is_composing()
    );
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .object_text(owner),
        Some("leaf😀")
    );
}

#[test]
fn selection_consumes_text_on_ready_root_and_window_owners_even_when_called_on_the_title() {
    use crate::prelude::*;
    use crate::properties::sync_property;
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .init_resource::<bevy::text::FontCx>()
    .init_resource::<bevy::text::LayoutCx>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: FuiPackage::parse(
                "Basics",
                include_bytes!("../../../fairygui-demo/assets/ui/Basics.fui"),
            )
            .unwrap(),
            res_key: "ui/Basics".into(),
        });
    let root = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), "Button1"))
        .id();
    let mut source = FairyGuiWindow::new(asset, "Button1");
    source.center_on_create = false;
    let window = app.world_mut().spawn(source).id();
    app.update();
    let inputs: Vec<_> = {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        [root, window]
            .into_iter()
            .map(|entity| {
                let parent = runtime
                    .ecs_root_object(entity)
                    .or_else(|| runtime.ecs_window_object(entity))
                    .unwrap();
                let store = runtime.store_mut();
                let old = store.child_by_name(parent, "title").unwrap();
                store.set_name(old, "old_title");
                let input = store.create(fairygui_core::package::ObjectType::InputText);
                store.set_name(input, "title");
                store.set_parent(input, Some(parent));
                store.set_size(input, 100., 30.);
                runtime.queue_subtree(input);
                input
            })
            .collect()
    };
    app.update();
    app.update();
    for (index, (entity, input)) in [root, window].into_iter().zip(inputs).enumerate() {
        let title = app
            .world()
            .non_send::<GuiRuntime>()
            .entity_of(input)
            .unwrap();
        sync_property::<GuiText>(app.world_mut());
        assert!(app.world().get::<GuiText>(entity).is_none());
        app.world_mut()
            .entity_mut(entity)
            .insert(GuiText::new("A中😀Z"));
        with_input(&mut app, |service| {
            assert!(service.set_selection(title, GuiTextSelection::new(4, 1)));
            assert_eq!(service.selected_text(entity), Some("中😀"));
        });
        app.update();
        with_input(&mut app, |service| {
            assert_eq!(service.selected_text(entity), Some("中😀"))
        });
        let replacement = format!("owner edit {index}");
        app.world_mut()
            .get_mut::<GuiText>(entity)
            .unwrap()
            .set(&replacement);
        with_input(&mut app, |service| {
            assert!(service.select_all(entity));
            assert_eq!(service.selected_text(title), Some(replacement.as_str()));
        });
    }
}

#[test]
fn selection_consumes_initial_text_without_a_property_baseline_and_never_replays_it() {
    use crate::{GuiText, properties::sync_property};
    let (mut app, owner, view, _) = fixture();
    // Reproduce a newly attached property before the first baseline projection.
    app.world_mut()
        .entity_mut(view)
        .remove::<(GuiText, crate::properties::NativeValue<GuiText>)>();
    assert!(
        app.world()
            .get::<crate::properties::NativeValue<GuiText>>(view)
            .is_none()
    );
    app.world_mut()
        .entity_mut(view)
        .insert(GuiText::new("A中😀Z"));
    with_input(&mut app, |input| {
        assert!(input.set_selection(view, GuiTextSelection::new(4, 1)));
        assert_eq!(input.selected_text(view), Some("中😀"));
    });
    // A different system parameter instance sees the same acknowledged write.
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text(owner, "later direct value");
    with_input(&mut app, |input| {
        assert!(input.select_all(view));
        assert_eq!(input.selected_text(view), Some("later direct value"));
    });
    sync_property::<GuiText>(app.world_mut());
    app.update();
    assert_eq!(
        app.world().get::<GuiText>(view).unwrap().value(),
        "later direct value"
    );
    assert_eq!(value(&app, view), "later direct value");
    with_input(&mut app, |input| {
        assert_eq!(input.selection(view).unwrap().range(), 0..18)
    });
}

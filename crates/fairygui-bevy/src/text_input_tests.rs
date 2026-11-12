use super::*;

fn fixture(restrict: Option<&str>, max: i32, single: bool) -> (App, GObjectId, Entity) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<FontCx>()
        .init_resource::<LayoutCx>()
        .init_resource::<Clipboard>();
    let mut runtime = GuiRuntime::default();
    let owner = runtime
        .store_mut()
        .create(fairygui_core::package::ObjectType::InputText);
    runtime.store_mut().set_object_single_line(owner, single);
    runtime
        .store_mut()
        .set_input_restrict(owner, restrict)
        .unwrap();
    runtime.store_mut().set_input_max_length(owner, max);
    let entity = app
        .world_mut()
        .spawn((
            GObjectRef(owner),
            Text::new(""),
            EditableText::new(""),
            InputTextState {
                source: String::new(),
                display: String::new(),
            },
        ))
        .id();
    app.insert_non_send(runtime)
        .add_systems(Update, (sync_text_inputs, sync_input_policy).chain())
        .add_systems(
            PostUpdate,
            (apply_input_edits, bevy::text::apply_text_edits).chain(),
        );
    app.update();
    (app, owner, entity)
}
fn edit(app: &mut App, entity: Entity, actions: impl IntoIterator<Item = TextEdit>) {
    app.world_mut()
        .get_mut::<EditableText>(entity)
        .unwrap()
        .pending_edits
        .extend(actions);
    app.update();
    app.update();
}
fn value(app: &App, entity: Entity) -> String {
    app.world()
        .get::<EditableText>(entity)
        .unwrap()
        .value()
        .to_string()
}

#[test]
fn input_edits_filter_complete_values_clip_utf16_and_preserve_programmatic_assignments() {
    let (mut app, owner, entity) = fixture(Some("0-9"), 6, true);
    edit(&mut app, entity, [TextEdit::Insert("a12b34567".into())]);
    assert_eq!(value(&app, entity), "1234");
    assert_eq!(app.world().get::<Text>(entity).unwrap().0, "1234");
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
        "1234"
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text(owner, "Program\r\n987654321A\n");
    app.update();
    assert_eq!(value(&app, entity), "Program987654321A");
    edit(
        &mut app,
        entity,
        [TextEdit::TextEnd(false), TextEdit::Backspace],
    );
    assert_eq!(value(&app, entity), "987654321");
    edit(
        &mut app,
        entity,
        [TextEdit::TextStart(false), TextEdit::Delete],
    );
    assert_eq!(value(&app, entity), "87654321");
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_input_restrict(owner, None)
        .unwrap();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_input_max_length(owner, 7);
    edit(
        &mut app,
        entity,
        [TextEdit::SelectAll, TextEdit::Insert("abc😀123456".into())],
    );
    assert_eq!(value(&app, entity), "abc😀12");
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_input_max_length(owner, 1);
    edit(
        &mut app,
        entity,
        [TextEdit::SelectAll, TextEdit::Insert("😀1".into())],
    );
    assert_eq!(value(&app, entity), "");
    assert!(
        app.world()
            .get::<EditableText>(entity)
            .unwrap()
            .max_characters
            .is_none()
    );
}

#[test]
fn clipboard_mixed_characters_and_ime_commit_share_policy_but_preedit_remains_unfiltered() {
    let (mut app, owner, entity) = fixture(Some("0-9"), 4, true);
    app.world_mut()
        .resource_mut::<Clipboard>()
        .set_text("a12b34z")
        .unwrap();
    edit(&mut app, entity, [TextEdit::Paste]);
    assert_eq!(value(&app, entity), "12"); // raw UTF-16 budget clips before native validation
    edit(
        &mut app,
        entity,
        [
            TextEdit::TextEnd(false),
            TextEdit::ImeSetCompose {
                value: "中文".into(),
                cursor: None,
            },
        ],
    );
    assert_eq!(value(&app, entity), "12");
    assert!(
        app.world()
            .get::<EditableText>(entity)
            .unwrap()
            .is_composing()
    );
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
        "12"
    );
    edit(
        &mut app,
        entity,
        [TextEdit::ImeCommit {
            value: "3A4".into(),
        }],
    );
    assert_eq!(value(&app, entity), "123");
    assert!(
        !app.world()
            .get::<EditableText>(entity)
            .unwrap()
            .is_composing()
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_input_restrict(owner, None)
        .unwrap();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_input_max_length(owner, 0);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_single_line(owner, false);
    edit(
        &mut app,
        entity,
        [TextEdit::SelectAll, TextEdit::Insert("A\r\nB\nC".into())],
    );
    assert_eq!(value(&app, entity), "A\r\nB\nC");
    assert!(
        app.world()
            .get::<EditableText>(entity)
            .unwrap()
            .allow_newlines
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_single_line(owner, true);
    edit(
        &mut app,
        entity,
        [TextEdit::TextEnd(false), TextEdit::Insert("\nD".into())],
    );
    assert_eq!(value(&app, entity), "ABCD");
}

#[test]
fn asynchronous_clipboard_preserves_fifo_and_disposal_reclaims_pending_state() {
    let (mut app, _, entity) = fixture(Some("0-9"), 8, true);
    let bevy::clipboard::ClipboardRead::Pending(shared) =
        ClipboardRead::Pending(Default::default())
    else {
        unreachable!()
    };
    app.world_mut()
        .get_mut::<InputEditPolicy>(entity)
        .unwrap()
        .paste = Some(ClipboardRead::Pending(shared.clone()));
    edit(&mut app, entity, [TextEdit::Insert("9".into())]);
    assert_eq!(value(&app, entity), "");
    assert!(
        app.world()
            .get::<EditableText>(entity)
            .unwrap()
            .pending_edits
            .is_empty()
    );
    *shared.lock().unwrap() = Some(Ok("a12".into()));
    app.update();
    app.update();
    assert_eq!(value(&app, entity), "129");
    let before = app
        .world()
        .entity(entity)
        .get_ref::<EditableText>()
        .unwrap()
        .last_changed();
    for _ in 0..60 {
        app.update();
    }
    assert_eq!(
        app.world()
            .entity(entity)
            .get_ref::<EditableText>()
            .unwrap()
            .last_changed(),
        before
    );
    app.world_mut()
        .get_mut::<InputEditPolicy>(entity)
        .unwrap()
        .paste = Some(ClipboardRead::Pending(Default::default()));
    app.world_mut().despawn(entity);
    app.update();
    assert!(app.world().get_entity(entity).is_err());
}

#[test]
fn invalid_loaded_restrictions_disable_insertion_without_panicking_and_valid_setters_recover() {
    let (mut app, owner, entity) = fixture(None, 0, true);
    let invalid = app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_input_restrict(owner, Some("z-a"));
    assert!(invalid.is_err());
    app.world_mut().entity_mut(entity).insert(InputEditPolicy {
        restrict: None,
        multiline: false,
        max: 10,
        editable: true,
        password: false,
        compiled: None,
        paste: None,
        queued: Default::default(),
    });
    edit(&mut app, entity, [TextEdit::Insert("123".into())]);
    assert_eq!(value(&app, entity), "");
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_input_restrict(owner, Some("0-9"))
        .unwrap();
    edit(&mut app, entity, [TextEdit::Insert("1a2".into())]);
    assert_eq!(value(&app, entity), "12");
}

#[test]
fn unrelated_editors_keep_bevy_default_edit_behavior() {
    let (mut app, _, _) = fixture(Some("0-9"), 1, true);
    let mut editor = EditableText::new("");
    editor.max_characters = Some(2);
    let entity = app.world_mut().spawn(editor).id();
    edit(&mut app, entity, [TextEdit::Insert("ABC".into())]);
    assert_eq!(value(&app, entity), "");
    edit(&mut app, entity, [TextEdit::Insert("A!".into())]);
    assert_eq!(value(&app, entity), "A!");
}

#[test]
fn read_only_inputs_keep_selection_copy_and_programmatic_updates_but_reject_edits_and_pending_paste()
 {
    let (mut app, owner, entity) = fixture(None, 0, true);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text(owner, "Read only");
    app.update();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_input_editable(owner, false);
    edit(
        &mut app,
        entity,
        [
            TextEdit::SelectAll,
            TextEdit::Copy,
            TextEdit::Cut,
            TextEdit::Insert("bad".into()),
            TextEdit::Backspace,
            TextEdit::Paste,
            TextEdit::ImeSetCompose {
                value: "预编辑".into(),
                cursor: None,
            },
            TextEdit::ImeCommit {
                value: "改".into()
            },
        ],
    );
    assert_eq!(value(&app, entity), "Read only");
    assert_eq!(
        *app.world()
            .get::<bevy::text::TextReadWriteMode>(entity)
            .unwrap(),
        bevy::text::TextReadWriteMode::ReadOnly
    );
    assert_eq!(
        app.world_mut()
            .resource_mut::<Clipboard>()
            .fetch_text()
            .poll_result()
            .unwrap()
            .unwrap(),
        "Read only"
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text(owner, "Programmatic readonly");
    app.update();
    assert_eq!(value(&app, entity), "Programmatic readonly");
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_input_editable(owner, true);
    edit(
        &mut app,
        entity,
        [TextEdit::SelectAll, TextEdit::Insert("editable".into())],
    );
    assert_eq!(value(&app, entity), "editable");
    let shared = std::sync::Arc::default();
    let read = ClipboardRead::Pending(shared);
    app.world_mut()
        .get_mut::<InputEditPolicy>(entity)
        .unwrap()
        .paste = Some(read);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_input_editable(owner, false);
    app.update();
    assert!(
        app.world()
            .get::<InputEditPolicy>(entity)
            .unwrap()
            .paste
            .is_none()
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_input_editable(owner, true);
    edit(
        &mut app,
        entity,
        [TextEdit::TextEnd(false), TextEdit::Insert("!".into())],
    );
    assert_eq!(value(&app, entity), "editable!");
}

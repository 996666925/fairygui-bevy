use super::*;

#[derive(Resource)]
struct BitmapInputAsset {
    _handle: Handle<crate::FuiAsset>,
}

fn canvas_input(bitmap: bool, rich: bool) -> (App, GObjectId, Entity, Entity) {
    let (mut app, owner, entity, window) = fixture();
    if bitmap {
        let package = FuiPackage::parse(
            "BitmapText",
            include_bytes!("../../../fairygui-core/tests/fixtures/BitmapText.fui"),
        )
        .unwrap();
        let handle = app
            .world_mut()
            .resource_mut::<Assets<crate::FuiAsset>>()
            .add(crate::FuiAsset {
                package,
                res_key: "ui/BitmapText".into(),
            });
        app.insert_resource(BitmapInputAsset { _handle: handle });
    }
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut()
            .set_object_font(owner, bitmap.then_some("ui://BitmapText/HitNumber"));
        gui.store_mut().set_object_ubb_enabled(owner, rich);
        gui.store_mut()
            .set_object_text(owner, if rich { "[b]12[/b]" } else { "12" });
    }
    app.update();
    app.update();
    assert!(app.world().get::<EditableText>(entity).is_none());
    events(&mut app);
    (app, owner, entity, window)
}

fn assert_canvas(app: &App, entity: Entity, bitmap: bool, rich: bool) {
    assert!(app.world().get::<EditableText>(entity).is_none());
    if bitmap && rich {
        assert!(app.world().get::<runtime::BitmapRichText>(entity).is_some());
    } else if bitmap {
        assert!(app.world().get::<runtime::BitmapGlyphs>(entity).is_some());
    } else {
        assert!(app.world().get::<runtime::RichTextBlock>(entity).is_some());
    }
}

#[test]
fn bitmap_and_styled_inputs_accept_the_first_keyboard_message_and_restore_canvas_on_blur() {
    for (bitmap, rich) in [(true, false), (true, true), (false, true)] {
        let (mut app, owner, entity, window) = canvas_input(bitmap, rich);
        let child = app
            .world_mut()
            .spawn((Node::default(), ChildOf(entity)))
            .id();
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .request_input_focus(Some(owner));
        key(
            &mut app,
            window,
            Key::Character("3".into()),
            KeyCode::Digit3,
            Some("3"),
        );
        app.update();
        // The suspended editor retains its caret at the beginning; activation
        // must not reset it simply to append the first incoming character.
        let expected = if rich { "3[b]12[/b]" } else { "312" };
        assert_eq!(value(&app, entity), expected);
        assert_eq!(app.world().get::<Text>(entity).unwrap().0, expected);
        assert_eq!(events(&mut app), [EventKind::FocusIn, EventKind::Input]);
        assert!(app.world().get::<runtime::RichTextBlock>(entity).is_none());
        assert!(app.world().get::<runtime::BitmapGlyphs>(entity).is_none());
        assert!(app.world().get::<runtime::BitmapRichText>(entity).is_none());
        blur(&mut app);
        app.update();
        assert_canvas(&app, entity, bitmap, rich);
        assert_eq!(events(&mut app), [EventKind::Change, EventKind::FocusOut]);
        assert_eq!(
            app.world().non_send::<GuiRuntime>().entity_of(owner),
            Some(entity)
        );
        assert_eq!(app.world().get::<ChildOf>(child).unwrap().parent(), entity);
        // A model write while the editor is suspended must win on reactivation.
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_object_text(owner, "45");
        app.update();
        assert!(focus(&mut app, entity));
        app.update();
        app.update();
        assert_eq!(value(&app, entity), "45");
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
            "45"
        );
    }
}

#[test]
fn direct_selection_activates_canvas_inputs_and_blur_cancels_unprojected_selection() {
    for (bitmap, rich) in [(true, false), (false, true)] {
        let (mut app, _, entity, window) = canvas_input(bitmap, rich);
        with_input(&mut app, |input| assert!(input.select_all(entity)));
        app.update();
        with_input(&mut app, |input| {
            assert_eq!(
                input.selected_text(entity),
                Some(if rich { "[b]12[/b]" } else { "12" })
            );
        });
        key(
            &mut app,
            window,
            Key::Character("7".into()),
            KeyCode::Digit7,
            Some("7"),
        );
        app.update();
        assert_eq!(value(&app, entity), "7");
        blur(&mut app);
        app.update();
        if bitmap {
            with_input(&mut app, |input| {
                assert!(input.select_all(entity));
                input.blur();
            });
            app.update();
            assert_eq!(app.world().resource::<InputFocus>().get(), None);
            assert_canvas(&app, entity, true, false);
        }
    }
}

#[test]
fn active_canvas_editor_keeps_selection_and_ime_during_font_and_style_changes() {
    let (mut app, owner, entity, window) = canvas_input(true, false);
    assert!(focus(&mut app, entity));
    app.update();
    with_input(&mut app, |input| {
        assert!(input.set_selection(entity, GuiTextSelection::new(0, 1)))
    });
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_ubb_enabled(owner, true);
    app.update();
    with_input(&mut app, |input| {
        assert_eq!(input.selected_text(entity), Some("1"))
    });
    app.world_mut().write_message(Ime::Preedit {
        window,
        value: "预编辑".into(),
        cursor: None,
    });
    app.update();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_font(owner, None);
    app.update();
    assert!(
        app.world()
            .get::<EditableText>(entity)
            .unwrap()
            .is_composing()
    );
    app.world_mut().write_message(Ime::Commit {
        window,
        value: "输入".into(),
    });
    app.update();
    app.update();
    assert_eq!(value(&app, entity), "输入2");
    assert_eq!(
        app.world().non_send::<GuiRuntime>().entity_of(owner),
        Some(entity)
    );
}

#[test]
fn readonly_bitmap_input_can_select_and_copy_but_rejects_keyboard_and_ime_mutations() {
    let (mut app, owner, entity, window) = canvas_input(true, false);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_input_editable(owner, false);
    with_input(&mut app, |input| assert!(input.select_all(entity)));
    app.update();
    key(
        &mut app,
        window,
        Key::Character("9".into()),
        KeyCode::Digit9,
        Some("9"),
    );
    app.world_mut().write_message(Ime::Commit {
        window,
        value: "输入".into(),
    });
    app.update();
    assert_eq!(value(&app, entity), "12");
    with_input(&mut app, |input| {
        assert_eq!(input.selected_text(entity), Some("12"))
    });
    app.world_mut()
        .get_mut::<EditableText>(entity)
        .unwrap()
        .queue_edit(TextEdit::Copy);
    app.update();
    assert_eq!(
        app.world_mut()
            .resource_mut::<bevy::clipboard::Clipboard>()
            .fetch_text()
            .poll_result()
            .unwrap()
            .unwrap(),
        "12"
    );
    events(&mut app);
    blur(&mut app);
    app.update();
    assert_eq!(events(&mut app), [EventKind::FocusOut]);
    assert_canvas(&app, entity, true, false);
}

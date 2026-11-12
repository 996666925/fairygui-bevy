use super::*;

fn fixture() -> (App, GObjectId, Entity) {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<FuiAsset>()
        .init_asset::<Image>()
        .init_asset::<Font>()
        .init_resource::<GuiFont>()
        .init_resource::<FontCx>()
        .init_resource::<LayoutCx>()
        .init_resource::<bevy::text::ScaleCx>()
        .init_resource::<bevy::text::FontAtlasSet>()
        .init_resource::<bevy::text::RemSize>()
        .init_resource::<bevy::clipboard::Clipboard>();
    let font = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(
            include_bytes!("../../fairygui-demo/assets/fonts/NotoSansSC-VF.ttf").to_vec(),
        ));
    let package = FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-core/tests/fixtures/BitmapText.fui"),
    )
    .unwrap();
    let handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: package.clone(),
            res_key: "ui/Basics".into(),
        });
    let mut runtime = GuiRuntime {
        font: Some(font),
        ..default()
    };
    let index = runtime.create_root(handle, &package, "Demo_Text").unwrap();
    let root = runtime.root_object(index).unwrap();
    let input = runtime.store().child_by_name(root, "n22").unwrap();
    runtime.store_mut().set_object_font(input, None);
    runtime.store_mut().set_object_ubb_enabled(input, false);
    runtime.store_mut().set_object_single_line(input, true);
    runtime.store_mut().set_input_password(input, true);
    runtime
        .store_mut()
        .set_input_prompt(input, Some("Password hint"));
    runtime.store_mut().set_object_text(input, "A中😀e\u{301}");
    app.insert_non_send(runtime)
        .add_systems(
            Update,
            (
                spawn_pending_roots,
                spawn_pending_objects,
                sync_text_inputs,
                sync_dirty,
                sync_text_fonts,
                sync_input_policy,
                sync_password_views,
                sync_input_prompts,
                sync_bitmap_rich_text,
                sync_rich_text,
                sync_stage_hierarchy,
            )
                .chain(),
        )
        .add_systems(
            PostUpdate,
            (
                bevy::text::load_font_assets_into_font_collection,
                bevy::ui::widget::update_editable_text_styles,
                apply_input_edits,
                prepare_password_layout,
                bevy::ui::widget::update_editable_text_layout,
                publish_password_layout,
                update_prompt_visibility,
            )
                .chain(),
        );
    app.update();
    app.update();
    let entity = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(input)
        .unwrap();
    (app, input, entity)
}
fn queue(app: &mut App, entity: Entity, actions: impl IntoIterator<Item = TextEdit>) {
    app.world_mut()
        .get_mut::<EditableText>(entity)
        .unwrap()
        .pending_edits
        .extend(actions);
    app.update();
    app.update();
}
fn real(app: &App, entity: Entity) -> String {
    app.world()
        .get::<EditableText>(entity)
        .unwrap()
        .value()
        .to_string()
}
fn assert_mask(app: &App, entity: Entity, count: usize) {
    let view = app.world().get::<InputPasswordView>(entity).unwrap().0;
    assert_eq!(
        app.world()
            .get::<EditableText>(view)
            .unwrap()
            .editor
            .raw_text(),
        "●".repeat(count)
    );
    assert_eq!(
        app.world().get::<Text>(entity).unwrap().0,
        "●".repeat(count)
    );
    let info = app.world().get::<TextLayoutInfo>(entity).unwrap();
    let projected = app.world().get::<TextLayoutInfo>(view).unwrap();
    assert_eq!(info.glyphs.len(), count);
    for (actual, expected) in info.glyphs.iter().zip(&projected.glyphs) {
        assert_eq!(actual.position, expected.position);
        assert_eq!(actual.atlas_info.texture, expected.atlas_info.texture);
        assert_eq!(actual.atlas_info.rect, expected.atlas_info.rect);
    }
    assert_eq!(info.size, projected.size);
}

#[test]
fn utf16_map_folds_crlf_and_snaps_pointer_endpoints_to_scalar_boundaries() {
    let map = PasswordMap::new("A😀\r\n中");
    assert_eq!(map.text, "●".repeat(5));
    assert_eq!(map.display(5), 9);
    assert_eq!(map.display(7), 12);
    for (raw, mask) in &map.boundaries {
        assert_eq!(map.source(*mask), *raw);
    }
    assert_eq!(map.source(6), 5); // inside astral pair: nearest legal endpoint
    assert_eq!(map.source(usize::MAX), 10);
}

#[test]
fn password_masks_draw_geometry_preserves_raw_value_and_edits_unicode_clipboard_and_ime() {
    let (mut app, owner, entity) = fixture();
    assert_eq!(real(&app, entity), "A中😀e\u{301}");
    assert_mask(&app, entity, 6);
    let view = app.world().get::<InputPasswordView>(entity).unwrap().0;
    assert!(app.world().get::<Node>(view).is_none());
    assert!(app.world().get::<GObjectRef>(view).is_none());
    app.world_mut()
        .resource_mut::<bevy::clipboard::Clipboard>()
        .set_text("sentinel")
        .unwrap();
    queue(&mut app, entity, [TextEdit::SelectAll, TextEdit::Copy]);
    assert_eq!(
        app.world_mut()
            .resource_mut::<bevy::clipboard::Clipboard>()
            .fetch_text()
            .poll_result()
            .unwrap()
            .unwrap(),
        "sentinel"
    );
    queue(&mut app, entity, [TextEdit::Cut]);
    assert_eq!(real(&app, entity), "");
    assert_mask(&app, entity, 0);
    queue(
        &mut app,
        entity,
        [TextEdit::Insert("A中😀".into()), TextEdit::Backspace],
    );
    assert_eq!(real(&app, entity), "A中");
    assert_mask(&app, entity, 2);
    queue(
        &mut app,
        entity,
        [TextEdit::ImeSetCompose {
            value: "预😀".into(),
            cursor: None,
        }],
    );
    assert_eq!(real(&app, entity), "A中");
    let info = app.world().get::<TextLayoutInfo>(entity).unwrap();
    assert_eq!(info.glyphs.len(), 5);
    assert!(!info.preedit_underline_rects.is_empty());
    assert!(info.cursor.is_none());
    queue(
        &mut app,
        entity,
        [TextEdit::ImeCommit {
            value: "完成".into(),
        }],
    );
    assert_eq!(real(&app, entity), "A中完成");
    assert_mask(&app, entity, 4);
    let selection = *app
        .world()
        .get::<EditableText>(entity)
        .unwrap()
        .editor
        .raw_selection();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_input_password(owner, false);
    app.update();
    assert!(app.world().get_entity(view).is_err());
    assert_eq!(real(&app, entity), "A中完成");
    assert_eq!(
        *app.world()
            .get::<EditableText>(entity)
            .unwrap()
            .editor
            .raw_selection(),
        selection
    );
    assert_eq!(app.world().get::<Text>(entity).unwrap().0, "A中完成");
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_input_password(owner, true);
    app.update();
    assert_mask(&app, entity, 4);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .remove(owner);
    app.update();
    assert!(app.world().get_entity(entity).is_err());
}

#[test]
fn password_pointer_hits_use_mask_advances_and_stable_frames_preserve_layout_and_scroll() {
    let (mut app, owner, entity) = fixture();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text(owner, "iW中😀Z");
    app.update();
    app.update();
    let view = app.world().get::<InputPasswordView>(entity).unwrap().0;
    let info = app.world().get::<TextLayoutInfo>(view).unwrap();
    let advance = info.size.x / 6.;
    let y = info.size.y / 2.;
    queue(
        &mut app,
        entity,
        [
            TextEdit::MoveToPoint(Vec2::new(advance * 3.1, y)),
            TextEdit::ExtendSelectionToPoint(Vec2::new(advance * 4.9, y)),
            TextEdit::Insert("Q".into()),
        ],
    );
    assert_eq!(real(&app, entity), "iW中QZ");
    assert_mask(&app, entity, 5);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_font_size(owner, 40);
    app.update();
    app.update();
    assert!(app.world().get::<TextLayoutInfo>(entity).unwrap().size.x > advance * 5.);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text(owner, &"i".repeat(50));
    {
        let mut editor = app.world_mut().get_mut::<EditableText>(entity).unwrap();
        editor.viewport.size = Vec2::new(90., 50.);
        editor.editor.set_width(Some(90.));
    }
    app.insert_resource(bevy::input_focus::InputFocus::from_entity(entity));
    queue(&mut app, entity, [TextEdit::TextEnd(false)]);
    assert!(
        app.world()
            .get::<EditableText>(entity)
            .unwrap()
            .viewport
            .offset
            .x
            > 0.
    );
    assert_eq!(
        app.world().get::<EditableText>(entity).unwrap().viewport,
        app.world().get::<EditableText>(view).unwrap().viewport
    );
    app.update();
    let generation = app
        .world()
        .get::<EditableText>(view)
        .unwrap()
        .editor
        .generation();
    for _ in 0..60 {
        app.update();
    }
    assert_eq!(
        app.world().get::<InputPasswordView>(entity).unwrap().0,
        view
    );
    assert_eq!(
        app.world()
            .get::<EditableText>(view)
            .unwrap()
            .editor
            .generation(),
        generation
    );
    assert_mask(&app, entity, 50);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text(owner, "");
    app.update();
    let prompt = app
        .world()
        .get::<Children>(entity)
        .unwrap()
        .iter()
        .find(|&e| app.world().get::<InputPromptOwner>(e).is_some())
        .unwrap();
    assert_eq!(
        app.world().get::<Visibility>(prompt),
        Some(&Visibility::Inherited)
    );
    assert_eq!(app.world().get::<Text>(prompt).unwrap().0, "Password hint");
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_input_editable(owner, false);
    queue(&mut app, entity, [TextEdit::Insert("rejected".into())]);
    assert_eq!(real(&app, entity), "");
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text(owner, "Readonly😀");
    app.update();
    assert_mask(&app, entity, 10);
}

#[test]
fn password_projection_applies_before_rich_and_bitmap_renderers_and_reclaims_owned_layouts() {
    let (mut app, owner, entity) = fixture();
    let view = app.world().get::<InputPasswordView>(entity).unwrap().0;
    let app_child = app
        .world_mut()
        .spawn((Node::default(), ChildOf(entity)))
        .id();
    {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        runtime.store_mut().set_object_ubb_enabled(owner, true);
        runtime.store_mut().set_object_text(owner, "[b]UI#[/b]");
    }
    app.update();
    app.update();
    assert!(app.world().get::<EditableText>(entity).is_none());
    assert!(app.world().get_entity(view).is_err());
    let parts: Vec<_> = app
        .world()
        .get::<Children>(entity)
        .unwrap()
        .iter()
        .filter_map(|e| app.world().get::<TextSpan>(e).map(|s| s.0.clone()))
        .collect();
    assert_eq!(
        parts.concat(),
        "●".repeat("<b>UI#</b>".encode_utf16().count())
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text(owner, "[color=red]A[/color]");
    app.update();
    let parts: Vec<_> = app
        .world()
        .get::<Children>(entity)
        .unwrap()
        .iter()
        .filter_map(|e| app.world().get::<TextSpan>(e).map(|s| s.0.clone()))
        .collect();
    assert_eq!(
        parts.concat(),
        fairygui_core::text::input::password_text("[color=red]A[/color]", true)
    );
    {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        runtime.store_mut().set_object_ubb_enabled(owner, false);
        runtime
            .store_mut()
            .set_object_font(owner, Some("BMFontTest"));
        runtime.store_mut().set_object_text(owner, "UI#");
    }
    app.update();
    app.update();
    // The package has UI# but no mask glyph; missing glyphs must not reveal UI#.
    assert!(
        app.world()
            .get::<BitmapGlyphs>(entity)
            .unwrap()
            .0
            .is_empty()
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_input_password(owner, false);
    app.update();
    assert_eq!(app.world().get::<BitmapGlyphs>(entity).unwrap().0.len(), 3);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_input_password(owner, true);
    app.update();
    assert!(
        app.world()
            .get::<BitmapGlyphs>(entity)
            .unwrap()
            .0
            .is_empty()
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_ubb_enabled(owner, true);
    app.update();
    app.update();
    assert!(
        app.world()
            .get::<BitmapGlyphs>(entity)
            .unwrap()
            .0
            .is_empty()
    );
    assert!(app.world().get_entity(app_child).is_ok());
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
        "UI#"
    );
}

#[test]
fn password_uses_single_line_user_edits_and_visible_line_navigation_even_with_multiline_settings() {
    let (mut app, owner, entity) = fixture();
    {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        runtime.store_mut().set_object_single_line(owner, false);
        runtime.store_mut().set_object_text(owner, "A\r\nB");
    }
    app.update();
    assert_eq!(real(&app, entity), "A\r\nB");
    assert_mask(&app, entity, 3);
    assert!(
        !app.world()
            .get::<EditableText>(entity)
            .unwrap()
            .allow_newlines
    );
    assert_eq!(
        app.world().get::<TextLayout>(entity).unwrap().linebreak,
        bevy::text::LineBreak::NoWrap
    );
    queue(
        &mut app,
        entity,
        [TextEdit::TextStart(false), TextEdit::LineEnd(false)],
    );
    assert_eq!(
        app.world()
            .get::<EditableText>(entity)
            .unwrap()
            .editor
            .raw_selection()
            .focus()
            .index(),
        4
    );
    queue(&mut app, entity, [TextEdit::Insert("\nC".into())]);
    assert_eq!(real(&app, entity), "ABC");
    assert_mask(&app, entity, 3);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_input_password(owner, false);
    app.update();
    assert!(
        app.world()
            .get::<EditableText>(entity)
            .unwrap()
            .allow_newlines
    );
    assert_eq!(
        app.world().get::<TextLayout>(entity).unwrap().linebreak,
        bevy::text::LineBreak::WordOrCharacter
    );
}

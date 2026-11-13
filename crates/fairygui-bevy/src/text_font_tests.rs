use super::*;

fn fixture() -> (App, usize, GObjectId) {
    let package = FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui"),
    )
    .unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<FuiAsset>()
        .init_asset::<Image>()
        .init_asset::<Font>()
        .init_resource::<crate::GuiFont>();
    let handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: package.clone(),
            res_key: "owner/Basics".into(),
        });
    let mut gui = GuiRuntime::default();
    let index = gui.create_root(handle, &package, "Demo_Text").unwrap();
    let root = gui.root_object(index).unwrap();
    app.insert_non_send(gui).add_systems(
        Update,
        (
            sync_gui_font,
            spawn_pending_roots,
            spawn_pending_objects,
            sync_text_inputs,
            sync_dirty,
            sync_text_fonts,
            sync_input_policy,
            sync_bitmap_rich_text,
            sync_rich_text,
            sync_appearance,
            sync_stage_hierarchy,
            sync_inline_image_hierarchy,
            sync_text_sizes,
        )
            .chain(),
    );
    app.update();
    app.update();
    (app, index, root)
}

fn child(app: &App, root: GObjectId, name: &str) -> GObjectId {
    app.world()
        .non_send::<GuiRuntime>()
        .store()
        .child_by_name(root, name)
        .unwrap()
}
fn view(app: &App, id: GObjectId) -> Entity {
    app.world().non_send::<GuiRuntime>().entity_of(id).unwrap()
}
fn parts(app: &App, id: GObjectId) -> Vec<Entity> {
    app.world()
        .get::<BitmapGlyphs>(view(app, id))
        .unwrap()
        .0
        .clone()
}
fn add_foreign(app: &mut App) -> (Handle<FuiAsset>, String) {
    let package = FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-core/tests/fixtures/BitmapText.fui"),
    )
    .unwrap();
    let item = package
        .item(package.item_index_by_name("HitNumber").unwrap())
        .unwrap();
    let url = format!("ui://{}{}", package.id(), item.id);
    let handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package,
            res_key: "foreign/Basics".into(),
        });
    (handle, url)
}

#[test]
fn plain_and_rich_views_switch_to_both_foreign_font_families_and_back_without_losing_children() {
    let (mut app, index, root) = fixture();
    let (_foreign, canonical) = add_foreign(&mut app);
    for (name, font, content) in [
        ("n10", canonical.as_str(), "12345"),
        ("n12", "ui://BitmapText/BMFontTest", "UI#"),
    ] {
        let owner = child(&app, root, name);
        let entity = view(&app, owner);
        let application_child = app
            .world_mut()
            .spawn((Node::default(), ChildOf(entity)))
            .id();
        let old_images: Vec<_> = app
            .world()
            .get::<InlineImageParts>(entity)
            .map(|p| p.iter().collect())
            .unwrap_or_default();
        {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            gui.store_mut().set_object_text(owner, content);
            gui.store_mut().set_object_font(owner, Some(font));
        }
        app.update();
        app.update();
        assert_eq!(view(&app, owner), entity);
        assert!(app.world().get::<Text>(entity).is_none());
        assert!(app.world().get::<ComputedTextBlock>(entity).is_none());
        assert!(
            app.world()
                .get::<bevy::text::TextLayoutInfo>(entity)
                .is_none()
        );
        assert!(app.world().get::<bevy::ui::ContentSize>(entity).is_none());
        assert!(app.world().get::<RichTextBlock>(entity).is_none());
        assert_eq!(
            app.world().get::<Node>(entity).unwrap().padding,
            UiRect::ZERO
        );
        assert!(
            old_images
                .iter()
                .all(|&e| app.world().get_entity(e).is_err())
        );
        let glyphs = parts(&app, owner);
        assert_eq!(glyphs.len(), content.len());
        let path = app
            .world()
            .resource::<AssetServer>()
            .get_path(app.world().get::<ImageNode>(glyphs[0]).unwrap().image.id())
            .unwrap();
        assert!(
            path.path().to_string_lossy().starts_with("foreign/Basics"),
            "{path}"
        );
        assert!(app.world().get_entity(application_child).is_ok());
        for _ in 0..60 {
            app.update();
        }
        assert_eq!(
            parts(&app, owner),
            glyphs,
            "selection polling must not churn glyph entities"
        );
        assert!(
            !app.world_mut()
                .non_send_mut::<GuiRuntime>()
                .store_mut()
                .set_object_font(owner, Some(font))
        );
        app.update();
        assert_eq!(parts(&app, owner), glyphs);
        {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            gui.store_mut().set_object_font(owner, None);
            gui.store_mut().set_object_ubb_enabled(owner, true);
            gui.store_mut()
                .set_object_text(owner, "[b]Text[/b][img]ui://9leh0eyfrpmb6[/img]");
        }
        app.update();
        app.update();
        assert_eq!(view(&app, owner), entity);
        assert!(app.world().get::<BitmapGlyphs>(entity).is_none());
        assert!(app.world().get::<BitmapTextMetrics>(entity).is_none());
        assert!(glyphs.iter().all(|&e| app.world().get_entity(e).is_err()));
        assert!(app.world().get::<RichTextBlock>(entity).is_some());
        let image = app
            .world()
            .get::<InlineImageParts>(entity)
            .unwrap()
            .iter()
            .next()
            .unwrap();
        assert_eq!(
            app.world().get::<ChildOf>(image).unwrap().parent(),
            view(&app, root)
        );
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_object_ubb_enabled(owner, false);
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_object_html_enabled(owner, false);
        app.update();
        app.update();
        assert!(app.world().get::<RichTextBlock>(entity).is_none());
        assert_eq!(
            app.world().get::<Text>(entity).unwrap().0,
            "[b]Text[/b][img]ui://9leh0eyfrpmb6[/img]"
        );
        assert!(app.world().get_entity(image).is_err());
        assert!(app.world().get_entity(application_child).is_ok());
    }
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let mut gui = app.world_mut().remove_non_send::<GuiRuntime>().unwrap();
    gui.remove_root(&mut Commands::new(&mut queue, app.world()), index);
    queue.apply(app.world_mut());
    app.insert_non_send(gui);
    app.update();
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .entity_of(root)
            .is_none()
    );
}

#[test]
fn foreign_font_arrival_unload_and_obsolete_request_follow_the_current_model_selection() {
    let (mut app, _, root) = fixture();
    let owner = child(&app, root, "n10");
    let stale = child(&app, root, "n2");
    let entity = view(&app, owner);
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        for id in [owner, stale] {
            gui.store_mut().set_object_text(id, "UI#");
            gui.store_mut()
                .set_object_font(id, Some("ui://BitmapText/BMFontTest"));
        }
    }
    app.update();
    assert!(app.world().get::<BitmapGlyphs>(entity).is_none());
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_font(stale, None);
    let (handle, _) = add_foreign(&mut app);
    app.update();
    app.update();
    assert_eq!(view(&app, owner), entity);
    let glyphs = parts(&app, owner);
    assert_eq!(glyphs.len(), 3);
    assert!(app.world().get::<BitmapGlyphs>(view(&app, stale)).is_none());
    assert!(app.world().get::<Text>(view(&app, stale)).is_some());
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .remove(handle.id());
    app.update();
    app.update();
    assert!(glyphs.iter().all(|&e| app.world().get_entity(e).is_err()));
    assert!(app.world().get::<Text>(entity).is_some());
    assert!(app.world().get::<BitmapGlyphs>(entity).is_none());
    let (_reloaded, _) = add_foreign(&mut app);
    app.update();
    assert_eq!(parts(&app, owner).len(), 3);
    // A wrong explicit owner must not bind a same-name font in Basics.
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_font(owner, Some("ui://Missing/BMFontTest"));
    app.update();
    assert!(app.world().get::<BitmapGlyphs>(entity).is_none());
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_font(owner, Some("BMFontTest"));
    app.update();
    let image = app.world().get::<ImageNode>(parts(&app, owner)[0]).unwrap();
    let path = app
        .world()
        .resource::<AssetServer>()
        .get_path(image.image.id())
        .unwrap();
    assert!(
        path.path().to_string_lossy().starts_with("owner/Basics"),
        "{path}"
    );
}

#[test]
fn default_font_changes_preserve_editable_state_and_bitmap_switch_restores_editability() {
    let (mut app, _, root) = fixture();
    let input = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let input = gui
            .store_mut()
            .create(fairygui_core::package::ObjectType::InputText);
        gui.store_mut().set_parent(input, Some(root));
        gui.store_mut().set_size(input, 160.0, 40.0);
        gui.store_mut().set_object_text(input, "123");
        gui.queue_subtree(input);
        input
    };
    app.update();
    app.update();
    let entity = view(&app, input);
    let before = app
        .world()
        .entity(entity)
        .get_ref::<bevy::text::EditableText>()
        .unwrap()
        .last_changed();
    let font = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(
            include_bytes!("../../fairygui-demo/assets/fonts/NotoSansSC-VF.ttf").to_vec(),
        ));
    app.world_mut().resource_mut::<crate::GuiFont>().0 = Some(font.clone());
    app.update();
    assert_eq!(
        app.world()
            .entity(entity)
            .get_ref::<bevy::text::EditableText>()
            .unwrap()
            .last_changed(),
        before
    );
    assert_eq!(
        app.world().get::<TextFont>(entity).unwrap().font,
        bevy::text::FontSource::Handle(font)
    );
    let (_source, url) = add_foreign(&mut app);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_font(input, Some(&url));
    app.update();
    assert_eq!(parts(&app, input).len(), 3);
    assert!(
        app.world()
            .get::<bevy::text::EditableText>(entity)
            .is_none()
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_font(input, None);
    app.update();
    assert!(
        app.world()
            .get::<bevy::text::EditableText>(entity)
            .is_some()
    );
    assert_eq!(app.world().get::<Text>(entity).unwrap().0, "123");
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut().set_object_ubb_enabled(input, true);
        gui.store_mut()
            .set_object_text(input, "[b]Styled input[/b][img]ui://9leh0eyfrpmb6[/img]");
    }
    app.update();
    assert!(app.world().get::<RichTextBlock>(entity).is_some());
    assert!(
        app.world()
            .get::<bevy::text::EditableText>(entity)
            .is_none()
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_ubb_enabled(input, false);
    app.update();
    assert!(app.world().get::<RichTextBlock>(entity).is_none());
    assert!(
        app.world()
            .get::<bevy::text::EditableText>(entity)
            .is_some()
    );
}

#[test]
fn underline_toggle_and_renderer_switch_reclaim_only_owned_decorations() {
    let (mut app, _, root) = fixture();
    let (_foreign, _) = add_foreign(&mut app);
    let owner = child(&app, root, "n10");
    let entity = view(&app, owner);
    let application_child = app
        .world_mut()
        .spawn((Node::default(), ChildOf(entity)))
        .id();
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut().set_object_text(owner, "UI#");
        gui.store_mut().set_object_underline(owner, true);
        gui.store_mut().set_object_ubb_enabled(owner, false);
        gui.store_mut().set_object_html_enabled(owner, false);
    }
    app.update();
    assert!(app.world().get::<Underline>(entity).is_some());
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_underline(owner, false);
    app.update();
    assert!(app.world().get::<Underline>(entity).is_none());
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_underline(owner, true);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_font(owner, Some("ui://BitmapText/BMFontTest"));
    app.update();
    app.update();
    let lines = app
        .world()
        .get::<BitmapDecorations>(entity)
        .unwrap()
        .0
        .clone();
    assert!(!lines.is_empty());
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_html_enabled(owner, true);
    app.update();
    app.update();
    assert!(lines.iter().all(|&e| app.world().get_entity(e).is_err()));
    assert!(app.world().get::<BitmapDecorations>(entity).is_none());
    assert!(app.world().get::<BitmapRichText>(entity).is_some());
    let rich_lines: Vec<_> = app
        .world()
        .get::<Children>(entity)
        .unwrap()
        .iter()
        .filter(|&e| app.world().get::<appearance::TextDecoration>(e).is_some())
        .collect();
    assert!(!rich_lines.is_empty());
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_font(owner, None);
    app.update();
    app.update();
    assert!(
        rich_lines
            .iter()
            .all(|&e| app.world().get_entity(e).is_err())
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_html_enabled(owner, false);
    app.update();
    assert!(app.world().get::<Underline>(entity).is_some());
    assert!(app.world().get::<BitmapDecorations>(entity).is_none());
    assert!(app.world().get_entity(application_child).is_ok());
    assert_eq!(view(&app, owner), entity);
}

fn raster_source(app: &App, entity: Entity) -> bevy::text::FontSource {
    app.world().get::<TextFont>(entity).unwrap().font.clone()
}
fn arrive_font(app: &mut App, handle: &Handle<Font>) {
    app.world_mut()
        .resource_mut::<Assets<Font>>()
        .insert(
            handle.id(),
            Font::from_bytes(
                include_bytes!("../../fairygui-demo/assets/fonts/NotoSansSC-VF.ttf").to_vec(),
            ),
        )
        .unwrap();
}
fn owned_text_parts(app: &App, entity: Entity) -> Vec<Entity> {
    app.world()
        .get::<Children>(entity)
        .map(|c| {
            c.iter()
                .filter(|&e| {
                    app.world().get::<TextSpan>(e).is_some()
                        || app.world().get::<bevy::text::InlineBox>(e).is_some()
                })
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn raster_family_lists_survive_dirty_updates_and_package_fonts_keep_precedence() {
    use bevy::text::FontSource;
    let (mut app, _, root) = fixture();
    let family = "Missing Family, SimHei, sans-serif";
    for name in ["n10", "n12"] {
        let owner = child(&app, root, name);
        let entity = view(&app, owner);
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_object_font(owner, Some(family));
        app.update();
        let expected = FontSource::list([FontSource::families(family), FontSource::default()]);
        assert_eq!(raster_source(&app, entity), expected);
        for part in owned_text_parts(&app, entity) {
            if app.world().get::<TextSpan>(part).is_some() {
                assert_eq!(raster_source(&app, part), expected);
            }
        }
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_object_font_size(owner, 36);
        app.update();
        assert_eq!(
            raster_source(&app, entity),
            expected,
            "dirty sync must preserve chosen family"
        );
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_object_font(owner, Some("ui://Missing/Font.ttf"));
        app.update();
        assert_eq!(raster_source(&app, entity), FontSource::default());
        assert!(
            app.world()
                .get::<TextFontBinding>(entity)
                .unwrap()
                .request
                .is_none()
        );
    }
    let (_foreign, _) = add_foreign(&mut app);
    let owner = child(&app, root, "n10");
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_font(owner, Some("ui://BitmapText/BMFontTest"));
    app.update();
    let entity = view(&app, owner);
    assert!(app.world().get::<BitmapGlyphs>(entity).is_some());
    assert!(
        app.world()
            .get::<TextFontBinding>(entity)
            .unwrap()
            .request
            .is_none()
    );
}

#[test]
fn raster_file_arrival_removal_and_replacement_keep_owners_and_editable_state() {
    use bevy::text::FontSource;
    let (mut app, _, root) = fixture();
    let input = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let input = gui
            .store_mut()
            .create(fairygui_core::package::ObjectType::InputText);
        gui.store_mut().set_parent(input, Some(root));
        gui.store_mut().set_size(input, 180.0, 40.0);
        gui.store_mut().set_object_text(input, "editable");
        gui.queue_subtree(input);
        input
    };
    app.update();
    let owners = [child(&app, root, "n10"), child(&app, root, "n12"), input];
    let views = owners.map(|id| view(&app, id));
    let unrelated = app
        .world_mut()
        .spawn((Node::default(), ChildOf(views[1])))
        .id();
    let editable_tick = app
        .world()
        .entity(views[2])
        .get_ref::<bevy::text::EditableText>()
        .unwrap()
        .last_changed();
    for owner in owners {
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_object_font(owner, Some("fonts/late.ttf"));
    }
    app.update();
    let request: Handle<Font> = app
        .world()
        .resource::<AssetServer>()
        .get_handle("fonts/late.ttf")
        .unwrap();
    assert_eq!(
        app.world()
            .resource::<AssetServer>()
            .get_path(request.id())
            .unwrap()
            .path()
            .to_string_lossy(),
        "fonts/late.ttf"
    );
    assert!(
        views
            .iter()
            .all(|&e| raster_source(&app, e) == FontSource::default())
    );
    let old = owned_text_parts(&app, views[1]);
    assert!(!old.is_empty());
    arrive_font(&mut app, &request);
    app.update();
    let expected = FontSource::Handle(request.clone());
    assert!(views.iter().all(|&e| raster_source(&app, e) == expected));
    assert!(old.iter().all(|&e| app.world().get_entity(e).is_err()));
    let stable = owned_text_parts(&app, views[1]);
    for &part in &stable {
        if app.world().get::<TextSpan>(part).is_some() {
            assert_eq!(raster_source(&app, part), expected);
        }
    }
    for _ in 0..60 {
        app.update();
    }
    assert_eq!(owned_text_parts(&app, views[1]), stable);
    assert_eq!(
        app.world()
            .entity(views[2])
            .get_ref::<bevy::text::EditableText>()
            .unwrap()
            .last_changed(),
        editable_tick
    );
    app.world_mut()
        .resource_mut::<Assets<Font>>()
        .remove(request.id());
    app.update();
    assert!(
        views
            .iter()
            .all(|&e| raster_source(&app, e) == FontSource::default())
    );
    for owner in owners {
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_object_font(owner, Some("fonts/new.otf"));
    }
    app.update();
    let newer: Handle<Font> = app
        .world()
        .resource::<AssetServer>()
        .get_handle("fonts/new.otf")
        .unwrap();
    arrive_font(&mut app, &newer);
    app.update();
    arrive_font(&mut app, &request);
    app.update();
    assert!(
        views
            .iter()
            .all(|&e| raster_source(&app, e) == FontSource::Handle(newer.clone()))
    );
    assert!(app.world().get_entity(unrelated).is_ok());
    assert!(owners.iter().zip(views).all(|(&id, e)| view(&app, id) == e));
    assert_eq!(
        app.world()
            .entity(views[2])
            .get_ref::<bevy::text::EditableText>()
            .unwrap()
            .last_changed(),
        editable_tick
    );
}

#[test]
fn cached_raster_file_loads_on_creation_and_late_results_cannot_restore_disposed_views() {
    use bevy::text::FontSource;
    let (mut app, _, root) = fixture();
    let request: Handle<Font> = app
        .world()
        .resource::<AssetServer>()
        .load("fonts/cached.ttf");
    arrive_font(&mut app, &request);
    let owner = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let owner = gui
            .store_mut()
            .create(fairygui_core::package::ObjectType::Text);
        gui.store_mut().set_parent(owner, Some(root));
        gui.store_mut().set_object_text(owner, "cached");
        gui.store_mut()
            .set_object_font(owner, Some("fonts/cached.ttf"));
        gui.queue_subtree(owner);
        owner
    };
    app.update();
    let entity = view(&app, owner);
    assert_eq!(raster_source(&app, entity), FontSource::Handle(request));
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_font(owner, Some("fonts/disposed.ttf"));
    app.update();
    let late: Handle<Font> = app
        .world()
        .resource::<AssetServer>()
        .get_handle("fonts/disposed.ttf")
        .unwrap();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .remove(owner);
    app.update();
    assert!(app.world().get_entity(entity).is_err());
    arrive_font(&mut app, &late);
    app.update();
    assert!(app.world().get_entity(entity).is_err());
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .entity_of(owner)
            .is_none()
    );
}

#[test]
fn failed_raster_file_keeps_fallback_and_reuses_handle_without_rebuilding_rich_parts() {
    let (mut app, _, root) = fixture();
    app.init_asset_loader::<bevy::text::FontLoader>();
    let owner = child(&app, root, "n12");
    let entity = view(&app, owner);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_font(owner, Some("missing-raster-font/regression.ttf"));
    app.update();
    let request: Handle<Font> = app
        .world()
        .resource::<AssetServer>()
        .get_handle("missing-raster-font/regression.ttf")
        .unwrap();
    let start = std::time::Instant::now();
    while !matches!(
        app.world()
            .resource::<AssetServer>()
            .load_state(request.id()),
        bevy::asset::LoadState::Failed(_)
    ) {
        assert!(
            start.elapsed().as_secs() < 5,
            "missing font did not report failure"
        );
        app.update();
        std::thread::yield_now();
    }
    let stable = owned_text_parts(&app, entity);
    assert!(!stable.is_empty());
    for _ in 0..60 {
        app.update();
    }
    assert_eq!(owned_text_parts(&app, entity), stable);
    assert_eq!(
        raster_source(&app, entity),
        bevy::text::FontSource::default()
    );
    assert_eq!(
        app.world().get::<TextFontBinding>(entity).unwrap().request,
        Some(request)
    );
}

#[test]
fn editable_buffer_initializes_mirrors_user_edits_and_accepts_model_and_translation_updates() {
    let (mut app, _, root) = fixture();
    let owner = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let owner = gui
            .store_mut()
            .create(fairygui_core::package::ObjectType::InputText);
        gui.store_mut().set_parent(owner, Some(root));
        gui.store_mut().set_object_text(owner, "Initial 输入");
        gui.queue_subtree(owner);
        owner
    };
    app.update();
    let entity = view(&app, owner);
    assert_eq!(
        app.world()
            .get::<bevy::text::EditableText>(entity)
            .unwrap()
            .value()
            .to_string(),
        "Initial 输入"
    );
    app.world_mut()
        .get_mut::<bevy::text::EditableText>(entity)
        .unwrap()
        .editor
        .set_text("Typed 编辑");
    app.update();
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
        "Typed 编辑"
    );
    assert_eq!(app.world().get::<Text>(entity).unwrap().0, "Typed 编辑");
    app.world_mut()
        .get_mut::<bevy::text::EditableText>(entity)
        .unwrap()
        .editor
        .set_text("stale editor");
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text(owner, "Model wins");
    app.update();
    assert_eq!(
        app.world()
            .get::<bevy::text::EditableText>(entity)
            .unwrap()
            .value()
            .to_string(),
        "Model wins"
    );
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let mut translations = fairygui_core::translation::TranslationTable::new();
        translations.set("editable-localized", "Translated 输入");
        gui.set_translations(translations);
        gui.store_mut()
            .set_object_text(owner, "!editable-localized!");
    }
    for _ in 0..5 {
        app.update();
    }
    assert_eq!(
        app.world()
            .get::<bevy::text::EditableText>(entity)
            .unwrap()
            .value()
            .to_string(),
        "Translated 输入"
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
        "!editable-localized!"
    );
    let mut translations = fairygui_core::translation::TranslationTable::new();
    translations.set("editable-localized", "New translation");
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .set_translations(translations);
    app.update();
    assert_eq!(
        app.world()
            .get::<bevy::text::EditableText>(entity)
            .unwrap()
            .value()
            .to_string(),
        "New translation"
    );
    app.world_mut()
        .get_mut::<bevy::text::EditableText>(entity)
        .unwrap()
        .queue_edit(bevy::text::TextEdit::SelectAll);
    let tick = app
        .world()
        .entity(entity)
        .get_ref::<bevy::text::EditableText>()
        .unwrap()
        .last_changed();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_font(owner, Some("SimHei"));
    app.update();
    let editable = app
        .world()
        .entity(entity)
        .get_ref::<bevy::text::EditableText>()
        .unwrap();
    assert_eq!(editable.pending_edits.len(), 2); // Initial TextEnd and queued SelectAll, unprocessed in this renderer-free fixture.
    assert_eq!(editable.last_changed(), tick);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_single_line(owner, true);
    app.update();
    assert_eq!(
        app.world().get::<TextLayout>(entity).unwrap().linebreak,
        bevy::text::LineBreak::NoWrap
    );
    assert!(
        !app.world()
            .get::<bevy::text::EditableText>(entity)
            .unwrap()
            .allow_newlines
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_single_line(owner, false);
    app.update();
    assert_eq!(
        app.world().get::<TextLayout>(entity).unwrap().linebreak,
        bevy::text::LineBreak::WordOrCharacter
    );
    assert!(
        app.world()
            .get::<bevy::text::EditableText>(entity)
            .unwrap()
            .allow_newlines
    );
}

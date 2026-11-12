use super::*;

fn fixture() -> (App, GObjectId) {
    let package = FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-core/tests/fixtures/BitmapText.fui"),
    )
    .unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<FuiAsset>()
        .init_asset::<Image>()
        .init_asset::<Font>();
    let handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: package.clone(),
            res_key: "bitmap/Basics".into(),
        });
    let mut gui = GuiRuntime::default();
    let index = gui.create_root(handle, &package, "Demo_Text").unwrap();
    let root = gui.root_object(index).unwrap();
    app.insert_non_send(gui).add_systems(
        Update,
        (
            spawn_pending_roots,
            sync_dirty,
            sync_text_sizes,
            sync_appearance,
        )
            .chain(),
    );
    (app, root)
}
fn id(app: &App, root: GObjectId, name: &str) -> GObjectId {
    app.world()
        .non_send::<GuiRuntime>()
        .store()
        .child_by_name(root, name)
        .unwrap()
}
fn entity(app: &App, id: GObjectId) -> Entity {
    app.world().non_send::<GuiRuntime>().entity_of(id).unwrap()
}
fn glyphs(app: &App, id: GObjectId) -> Vec<Entity> {
    app.world()
        .get::<BitmapGlyphs>(entity(app, id))
        .unwrap()
        .0
        .clone()
}
fn pixels(value: Val) -> f32 {
    if let Val::Px(v) = value {
        v
    } else {
        panic!("pixel coordinate")
    }
}

#[test]
fn scalable_atlas_font_resizes_auto_bounds_and_keeps_its_owner_at_noninteger_dpi() {
    let (mut app, root) = fixture();
    let owner = id(&app, root, "n2");
    app.update();
    app.update();
    app.update();
    let view = entity(&app, owner);
    let before = glyphs(&app, owner);
    assert!(!before.is_empty());
    let first = app.world().get::<Node>(before[0]).unwrap().clone();
    let metrics = *app.world().get::<BitmapTextMetrics>(view).unwrap();
    app.world_mut()
        .get_mut::<ComputedNode>(view)
        .unwrap()
        .inverse_scale_factor = 1.0 / 1.75;
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_font_size(owner, 64);
    for _ in 0..5 {
        app.update();
    }
    let after = glyphs(&app, owner);
    assert_eq!(after.len(), before.len());
    assert!(before.iter().all(|&e| app.world().get_entity(e).is_err()));
    let next = app.world().get::<Node>(after[0]).unwrap();
    assert_eq!(pixels(next.width), pixels(first.width) * 2.0);
    assert_eq!(pixels(next.height), pixels(first.height) * 2.0);
    let scaled = *app.world().get::<BitmapTextMetrics>(view).unwrap();
    assert!((scaled.text_size.width - (metrics.text_size.width - 4.0) * 2.0 - 4.0).abs() <= 2.0);
    let stable = after.clone();
    let size = app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .object(owner)
        .unwrap()
        .size();
    for _ in 0..60 {
        app.update();
    }
    assert_eq!(glyphs(&app, owner), stable);
    assert_eq!(entity(&app, owner), view);
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .object(owner)
            .unwrap()
            .size(),
        size
    );
    assert_eq!(size, scaled.object_size);
}

#[test]
fn missing_and_empty_bitmap_text_never_fall_back_and_dynamic_translation_rebuilds_glyphs() {
    let (mut app, root) = fixture();
    let owner = id(&app, root, "n2");
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text(owner, "");
    app.update();
    app.update();
    let view = entity(&app, owner);
    assert!(app.world().get::<Text>(view).is_none());
    assert!(glyphs(&app, owner).is_empty());
    assert_eq!(
        app.world()
            .get::<BitmapTextMetrics>(view)
            .unwrap()
            .text_size,
        fairygui_core::Size::ZERO
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text(owner, "?😀?");
    app.update();
    app.update();
    assert!(glyphs(&app, owner).is_empty());
    assert!(app.world().get::<Text>(view).is_none());
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let mut table = fairygui_core::translation::TranslationTable::new();
        table.set("sample", "UI");
        gui.set_translations(table);
        gui.store_mut().set_object_text(owner, "!sample!");
    }
    app.update();
    app.update();
    assert_eq!(glyphs(&app, owner).len(), 2);
    let old = glyphs(&app, owner);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text(owner, "UI#");
    app.update();
    app.update();
    assert_eq!(glyphs(&app, owner).len(), 3);
    assert!(old.into_iter().all(|e| app.world().get_entity(e).is_err()));
}

#[test]
fn resizing_a_bitmap_text_box_rewraps_and_trimmed_image_glyphs_use_native_virtual_bounds() {
    let (mut app, root) = fixture();
    let owner = id(&app, root, "n0");
    app.update();
    app.update();
    let old = glyphs(&app, owner);
    assert!(!old.is_empty());
    let first = app.world().get::<Node>(old[0]).unwrap();
    let first_top = pixels(first.top);
    let image = app.world().get::<ImageNode>(old[0]).unwrap();
    let width = pixels(first.width);
    let height = pixels(first.height);
    let rect = image.rect.unwrap();
    assert!(width > 0.0 && height > 0.0);
    // The derived font's design size is 48 and this field uses size 24.
    // Source-cropped glyph nodes occupy half the actual atlas rectangle.
    assert!(
        (width - rect.width() * 0.5).abs() < 0.001 && (height - rect.height() * 0.5).abs() < 0.001
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_size(owner, 35.0, 300.0);
    app.update();
    app.update();
    let next = glyphs(&app, owner);
    assert!(old.iter().all(|&e| app.world().get_entity(e).is_err()));
    assert!(
        next.iter()
            .any(|&e| pixels(app.world().get::<Node>(e).unwrap().top) > first_top + 100.0)
    );
    let path = app
        .world()
        .resource::<AssetServer>()
        .get_path(app.world().get::<ImageNode>(next[0]).unwrap().image.id())
        .unwrap();
    assert!(path.path().to_string_lossy().starts_with("bitmap/Basics"));
}

fn decorations(app: &App, owner: GObjectId) -> Vec<Entity> {
    app.world()
        .get::<BitmapDecorations>(entity(app, owner))
        .unwrap()
        .0
        .clone()
}

#[test]
fn plain_bitmap_underlines_follow_native_lines_color_alpha_resize_empty_and_disposal() {
    for (name, content) in [("n0", "12345\n12"), ("n8", "UI#\nUI")] {
        let (mut app, root) = fixture();
        let owner = id(&app, root, name);
        {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            gui.store_mut().set_object_ubb_enabled(owner, false);
            gui.store_mut().set_object_html_enabled(owner, false);
            gui.store_mut().set_object_underline(owner, true);
            gui.store_mut().set_object_text(owner, content);
            gui.store_mut().set_alpha(root, 0.5);
            gui.store_mut().set_alpha(owner, 0.6);
        }
        app.update();
        app.update();
        let view = entity(&app, owner);
        let application_child = app.world_mut().spawn((Node::default(), ChildOf(view))).id();
        let before = decorations(&app, owner);
        assert!(!before.is_empty());
        let layout = {
            let gui = app.world().non_send::<GuiRuntime>();
            let object = gui.store().object(owner).unwrap();
            let data = object.control().as_text_field().unwrap();
            let assets = app.world().resource::<Assets<FuiAsset>>();
            let asset = assets.get(&gui.asset_handle_for(owner).unwrap()).unwrap();
            let (_, font) =
                text_fonts::resolve_bitmap_font(asset, assets, data.font.as_deref().unwrap())
                    .unwrap();
            fairygui_core::bitmap_text::layout_bitmap_text(&font, data, content, object.size())
        };
        assert_eq!(layout.underlines.len(), before.len());
        for (&part, rect) in before.iter().zip(layout.underlines) {
            let node = app.world().get::<Node>(part).unwrap();
            assert_eq!(
                [
                    pixels(node.left),
                    pixels(node.top),
                    pixels(node.width),
                    pixels(node.height)
                ],
                [rect.x, rect.y, rect.width, rect.height]
            );
            assert!(app.world().get::<ImageNode>(part).is_none());
            assert_eq!(app.world().get::<ChildOf>(part).unwrap().parent(), view);
            assert_eq!(
                app.world().get::<bevy::picking::Pickable>(part).unwrap(),
                &bevy::picking::Pickable::IGNORE
            );
        }
        let red = fairygui_core::color::Color::from_rgba8(240, 30, 90, 255);
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_control_color(owner, red, None);
        app.update();
        for &part in &before {
            assert_eq!(
                app.world().get::<BackgroundColor>(part).unwrap().0,
                backend::apply_alpha(backend::to_bevy_color(red), 0.3)
            );
        }
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_grayed(owner, true);
        for _ in 0..60 {
            app.update();
        }
        assert_eq!(decorations(&app, owner), before);
        let gray = app
            .world()
            .get::<BackgroundColor>(before[0])
            .unwrap()
            .0
            .to_srgba();
        assert!((gray.red - 2.0 / 3.0).abs() < 1e-6 && (gray.alpha - 0.3).abs() < 1e-6);
        {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            assert!(!gui.store_mut().set_object_underline(owner, true));
            gui.store_mut().set_size(owner, 35.0, 250.0);
            gui.store_mut().set_object_font_size(owner, 40);
        }
        app.update();
        app.update();
        let resized = decorations(&app, owner);
        assert!(!resized.is_empty());
        assert!(before.iter().all(|&e| app.world().get_entity(e).is_err()));
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_object_underline(owner, false);
        app.update();
        assert!(decorations(&app, owner).is_empty());
        assert!(resized.iter().all(|&e| app.world().get_entity(e).is_err()));
        {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            gui.store_mut().set_object_underline(owner, true);
            gui.store_mut().set_object_text(owner, "");
        }
        app.update();
        assert!(decorations(&app, owner).is_empty());
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_object_text(owner, "?😀?");
        app.update();
        assert!(decorations(&app, owner).is_empty());
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_object_text(owner, content);
        app.update();
        let final_parts = decorations(&app, owner);
        assert!(!final_parts.is_empty());
        assert!(app.world().get_entity(application_child).is_ok());
        assert_eq!(entity(&app, owner), view);
        app.world_mut().despawn(view);
        assert!(
            final_parts
                .iter()
                .all(|&e| app.world().get_entity(e).is_err())
        );
    }
}

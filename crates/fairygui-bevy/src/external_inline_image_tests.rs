use super::*;
use bevy::render::render_resource::Extent3d;

fn fixture(bitmap: bool, text: &str) -> (App, GObjectId, Entity) {
    fixture_with_loader(bitmap, text, false)
}
fn fixture_with_loader(bitmap: bool, text: &str, real_loader: bool) -> (App, GObjectId, Entity) {
    let bytes: &[u8] = if bitmap {
        include_bytes!("../../fairygui-core/tests/fixtures/BitmapText.fui")
    } else {
        include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui")
    };
    let package = FuiPackage::parse("Basics", bytes).unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<FuiAsset>()
        .init_asset::<Image>()
        .init_asset::<Font>();
    if real_loader {
        app.register_asset_loader(bevy::image::ImageLoader::new(
            bevy::image::CompressedImageFormats::NONE,
        ));
    }
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
    let owner = gui
        .store()
        .child_by_name(root, if bitmap { "n8" } else { "n12" })
        .unwrap();
    gui.store_mut().set_object_ubb_enabled(owner, false);
    gui.store_mut().set_object_html_enabled(owner, true);
    gui.store_mut().set_object_text(owner, text);
    gui.store_mut().set_size(owner, 270.0, 180.0);
    gui.store_mut().set_visible(owner, true);
    gui.store_mut().set_alpha(root, 0.5);
    gui.store_mut().set_alpha(owner, 0.6);
    app.insert_non_send(gui).add_systems(
        Update,
        (
            spawn_pending_roots,
            sync_dirty,
            sync_text_fonts,
            sync_bitmap_rich_text,
            sync_rich_text,
            sync_appearance,
            sync_stage_hierarchy,
            sync_inline_image_hierarchy,
        )
            .chain(),
    );
    app.update();
    app.update();
    let entity = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(owner)
        .unwrap();
    (app, owner, entity)
}
fn parts(app: &App, entity: Entity) -> Vec<Entity> {
    if let Some(parts) = app.world().get::<InlineImageParts>(entity) {
        parts.iter().collect()
    } else {
        let glyphs = &app.world().get::<BitmapGlyphs>(entity).unwrap().0;
        app.world()
            .get::<Children>(entity)
            .unwrap()
            .iter()
            .filter(|e| !glyphs.contains(e) && app.world().get::<ImageNode>(*e).is_some())
            .collect()
    }
}
fn handle(app: &App, path: &str) -> Handle<Image> {
    app.world()
        .resource::<AssetServer>()
        .get_handle(path.to_owned())
        .unwrap()
}
fn arrive(app: &mut App, handle: &Handle<Image>, width: u32, height: u32) {
    let mut image = Image::default();
    image.resize(Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    });
    app.world_mut()
        .resource_mut::<Assets<Image>>()
        .insert(handle.id(), image)
        .unwrap();
}
fn size(app: &App, part: Entity) -> Vec2 {
    let node = app.world().get::<Node>(part).unwrap();
    let px = |v| {
        if let Val::Px(v) = v {
            v
        } else {
            panic!("pixel image size")
        }
    };
    Vec2::new(px(node.width), px(node.height))
}

#[test]
fn deferred_external_texture_reflows_automatic_axes_preserves_explicit_axes_and_stabilizes_both_text_backends()
 {
    for bitmap in [false, true] {
        let (mut app, owner, entity) = fixture(
            bitmap,
            "<a href='event:image'>U<img src='icons/inline.png' width='40'/>I</a>",
        );
        let application_child = app
            .world_mut()
            .spawn((Node::default(), ChildOf(entity)))
            .id();
        let old = parts(&app, entity)[0];
        let texture = handle(&app, "icons/inline.png");
        assert_eq!(size(&app, old), Vec2::new(40.0, 0.0));
        assert_eq!(
            app.world().get::<Visibility>(old),
            Some(&Visibility::Hidden)
        );
        if !bitmap {
            let slot = app.world().get::<InlineImage>(old).unwrap().inline_box;
            assert_eq!(
                app.world().get::<bevy::text::InlineBox>(slot).unwrap().size,
                Vec2::new(42.0, 0.0)
            );
        }
        arrive(&mut app, &texture, 64, 32);
        app.update();
        let ready = parts(&app, entity)[0];
        assert!(app.world().get_entity(old).is_err());
        assert_eq!(size(&app, ready), Vec2::new(40.0, 32.0));
        assert_eq!(
            app.world().get::<Visibility>(ready),
            Some(&Visibility::Inherited)
        );
        assert_eq!(app.world().get::<ImageNode>(ready).unwrap().rect, None);
        let color = app
            .world()
            .get::<ImageNode>(ready)
            .unwrap()
            .color
            .to_srgba();
        assert_eq!((color.red, color.green, color.blue), (1.0, 1.0, 1.0));
        assert!((color.alpha - 0.3).abs() < 1e-6);
        let parent = app.world().get::<ChildOf>(ready).unwrap().parent();
        assert_eq!(
            parent == entity,
            bitmap,
            "ordinary images remain owned siblings; bitmap images are children"
        );
        let path = app
            .world()
            .resource::<AssetServer>()
            .get_path(texture.id())
            .unwrap();
        assert_eq!(
            path.path().to_string_lossy(),
            "icons/inline.png",
            "external sources resolve from the asset root"
        );
        for _ in 0..60 {
            app.update();
        }
        assert_eq!(parts(&app, entity), vec![ready]);
        assert!(app.world().get_entity(application_child).is_ok());
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_object_text(owner, "<img src='icons/inline.png' height='12'/>");
        app.update();
        let resized = parts(&app, entity)[0];
        assert_eq!(size(&app, resized), Vec2::new(64.0, 12.0));
        assert!(app.world().get_entity(ready).is_err());
        // A replaced image descriptor with a different natural height reflows
        // only the unspecified axis, just as a later texture delivery does.
        arrive(&mut app, &texture, 80, 48);
        app.update();
        assert_eq!(size(&app, parts(&app, entity)[0]), Vec2::new(80.0, 12.0));
        assert!(app.world().get_entity(resized).is_err());
        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .remove(texture.id());
        app.update();
        let unavailable = parts(&app, entity)[0];
        assert_eq!(size(&app, unavailable), Vec2::new(0.0, 12.0));
        assert_eq!(
            app.world().get::<Visibility>(unavailable),
            Some(&Visibility::Hidden)
        );
        for _ in 0..10 {
            app.update();
        }
        assert_eq!(parts(&app, entity), vec![unavailable]);
    }
}

#[test]
fn late_old_texture_and_disposed_owner_cannot_restore_obsolete_inline_images() {
    for bitmap in [false, true] {
        let (mut app, owner, entity) = fixture(bitmap, "<img src='icons/old.png' width='10'/>");
        let old_texture = handle(&app, "icons/old.png");
        let old = parts(&app, entity)[0];
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_object_text(owner, "<img src='icons/new.png' width='20'/>");
        app.update();
        assert!(app.world().get_entity(old).is_err());
        let fresh_texture = handle(&app, "icons/new.png");
        arrive(&mut app, &fresh_texture, 90, 45);
        app.update();
        let ready = parts(&app, entity)[0];
        assert_eq!(size(&app, ready), Vec2::new(20.0, 45.0));
        arrive(&mut app, &old_texture, 200, 180);
        app.update();
        assert_eq!(parts(&app, entity), vec![ready]);
        assert_eq!(size(&app, ready), Vec2::new(20.0, 45.0));
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_object_text(owner, "<img src='icons/last.png'/>");
        app.update();
        let last = parts(&app, entity)[0];
        let last_texture = handle(&app, "icons/last.png");
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .remove(owner);
        app.update();
        assert!(app.world().get_entity(entity).is_err());
        assert!(app.world().get_entity(last).is_err());
        arrive(&mut app, &last_texture, 200, 180);
        app.update();
        assert!(
            app.world()
                .non_send::<GuiRuntime>()
                .entity_of(owner)
                .is_none()
        );
        assert!(app.world().get_entity(last).is_err());
    }
}

#[test]
fn zero_fixed_and_automatic_external_axes_use_native_margins_and_ubb_uses_natural_size() {
    for bitmap in [false, true] {
        let (mut app, owner, entity) = fixture(
            bitmap,
            "<img src='icons/axes.png' width='0' height='12'/><img src='icons/axes.png' width='40' height='12'/><img src='icons/axes.png'/>",
        );
        let texture = handle(&app, "icons/axes.png");
        arrive(&mut app, &texture, 64, 32);
        app.update();
        let images = parts(&app, entity);
        assert_eq!(images.len(), 3);
        assert_eq!(
            images.iter().map(|&e| size(&app, e)).collect::<Vec<_>>(),
            vec![
                Vec2::new(0.0, 12.0),
                Vec2::new(40.0, 12.0),
                Vec2::new(64.0, 32.0)
            ]
        );
        if !bitmap {
            let slot = app
                .world()
                .get::<InlineImage>(images[0])
                .unwrap()
                .inline_box;
            assert_eq!(
                app.world().get::<bevy::text::InlineBox>(slot).unwrap().size,
                Vec2::new(0.0, 12.0),
                "zero image width does not reserve horizontal margins"
            );
        }
        {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            gui.store_mut().set_object_ubb_enabled(owner, true);
            gui.store_mut()
                .set_object_text(owner, "[img]icons/axes.png[/img]");
        }
        app.update();
        assert_eq!(size(&app, parts(&app, entity)[0]), Vec2::new(64.0, 32.0));
    }
}

#[test]
fn failed_external_files_keep_reserved_space_hidden_and_reuse_the_failed_handle() {
    for bitmap in [false, true] {
        let (mut app, _, entity) = fixture_with_loader(
            bitmap,
            "<img src='missing-external-inline/regression.png' width='20' height='10'/>",
            true,
        );
        let texture = handle(&app, "missing-external-inline/regression.png");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !matches!(
            app.world()
                .resource::<AssetServer>()
                .load_state(texture.id()),
            bevy::asset::LoadState::Failed(_)
        ) {
            assert!(
                std::time::Instant::now() < deadline,
                "missing file should reach the actual ImageLoader failure state"
            );
            app.update();
            std::thread::yield_now();
        }
        let failed = parts(&app, entity)[0];
        assert_eq!(size(&app, failed), Vec2::new(20.0, 10.0));
        assert_eq!(
            app.world().get::<Visibility>(failed),
            Some(&Visibility::Hidden)
        );
        for _ in 0..60 {
            app.update();
        }
        assert_eq!(parts(&app, entity), vec![failed]);
        assert_eq!(
            handle(&app, "missing-external-inline/regression.png"),
            texture
        );
        assert!(matches!(
            app.world()
                .resource::<AssetServer>()
                .load_state(texture.id()),
            bevy::asset::LoadState::Failed(_)
        ));
    }
}

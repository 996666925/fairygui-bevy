use super::*;

fn fixture(name: &str, content: &str) -> (App, usize, GObjectId, GObjectId, Entity) {
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
    let owner = gui.store().child_by_name(root, name).unwrap();
    gui.store_mut().set_object_ubb_enabled(owner, true);
    gui.store_mut().set_object_text(owner, content);
    gui.store_mut().set_size(owner, 270.0, 180.0);
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
            sync_text_sizes,
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
    (app, index, root, owner, entity)
}

#[test]
fn exported_bitmap_spans_scale_tint_images_and_underlines_and_keep_pool_and_rebuild_ownership() {
    for (name, text, tinted) in [
        (
            "n0",
            "1[size=60][color=#22AA88]2[/color][/size]\n[u]3[/u][img]ui://bmfont01rpmb6[/img]",
            false,
        ),
        (
            "n8",
            "U[size=60][color=#22AA88]I[/color][/size]\n[u]#[/u][img]ui://bmfont01rpmb6[/img]",
            true,
        ),
    ] {
        let (mut app, index, root, owner, entity) = fixture(name, text);
        let application_child = app
            .world_mut()
            .spawn((Node::default(), ChildOf(entity)))
            .id();
        assert!(app.world().get::<Text>(entity).is_none());
        assert!(app.world().get::<ComputedTextBlock>(entity).is_none());
        let glyphs = app.world().get::<BitmapGlyphs>(entity).unwrap().0.clone();
        assert_eq!(glyphs.len(), 3);
        let block = app.world().get::<BitmapRichText>(entity).unwrap();
        let parts = block.parts.clone();
        let image = *parts
            .iter()
            .find(|&&p| app.world().get::<ImageNode>(p).is_some())
            .unwrap();
        let underline = *parts
            .iter()
            .find(|&&p| app.world().get::<appearance::TextDecoration>(p).is_some())
            .unwrap();
        assert_eq!(app.world().get::<ChildOf>(image).unwrap().parent(), entity);
        for &part in glyphs.iter().chain(std::iter::once(&image)) {
            assert_eq!(
                app.world().get::<BackgroundColor>(part).unwrap().0,
                Color::NONE,
                "sampled glyphs/images must keep transparent node backgrounds"
            );
        }
        let path = app
            .world()
            .resource::<AssetServer>()
            .get_path(app.world().get::<ImageNode>(image).unwrap().image.id())
            .unwrap();
        assert!(path.path().to_string_lossy().starts_with("bitmap/Basics"));
        let explicit = app
            .world()
            .get::<appearance::TextTintSource>(glyphs[1])
            .unwrap()
            .color
            .unwrap();
        assert_eq!(
            explicit,
            if tinted {
                fairygui_core::color::Color::from_rgba8(34, 170, 136, 255)
            } else {
                fairygui_core::color::Color::WHITE
            }
        );
        let data = app
            .world()
            .non_send::<GuiRuntime>()
            .store()
            .object(owner)
            .unwrap()
            .control()
            .as_text_field()
            .unwrap()
            .clone();
        if tinted {
            let rect = app
                .world()
                .get::<ImageNode>(glyphs[1])
                .unwrap()
                .rect
                .unwrap();
            let node = app.world().get::<Node>(glyphs[1]).unwrap();
            // The exported BMFontTest's design size is 32.
            assert_eq!(node.width, Val::Px(rect.width() * 60.0 / 32.0));
        }
        {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            gui.store_mut().set_alpha(root, 0.5);
            gui.store_mut().set_alpha(owner, 0.6);
            gui.store_mut().set_grayed(owner, true);
        }
        app.update();
        assert_eq!(app.world().get::<BitmapGlyphs>(entity).unwrap().0, glyphs);
        assert_eq!(
            app.world().get::<BitmapRichText>(entity).unwrap().parts,
            parts
        );
        assert!((app.world().get::<ImageNode>(image).unwrap().color.alpha() - 0.3).abs() < 1e-6);
        assert!(
            (app.world()
                .get::<BackgroundColor>(underline)
                .unwrap()
                .0
                .alpha()
                - 0.3)
                .abs()
                < 1e-6
        );
        {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            let gui = &mut *gui;
            gui.pool.release(&mut gui.store, "bitmap-rich", owner);
        }
        app.update();
        assert_eq!(app.world().get::<ChildOf>(image).unwrap().parent(), entity);
        assert_eq!(
            app.world().get::<Visibility>(entity),
            Some(&Visibility::Hidden)
        );
        {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            let gui = &mut *gui;
            assert_eq!(
                gui.pool.acquire_checked(&gui.store, "bitmap-rich"),
                Some(owner)
            );
            gui.store_mut().set_parent(owner, Some(root));
            gui.store_mut().set_visible(owner, true);
            gui.store_mut().set_grayed(owner, false);
            gui.store_mut().set_size(owner, 100.0, 150.0);
            gui.store_mut()
                .set_object_font_size(owner, data.font_size + 3);
        }
        app.update();
        app.update();
        assert!(
            parts
                .iter()
                .chain(&glyphs)
                .all(|&e| app.world().get_entity(e).is_err())
        );
        assert!(app.world().get_entity(application_child).is_ok());
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_object_text(owner, "");
        app.update();
        assert!(
            app.world()
                .get::<BitmapGlyphs>(entity)
                .unwrap()
                .0
                .is_empty()
        );
        assert_eq!(
            app.world()
                .get::<BitmapTextMetrics>(entity)
                .unwrap()
                .text_size,
            fairygui_core::Size::ZERO
        );
        assert!(app.world().get_entity(application_child).is_ok());
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let mut gui = app.world_mut().remove_non_send::<GuiRuntime>().unwrap();
        gui.remove_root(&mut Commands::new(&mut queue, app.world()), index);
        queue.apply(app.world_mut());
        app.insert_non_send(gui);
        app.update();
        assert!(app.world().get_entity(entity).is_err());
    }
}

fn point(app: &mut App, entity: Entity, logical: Vec2) {
    let computed = app.world().get::<ComputedNode>(entity).unwrap();
    let normalized =
        logical / (computed.size() * computed.inverse_scale_factor()) - Vec2::splat(0.5);
    app.world_mut()
        .entity_mut(entity)
        .insert(bevy::ui::RelativeCursorPosition {
            normalized: Some(normalized),
            cursor_over: true,
        });
}

#[test]
fn bitmap_raw_html_uses_native_stack_size_entity_link_underline_and_strikethrough() {
    let (mut app, _, _, owner, entity) = fixture("n8", "UI");
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut().set_object_ubb_enabled(owner, false);
        gui.store_mut().set_object_html_enabled(owner, true);
        gui.store_mut().set_object_font_size(owner, 32);
        gui.store_mut().set_object_text(owner,"<font size='150%' color='#FF000080'><strike>U</strike></font><a href='event:raw'>I&#35;</a><img src='ui://bmfont01rpmb6' width='1' height='1'/>");
    }
    app.update();
    app.update();
    assert!(app.world().get::<Text>(entity).is_none());
    let glyphs = &app.world().get::<BitmapGlyphs>(entity).unwrap().0;
    assert_eq!(glyphs.len(), 3);
    let tint = app
        .world()
        .get::<appearance::TextTintSource>(glyphs[0])
        .unwrap()
        .color
        .unwrap();
    assert_eq!((tint.r, tint.g, tint.b), (1.0, 0.0, 0.0));
    assert!((tint.a - 128.0 / 255.0).abs() < 1e-6);
    let rect = app
        .world()
        .get::<ImageNode>(glyphs[0])
        .unwrap()
        .rect
        .unwrap();
    assert_eq!(
        app.world().get::<Node>(glyphs[0]).unwrap().width,
        Val::Px(rect.width() * 1.5)
    );
    let block = app.world().get::<BitmapRichText>(entity).unwrap();
    assert_eq!(block.links.len(), 1);
    assert_eq!(block.links[0].1, "event:raw");
    assert_eq!(
        block
            .parts
            .iter()
            .filter(|&&e| app.world().get::<appearance::TextDecoration>(e).is_some())
            .count(),
        2
    );
    let image = *block
        .parts
        .iter()
        .find(|&&e| app.world().get::<ImageNode>(e).is_some())
        .unwrap();
    assert_ne!(
        app.world().get::<Node>(image).unwrap().width,
        Val::Px(1.0),
        "FairyGUI GHtmlImage ignores HTML dimensions for ui:// images"
    );
}

#[test]
fn foreign_inline_image_arrival_and_removal_reflow_the_bitmap_commands_without_a_text_edit() {
    let (mut app, _, _, _, entity) = fixture("n8", "U[img]ui://9leh0eyfrpmb6[/img]I");
    let before = app
        .world()
        .get::<BitmapTextMetrics>(entity)
        .unwrap()
        .text_size;
    let old = app.world().get::<BitmapGlyphs>(entity).unwrap().0.clone();
    assert!(
        app.world()
            .get::<BitmapRichText>(entity)
            .unwrap()
            .parts
            .iter()
            .all(|&e| app.world().get::<ImageNode>(e).is_none())
    );
    let package = FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui"),
    )
    .unwrap();
    let handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package,
            res_key: "foreign/Basics".into(),
        });
    app.update();
    let after = app
        .world()
        .get::<BitmapTextMetrics>(entity)
        .unwrap()
        .text_size;
    assert!(after.width > before.width);
    assert!(old.iter().all(|&e| app.world().get_entity(e).is_err()));
    let part = *app
        .world()
        .get::<BitmapRichText>(entity)
        .unwrap()
        .parts
        .iter()
        .find(|&&e| app.world().get::<ImageNode>(e).is_some())
        .unwrap();
    let path = app
        .world()
        .resource::<AssetServer>()
        .get_path(app.world().get::<ImageNode>(part).unwrap().image.id())
        .unwrap();
    assert!(path.path().to_string_lossy().starts_with("foreign/Basics"));
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .remove(handle.id());
    app.update();
    assert_eq!(
        app.world()
            .get::<BitmapTextMetrics>(entity)
            .unwrap()
            .text_size,
        before
    );
    assert!(app.world().get_entity(part).is_err());
}
fn mouse(app: &mut App, down: bool) {
    let mut input = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
    if down {
        input.press(MouseButton::Left);
    } else {
        input.release(MouseButton::Left);
    }
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
}
#[test]
fn native_bitmap_link_areas_publish_once_and_reject_adjacent_equal_urls_and_stale_content() {
    let (mut app, _, _, owner, entity) = fixture(
        "n8",
        "[url=event:bitmap]U[color=#FF0000][size=40]I[/size][/color]#[/url] [url=event:bitmap]UI[/url]",
    );
    app.init_resource::<ButtonInput<MouseButton>>()
        .add_message::<bevy::input::mouse::MouseWheel>()
        .add_message::<GuiLink>()
        .configure_sets(
            Update,
            (crate::GuiSystems::Input, crate::GuiSystems::Publish).chain(),
        )
        .add_systems(
            Update,
            emit_link_messages.in_set(crate::GuiSystems::Publish),
        );
    crate::input::register(&mut app);
    let mut window = Window {
        focused: true,
        ..default()
    };
    window.set_cursor_position(Some(Vec2::splat(20.0)));
    app.world_mut().spawn((window, bevy::window::PrimaryWindow));
    app.world_mut().entity_mut(entity).insert((
        ComputedNode {
            size: Vec2::new(270.0, 180.0) * 1.75,
            inverse_scale_factor: 1.0 / 1.75,
            ..default()
        },
        bevy::ui::ComputedStackIndex(100),
        bevy::picking::hover::Hovered(true),
        bevy::ui::RelativeCursorPosition::default(),
    ));
    let links = app
        .world()
        .get::<BitmapRichText>(entity)
        .unwrap()
        .links
        .clone();
    assert!(links.len() >= 2);
    let first = links[0].2;
    let second = links.iter().find(|l| l.0 != links[0].0).unwrap().2;
    let center =
        |r: fairygui_core::geom::Rect| Vec2::new(r.x + r.width / 2.0, r.y + r.height / 2.0);
    point(&mut app, entity, center(first));
    mouse(&mut app, true);
    mouse(&mut app, false);
    let messages: Vec<_> = app
        .world_mut()
        .resource_mut::<Messages<GuiLink>>()
        .drain()
        .collect();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].event.target, owner);
    assert_eq!(messages[0].href, "event:bitmap");
    point(&mut app, entity, center(first));
    mouse(&mut app, true);
    point(&mut app, entity, center(second));
    mouse(&mut app, false);
    assert!(
        app.world_mut()
            .resource_mut::<Messages<GuiLink>>()
            .drain()
            .next()
            .is_none()
    );
    point(&mut app, entity, center(first));
    mouse(&mut app, true);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text(owner, "[url=event:new]U[/url]");
    mouse(&mut app, false);
    assert!(
        app.world_mut()
            .resource_mut::<Messages<GuiLink>>()
            .drain()
            .next()
            .is_none()
    );
}

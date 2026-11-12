use super::*;

#[path = "inline_box_tests.rs"]
mod inline_boxes;

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
        .init_asset::<Font>();
    let handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: package.clone(),
            res_key: "ui/Basics".into(),
        });
    let mut gui = GuiRuntime::default();
    let index = gui.create_root(handle, &package, "Demo_Text").unwrap();
    let root = gui.root_object(index).unwrap();
    let plain = gui.store().child_by_name(root, "n10").unwrap();
    gui.store_mut().set_object_text(plain, "plain");
    let text = gui.store().child_by_name(root, "n12").unwrap();
    app.insert_non_send(gui).add_systems(
        Update,
        (
            spawn_pending_roots,
            sync_dirty,
            sync_rich_text,
            sync_appearance,
            sync_stage_hierarchy,
            sync_inline_image_hierarchy,
        )
            .chain(),
    );
    app.update();
    (app, index, text)
}

fn images(app: &App, text: GObjectId) -> Vec<Entity> {
    let entity = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(text)
        .unwrap();
    app.world()
        .get::<InlineImageParts>(entity)
        .map(|parts| parts.iter().collect())
        .unwrap_or_default()
}

fn spans(app: &App, text: GObjectId) -> Vec<Entity> {
    let entity = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(text)
        .unwrap();
    app.world()
        .get::<RichTextBlock>(entity)
        .unwrap()
        .spans
        .clone()
}

#[test]
fn native_richtext_html_mode_survives_ubb_disable_and_maps_entities_percent_sizes_and_strikes() {
    let (mut app, _, owner) = fixture();
    // Runtime switches must be reconciled before the paragraph snapshot sync.
    app.add_systems(
        Update,
        sync_text_fonts.before(sync_rich_text).after(sync_dirty),
    );
    let entity = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(owner)
        .unwrap();
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .object(owner)
            .unwrap()
            .control()
            .as_text_field()
            .unwrap()
            .html_enabled
    );
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut().set_object_ubb_enabled(owner, false);
        gui.store_mut().set_object_font_size(owner, 30);
        gui.store_mut().set_object_text(owner,"<font size='50%' color='#FF000080'><strike>A&amp;B</strike></font><a href='event:raw'>C</a><img src='ui://9leh0eyfrpmb6'/>");
    }
    app.update();
    let parts = spans(&app, owner);
    assert_eq!(app.world().get::<TextSpan>(parts[0]).unwrap().0, "A&B");
    assert_eq!(
        app.world().get::<TextFont>(parts[0]).unwrap().font_size,
        bevy::text::FontSize::Px(15.0)
    );
    let color = app.world().get::<TextColor>(parts[0]).unwrap().0.to_srgba();
    assert_eq!((color.red, color.green, color.blue), (1.0, 0.0, 0.0));
    assert!((color.alpha - 128.0 / 255.0).abs() < 1e-6);
    assert!(
        app.world()
            .get::<bevy::text::Strikethrough>(parts[0])
            .is_some()
    );
    assert!(
        app.world().get::<Underline>(parts[1]).is_some(),
        "links use native default underline"
    );
    let image = images(&app, owner)[0];
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_html_enabled(owner, false);
    app.update();
    assert!(app.world().get::<RichTextBlock>(entity).is_none());
    assert!(
        app.world()
            .get::<Text>(entity)
            .unwrap()
            .0
            .starts_with("<font")
    );
    assert!(app.world().get_entity(image).is_err());
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_ubb_enabled(owner, true);
    app.update();
    assert_eq!(
        app.world()
            .get::<TextSpan>(spans(&app, owner)[0])
            .unwrap()
            .0,
        "A&B",
        "UBB also enables HTML interpretation"
    );
}

#[test]
fn official_inline_image_owns_its_text_and_follows_hidden_reparent_and_pool_reuse() {
    let (mut app, index, text) = fixture();
    let image = images(&app, text)[0];
    let text_entity = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(text)
        .unwrap();
    let root = app
        .world()
        .non_send::<GuiRuntime>()
        .root_object(index)
        .unwrap();
    let root_entity = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(root)
        .unwrap();
    assert_eq!(
        app.world().get::<InlineImageOwner>(image).unwrap().0,
        text_entity
    );
    assert_eq!(
        app.world().get::<ChildOf>(image).unwrap().parent(),
        root_entity
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_visible(text, false);
    app.update();
    assert_eq!(
        app.world().get::<Visibility>(image),
        Some(&Visibility::Hidden)
    );
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let gui = &mut *gui;
        gui.store_mut().set_visible(text, true);
        gui.pool.release(&mut gui.store, "ui://text", text);
    }
    app.update();
    assert!(app.world().get::<ChildOf>(image).is_none());
    assert!(app.world().get_entity(image).is_ok());
    assert_eq!(
        app.world().get::<Visibility>(image),
        Some(&Visibility::Hidden)
    );
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let gui = &mut *gui;
        let pooled = gui.pool.acquire_checked(&gui.store, "ui://text").unwrap();
        assert_eq!(pooled, text);
        gui.store_mut().set_parent(text, Some(root));
        gui.store_mut().set_visible(text, true);
    }
    app.update();
    assert_eq!(images(&app, text), vec![image]);
    assert_eq!(
        app.world().get::<ChildOf>(image).unwrap().parent(),
        root_entity
    );
    assert_eq!(
        app.world().get::<Visibility>(image),
        Some(&Visibility::Inherited)
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .attach_to_stage(text);
    app.update();
    assert!(
        app.world().get::<ChildOf>(image).is_none(),
        "standalone rich-text roots have sibling root images"
    );
    assert_eq!(images(&app, text), vec![image]);
}

#[test]
fn same_frame_reparent_and_old_root_removal_preserves_owned_siblings() {
    let (mut app, index, text) = fixture();
    let image = images(&app, text)[0];
    let text_entity = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(text)
        .unwrap();
    let new_parent = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let owner = gui
            .store_mut()
            .create(fairygui_core::package::ObjectType::Component);
        gui.store_mut().attach_to_stage(owner);
        owner
    };
    let new_entity = app
        .world_mut()
        .spawn((
            GObjectRef(new_parent),
            Node::default(),
            Visibility::Inherited,
        ))
        .id();
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.entities.insert(new_parent, new_entity);
        gui.store_mut().set_parent(text, Some(new_parent));
    }
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let mut gui = app.world_mut().remove_non_send::<GuiRuntime>().unwrap();
    gui.remove_root(&mut Commands::new(&mut queue, app.world()), index);
    queue.apply(app.world_mut());
    app.insert_non_send(gui);
    assert!(app.world().get_entity(text_entity).is_ok());
    assert!(app.world().get_entity(image).is_ok());
    assert_eq!(
        app.world().get::<ChildOf>(image).unwrap().parent(),
        new_entity
    );
    app.update();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .remove(text);
    app.update();
    assert!(app.world().get_entity(text_entity).is_err());
    assert!(app.world().get_entity(image).is_err());
}

#[test]
fn despawning_text_entity_reclaims_sibling_images_through_linked_ownership() {
    let (mut app, _, text) = fixture();
    let image = images(&app, text)[0];
    let entity = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(text)
        .unwrap();
    app.world_mut().entity_mut(entity).despawn();
    assert!(app.world().get_entity(image).is_err());
}

#[test]
fn rich_text_edits_replace_owned_parts_and_preserve_object_children() {
    let (mut app, _, text) = fixture();
    let old_images = images(&app, text);
    let old_spans = spans(&app, text);
    let (object, entity) = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let object = gui
            .store_mut()
            .create(fairygui_core::package::ObjectType::Graph);
        gui.store_mut().set_parent(object, Some(text));
        (object, gui.entity_of(text).unwrap())
    };
    let child = app
        .world_mut()
        .spawn((
            GObjectRef(object),
            Node::default(),
            Visibility::Inherited,
            ChildOf(entity),
        ))
        .id();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .entities
        .insert(object, child);
    for content in [
        "plain text",
        "[b]new[/b][img]ui://9leh0eyfrpmb6[/img][img]ui://9leh0eyfrpmb6[/img]",
        "",
        "[color=#ff0000]red[/color]",
    ] {
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_object_text(text, content);
        app.update();
        let count = if content.contains("[img]") { 2 } else { 0 };
        assert_eq!(images(&app, text).len(), count);
        assert!(app.world().get_entity(child).is_ok());
        assert_eq!(app.world().get::<ChildOf>(child).unwrap().parent(), entity);
        for image in images(&app, text) {
            assert_eq!(
                app.world().get::<Node>(image).unwrap().display,
                Display::None,
                "new images wait for run layout"
            );
        }
    }
    assert!(
        old_images
            .into_iter()
            .chain(old_spans)
            .all(|entity| app.world().get_entity(entity).is_err())
    );
    let current = spans(&app, text);
    app.update();
    assert_eq!(
        spans(&app, text),
        current,
        "unchanged content does not rebuild parts"
    );
}

#[test]
fn image_quad_matches_text_transform_under_pivot_rotation_mirroring_and_nonuniform_scale() {
    let text_size = Vec2::new(200.0, 100.0);
    let local = Vec2::new(10.0, 20.0);
    let image_size = Vec2::new(30.0, 40.0);
    for rotation in [0.0, 90.0, -37.0] {
        for scale in [Vec2::ONE, Vec2::new(2.0, 0.5), Vec2::new(-1.5, 2.0)] {
            let text = UiTransform {
                translation: Val2::px(-125.0, 150.0),
                scale,
                rotation: Rot2::degrees(rotation),
            };
            let image = image_transform(&text, text_size, local, image_size);
            for corner in [
                Vec2::ZERO,
                Vec2::new(30.0, 0.0),
                image_size,
                Vec2::new(0.0, 40.0),
            ] {
                let expected = text_size * 0.5
                    + Vec2::new(-125.0, 150.0)
                    + text.rotation * (scale * (local + corner - text_size * 0.5));
                let actual = local
                    + image_size * 0.5
                    + Vec2::new(px(image.translation.x), px(image.translation.y))
                    + image.rotation * (image.scale * (corner - image_size * 0.5));
                assert!(
                    actual.abs_diff_eq(expected, 0.0001),
                    "{rotation} {scale:?} {corner:?}: {actual:?} != {expected:?}"
                );
            }
        }
    }
}

#[test]
fn image_order_and_tint_follow_owner_without_rebuilding_parts() {
    let (mut app, _, text) = fixture();
    let image = images(&app, text)[0];
    let entity = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(text)
        .unwrap();
    app.world_mut().entity_mut(entity).insert(ZIndex(13));
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_alpha(text, 0.25);
    app.update();
    assert_eq!(images(&app, text), vec![image]);
    assert_eq!(app.world().get::<ZIndex>(image), Some(&ZIndex(13)));
    let tint = app.world().get::<ImageNode>(image).unwrap().color;
    assert_eq!(tint.to_srgba().red, 1.0, "image ignores text color");
    assert_eq!(tint.alpha(), 0.25);
}

#[test]
fn initially_plain_ubb_text_can_gain_images_and_missing_image_addresses_do_not_leave_nodes() {
    let (mut app, index, _) = fixture();
    let text = {
        let gui = app.world().non_send::<GuiRuntime>();
        let root = gui.root_object(index).unwrap();
        gui.store().child_by_name(root, "n10").unwrap()
    };
    let entity = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(text)
        .unwrap();
    assert!(app.world().get::<RichTextBlock>(entity).is_some());
    for (content, count) in [
        ("plain", 0),
        ("[img]ui://9leh0eyfrpmb6[/img]", 1),
        ("[img]ui://does-not-exist[/img]", 0),
        ("[img]ui://9leh0eyft9fj5f[/img]", 1), // Package movie clips are valid animated inline images.
    ] {
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_object_text(text, content);
        app.update();
        assert_eq!(images(&app, text).len(), count);
    }
}

#[test]
fn reparented_text_rebuilds_using_its_original_asset_directory_even_with_a_duplicate_package_id() {
    let (mut app, _, text) = fixture();
    let original_entity = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(text)
        .unwrap();
    let old_images = images(&app, text);
    let package = FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui"),
    )
    .unwrap();
    let handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: package.clone(),
            res_key: "other/Basics".into(),
        });
    let index = app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .create_root(handle, &package, "Demo_Text")
        .unwrap();
    let root = app
        .world()
        .non_send::<GuiRuntime>()
        .root_object(index)
        .unwrap();
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut().set_parent(text, Some(root));
        gui.store_mut()
            .set_object_text(text, "Changed [img]ui://9leh0eyfrpmb6[/img]");
    }
    app.update();
    assert_eq!(
        app.world().non_send::<GuiRuntime>().entity_of(text),
        Some(original_entity)
    );
    assert!(
        old_images
            .into_iter()
            .all(|entity| app.world().get_entity(entity).is_err())
    );
    let image = images(&app, text)[0];
    let asset = app.world().get::<ImageNode>(image).unwrap().image.id();
    let path = app
        .world()
        .resource::<AssetServer>()
        .get_path(asset)
        .unwrap();
    assert!(
        path.path().to_string_lossy().starts_with("ui/Basics"),
        "{path}"
    );
}

#[test]
fn external_images_resolve_both_url_forms_retry_loading_and_disappear_on_unload() {
    let (mut app, _, text) = fixture();
    let package = FuiPackage::parse(
        "Bag",
        include_bytes!("../../fairygui-core/tests/fixtures/Bag.fui"),
    )
    .unwrap();
    let (index, item) = package
        .items()
        .iter()
        .enumerate()
        .find(|(index, item)| !item.name.is_empty() && package.sprite_for(*index).is_some())
        .unwrap();
    let canonical = format!("ui://{}{}", package.id(), item.id);
    let named = format!("ui://{}/{}", package.name(), item.name);
    let content = format!("[img]{canonical}[/img][img]{named}[/img]");
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text(text, &content);
    app.update();
    assert!(images(&app, text).is_empty());
    let unchanged = spans(&app, text);
    app.update();
    assert_eq!(
        spans(&app, text),
        unchanged,
        "missing dependencies do not rebuild every frame"
    );
    let handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: package.clone(),
            res_key: "external/Bag".into(),
        });
    app.update();
    let loaded = images(&app, text);
    assert_eq!(loaded.len(), 2);
    let expected = package.sprite_for(index).unwrap();
    for image in &loaded {
        let node = app.world().get::<ImageNode>(*image).unwrap();
        let path = app
            .world()
            .resource::<AssetServer>()
            .get_path(node.image.id())
            .unwrap();
        assert!(
            path.path().to_string_lossy().starts_with("external/Bag"),
            "{path}"
        );
        assert_eq!(
            node.rect.unwrap().min,
            Vec2::new(expected.rect.x, expected.rect.y)
        );
    }
    let unchanged = spans(&app, text);
    app.update();
    assert_eq!(spans(&app, text), unchanged);
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .remove(handle.id());
    app.update();
    assert!(images(&app, text).is_empty());
    assert!(
        loaded
            .iter()
            .all(|image| app.world().get_entity(*image).is_err())
    );
}

#[test]
fn foreign_urls_never_resolve_a_coincidentally_matching_local_item() {
    let (app, _, _) = fixture();
    let assets = app.world().resource::<Assets<FuiAsset>>();
    let (_, own) = assets.iter().next().unwrap();
    let (index, item) = own
        .package
        .items()
        .iter()
        .enumerate()
        .find(|(index, item)| !item.name.is_empty() && own.package.sprite_for(*index).is_some())
        .unwrap();
    let name = &item.name;
    assert_eq!(image_source(own, assets, name).unwrap().1, index);
    assert!(image_source(own, assets, &format!("ui://Foreign/{name}")).is_none());
    assert!(image_source(own, assets, "ui://bad").is_none());
}

#[test]
fn linked_image_and_nested_styles_keep_their_link_identity_but_adjacent_tags_do_not() {
    let segments = crate::text::layout_ubb(
        "[url=event:xx]A[b]B[/b][img]icon[/img][/url][url=event:xx]C[/url]",
    );
    assert_eq!(segments.len(), 4);
    let identities: Vec<_> = segments
        .iter()
        .map(|segment| match segment {
            crate::text::RichSegment::Text { url, link_id, .. }
            | crate::text::RichSegment::Image { url, link_id, .. } => {
                assert_eq!(url.as_deref(), Some("event:xx"));
                link_id.unwrap()
            }
        })
        .collect();
    assert_eq!(identities[0], identities[1]);
    assert_eq!(identities[0], identities[2]);
    assert_ne!(identities[0], identities[3]);
}

// Use Bevy's public shaping entry point to populate the private section table.
// Empty text avoids font/raster dependencies; the tests supply explicit run
// rectangles. The window smoke exercises real measured glyph runs as well.
fn computed_block(entities: &[Entity]) -> ComputedTextBlock {
    use bevy::text::*;
    let mut block = ComputedTextBlock::default();
    let font = TextFont::default();
    TextPipeline::default()
        .update_buffer(
            &Assets::default(),
            entities.iter().map(|entity| {
                (
                    *entity,
                    1,
                    TextElement::Text {
                        text: "",
                        font: &font,
                        color: Color::WHITE,
                        line_height: LineHeight::default(),
                        letter_spacing: LetterSpacing::default(),
                    },
                )
            }),
            LineBreak::WordBoundary,
            Justify::Left,
            TextBounds::UNBOUNDED,
            2.0,
            &mut block,
            &mut FontCx::default(),
            &mut LayoutCx::default(),
            Vec2::new(800.0, 600.0),
            RemSize::default(),
        )
        .unwrap();
    block
}

fn pointer_fixture() -> (App, Entity, Entity, GObjectId) {
    use bevy::picking::hover::Hovered;
    use bevy::ui::{ComputedStackIndex, RelativeCursorPosition};
    let (mut app, _, text) = fixture();
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
    let window = app
        .world_mut()
        .spawn((window, bevy::window::PrimaryWindow))
        .id();
    let entity = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(text)
        .unwrap();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text(
            text,
            "[url=event:xx]A[b]B[/b]\nC[/url] gap [url=event:xx]D[/url][url=event:xx]E[/url]",
        );
    app.update();
    let spans = spans(&app, text);
    assert_eq!(spans.len(), 6);
    let block = computed_block(&spans);
    let runs = [
        (0, Rect::new(0.0, 0.0, 30.0, 20.0)),
        (1, Rect::new(30.0, 0.0, 60.0, 20.0)),
        (2, Rect::new(0.0, 40.0, 30.0, 60.0)),
        (3, Rect::new(70.0, 0.0, 100.0, 20.0)),
        (4, Rect::new(110.0, 0.0, 140.0, 20.0)),
        (5, Rect::new(140.0, 0.0, 170.0, 20.0)),
    ]
    .into_iter()
    .map(|(section_index, bounds)| bevy::text::RunGeometry {
        section_index,
        bounds,
        ..default()
    })
    .collect();
    app.world_mut().entity_mut(entity).insert((
        block,
        bevy::text::TextLayoutInfo {
            scale_factor: 2.0,
            run_geometry: runs,
            ..default()
        },
        ComputedNode {
            size: Vec2::new(400.0, 200.0),
            padding: bevy::sprite::BorderRect {
                min_inset: Vec2::new(10.0, 20.0),
                ..default()
            },
            inverse_scale_factor: 0.5,
            ..default()
        },
        ComputedStackIndex(10),
        Hovered(true),
        RelativeCursorPosition::default(),
    ));
    point_at(&mut app, entity, Vec2::new(15.0, 10.0));
    (app, window, entity, text)
}

fn point_at(app: &mut App, entity: Entity, physical: Vec2) {
    let computed = app.world().get::<ComputedNode>(entity).unwrap();
    let normalized =
        (physical + computed.content_inset().min_inset) / computed.size() - Vec2::splat(0.5);
    app.world_mut()
        .entity_mut(entity)
        .insert(bevy::ui::RelativeCursorPosition {
            normalized: Some(normalized),
            cursor_over: true,
        });
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

fn link_events(app: &mut App) -> Vec<fairygui_core::object::UiEvent> {
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .drain_events()
        .filter(|event| event.kind == fairygui_core::object::EventKind::Link)
        .collect()
}

#[test]
fn link_capture_spans_nested_styles_and_lines_and_publishes_verbatim_payload_once() {
    let (mut app, _, entity, text) = pointer_fixture();
    mouse(&mut app, true);
    point_at(&mut app, entity, Vec2::new(45.0, 10.0));
    mouse(&mut app, false);
    let events = link_events(&mut app);
    assert_eq!(events.len(), 2, "text then parent");
    assert_eq!(events[0].target, text);
    assert!(events[0].is_at_target());
    assert!(events[1].bubbling);
    for event in events {
        assert_eq!(
            event.data,
            fairygui_core::object::EventData::Text("event:xx".into())
        );
        assert_eq!(
            event.pointer,
            Some(fairygui_core::geom::Point::new(20.0, 20.0))
        );
    }
    let messages: Vec<_> = app
        .world_mut()
        .resource_mut::<Messages<GuiLink>>()
        .drain()
        .collect();
    assert_eq!(
        messages.len(),
        1,
        "published during the release frame, once per activation"
    );
    assert_eq!(messages[0].href, "event:xx");
    point_at(&mut app, entity, Vec2::new(15.0, 50.0));
    mouse(&mut app, true);
    mouse(&mut app, false);
    assert_eq!(link_events(&mut app).len(), 2);
    for point in [Vec2::new(15.0, 30.0), Vec2::new(85.0, 10.0)] {
        point_at(&mut app, entity, point);
        mouse(&mut app, true);
        mouse(&mut app, false);
        assert!(
            link_events(&mut app).is_empty(),
            "line gap and plain text are not links"
        );
    }
}

#[test]
fn linked_image_box_uses_the_same_capture_and_transformed_content_coordinates() {
    let (mut app, _, entity, text) = pointer_fixture();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text(
            text,
            "[url=https://example.com/path][img]ui://9leh0eyfrpmb6[/img][/url]",
        );
    app.update();
    let image = images(&app, text)[0];
    let inline_box = app.world().get::<InlineImage>(image).unwrap().inline_box;
    let block = computed_block(&[inline_box]);
    app.world_mut().entity_mut(entity).insert((
        block,
        bevy::text::TextLayoutInfo {
            scale_factor: 2.0,
            inline_boxes: vec![(
                inline_box,
                InlineBoxKind::InFlow,
                Rect::new(0.0, 0.0, 30.0, 20.0),
            )],
            ..default()
        },
    ));
    for scale in [Vec2::ONE, Vec2::new(-1.2, 0.7)] {
        let transform = UiGlobalTransform::from(bevy::math::Affine2::from_scale_angle_translation(
            scale,
            0.6,
            Vec2::new(120.0, 90.0),
        ));
        let computed = app.world().get::<ComputedNode>(entity).unwrap();
        let local = Vec2::new(15.0, 10.0) + computed.content_box().min;
        let physical_cursor = transform.transform_point2(local);
        let normalized = computed
            .normalize_point(transform, physical_cursor)
            .unwrap();
        app.world_mut()
            .entity_mut(entity)
            .insert(bevy::ui::RelativeCursorPosition {
                normalized: Some(normalized),
                cursor_over: true,
            });
        mouse(&mut app, true);
        mouse(&mut app, false);
        let events = link_events(&mut app);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].target, text);
        assert_eq!(
            events[0].data,
            fairygui_core::object::EventData::Text("https://example.com/path".into())
        );
    }
    assert!(
        app.world().get::<ImageNode>(image).is_some(),
        "routing URLs does not navigate or destroy the view"
    );
}

#[test]
fn links_reject_different_same_href_tags_drag_hidden_offstage_occlusion_and_stale_content() {
    for case in 0..8 {
        let (mut app, window, entity, text) = pointer_fixture();
        if case == 0 {
            point_at(&mut app, entity, Vec2::new(125.0, 10.0));
        }
        mouse(&mut app, true);
        match case {
            0 => point_at(&mut app, entity, Vec2::new(155.0, 10.0)),
            1 => app
                .world_mut()
                .get_mut::<Window>(window)
                .unwrap()
                .set_cursor_position(Some(Vec2::splat(100.0))),
            2 => app
                .world_mut()
                .non_send_mut::<GuiRuntime>()
                .store_mut()
                .set_visible(text, false),
            3 => {
                app.world_mut()
                    .non_send_mut::<GuiRuntime>()
                    .store_mut()
                    .detach_from_parent(text);
            }
            4 => {
                let overlay = {
                    let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
                    let overlay = gui
                        .store_mut()
                        .create(fairygui_core::package::ObjectType::Component);
                    gui.store_mut().attach_to_stage(overlay);
                    overlay
                };
                app.world_mut().spawn((
                    GObjectRef(overlay),
                    bevy::picking::hover::Hovered(true),
                    ComputedNode::default(),
                    bevy::ui::ComputedStackIndex(20),
                ));
            }
            5 => {
                app.world_mut()
                    .non_send_mut::<GuiRuntime>()
                    .store_mut()
                    .set_object_text(text, "[url=event:changed]A[/url]");
            }
            6 => {
                // Returning to the same source must not revive an old capture.
                let original = app
                    .world()
                    .non_send::<GuiRuntime>()
                    .store()
                    .object(text)
                    .unwrap()
                    .control()
                    .as_text_field()
                    .unwrap()
                    .text
                    .clone();
                app.world_mut()
                    .non_send_mut::<GuiRuntime>()
                    .store_mut()
                    .set_object_text(text, "changed");
                app.update();
                app.world_mut()
                    .non_send_mut::<GuiRuntime>()
                    .store_mut()
                    .set_object_text(text, &original);
                app.update();
            }
            7 => {
                let package = FuiPackage::parse(
                    "MainMenu",
                    include_bytes!("../../fairygui-core/tests/fixtures/MainMenu.fui"),
                )
                .unwrap();
                app.world_mut()
                    .non_send_mut::<GuiRuntime>()
                    .show_window(
                        &package,
                        "Main",
                        true,
                        fairygui_core::geom::Size::new(800.0, 600.0),
                    )
                    .unwrap();
            }
            _ => unreachable!(),
        }
        mouse(&mut app, false);
        assert!(link_events(&mut app).is_empty(), "case {case}");
    }
}

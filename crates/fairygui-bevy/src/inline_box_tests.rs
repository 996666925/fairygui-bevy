use super::*;
use bevy::ecs::system::{RunSystemOnce, SystemState};
use bevy::text::*;

/// Shape the actual runtime paragraph through Bevy's TextReader and Parley,
/// including rasterized glyphs when present. The tracked demo font makes this
/// independent of installed OS fonts. Only the UI frame is supplied by the test;
/// window smoke also runs the complete Bevy UI layout and clipping schedules.
fn shape(app: &mut App, text: GObjectId, width: f32, scale: f32, padding: Vec2) -> Entity {
    let root = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(text)
        .unwrap();
    let font = Font::from_bytes(
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../fairygui-demo/assets/fonts/NotoSansSC-VF.ttf"
        ))
        .unwrap(),
    );
    let mut fonts = FontCx::default();
    fonts.collection.register_fonts(font.data, None);
    let family = fonts
        .collection
        .family_id("SimHei")
        .expect("tracked font family");
    let family = fonts.collection.family_name(family).unwrap().to_owned();
    let mut query = app.world_mut().query::<&mut TextFont>();
    for mut font in query.iter_mut(app.world_mut()) {
        font.font = FontSource::Family(family.clone().into());
    }
    let text_layout = *app.world().get::<TextLayout>(root).unwrap();
    let mut computed = ComputedTextBlock::default();
    let mut layout = TextLayoutInfo::default();
    let mut pipeline = TextPipeline::default();
    // The Bevy UI adapter supplies physical content bounds to this API.
    let bounds = TextBounds::new_horizontal(width * scale);
    {
        let mut state = SystemState::<TextReader<Text>>::new(app.world_mut());
        let mut reader = state.get_mut(app.world_mut()).unwrap();
        pipeline
            .update_buffer(
                &Assets::default(),
                reader.iter(root),
                text_layout.linebreak,
                text_layout.justify,
                bounds,
                scale,
                &mut computed,
                &mut fonts,
                &mut LayoutCx::default(),
                Vec2::new(1280.0, 720.0),
                RemSize::default(),
            )
            .unwrap();
    }
    pipeline
        .update_text_layout_info(
            &mut layout,
            &mut FontAtlasSet::default(),
            &mut Assets::default(),
            &mut computed,
            &mut ScaleCx::default(),
            bounds,
            text_layout.justify,
            FontHinting::default(),
        )
        .unwrap();
    // Bevy UI converts only the paragraph size back to logical pixels.
    layout.size /= scale;
    app.world_mut().entity_mut(root).insert((
        computed,
        layout,
        ComputedNode {
            size: Vec2::new(width, 300.0) * scale + padding,
            padding: bevy::sprite::BorderRect {
                min_inset: padding,
                ..default()
            },
            inverse_scale_factor: scale.recip(),
            ..default()
        },
    ));
    root
}

fn source(app: &App, url: &str) -> InlineImageSource {
    let assets = app.world().resource::<Assets<FuiAsset>>();
    let (_, own) = assets.iter().next().unwrap();
    inline_image(
        own,
        assets,
        app.world().resource::<AssetServer>(),
        app.world().resource::<Assets<Image>>(),
        url,
        Default::default(),
    )
    .unwrap()
}

#[test]
fn package_movie_clips_animate_as_rich_text_inline_images() {
    let (mut app, _, text) = fixture();
    let package = FuiPackage::parse(
        "Bag",
        include_bytes!("../../fairygui-demo/assets/ui/Bag.fui"),
    )
    .unwrap();
    let (index, clip) = package
        .items()
        .iter()
        .enumerate()
        .find_map(|(index, item)| {
            if item.item_type != fairygui_core::package::PackageItemType::MovieClip {
                return None;
            }
            let clip = package
                .load_movie_clip(index, &fairygui_core::package::LoadContext::none())
                .ok()?;
            (clip.frame_count() > 1).then_some((index, clip))
        })
        .expect("Bag fixture contains a multi-frame movie clip");
    let url = format!("ui://{}{}", package.id(), package.item(index).unwrap().id);
    let _bag_handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: package.clone(),
            res_key: "ui/Bag".into(),
        });
    {
        let assets = app.world().resource::<Assets<FuiAsset>>();
        let own = assets
            .iter()
            .find_map(|(_, asset)| (asset.package.id() != package.id()).then_some(asset))
            .unwrap();
        assert!(matches!(
            image_identity(
                own,
                assets,
                app.world().resource::<AssetServer>(),
                app.world().resource::<Assets<Image>>(),
                &url,
            ),
            Some(InlineImageIdentity::PackageMovieClip { .. })
        ));
        assert!(
            inline_image(
                own,
                assets,
                app.world().resource::<AssetServer>(),
                app.world().resource::<Assets<Image>>(),
                &url,
                Default::default(),
            )
            .is_some()
        );
    }
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_ubb_enabled(text, true);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text(text, &format!("[img]{url}[/img]"));
    app.update();
    app.update();

    let image = images(&app, text)[0];
    let player = app
        .world()
        .get::<InlineMovieClipPlayer>(image)
        .expect("inline movie source keeps its player");
    assert_eq!(player.frame_count(), clip.frame_count());
    let first = player.frame();
    for _ in 0..120 {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(100));
        app.world_mut()
            .run_system_once(advance_inline_movie_clips)
            .unwrap();
        if app
            .world()
            .get::<InlineMovieClipPlayer>(image)
            .unwrap()
            .frame()
            != first
        {
            break;
        }
    }
    assert_ne!(
        app.world()
            .get::<InlineMovieClipPlayer>(image)
            .unwrap()
            .frame(),
        first,
        "the inline clip advances while its FairyGUI owner is on stage"
    );
}

#[test]
fn image_only_paragraphs_use_real_box_widths_and_wrap_without_phantom_glyphs() {
    let (mut app, _, text) = fixture();
    let url = "ui://9leh0eyfrpmb6";
    let source = source(&app, url);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text(text, &format!("[img]{url}[/img][img]{url}[/img]"));
    app.update();
    let parts = images(&app, text);
    for scale in [1.0, 1.75, 2.0] {
        for width in [source.canvas.x * 2.0 + 1.0, source.canvas.x * 2.0 - 1.0] {
            let entity = shape(&mut app, text, width, scale, Vec2::ZERO);
            let layout = app.world().get::<TextLayoutInfo>(entity).unwrap();
            assert_eq!(layout.inline_boxes.len(), 2);
            assert!(layout.glyphs.is_empty());
            assert!(layout.run_geometry.is_empty());
            let first = layout.inline_boxes[0].2;
            let second = layout.inline_boxes[1].2;
            for &image in &parts {
                let anchor = app.world().get::<InlineImage>(image).unwrap().inline_box;
                let reserved = app.world().get::<InlineBox>(anchor).unwrap();
                assert_eq!(reserved.kind, InlineBoxKind::InFlow);
                assert_eq!(reserved.size, source.canvas);
                assert!(app.world().get::<TextSpan>(anchor).is_none());
            }
            assert!(first.size().abs_diff_eq(source.canvas * scale, 0.001));
            assert!(second.size().abs_diff_eq(source.canvas * scale, 0.001));
            if width > source.canvas.x * 2.0 {
                assert!((first.min.y - second.min.y).abs() < 0.001);
                assert!((second.min.x - first.max.x).abs() < 0.001);
            } else {
                assert!(second.min.y >= first.max.y - 0.001, "{first:?} {second:?}");
                assert!(second.min.x.abs() < 0.001);
            }
        }
    }
}

#[test]
fn mixed_text_flows_after_the_image_canvas_instead_of_a_font_dependent_placeholder() {
    let (mut app, _, text) = fixture();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text(text, "A[img]ui://9leh0eyfrpmb6[/img][size=48]Z[/size]");
    app.update();
    for scale in [1.0, 2.0] {
        let entity = shape(&mut app, text, 400.0, scale, Vec2::ZERO);
        let layout = app.world().get::<TextLayoutInfo>(entity).unwrap();
        assert_eq!(layout.inline_boxes.len(), 1);
        assert!(!layout.glyphs.is_empty());
        let bounds = layout.inline_boxes[0].2;
        let block = app.world().get::<ComputedTextBlock>(entity).unwrap();
        let trailing = block
            .entities()
            .iter()
            .position(|span| {
                app.world()
                    .get::<TextSpan>(span.entity)
                    .is_some_and(|span| span.0 == "Z")
            })
            .unwrap();
        let run = layout
            .run_geometry
            .iter()
            .find(|run| run.section_index as usize == trailing)
            .unwrap();
        assert!(
            (run.bounds.min.x - bounds.max.x).abs() < 0.001,
            "next text starts after the exact original canvas and native side margins"
        );
    }
}

#[test]
fn trimmed_images_use_the_current_padded_box_and_text_transform_in_the_same_frame() {
    let (mut app, _, text) = fixture();
    let url = {
        let assets = app.world().resource::<Assets<FuiAsset>>();
        let (_, own) = assets.iter().next().unwrap();
        let item = own
            .package
            .items()
            .iter()
            .enumerate()
            .find(|(index, _)| {
                own.package
                    .sprite_for(*index)
                    .is_some_and(|sprite| sprite.offset.x > 0.0 || sprite.offset.y > 0.0)
            })
            .unwrap()
            .1;
        format!("ui://{}{}", own.package.id(), item.id)
    };
    let source = source(&app, &url);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text(text, &format!("A[img]{url}[/img]"));
    app.update();
    let image = images(&app, text)[0];
    let scale = 2.0;
    for width in [400.0, source.canvas.x] {
        let entity = shape(&mut app, text, width, scale, Vec2::new(12.0, 24.0));
        let transform = UiGlobalTransform::from(bevy::math::Affine2::from_scale_angle_translation(
            Vec2::new(-1.1, 0.75),
            0.4,
            Vec2::new(800.0, 350.0),
        ));
        app.world_mut().entity_mut(entity).insert(transform);
        app.world_mut()
            .run_system_once(position_inline_images)
            .unwrap();
        app.world_mut()
            .run_system_once(finalize_inline_images)
            .unwrap();
        let computed = app.world().get::<ComputedNode>(entity).unwrap();
        let layout = app.world().get::<TextLayoutInfo>(entity).unwrap();
        let bounds = layout.inline_boxes[0].2;
        let node = app.world().get::<Node>(image).unwrap();
        assert_eq!(node.display, Display::Flex);
        assert_eq!(Vec2::new(px(node.width), px(node.height)), source.size);
        let actual_size = app.world().get::<ComputedNode>(image).unwrap().size();
        assert_eq!(actual_size, source.size * scale);
        let global = app.world().get::<UiGlobalTransform>(image).unwrap();
        for corner in [Vec2::ZERO, source.size, Vec2::new(source.size.x, 0.0)] {
            let expected = transform.transform_point2(
                computed.content_box().min + bounds.min + (source.offset + corner) * scale,
            );
            let actual = global.transform_point2((corner - source.size * 0.5) * scale);
            assert!(
                actual.abs_diff_eq(expected, 0.001),
                "{actual:?} != {expected:?}"
            );
        }
    }
    let entity = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(text)
        .unwrap();
    app.world_mut()
        .get_mut::<TextLayoutInfo>(entity)
        .unwrap()
        .inline_boxes
        .clear();
    app.world_mut()
        .run_system_once(position_inline_images)
        .unwrap();
    app.world_mut()
        .run_system_once(finalize_inline_images)
        .unwrap();
    assert_eq!(
        app.world().get::<Node>(image).unwrap().display,
        Display::None
    );
    assert_eq!(
        app.world().get::<ComputedNode>(image).unwrap().size(),
        Vec2::ZERO
    );
}

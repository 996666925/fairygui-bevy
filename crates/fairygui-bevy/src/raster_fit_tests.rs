use super::*;
use crate::prelude::*;
use bevy::ecs::system::SystemState;

fn fixture(source: &str, rich: bool, loaded: bool) -> (App, Entity, Entity, Handle<Font>) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_asset::<Font>()
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let font = app.world().resource::<Assets<Font>>().reserve_handle();
    if loaded {
        load(&mut app, &font);
    }
    app.insert_resource(GuiFont(Some(font.clone())));
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: FuiPackage::parse(
                "TextTemplate",
                include_bytes!("../../fairygui-core/tests/fixtures/TextTemplate.fui"),
            )
            .unwrap(),
            res_key: "ui/TextTemplate".into(),
        });
    let owner = app
        .world_mut()
        .spawn((
            FairyGuiRoot::new(asset, "Label"),
            GuiText::new(source),
            GuiTextFormat {
                font_size: 36,
                leading: 4,
                auto_size: 3,
                single_line: !rich,
                ubb_enabled: rich,
                ..default()
            },
        ))
        .id();
    app.update();
    let title = SystemState::<GuiQuery>::new(app.world_mut())
        .get(app.world())
        .unwrap()
        .child_by_name(owner, "title")
        .unwrap();
    (app, owner, title, font)
}
fn load(app: &mut App, handle: &Handle<Font>) {
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
fn pixels(app: &App, entity: Entity) -> f32 {
    let FontSize::Px(size) = app.world().get::<TextFont>(entity).unwrap().font_size else {
        panic!("pixel font")
    };
    size
}

/// Shape the projected ECS paragraph, using its actual font sizes/line heights,
/// independently of RasterFit's saved unscaled runs and selected scale.
fn visible_size(app: &mut App, title: Entity) -> Vec2 {
    let mut state = SystemState::<TextReader<Text>>::new(app.world_mut());
    let mut reader = state.get(app.world()).unwrap();
    let runs = reader
        .iter(title)
        .map(|(entity, _, element)| match element {
            TextElement::Text {
                text,
                font,
                line_height,
                letter_spacing,
                ..
            } => Run::Text {
                entity,
                text: text.to_owned(),
                font: TextFont {
                    font: bevy::text::FontSource::Family("Fit Test".into()),
                    ..font.clone()
                },
                height: line_height,
                spacing: letter_spacing,
                metrics: None,
            },
            TextElement::Box(value) => Run::Box {
                entity,
                value: *value,
            },
        })
        .collect();
    let mut measure = TextMeasurer {
        scale_factor: 1.,
        ..default()
    };
    let font = Font::from_bytes(
        include_bytes!("../../fairygui-demo/assets/fonts/NotoSansSC-VF.ttf").to_vec(),
    );
    measure.font_cx.collection.register_fonts(
        font.data,
        Some(parley::fontique::FontInfoOverride {
            family_name: Some("Fit Test"),
            ..default()
        }),
    );
    let bevy::text::LineHeight::Px(line_height) =
        *app.world().get::<bevy::text::LineHeight>(title).unwrap()
    else {
        panic!("pixel line height");
    };
    let leading = (line_height - pixels(app, title)) as i32;
    let size = app.world().get::<GuiTransform>(title).unwrap().size();
    measure
        .shape_raster(
            &Key {
                size,
                gutter: 2.,
                font_size: 36,
                leading,
                native_vertical: true,
                wrap: Some(super::super::wrapping::Config {
                    width: Some((size.x - 4.).max(0.)),
                    wrap: app.world().get::<TextLayout>(title).unwrap().linebreak
                        != LineBreak::NoWrap,
                    no_break_word: true,
                    align: true,
                }),
                commands: None,
                layout: *app.world().get::<TextLayout>(title).unwrap(),
                revision: 0,
                runs,
            },
            1.,
        )
        .unwrap()
        .0
}

#[test]
fn first_frame_shrink_preserves_source_and_requested_size_then_restores_and_stays_stable() {
    const SOURCE: &str = "Long raster text 中文 that must shrink";
    let (mut app, owner, title, _) = fixture(SOURCE, false, true);
    assert!(pixels(&app, title) < 36.);
    assert_eq!(
        app.world().get::<GuiTextFormat>(owner).unwrap().font_size,
        36
    );
    assert_eq!(app.world().get::<Text>(title).unwrap().0, SOURCE);
    assert_eq!(
        app.world().get::<Node>(title).unwrap().padding,
        UiRect::all(Val::Px(2.))
    );
    let box_size = app.world().get::<GuiTransform>(title).unwrap().size();
    assert!(visible_size(&mut app, title).x <= box_size.x - 4. + 0.01);
    app.world_mut()
        .gui_mut(title)
        .unwrap()
        .set_size(Vec2::new(1600., 80.));
    app.update();
    assert_eq!(pixels(&app, title), 36.);
    assert_eq!(
        *app.world().get::<LineHeight>(title).unwrap(),
        LineHeight::Px(40.)
    );
    app.world_mut()
        .gui_mut(title)
        .unwrap()
        .set_size(Vec2::new(100., 80.));
    app.update();
    assert!(pixels(&app, title) < 36.);
    let tick = app
        .world()
        .entity(title)
        .get_ref::<TextFont>()
        .unwrap()
        .last_changed();
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(
        app.world()
            .entity(title)
            .get_ref::<TextFont>()
            .unwrap()
            .last_changed(),
        tick
    );
    let revision = app.world().get::<RasterFit>(title).unwrap().key.revision;
    app.insert_resource(bevy::ui::UiScale(1.25));
    app.update();
    assert_ne!(
        app.world().get::<RasterFit>(title).unwrap().key.revision,
        revision
    );
    assert!(pixels(&app, title) < 36.);
    app.world_mut().get_mut::<GuiText>(owner).unwrap().set("OK");
    app.update();
    assert_eq!(pixels(&app, title), 36.);
    app.world_mut()
        .get_mut::<GuiTextFormat>(owner)
        .unwrap()
        .auto_size = 0;
    app.update();
    assert!(app.world().get::<RasterFit>(title).is_none());
    assert_eq!(pixels(&app, title), 36.);
}

#[test]
fn multiline_rich_text_shrinks_style_sizes_and_leading_but_keeps_inline_boxes_and_links() {
    let (mut app, owner, title, _) = fixture(
        "[url=event:fit][size=60]Big 中文[/size]\nSmall text line\nMore words to wrap[/url]",
        true,
        true,
    );
    app.world_mut()
        .gui_mut(title)
        .unwrap()
        .set_size(Vec2::new(160., 62.));
    app.update();
    let fitted = app.world().get::<RasterFit>(title).unwrap();
    assert!(fitted.scale < 1.);
    let mut count = 0;
    for run in &fitted.key.runs {
        if let Run::Text {
            entity,
            metrics: Some(original),
            text,
            ..
        } = run
            && !text.is_empty()
        {
            assert!(pixels(&app, *entity) < original.size as f32);
            assert!(app.world().get::<Underline>(*entity).is_some());
            count += 1;
        }
    }
    assert_eq!(count, 2);
    let measured = visible_size(&mut app, title);
    assert!(measured.x <= 156.01 && measured.y <= 58.01, "{measured:?}");
    assert_eq!(
        app.world().get::<GuiTextFormat>(owner).unwrap().font_size,
        36
    );
    // A generated image-like inline box keeps its own dimensions during fitting.
    let image = app
        .world_mut()
        .spawn((
            InlineBox {
                size: Vec2::new(22., 14.),
                ..default()
            },
            ChildOf(title),
        ))
        .id();
    app.update();
    assert_eq!(
        app.world().get::<InlineBox>(image).unwrap().size,
        Vec2::new(22., 14.)
    );
    app.world_mut()
        .gui_mut(title)
        .unwrap()
        .set_size(Vec2::new(1600., 400.));
    app.update();
    let mut state = SystemState::<TextReader<Text>>::new(app.world_mut());
    let mut reader = state.get(app.world()).unwrap();
    for (entity, _, element) in reader.iter(title) {
        if let TextElement::Text { .. } = element {
            let original = app.world().get::<FontMetrics>(entity).unwrap();
            assert_eq!(pixels(&app, entity), original.size as f32);
        }
    }
}

#[test]
fn pending_fonts_retry_and_single_tall_lines_follow_native_width_only_policy() {
    let (mut app, _, title, font) = fixture("Late font must shrink into this field", false, false);
    assert_eq!(pixels(&app, title), 36.);
    assert!(app.world().get::<RasterFit>(title).is_none());
    load(&mut app, &font);
    app.update();
    assert!(pixels(&app, title) < 36.);
    app.world_mut().gui_mut(title).unwrap().set_text("OK");
    app.world_mut()
        .gui_mut(title)
        .unwrap()
        .set_size(Vec2::new(1600., 5.));
    app.update();
    assert_eq!(
        pixels(&app, title),
        36.,
        "one tall line alone does not shrink in native Text.SHRINK"
    );
    app.world_mut()
        .gui_mut(title)
        .unwrap()
        .set_text("long text");
    app.world_mut().gui_mut(title).unwrap().set_size(Vec2::ZERO);
    app.update();
    assert!(pixels(&app, title).is_finite());
    assert!(pixels(&app, title) >= 1.);
}

#[test]
fn input_fitting_preserves_editor_value_and_uses_input_padding_instead_of_label_padding() {
    let (mut app, owner, _, _) = fixture("label", false, true);
    let asset = app
        .world()
        .get::<FairyGuiRoot>(owner)
        .unwrap()
        .asset
        .clone();
    let owner = app.world_mut().spawn(FairyGuiRoot::new(asset, "Main")).id();
    app.update();
    let input = SystemState::<GuiQuery>::new(app.world_mut())
        .get(app.world())
        .unwrap()
        .child_by_name(owner, "input")
        .unwrap();
    const SOURCE: &str = "Input 中文 that must shrink";
    app.world_mut().entity_mut(input).insert((
        GuiText::new(SOURCE),
        GuiTextFormat {
            font_size: 36,
            auto_size: 3,
            single_line: true,
            ..default()
        },
    ));
    app.world_mut()
        .gui_mut(input)
        .unwrap()
        .set_size(Vec2::new(120., 30.));
    app.update();
    assert!(pixels(&app, input) < 36.);
    assert_eq!(
        app.world().get::<Node>(input).unwrap().padding,
        UiRect::ZERO
    );
    assert_eq!(
        app.world()
            .get::<bevy::text::EditableText>(input)
            .unwrap()
            .value(),
        SOURCE
    );
    assert_eq!(app.world().get::<GuiText>(input).unwrap().value(), SOURCE);
    assert_eq!(app.world().get::<RasterFit>(input).unwrap().key.gutter, 0.);
}

#[test]
fn native_padding_participates_in_the_shrink_decision_even_when_the_raw_glyphs_fit() {
    let (mut app, _, title, _) = fixture("OK", false, true);
    assert_eq!(pixels(&app, title), 36.);
    let glyph_width = visible_size(&mut app, title).x;
    // Glyphs fit inside the width minus 4px label padding. Native's padded
    // textWidth still exceeds that content width and must trigger shrinking.
    app.world_mut()
        .gui_mut(title)
        .unwrap()
        .set_size(Vec2::new(glyph_width + 6., 80.));
    app.update();
    assert!(pixels(&app, title) < 36.);
}

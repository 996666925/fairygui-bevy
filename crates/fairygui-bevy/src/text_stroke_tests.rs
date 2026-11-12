use super::*;
use crate::{FuiAsset, runtime::*};
use bevy::text::*;

fn fixture() -> (App, fairygui_core::object::GObjectId, Entity) {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<FuiAsset>()
        .init_asset::<Image>()
        .init_asset::<Font>()
        .init_resource::<FontAtlasSet>()
        .init_resource::<GuiFont>();
    let package = fairygui_core::FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-core/tests/fixtures/BitmapText.fui"),
    )
    .unwrap();
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: package.clone(),
            res_key: "ui/Basics".into(),
        });
    let mut gui = GuiRuntime::default();
    let index = gui.create_root(asset, &package, "Demo_Text").unwrap();
    let root = gui.root_object(index).unwrap();
    let owner = gui.store().child_by_name(root, "n10").unwrap();
    gui.store_mut().set_object_font(owner, None);
    gui.store_mut().set_object_ubb_enabled(owner, false);
    gui.store_mut().set_object_text(owner, "O中");
    gui.store_mut().set_object_stroke(owner, 4.);
    app.insert_non_send(gui).add_systems(
        Update,
        (
            spawn_pending_roots,
            spawn_pending_objects,
            sync_dirty,
            sync_text_fonts,
            sync_rich_text,
            sync_appearance,
            sync_stage_hierarchy,
        )
            .chain(),
    );
    install(&mut app);
    app.update();
    app.update();
    let entity = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(owner)
        .unwrap();
    (app, owner, entity)
}

fn shape(app: &mut App, entity: Entity, scale: f32) {
    use bevy::ecs::system::SystemState;
    let mut fonts = FontCx::default();
    let font = Font::from_bytes(
        include_bytes!("../../fairygui-demo/assets/fonts/NotoSansSC-VF.ttf").to_vec(),
    );
    fonts.collection.register_fonts(font.data, None);
    let mut q = app.world_mut().query::<&mut TextFont>();
    for mut font in q.iter_mut(app.world_mut()) {
        font.font = FontSource::Family("SimHei".into());
    }
    let layout = *app.world().get::<TextLayout>(entity).unwrap();
    let mut computed = ComputedTextBlock::default();
    let mut info = TextLayoutInfo::default();
    let mut pipeline = TextPipeline::default();
    let bounds = TextBounds::new_horizontal(400. * scale);
    {
        let mut state = SystemState::<TextReader<Text>>::new(app.world_mut());
        let mut reader = state.get_mut(app.world_mut()).unwrap();
        pipeline
            .update_buffer(
                &Assets::default(),
                reader.iter(entity),
                layout.linebreak,
                layout.justify,
                bounds,
                scale,
                &mut computed,
                &mut fonts,
                &mut LayoutCx::default(),
                Vec2::new(1280., 720.),
                RemSize::default(),
            )
            .unwrap();
    }
    let mut atlases = app.world_mut().remove_resource::<FontAtlasSet>().unwrap();
    pipeline
        .update_text_layout_info(
            &mut info,
            &mut atlases,
            &mut app.world_mut().resource_mut::<Assets<Image>>(),
            &mut computed,
            &mut ScaleCx::default(),
            bounds,
            layout.justify,
            FontHinting::default(),
        )
        .unwrap();
    app.insert_resource(atlases);
    info.size /= scale;
    app.world_mut().entity_mut(entity).insert((computed, info));
}

#[test]
fn composed_surfaces_reuse_opacity_only_edits_and_rebuild_style_filter_selection_and_layout() {
    use fairygui_core::{color::Color as GuiColor, color_matrix::ColorFilterParams};
    let (mut app, owner, entity) = fixture();
    shape(&mut app, entity, 1.);
    app.update();
    let initial = app
        .world()
        .get::<TextStrokeGeometry>(entity)
        .unwrap()
        .surfaces
        .clone();
    assert_eq!(initial.len(), 1);
    let size = app.world().get::<TextLayoutInfo>(entity).unwrap().size;
    for _ in 0..5 {
        app.update();
    }
    assert_eq!(
        app.world()
            .get::<TextStrokeGeometry>(entity)
            .unwrap()
            .surfaces,
        initial
    );
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let parent = gui.store().object(owner).unwrap().parent().unwrap();
        gui.store_mut().set_alpha(parent, 0.5);
        gui.store_mut().set_alpha(owner, 0.5);
    }
    app.update();
    let surface = &app
        .world()
        .get::<TextStrokeGeometry>(entity)
        .unwrap()
        .surfaces[0];
    assert_eq!(surface.image, initial[0].image);
    assert_eq!(surface.alpha, 0.25);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_control_color(owner, GuiColor::new(0., 0., 1., 0.5), None);
    app.update();
    let recolored = app
        .world()
        .get::<TextStrokeGeometry>(entity)
        .unwrap()
        .surfaces[0]
        .image
        .clone();
    assert_ne!(recolored, initial[0].image);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_color_filter(
            owner,
            Some(ColorFilterParams {
                saturation: -1.,
                ..default()
            }),
        );
    app.update();
    let filtered = app
        .world()
        .get::<TextStrokeGeometry>(entity)
        .unwrap()
        .surfaces[0]
        .image
        .clone();
    assert_ne!(filtered, recolored);
    app.world_mut()
        .entity_mut(entity)
        .insert(bevy::text::TextCursorStyle {
            selected_text_color: Some(Color::srgb(0., 1., 0.)),
            ..default()
        });
    app.world_mut()
        .get_mut::<TextLayoutInfo>(entity)
        .unwrap()
        .selection_rects = vec![Rect::from_corners(Vec2::splat(-100.), Vec2::splat(1000.))];
    app.update();
    assert!(
        app.world()
            .get::<TextStrokeGeometry>(entity)
            .unwrap()
            .surfaces
            .is_empty(),
        "custom selected paint retains the normal foreground to avoid double alpha"
    );
    app.world_mut()
        .get_mut::<TextLayoutInfo>(entity)
        .unwrap()
        .selection_rects
        .clear();
    app.update();
    assert_ne!(
        app.world()
            .get::<TextStrokeGeometry>(entity)
            .unwrap()
            .surfaces[0]
            .image,
        filtered
    );
    assert_eq!(
        app.world().get::<TextLayoutInfo>(entity).unwrap().size,
        size
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_stroke(owner, 0.);
    app.update();
    assert!(
        app.world()
            .get::<TextStrokeGeometry>(entity)
            .unwrap()
            .surfaces
            .is_empty()
    );
}

#[test]
fn vector_stroke_cache_distinguishes_width_and_preserves_glyph_counter_and_shape() {
    let data = include_bytes!("../../fairygui-demo/assets/fonts/NotoSansSC-VF.ttf");
    let font = swash::FontRef::from_index(data, 0).unwrap();
    let glyph = font.charmap().map('O');
    let key = StrokeKey {
        font: 1,
        index: 0,
        size: 48f32.to_bits(),
        width: 4f32.to_bits(),
        coords: vec![],
        hint: true,
        smooth: true,
    };
    let mut images = Assets::<Image>::default();
    let mut cache = StrokeCache::default();
    let info = cache.glyph(&key, data, glyph, &mut images).unwrap();
    assert!(info.rect.width() > 20. && info.rect.height() > 25.);
    let page = images.get(info.texture).unwrap();
    let bytes = page.data.as_ref().unwrap();
    let pixel = |x: usize, y: usize| bytes[(y * page.width() as usize + x) * 4 + 3];
    let center = info.rect.center();
    assert_eq!(
        pixel(center.x as usize, center.y as usize),
        0,
        "O counter must remain open"
    );
    assert!(
        bytes
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[3] > 0 && p[3] < 255),
        "antialiasing remains"
    );
    let same = cache.glyph(&key, data, glyph, &mut images).unwrap();
    assert_eq!(same.texture, info.texture);
    assert_eq!(same.rect, info.rect);
    let thick = cache
        .glyph(
            &StrokeKey {
                width: 10f32.to_bits(),
                ..key.clone()
            },
            data,
            glyph,
            &mut images,
        )
        .unwrap();
    assert!(thick.rect.width() > info.rect.width());
    assert_ne!(thick.texture, info.texture);
    assert!(
        cache
            .glyph(&key, data, font.charmap().map(' '), &mut images)
            .is_none()
    );
}

#[test]
fn foreground_replacement_preserves_shadows_and_nonstroked_span_color_and_order() {
    use bevy::{
        ecs::{entity::EntityIndexMap, system::SystemState},
        ui_render::{ExtractedGlyph, ExtractedUiItem, ExtractedUiNode},
    };
    let (mut app, owner, entity) = fixture();
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut().set_object_ubb_enabled(owner, true);
        gui.store_mut()
            .set_object_text(owner, "O[stroke=0]X[/stroke]中");
    }
    app.update();
    shape(&mut app, entity, 1.);
    app.update();
    let surfaces = app
        .world()
        .get::<TextStrokeGeometry>(entity)
        .unwrap()
        .surfaces
        .clone();
    assert_eq!(surfaces.len(), 2);
    let glyphs = app
        .world()
        .get::<TextLayoutInfo>(entity)
        .unwrap()
        .glyphs
        .clone();
    assert_eq!(glyphs.len(), 3);
    let mut render = World::new();
    let shadow = render.spawn_empty().id();
    let first = render.spawn_empty().id();
    let second = render.spawn_empty().id();
    let node = |glyphs: Vec<ExtractedGlyph>| ExtractedUiNode {
        z_order: 4.06,
        image: Handle::<Image>::default().id(),
        clip: None,
        transform: bevy::math::Affine2::IDENTITY,
        item: ExtractedUiItem::Glyphs { glyphs },
    };
    let make = |g: &bevy::text::PositionedGlyph| ExtractedGlyph {
        color: LinearRgba::GREEN,
        translation: g.position,
        rect: g.atlas_info.rect,
    };
    let mut items = EntityIndexMap::default();
    items.insert(shadow, node(vec![make(&glyphs[0])]));
    items.insert(first, node(glyphs[..2].iter().map(make).collect()));
    items.insert(second, node(glyphs[2..].iter().map(make).collect()));
    let mut state = SystemState::<Commands>::new(&mut render);
    let mut commands = state.get_mut(&mut render).unwrap();
    replace_foreground(&mut commands, &mut items, &[first, second], &surfaces);
    state.apply(&mut render);
    assert!(items.contains_key(&shadow));
    assert!(!items.contains_key(&first) && !items.contains_key(&second));
    assert_eq!(items.len(), 4);
    assert!(render.get_entity(first).is_err() && render.get_entity(second).is_err());
    let output: Vec<_> = items
        .iter()
        .filter(|(id, _)| **id != shadow)
        .map(|(_, n)| n)
        .collect();
    assert_eq!(output[0].image, surfaces[0].image.id());
    assert_eq!(output[2].image, surfaces[1].image.id());
    let ExtractedUiItem::Glyphs { glyphs: middle } = &output[1].item else {
        panic!("foreground")
    };
    assert_eq!(middle.len(), 1);
    assert_eq!(middle[0].color, LinearRgba::GREEN);
    assert_eq!(middle[0].translation, glyphs[1].position);
}

#[test]
fn shaped_strokes_keep_advances_and_entities_track_native_color_width_scale_and_disposal() {
    let (mut app, owner, entity) = fixture();
    shape(&mut app, entity, 1.);
    let fill_size = app.world().get::<TextLayoutInfo>(entity).unwrap().size;
    app.update();
    let glyphs = app
        .world()
        .get::<TextStrokeGeometry>(entity)
        .unwrap()
        .glyphs
        .clone();
    assert_eq!(glyphs.len(), 2);
    assert_eq!(
        app.world().get::<TextLayoutInfo>(entity).unwrap().size,
        fill_size
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
        "O中"
    );
    let pages = app.world().resource::<StrokeCache>().fonts.len();
    let images = app.world().resource::<Assets<Image>>().len();
    for _ in 0..60 {
        app.update();
    }
    assert_eq!(
        app.world()
            .get::<TextStrokeGeometry>(entity)
            .unwrap()
            .glyphs,
        glyphs
    );
    assert_eq!(app.world().resource::<StrokeCache>().fonts.len(), pages);
    assert_eq!(app.world().resource::<Assets<Image>>().len(), images);
    let red = fairygui_core::color::Color::from_rgba8(220, 10, 30, 255);
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut().set_control_outline_color(owner, red);
        gui.store_mut().set_alpha(owner, 0.5);
    }
    app.update();
    let changed = &app
        .world()
        .get::<TextStrokeGeometry>(entity)
        .unwrap()
        .glyphs;
    assert_eq!(
        changed[0].texture, glyphs[0].texture,
        "color-only changes reuse the atlas"
    );
    assert_eq!(
        changed[0].color,
        crate::backend::apply_alpha(crate::backend::to_bevy_color(red), 0.5).to_linear()
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_stroke(owner, 8.);
    app.update();
    assert!(
        app.world()
            .get::<TextStrokeGeometry>(entity)
            .unwrap()
            .glyphs[0]
            .rect
            .width()
            > glyphs[0].rect.width()
    );
    shape(&mut app, entity, 2.);
    app.update();
    assert!(
        app.world()
            .get::<TextStrokeGeometry>(entity)
            .unwrap()
            .glyphs[0]
            .rect
            .width()
            > glyphs[0].rect.width() * 1.8
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_stroke(owner, 0.);
    app.update();
    assert!(
        app.world()
            .get::<TextStrokeGeometry>(entity)
            .unwrap()
            .glyphs
            .is_empty()
    );
    assert!(app.world().resource::<StrokeCache>().fonts.is_empty());
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .remove(owner);
    app.update();
    assert!(app.world().get_entity(entity).is_err());
}

#[test]
fn rich_span_stroke_overrides_inherit_restore_and_bitmap_switch_reclaims_geometry_without_layout_children()
 {
    let (mut app, owner, entity) = fixture();
    let child = app
        .world_mut()
        .spawn((Node::default(), ChildOf(entity)))
        .id();
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut().set_object_ubb_enabled(owner, true);
        gui.store_mut()
            .set_object_text(owner, "O[stroke=0]X[/stroke][stroke=8,#ff0000]中[/stroke]");
    }
    app.update();
    shape(&mut app, entity, 1.);
    app.update();
    let glyphs = &app
        .world()
        .get::<TextStrokeGeometry>(entity)
        .unwrap()
        .glyphs;
    assert_eq!(
        glyphs.len(),
        2,
        "explicit zero suppresses only the middle segment"
    );
    assert_eq!(glyphs[1].color, LinearRgba::RED);
    let spans: Vec<_> = app
        .world()
        .get::<Children>(entity)
        .unwrap()
        .iter()
        .filter(|&e| app.world().get::<TextSpan>(e).is_some())
        .collect();
    assert_eq!(spans.len(), 3);
    assert_eq!(
        app.world().get::<TextStrokeSpan>(spans[0]).unwrap().width,
        None
    );
    assert_eq!(
        app.world().get::<TextStrokeSpan>(spans[1]).unwrap().width,
        Some(0.)
    );
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut().set_object_font(owner, Some("BMFontTest"));
        gui.store_mut().set_object_text(owner, "UI#");
    }
    app.update();
    app.update();
    assert!(app.world().get::<TextStrokeGeometry>(entity).is_none());
    assert!(app.world().get::<Text>(entity).is_none());
    assert_eq!(app.world().get::<BitmapGlyphs>(entity).unwrap().0.len(), 3);
    assert!(app.world().get_entity(child).is_ok());
    assert!(app.world().resource::<StrokeCache>().fonts.is_empty());
}

#[test]
fn input_and_password_strokes_use_the_display_editor_while_empty_prompts_keep_their_own_geometry() {
    let (mut app, text, _) = fixture();
    let root = app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .object(text)
        .unwrap()
        .parent()
        .unwrap();
    let owner = app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .child_by_name(root, "n22")
        .unwrap();
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut().set_object_font(owner, None);
        gui.store_mut().set_object_ubb_enabled(owner, false);
        gui.store_mut().set_object_text(owner, "A😀");
        gui.store_mut().set_object_stroke(owner, 4.);
        gui.store_mut().set_input_prompt(owner, Some("Hint"));
    }
    let mut fonts = FontCx::default();
    let font = Font::from_bytes(
        include_bytes!("../../fairygui-demo/assets/fonts/NotoSansSC-VF.ttf").to_vec(),
    );
    fonts.collection.register_fonts(font.data, None);
    app.insert_resource(fonts)
        .init_resource::<LayoutCx>()
        .add_systems(
            Update,
            (
                sync_text_inputs,
                sync_input_policy,
                sync_password_views,
                sync_input_prompts,
            )
                .chain()
                .after(sync_stage_hierarchy),
        )
        .add_systems(PostUpdate, prepare_password_layout.before(build_strokes));
    app.update();
    let entity = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(owner)
        .unwrap();
    let mut fonts = app.world_mut().remove_resource::<FontCx>().unwrap();
    {
        let mut editor = app.world_mut().get_mut::<EditableText>(entity).unwrap();
        editor
            .editor
            .edit_styles()
            .insert(parley::StyleProperty::FontFamily(
                parley::FontFamily::Source("SimHei".into()),
            ));
        editor
            .editor
            .edit_styles()
            .insert(parley::StyleProperty::FontSize(32.));
        editor
            .editor
            .driver(&mut fonts, &mut LayoutCx::default().0)
            .refresh_layout();
    }
    app.insert_resource(fonts);
    app.update();
    assert!(
        !app.world()
            .get::<TextStrokeGeometry>(entity)
            .unwrap()
            .glyphs
            .is_empty()
    );
    let selection = *app
        .world()
        .get::<EditableText>(entity)
        .unwrap()
        .editor
        .raw_selection();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_input_password(owner, true);
    app.update();
    assert_eq!(
        app.world()
            .get::<TextStrokeGeometry>(entity)
            .unwrap()
            .glyphs
            .len(),
        3
    );
    assert_eq!(
        app.world()
            .get::<EditableText>(entity)
            .unwrap()
            .value()
            .to_string(),
        "A😀"
    );
    assert_eq!(
        *app.world()
            .get::<EditableText>(entity)
            .unwrap()
            .editor
            .raw_selection(),
        selection
    );
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
    shape(&mut app, prompt, 1.);
    app.update();
    assert_eq!(
        app.world()
            .get::<TextStrokeGeometry>(prompt)
            .unwrap()
            .glyphs
            .len(),
        4
    );
    assert!(
        app.world()
            .get::<TextStrokeGeometry>(entity)
            .unwrap()
            .glyphs
            .is_empty()
    );
    assert!(app.world().get::<Text>(entity).unwrap().0.is_empty());
}

use super::*;
use crate::prelude::*;
use bevy::{
    camera::{ComputedCameraValues, RenderTargetInfo, Viewport},
    ecs::system::SystemState,
};

fn fixture(source: &str, scale: f32, leading: i32) -> (App, Entity, Entity) {
    fixture_with_fonts(source, scale, leading, false)
}

fn fixture_with_fonts(
    source: &str,
    scale: f32,
    leading: i32,
    sparse: bool,
) -> (App, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<Image>()
        .init_asset::<TextureAtlasLayout>()
        .add_plugins((
            bevy::input::InputPlugin,
            bevy::transform::TransformPlugin,
            bevy::text::TextPlugin,
            bevy::picking::PickingPlugin,
            bevy::picking::InteractionPlugin,
            bevy::ui::UiPlugin,
            FairyGuiPlugin,
        ));
    app.world_mut().resource_mut::<bevy::ui::UiScale>().0 = scale;
    app.world_mut().spawn((
        Camera2d,
        Camera {
            computed: ComputedCameraValues {
                target_info: Some(RenderTargetInfo {
                    physical_size: UVec2::new(1280, 720),
                    scale_factor: 1.,
                }),
                ..default()
            },
            viewport: Some(Viewport {
                physical_size: UVec2::new(1280, 720),
                ..default()
            }),
            ..default()
        },
    ));
    let bytes = include_bytes!("../../fairygui-demo/assets/fonts/NotoSansSC-VF.ttf");
    let face = Font::from_bytes(bytes.to_vec());
    let font = if sparse {
        #[derive(Resource)]
        struct PinnedFonts {
            _handles: Vec<Handle<Font>>,
        }
        let mut different = bytes.to_vec();
        let count = usize::from(u16::from_be_bytes(different[4..6].try_into().unwrap()));
        let offset = different[12..12 + count * 16]
            .as_chunks::<16>()
            .0
            .iter()
            .find(|table| &table[..4] == b"OS/2")
            .map(|table| u32::from_be_bytes(table[8..12].try_into().unwrap()) as usize)
            .unwrap();
        // Distinguishable valid typographic metrics expose a wrong face even
        // when all assets share the same family name and glyph outlines.
        different[offset + 68..offset + 70].copy_from_slice(&2000_i16.to_be_bytes());
        let (first, removed, decoy, target) = {
            let mut fonts = app.world_mut().resource_mut::<Assets<Font>>();
            let first = fonts.add(face.clone());
            let removed = fonts.add(face.clone());
            let decoy = fonts.add(Font::from_bytes(different));
            let target = fonts.add(face);
            fonts.remove(removed.id());
            (first, removed, decoy, target)
        };
        app.insert_resource(PinnedFonts {
            _handles: vec![first, removed, decoy],
        });
        target
    } else {
        app.world_mut().resource_mut::<Assets<Font>>().add(face)
    };
    app.insert_resource(GuiFont(Some(font)));
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
                font_size: 24,
                leading,
                auto_size: 1,
                single_line: false,
                html_enabled: true,
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
    (app, owner, title)
}

#[test]
fn font_assets_with_holes_do_not_confuse_measurement_handles_with_application_handles() {
    let (app, _, title) = fixture_with_fonts("A\nB", 1.3, 3, true);
    let info = app.world().get::<TextLayoutInfo>(title).unwrap();
    assert!((info.size.y / 1.3 - 61.).abs() < 0.001);
    assert_eq!(
        app.world().get::<GuiTransform>(title).unwrap().size().y,
        65.
    );
}

fn baseline(glyph: &bevy::text::PositionedGlyph) -> f32 {
    glyph.position.y - glyph.atlas_info.offset.y - glyph.atlas_info.rect.size().y * 0.5
}

#[test]
fn first_frame_auto_size_matches_rendered_rows_at_fractional_dpi_without_trailing_leading() {
    for scale in [0.7, 1., 1.25, 1.3, 1.75, 2.] {
        let (mut app, owner, title) = fixture("A\nB", scale, 3);
        let height = app.world().get::<GuiTransform>(title).unwrap().size().y;
        assert_eq!(height, 65., "scale={scale}");
        let info = app.world().get::<TextLayoutInfo>(title).unwrap();
        assert!(!info.glyphs.is_empty());
        assert!((info.size.y / scale - 61.).abs() < 0.001);
        let expected: Vec<_> = info.glyphs.iter().map(|g| g.position).collect();
        for _ in 0..4 {
            app.update();
            assert_eq!(
                app.world().get::<GuiTransform>(title).unwrap().size().y,
                height
            );
            let info = app.world().get::<TextLayoutInfo>(title).unwrap();
            assert_eq!(
                info.glyphs.iter().map(|g| g.position).collect::<Vec<_>>(),
                expected
            );
        }
        assert_eq!(app.world().get::<GuiText>(owner).unwrap().value(), "A\nB");
    }
    for leading in [-2, 0, 20] {
        let (app, _, title) = fixture("A", 1.3, leading);
        assert_eq!(
            app.world().get::<GuiTransform>(title).unwrap().size().y,
            33.
        );
    }
}

#[test]
fn mixed_font_sizes_and_images_use_command_centers_and_native_baselines() {
    for scale in [1., 1.3, 2.] {
        let (app, _, title) = fixture(
            "<font size='48'>A</font><font size='12'>g</font><img src='missing.png' width='18' height='10'/>",
            scale,
            3,
        );
        let info = app.world().get::<TextLayoutInfo>(title).unwrap();
        // The tracked font's 32px g/有 probe is 37px high. A 48px run is
        // ceil(37*48/32)=56px; the 12px run is 14px and starts 21px lower.
        assert!((info.size.y / scale - 57.).abs() < 0.001);
        assert_eq!(info.glyphs.len(), 2);
        assert!((baseline(&info.glyphs[0]) / scale - 44.234375).abs() < 0.01);
        assert!((baseline(&info.glyphs[1]) / scale - 32.5625).abs() < 0.01);
        assert!((info.inline_boxes[0].2.min.y / scale - 23.).abs() < 0.001);
        for run in &info.run_geometry {
            assert!((run.bounds.min.y).abs() < 0.001);
            assert!((run.bounds.max.y / scale - 57.).abs() < 0.001);
        }
    }
}

#[test]
fn dynamic_text_and_empty_values_remeasure_without_replacing_entities_or_accumulating_offsets() {
    let (mut app, owner, title) = fixture("A\nB", 1.3, 3);
    app.world_mut().get_mut::<GuiText>(owner).unwrap().set("A");
    app.update();
    assert_eq!(
        app.world().get::<GuiTransform>(title).unwrap().size().y,
        33.
    );
    assert!((app.world().get::<TextLayoutInfo>(title).unwrap().size.y / 1.3 - 29.).abs() < 0.001);
    app.world_mut().get_mut::<GuiText>(owner).unwrap().set("");
    app.update();
    assert_eq!(app.world().get::<GuiTransform>(title).unwrap().size().y, 0.);
    assert_eq!(app.world().get::<TextLayoutInfo>(title).unwrap().size.y, 0.);
    assert!(
        app.world()
            .get::<TextLayoutInfo>(title)
            .unwrap()
            .glyphs
            .is_empty()
    );
    app.world_mut()
        .get_mut::<GuiText>(owner)
        .unwrap()
        .set("A\nB");
    app.update();
    assert_eq!(
        app.world().get::<GuiTransform>(title).unwrap().size().y,
        65.
    );
    assert!(app.world().get::<NativeTextLayout>(title).is_some());
}

#[test]
fn stable_text_reuses_vertical_projection_when_bevy_refreshes_identical_geometry() {
    #[derive(Resource)]
    struct Watched(Entity);
    #[derive(Resource, Default)]
    struct Changes(usize);
    fn count(
        watched: Res<Watched>,
        mut changes: ResMut<Changes>,
        query: Query<Entity, Changed<NativeTextLayout>>,
    ) {
        if query.contains(watched.0) {
            changes.0 += 1;
        }
    }
    let (mut app, owner, title) = fixture("A\nB", 1.3, 3);
    for _ in 0..5 {
        app.update();
    }
    app.insert_resource(Watched(title))
        .init_resource::<Changes>()
        .add_systems(Last, count);
    app.update();
    app.world_mut().resource_mut::<Changes>().0 = 0;
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(app.world().resource::<Changes>().0, 0);
    app.world_mut().get_mut::<GuiText>(owner).unwrap().set("C");
    app.update();
    assert!(app.world().resource::<Changes>().0 > 0);
}

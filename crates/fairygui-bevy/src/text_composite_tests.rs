use super::*;

fn image(coverage: u8, size: UVec2) -> Image {
    Image::new_fill(
        bevy::render::render_resource::Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        &[255, 255, 255, coverage],
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::default(),
    )
}

#[test]
fn native_offscreen_order_matches_288_numerical_canvas_samples() {
    let mut count = 0;
    for line in include_str!("../tests/fixtures/text-composite-reference.txt")
        .lines()
        .filter(|l| !l.starts_with('#'))
    {
        let v: Vec<f32> = line
            .split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect();
        let mut images = Assets::<Image>::default();
        let stroke = images.add(image(v[0] as u8, UVec2::ONE));
        let fill = images.add(image(v[1] as u8, UVec2::ONE));
        let mask = |texture, color| Mask {
            texture,
            rect: Rect::from_corners(Vec2::ZERO, Vec2::ONE),
            position: Vec2::splat(0.5),
            color,
        };
        let group = Projection {
            section: 0,
            line: 0,
            members: vec![0],
            matrix: None,
            strokes: if v[4] > 0. {
                vec![mask(
                    stroke.id(),
                    GuiColor::new(200. / 255., 30. / 255., 40. / 255., v[2]),
                )]
            } else {
                vec![]
            },
            fills: vec![mask(
                fill.id(),
                GuiColor::new(20. / 255., 70. / 255., 110. / 255., v[3]),
            )],
        };
        let actual = raster_tile(&group, Vec2::ZERO, UVec2::ONE, &images);
        assert_eq!(
            actual,
            v[5..].iter().map(|v| *v as u8).collect::<Vec<_>>(),
            "{line}"
        );
        count += 1;
    }
    assert_eq!(count, 288);
}

#[test]
fn atlas_sampling_respects_rects_and_subpixels_without_neighbor_bleeding() {
    let mut atlas = image(255, UVec2::new(3, 1));
    atlas.data.as_mut().unwrap()[7] = 128;
    let rect = Rect::from_corners(Vec2::new(1., 0.), Vec2::new(2., 1.));
    assert_eq!(sample(&atlas, rect, Vec2::new(1.5, 0.5)), 128. / 255.);
    assert_eq!(sample(&atlas, rect, Vec2::new(1., 0.5)), 64. / 255.);
    assert_eq!(sample(&atlas, rect, Vec2::new(0.5, 0.5)), 0.);
    atlas.sampler = bevy::image::ImageSampler::nearest();
    assert_eq!(sample(&atlas, rect, Vec2::new(1., 0.5)), 128. / 255.);
}

#[test]
fn wide_commands_tile_with_identical_coverage_and_filter_after_composition() {
    let mut images = Assets::<Image>::default();
    let atlas = images.add(image(255, UVec2::new(TILE + 8, 2)));
    let stroke = Mask {
        texture: atlas.id(),
        rect: Rect::from_corners(Vec2::ZERO, Vec2::new((TILE + 8) as f32, 2.)),
        position: Vec2::new((TILE + 8) as f32 / 2., 1.),
        color: GuiColor::new(1., 0., 0., 0.5),
    };
    let fill = Mask {
        color: GuiColor::new(0., 0., 1., 0.5),
        ..stroke.clone()
    };
    let matrix = fairygui_core::color_matrix::ColorFilterParams {
        saturation: -1.,
        ..default()
    }
    .to_matrix();
    let group = Arc::new(Projection {
        section: 0,
        line: 0,
        members: vec![0],
        strokes: vec![stroke.clone(), stroke],
        fills: vec![fill],
        matrix: Some(matrix),
    });
    let surfaces = rasterize(group, 0.25, &mut images);
    assert_eq!(surfaces.len(), 2);
    let color = matrix.apply(GuiColor::new(1. / 3., 0., 2. / 3., 0.75));
    let expected = [color.r, color.g, color.b, color.a].map(|v| (v * 255.).round() as u8);
    for surface in &surfaces {
        assert_eq!(surface.alpha, 0.25);
        let image = images.get(&surface.image).unwrap();
        assert!(image.width() <= TILE + 2 && image.height() <= TILE + 2);
        let bytes = image.data.as_ref().unwrap();
        let index = (image.width() as usize + 1) * 4;
        assert_eq!(&bytes[index..index + 4], &expected);
    }
    // Both sides of the tile boundary include the same neighboring content.
    let left = images.get(&surfaces[0].image).unwrap();
    let right = images.get(&surfaces[1].image).unwrap();
    let a = ((left.width() + TILE + 1) * 4) as usize;
    let b = (right.width() * 4) as usize;
    assert_eq!(
        &left.data.as_ref().unwrap()[a..a + 4],
        &right.data.as_ref().unwrap()[b..b + 4]
    );
}

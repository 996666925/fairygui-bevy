//! Inline row centering, resize/reuse and final rendered image transforms.
use super::*;

const PAIR: &str = "<img src='images/inline-swatch.png' width='18' height='10'/><img src='images/inline-swatch.png' width='18' height='33'/>";
const LINK: &str = "<a href='event:small'><img src='images/inline-swatch.png' width='18' height='10'/></a><img src='images/inline-swatch.png' width='18' height='80'/>";
const MIXED: &str = "<font size='48' stroke='2' strokecolor='#0088FF'>Aa 中文</font><img src='images/inline-swatch.png' width='40' height='14'/> text";

#[derive(Resource, Default)]
struct State {
    frame: u32,
    owners: Vec<Entity>,
    titles: Vec<Entity>,
    images: Vec<Entity>,
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<State>().add_systems(PreUpdate, drive);
}

#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    demo: Res<Demo>,
    gui: GuiQuery,
    mut ui: GuiMut,
    mut assets: ResMut<Assets<fairygui_bevy::FuiAsset>>,
    paragraphs: Query<(&bevy::text::ComputedTextBlock, &bevy::text::TextLayoutInfo)>,
    parts: Query<&fairygui_bevy::runtime::InlineImageParts>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    if state.owners.is_empty() {
        assert!(
            state.frame < 700,
            "inline alignment smoke did not initialize"
        );
        let Some(menu) = demo.menu_owner.filter(|&owner| gui.view(owner).is_some()) else {
            return;
        };
        ui.get_mut(menu).unwrap().set_visible(false);
        let asset = assets.add(fairygui_bevy::FuiAsset {
            package: fairygui_bevy::core::FuiPackage::parse(
                "TextTemplate",
                include_bytes!("../../fairygui-core/tests/fixtures/TextTemplate.fui"),
            )
            .unwrap(),
            res_key: "ui/TextTemplate".into(),
        });
        for (index, source) in [PAIR, LINK, MIXED].into_iter().enumerate() {
            state.owners.push(
                commands
                    .spawn((
                        FairyGuiRoot::new(asset.clone(), "Label")
                            .with_position(Vec2::new(100., 90. + index as f32 * 150.)),
                        GuiText::new(source),
                        GuiTextFormat {
                            font_size: 24,
                            leading: 3,
                            auto_size: 0,
                            single_line: false,
                            html_enabled: true,
                            color: Color::WHITE,
                            ..default()
                        },
                    ))
                    .id(),
            );
        }
        state.frame = 0;
        return;
    }
    if state.frame == 8 {
        state.titles = state
            .owners
            .iter()
            .map(|&owner| gui.child_by_name(owner, "title").unwrap())
            .collect();
        for &title in &state.titles {
            ui.get_mut(title).unwrap().set_size(Vec2::new(500., 120.));
        }
    }
    if [40, 80, 120].contains(&state.frame) {
        let images: Vec<_> = state
            .titles
            .iter()
            .flat_map(|&title| parts.get(title).unwrap().iter())
            .collect();
        if state.frame == 40 {
            state.images = images.clone();
        } else {
            assert_eq!(state.images, images, "resizing reuses inline image views");
        }
        for (index, &title) in state.titles.iter().enumerate() {
            assert_eq!(gui.text(title), Some([PAIR, LINK, MIXED][index]));
            let (block, layout) = paragraphs.get(title).unwrap();
            assert_eq!(layout.inline_boxes.len(), if index == 2 { 1 } else { 2 });
            if index < 2 {
                let scale = layout.scale_factor;
                let (first, second) = (layout.inline_boxes[0].2, layout.inline_boxes[1].2);
                let expected = if index == 0 && state.frame == 80 {
                    assert_eq!(block.buffer().lines().count(), 2);
                    [0., 14. * scale]
                } else {
                    assert_eq!(block.buffer().lines().count(), 1);
                    [if index == 0 { 11. } else { 35. } * scale, 0.]
                };
                assert!((first.min.y - expected[0]).abs() < 0.01);
                assert!((second.min.y - expected[1]).abs() < 0.01);
            } else {
                let baseline = |glyph: &bevy::text::PositionedGlyph| {
                    (glyph.position.y
                        - glyph.atlas_info.offset.y
                        - glyph.atlas_info.rect.size().y * 0.5)
                        / layout.scale_factor
                };
                assert!((layout.size.y / layout.scale_factor - 57.).abs() < 0.01);
                assert!((baseline(&layout.glyphs[0]) - 44.234375).abs() < 0.01);
                assert!((baseline(layout.glyphs.last().unwrap()) - 36.125).abs() < 0.01);
                assert!((layout.inline_boxes[0].2.min.y / layout.scale_factor - 21.).abs() < 0.01);
            }
            let (text_node, text_global) = nodes.get(title).unwrap();
            for (image, (_, _, bounds)) in
                parts.get(title).unwrap().iter().zip(&layout.inline_boxes)
            {
                let (image_node, image_global) = nodes.get(image).unwrap();
                assert!(image_node.size().min_element() > 0.);
                let actual = image_global.transform_point2(Vec2::ZERO);
                let expected =
                    text_global.transform_point2(text_node.content_box().min + bounds.center());
                assert!(
                    actual.abs_diff_eq(expected, 0.05),
                    "{actual:?} != {expected:?}"
                );
            }
        }
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(match state.frame {
                40 => "target/fairygui-demo-inline-alignment.png",
                80 => "target/fairygui-demo-inline-alignment-wrapped.png",
                _ => "target/fairygui-demo-inline-alignment-restored.png",
            }));
    }
    if state.frame == 55 || state.frame == 95 {
        ui.get_mut(state.titles[0])
            .unwrap()
            .set_size(Vec2::new(if state.frame == 55 { 30. } else { 500. }, 120.));
    }
    if state.frame == 135 {
        for &owner in &state.owners {
            commands.entity(owner).despawn();
        }
    }
    if state.frame == 145 {
        assert!(state.owners.iter().all(|&owner| gui.view(owner).is_none()));
        info!(
            "demo smoke PASSED: inline image row alignment, resize/reuse and rendered transforms"
        );
        exit.write(AppExit::Success);
    }
}

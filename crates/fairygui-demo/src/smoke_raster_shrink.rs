//! Real paragraph fitting with independent rich sizes and a fixed inline image.
use super::*;

const PLAIN: &str = "Single line ordinary text 中文 that exceeds its field";
const RICH: &str = "[url=event:fit][size=48]Rich text[/size] 中文 <img src='images/inline-swatch.png' width='22' height='14'/> with an inline image and several words across lines[/url]";

#[derive(Resource, Default)]
struct State {
    frame: u32,
    owners: Vec<Entity>,
    titles: Vec<Entity>,
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
    paragraphs: Query<(
        &TextFont,
        &bevy::text::ComputedTextBlock,
        &bevy::text::TextLayoutInfo,
    )>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    if state.owners.is_empty() {
        assert!(state.frame < 700, "raster shrink smoke did not initialize");
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
        for (index, (source, mode)) in [(PLAIN, 0), (PLAIN, 3), (RICH, 3)].into_iter().enumerate() {
            state.owners.push(
                commands
                    .spawn((
                        FairyGuiRoot::new(asset.clone(), "Label")
                            .with_position(Vec2::new(60., 80. + index as f32 * 120.)),
                        GuiText::new(source),
                        GuiTextFormat {
                            font_size: 32,
                            leading: 4,
                            auto_size: mode,
                            single_line: index != 2,
                            html_enabled: index == 2,
                            ubb_enabled: index == 2,
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
        for (index, &title) in state.titles.iter().enumerate() {
            ui.get_mut(title)
                .unwrap()
                .set_size(Vec2::new(280., if index == 2 { 84. } else { 50. }));
        }
    }
    if state.frame == 40 || state.frame == 80 || state.frame == 120 {
        for (index, &title) in state.titles.iter().enumerate() {
            assert_eq!(gui.child_by_name(state.owners[index], "title"), Some(title));
            assert_eq!(gui.text_format(title).unwrap().font_size, 32);
            assert_eq!(gui.text(title), Some(if index == 2 { RICH } else { PLAIN }));
            let (font, block, layout) = paragraphs.get(title).unwrap();
            let FontSize::Px(size) = font.font_size else {
                panic!("pixel size")
            };
            if index == 0 || state.frame == 80 {
                assert_eq!(size, 32.);
            } else {
                assert!(size < 32.);
                let measured = Vec2::new(block.buffer().full_width(), block.buffer().height())
                    / layout.scale_factor;
                assert!(measured.x <= 276.1, "{measured:?}");
                if index == 2 {
                    assert!(measured.y <= 80.1, "{measured:?}");
                }
            }
        }
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(match state.frame {
                40 => "target/fairygui-demo-raster-shrink.png",
                80 => "target/fairygui-demo-raster-shrink-expanded.png",
                _ => "target/fairygui-demo-raster-shrink-restored.png",
            }));
    }
    if state.frame == 55 || state.frame == 95 {
        for &title in &state.titles[1..] {
            ui.get_mut(title).unwrap().set_size(if state.frame == 55 {
                Vec2::new(1100., 220.)
            } else {
                Vec2::new(280., 84.)
            });
        }
    }
    if state.frame == 135 {
        for &owner in &state.owners {
            commands.entity(owner).despawn();
        }
    }
    if state.frame == 145 {
        assert!(state.owners.iter().all(|&owner| gui.view(owner).is_none()));
        info!(
            "demo smoke PASSED: raster shrink, rich sizes and inline images, resize restoration, raw source and retained title identity"
        );
        exit.write(AppExit::Success);
    }
}

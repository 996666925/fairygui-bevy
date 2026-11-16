//! Fixed-size text overflow, rich images/links and input focus restoration.
use super::*;

const PLAIN: &str = "Ordinary ellipsis 中文😀 with a long suffix that must stay in the source";
const RICH: &str = "[url=front][color=#FF8800][size=32]Rich 中文[/size][/color] <img src='images/inline-swatch.png' width='20' height='14'/> text that must end with a marker[/url]\n[url=hidden]Hidden link after the last visible line[/url]";
#[derive(Resource, Default)]
struct State {
    frame: u32,
    owners: Vec<Entity>,
    titles: Vec<Entity>,
    input: Option<Entity>,
    clipped_glyphs: usize,
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<State>()
        .add_systems(PreUpdate, (drive, focus).chain());
}
fn focus(state: Res<State>, mut input: GuiInput) {
    if state.frame == 100 {
        assert!(input.focus(state.input.unwrap()));
    }
    if state.frame == 135 {
        input.blur();
    }
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
        &bevy::text::TextLayoutInfo,
        &bevy::text::ComputedTextBlock,
    )>,
    editors: Query<&bevy::text::EditableText>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    if state.owners.is_empty() {
        assert!(state.frame < 700, "ellipsis smoke did not initialize");
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
        for (index, (text, mode)) in [(PLAIN, 0), (PLAIN, 4), (RICH, 4)].into_iter().enumerate() {
            state.owners.push(
                commands
                    .spawn((
                        FairyGuiRoot::new(asset.clone(), "Label")
                            .with_position(Vec2::new(60., 60. + index as f32 * 110.)),
                        GuiText::new(text),
                        GuiTextFormat {
                            font_size: 24,
                            leading: 3,
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
        state.owners.push(
            commands
                .spawn(FairyGuiRoot::new(asset, "Main").with_position(Vec2::new(60., 310.)))
                .id(),
        );
        state.frame = 0;
        return;
    }
    if state.frame == 8 {
        state.titles = state.owners[..3]
            .iter()
            .map(|&owner| gui.child_by_name(owner, "title").unwrap())
            .collect();
        for (index, &title) in state.titles.iter().enumerate() {
            ui.get_mut(title)
                .unwrap()
                .set_size(Vec2::new(280., if index == 2 { 70. } else { 45. }));
        }
        let owner = state.owners[3];
        for name in ["plain", "rich", "disabled", "nested"] {
            let child = gui.child_by_name(owner, name).unwrap();
            ui.get_mut(child).unwrap().set_visible(false);
        }
        let input = gui.child_by_name(owner, "input").unwrap();
        ui.get_mut(input).unwrap().set_size(Vec2::new(280., 45.));
        commands.entity(input).insert((
            GuiText::new(PLAIN),
            GuiTextFormat {
                font_size: 24,
                auto_size: 4,
                single_line: true,
                color: Color::WHITE,
                ..default()
            },
        ));
        state.input = Some(input);
    }
    if [40, 80, 120, 160].contains(&state.frame) {
        let mut clipped_glyphs = None;
        for (index, &title) in state.titles.iter().enumerate() {
            assert_eq!(gui.child_by_name(state.owners[index], "title"), Some(title));
            assert_eq!(gui.text(title), Some(if index == 2 { RICH } else { PLAIN }));
            let (font, info, block) = paragraphs.get(title).unwrap();
            assert_eq!(font.font_size, FontSize::Px(24.));
            if index == 1 {
                assert_eq!(block.buffer().lines().count(), 1);
                if state.frame == 40 {
                    clipped_glyphs = Some(info.glyphs.len());
                } else if state.frame == 80 {
                    assert!(info.glyphs.len() > state.clipped_glyphs);
                } else {
                    assert_eq!(info.glyphs.len(), state.clipped_glyphs);
                }
            }
            if index == 2 && state.frame != 80 {
                assert!(block.buffer().lines().count() <= 2);
            }
        }
        if let Some(count) = clipped_glyphs {
            state.clipped_glyphs = count;
        }
        assert_eq!(editors.get(state.input.unwrap()).unwrap().value(), PLAIN);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(match state.frame {
                40 => "target/fairygui-demo-raster-ellipsis.png",
                80 => "target/fairygui-demo-raster-ellipsis-expanded.png",
                120 => "target/fairygui-demo-raster-ellipsis-focused.png",
                _ => "target/fairygui-demo-raster-ellipsis-restored.png",
            }));
    }
    if state.frame == 55 || state.frame == 95 {
        for &title in &state.titles[1..] {
            ui.get_mut(title).unwrap().set_size(if state.frame == 55 {
                Vec2::new(1160., 180.)
            } else {
                Vec2::new(280., 70.)
            });
        }
    }
    if state.frame == 175 {
        for &owner in &state.owners {
            commands.entity(owner).despawn();
        }
    }
    if state.frame == 185 {
        assert!(state.owners.iter().all(|&owner| gui.view(owner).is_none()));
        info!(
            "demo smoke PASSED: ellipsis, rich styles/images/links, resize, editable focus restoration and unchanged source"
        );
        exit.write(AppExit::Success);
    }
}

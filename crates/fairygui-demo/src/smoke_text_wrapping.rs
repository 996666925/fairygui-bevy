//! Native command breaks across styles, punctuation and width changes.
use super::*;

const WORD: &str = "<a href='event:empty'></a>中<a href='event:word'><font color='#FF8800' stroke='1' strokecolor='#0044FF'>hel</font><font color='#00FF88'>lo</font></a><a href='event:empty'></a>";
const PUNCTUATION: &str = "中文，标点。";
const IMAGE: &str = "中<img src='images/inline-swatch.png' width='18' height='10'/>hello";
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
    paragraphs: Query<(&bevy::text::ComputedTextBlock, &bevy::text::TextLayoutInfo)>,
    spans: Query<&TextSpan>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    if state.owners.is_empty() {
        assert!(state.frame < 700, "text wrapping smoke did not initialize");
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
        for (index, source) in [WORD, PUNCTUATION, IMAGE].into_iter().enumerate() {
            state.owners.push(
                commands
                    .spawn((
                        FairyGuiRoot::new(asset.clone(), "Label")
                            .with_position(Vec2::new(120. + index as f32 * 260., 100.)),
                        GuiText::new(source),
                        GuiTextFormat {
                            font_size: 24,
                            leading: 3,
                            auto_size: 2,
                            single_line: false,
                            html_enabled: true,
                            align: if index == 1 { 1 } else { 0 },
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
                .set_size(Vec2::new(if index == 1 { 52. } else { 64. }, 300.));
        }
    }
    if [40, 80, 120].contains(&state.frame) {
        for (index, &title) in state.titles.iter().enumerate() {
            assert_eq!(gui.child_by_name(state.owners[index], "title"), Some(title));
            assert_eq!(gui.text(title), Some([WORD, PUNCTUATION, IMAGE][index]));
            let (block, info) = paragraphs.get(title).unwrap();
            assert!(!info.glyphs.is_empty());
            let scale = info.scale_factor;
            assert!(
                (gui.transform(title).unwrap().size().y - info.size.y / scale - 4.).abs() < 0.01
            );
            if index == 0 {
                assert!(
                    info.inline_boxes.is_empty(),
                    "link boundaries must not publish image geometry"
                );
                assert_eq!(
                    block.buffer().lines().count(),
                    if state.frame == 80 { 1 } else { 2 }
                );
                assert_eq!(
                    info.glyphs
                        .iter()
                        .filter(|glyph| glyph.line_index == 0)
                        .count(),
                    if state.frame == 80 { 6 } else { 1 }
                );
                let mut colors = Vec::new();
                for section in block.entities() {
                    if let Ok(span) = spans.get(section.entity)
                        && (span.0 == "hel" || span.0 == "lo")
                    {
                        colors.push(span.0.as_str());
                    }
                }
                assert!(colors.contains(&"hel") && colors.contains(&"lo"));
            }
        }
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(match state.frame {
                40 => "target/fairygui-demo-text-wrapping.png",
                80 => "target/fairygui-demo-text-wrapping-expanded.png",
                _ => "target/fairygui-demo-text-wrapping-restored.png",
            }));
    }
    if state.frame == 55 || state.frame == 95 {
        for (index, &title) in state.titles.iter().enumerate() {
            ui.get_mut(title).unwrap().set_size(Vec2::new(
                if state.frame == 55 {
                    220.
                } else if index == 1 {
                    52.
                } else {
                    64.
                },
                300.,
            ));
        }
    }
    if state.frame == 135 {
        for &owner in &state.owners {
            commands.entity(owner).despawn();
        }
    }
    if state.frame == 145 {
        assert!(state.owners.iter().all(|&owner| gui.view(owner).is_none()));
        info!("demo smoke PASSED: native text wrapping, styles, dimensions and width restoration");
        exit.write(AppExit::Success);
    }
}

//! Per-field link defaults, explicit styles, whitespace and live reparse.
use super::*;

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
    spans: Query<(&TextSpan, Option<&Underline>, &TextColor)>,
    children: Query<&Children>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    if state.owners.is_empty() {
        assert!(state.frame < 700, "HTML options smoke did not initialize");
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
        for (index, (source, ubb, options)) in [
            ("Default: <a href='default'>link</a>", false, GuiHtmlOptions::default()),
            ("  HTML: <a href='html'>link</a> / <font color='#00FF00'><a href='explicit'>green</a></font> / <u><a href='u'>underlined</a></u>  ", false, GuiHtmlOptions { link_underline: false, link_color: Some("#FF8800".into()), ignore_whitespace: true }),
            ("UBB: [url=ubb]link[/url] / [color=#00FF00][url=green]green[/url][/color]", true, GuiHtmlOptions { link_color: Some("#FF8800".into()), ..default() }),
        ].into_iter().enumerate() {
            state.owners.push(commands.spawn((
                FairyGuiRoot::new(asset.clone(), "Label").with_position(Vec2::new(100., 100. + index as f32 * 90.)),
                GuiText::new(source),
                GuiTextFormat { html_enabled: true, ubb_enabled: ubb, font_size: 26, auto_size: 1, color: Color::WHITE, ..default() },
                options,
            )).id());
        }
        state.frame = 0;
        return;
    }
    if state.frame == 40 || state.frame == 80 {
        let titles: Vec<_> = state
            .owners
            .iter()
            .map(|&owner| gui.child_by_name(owner, "title").unwrap())
            .collect();
        if state.frame == 40 {
            state.titles = titles.clone();
        } else {
            assert_eq!(titles, state.titles);
        }
        let mut orange = 0;
        let mut green = 0;
        for (index, title) in titles.into_iter().enumerate() {
            for entity in children.iter_descendants(title) {
                let Ok((text, underline, color)) = spans.get(entity) else {
                    continue;
                };
                if text.0 == "link" {
                    assert_eq!(underline.is_some(), index != 1 || state.frame == 80);
                }
                let color = color.0.to_srgba();
                if color.red > 0.99 && color.green > 0.5 && color.green < 0.6 && color.blue < 0.01 {
                    orange += 1;
                }
                if text.0 == "green" {
                    assert!(color.green > 0.99 && color.red < 0.01);
                    green += 1;
                }
            }
        }
        assert_eq!(green, 2);
        assert_eq!(orange, if state.frame == 40 { 3 } else { 0 });
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(if state.frame == 40 {
                "target/fairygui-demo-html-options.png"
            } else {
                "target/fairygui-demo-html-options-reset.png"
            }));
    }
    if state.frame == 55 {
        for &owner in &state.owners {
            commands.entity(owner).insert(GuiHtmlOptions::default());
        }
    }
    if state.frame == 95 {
        for &owner in &state.owners {
            commands.entity(owner).despawn();
        }
    }
    if state.frame == 105 {
        assert!(state.owners.iter().all(|&owner| gui.view(owner).is_none()));
        info!(
            "demo smoke PASSED: per-field HTML/UBB options, explicit colors/underlines, reset, retained titles and disposal"
        );
        exit.write(AppExit::Success);
    }
}

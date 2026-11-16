//! Native line centers and strike extensions, with screen-space pixel samples.
use super::*;

const SPACES: &str = "                ";
const MIXED: &str = "<strike><u><font size='48' color='#FF8800'>Large</font><font size='24' color='#00FF88'> Small</font></u></strike>";

#[derive(Resource, Default)]
struct State {
    frame: u32,
    owners: Vec<Entity>,
    titles: Vec<Entity>,
    samples: String,
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
    paragraphs: Query<&bevy::text::TextLayoutInfo>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    if state.owners.is_empty() {
        assert!(state.frame < 700, "decoration smoke did not initialize");
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
        for (index, source) in [
            format!("<strike>{SPACES}</strike>"),
            format!("<u>{SPACES}</u>"),
            MIXED.into(),
        ]
        .into_iter()
        .enumerate()
        {
            state.owners.push(
                commands
                    .spawn((
                        FairyGuiRoot::new(asset.clone(), "Label")
                            .with_position(Vec2::new(150., 80. + index as f32 * 140.)),
                        GuiText::new(source),
                        GuiTextFormat {
                            font_size: 48,
                            auto_size: 1,
                            single_line: true,
                            html_enabled: true,
                            color: if index == 0 {
                                Color::srgb(1., 0., 0.)
                            } else {
                                Color::srgb(0., 1., 0.)
                            },
                            ..default()
                        },
                    ))
                    .id(),
            );
        }
        state.samples = "phase,panel,point,x,y,expected\n".into();
        state.frame = 0;
        return;
    }
    if state.frame == 8 {
        state.titles = state
            .owners
            .iter()
            .map(|&owner| gui.child_by_name(owner, "title").unwrap())
            .collect();
    }
    if [40, 80, 120].contains(&state.frame) {
        let mut samples = String::new();
        for (index, &title) in state.titles.iter().take(2).enumerate() {
            let info = paragraphs.get(title).unwrap();
            assert_eq!(info.run_geometry.len(), 1);
            let run = &info.run_geometry[0];
            let scale = info.scale_factor;
            let (position, size) = if index == 0 {
                (
                    run.strikethrough_position(),
                    run.strikethrough_size() + Vec2::new(8. * scale, 0.),
                )
            } else {
                (run.underline_position(), run.underline_size())
            };
            let bounds = Rect::from_center_size(position, size);
            let (node, global) = nodes.get(title).unwrap();
            let center = node.content_box().min + position;
            for (name, point, expected) in [
                ("center", center, "paint"),
                (
                    "inside_left",
                    Vec2::new(node.content_box().min.x + bounds.min.x + scale, center.y),
                    "paint",
                ),
                (
                    "inside_right",
                    Vec2::new(node.content_box().min.x + bounds.max.x - scale, center.y),
                    "paint",
                ),
                (
                    "outside_left",
                    Vec2::new(
                        node.content_box().min.x + bounds.min.x - 2. * scale,
                        center.y,
                    ),
                    "background",
                ),
                (
                    "outside_right",
                    Vec2::new(
                        node.content_box().min.x + bounds.max.x + 2. * scale,
                        center.y,
                    ),
                    "background",
                ),
                (
                    "above",
                    center - Vec2::new(0., size.y * 0.5 + 2. * scale),
                    "background",
                ),
                (
                    "below",
                    center + Vec2::new(0., size.y * 0.5 + 2. * scale),
                    "background",
                ),
            ] {
                let point = global.transform_point2(point);
                let expected = if state.frame == 80 {
                    "background"
                } else {
                    expected
                };
                samples.push_str(&format!(
                    "{},{index},{name},{},{},{expected}\n",
                    state.frame, point.x, point.y
                ));
            }
        }
        state.samples.push_str(&samples);
        std::fs::write("target/text-decoration-samples.csv", &state.samples).unwrap();
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(match state.frame {
                40 => "target/fairygui-demo-text-decorations.png",
                80 => "target/fairygui-demo-text-decorations-off.png",
                _ => "target/fairygui-demo-text-decorations-restored.png",
            }));
    }
    if state.frame == 55 || state.frame == 95 {
        for (index, &owner) in state.owners.iter().take(2).enumerate() {
            commands
                .entity(owner)
                .insert(GuiText::new(if state.frame == 55 {
                    SPACES.into()
                } else if index == 0 {
                    format!("<strike>{SPACES}</strike>")
                } else {
                    format!("<u>{SPACES}</u>")
                }));
        }
    }
    if state.frame == 135 {
        for &owner in &state.owners {
            commands.entity(owner).despawn();
        }
    }
    if state.frame == 145 {
        assert!(state.owners.iter().all(|&owner| gui.view(owner).is_none()));
        info!("demo smoke PASSED: native text decoration geometry and dynamic restoration");
        exit.write(AppExit::Success);
    }
}

use super::*;
use bevy::text::{EditableText, TextEdit};
use fairygui_core::{FuiPackage, color::Color as GuiColor, package::ObjectType};

const FILL: GuiColor = GuiColor::new(20. / 255., 70. / 255., 110. / 255., 1.);
const RED: GuiColor = GuiColor::new(200. / 255., 30. / 255., 40. / 255., 1.);
const GREEN: GuiColor = GuiColor::new(40. / 255., 134. / 255., 40. / 255., 1.);
const PURPLE: GuiColor = GuiColor::new(140. / 255., 30. / 255., 200. / 255., 1.);
const ORANGE: GuiColor = GuiColor::new(230. / 255., 90. / 255., 20. / 255., 1.);

#[derive(Default)]
pub(super) struct State {
    stage: u8,
    since: u32,
    root: Option<usize>,
    owners: Vec<GObjectId>,
    views: Vec<Entity>,
    labels: Vec<Entity>,
}
impl State {
    pub fn complete(&self) -> bool {
        self.stage == 12
    }
}

fn composite_panels(
    state: &State,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    suffix: &str,
    filtered: bool,
    alpha: f32,
) {
    use fairygui_core::color_matrix::ColorFilterParams;
    use std::fmt::Write;
    let matrix = ColorFilterParams {
        saturation: -1.,
        ..default()
    }
    .to_matrix();
    let screenshot = |mut color: GuiColor| {
        if filtered {
            color = matrix.apply(color);
        }
        // Match the 8-bit offscreen image, then Bevy's linear framebuffer blend.
        let color = GuiColor::from_argb(color.to_argb());
        let color = Color::srgba(color.r, color.g, color.b, color.a * alpha).to_linear();
        let color = Color::LinearRgba(LinearRgba::new(
            color.red * color.alpha + 1. - color.alpha,
            color.green * color.alpha + 1. - color.alpha,
            color.blue * color.alpha + 1. - color.alpha,
            1.,
        ))
        .to_srgba();
        [color.red, color.green, color.blue].map(|v| (v * 255.).round() as u8)
    };
    let mut csv = String::from("panel,x0,y0,x1,y1,sr,sg,sb,fr,fg,fb,present,clip\n");
    for (i, &entity) in state.views.iter().enumerate() {
        let (node, transform) = nodes.get(entity).unwrap();
        let rect = node.content_box();
        let low = transform.transform_point2(rect.min);
        let high = transform.transform_point2(rect.max);
        let stroke = if i == 1 {
            GREEN
        } else {
            let color = [PURPLE, GREEN, ORANGE][i];
            GuiColor::new(color.r, color.g, color.b, 0.75)
        };
        let a = 0.5 + stroke.a * 0.5;
        let fill = GuiColor::new(
            (FILL.r * 0.5 + stroke.r * stroke.a * 0.5) / a,
            (FILL.g * 0.5 + stroke.g * stroke.a * 0.5) / a,
            (FILL.b * 0.5 + stroke.b * stroke.a * 0.5) / a,
            a,
        );
        let [sr, sg, sb] = screenshot(stroke);
        let [fr, fg, fb] = screenshot(fill);
        writeln!(
            csv,
            "{i},{},{},{},{},{sr},{sg},{sb},{fr},{fg},{fb},1,{}",
            low.x,
            low.y,
            high.x,
            high.y,
            u8::from(i == 2)
        )
        .unwrap();
    }
    std::fs::write(format!("target/text-stroke-panels{suffix}.csv"), csv).unwrap();
}

fn panels(
    state: &State,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    suffix: &str,
    colors: [GuiColor; 3],
    present: bool,
    clip: bool,
) {
    use std::fmt::Write;
    let mut csv = String::from("panel,x0,y0,x1,y1,sr,sg,sb,fr,fg,fb,present,clip\n");
    for (i, &entity) in state.views.iter().enumerate() {
        let (node, transform) = nodes.get(entity).unwrap();
        let r = node.content_box();
        let points = [
            r.min,
            r.max,
            Vec2::new(r.min.x, r.max.y),
            Vec2::new(r.max.x, r.min.y),
        ]
        .map(|p| transform.transform_point2(p));
        let low = points.into_iter().reduce(Vec2::min).unwrap();
        let high = points.into_iter().reduce(Vec2::max).unwrap();
        let s = colors[i].to_argb();
        let f = FILL.to_argb();
        writeln!(
            csv,
            "{i},{},{},{},{},{},{},{},{},{},{},{},{}",
            low.x,
            low.y,
            high.x,
            high.y,
            (s >> 16) & 255,
            (s >> 8) & 255,
            s & 255,
            (f >> 16) & 255,
            (f >> 8) & 255,
            f & 255,
            u8::from(present),
            u8::from(clip && i == 2)
        )
        .unwrap();
    }
    std::fs::write(format!("target/text-stroke-panels{suffix}.csv"), csv).unwrap();
}

#[allow(clippy::too_many_arguments)]
pub(super) fn drive(
    state: &mut State,
    frames: u32,
    runtime: &mut GuiRuntime,
    assets: &mut Assets<FuiAsset>,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    editors: &Query<&EditableText>,
    commands: &mut Commands,
    original: GObjectId,
) {
    match state.stage {
        0 => {
            let package = FuiPackage::parse(
                "Basics",
                include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui"),
            )
            .unwrap();
            let handle = assets.add(FuiAsset {
                package: package.clone(),
                res_key: "ui/Basics".into(),
            });
            let xy = runtime
                .store()
                .object(original)
                .map(|o| Vec2::new(o.x(), o.y()))
                .unwrap();
            let index = runtime.create_root(handle, &package, "Demo_Text").unwrap();
            let root = runtime.root_object(index).unwrap();
            runtime.store_mut().set_xy(root, xy.x, xy.y);
            for child in runtime.store().object(root).unwrap().children().to_vec() {
                runtime.store_mut().set_visible(child, false);
            }
            runtime.store_mut().set_visible(original, false);
            let background = runtime.store_mut().create(ObjectType::Graph);
            runtime.store_mut().set_parent(background, Some(root));
            runtime.store_mut().set_xy(background, 70., 180.);
            runtime.store_mut().set_size(background, 1000., 420.);
            runtime.store_mut().set_graph(
                background,
                fairygui_core::widget::GraphData {
                    graph_type: 1,
                    fill_color: GuiColor::WHITE,
                    ..default()
                },
            );
            runtime.queue_subtree(background);
            for i in 0..3 {
                let owner = runtime.store_mut().create(if i == 2 {
                    ObjectType::InputText
                } else {
                    ObjectType::Text
                });
                runtime.store_mut().set_parent(owner, Some(root));
                runtime
                    .store_mut()
                    .set_xy(owner, 90., 220. + i as f32 * 120.);
                runtime.store_mut().set_size(owner, 900., 75.);
                runtime.store_mut().set_object_font_size(owner, 38);
                runtime.store_mut().set_object_single_line(owner, true);
                runtime
                    .store_mut()
                    .set_control_color(owner, FILL, Some(RED));
                runtime.store_mut().set_object_stroke(owner, 4.);
                runtime.store_mut().set_object_ubb_enabled(owner, i == 1);
                runtime.store_mut().set_object_text(
                    owner,
                    [
                        "Outline 描边 O",
                        "[stroke=6,#288628]UBB 描边[/stroke] [stroke=0]plain[/stroke]",
                        "Edit 输入 O",
                    ][i],
                );
                runtime.queue_subtree(owner);
                state.owners.push(owner);
                state.labels.push(
                    commands
                        .spawn((
                            Text::new(
                                [
                                    "Vector stroke / field width and outline color",
                                    "UBB overrides / explicit zero keeps plain foreground",
                                    "Editable text / shared layout and cursor",
                                ][i],
                            ),
                            TextFont {
                                font_size: bevy::text::FontSize::Px(17.),
                                ..default()
                            },
                            TextColor(Color::BLACK),
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(xy.x + 90.),
                                top: Val::Px(xy.y + 193. + i as f32 * 120.),
                                ..default()
                            },
                            GlobalZIndex(652),
                        ))
                        .id(),
                );
            }
            state.root = Some(index);
            state.stage = 1;
            state.since = frames;
        }
        1 if frames - state.since >= 18 => {
            state.views = state
                .owners
                .iter()
                .map(|&o| runtime.entity_of(o).unwrap())
                .collect();
            panels(state, nodes, "", [RED, GREEN, RED], true, false);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-text-stroke.png"));
            state.stage = 2;
            state.since = frames;
        }
        2 if frames - state.since >= 10 => {
            runtime.store_mut().set_object_stroke(state.owners[0], 8.);
            runtime
                .store_mut()
                .set_control_outline_color(state.owners[0], PURPLE);
            runtime.store_mut().set_scale(state.owners[0], 1.05, 1.);
            runtime.store_mut().set_object_text(
                state.owners[1],
                "[stroke=6,#8c1ec8]Changed UBB[/stroke] [stroke=0]plain[/stroke]",
            );
            runtime
                .store_mut()
                .set_control_outline_color(state.owners[2], ORANGE);
            runtime.store_mut().set_object_stroke(state.owners[2], 6.);
            runtime
                .store_mut()
                .set_input_password(state.owners[2], true);
            runtime.store_mut().set_object_text(
                state.owners[2],
                "Password😀Password😀Password😀Password😀Password😀",
            );
            runtime.store_mut().set_size(state.owners[2], 450., 75.);
            runtime.request_input_focus(Some(state.owners[2]));
            let entity = state.views[2];
            commands.queue(move |world: &mut World| {
                world
                    .get_mut::<EditableText>(entity)
                    .unwrap()
                    .queue_edit(TextEdit::TextEnd(false));
            });
            state.stage = 3;
            state.since = frames;
        }
        3 if frames - state.since >= 18 => {
            assert!(editors.get(state.views[2]).unwrap().viewport.offset.x > 0.);
            assert!(
                editors
                    .get(state.views[2])
                    .unwrap()
                    .value()
                    .to_string()
                    .starts_with("Password😀")
            );
            panels(
                state,
                nodes,
                "-updated",
                [PURPLE, PURPLE, ORANGE],
                true,
                true,
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-text-stroke-updated.png"));
            state.stage = 4;
            state.since = frames;
        }
        4 if frames - state.since >= 10 => {
            for (i, &owner) in state.owners.iter().enumerate() {
                runtime.store_mut().set_object_stroke(owner, 16.);
                runtime.store_mut().set_alpha(owner, 0.5);
                let stroke = [PURPLE, GREEN, ORANGE][i];
                runtime.store_mut().set_control_color(
                    owner,
                    GuiColor::new(FILL.r, FILL.g, FILL.b, 0.5),
                    Some(GuiColor::new(stroke.r, stroke.g, stroke.b, 0.75)),
                );
            }
            runtime.store_mut().set_object_text(
                state.owners[1],
                "[stroke=16,#288628]MMMM 描边[/stroke] [stroke=0]plain[/stroke]",
            );
            state.stage = 5;
            state.since = frames;
        }
        5 if frames - state.since >= 18 => {
            composite_panels(state, nodes, "-composite", false, 0.5);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-text-stroke-composite.png",
                ));
            state.stage = 6;
            state.since = frames;
        }
        6 if frames - state.since >= 10 => {
            for &owner in &state.owners {
                runtime.store_mut().set_alpha(owner, 0.25);
                runtime.store_mut().set_color_filter(
                    owner,
                    Some(fairygui_core::color_matrix::ColorFilterParams {
                        saturation: -1.,
                        ..default()
                    }),
                );
            }
            state.stage = 7;
            state.since = frames;
        }
        7 if frames - state.since >= 18 => {
            composite_panels(state, nodes, "-composite-filtered", true, 0.25);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-text-stroke-composite-filtered.png",
                ));
            state.stage = 8;
            state.since = frames;
        }
        8 if frames - state.since >= 10 => {
            for &owner in &state.owners {
                runtime.store_mut().set_alpha(owner, 1.);
                runtime.store_mut().set_color_filter(owner, None);
                runtime.store_mut().set_control_color(owner, FILL, None);
                runtime.store_mut().set_object_stroke(owner, 0.);
                runtime.store_mut().set_object_ubb_enabled(owner, false);
            }
            runtime
                .store_mut()
                .set_object_text(state.owners[1], "Strokes disabled / plain foreground");
            state.stage = 9;
            state.since = frames;
        }
        9 if frames - state.since >= 14 => {
            panels(
                state,
                nodes,
                "-disabled",
                [PURPLE, PURPLE, ORANGE],
                false,
                true,
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-text-stroke-disabled.png",
                ));
            state.stage = 10;
            state.since = frames;
        }
        10 if frames - state.since >= 10 => {
            runtime.request_input_focus(None);
            runtime.remove_root(commands, state.root.unwrap());
            for &label in &state.labels {
                commands.entity(label).despawn();
            }
            runtime.store_mut().set_visible(original, true);
            state.stage = 11;
            state.since = frames;
        }
        11 if frames - state.since >= 10 => {
            assert!(state.views.iter().all(|&e| nodes.get(e).is_err()));
            state.stage = 12;
            println!(
                "demo smoke: vector text strokes, UBB width/color/zero overrides, runtime edits, editable/password display, clipped scrolling, translucent stroke/fill precomposition, filtered opacity, stroke disabling and disposal captured"
            );
        }
        _ => {}
    }
}

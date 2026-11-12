//! Real GPU overlap probes for primitive blends and isolated subtrees.
use super::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use fairygui_core::{
    color::Color as GuiColor, color_matrix::ColorFilterParams, object::BlendMode,
    package::ObjectType, widget::GraphData,
};
use std::fmt::Write;

const DST: GuiColor = GuiColor::new(0.25, 0.5, 0.7, 1.);
const SRC: GuiColor = GuiColor::new(0.8, 0.35, 0.15, 0.6);
const MODES: [BlendMode; 8] = [
    BlendMode::Normal,
    BlendMode::Add,
    BlendMode::Multiply,
    BlendMode::Screen,
    BlendMode::Erase,
    BlendMode::Mask,
    BlendMode::Add,
    BlendMode::Normal,
];
#[derive(Resource, Default)]
struct State {
    frames: u32,
    since: u32,
    step: u8,
    root: Option<GObjectId>,
    panels: Vec<GObjectId>,
    holders: Vec<GObjectId>,
    sources: Vec<GObjectId>,
    views: Vec<Entity>,
    extras: Vec<GObjectId>,
    texts: Vec<GObjectId>,
    viewport_offset: Vec2,
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<State>().add_systems(PreUpdate, drive);
}
fn graph(gui: &mut GuiRuntime, parent: GObjectId, rect: [f32; 4], color: GuiColor) -> GObjectId {
    let id = gui.store_mut().create(ObjectType::Graph);
    gui.store_mut().set_parent(id, Some(parent));
    gui.store_mut().set_xy(id, rect[0], rect[1]);
    gui.store_mut().set_size(id, rect[2], rect[3]);
    gui.store_mut().set_graph(
        id,
        GraphData {
            graph_type: 1,
            fill_color: color,
            ..default()
        },
    );
    id
}
fn component(gui: &mut GuiRuntime, parent: GObjectId, rect: [f32; 4]) -> GObjectId {
    let id = gui.store_mut().create(ObjectType::Component);
    gui.store_mut().set_parent(id, Some(parent));
    gui.store_mut().set_xy(id, rect[0], rect[1]);
    gui.store_mut().set_size(id, rect[2], rect[3]);
    id
}
fn lin(color: GuiColor) -> [f32; 4] {
    let c = Color::srgba(color.r, color.g, color.b, color.a).to_linear();
    [
        c.red * color.a,
        c.green * color.a,
        c.blue * color.a,
        color.a,
    ]
}
fn over(src: [f32; 4], dst: [f32; 4], mode: BlendMode) -> [f32; 4] {
    std::array::from_fn(|i| {
        match mode {
            BlendMode::Add | BlendMode::Screen => src[i] + dst[i],
            BlendMode::Multiply => src[i] * dst[i] + dst[i] * (1. - src[3]),
            BlendMode::Erase => dst[i] * (1. - src[3]),
            BlendMode::Mask => dst[i] * src[3],
            _ => src[i] + dst[i] * (1. - src[3]),
        }
        .clamp(0., 1.)
    })
}
fn filtered(src: [f32; 4], filter: ColorFilterParams) -> [f32; 4] {
    if src[3] == 0. {
        return src;
    }
    let c =
        Color::linear_rgba(src[0] / src[3], src[1] / src[3], src[2] / src[3], src[3]).to_srgba();
    lin(filter
        .to_matrix()
        .apply(GuiColor::new(c.red, c.green, c.blue, c.alpha)))
}
fn expected(i: usize, probe: usize, updated: bool) -> [u8; 3] {
    let dst = lin(DST);
    let result = if probe == 1 {
        dst
    } else if i < 8 {
        let src = if i == 5 && probe == 2 {
            [0.; 4]
        } else {
            lin(SRC)
        };
        over(src, dst, if updated { BlendMode::Normal } else { MODES[i] })
    } else {
        let first = lin(SRC);
        let second = lin(GuiColor::new(0.15, 0.7, 0.85, 0.65));
        let mut src = over(second, first, BlendMode::Normal);
        let mut opacity = if updated { 0.8 } else { 0.5 };
        let mut mode = BlendMode::Normal;
        if i == 9 {
            // Inner hue, outer desaturation/brightness, then group alpha.
            src = filtered(
                over(
                    filtered(
                        second,
                        ColorFilterParams {
                            hue: 0.5,
                            ..default()
                        },
                    ),
                    first,
                    BlendMode::Normal,
                ),
                ColorFilterParams {
                    saturation: -1.,
                    brightness: 0.1,
                    ..default()
                },
            );
        } else if i == 10 {
            src = filtered(
                src,
                ColorFilterParams {
                    brightness: 0.2,
                    ..default()
                },
            );
            mode = BlendMode::Multiply;
        } else if i == 11 {
            mode = BlendMode::Mask;
            src = [0.; 4];
            opacity = 1.;
        }
        for c in &mut src {
            *c *= opacity;
        }
        over(src, dst, mode)
    };
    let c = Color::linear_rgb(result[0], result[1], result[2]).to_srgba();
    [c.red, c.green, c.blue].map(|c| (c.clamp(0., 1.) * 255.).round() as u8)
}
fn capture(
    state: &State,
    gui: &GuiRuntime,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    commands: &mut Commands,
    suffix: &str,
) {
    let mut csv = String::from("panel,probe,x,y,r,g,b\n");
    for (i, &id) in state.panels.iter().enumerate() {
        let (node, transform) = nodes.get(gui.entity_of(id).unwrap()).unwrap();
        for (probe, p) in [
            Vec2::new(105., 75.),
            Vec2::new(25., 75.),
            Vec2::new(145., 75.),
        ]
        .into_iter()
        .enumerate()
        {
            // Group probes stay in both overlapping quads (or the mask hole).
            let p = if i >= 8 && probe == 2 {
                Vec2::new(110., 80.)
            } else {
                p
            };
            let p = transform.transform_point2(p / node.inverse_scale_factor() - node.size() * 0.5)
                + state.viewport_offset;
            let c = expected(i, probe, !suffix.is_empty());
            writeln!(
                csv,
                "{i},{probe},{},{},{},{},{}",
                p.x, p.y, c[0], c[1], c[2]
            )
            .unwrap();
        }
        if i < 8 {
            let p = transform.transform_point2(
                Vec2::new(200., 60.) / node.inverse_scale_factor() - node.size() * 0.5,
            ) + state.viewport_offset;
            let c = expected(i, 0, !suffix.is_empty());
            writeln!(
                csv,
                "{i},polygon,{},{},{},{},{}",
                p.x, p.y, c[0], c[1], c[2]
            )
            .unwrap();
        }
    }
    let mut text_csv = String::from("panel,left,top,right,bottom,r,g,b\n");
    for (i, &id) in state.texts.iter().enumerate() {
        let (node, t) = nodes.get(gui.entity_of(id).unwrap()).unwrap();
        let min = t.transform_point2(-node.size() * 0.5) + state.viewport_offset;
        let max = t.transform_point2(node.size() * 0.5) + state.viewport_offset;
        let c = expected(i, 0, !suffix.is_empty());
        writeln!(
            text_csv,
            "{i},{},{},{},{},{},{},{}",
            min.x, min.y, max.x, max.y, c[0], c[1], c[2]
        )
        .unwrap();
    }
    std::fs::write(format!("target/compositor-texts{suffix}.csv"), text_csv).unwrap();
    std::fs::write(format!("target/compositor-samples{suffix}.csv"), csv).unwrap();
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(format!(
            "target/fairygui-demo-compositor{suffix}.png"
        )));
}
#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    demo: Res<Demo>,
    mut gui: NonSendMut<GuiRuntime>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    mut windows: Query<&mut Window>,
    mut images: ResMut<Assets<Image>>,
    mut cameras: Query<&mut Camera, With<Camera2d>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
    time: Res<Time<bevy::time::Real>>,
) {
    state.frames += 1;
    assert!(
        time.elapsed_secs_f64() < 40.,
        "compositor smoke timed out at {}",
        state.step
    );
    let Some(menu) = demo.menu.and_then(|r| gui.root_object(r)) else {
        return;
    };
    let elapsed = state.frames - state.since;
    match state.step {
        0 => {
            let root = component(&mut gui, menu, [25., 100., 1060., 480.]);
            graph(&mut gui, root, [0., 0., 1060., 480.], GuiColor::WHITE);
            for i in 0..12 {
                let panel = component(
                    &mut gui,
                    root,
                    [
                        10. + (i % 4) as f32 * 260.,
                        (i / 4) as f32 * 155.,
                        250.,
                        150.,
                    ],
                );
                graph(&mut gui, panel, [10., 30., 220., 90.], DST);
                let holder = component(&mut gui, panel, [0., 0., 240., 140.]);
                if i < 8 {
                    let polygon = graph(&mut gui, holder, [180., 45., 35., 30.], SRC);
                    gui.store_mut().set_graph(
                        polygon,
                        GraphData {
                            graph_type: 3,
                            fill_color: SRC,
                            polygon_points: Some(vec![0., 0., 35., 0., 35., 30., 0., 30.]),
                            ..default()
                        },
                    );
                    let text = gui.store_mut().create(ObjectType::Text);
                    gui.store_mut().set_parent(text, Some(holder));
                    gui.store_mut().set_xy(text, 180., 80.);
                    gui.store_mut().set_size(text, 48., 32.);
                    gui.store_mut().set_object_font_size(text, 24);
                    gui.store_mut().set_control_color(text, SRC, None);
                    gui.store_mut().set_object_text(text, "HH");
                    if i != 6 {
                        for id in [polygon, text] {
                            gui.store_mut().set_blend_mode(id, MODES[i]);
                        }
                    }
                    state.extras.extend([polygon, text]);
                    state.texts.push(text);
                }
                let source = if i == 5 {
                    component(&mut gui, holder, [60., 45., 100., 60.])
                } else {
                    graph(&mut gui, holder, [60., 45., 100., 60.], SRC)
                };
                if i == 6 || i == 7 {
                    gui.store_mut().set_blend_mode(holder, BlendMode::Add);
                    if i == 7 {
                        gui.store_mut().set_blend_mode(source, BlendMode::Normal);
                    }
                } else if i < 8 {
                    gui.store_mut().set_blend_mode(source, MODES[i]);
                } else {
                    gui.store_mut().set_alpha(holder, 0.5);
                    gui.store_mut()
                        .set_color_filter(holder, Some(ColorFilterParams::default()));
                    let nested = component(&mut gui, holder, [0., 0., 240., 140.]);
                    let second = graph(
                        &mut gui,
                        nested,
                        [85., 55., 100., 60.],
                        GuiColor::new(0.15, 0.7, 0.85, 0.65),
                    );
                    if i == 9 {
                        gui.store_mut().set_color_filter(
                            holder,
                            Some(ColorFilterParams {
                                saturation: -1.,
                                brightness: 0.1,
                                ..default()
                            }),
                        );
                        gui.store_mut().set_color_filter(
                            nested,
                            Some(ColorFilterParams {
                                hue: 0.5,
                                ..default()
                            }),
                        );
                    } else if i == 10 {
                        gui.store_mut().set_color_filter(
                            holder,
                            Some(ColorFilterParams {
                                brightness: 0.2,
                                ..default()
                            }),
                        );
                        gui.store_mut().set_blend_mode(holder, BlendMode::Multiply);
                    } else if i == 11 {
                        gui.store_mut().set_alpha(holder, 1.);
                        gui.store_mut().set_blend_mode(holder, BlendMode::Mask);
                        gui.store_mut().set_size(source, 25., 60.);
                        gui.store_mut().set_xy(nested, 135., 45.);
                        gui.store_mut().set_size(nested, 25., 60.);
                        gui.store_mut().set_xy(second, 0., 0.);
                        gui.store_mut().set_size(second, 25., 60.);
                    }
                }
                let label = gui.store_mut().create(ObjectType::Text);
                gui.store_mut().set_parent(label, Some(panel));
                gui.store_mut().set_xy(label, 10., 3.);
                gui.store_mut().set_size(label, 240., 23.);
                gui.store_mut().set_object_font_size(label, 14);
                gui.store_mut()
                    .set_control_color(label, GuiColor::new(0., 0., 0., 1.), None);
                gui.store_mut().set_object_text(
                    label,
                    [
                        "Normal",
                        "Add",
                        "Multiply",
                        "Screen (= Add)",
                        "Erase",
                        "Mask / transparent texels",
                        "Inherited Add",
                        "Explicit Normal",
                        "Isolated alpha",
                        "Nested hue + gray",
                        "Filtered multiply",
                        "Isolated mask hole",
                    ][i],
                );
                state.panels.push(panel);
                state.holders.push(holder);
                state.sources.push(source);
            }
            gui.queue_subtree(root);
            state.root = Some(root);
            state.step = 1;
            state.since = state.frames;
        }
        1 if elapsed >= 15 => {
            let root = gui.entity_of(state.root.unwrap()).unwrap();
            commands.entity(root).insert(GlobalZIndex(600));
            // A transparent half tests destination-in coverage inside a quad.
            let rgba: Vec<u8> = (0..2)
                .flat_map(|_| [[204, 89, 38, 255], [204, 89, 38, 0]].into_iter().flatten())
                .collect();
            let mut image = Image::new_fill(
                Extent3d {
                    width: 2,
                    height: 2,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                &rgba,
                TextureFormat::Rgba8UnormSrgb,
                default(),
            );
            image.sampler = bevy::image::ImageSampler::nearest();
            let image = images.add(image);
            let source = gui.entity_of(state.sources[5]).unwrap();
            commands.entity(source).insert(ImageNode {
                image,
                image_mode: bevy::ui::widget::NodeImageMode::Stretch,
                color: Color::srgba(1., 1., 1., 0.6),
                ..default()
            });
            // Give this source its alpha through the owner policy as well.
            gui.store_mut().set_alpha(state.sources[5], 0.6);
            state.views = state
                .panels
                .iter()
                .map(|&id| gui.entity_of(id).unwrap())
                .collect();
            state.step = 2;
            state.since = state.frames;
        }
        2 if elapsed >= 20 => {
            capture(&state, &gui, &nodes, &mut commands, "");
            state.step = 3;
            state.since = state.frames;
        }
        3 if elapsed >= 12 => {
            for i in 0..8 {
                gui.store_mut()
                    .set_blend_mode(state.holders[i], BlendMode::Normal);
                gui.store_mut()
                    .set_blend_mode(state.sources[i], BlendMode::Normal);
            }
            for i in 8..11 {
                gui.store_mut().set_alpha(state.holders[i], 0.8);
            }
            for &id in &state.extras {
                gui.store_mut().set_blend_mode(id, BlendMode::Normal);
            }
            state.step = 4;
            state.since = state.frames;
        }
        4 if elapsed >= 20 => {
            capture(&state, &gui, &nodes, &mut commands, "-updated");
            state.step = 5;
            state.since = state.frames;
        }
        5 if elapsed >= 12 => {
            windows.single_mut().unwrap().resolution.set(1120., 680.);
            state.step = 6;
            state.since = state.frames;
        }
        6 if elapsed >= 25 => {
            for (&id, &view) in state.panels.iter().zip(&state.views) {
                assert_eq!(gui.entity_of(id), Some(view));
            }
            capture(&state, &gui, &nodes, &mut commands, "-resized");
            state.step = 7;
            state.since = state.frames;
        }
        7 if elapsed >= 15 => {
            let window = windows.single_mut().unwrap();
            gui.store_mut().set_xy(state.root.unwrap(), -55., 100.);
            state.viewport_offset = Vec2::new(30., 20.);
            cameras.single_mut().unwrap().viewport = Some(bevy::camera::Viewport {
                physical_position: UVec2::new(30, 20),
                physical_size: UVec2::new(
                    window.physical_width() - 60,
                    window.physical_height() - 40,
                ),
                ..default()
            });
            state.step = 8;
            state.since = state.frames;
        }
        8 if elapsed >= 25 => {
            capture(&state, &gui, &nodes, &mut commands, "-viewport");
            state.step = 9;
            state.since = state.frames;
        }
        9 if elapsed >= 15 => {
            gui.store_mut().remove(state.root.unwrap());
            state.step = 10;
            state.since = state.frames;
        }
        10 if elapsed >= 15 => {
            assert!(state.panels.iter().all(|&id| gui.entity_of(id).is_none()));
            eprintln!(
                "compositor smoke PASSED: 12 overlap panels, native blends/inheritance, transparent mask coverage, text/polygon draws, nested filters, isolated alpha, resize, offset viewport, retained views and disposal; 176 GPU probes and 32 text regions captured"
            );
            exit.write(AppExit::Success);
            state.step = 11;
        }
        _ => (),
    }
}

//! Alpha masks, reverse paint order, nested caches and actual input hit gates.
use super::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use fairygui_core::{
    color::Color as GuiColor, color_matrix::ColorFilterParams, package::ObjectType,
    widget::GraphData,
};
use std::fmt::Write;

const CONTENT: GuiColor = GuiColor::new(0.8, 0.25, 0.35, 1.);
const BACK: GuiColor = GuiColor::new(0.25, 0.5, 0.7, 1.);
const PROBES: [Vec2; 6] = [
    Vec2::new(10., 10.),
    Vec2::new(40., 35.),
    Vec2::new(95., 35.),
    Vec2::new(95., 65.),
    Vec2::new(120., 65.),
    Vec2::new(160., 95.),
];
#[derive(Resource, Default)]
struct State {
    frame: u32,
    since: u32,
    step: u8,
    root: Option<GObjectId>,
    panels: Vec<GObjectId>,
    owners: Vec<GObjectId>,
    masks: Vec<GObjectId>,
    contents: Vec<GObjectId>,
    backs: Vec<GObjectId>,
    views: Vec<Entity>,
    cursor: Option<Vec2>,
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<State>().add_systems(
        PreUpdate,
        drive
            .after(InputSystems)
            .before(UiSystems::Focus)
            .before(PickingSystems::ProcessInput),
    );
}
fn node(gui: &mut GuiRuntime, parent: GObjectId, rect: [f32; 4], kind: ObjectType) -> GObjectId {
    let id = gui.store_mut().create(kind);
    gui.store_mut().set_parent(id, Some(parent));
    gui.store_mut().set_xy(id, rect[0], rect[1]);
    gui.store_mut().set_size(id, rect[2], rect[3]);
    id
}
fn graph(gui: &mut GuiRuntime, parent: GObjectId, rect: [f32; 4], color: GuiColor) -> GObjectId {
    let id = node(gui, parent, rect, ObjectType::Graph);
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
fn shape(gui: &mut GuiRuntime, parent: GObjectId, kind: usize) -> GObjectId {
    let mask = if [3, 7, 10].contains(&kind) {
        node(gui, parent, [30., 20., 100., 60.], ObjectType::Component)
    } else {
        graph(gui, parent, [30., 20., 100., 60.], GuiColor::WHITE)
    };
    gui.store_mut().set_touchable(mask, false);
    if [1, 7].contains(&kind) {
        if kind == 1 {
            gui.store_mut().set_size(mask, 80., 80.);
            gui.store_mut().set_xy(mask, 40., 10.);
            gui.store_mut().set_graph(
                mask,
                GraphData {
                    graph_type: 2,
                    fill_color: GuiColor::WHITE,
                    ..default()
                },
            );
        } else {
            graph(gui, mask, [0., 0., 100., 60.], GuiColor::WHITE);
            let inner = shape(gui, mask, 1);
            gui.store_mut().set_xy(inner, 20., 0.);
            gui.store_mut().set_size(inner, 60., 60.);
            gui.store_mut().set_component_mask(mask, Some(inner), false);
            gui.store_mut().set_alpha(mask, 0.8);
        }
    } else if kind == 2 {
        gui.store_mut().set_graph(
            mask,
            GraphData {
                graph_type: 3,
                fill_color: GuiColor::WHITE,
                polygon_points: Some(vec![
                    0., 0., 100., 0., 100., 20., 40., 20., 40., 60., 0., 60.,
                ]),
                ..default()
            },
        );
    } else if [4, 5, 9].contains(&kind) {
        gui.store_mut().set_alpha(mask, 0.6);
    } else if kind == 6 {
        gui.store_mut().set_size(mask, 90., 50.);
        gui.store_mut().set_pivot(mask, 0.5, 0.5, true);
        gui.store_mut().set_xy(mask, 80., 50.);
        gui.store_mut().set_rotation(mask, 45.);
    } else if kind == 10 {
        graph(gui, mask, [0., 0., 35., 60.], GuiColor::WHITE);
        let text = node(gui, mask, [50., 5., 50., 45.], ObjectType::Text);
        gui.store_mut().set_object_font_size(text, 26);
        gui.store_mut()
            .set_control_color(text, GuiColor::WHITE, None);
        gui.store_mut().set_object_text(text, "HH");
        gui.store_mut().set_alpha(mask, 0.5);
    } else if kind == 3 {
        gui.store_mut().set_alpha(mask, 0.8);
    }
    mask
}
fn contains_rect(p: Vec2, rect: [f32; 4]) -> bool {
    p.x >= rect[0] && p.y >= rect[1] && p.x < rect[0] + rect[2] && p.y < rect[1] + rect[3]
}
fn alpha(i: usize, p: Vec2, updated: bool) -> f32 {
    let shift = if updated { 10. } else { 0. };
    let p = p - Vec2::new(shift, 0.);
    if i == 2 && updated {
        return 1.;
    }
    if i == 4 && updated {
        return 0.7;
    }
    if i == 5 && !updated {
        return 0.7;
    }
    let inside = if i == 1 || (i == 0 && updated) {
        (p - Vec2::new(80., 50.)).length_squared() < 40_f32.powi(2)
    } else if i == 2 {
        contains_rect(p, [30., 20., 100., 20.]) || contains_rect(p, [30., 20., 40., 60.])
    } else if i == 6 {
        let delta = p - Vec2::new(80., 50.);
        let angle = if updated { -25_f32 } else { 45_f32 }.to_radians();
        contains_rect(
            Vec2::new(
                delta.x * angle.cos() + delta.y * angle.sin(),
                -delta.x * angle.sin() + delta.y * angle.cos(),
            ),
            [-45., -25., 90., 50.],
        )
    } else if i == 7 {
        !updated && (p - Vec2::new(80., 50.)).length_squared() < 30_f32.powi(2)
    } else if i == 10 {
        contains_rect(p, [30., 20., 35., 60.])
    } else if i == 11 {
        (p - Vec2::new(80., 50.)).length_squared() < 40_f32.powi(2)
    } else {
        contains_rect(p, [30., 20., 100., 60.])
    };
    let mask = if !inside {
        0.
    } else if i == 3 {
        let tex = if p.x < 80. { 1. } else { 64. / 255. };
        tex * if updated { 0.4 } else { 0.8 }
    } else if i == 7 {
        0.8
    } else if i == 10 {
        0.5
    } else if [4, 5, 9].contains(&i) {
        0.6
    } else {
        1.
    };
    let result = if [4, 5, 9].contains(&i) || (i == 1 && updated) {
        1. - mask
    } else {
        mask
    };
    let opacity = if [4, 5, 9].contains(&i) {
        0.7
    } else if i == 8 {
        0.5
    } else if i == 11 {
        0.75 * 0.5
    } else {
        1.
    };
    let inner = if i == 11 {
        contains_rect(p + Vec2::new(shift, 0.), [60., 20., 80., 60.])
    } else {
        true
    };
    result * opacity * if inner { 1. } else { 0. }
}
fn expected(i: usize, alpha: f32) -> [u8; 3] {
    let filter = if i == 8 {
        Some(ColorFilterParams {
            brightness: 0.15,
            saturation: -1.,
            ..default()
        })
    } else if i == 9 {
        Some(ColorFilterParams {
            hue: 0.5,
            ..default()
        })
    } else {
        None
    };
    let c = filter.map_or(CONTENT, |f| f.to_matrix().apply(CONTENT));
    let src = Color::srgb(c.r, c.g, c.b).to_linear();
    let dst = Color::srgb(BACK.r, BACK.g, BACK.b).to_linear();
    let c = Color::linear_rgb(
        src.red * alpha + dst.red * (1. - alpha),
        src.green * alpha + dst.green * (1. - alpha),
        src.blue * alpha + dst.blue * (1. - alpha),
    )
    .to_srgba();
    [c.red, c.green, c.blue].map(|v| (v.clamp(0., 1.) * 255.).round() as u8)
}
fn capture(
    state: &State,
    gui: &GuiRuntime,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    commands: &mut Commands,
    suffix: &str,
) {
    let mut csv = String::from("panel,probe,x,y,r,g,b\n");
    for (i, &id) in state.owners.iter().enumerate() {
        let (n, t) = nodes.get(gui.entity_of(id).unwrap()).unwrap();
        for (j, &p) in PROBES.iter().enumerate() {
            if i == 10 && [2, 3, 4].contains(&j) {
                continue;
            }
            let pixel = t.transform_point2(p / n.inverse_scale_factor() - n.size() * 0.5);
            let c = expected(i, alpha(i, p, !suffix.is_empty()));
            writeln!(
                csv,
                "{i},{j},{},{},{},{},{}",
                pixel.x, pixel.y, c[0], c[1], c[2]
            )
            .unwrap();
        }
    }
    std::fs::write(format!("target/mask-samples{suffix}.csv"), csv).unwrap();
    let (n, t) = nodes.get(gui.entity_of(state.masks[10]).unwrap()).unwrap();
    let min = t.transform_point2(Vec2::new(50., 5.) / n.inverse_scale_factor() - n.size() * 0.5);
    let max = t.transform_point2(Vec2::new(99., 49.) / n.inverse_scale_factor() - n.size() * 0.5);
    let c = expected(10, 0.5);
    std::fs::write(
        format!("target/mask-texts{suffix}.csv"),
        format!(
            "panel,left,top,right,bottom,r,g,b\n10,{},{},{},{},{},{},{}\n",
            min.x, min.y, max.x, max.y, c[0], c[1], c[2]
        ),
    )
    .unwrap();
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(format!(
            "target/fairygui-demo-masks{suffix}.png"
        )));
}
#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    demo: Res<Demo>,
    mut gui: NonSendMut<GuiRuntime>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    mut windows: Query<(Entity, &mut Window)>,
    mut image_assets: ResMut<Assets<Image>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    input: Res<GuiInputState>,
    time: Res<Time<bevy::time::Real>>,
) {
    state.frame += 1;
    assert!(
        time.elapsed_secs_f64() < 50.,
        "mask window timeout at {}",
        state.step
    );
    let Some(menu) = demo.menu.and_then(|r| gui.root_object(r)) else {
        return;
    };
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    let elapsed = state.frame - state.since;
    match state.step {
        0 => {
            let root = node(
                &mut gui,
                menu,
                [25., 110., 1060., 465.],
                ObjectType::Component,
            );
            graph(&mut gui, root, [0., 0., 1060., 465.], GuiColor::WHITE);
            for i in 0..12 {
                let panel = node(
                    &mut gui,
                    root,
                    [
                        10. + (i % 4) as f32 * 260.,
                        (i / 4) as f32 * 150.,
                        250.,
                        145.,
                    ],
                    ObjectType::Component,
                );
                let back = graph(&mut gui, panel, [10., 26., 220., 115.], BACK);
                let owner = node(
                    &mut gui,
                    panel,
                    [15., 30., 180., 110.],
                    ObjectType::Component,
                );
                let early = (i == 5).then(|| shape(&mut gui, owner, i));
                let content = if i == 11 {
                    let inner = node(&mut gui, owner, [0., 0., 180., 110.], ObjectType::Component);
                    graph(&mut gui, inner, [0., 0., 180., 110.], CONTENT);
                    let clip = graph(&mut gui, inner, [60., 20., 80., 60.], GuiColor::WHITE);
                    gui.store_mut().set_touchable(clip, false);
                    gui.store_mut().set_component_mask(inner, Some(clip), false);
                    gui.store_mut().set_alpha(inner, 0.5);
                    inner
                } else {
                    graph(&mut gui, owner, [0., 0., 180., 110.], CONTENT)
                };
                gui.store_mut().set_touchable(content, false);
                gui.store_mut().set_touchable(content, true);
                let mask =
                    early.unwrap_or_else(|| shape(&mut gui, owner, if i == 11 { 1 } else { i }));
                gui.store_mut()
                    .set_component_mask(owner, Some(mask), [4, 5, 9].contains(&i));
                if [4, 5, 9].contains(&i) {
                    gui.store_mut().set_alpha(owner, 0.7);
                }
                if i == 8 {
                    gui.store_mut().set_alpha(owner, 0.5);
                    gui.store_mut().set_color_filter(
                        owner,
                        Some(ColorFilterParams {
                            brightness: 0.15,
                            saturation: -1.,
                            ..default()
                        }),
                    );
                }
                if i == 9 {
                    gui.store_mut().set_color_filter(
                        owner,
                        Some(ColorFilterParams {
                            hue: 0.5,
                            ..default()
                        }),
                    );
                }
                if i == 11 {
                    gui.store_mut().set_alpha(owner, 0.75);
                }
                let label = node(&mut gui, panel, [10., 3., 240., 20.], ObjectType::Text);
                gui.store_mut().set_object_font_size(label, 13);
                gui.store_mut()
                    .set_control_color(label, GuiColor::new(0., 0., 0., 1.), None);
                gui.store_mut().set_object_text(
                    label,
                    [
                        "Normal rect",
                        "Normal circle",
                        "Normal concave polygon",
                        "Image alpha / no hit area",
                        "Reverse after content",
                        "Reverse before content",
                        "Rotated normal mask",
                        "Mask with nested mask",
                        "Filter / owner alpha",
                        "Reverse + hue / alpha",
                        "Text + graph mask",
                        "Nested masked content",
                    ][i],
                );
                state.panels.push(panel);
                state.owners.push(owner);
                state.masks.push(mask);
                state.contents.push(content);
                state.backs.push(back);
            }
            gui.queue_subtree(root);
            state.root = Some(root);
            state.step = 1;
            state.since = state.frame;
        }
        1 if elapsed >= 15 => {
            commands
                .entity(gui.entity_of(state.root.unwrap()).unwrap())
                .insert(GlobalZIndex(600));
            let mut image = Image::new_fill(
                Extent3d {
                    width: 2,
                    height: 2,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                &[
                    255, 255, 255, 255, 255, 255, 255, 64, 255, 255, 255, 255, 255, 255, 255, 64,
                ],
                TextureFormat::Rgba8UnormSrgb,
                default(),
            );
            image.sampler = bevy::image::ImageSampler::nearest();
            let image = image_assets.add(image);
            commands
                .entity(gui.entity_of(state.masks[3]).unwrap())
                .insert(ImageNode {
                    image,
                    image_mode: bevy::ui::widget::NodeImageMode::Stretch,
                    ..default()
                });
            state.views = state
                .owners
                .iter()
                .map(|&id| gui.entity_of(id).unwrap())
                .collect();
            state.step = 2;
            state.since = state.frame;
        }
        2 if elapsed >= 20 => {
            capture(&state, &gui, &nodes, &mut commands, "");
            state.step = 3;
            state.since = state.frame;
        }
        3 if elapsed >= 12 => {
            let owner = state.owners[0];
            let old = state.masks[0];
            let new = shape(&mut gui, owner, 1);
            gui.store_mut().set_component_mask(owner, Some(new), false);
            gui.store_mut().set_visible(old, false);
            gui.queue_subtree(new);
            state.masks[0] = new;
            gui.store_mut()
                .set_component_mask(state.owners[1], Some(state.masks[1]), true);
            for i in [2, 4] {
                gui.store_mut()
                    .set_component_mask(state.owners[i], None, false);
                gui.store_mut().set_visible(state.masks[i], false);
            }
            gui.store_mut().set_alpha(state.masks[3], 0.4);
            gui.store_mut()
                .add_child_at(state.owners[5], state.masks[5], usize::MAX);
            gui.store_mut().set_rotation(state.masks[6], -25.);
            gui.store_mut().set_visible(state.masks[7], false);
            for &id in &state.masks {
                let o = gui.store().object(id).unwrap();
                let xy = (o.x() + 10., o.y());
                gui.store_mut().set_xy(id, xy.0, xy.1);
            }
            state.step = 4;
            state.since = state.frame;
        }
        4 if elapsed >= 25 => {
            capture(&state, &gui, &nodes, &mut commands, "-updated");
            state.step = 5;
            state.since = state.frame;
        }
        5 if elapsed >= 12 => {
            let (_, t) = nodes.get(gui.entity_of(state.owners[0]).unwrap()).unwrap();
            let (n, _) = nodes.get(gui.entity_of(state.owners[0]).unwrap()).unwrap();
            state.cursor = Some(
                t.transform_point2(PROBES[2] / n.inverse_scale_factor() - n.size() * 0.5)
                    / window.scale_factor(),
            );
            state.step = 6;
            state.since = state.frame;
        }
        6 if elapsed >= 6 => {
            assert_eq!(input.hovered(), gui.entity_of(state.contents[0]));
            mouse.press(MouseButton::Left);
            state.step = 7;
            state.since = state.frame;
        }
        7 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            let (n, t) = nodes.get(gui.entity_of(state.owners[0]).unwrap()).unwrap();
            state.cursor = Some(
                t.transform_point2(PROBES[0] / n.inverse_scale_factor() - n.size() * 0.5)
                    / window.scale_factor(),
            );
            state.step = 8;
            state.since = state.frame;
        }
        8 if elapsed >= 6 => {
            assert_eq!(input.hovered(), gui.entity_of(state.backs[0]));
            let (n, t) = nodes.get(gui.entity_of(state.owners[3]).unwrap()).unwrap();
            state.cursor = Some(
                t.transform_point2(PROBES[0] / n.inverse_scale_factor() - n.size() * 0.5)
                    / window.scale_factor(),
            );
            state.step = 9;
            state.since = state.frame;
        }
        9 if elapsed >= 6 => {
            assert_eq!(
                input.hovered(),
                gui.entity_of(state.contents[3]),
                "an image mask without hitArea must not add a pixel-input gate"
            );
            let (n, t) = nodes.get(gui.entity_of(state.owners[5]).unwrap()).unwrap();
            state.cursor = Some(
                t.transform_point2(PROBES[2] / n.inverse_scale_factor() - n.size() * 0.5)
                    / window.scale_factor(),
            );
            state.step = 20;
            state.since = state.frame;
        }
        20 if elapsed >= 6 => {
            assert_eq!(
                input.hovered(),
                gui.entity_of(state.backs[5]),
                "reversed graphic hit gate rejects its interior"
            );
            let (n, t) = nodes.get(gui.entity_of(state.owners[5]).unwrap()).unwrap();
            state.cursor = Some(
                t.transform_point2(PROBES[0] / n.inverse_scale_factor() - n.size() * 0.5)
                    / window.scale_factor(),
            );
            state.step = 21;
            state.since = state.frame;
        }
        21 if elapsed >= 6 => {
            assert_eq!(input.hovered(), gui.entity_of(state.contents[5]));
            state.cursor = None;
            window.resolution.set(1120., 680.);
            state.step = 10;
            state.since = state.frame;
        }
        10 if elapsed >= 25 => {
            capture(&state, &gui, &nodes, &mut commands, "-resized");
            state.step = 11;
            state.since = state.frame;
        }
        11 if elapsed >= 12 => {
            for (&id, &view) in state.owners.iter().zip(&state.views) {
                assert_eq!(gui.entity_of(id), Some(view));
            }
            gui.store_mut().remove(state.root.unwrap());
            state.step = 12;
            state.since = state.frame;
        }
        12 if elapsed >= 12 => {
            assert!(state.owners.iter().all(|&id| gui.entity_of(id).is_none()));
            eprintln!(
                "mask smoke PASSED: normal/reversed order, image alpha, transforms, nested masks/content, filter/alpha, replacement/clear/hide, input gates, resize and disposal"
            );
            exit.write(AppExit::Success);
            state.step = 13;
        }
        _ => (),
    }
    window.set_cursor_position(state.cursor);
    super::move_pointer(window_entity, state.cursor, &mut pointers);
}

//! GPU samples cover overlap, inherited geometry, scroll edges and viewport offsets.
use super::*;
use fairygui_core::{color::Color as GuiColor, package::ObjectType, widget::GraphData};
use std::fmt::Write;

const BG: Color = Color::srgb(0.08, 0.13, 0.2);
const SRC: Color = Color::srgb(0.9, 0.35, 0.12);
#[derive(Resource, Default)]
struct State {
    frame: u32,
    since: u32,
    step: u8,
    asset: Option<Handle<FuiAsset>>,
    owners: Vec<GObjectId>,
    offset: Vec2,
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<State>().add_systems(PreUpdate, drive);
}

#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    demo: Res<Demo>,
    server: Res<AssetServer>,
    assets: Res<Assets<FuiAsset>>,
    mut gui: NonSendMut<GuiRuntime>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    settings: Query<&GuiScrollSettings>,
    view_sizes: Query<&GuiViewSize>,
    mut cameras: Query<&mut Camera, With<Camera2d>>,
    windows: Query<&Window>,
    clear: Res<ClearColor>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    assert!(state.frame < 1400, "soft clip timeout at {}", state.step);
    let elapsed = state.frame - state.since;
    let mut next = false;
    match state.step {
        0 => {
            let handle = state
                .asset
                .get_or_insert_with(|| server.load("ui/Basics.fui"))
                .clone();
            let Some(asset) = assets.get(&handle) else {
                return;
            };
            let Some(menu) = demo.menu.and_then(|index| gui.root_object(index)) else {
                return;
            };
            gui.store_mut().set_visible(menu, false);
            for i in 0..5 {
                let index = gui
                    .create_root(
                        handle.clone(),
                        &asset.package,
                        if i == 3 { "Component7" } else { "Component1" },
                    )
                    .unwrap();
                let owner = gui.root_object(index).unwrap();
                let children = gui.store().object(owner).unwrap().children().to_vec();
                for child in children {
                    if !gui.store().is_scroll_overlay(child) {
                        gui.store_mut().remove(child);
                    }
                }
                gui.store_mut().set_xy(owner, 40. + i as f32 * 240., 150.);
                gui.store_mut().set_size(owner, 180., 140.);
                if i == 2 {
                    gui.store_mut().set_pivot(owner, 0.5, 0.5, false);
                    gui.store_mut().set_scale(owner, 1.1, 0.85);
                    gui.store_mut().set_rotation(owner, 12.);
                }
                for _ in 0..if i == 1 { 2 } else { 1 } {
                    let child = gui.store_mut().create(ObjectType::Graph);
                    gui.store_mut().set_parent(child, Some(owner));
                    gui.store_mut().set_size(child, 400., 300.);
                    gui.store_mut().set_graph(
                        child,
                        GraphData {
                            graph_type: 1,
                            fill_color: GuiColor::new(0.9, 0.35, 0.12, 1.),
                            ..default()
                        },
                    );
                    if i == 4 {
                        gui.store_mut().set_blend_mode(child, BlendMode::Add);
                    }
                }
                if i == 1 {
                    let mask = gui.store_mut().create(ObjectType::Graph);
                    gui.store_mut().set_parent(mask, Some(owner));
                    gui.store_mut().set_size(mask, 400., 300.);
                    gui.store_mut().set_graph(
                        mask,
                        GraphData {
                            graph_type: 1,
                            fill_color: GuiColor::WHITE,
                            ..default()
                        },
                    );
                    gui.store_mut().set_component_mask(owner, Some(mask), false);
                }
                gui.queue_subtree(owner);
                state.owners.push(owner);
            }
            next = true;
        }
        1 if elapsed >= 20 && state.owners.iter().all(|&id| gui.entity_of(id).is_some()) => {
            for (i, &id) in state.owners.iter().enumerate() {
                commands.entity(gui.entity_of(id).unwrap()).insert((
                    BackgroundColor(BG),
                    GuiClipSoftness(if i == 3 {
                        Vec2::new(300., 200.)
                    } else {
                        Vec2::new(24., 20.)
                    }),
                ));
            }
            next = true;
        }
        2 if elapsed >= 20 => {
            capture(&state, &gui, &nodes, &mut commands, "", false);
            next = true;
        }
        3 if elapsed >= 12 => {
            let owner = state.owners[3];
            let mut value = settings.get(gui.entity_of(owner).unwrap()).unwrap().clone();
            value.softness_on_top_left = true;
            commands.entity(gui.entity_of(owner).unwrap()).insert(value);
            gui.store_mut().set_scroll_pos(owner, 80., 60., false);
            next = true;
        }
        4 if elapsed >= 20 => {
            capture(&state, &gui, &nodes, &mut commands, "-scrolled", false);
            next = true;
        }
        5 if elapsed >= 12 => {
            let window = windows.single().unwrap();
            state.offset = Vec2::new(37., 23.);
            cameras.single_mut().unwrap().viewport = Some(bevy::camera::Viewport {
                physical_position: UVec2::new(37, 23),
                physical_size: UVec2::new(
                    window.physical_width() - 60,
                    window.physical_height() - 40,
                ),
                ..default()
            });
            next = true;
        }
        6 if elapsed >= 20 => {
            capture(&state, &gui, &nodes, &mut commands, "-viewport", false);
            next = true;
        }
        7 if elapsed >= 12 => {
            for &id in &state.owners {
                commands
                    .entity(gui.entity_of(id).unwrap())
                    .insert(GuiClipSoftness(Vec2::ZERO));
            }
            next = true;
        }
        8 if elapsed >= 20 => {
            capture(&state, &gui, &nodes, &mut commands, "-disabled", true);
            next = true;
        }
        9 if elapsed >= 20 => {
            for &id in &state.owners {
                commands
                    .entity(gui.entity_of(id).unwrap())
                    .insert(GuiMargin::new(12., 8., 30., 24.));
            }
            next = true;
        }
        10 if elapsed >= 20 => {
            capture_hard_margins(&state, &gui, &nodes, &mut commands, clear.0, "-margins");
            next = true;
        }
        11 if elapsed >= 20 => {
            for &id in &state.owners {
                commands
                    .entity(gui.entity_of(id).unwrap())
                    .insert(GuiViewSize(Vec2::new(120., 90.)));
            }
            next = true;
        }
        12 if elapsed >= 20 => {
            for &id in &state.owners {
                let entity = gui.entity_of(id).unwrap();
                assert_eq!(view_sizes.get(entity).unwrap().0, Vec2::new(120., 90.));
                let object = gui.store().object(id).unwrap();
                let pane = object.control().component().unwrap().scroll_pane.as_ref();
                let vertical = pane
                    .filter(|p| !p.floating)
                    .and_then(|p| p.vt_bar)
                    .and_then(|b| gui.store().object(b))
                    .map_or(0., |b| b.width());
                let horizontal = pane
                    .filter(|p| !p.floating)
                    .and_then(|p| p.hz_bar)
                    .and_then(|b| gui.store().object(b))
                    .map_or(0., |b| b.height());
                assert_eq!(object.width(), 162. + vertical);
                assert_eq!(object.height(), 122. + horizontal);
                let (node, _) = nodes.get(entity).unwrap();
                assert!(
                    (node.size() * node.inverse_scale_factor())
                        .abs_diff_eq(Vec2::new(object.width(), object.height()), 0.001)
                );
            }
            capture_hard_margins(&state, &gui, &nodes, &mut commands, clear.0, "-view-size");
            next = true;
        }
        13 if elapsed >= 20 => {
            eprintln!(
                "soft clip smoke PASSED: ECS fade updates, opaque overlaps, additive blending, transformed owners, scroll edges, viewport offsets, margins and writable view sizes; GPU samples captured"
            );
            exit.write(AppExit::Success);
            next = true;
        }
        _ => {}
    }
    if next {
        state.step += 1;
        state.since = state.frame;
    }
}

fn capture_hard_margins(
    state: &State,
    gui: &GuiRuntime,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    commands: &mut Commands,
    clear: Color,
    suffix: &str,
) {
    let mut csv = String::from("panel,x,y,r,g,b\n");
    for (index, &owner) in state.owners.iter().enumerate() {
        let object = gui.store().object(owner).unwrap();
        // Independent expectations from the assigned margins and native
        // updateMask/setSize, rather than the projected CalculatedClip.
        let pane = object.control().component().unwrap().scroll_pane.as_ref();
        let min = if pane.is_some() {
            Vec2::new(12., 8.)
        } else {
            Vec2::ZERO
        };
        let size = if let Some(pane) = pane {
            let v = pane
                .vt_bar
                .and_then(|id| gui.store().object(id))
                .map_or(0., |b| b.width());
            let h = pane
                .hz_bar
                .and_then(|id| gui.store().object(id))
                .map_or(0., |b| b.height());
            Vec2::new(object.width() - v - 42., object.height() - h - 32.)
        } else {
            Vec2::new(object.width() - 30., object.height() - 24.)
        };
        let (node, transform) = nodes.get(gui.entity_of(owner).unwrap()).unwrap();
        for (p, inside) in [
            (min + Vec2::splat(3.), true),
            (min + size - Vec2::splat(3.), true),
            (min + Vec2::new(size.x * 0.5, size.y * 0.5), true),
            (min + Vec2::new(-3., size.y * 0.4), false),
            (min + Vec2::new(size.x + 3., size.y * 0.4), false),
            (min + Vec2::new(size.x * 0.4, -3.), false),
            (min + Vec2::new(size.x * 0.4, size.y + 3.), false),
        ] {
            let screen = transform
                .transform_point2(p / node.inverse_scale_factor() - node.size() * 0.5)
                + state.offset;
            let color = if inside {
                if index == 4 {
                    let src = SRC.to_linear();
                    let bg = BG.to_linear();
                    Color::linear_rgb(
                        (src.red + bg.red).min(1.),
                        (src.green + bg.green).min(1.),
                        (src.blue + bg.blue).min(1.),
                    )
                } else {
                    SRC
                }
            } else if pane.is_some() {
                BG
            } else {
                clear
            };
            let c = color.to_srgba();
            writeln!(
                csv,
                "{index},{},{},{},{},{}",
                screen.x,
                screen.y,
                (c.red * 255.).round() as u8,
                (c.green * 255.).round() as u8,
                (c.blue * 255.).round() as u8
            )
            .unwrap();
        }
    }
    std::fs::write(format!("target/softclip-samples{suffix}.csv"), csv).unwrap();
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(format!(
            "target/fairygui-demo-softclip{suffix}.png"
        )));
}

fn capture(
    state: &State,
    gui: &GuiRuntime,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    commands: &mut Commands,
    suffix: &str,
    disabled: bool,
) {
    let mut csv = String::from("panel,x,y,r,g,b\n");
    for (index, &owner) in state.owners.iter().enumerate() {
        let object = gui.store().object(owner).unwrap();
        let component = object.control().component().unwrap();
        let (size, edges) = if let Some(pane) = &component.scroll_pane {
            let view = pane.view_size();
            let pos = pane.display_pos();
            let max = pane.max_scroll_pos();
            (
                Vec2::new(view.width, view.height),
                [
                    if pane.softness_on_top_left && pos.x > 0. {
                        300.
                    } else {
                        0.
                    },
                    if pane.softness_on_top_left && pos.y > 0. {
                        200.
                    } else {
                        0.
                    },
                    if pos.x < max.x { 300. } else { 0. },
                    if pos.y < max.y { 200. } else { 0. },
                ],
            )
        } else {
            (
                Vec2::new(object.width(), object.height()),
                [24., 20., 24., 20.],
            )
        };
        let (node, transform) = nodes.get(gui.entity_of(owner).unwrap()).unwrap();
        let inverse = transform.try_inverse().unwrap();
        for point in [
            Vec2::new(2., size.y * 0.43),
            Vec2::new(7., size.y * 0.43),
            Vec2::new(12., size.y * 0.43),
            Vec2::new(size.x - 7., size.y * 0.43),
            Vec2::new(size.x - 2., size.y * 0.43),
            Vec2::new(6., 8.),
            Vec2::new(size.x * 0.43, size.y * 0.43),
        ] {
            let screen = transform
                .transform_point2(point / node.inverse_scale_factor() - node.size() * 0.5)
                + state.offset;
            // Account for the exact raster sample, including fractional DPI.
            let pixel = screen.floor() + Vec2::splat(0.5);
            let p = (inverse.transform_point2(pixel - state.offset) + node.size() * 0.5)
                * node.inverse_scale_factor();
            let axis = |p: f32, size: f32, leading: f32, trailing: f32| {
                let (distance, width) = if p <= size * 0.5 {
                    (p, leading)
                } else {
                    (size - p, trailing)
                };
                if width == 0. {
                    1.
                } else {
                    (distance / width).clamp(0., 1.)
                }
            };
            let fade = if disabled {
                1.
            } else {
                axis(p.x, size.x, edges[0], edges[2]).min(axis(p.y, size.y, edges[1], edges[3]))
            };
            let alpha = if index == 1 {
                1. - (1. - fade).powi(2)
            } else {
                fade
            };
            let bg = BG.to_linear();
            let src = SRC.to_linear();
            let channel = |src: f32, bg: f32| {
                if index == 4 {
                    (src * alpha + bg).min(1.)
                } else {
                    src * alpha + bg * (1. - alpha)
                }
            };
            let color = Color::linear_rgb(
                channel(src.red, bg.red),
                channel(src.green, bg.green),
                channel(src.blue, bg.blue),
            )
            .to_srgba();
            writeln!(
                csv,
                "{index},{},{},{},{},{}",
                screen.x,
                screen.y,
                (color.red * 255.).round() as u8,
                (color.green * 255.).round() as u8,
                (color.blue * 255.).round() as u8
            )
            .unwrap();
        }
    }
    std::fs::write(format!("target/softclip-samples{suffix}.csv"), csv).unwrap();
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(format!(
            "target/fairygui-demo-softclip{suffix}.png"
        )));
}

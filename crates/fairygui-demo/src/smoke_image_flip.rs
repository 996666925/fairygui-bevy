//! Native image flip scale/position, public ECS edits and asymmetric GPU probes.
use super::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::fmt::Write;

const COLORS: [[u8; 4]; 4] = [
    [230, 50, 30, 255],
    [30, 210, 60, 255],
    [40, 70, 230, 255],
    [230, 210, 40, 255],
];
#[derive(Resource, Default)]
struct State {
    frame: u32,
    since: u32,
    phase: u8,
    stage: u8,
    owner: Option<Entity>,
    view: Option<Entity>,
    child: Option<Entity>,
}
pub(super) fn install(app: &mut App) {
    app.insert_resource(ClearColor(Color::BLACK))
        .init_resource::<State>()
        .add_systems(PreUpdate, drive);
}
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn drive(
    mut state: ResMut<State>,
    demo: Res<Demo>,
    assets: Res<AssetServer>,
    mut ui: ParamSet<(
        GuiQuery,
        Query<&mut GuiImageFlip>,
        Query<&mut GuiTransform>,
        Query<&mut GuiDisplay>,
        Query<&mut GuiImageFill>,
    )>,
    mut direct: GuiMut,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    images: Query<&ImageNode>,
    mut textures: ResMut<Assets<Image>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    entities: Query<()>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    assert!(
        state.frame < 1600,
        "image flip smoke timed out: phase {} stage {}",
        state.phase,
        state.stage
    );
    let elapsed = state.frame - state.since;
    match state.phase {
        0 if state.frame >= 30 => {
            let Some(menu) = demo.menu_owner.and_then(|owner| ui.p0().view(owner)) else {
                return;
            };
            ui.p3().get_mut(menu).unwrap().visible = false;
            state.owner = Some(
                commands
                    .spawn((
                        FairyGuiRoot::new(assets.load("ui/Basics.fui"), "pic")
                            .with_size(Vec2::new(240., 160.))
                            .with_position(Vec2::new(440., 240.)),
                        GuiImageFlip(FlipType::Horizontal),
                    ))
                    .id(),
            );
            state.phase = 1;
            state.since = state.frame;
        }
        1 if elapsed >= 16 && ui.p0().view(state.owner.unwrap()).is_some() => {
            let view = ui.p0().view(state.owner.unwrap()).unwrap();
            // Preserve the exported atlas descriptor. IMAGE dirtiness can
            // republish that rect when fill changes; a 2x2 replacement texture
            // would then sample coordinates outside the synthetic texture.
            let mut descriptor = images.get(view).unwrap().clone();
            let Some(original) = textures.get(&descriptor.image) else {
                return;
            };
            let size = original.size();
            let rect = descriptor
                .rect
                .unwrap_or(Rect::from_corners(Vec2::ZERO, size.as_vec2()));
            let mut rgba = Vec::with_capacity((size.x * size.y * 4) as usize);
            for y in 0..size.y {
                for x in 0..size.x {
                    let right = f32::from(x as u16) >= rect.center().x;
                    let bottom = f32::from(y as u16) >= rect.center().y;
                    rgba.extend(COLORS[usize::from(right) + usize::from(bottom) * 2]);
                }
            }
            let mut texture = Image::new_fill(
                Extent3d {
                    width: size.x,
                    height: size.y,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                &rgba,
                TextureFormat::Rgba8UnormSrgb,
                default(),
            );
            texture.sampler = bevy::image::ImageSampler::nearest();
            descriptor.image = textures.add(texture);
            commands.entity(view).insert(descriptor);
            state.child = Some(commands.spawn((Node::default(), ChildOf(view))).id());
            state.view = Some(view);
            state.phase = 2;
            state.since = state.frame;
        }
        2 if elapsed >= 16 => {
            let view = state.view.unwrap();
            let gui = ui.p0();
            assert_eq!(gui.view(state.owner.unwrap()), Some(view));
            assert!(entities.contains(state.child.unwrap()));
            let image = images.get(view).unwrap();
            assert!(!image.flip_x && !image.flip_y);
            let expected_flip = match state.stage {
                0 | 4..=11 => FlipType::Horizontal,
                1 => FlipType::Vertical,
                2 => FlipType::Both,
                3 => FlipType::None,
                _ => unreachable!(),
            };
            assert_eq!(gui.image_flip(view), Some(expected_flip));
            let transform = gui.transform(view).unwrap();
            let scale = if state.stage == 4 || state.stage == 5 {
                Vec2::new(1.5, 0.75)
            } else {
                let (x, y) = expected_flip.scale();
                Vec2::new(x, y)
            };
            assert_eq!(transform.scale, scale);
            let (node, projected) = nodes.get(view).unwrap();
            let window = windows.single().unwrap();
            let anchor = if transform.pivot_as_anchor {
                transform.pivot * transform.size
            } else {
                Vec2::ZERO
            };
            let mut csv = String::from("x,y,r,g,b\n");
            for (index, uv) in [
                Vec2::new(0.25, 0.25),
                Vec2::new(0.75, 0.25),
                Vec2::new(0.25, 0.75),
                Vec2::new(0.75, 0.75),
            ]
            .into_iter()
            .enumerate()
            {
                let local = uv * transform.size;
                let physical = projected
                    .transform_point2(node.border_box().min + local / node.inverse_scale_factor());
                let native = gui.local_to_global(view, local - anchor).unwrap();
                assert!(
                    // Bevy rounds absolute layout positions to physical pixels.
                    physical.abs_diff_eq(native * window.scale_factor(), 0.6),
                    "stage {} native {native:?}, Bevy {physical:?}",
                    state.stage
                );
                let raw = COLORS[index];
                let mut color = Color::srgb_u8(raw[0], raw[1], raw[2]);
                if state.stage == 10 {
                    let native = fairygui_core::color_matrix::ColorMatrix::LAYA_GRAY.apply(
                        fairygui_core::color::Color::from_rgba8(raw[0], raw[1], raw[2], 255),
                    );
                    color = Color::srgb(native.r, native.g, native.b);
                }
                if state.stage == 11 {
                    let c = color.to_linear();
                    color = Color::linear_rgb(c.red * 0.5, c.green * 0.5, c.blue * 0.5);
                }
                if state.stage >= 9 && uv.x > 0.5 {
                    color = Color::BLACK;
                }
                let c = color.to_srgba();
                let [r, g, b] =
                    [c.red, c.green, c.blue].map(|v| (v.clamp(0., 1.) * 255.).round() as u8);
                writeln!(csv, "{},{},{r},{g},{b}", physical.x, physical.y).unwrap();
            }
            std::fs::write(format!("target/image-flip-{}.csv", state.stage), csv).unwrap();
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(format!(
                    "target/fairygui-demo-image-flip-{}.png",
                    state.stage
                )));
            state.phase = 3;
            state.since = state.frame;
        }
        3 if elapsed >= 8 => {
            let view = state.view.unwrap();
            state.stage += 1;
            match state.stage {
                1 => ui.p1().get_mut(view).unwrap().0 = FlipType::Vertical,
                2 => ui.p1().get_mut(view).unwrap().0 = FlipType::Both,
                3 => ui.p1().get_mut(view).unwrap().0 = FlipType::None,
                4 => {
                    ui.p1().get_mut(view).unwrap().0 = FlipType::Horizontal;
                    ui.p2().get_mut(view).unwrap().scale = Vec2::new(1.5, 0.75);
                }
                5 => direct
                    .get_mut(view)
                    .unwrap()
                    .set_image_flip(FlipType::Horizontal),
                6 => {
                    let mut q = ui.p2();
                    let mut t = q.get_mut(view).unwrap();
                    t.size = Vec2::new(300., 180.);
                    t.scale = Vec2::new(-1., 1.);
                }
                7 => ui.p2().get_mut(view).unwrap().position += Vec2::new(10., 10.),
                8 => {
                    let mut q = ui.p2();
                    let mut t = q.get_mut(view).unwrap();
                    t.pivot = Vec2::splat(0.5);
                    t.pivot_as_anchor = true;
                    t.rotation = 30.;
                }
                9 => {
                    *ui.p4().get_mut(view).unwrap() = GuiImageFill {
                        method: FillMethod::Horizontal,
                        amount: 0.5,
                        ..default()
                    }
                }
                10 => ui.p3().get_mut(view).unwrap().grayed = true,
                11 => {
                    let mut q = ui.p3();
                    let mut d = q.get_mut(view).unwrap();
                    d.grayed = false;
                    d.alpha = 0.5;
                }
                12 => {
                    commands.entity(state.owner.unwrap()).despawn();
                    state.phase = 4;
                    state.since = state.frame;
                    return;
                }
                _ => unreachable!(),
            }
            state.phase = 2;
            state.since = state.frame;
        }
        4 if elapsed >= 12 => {
            assert!(!entities.contains(state.view.unwrap()));
            assert!(!entities.contains(state.child.unwrap()));
            eprintln!(
                "image flip smoke PASSED: four native flip modes, explicit scale precedence, same-value no-op, cached resize/XY compensation, anchor/rotation, mirrored fill mask, gray/restore/alpha, retained view/children and disposal; 48 GPU probes captured"
            );
            exit.write(AppExit::Success);
            state.phase = 5;
        }
        _ => {}
    }
}

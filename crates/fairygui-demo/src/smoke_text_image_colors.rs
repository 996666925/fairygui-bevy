use super::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use fairygui_core::{
    color::Color as GuiColor,
    color_matrix::{ColorFilterParams, ColorMatrix},
};

const SWATCHES: [[u8; 4]; 8] = [
    [255, 0, 0, 255],
    [0, 255, 0, 255],
    [0, 0, 255, 255],
    [128, 128, 128, 255],
    [60, 150, 210, 255],
    [255, 100, 30, 128],
    [255, 0, 255, 0],
    [255, 255, 255, 255],
];

#[derive(Default)]
pub(super) struct State {
    stage: u8,
    since: u32,
    roots: Vec<usize>,
    owners: Vec<GObjectId>,
    samples: Vec<Entity>,
    parts: Vec<Entity>,
    decorations: Vec<Entity>,
    pattern: Option<Handle<Image>>,
}
impl State {
    pub fn complete(&self) -> bool {
        self.stage == 7
    }
}

fn record_samples(
    state: &State,
    runtime: &GuiRuntime,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    suffix: &str,
) {
    let mut lines = vec!["panel,band,x,y,r,g,b,a".to_owned()];
    for (i, &view) in state.samples.iter().enumerate() {
        let owner = state.owners[i];
        let object = runtime.store().object(owner).unwrap();
        let matrix = if object.grayed() {
            ColorMatrix::LAYA_GRAY
        } else {
            object
                .color_filter()
                .map(|p| p.to_matrix())
                .unwrap_or_default()
        };
        let tint = if i / 3 == 1 {
            if object.grayed() {
                GuiColor::from_rgba8(170, 170, 170, 255)
            } else {
                GuiColor::from_rgba8(50, 100, 150, 255)
            }
        } else {
            GuiColor::WHITE
        };
        let (node, transform) = nodes.get(view).unwrap();
        assert!(node.size().min_element() > 0.0);
        for (band, c) in SWATCHES.iter().enumerate() {
            let p = transform.transform_point2(Vec2::new(
                ((band as f32 + 0.5) / 8.0 - 0.5) * node.size().x,
                0.0,
            ));
            let c = GuiColor::from_rgba8(c[0], c[1], c[2], c[3]);
            let c = matrix.apply(GuiColor::new(c.r * tint.r, c.g * tint.g, c.b * tint.b, c.a));
            lines.push(format!(
                "{i},{band},{},{},{},{},{},{}",
                p.x,
                p.y,
                c.r,
                c.g,
                c.b,
                c.a * runtime.store().effective_alpha(owner)
            ));
        }
    }
    std::fs::write(
        format!("target/text-image-color-samples{suffix}.csv"),
        lines.join("\n") + "\n",
    )
    .unwrap();
}

#[allow(clippy::too_many_arguments)]
pub(super) fn drive(
    state: &mut State,
    frames: u32,
    runtime: &mut GuiRuntime,
    assets: &mut Assets<FuiAsset>,
    image_assets: &mut Assets<Image>,
    glyphs: &Query<&fairygui_bevy::runtime::BitmapGlyphs>,
    inline: &Query<&fairygui_bevy::runtime::InlineImageParts>,
    images: &Query<&ImageNode>,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    layout: &Query<&Node>,
    commands: &mut Commands,
    original: GObjectId,
) {
    match state.stage {
        0 => {
            let packages = [
                fairygui_core::FuiPackage::parse(
                    "Basics",
                    include_bytes!("../../fairygui-core/tests/fixtures/BitmapText.fui"),
                )
                .unwrap(),
                fairygui_core::FuiPackage::parse(
                    "Basics",
                    include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui"),
                )
                .unwrap(),
            ];
            let handles: Vec<_> = packages
                .iter()
                .map(|package| {
                    assets.add(FuiAsset {
                        package: package.clone(),
                        res_key: "ui/Basics".into(),
                    })
                })
                .collect();
            let object = runtime.store().object(original).unwrap();
            let origin = Vec2::new(object.x() + 50.0, object.y() + 135.0);
            runtime.store_mut().set_visible(original, false);
            state.decorations.push(
                commands
                    .spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(origin.x),
                            top: Val::Px(origin.y),
                            width: Val::Px(1000.0),
                            height: Val::Px(440.0),
                            ..default()
                        },
                        BackgroundColor(Color::WHITE),
                        GlobalZIndex(600),
                    ))
                    .id(),
            );
            for i in 0..9 {
                let row = i / 3;
                let col = i % 3;
                let package_index = usize::from(row == 2);
                let index = runtime
                    .create_root(
                        handles[package_index].clone(),
                        &packages[package_index],
                        "Demo_Text",
                    )
                    .unwrap();
                let root = runtime.root_object(index).unwrap();
                let name = ["n0", "n2", "n12"][row];
                let owner = runtime.store().child_by_name(root, name).unwrap();
                let children = runtime.store().object(root).unwrap().children().to_vec();
                for child in children {
                    runtime.store_mut().set_visible(child, child == owner);
                }
                let xy = origin + Vec2::new(25.0 + col as f32 * 320.0, 12.0 + row as f32 * 140.0);
                runtime.store_mut().set_xy(root, xy.x, xy.y + 25.0);
                runtime.store_mut().set_xy(owner, 0.0, 0.0);
                runtime.store_mut().set_size(owner, 280.0, 100.0);
                runtime.store_mut().set_control_color(
                    owner,
                    GuiColor::from_rgba8(50, 100, 150, 255),
                    None,
                );
                runtime.store_mut().set_object_text(
                    owner,
                    [
                        "12345",
                        "UI#",
                        "[img]ui://9leh0eyfrpmb6[/img][img]ui://9leh0eyfrpmb6[/img]",
                    ][row],
                );
                // Near identity exercises the encoded tint path on the first column.
                runtime.store_mut().set_color_filter(
                    owner,
                    Some(if col == 0 {
                        ColorFilterParams {
                            brightness: 0.0001,
                            ..default()
                        }
                    } else {
                        ColorFilterParams {
                            hue: 0.5,
                            saturation: 0.3,
                            ..default()
                        }
                    }),
                );
                runtime.store_mut().set_grayed(owner, col == 2);
                if col == 2 {
                    runtime.store_mut().set_alpha(root, 0.8);
                    runtime.store_mut().set_alpha(owner, 0.75);
                }
                state.decorations.push(
                    commands
                        .spawn((
                            Text::new(format!(
                                "{} / {}",
                                ["Color font", "Tinted font", "Inline image"][row],
                                ["near identity", "hue +0.5", "gray, a=0.6"][col]
                            )),
                            TextFont {
                                font_size: bevy::text::FontSize::Px(16.0),
                                ..default()
                            },
                            TextColor(Color::BLACK),
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(xy.x),
                                top: Val::Px(xy.y),
                                ..default()
                            },
                            GlobalZIndex(603),
                        ))
                        .id(),
                );
                state.roots.push(index);
                state.owners.push(owner);
            }
            let pixels: Vec<_> = (0..32)
                .flat_map(|_| (0..64).flat_map(|x| SWATCHES[x / 8]))
                .collect();
            state.pattern = Some(image_assets.add(Image::new_fill(
                Extent3d {
                    width: 64,
                    height: 32,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                &pixels,
                TextureFormat::Rgba8UnormSrgb,
                default(),
            )));
            state.stage = 1;
            state.since = frames;
        }
        1 if frames - state.since >= 16 => {
            for (i, &owner) in state.owners.iter().enumerate() {
                let view = runtime.entity_of(owner).unwrap();
                let parts: Vec<_> = if i / 3 == 2 {
                    inline.get(view).unwrap().iter().collect()
                } else {
                    glyphs.get(view).unwrap().0.clone()
                };
                assert!(parts.len() >= 2);
                let sample = if i / 3 == 2 { parts[1] } else { parts[0] };
                let mut image = images.get(sample).unwrap().clone();
                image.image = state.pattern.as_ref().unwrap().clone();
                image.rect = Some(Rect::new(0.0, 0.0, 64.0, 32.0));
                commands.entity(sample).insert(image);
                if i / 3 != 2 {
                    let mut node = layout.get(sample).unwrap().clone();
                    node.left = Val::Px(0.0);
                    node.top = Val::Px(55.0);
                    node.width = Val::Px(240.0);
                    node.height = Val::Px(32.0);
                    commands.entity(sample).insert(node);
                }
                let root_view = runtime
                    .entity_of(runtime.root_object(state.roots[i]).unwrap())
                    .unwrap();
                commands.entity(root_view).insert(GlobalZIndex(602));
                state.samples.push(sample);
                state.parts.extend(parts);
            }
            state.stage = 2;
            state.since = frames;
        }
        2 if frames - state.since >= 16 => {
            record_samples(state, runtime, nodes, "");
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-text-image-colors.png"));
            state.stage = 3;
            state.since = frames;
        }
        3 if frames - state.since >= 10 => {
            for &owner in &state.owners {
                runtime.store_mut().set_color_filter(
                    owner,
                    Some(ColorFilterParams {
                        brightness: 0.12,
                        contrast: -0.3,
                        hue: -0.4,
                        saturation: 0.7,
                    }),
                );
                runtime.store_mut().set_grayed(owner, false);
            }
            for (i, &view) in state.decorations.iter().skip(1).enumerate() {
                commands.entity(view).insert(Text::new(format!(
                    "{} / combined{}",
                    ["Color font", "Tinted font", "Inline image"][i / 3],
                    if i % 3 == 2 { ", a=0.6" } else { "" },
                )));
            }
            state.stage = 4;
            state.since = frames;
        }
        4 if frames - state.since >= 16 => {
            assert!(state.parts.iter().all(|&e| images.get(e).is_ok()));
            record_samples(state, runtime, nodes, "-updated");
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-text-image-colors-updated.png",
                ));
            state.stage = 5;
            state.since = frames;
        }
        5 if frames - state.since >= 10 => {
            for &index in &state.roots {
                runtime.remove_root(commands, index);
            }
            for &view in &state.decorations {
                commands.entity(view).despawn();
            }
            runtime.store_mut().set_visible(original, true);
            state.pattern = None;
            state.stage = 6;
            state.since = frames;
        }
        6 if frames - state.since >= 10 => {
            assert!(state.parts.iter().all(|&e| images.get(e).is_err()));
            state.stage = 7;
            println!(
                "demo smoke: native image/atlas bitmap fonts and sibling inline images, local sampled filters, gray/restore, retained geometry, alpha and disposal verified; 144 GPU samples captured"
            );
        }
        _ => {}
    }
}

use super::*;
use fairygui_core::{
    FuiPackage,
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
    views: Vec<Entity>,
    old: Vec<Entity>,
    decorations: Vec<Entity>,
}
impl State {
    pub fn complete(&self) -> bool {
        self.stage == 7
    }
}
fn content(bitmap: bool, column: usize, updated: bool) -> String {
    let attrs = if updated {
        ["height='40'", "width='140'", "width='120' height='24'"][column]
    } else {
        ["", "width='96'", "width='80' height='16'"][column]
    };
    format!(
        "{}<br/><a href='event:external'><img src='images/inline-swatch.png' {attrs}/></a><br/>{}",
        if bitmap { "UI#" } else { "External PNG" },
        if bitmap {
            "使用字体"
        } else {
            "Loaded from assets/images"
        }
    )
}
fn image_of(
    owner: Entity,
    inline: &Query<&fairygui_bevy::runtime::InlineImageParts>,
    glyphs: &Query<&fairygui_bevy::runtime::BitmapGlyphs>,
    children: &Query<&Children>,
    images: &Query<&ImageNode>,
) -> Entity {
    if let Ok(parts) = inline.get(owner) {
        parts.iter().next().unwrap()
    } else {
        let glyphs = &glyphs.get(owner).unwrap().0;
        children
            .get(owner)
            .unwrap()
            .iter()
            .find(|e| !glyphs.contains(e) && images.get(*e).is_ok())
            .unwrap()
    }
}
fn record(
    state: &State,
    runtime: &GuiRuntime,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    suffix: &str,
) {
    let mut csv = vec!["panel,band,x,y,r,g,b,a".to_owned()];
    for (i, &view) in state.views.iter().enumerate() {
        let id = state.owners[i];
        let object = runtime.store().object(id).unwrap();
        let matrix = if object.grayed() {
            ColorMatrix::LAYA_GRAY
        } else {
            object
                .color_filter()
                .map(|p| p.to_matrix())
                .unwrap_or_default()
        };
        let (node, transform) = nodes.get(view).unwrap();
        assert!(node.size().min_element() > 0.0);
        for (band, c) in SWATCHES.iter().enumerate() {
            let xy = transform.transform_point2(Vec2::new(
                ((band as f32 + 0.5) / 8.0 - 0.5) * node.size().x,
                0.0,
            ));
            let color = matrix.apply(GuiColor::from_rgba8(c[0], c[1], c[2], c[3]));
            csv.push(format!(
                "{i},{band},{},{},{},{},{},{}",
                xy.x,
                xy.y,
                color.r,
                color.g,
                color.b,
                color.a * runtime.store().effective_alpha(id)
            ));
        }
    }
    std::fs::write(
        format!("target/external-inline-samples{suffix}.csv"),
        csv.join("\n") + "\n",
    )
    .unwrap();
}
#[allow(clippy::too_many_arguments)]
pub(super) fn drive(
    state: &mut State,
    frames: u32,
    runtime: &mut GuiRuntime,
    assets: &mut Assets<FuiAsset>,
    image_assets: &Assets<Image>,
    inline: &Query<&fairygui_bevy::runtime::InlineImageParts>,
    glyphs: &Query<&fairygui_bevy::runtime::BitmapGlyphs>,
    children: &Query<&Children>,
    images: &Query<&ImageNode>,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    layout: &Query<&Node>,
    commands: &mut Commands,
    original: GObjectId,
) {
    match state.stage {
        0 => {
            let packages = [
                FuiPackage::parse(
                    "Basics",
                    include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui"),
                )
                .unwrap(),
                FuiPackage::parse(
                    "Basics",
                    include_bytes!("../../fairygui-core/tests/fixtures/BitmapText.fui"),
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
            let page = runtime.store().object(original).unwrap();
            let origin = Vec2::new(page.x() + 50.0, page.y() + 135.0);
            runtime.store_mut().set_visible(original, false);
            state.decorations.push(
                commands
                    .spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(origin.x),
                            top: Val::Px(origin.y),
                            width: Val::Px(1000.0),
                            height: Val::Px(430.0),
                            ..default()
                        },
                        BackgroundColor(Color::WHITE),
                        GlobalZIndex(600),
                    ))
                    .id(),
            );
            for i in 0..6 {
                let row = i / 3;
                let column = i % 3;
                let index = runtime
                    .create_root(handles[row].clone(), &packages[row], "Demo_Text")
                    .unwrap();
                let root = runtime.root_object(index).unwrap();
                let owner = runtime
                    .store()
                    .child_by_name(root, if row == 0 { "n12" } else { "n8" })
                    .unwrap();
                let all = runtime.store().object(root).unwrap().children().to_vec();
                for child in all {
                    runtime.store_mut().set_visible(child, child == owner);
                }
                let xy =
                    origin + Vec2::new(25.0 + column as f32 * 320.0, 15.0 + row as f32 * 205.0);
                runtime.store_mut().set_xy(root, xy.x, xy.y + 32.0);
                runtime.store_mut().set_xy(owner, 0.0, 0.0);
                runtime.store_mut().set_size(owner, 280.0, 160.0);
                runtime.store_mut().set_object_html_enabled(owner, true);
                runtime.store_mut().set_object_ubb_enabled(owner, false);
                runtime.store_mut().set_object_font_size(owner, 24);
                runtime.store_mut().set_control_color(
                    owner,
                    GuiColor::from_rgba8(50, 100, 150, 255),
                    None,
                );
                runtime
                    .store_mut()
                    .set_object_text(owner, &content(row == 1, column, false));
                if column > 0 {
                    runtime.store_mut().set_color_filter(
                        owner,
                        Some(ColorFilterParams {
                            hue: 0.5,
                            saturation: 0.3,
                            ..default()
                        }),
                    );
                }
                if column == 2 {
                    runtime.store_mut().set_grayed(owner, true);
                    runtime.store_mut().set_alpha(root, 0.8);
                    runtime.store_mut().set_alpha(owner, 0.75);
                }
                state.decorations.push(
                    commands
                        .spawn((
                            Text::new(format!(
                                "{} / {}",
                                if row == 0 { "Paragraph" } else { "Bitmap" },
                                ["Auto size", "Width 96 / hue", "80x16 / gray a=0.6"][column]
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
                            GlobalZIndex(602),
                        ))
                        .id(),
                );
                state.roots.push(index);
                state.owners.push(owner);
            }
            state.stage = 1;
            state.since = frames;
        }
        1 if frames - state.since >= 16 => {
            if state.owners.iter().any(|&owner| {
                let view = image_of(
                    runtime.entity_of(owner).unwrap(),
                    inline,
                    glyphs,
                    children,
                    images,
                );
                image_assets.get(&images.get(view).unwrap().image).is_none()
            }) {
                return;
            }
            for (i, &owner) in state.owners.iter().enumerate() {
                let view = image_of(
                    runtime.entity_of(owner).unwrap(),
                    inline,
                    glyphs,
                    children,
                    images,
                );
                let image = images.get(view).unwrap();
                assert_eq!(
                    image_assets.get(&image.image).unwrap().size(),
                    UVec2::new(64, 32)
                );
                let node = layout.get(view).unwrap();
                let expected = [
                    Vec2::new(64.0, 32.0),
                    Vec2::new(96.0, 32.0),
                    Vec2::new(80.0, 16.0),
                ][i % 3];
                assert_eq!(
                    (node.width, node.height),
                    (Val::Px(expected.x), Val::Px(expected.y))
                );
                let root = runtime.root_object(state.roots[i]).unwrap();
                commands
                    .entity(runtime.entity_of(root).unwrap())
                    .insert(GlobalZIndex(601));
                state.views.push(view);
            }
            state.stage = 2;
            state.since = frames;
        }
        2 if frames - state.since >= 12 => {
            record(state, runtime, nodes, "");
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-external-inline.png"));
            state.stage = 3;
            state.since = frames;
        }
        3 if frames - state.since >= 10 => {
            state.old = state.views.clone();
            for (i, &owner) in state.owners.iter().enumerate() {
                runtime
                    .store_mut()
                    .set_object_text(owner, &content(i / 3 == 1, i % 3, true));
                runtime.store_mut().set_grayed(owner, false);
                runtime.store_mut().set_color_filter(
                    owner,
                    Some(ColorFilterParams {
                        hue: -0.5,
                        ..default()
                    }),
                );
                commands
                    .entity(state.decorations[i + 1])
                    .insert(Text::new(format!(
                        "{} / {}",
                        if i / 3 == 0 { "Paragraph" } else { "Bitmap" },
                        ["Height 40", "Width 140", "120x24 / a=0.6"][i % 3]
                    )));
            }
            state.stage = 4;
            state.since = frames;
        }
        4 if frames - state.since >= 16 => {
            assert!(state.old.iter().all(|&e| images.get(e).is_err()));
            state.views.clear();
            for (i, &owner) in state.owners.iter().enumerate() {
                let view = image_of(
                    runtime.entity_of(owner).unwrap(),
                    inline,
                    glyphs,
                    children,
                    images,
                );
                let node = layout.get(view).unwrap();
                let expected = [
                    Vec2::new(64.0, 40.0),
                    Vec2::new(140.0, 32.0),
                    Vec2::new(120.0, 24.0),
                ][i % 3];
                assert_eq!(
                    (node.width, node.height),
                    (Val::Px(expected.x), Val::Px(expected.y))
                );
                state.views.push(view);
            }
            record(state, runtime, nodes, "-updated");
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-external-inline-updated.png",
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
            state.stage = 6;
            state.since = frames;
        }
        6 if frames - state.since >= 10 => {
            assert!(state.views.iter().all(|&e| images.get(e).is_err()));
            state.stage = 7;
            println!(
                "demo smoke: external PNG decoded through AssetServer, natural/one-axis/fixed dimensions in paragraph and bitmap text, HTML edits, sampled filters/gray/alpha and disposal verified; 96 GPU samples captured"
            );
        }
        _ => {}
    }
}

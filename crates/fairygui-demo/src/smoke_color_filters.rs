use super::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use fairygui_core::{
    color::Color as GuiColor,
    color_matrix::{ColorFilterParams, ColorMatrix},
    package::{ObjectType, PackageItemType},
    widget::GraphData,
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
    container: Option<GObjectId>,
    roots: Vec<usize>,
    sources: Vec<GObjectId>,
    views: Vec<Entity>,
    pattern: Option<Handle<Image>>,
}
impl State {
    pub fn complete(&self) -> bool {
        self.stage == 6
    }
}

fn filter(i: usize) -> Option<ColorFilterParams> {
    match i {
        0 => None,
        1 => Some(ColorFilterParams {
            saturation: -1.0,
            ..default()
        }),
        2 => Some(ColorFilterParams {
            hue: 0.5,
            ..default()
        }),
        3 | 4 => Some(ColorFilterParams {
            brightness: 0.12,
            contrast: 0.3,
            saturation: 0.5,
            hue: -0.4,
        }),
        _ => Some(ColorFilterParams {
            brightness: 0.25,
            ..default()
        }),
    }
}

// Physical positions and encoded straight colors, including transparent texels.
fn record_samples(
    state: &State,
    runtime: &GuiRuntime,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    suffix: &str,
) {
    let mut lines = vec!["panel,band,x,y,r,g,b,a".to_string()];
    for (i, &view) in state.views.iter().enumerate() {
        let id = state.sources[i * 2];
        let object = runtime.store().object(id).unwrap();
        let (node, transform) = nodes.get(view).unwrap();
        let matrix = if object.grayed() {
            ColorMatrix::LAYA_GRAY
        } else {
            object
                .color_filter()
                .map(|p| p.to_matrix())
                .unwrap_or_default()
        };
        for (band, c) in SWATCHES.into_iter().enumerate() {
            let p = transform.transform_point2(Vec2::new(
                ((band as f32 + 0.5) / 8.0 - 0.5) * node.size().x,
                0.0,
            ));
            let c = matrix.apply(GuiColor::from_rgba8(c[0], c[1], c[2], c[3]));
            lines.push(format!(
                "{i},{band},{},{},{},{},{},{}",
                p.x,
                p.y,
                c.r,
                c.g,
                c.b,
                c.a * runtime.store().effective_alpha(id)
            ));
        }
    }
    std::fs::write(
        format!("target/color-filter-samples{suffix}.csv"),
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
    images: &Query<&ImageNode>,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    commands: &mut Commands,
    root: GObjectId,
) {
    match state.stage {
        0 => {
            let package = fairygui_core::FuiPackage::parse(
                "Basics",
                include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui"),
            )
            .unwrap();
            let image = package
                .items()
                .iter()
                .enumerate()
                .find(|(i, item)| {
                    item.item_type == PackageItemType::Image
                        && item.scale9_grid.is_none()
                        && package.sprite_for(*i).is_some_and(|s| !s.rotated)
                })
                .unwrap()
                .1;
            let movie = package
                .items()
                .iter()
                .find(|item| item.item_type == PackageItemType::MovieClip)
                .unwrap();
            let handle = assets.add(FuiAsset {
                package: package.clone(),
                res_key: "ui/Basics".into(),
            });
            let page = runtime.store().object(root).unwrap();
            let origin = Vec2::new(page.x(), page.y()) + Vec2::new(50.0, 150.0);
            let container = runtime.store_mut().create(ObjectType::Component);
            runtime.store_mut().set_parent(container, Some(root));
            runtime.store_mut().set_xy(container, 50.0, 150.0);
            runtime.store_mut().set_size(container, 1000.0, 440.0);
            let bg = runtime.store_mut().create(ObjectType::Graph);
            runtime.store_mut().set_parent(bg, Some(container));
            runtime.store_mut().set_size(bg, 1000.0, 440.0);
            runtime.store_mut().set_graph(
                bg,
                GraphData {
                    graph_type: 1,
                    fill_color: GuiColor::WHITE,
                    ..default()
                },
            );
            for i in 0..6 {
                let xy = Vec2::new(30.0 + (i % 3) as f32 * 320.0, 25.0 + (i / 3) as f32 * 210.0);
                let index = runtime
                    .create_root(handle.clone(), &package, &image.name)
                    .unwrap();
                let id = runtime.root_object(index).unwrap();
                runtime
                    .store_mut()
                    .set_xy(id, origin.x + xy.x, origin.y + xy.y + 20.0);
                runtime.store_mut().set_size(id, 240.0, 70.0);
                let loader = runtime.store_mut().create(ObjectType::Loader);
                runtime.store_mut().set_parent(loader, Some(container));
                runtime.store_mut().set_xy(loader, xy.x, xy.y + 100.0);
                runtime.store_mut().set_size(loader, 240.0, 85.0);
                runtime.store_mut().set_loader_layout(loader, 1, 1, 1);
                let source = if i % 2 == 0 { image } else { movie };
                runtime
                    .store_mut()
                    .apply_icon_to(loader, &format!("ui://{}{}", package.id(), source.id));
                for source in [id, loader] {
                    runtime.store_mut().set_color_filter(source, filter(i));
                    runtime.store_mut().set_grayed(source, i == 4);
                    runtime
                        .store_mut()
                        .set_alpha(source, if i == 5 { 0.5 } else { 1.0 });
                    state.sources.push(source);
                }
                let label = runtime.store_mut().create(ObjectType::Text);
                runtime.store_mut().set_parent(label, Some(container));
                runtime.store_mut().set_xy(label, xy.x, xy.y);
                runtime.store_mut().set_size(label, 280.0, 20.0);
                runtime.store_mut().set_object_text(
                    label,
                    [
                        "Original",
                        "Saturation -1",
                        "Hue +0.5",
                        "Combined",
                        "Grayed over combined",
                        "Brightness +0.25 / alpha 0.5",
                    ][i],
                );
                state.roots.push(index);
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
            runtime.queue_subtree(container);
            state.container = Some(container);
            state.stage = 1;
            state.since = frames;
        }
        1 if frames - state.since >= 12 => {
            for &index in &state.roots {
                let view = runtime
                    .entity_of(runtime.root_object(index).unwrap())
                    .unwrap();
                let mut image = images.get(view).unwrap().clone();
                image.image = state.pattern.as_ref().unwrap().clone();
                image.rect = Some(Rect::new(0.0, 0.0, 64.0, 32.0));
                commands.entity(view).insert((image, GlobalZIndex(601)));
                state.views.push(view);
            }
            commands
                .entity(runtime.entity_of(state.container.unwrap()).unwrap())
                .insert(GlobalZIndex(600));
            state.stage = 2;
            state.since = frames;
        }
        2 if frames - state.since >= 12 => {
            record_samples(state, runtime, nodes, "");
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-color-filters.png"));
            state.stage = 3;
            state.since = frames;
        }
        3 if frames - state.since >= 10 => {
            for &id in &state.sources {
                runtime.store_mut().set_color_filter(
                    id,
                    Some(ColorFilterParams {
                        hue: -0.5,
                        ..default()
                    }),
                );
                runtime.store_mut().set_grayed(id, false);
            }
            state.stage = 4;
            state.since = frames;
        }
        4 if frames - state.since >= 12 => {
            assert!(state.roots.iter().zip(&state.views).all(|(&root, &view)| {
                runtime.entity_of(runtime.root_object(root).unwrap()) == Some(view)
            }));
            record_samples(state, runtime, nodes, "-updated");
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-color-filters-updated.png",
                ));
            state.stage = 5;
            state.since = frames;
        }
        5 if frames - state.since >= 10 => {
            for &index in &state.roots {
                runtime.remove_root(commands, index);
            }
            runtime.store_mut().remove(state.container.unwrap());
            state.pattern = None;
            state.stage = 6;
            println!(
                "demo smoke: native local image/Loader/movie matrices, gray precedence/restore, dynamic hue, alpha, transparent swatches and 96 GPU samples captured"
            );
        }
        _ => {}
    }
}

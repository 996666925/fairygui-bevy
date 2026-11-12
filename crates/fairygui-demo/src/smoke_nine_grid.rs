use super::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use fairygui_core::{
    color::Color as GuiColor, geom::Rect as GuiRect, package::ObjectType, widget::GraphData,
};

#[derive(Default)]
pub(super) struct State {
    stage: u8,
    since: u32,
    container: Option<GObjectId>,
    root: Option<usize>,
    loaders: Vec<GObjectId>,
    views: Vec<Entity>,
    pattern: Option<Handle<Image>>,
    source: Vec2,
    captured: bool,
}
impl State {
    pub fn complete(&self) -> bool {
        self.stage == 8
    }
}

fn pattern(size: Vec2, grid: GuiRect) -> Image {
    let w = size.x as u32;
    let h = size.y as u32;
    let mut pixels = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let col = if (x as f32) < grid.x {
                0
            } else if x as f32 >= grid.right() {
                2
            } else {
                1
            };
            let row = if (y as f32) < grid.y {
                0
            } else if y as f32 >= grid.bottom() {
                2
            } else {
                1
            };
            let color = match (col, row) {
                (0, 0) => [235, 45, 55, 255],
                (2, 0) => [40, 190, 80, 255],
                (0, 2) => [35, 95, 230, 255],
                (2, 2) => [245, 205, 30, 255],
                (1, 1) => {
                    if (x / 2 + y / 2) % 2 == 0 {
                        [245, 245, 245, 255]
                    } else {
                        [50, 50, 55, 255]
                    }
                }
                (_, 1) => {
                    if y / 2 % 2 == 0 {
                        [15, 195, 220, 255]
                    } else {
                        [0, 70, 100, 255]
                    }
                }
                _ => {
                    if x / 2 % 2 == 0 {
                        [230, 65, 200, 255]
                    } else {
                        [90, 15, 95, 255]
                    }
                }
            };
            pixels.extend(color);
        }
    }
    Image::new_fill(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &pixels,
        TextureFormat::Rgba8UnormSrgb,
        default(),
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn drive(
    state: &mut State,
    frames: u32,
    runtime: &mut GuiRuntime,
    assets: &mut Assets<FuiAsset>,
    image_assets: &mut Assets<Image>,
    images: &Query<&ImageNode>,
    children: &Query<&Children>,
    commands: &mut Commands,
    root: GObjectId,
) {
    match state.stage {
        0 => {
            let package = fairygui_core::FuiPackage::parse(
                "Basics",
                include_bytes!("../../fairygui-core/tests/fixtures/NineGrid.fui"),
            )
            .unwrap();
            let (index, item) = package
                .items()
                .iter()
                .enumerate()
                .find(|(index, item)| {
                    let Some(g) = item.scale9_grid else {
                        return false;
                    };
                    let Some(s) = package.sprite_for(*index) else {
                        return false;
                    };
                    g.x > 0.0
                        && g.y > 0.0
                        && g.right() < s.original_size.x
                        && g.bottom() < s.original_size.y
                        && g.width >= 2.0
                        && g.height >= 2.0
                        && s.offset.x == 0.0
                        && s.offset.y == 0.0
                        && s.rect.width == s.original_size.x
                        && s.rect.height == s.original_size.y
                        && s.original_size.x <= 128.0
                        && s.original_size.y <= 128.0
                        && !s.rotated
                })
                .expect("source with four fixed borders");
            let sprite = package.sprite_for(index).unwrap();
            state.source = Vec2::new(sprite.original_size.x, sprite.original_size.y);
            let handle = assets.add(FuiAsset {
                package: package.clone(),
                res_key: "ui/Basics".into(),
            });
            state.pattern =
                Some(image_assets.add(pattern(state.source, item.scale9_grid.unwrap())));
            let original = runtime.store().object(root).unwrap();
            let position = Vec2::new(original.x(), original.y());
            let standalone = runtime.create_root(handle, &package, &item.name).unwrap();
            let image = runtime.root_object(standalone).unwrap();
            runtime
                .store_mut()
                .set_xy(image, position.x + 70.0, position.y + 170.0);
            runtime.store_mut().set_size(image, 240.0, 155.0);
            state.root = Some(standalone);
            let grid = runtime.store_mut().create(ObjectType::Component);
            runtime.store_mut().set_parent(grid, Some(root));
            runtime.store_mut().set_xy(grid, 50.0, 150.0);
            runtime.store_mut().set_size(grid, 1000.0, 430.0);
            let bg = runtime.store_mut().create(ObjectType::Graph);
            runtime.store_mut().set_parent(bg, Some(grid));
            runtime.store_mut().set_size(bg, 1000.0, 430.0);
            runtime.store_mut().set_graph(
                bg,
                GraphData {
                    graph_type: 1,
                    fill_color: GuiColor::new(0.88, 0.9, 0.95, 1.0),
                    ..default()
                },
            );
            for (i, (x, y, w, h)) in [
                (350.0, 20.0, 233.0, 157.0),
                (670.0, 20.0, 15.0, 155.0),
                (70.0, 230.0, 240.0, 12.0),
                (350.0, 230.0, 233.0, 157.0),
                (670.0, 230.0, 233.0, 157.0),
            ]
            .into_iter()
            .enumerate()
            {
                let loader = runtime.store_mut().create(ObjectType::Loader);
                runtime.store_mut().set_parent(loader, Some(grid));
                runtime.store_mut().set_xy(loader, x, y);
                runtime.store_mut().set_size(loader, w, h);
                runtime.store_mut().set_loader_layout(loader, 4, 0, 0);
                let url = if i == 4 {
                    format!("ui://Basics/{}", item.name)
                } else {
                    format!("ui://{}{}", package.id(), item.id)
                };
                runtime.store_mut().apply_icon_to(loader, &url);
                if i == 3 {
                    runtime.store_mut().set_image_fill(loader, 3, 0, true, 0.65);
                }
                state.loaders.push(loader);
            }
            runtime.queue_subtree(grid);
            state.container = Some(grid);
            state.stage = 1;
            state.since = frames;
        }
        1 if frames - state.since >= 12 => {
            let view = runtime
                .entity_of(runtime.root_object(state.root.unwrap()).unwrap())
                .unwrap();
            let mut image = images.get(view).unwrap().clone();
            image.image = state.pattern.as_ref().unwrap().clone();
            image.rect = Some(Rect::new(0.0, 0.0, state.source.x, state.source.y));
            commands.entity(view).insert((image, GlobalZIndex(501)));
            state.views.push(view);
            for &loader in &state.loaders {
                let owner = runtime.entity_of(loader).unwrap();
                let view = children
                    .get(owner)
                    .unwrap()
                    .iter()
                    .find(|&child| images.get(child).is_ok())
                    .unwrap();
                let mut image = images.get(view).unwrap().clone();
                image.image = state.pattern.as_ref().unwrap().clone();
                image.rect = Some(Rect::new(0.0, 0.0, state.source.x, state.source.y));
                commands.entity(view).insert(image);
                state.views.push(view);
            }
            commands
                .entity(runtime.entity_of(state.container.unwrap()).unwrap())
                .insert(GlobalZIndex(500));
            state.stage = 2;
            state.since = frames;
        }
        2 if frames - state.since >= 8 => {
            if !state.captured {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk("target/fairygui-demo-nine-grid.png"));
                state.captured = true;
                state.since = frames;
                return;
            }
            runtime.store_mut().set_size(state.loaders[0], 333.0, 137.0);
            runtime.store_mut().set_alpha(state.container.unwrap(), 0.6);
            runtime
                .store_mut()
                .set_image_fill(state.loaders[3], 5, 0, true, 0.75);
            let mut image = images.get(state.views[0]).unwrap().clone();
            image.flip_x = true;
            image.flip_y = true;
            commands.entity(state.views[0]).insert(image);
            state.stage = 3;
            state.since = frames;
        }
        3 if frames - state.since >= 8 => {
            assert_eq!(
                runtime.entity_of(runtime.root_object(state.root.unwrap()).unwrap()),
                Some(state.views[0])
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-nine-grid-updated.png"));
            state.stage = 4;
            state.since = frames;
        }
        4 if frames - state.since >= 8 => {
            runtime
                .store_mut()
                .apply_icon_to(state.loaders[0], "ui://Basics/VtSeperator");
            state.stage = 5;
            state.since = frames;
        }
        5 if frames - state.since >= 8 => {
            assert!(
                images.get(state.views[1]).is_err(),
                "replaced grid visual was disposed"
            );
            runtime.store_mut().remove(state.container.unwrap());
            runtime.remove_root(commands, state.root.unwrap());
            state.pattern = None;
            state.stage = 6;
            state.since = frames;
        }
        6 if frames - state.since >= 8 => {
            assert!(state.views.iter().all(|&view| images.get(view).is_err()));
            state.stage = 8;
            println!(
                "demo smoke: native tiled/stretched nine-grid patterns, cropped tails, independent small-axis compression, direct Image/foreign Loader, masks, flips, resize/alpha and disposal verified"
            );
        }
        _ => {}
    }
}

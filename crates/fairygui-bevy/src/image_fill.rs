//! Image masks share the normal Bevy UI material pipeline, preserving image
//! descriptors for atlas/frame changes while suppressing their unmasked draw.
use crate::{GObjectRef, GuiRuntime, backend, runtime::content::ContentVisual};
use bevy::{
    asset::embedded_asset,
    prelude::*,
    render::{
        RenderApp,
        render_resource::{AsBindGroup, ShaderType},
    },
    shader::ShaderRef,
    ui::widget::NodeImageMode,
    ui_render::{UiMaterialPlugin, ui_material::UiMaterial},
};
use fairygui_core::{
    Size,
    fill::fill_polygon,
    widget::{ControlData, FillMethod},
};

/// Source texture geometry for native nine-grid drawing. Layout covers its
/// virtual untrimmed canvas; the shader clips UVs to the actual atlas subtexture.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct ImageGrid {
    pub source: Vec2,
    pub offset: Vec2,
    pub grid: fairygui_core::geom::Rect,
    pub tiled: bool,
}
impl ImageGrid {
    pub(crate) fn from_package(
        sprite: &fairygui_core::package::AtlasSprite,
        item: &fairygui_core::package::PackageItem,
    ) -> Option<Self> {
        Some(Self {
            source: Vec2::new(sprite.original_size.x, sprite.original_size.y),
            offset: Vec2::new(sprite.offset.x, sprite.offset.y),
            grid: item.scale9_grid?,
            tiled: item.tile_grid_indice.unwrap_or(0) != 0,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, ShaderType)]
struct FillUniform {
    points: [Vec4; 12],
    domain: Vec4,
    atlas: Vec4,
    source_slices: Vec4,
    target_slices: Vec4,
    repeat: Vec4,
    grid_u: Vec4,
    grid_v: Vec4,
    clip: Vec4,
    tint: Vec4,
    color_rows: [Vec4; 4],
    color_offset: Vec4,
    effects: UVec4,
    flags: UVec4,
}
#[derive(Debug, Clone, Asset, TypePath, AsBindGroup)]
pub(crate) struct FillMaterial {
    #[uniform(0)]
    uniform: FillUniform,
    #[texture(1)]
    #[sampler(2)]
    texture: Handle<Image>,
}
impl UiMaterial for FillMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://fairygui_bevy/image_fill.wesl".into()
    }
}
pub(crate) fn install(app: &mut App) {
    if app.get_sub_app(RenderApp).is_some() {
        embedded_asset!(app, "image_fill.wesl");
        app.add_plugins(UiMaterialPlugin::<FillMaterial>::default());
    } else {
        app.init_asset::<FillMaterial>();
    }
}

fn slices(mode: &NodeImageMode, source: Vec2, target: Vec2) -> (Vec4, Vec4, Vec4) {
    let full = Vec4::new(0.0, 0.0, 1.0, 1.0);
    let repeat = |extent: f32, target: f32, mode: &bevy::sprite::SliceScaleMode| match mode {
        bevy::sprite::SliceScaleMode::Stretch => 1.0,
        bevy::sprite::SliceScaleMode::Tile { stretch_value } => {
            target / (extent * stretch_value.max(0.001))
        }
    };
    match mode {
        NodeImageMode::Sliced(slicer) => {
            let scale = (target / source).min_element().min(slicer.max_corner_scale);
            let low = slicer.border.min_inset;
            let high = slicer.border.max_inset;
            let source_slices = Vec4::new(
                low.x / source.x,
                low.y / source.y,
                1.0 - high.x / source.x,
                1.0 - high.y / source.y,
            );
            let target_slices = Vec4::new(
                low.x / target.x * scale,
                low.y / target.y * scale,
                1.0 - high.x / target.x * scale,
                1.0 - high.y / target.y * scale,
            );
            let source_inner = source
                * Vec2::new(
                    source_slices.z - source_slices.x,
                    source_slices.w - source_slices.y,
                );
            let target_inner = target
                * Vec2::new(
                    target_slices.z - target_slices.x,
                    target_slices.w - target_slices.y,
                );
            (
                source_slices,
                target_slices,
                Vec4::new(
                    repeat(source_inner.x, target_inner.x, &slicer.sides_scale_mode),
                    repeat(source_inner.y, target_inner.y, &slicer.sides_scale_mode),
                    repeat(source_inner.x, target_inner.x, &slicer.center_scale_mode),
                    repeat(source_inner.y, target_inner.y, &slicer.center_scale_mode),
                ),
            )
        }
        NodeImageMode::Tiled {
            tile_x,
            tile_y,
            stretch_value,
        } => {
            let r = target / (source * stretch_value.max(0.001));
            (
                full,
                full,
                Vec4::new(
                    1.0,
                    1.0,
                    if *tile_x { r.x } else { 1.0 },
                    if *tile_y { r.y } else { 1.0 },
                ),
            )
        }
        _ => (full, full, Vec4::ONE),
    }
}

fn extent(value: Val, fallback: f32) -> f32 {
    if let Val::Px(v) = value { v } else { fallback }
}

#[allow(clippy::type_complexity)]
pub(crate) fn sync_image_fills(
    mut commands: Commands,
    gui: NonSend<GuiRuntime>,
    images: Option<Res<Assets<Image>>>,
    materials: Option<ResMut<Assets<FillMaterial>>>,
    roots: Query<&GObjectRef>,
    players: Query<&crate::movie::MovieClipPlayer>,
    mut images_query: Query<(
        Entity,
        Option<&GObjectRef>,
        Option<&ContentVisual>,
        Option<&ImageGrid>,
        Option<&crate::runtime::appearance::TextTintSource>,
        &Node,
        &mut ImageNode,
        Option<&MaterialNode<FillMaterial>>,
    )>,
) {
    let Some(mut materials) = materials else {
        return;
    };
    for (entity, object, visual, grid, source, node, mut image, previous) in &mut images_query {
        let id = object
            .map(|o| o.0)
            .or_else(|| source.map(|source| source.owner))
            .or_else(|| visual.and_then(|v| roots.get(v.root).ok().map(|o| o.0)));
        let Some(id) = id else {
            continue;
        };
        let Some(object) = gui.store().object(id) else {
            continue;
        };
        let (method, origin, cw, amount) = match object.control() {
            ControlData::Image(d) => (
                d.fill_method,
                d.fill_origin,
                d.fill_clockwise,
                d.fill_amount,
            ),
            ControlData::Loader(d) => (
                d.fill_method,
                d.fill_origin,
                d.fill_clockwise,
                d.fill_amount,
            ),
            ControlData::MovieClip(_) => (0, 0, false, 1.0),
            ControlData::Text(_) | ControlData::TextInput(_) if source.is_some() => {
                (0, 0, false, 1.0)
            }
            _ => continue,
        };
        let base = crate::runtime::appearance::base_color(object, source.and_then(|s| s.color));
        // Offscreen filtering must not change the earlier texture/tint product:
        // Laya multiplies encoded channels, whereas stock Bevy multiplies linear
        // samples. An identity sampling matrix preserves that native tint step
        // while the actual object/ancestor filter runs on the complete subtree.
        let matrix = gui.paint_matrix(object).or_else(|| {
            ((base.r != 1. || base.g != 1. || base.b != 1.) && gui.has_composited_filter(id))
                .then_some(fairygui_core::color_matrix::ColorMatrix::IDENTITY)
        });
        let color = backend::apply_alpha(backend::to_bevy_color(base), gui.paint_alpha(id));
        if method == 0 && grid.is_none() && matrix.is_none() {
            if previous.is_some() {
                commands
                    .entity(entity)
                    .remove::<MaterialNode<FillMaterial>>();
            }
            if image.color != color {
                image.color = color;
            }
            continue;
        }
        // Keep the ImageNode descriptor for movies, texture replacement and the
        // existing source layout. Transparent tint suppresses only its raw draw.
        if image.color != Color::NONE {
            image.color = Color::NONE;
        }
        let Some(texture) = images.as_ref().and_then(|images| images.get(&image.image)) else {
            continue;
        };
        let texture_size = texture.size().as_vec2();
        let rect = image
            .rect
            .unwrap_or(Rect::from_corners(Vec2::ZERO, texture_size));
        let target = Vec2::new(
            extent(node.width, object.width()),
            extent(node.height, object.height()),
        );
        if target.min_element() <= 0.0 || rect.size().min_element() <= 0.0 {
            continue;
        }
        let (size, domain) = if let Some(visual) = visual {
            let source = players
                .get(visual.root)
                .ok()
                .and_then(|player| player.current())
                .map(|frame| Vec2::new(frame.original_size.width, frame.original_size.height))
                .unwrap_or_else(|| {
                    if visual.source_size.min_element() > 0.0 {
                        visual.source_size
                    } else {
                        rect.size()
                    }
                });
            let canvas = match object.control() {
                ControlData::Loader(loader) => {
                    loader.content_rect(Size::new(source.x, source.y), object.size())
                }
                _ => fairygui_core::geom::Rect::new(0.0, 0.0, object.width(), object.height()),
            };
            (
                Size::new(canvas.width, canvas.height),
                Vec4::new(
                    extent(node.left, 0.0) - canvas.x,
                    extent(node.top, 0.0) - canvas.y,
                    target.x,
                    target.y,
                ),
            )
        } else if source.is_some() && object.control().as_text_field().is_some() {
            // Text glyph/inline quads can extend beyond an auto-sized owner's
            // model rectangle. The matrix covers the sampled quad itself.
            (
                Size::new(target.x, target.y),
                Vec4::new(0.0, 0.0, target.x, target.y),
            )
        } else {
            (object.size(), Vec4::new(0.0, 0.0, target.x, target.y))
        };
        let polygon = fill_polygon(
            size,
            FillMethod::from_code(method),
            origin,
            cw,
            if method == 0 { 1.0 } else { amount },
        );
        let mut points = [Vec4::ZERO; 12];
        assert!(
            polygon.len() <= points.len(),
            "native fill mask exceeds uniform capacity"
        );
        for (target, point) in points.iter_mut().zip(&polygon) {
            *target = Vec4::new(point.x as f32, point.y as f32, 0.0, 0.0);
        }
        let (source_slices, mut target_slices, mut repeat) =
            slices(&image.image_mode, rect.size(), target);
        let clip = Vec4::new(
            rect.min.x / texture_size.x,
            rect.min.y / texture_size.y,
            rect.max.x / texture_size.x,
            rect.max.y / texture_size.y,
        );
        let mut grid_u = Vec4::ZERO;
        let mut grid_v = Vec4::ZERO;
        if let Some(grid) = grid {
            let uv = (rect.min - grid.offset) / texture_size;
            let uv_size = grid.source / texture_size;
            let axes = fairygui_core::tile_grid::axes(
                grid.grid,
                Size::new(target.x, target.y),
                fairygui_core::tile_grid::GridTexture {
                    source: Size::new(grid.source.x, grid.source.y),
                    virtual_uv: fairygui_core::geom::Rect::new(uv.x, uv.y, uv_size.x, uv_size.y),
                    clip_uv: fairygui_core::geom::Rect::new(
                        clip.x,
                        clip.y,
                        clip.z - clip.x,
                        clip.w - clip.y,
                    ),
                    atlas: Size::new(texture_size.x, texture_size.y),
                },
            );
            if axes
                .x
                .into_iter()
                .chain(axes.y)
                .chain(axes.u)
                .chain(axes.v)
                .any(|v| !v.is_finite())
            {
                if let Some(mut material) = previous.and_then(|handle| materials.get_mut(&handle.0))
                {
                    material.uniform.flags.x = 0;
                }
                continue;
            }
            target_slices = Vec4::new(
                axes.x[1] as f32 / target.x,
                axes.y[1] as f32 / target.y,
                axes.x[2] as f32 / target.x,
                axes.y[2] as f32 / target.y,
            );
            grid_u = Vec4::from_array(axes.u.map(|v| v as f32));
            grid_v = Vec4::from_array(axes.v.map(|v| v as f32));
            if grid.tiled {
                let rx = ((axes.x[2] - axes.x[1]) as f32 / grid.grid.width).max(0.0);
                let ry = ((axes.y[2] - axes.y[1]) as f32 / grid.grid.height).max(0.0);
                repeat = Vec4::new(rx, ry, rx, ry);
            } else {
                repeat = Vec4::ONE;
            }
        }
        let c = color.to_linear();
        let m = matrix.unwrap_or_default().values;
        let uniform = FillUniform {
            points,
            domain,
            atlas: Vec4::new(
                rect.min.x / texture_size.x,
                rect.min.y / texture_size.y,
                rect.max.x / texture_size.x,
                rect.max.y / texture_size.y,
            ),
            source_slices,
            target_slices,
            repeat,
            grid_u,
            grid_v,
            clip,
            tint: Vec4::new(c.red, c.green, c.blue, c.alpha),
            color_rows: std::array::from_fn(|r| Vec4::from_slice(&m[r * 5..r * 5 + 4])),
            color_offset: Vec4::new(m[4], m[9], m[14], m[19]),
            effects: UVec4::new(u32::from(matrix.is_some()), 0, 0, 0),
            flags: UVec4::new(
                polygon.len() as u32,
                u32::from(image.flip_x),
                u32::from(image.flip_y),
                u32::from(grid.is_some()),
            ),
        };
        if let Some(mut material) = previous.and_then(|handle| materials.get_mut(&handle.0)) {
            if material.uniform != uniform || material.texture != image.image {
                material.uniform = uniform;
                material.texture = image.image.clone();
            }
        } else {
            let handle = materials.add(FillMaterial {
                uniform,
                texture: image.image.clone(),
            });
            commands.entity(entity).insert(MaterialNode(handle));
        }
    }
}

#[cfg(test)]
#[path = "image_fill_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "color_filter_tests.rs"]
mod color_tests;

#[cfg(test)]
#[path = "text_image_color_tests.rs"]
mod text_color_tests;

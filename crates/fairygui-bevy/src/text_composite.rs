//! Compose one vector text command before applying object opacity.
use super::*;
use fairygui_core::{color::Color as GuiColor, color_matrix::ColorMatrix};
use std::sync::Arc;

const TILE: u32 = 1024;

#[cfg(test)]
#[path = "text_composite_tests.rs"]
mod tests;

#[derive(Debug, Clone, PartialEq)]
struct Mask {
    texture: AssetId<Image>,
    rect: Rect,
    position: Vec2,
    color: GuiColor,
}

#[derive(Debug, Clone, PartialEq)]
struct Projection {
    section: u32,
    line: u32,
    members: Vec<usize>,
    strokes: Vec<Mask>,
    fills: Vec<Mask>,
    matrix: Option<ColorMatrix>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Surface {
    pub image: Handle<Image>,
    pub position: Vec2,
    pub rect: Rect,
    pub alpha: f32,
    projection: Arc<Projection>,
}

impl Surface {
    pub fn covers(&self, section: u32, line: u32) -> bool {
        self.projection.section == section && self.projection.line == line
    }

    pub fn first(&self) -> usize {
        self.projection.members[0]
    }

    pub fn contains(&self, index: usize) -> bool {
        self.projection.members.binary_search(&index).is_ok()
    }
}

pub(super) fn same_surfaces(a: &[Surface], b: &[Surface]) -> bool {
    a == b
}

#[allow(clippy::too_many_arguments)]
pub(super) fn project(
    strokes: &[StrokeGlyph],
    info: &TextLayoutInfo,
    block: &ComputedTextBlock,
    object: &fairygui_core::GObject,
    alpha: f32,
    matrix: Option<ColorMatrix>,
    source: Option<&runtime::appearance::TextTintSource>,
    sources: &Query<&runtime::appearance::TextTintSource>,
    cursor_style: Option<&bevy::text::TextCursorStyle>,
    previous: &[Surface],
    images: &mut Assets<Image>,
) -> Vec<Surface> {
    let mut groups: Vec<Projection> = Vec::new();
    for stroke in strokes {
        let index = groups
            .iter()
            .position(|g| g.section == stroke.section && g.line == stroke.line);
        let index = index.unwrap_or_else(|| {
            groups.push(Projection {
                section: stroke.section,
                line: stroke.line,
                members: Vec::new(),
                strokes: Vec::new(),
                fills: Vec::new(),
                matrix,
            });
            groups.len() - 1
        });
        groups[index].strokes.push(Mask {
            texture: stroke.texture,
            rect: stroke.rect,
            position: stroke.position,
            color: stroke.raw_color,
        });
    }
    let selection_override = cursor_style.is_some_and(|s| s.selected_text_color.is_some());
    let mut unsupported = HashSet::new();
    for (index, glyph) in info.glyphs.iter().enumerate() {
        let Some(group) = groups
            .iter_mut()
            .find(|g| g.section == glyph.section_index && g.line == glyph.line_index)
        else {
            continue;
        };
        // Keep Bevy's native color-font sampling for commands containing bitmap glyphs.
        if !glyph.atlas_info.is_alpha_mask {
            unsupported.insert((group.section, group.line));
        }
        // Custom selected-text paint is already opacity-adjusted by the input
        // system. Preserve Bevy's exact foreground override instead of filtering
        // or applying object alpha to it a second time.
        if selection_override && {
            let rect = Rect::from_center_size(glyph.position, glyph.atlas_info.rect.size());
            info.selection_rects
                .iter()
                .any(|s| s.contains(rect.min) && s.contains(rect.max))
        } {
            unsupported.insert((group.section, group.line));
        }
        group.members.push(index);
        let override_color = if glyph.section_index == 0 {
            source.and_then(|s| s.color)
        } else {
            block
                .entities()
                .get(glyph.section_index as usize)
                .and_then(|s| sources.get(s.entity).ok())
                .and_then(|s| s.color)
        };
        let color = runtime::appearance::base_color(object, override_color);
        group.fills.push(Mask {
            texture: glyph.atlas_info.texture,
            rect: glyph.atlas_info.rect,
            position: glyph.position,
            color,
        });
    }
    let mut surfaces = Vec::new();
    for group in groups {
        if group.members.is_empty() || unsupported.contains(&(group.section, group.line)) {
            continue;
        }
        // Missing CPU atlases must never hide the normal foreground.
        if !group.strokes.iter().chain(&group.fills).all(|m| {
            images.get(m.texture).is_some_and(|i| {
                i.data.is_some()
                    && i.texture_descriptor.format
                        == bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb
            })
        }) {
            continue;
        }
        let cached: Vec<_> = previous.iter().filter(|s| *s.projection == group).collect();
        if !cached.is_empty() {
            surfaces.extend(cached.into_iter().cloned().map(|mut s| {
                s.alpha = alpha;
                s
            }));
            continue;
        }
        surfaces.extend(rasterize(Arc::new(group), alpha, images));
    }
    surfaces
}

fn rasterize(group: Arc<Projection>, alpha: f32, images: &mut Assets<Image>) -> Vec<Surface> {
    let mut low = Vec2::splat(f32::INFINITY);
    let mut high = Vec2::splat(f32::NEG_INFINITY);
    for mask in group.strokes.iter().chain(&group.fills) {
        low = low.min(mask.position - mask.rect.size() / 2.);
        high = high.max(mask.position + mask.rect.size() / 2.);
    }
    low = low.floor();
    high = high.ceil();
    if !low.is_finite() || !high.is_finite() || high.cmple(low).any() {
        return Vec::new();
    }
    let size = (high - low).as_uvec2();
    let mut surfaces = Vec::new();
    for y in (0..size.y).step_by(TILE as usize) {
        for x in (0..size.x).step_by(TILE as usize) {
            let extent = UVec2::new((size.x - x).min(TILE), (size.y - y).min(TILE));
            // One pixel of neighboring content on all sides prevents tile seams.
            let padded = extent + UVec2::splat(2);
            let origin = low + Vec2::new(x as f32, y as f32) - Vec2::ONE;
            let bytes = raster_tile(&group, origin, padded, images);
            let sampler = images
                .get(group.strokes[0].texture)
                .map(|i| i.sampler.clone())
                .unwrap_or_default();
            let mut image = Image::new(
                bevy::render::render_resource::Extent3d {
                    width: padded.x,
                    height: padded.y,
                    depth_or_array_layers: 1,
                },
                bevy::render::render_resource::TextureDimension::D2,
                bytes,
                bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
                bevy::asset::RenderAssetUsages::default(),
            );
            image.sampler = sampler;
            let image = images.add(image);
            surfaces.push(Surface {
                image,
                position: origin + Vec2::ONE + extent.as_vec2() / 2.,
                rect: Rect::from_corners(Vec2::ONE, extent.as_vec2() + Vec2::ONE),
                alpha,
                projection: group.clone(),
            });
        }
    }
    surfaces
}

fn raster_tile(group: &Projection, origin: Vec2, size: UVec2, images: &Assets<Image>) -> Vec<u8> {
    let count = (size.x * size.y) as usize;
    let mut pixels = vec![GuiColor::TRANSPARENT; count];
    for masks in [&group.strokes, &group.fills] {
        // Union equal paints before applying their alpha, including fallback runs.
        let mut paints: Vec<GuiColor> = Vec::new();
        for mask in masks {
            if !paints.contains(&mask.color) {
                paints.push(mask.color);
            }
        }
        for color in paints {
            let mut coverage = vec![0f32; count];
            for mask in masks.iter().filter(|m| m.color == color) {
                let Some(image) = images.get(mask.texture) else {
                    continue;
                };
                let low = mask.position - mask.rect.size() / 2. - origin;
                let high = low + mask.rect.size();
                let start = (low.floor() - Vec2::ONE)
                    .max(Vec2::ZERO)
                    .as_uvec2()
                    .min(size);
                let end = (high.ceil() + Vec2::ONE)
                    .max(Vec2::ZERO)
                    .as_uvec2()
                    .min(size);
                for y in start.y..end.y {
                    for x in start.x..end.x {
                        let uv = mask.rect.min + Vec2::new(x as f32 + 0.5, y as f32 + 0.5) - low;
                        let index = (y * size.x + x) as usize;
                        coverage[index] = coverage[index].max(sample(image, mask.rect, uv));
                    }
                }
            }
            for (pixel, coverage) in pixels.iter_mut().zip(coverage) {
                *pixel = source_over(*pixel, color, coverage);
            }
        }
    }
    pixels
        .into_iter()
        .flat_map(|color| {
            let color = group.matrix.map_or(color, |m| m.apply(color));
            [color.r, color.g, color.b, color.a].map(|v| (v.clamp(0., 1.) * 255.).round() as u8)
        })
        .collect()
}

fn sample(image: &Image, rect: Rect, point: Vec2) -> f32 {
    let Some(bytes) = &image.data else { return 0. };
    let read = |x: f32, y: f32| {
        if x < rect.min.x || y < rect.min.y || x >= rect.max.x || y >= rect.max.y {
            return 0.;
        }
        bytes
            .get(((y as u32 * image.width() + x as u32) * 4 + 3) as usize)
            .map_or(0., |a| f32::from(*a) / 255.)
    };
    if image.sampler == bevy::image::ImageSampler::nearest() {
        return read(point.x.floor(), point.y.floor());
    }
    let point = point - Vec2::splat(0.5);
    let low = point.floor();
    let fraction = point - low;
    let a = read(low.x, low.y) * (1. - fraction.x) + read(low.x + 1., low.y) * fraction.x;
    let b = read(low.x, low.y + 1.) * (1. - fraction.x) + read(low.x + 1., low.y + 1.) * fraction.x;
    a * (1. - fraction.y) + b * fraction.y
}

fn source_over(dst: GuiColor, src: GuiColor, coverage: f32) -> GuiColor {
    let alpha = (src.a * coverage).clamp(0., 1.);
    let remaining = dst.a * (1. - alpha);
    let out = alpha + remaining;
    if out == 0. {
        return GuiColor::TRANSPARENT;
    }
    GuiColor::new(
        (src.r * alpha + dst.r * remaining) / out,
        (src.g * alpha + dst.g * remaining) / out,
        (src.b * alpha + dst.b * remaining) / out,
        out,
    )
}

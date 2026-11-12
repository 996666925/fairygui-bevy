//! Stroke the actual shaped vector glyphs; keep layout, input and fill unchanged.
use crate::{
    GObjectRef, GuiRuntime,
    runtime::{self, InputPasswordView, InputPromptOwner},
};
use bevy::{
    prelude::*,
    render::{Extract, ExtractSchedule, RenderApp},
    text::{
        ComputedTextBlock, EditableText, FontAtlas, FontHinting, GlyphCacheKey, TextLayoutInfo,
    },
};
use std::collections::{HashMap, HashSet};

#[path = "text_composite.rs"]
mod composite;
#[cfg(test)]
#[path = "text_stroke_tests.rs"]
mod tests;

/// Per-span overrides. None inherits the field's current style.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct TextStrokeSpan {
    pub width: Option<f32>,
    pub color: Option<fairygui_core::color::Color>,
}

#[derive(Debug, Clone, PartialEq)]
struct StrokeGlyph {
    texture: AssetId<Image>,
    rect: Rect,
    position: Vec2,
    color: LinearRgba,
    raw_color: fairygui_core::color::Color,
    section: u32,
    line: u32,
}

#[derive(Component, Debug, Default)]
pub(crate) struct TextStrokeGeometry {
    glyphs: Vec<StrokeGlyph>,
    surfaces: Vec<composite::Surface>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct StrokeKey {
    font: u64,
    index: u32,
    size: u32,
    width: u32,
    coords: Vec<i16>,
    hint: bool,
    smooth: bool,
}

#[derive(Debug, Default)]
struct StrokeAtlas {
    atlases: Vec<FontAtlas>,
    missing: HashSet<u16>,
}

#[derive(Resource, Default)]
struct StrokeCache {
    fonts: HashMap<StrokeKey, StrokeAtlas>,
    scaler: swash::scale::ScaleContext,
}
impl std::fmt::Debug for StrokeCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StrokeCache")
            .field("fonts", &self.fonts)
            .finish_non_exhaustive()
    }
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<StrokeCache>().add_systems(
        PostUpdate,
        build_strokes
            .after(bevy::ui::widget::text_system)
            .after(bevy::ui::widget::update_editable_text_layout)
            .after(runtime::publish_password_layout),
    );
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render.init_resource::<ForegroundSnapshot>().add_systems(
            ExtractSchedule,
            (
                remember_foreground.before(bevy::ui_render::RenderUiSystems::ExtractText),
                extract_strokes.after(bevy::ui_render::RenderUiSystems::ExtractText),
            )
                .chain()
                .after(bevy::ui_render::RenderUiSystems::ExtractChanges)
                .after(bevy::ui_render::RenderUiSystems::ExtractCameraViews)
                .after(bevy::ui_render::RenderUiSystems::ExtractTextShadows)
                .before(bevy::ui_render::RenderUiSystems::ExtractCursor),
        );
    }
}

impl StrokeCache {
    fn glyph(
        &mut self,
        key: &StrokeKey,
        font: &[u8],
        glyph: u16,
        images: &mut Assets<Image>,
    ) -> Option<bevy::text::GlyphAtlasInfo> {
        let atlas = self.fonts.entry(key.clone()).or_default();
        let cache_key = GlyphCacheKey { glyph_id: glyph };
        if let Some(info) = bevy::text::get_glyph_atlas_info(&mut atlas.atlases, cache_key) {
            return Some(info);
        }
        if atlas.missing.contains(&glyph) {
            return None;
        }
        let font = swash::FontRef::from_index(font, key.index as usize)?;
        let mut scaler = self
            .scaler
            .builder_with_id(font, [key.font, key.index.into()])
            .size(f32::from_bits(key.size))
            .hint(key.hint)
            .normalized_coords(&key.coords)
            .build();
        let mut stroke = swash::zeno::Stroke::new(f32::from_bits(key.width));
        stroke.join(swash::zeno::Join::Round);
        let Some(mask) = swash::scale::Render::new(&[swash::scale::Source::Outline])
            .style(stroke)
            .render(&mut scaler, glyph)
        else {
            atlas.missing.insert(glyph);
            return None;
        };
        if mask.placement.width == 0 || mask.placement.height == 0 {
            atlas.missing.insert(glyph);
            return None;
        }
        let mut bytes = Vec::with_capacity(mask.data.len() * 4);
        for alpha in mask.data {
            bytes.extend_from_slice(&[
                255,
                255,
                255,
                if key.smooth {
                    alpha
                } else {
                    if alpha > 127 { 255 } else { 0 }
                },
            ]);
        }
        let image = Image::new(
            bevy::render::render_resource::Extent3d {
                width: mask.placement.width,
                height: mask.placement.height,
                depth_or_array_layers: 1,
            },
            bevy::render::render_resource::TextureDimension::D2,
            bytes,
            bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
            bevy::asset::RenderAssetUsages::MAIN_WORLD,
        );
        let offset = Vec2::new(mask.placement.left as f32, -mask.placement.top as f32);
        for atlas in &mut atlas.atlases {
            if atlas
                .add_glyph(images, cache_key, &image, offset, true)
                .is_ok()
            {
                return bevy::text::get_glyph_atlas_info(std::slice::from_mut(atlas), cache_key);
            }
        }
        let size = UVec2::new(
            (mask.placement.width + 4).next_power_of_two().max(512),
            (mask.placement.height + 4).next_power_of_two().max(512),
        );
        let smoothing = if key.smooth {
            bevy::text::FontSmoothing::AntiAliased
        } else {
            bevy::text::FontSmoothing::None
        };
        let mut page = FontAtlas::new(images, size, smoothing);
        page.add_glyph(images, cache_key, &image, offset, true)
            .ok()?;
        atlas.atlases.push(page);
        bevy::text::get_glyph_atlas_info(&mut atlas.atlases, cache_key)
    }
}

#[allow(clippy::too_many_arguments)]
fn layout_strokes(
    layout: &parley::Layout<bevy::text::TextBrush>,
    block: &ComputedTextBlock,
    data: &fairygui_core::widget::TextFieldData,
    alpha: f32,
    matrix: Option<fairygui_core::color_matrix::ColorMatrix>,
    hint: FontHinting,
    spans: &Query<&TextStrokeSpan>,
    cache: &mut StrokeCache,
    images: &mut Assets<Image>,
    used: &mut HashSet<StrokeKey>,
) -> Vec<StrokeGlyph> {
    let mut out = Vec::new();
    for (line_index, line) in layout.lines().enumerate() {
        for item in line.items() {
            let parley::PositionedLayoutItem::GlyphRun(run) = item else {
                continue;
            };
            let brush = run.style().brush;
            let span = block
                .entities()
                .get(brush.section_index as usize)
                .and_then(|s| spans.get(s.entity).ok());
            let width = span.and_then(|s| s.width).unwrap_or(data.stroke);
            if !width.is_finite() || width <= 0. {
                continue;
            }
            let color = span
                .and_then(|s| s.color)
                .or(data.stroke_color)
                .unwrap_or(fairygui_core::color::Color::new(0., 0., 0., 1.));
            let raw_color = color;
            let color = matrix.map_or(color, |m| m.apply(color));
            let color = crate::backend::apply_alpha(crate::backend::to_bevy_color(color), alpha)
                .to_linear();
            let shaped = run.run();
            let font = shaped.font();
            let smooth = brush.font_smoothing == bevy::text::FontSmoothing::AntiAliased;
            let key = StrokeKey {
                font: font.data.id(),
                index: font.index,
                size: shaped.font_size().to_bits(),
                width: (width * layout.scale()).to_bits(),
                coords: shaped.normalized_coords().to_vec(),
                hint: hint == FontHinting::Enabled && smooth,
                smooth,
            };
            used.insert(key.clone());
            for glyph in run.positioned_glyphs() {
                let Ok(glyph_id) = u16::try_from(glyph.id) else {
                    continue;
                };
                let Some(atlas) = cache.glyph(&key, font.data.as_ref(), glyph_id, images) else {
                    continue;
                };
                let baseline = Vec2::new(glyph.x, glyph.y);
                out.push(StrokeGlyph {
                    texture: atlas.texture,
                    rect: atlas.rect,
                    position: (if smooth { baseline } else { baseline.floor() })
                        + atlas.offset
                        + atlas.rect.size() / 2.,
                    color,
                    raw_color,
                    section: brush.section_index,
                    line: line_index as u32,
                });
            }
        }
    }
    out
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn build_strokes(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    images: Option<ResMut<Assets<Image>>>,
    mut cache: ResMut<StrokeCache>,
    mut roots: Query<
        (
            Entity,
            Option<&GObjectRef>,
            Option<&InputPromptOwner>,
            &ComputedTextBlock,
            &mut TextLayoutInfo,
            &FontHinting,
            Option<&EditableText>,
            Option<&InputPasswordView>,
            Option<&mut TextStrokeGeometry>,
            Option<&runtime::appearance::TextTintSource>,
            Option<&bevy::text::TextCursorStyle>,
        ),
        With<Text>,
    >,
    editors: Query<&EditableText>,
    spans: Query<&TextStrokeSpan>,
    tint_sources: Query<&runtime::appearance::TextTintSource>,
    obsolete: Query<Entity, (With<TextStrokeGeometry>, Without<Text>)>,
) {
    let Some(mut images) = images else {
        return;
    };
    let mut used = HashSet::new();
    for (
        entity,
        owner,
        prompt,
        block,
        mut info,
        hint,
        editor,
        password,
        previous,
        tint_source,
        cursor_style,
    ) in &mut roots
    {
        let Some(owner) = owner.map(|o| o.0).or_else(|| prompt.map(|o| o.0)) else {
            continue;
        };
        let Some(object) = runtime.store().object(owner) else {
            continue;
        };
        let Some(data) = object.control().as_text_field() else {
            continue;
        };
        let display_editor = password.and_then(|p| editors.get(p.0).ok()).or(editor);
        let layout = if let Some(editor) = display_editor {
            let Some(layout) = editor.editor.try_layout() else {
                continue;
            };
            layout
        } else {
            block.buffer()
        };
        // Rich spans can request strokes even when the base style is zero.
        let glyphs = layout_strokes(
            layout,
            block,
            data,
            runtime.paint_alpha(owner),
            runtime.paint_matrix(object),
            *hint,
            &spans,
            &mut cache,
            &mut images,
            &mut used,
        );
        let surfaces = composite::project(
            &glyphs,
            &info,
            block,
            object,
            runtime.paint_alpha(owner),
            runtime.paint_matrix(object),
            tint_source,
            &tint_sources,
            cursor_style,
            previous.as_deref().map_or(&[], |p| &p.surfaces),
            &mut images,
        );
        if let Some(mut previous) = previous {
            if previous.glyphs != glyphs || !composite::same_surfaces(&previous.surfaces, &surfaces)
            {
                previous.glyphs = glyphs;
                previous.surfaces = surfaces;
                // Bevy invalidates/extracts UI nodes through TextLayoutInfo.
                info.set_changed();
            }
        } else if !glyphs.is_empty() {
            commands
                .entity(entity)
                .insert(TextStrokeGeometry { glyphs, surfaces });
            info.set_changed();
        }
    }
    for entity in &obsolete {
        commands.entity(entity).remove::<TextStrokeGeometry>();
    }
    // Keep only font/size/variation/width combinations referenced this frame.
    // Releasing pages also releases generated image handles after disposal/switch.
    cache.fonts.retain(|key, _| used.contains(key));
}

#[derive(Resource, Default)]
struct ForegroundSnapshot(HashMap<Entity, HashSet<Entity>>);

fn remember_foreground(
    nodes: Res<bevy::ui_render::ExtractedUiNodes>,
    roots: Extract<Query<(), With<TextStrokeGeometry>>>,
    mut snapshot: ResMut<ForegroundSnapshot>,
) {
    snapshot.0.clear();
    for owner in &nodes.changed {
        if roots.contains(owner.entity()) {
            snapshot.0.insert(
                owner.entity(),
                nodes
                    .uinodes
                    .get(owner)
                    .map(|(_, items)| items.keys().copied().collect())
                    .unwrap_or_default(),
            );
        }
    }
}

#[allow(clippy::type_complexity)]
fn extract_strokes(
    mut commands: Commands,
    mut nodes: ResMut<bevy::ui_render::ExtractedUiNodes>,
    snapshot: Res<ForegroundSnapshot>,
    roots: Extract<
        Query<(
            Entity,
            &TextStrokeGeometry,
            &ComputedNode,
            &bevy::ui::ComputedStackIndex,
            &UiGlobalTransform,
            &InheritedVisibility,
            Option<&bevy::ui::CalculatedClip>,
            &bevy::ui::ComputedUiTargetCamera,
            Option<&EditableText>,
        )>,
    >,
    cameras: Extract<bevy::ui_render::UiCameraMap>,
) {
    use bevy::ui_render::{ExtractedGlyph, ExtractedUiItem, ExtractedUiNode};
    let mut mapper = cameras.get_mapper();
    let nodes = &mut *nodes;
    for (entity, stroke, node, stack, transform, visibility, clip, camera, editor) in nodes
        .changed
        .iter()
        .filter_map(|entity| roots.get(entity.entity()).ok())
    {
        if !visibility.get() || node.is_empty() {
            continue;
        }
        let Some(camera) = mapper.map(camera) else {
            continue;
        };
        let clip = if editor.is_some() {
            Some(
                clip.cloned()
                    .unwrap_or_default()
                    .with_rect(node.content_box(), transform),
            )
        } else {
            clip.cloned()
        };
        let transform = bevy::math::Affine2::from(transform)
            * bevy::math::Affine2::from_translation(
                node.content_box().min - editor.map_or(Vec2::ZERO, |e| e.viewport.offset),
            );
        let old = snapshot.0.get(&entity);
        let main_entity = bevy::render::sync_world::MainEntity::from(entity);
        let foreground: Vec<_> = nodes
            .uinodes
            .get(&main_entity)
            .into_iter()
            .flat_map(|(_, items)| items.iter())
            .filter(|(id, n)| {
                !old.is_some_and(|s| s.contains(id))
                    && matches!(n.item, ExtractedUiItem::Glyphs { .. })
            })
            .map(|(id, _)| *id)
            .collect();
        // Retain separate strokes only for commands with no compositable fill.
        let fallback: Vec<_> = stroke
            .glyphs
            .iter()
            .filter(|g| !stroke.surfaces.iter().any(|s| s.covers(g.section, g.line)))
            .collect();
        for batch in fallback.chunk_by(|a, b| a.texture == b.texture) {
            nodes
                .uinodes
                .entry(entity.into())
                .or_insert_with(|| (camera, default()))
                .1
                .insert(
                    commands.spawn_empty().id(),
                    ExtractedUiNode {
                        z_order: stack.0 as f32 + 0.058,
                        image: batch[0].texture,
                        clip: clip.clone(),
                        transform,
                        item: ExtractedUiItem::Glyphs {
                            glyphs: batch
                                .iter()
                                .map(|g| ExtractedGlyph {
                                    color: g.color,
                                    translation: g.position,
                                    rect: g.rect,
                                })
                                .collect(),
                        },
                    },
                );
        }
        if stroke.surfaces.is_empty() {
            continue;
        }
        let Some((_, items)) = nodes.uinodes.get_mut(&main_entity) else {
            continue;
        };
        replace_foreground(&mut commands, items, &foreground, &stroke.surfaces);
    }
}

fn replace_foreground(
    commands: &mut Commands,
    items: &mut bevy::ecs::entity::EntityIndexMap<bevy::ui_render::ExtractedUiNode>,
    foreground: &[Entity],
    surfaces: &[composite::Surface],
) {
    use bevy::ui_render::{ExtractedGlyph, ExtractedUiItem, ExtractedUiNode};
    // Remove all original batches before emitting replacements: swap_remove can
    // otherwise move an already emitted span ahead of the preceding surface.
    let originals: Vec<_> = foreground
        .iter()
        .filter_map(|id| {
            let original = items.swap_remove(id)?;
            commands.entity(*id).despawn();
            Some(original)
        })
        .collect();
    let mut output: Vec<ExtractedUiNode> = Vec::new();
    let mut index = 0;
    for original in originals {
        let ExtractedUiItem::Glyphs { glyphs } = original.item else {
            unreachable!()
        };
        for glyph in glyphs {
            let mut emit = |image, glyph| {
                // All foreground items for this owner share clip/transform/z.
                if let Some(last) = output.last_mut()
                    && last.image == image
                    && let ExtractedUiItem::Glyphs { glyphs } = &mut last.item
                {
                    glyphs.push(glyph);
                    return;
                }
                output.push(ExtractedUiNode {
                    z_order: original.z_order,
                    image,
                    clip: original.clip.clone(),
                    transform: original.transform,
                    item: ExtractedUiItem::Glyphs {
                        glyphs: vec![glyph],
                    },
                });
            };
            for surface in surfaces.iter().filter(|s| s.first() == index) {
                emit(
                    surface.image.id(),
                    ExtractedGlyph {
                        color: LinearRgba::new(1., 1., 1., surface.alpha),
                        translation: surface.position,
                        rect: surface.rect,
                    },
                );
            }
            if !surfaces.iter().any(|s| s.contains(index)) {
                emit(original.image, glyph);
            }
            index += 1;
        }
    }
    for node in output {
        items.insert(commands.spawn_empty().id(), node);
    }
}

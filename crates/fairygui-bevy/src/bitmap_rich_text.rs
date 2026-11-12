use super::*;
#[cfg(test)]
#[path = "bitmap_rich_text_tests.rs"]
mod tests;
use fairygui_core::{
    bitmap_text::{BitmapElement, BitmapElementKind, layout_bitmap_rich_text},
    package::BitmapFontData,
    text::UbbSpan,
};

#[derive(Component, Debug)]
pub(crate) struct BitmapRichText {
    data: fairygui_core::widget::TextFieldData,
    source: String,
    size: fairygui_core::Size,
    image_sources: Vec<(String, Option<rich_text::InlineImageIdentity>)>,
    parts: Vec<Entity>,
    links: Vec<(usize, String, fairygui_core::geom::Rect)>,
    generation: Entity,
}
impl BitmapRichText {
    pub(super) fn despawn_parts(&self, commands: &mut Commands) {
        for &part in &self.parts {
            commands.entity(part).despawn();
        }
    }
    pub(crate) fn link_at(
        &self,
        runtime: &GuiRuntime,
        object: GObjectId,
        normalized: Vec2,
        computed: &ComputedNode,
    ) -> Option<LinkHit> {
        let current = runtime.store().object(object)?;
        let data = current.control().as_text_field()?;
        if self.data != *data
            || self.source != text_fonts::display_text(current, &runtime.translate(&data.text))
            || self.size != current.size()
        {
            return None;
        }
        let local = normalized * computed.size() * computed.inverse_scale_factor();
        self.links.iter().find_map(|(group, href, r)| {
            r.contains(fairygui_core::geom::Point::new(local.x, local.y))
                .then(|| LinkHit::bitmap(object, self.generation, *group, href.clone()))
        })
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn spawn(
    commands: &mut Commands,
    entity: Entity,
    owner: GObjectId,
    object: &fairygui_core::GObject,
    asset: &FuiAsset,
    font_asset: &FuiAsset,
    assets: &Assets<FuiAsset>,
    server: &AssetServer,
    image_assets: &Assets<Image>,
    font: &BitmapFontData,
    text: &str,
) {
    let data = object.control().as_text_field().unwrap();
    let spans = crate::text::parse_for(data, text).spans;
    let mut elements = Vec::new();
    let mut styles = Vec::new();
    let mut sources = Vec::new();
    let mut image_sources = Vec::new();
    let mut active = None;
    let push = |elements: &mut Vec<BitmapElement>,
                styles: &mut Vec<usize>,
                sources: &mut Vec<Option<rich_text::InlineImageSource>>,
                kind,
                span: usize,
                s: &UbbSpan,
                image| {
        elements.push(BitmapElement {
            kind,
            font_size: s.size.unwrap_or(data.font_size),
        });
        styles.push(span);
        sources.push(image);
    };
    for (i, span) in spans.iter().enumerate() {
        if span.link_id != active {
            if active.is_some() {
                push(
                    &mut elements,
                    &mut styles,
                    &mut sources,
                    BitmapElementKind::LinkEnd,
                    i,
                    span,
                    None,
                );
            }
            active = span.link_id;
            if active.is_some() {
                push(
                    &mut elements,
                    &mut styles,
                    &mut sources,
                    BitmapElementKind::LinkStart,
                    i,
                    span,
                    None,
                );
            }
        }
        if let Some(url) = &span.image {
            image_sources.push((
                url.clone(),
                rich_text::image_identity(asset, assets, server, image_assets, url),
            ));
            if let Some(image) = rich_text::inline_image(
                asset,
                assets,
                server,
                image_assets,
                url,
                span.image_dimensions,
            ) {
                let size = fairygui_core::Size::new(image.logical_size.x, image.logical_size.y);
                push(
                    &mut elements,
                    &mut styles,
                    &mut sources,
                    BitmapElementKind::Image(size),
                    i,
                    span,
                    Some(image),
                );
            }
        } else {
            push(
                &mut elements,
                &mut styles,
                &mut sources,
                BitmapElementKind::Text(span.text.clone()),
                i,
                span,
                None,
            );
        }
    }
    if active.is_some()
        && let Some(span) = spans.last()
    {
        push(
            &mut elements,
            &mut styles,
            &mut sources,
            BitmapElementKind::LinkEnd,
            spans.len() - 1,
            span,
            None,
        );
    }
    let layout = layout_bitmap_rich_text(font, data, &elements, object.size());
    let mut glyphs = Vec::new();
    let mut parts = Vec::new();
    for quad in &layout.glyphs {
        let g = &quad.glyph;
        // Native can select negative draw sizes after degenerate tiny-box shrink.
        // Bevy UI cannot represent negative node extents; keep those in core
        // measurements/reference geometry and omit the invalid render quad.
        if g.width <= 0.0 || g.height <= 0.0 {
            continue;
        }
        let glyph = crate::text::PlacedGlyph {
            tint: font.tint,
            x: g.x,
            y: g.y,
            width: g.width,
            height: g.height,
            texture: font.glyph(g.char_code).and_then(|g| g.texture.clone()),
        };
        if let Some(part) = spawn_glyph(commands, entity, owner, &glyph, font_asset, server) {
            commands.entity(part).insert(appearance::TextTintSource {
                owner,
                color: if font.tint {
                    spans[styles[quad.element]].color
                } else {
                    Some(fairygui_core::color::Color::WHITE)
                },
            });
            glyphs.push(part);
        }
    }
    for &(i, rect) in &layout.images {
        let Some(source) = &sources[i] else {
            continue;
        };
        let mut image = commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(
                    rect.x + source.offset.x
                        - if source.logical_size.x > 0.0 {
                            1.0
                        } else {
                            0.0
                        },
                ),
                top: Val::Px(rect.y + source.offset.y),
                width: Val::Px(source.size.x),
                height: Val::Px(source.size.y),
                ..default()
            },
            source.image.clone(),
            if source.pending {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            },
            appearance::TextTintSource {
                owner,
                color: Some(fairygui_core::color::Color::WHITE),
            },
            bevy::picking::Pickable::IGNORE,
            ChildOf(entity),
        ));
        if let Some(player) = source.movie.clone() {
            image.insert(rich_text::InlineMovieClipPlayer::new(owner, player));
        }
        let part = image.id();
        parts.push(part);
    }
    for run in &layout.runs {
        let span = &spans[styles[run.element]];
        if run.rect.width <= 0.0
            || !matches!(elements[run.element].kind, BitmapElementKind::Text(_))
        {
            continue;
        }
        for strike in [false, true] {
            if (!strike && !(data.underline || span.underline)) || (strike && !span.strikethrough) {
                continue;
            }
            let rect = fairygui_core::bitmap_text::bitmap_decoration_rect(
                f64::from(run.rect.x),
                f64::from(run.rect.width),
                f64::from(run.line_y),
                f64::from(run.line_height),
                f64::from(run.font_size),
                strike,
            );
            let part = spawn_text_decoration(commands, entity, owner, rect, span.color);
            parts.push(part);
        }
    }
    let links = layout
        .links
        .into_iter()
        .filter_map(|(i, rect)| {
            let span = &spans[styles[i]];
            Some((span.link_id?, span.url.clone()?, rect))
        })
        .collect();
    let generation = commands.spawn(ChildOf(entity)).id();
    parts.push(generation);
    commands.entity(entity).insert((
        BitmapGlyphs(glyphs),
        BitmapTextMetrics {
            text_size: layout.text_size,
            object_size: layout.object_size,
        },
        BitmapRichText {
            data: data.clone(),
            source: text.into(),
            size: object.size(),
            image_sources,
            parts,
            links,
            generation,
        },
    ));
}

#[allow(clippy::type_complexity)]
pub(crate) fn sync_bitmap_rich_text(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    assets: Res<Assets<FuiAsset>>,
    server: Res<AssetServer>,
    image_assets: Res<Assets<Image>>,
    query: Query<(Entity, &GObjectRef, &BitmapRichText, &BitmapGlyphs)>,
) {
    for (entity, owner, previous, glyphs) in &query {
        let Some(object) = runtime.store().object(owner.0) else {
            continue;
        };
        let Some(data) = object
            .control()
            .as_text_field()
            .filter(|d| d.ubb_enabled || d.html_enabled)
        else {
            continue;
        };
        let Some(handle) = runtime.asset_handle_for(owner.0) else {
            continue;
        };
        let Some(asset) = assets.get(&handle) else {
            continue;
        };
        let text = text_fonts::display_text(object, &runtime.translate(&data.text));
        if previous.data == *data
            && previous.source == text
            && previous.size == object.size()
            && previous.image_sources.iter().all(|(url, id)| {
                *id == rich_text::image_identity(asset, &assets, &server, &image_assets, url)
            })
        {
            continue;
        }
        let Some((font_asset, font)) = data
            .font
            .as_deref()
            .and_then(|name| text_fonts::resolve_bitmap_font(asset, &assets, name))
        else {
            continue;
        };
        for &glyph in &glyphs.0 {
            commands.entity(glyph).despawn();
        }
        previous.despawn_parts(&mut commands);
        spawn(
            &mut commands,
            entity,
            owner.0,
            object,
            asset,
            font_asset,
            &assets,
            &server,
            &image_assets,
            &font,
            &text,
        );
    }
}

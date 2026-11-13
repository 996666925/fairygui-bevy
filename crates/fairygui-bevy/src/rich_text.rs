use super::*;
use bevy::text::{InlineBox, InlineBoxKind};

#[cfg(test)]
#[path = "external_inline_image_tests.rs"]
mod external_tests;
#[cfg(test)]
#[path = "rich_text_tests.rs"]
mod tests;

/// Text entity that owns an inline image, independently of its layout parent.
/// Linked ownership reclaims sibling images when that text entity is despawned.
#[derive(Component, Debug, Clone, Copy)]
#[relationship(relationship_target = InlineImageParts)]
pub struct InlineImageOwner(pub Entity);

/// Automatically maintained image entities belonging to one text entity.
#[derive(Component, Debug, Default)]
#[relationship_target(relationship = InlineImageOwner, linked_spawn)]
pub struct InlineImageParts(Vec<Entity>);

impl InlineImageParts {
    pub fn iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.0.iter().copied()
    }
}

/// A sibling rotates/scales about its own center. Compensate that center so its
/// quad follows the text's transform about the text center/pivot instead.
pub(super) fn image_transform(
    text: &UiTransform,
    text_size: Vec2,
    local: Vec2,
    size: Vec2,
) -> UiTransform {
    let relative_center = local + size * 0.5 - text_size * 0.5;
    let translation = Vec2::new(px(text.translation.x), px(text.translation.y))
        + text.rotation * (text.scale * relative_center)
        - relative_center;
    UiTransform {
        translation: Val2::px(translation.x, translation.y),
        scale: text.scale,
        rotation: text.rotation,
    }
}

#[derive(Component, Debug)]
pub(crate) struct InlineImage {
    pub inline_box: Entity,
    pub offset: Vec2,
    pub size: Vec2,
}

#[derive(Component, Debug)]
pub(crate) struct InlineImagePending;

#[derive(Component, Debug)]
pub(crate) struct InlineMovieClipPlayer {
    owner: GObjectId,
    player: crate::movie::MovieClipPlayer,
    visible: bool,
}

impl InlineMovieClipPlayer {
    pub(crate) fn new(owner: GObjectId, player: crate::movie::MovieClipPlayer) -> Self {
        let visible = player.current().is_some();
        Self {
            owner,
            player,
            visible,
        }
    }

    #[cfg(test)]
    pub(crate) fn frame(&self) -> usize {
        self.player.frame()
    }

    #[cfg(test)]
    pub(crate) fn frame_count(&self) -> usize {
        self.player.frame_count()
    }

    fn visible(&self) -> bool {
        self.visible
    }
}

pub(crate) fn advance_inline_movie_clips(
    mut query: Query<(&mut InlineMovieClipPlayer, &mut ImageNode)>,
    time: Res<Time>,
    runtime: NonSend<GuiRuntime>,
) {
    for (mut clip, mut image) in &mut query {
        if runtime.store().is_on_stage(clip.owner) && clip.player.advance(time.delta_secs()) {
            clip.visible = clip.player.current().is_some();
            if clip.visible {
                clip.player.apply_frame(&mut image);
            }
        }
    }
}

/// Keep image spans beside their text node, but inherit that node's visibility
/// and order. Explicit hidden text cannot leave a visible sibling image behind.
#[allow(clippy::type_complexity)]
pub(crate) fn sync_inline_image_hierarchy(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    owners: Query<(&GObjectRef, Option<&ChildOf>, Option<&ZIndex>), Without<InlineImage>>,
    mut images: Query<
        (
            Entity,
            &InlineImageOwner,
            Option<&ChildOf>,
            &mut Visibility,
            &mut ZIndex,
            Option<&InlineImagePending>,
            Option<&InlineMovieClipPlayer>,
        ),
        With<InlineImage>,
    >,
) {
    for (entity, owner, parent, mut visibility, mut z, pending, movie) in &mut images {
        let Ok((object, text_parent, order)) = owners.get(owner.0) else {
            commands.entity(entity).despawn();
            continue;
        };
        let Some(gobject) = runtime.store().object(object.0) else {
            commands.entity(entity).despawn();
            continue;
        };
        let next = gobject.parent().and_then(|id| runtime.entity_of(id));
        if parent.map(ChildOf::parent) != next {
            if let Some(parent) = next {
                commands.entity(entity).insert(ChildOf(parent));
            } else {
                commands.entity(entity).remove::<ChildOf>();
            }
        }
        let shown = pending.is_none()
            && runtime.store().is_on_stage(object.0)
            && runtime.store().display_visible(object.0)
            && movie.is_none_or(|movie| movie.visible())
            && (gobject.parent().is_none() || next.is_some())
            && text_parent.map(ChildOf::parent) == next;
        let next = if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != next {
            *visibility = next;
        }
        let next = order.copied().unwrap_or_default();
        if *z != next {
            *z = next;
        }
    }
}

/// Rebuild only text-owned spans and images; ordinary GObject children survive.
/// Compare a rendering snapshot instead of consuming the model's dirty queue.
#[allow(clippy::type_complexity)]
pub(crate) fn sync_rich_text(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    assets: Res<Assets<FuiAsset>>,
    asset_server: Res<AssetServer>,
    image_assets: Res<Assets<Image>>,
    query: Query<(
        Entity,
        &GObjectRef,
        &RichTextBlock,
        Option<&InlineImageParts>,
        Option<&text_fonts::TextFontBinding>,
    )>,
) {
    for (entity, object, previous, images, binding) in &query {
        let Some(gobject) = runtime.store().object(object.0) else {
            continue;
        };
        let Some(data) = gobject.control().as_text_field() else {
            continue;
        };
        let text = text_fonts::display_text(gobject, &runtime.translate(&data.text));
        let font = runtime.font.as_ref();
        let raster = binding
            .map(text_fonts::TextFontBinding::raster)
            .cloned()
            .unwrap_or_else(|| {
                font.cloned()
                    .map(bevy::text::FontSource::Handle)
                    .unwrap_or_default()
            });
        let Some(asset) = assets.iter().find_map(|(_, asset)| {
            (asset.package.id() == previous.package_id && asset.res_key == previous.res_key)
                .then_some(asset)
        }) else {
            continue;
        };
        if previous.source == text
            && previous.data == *data
            && previous.font.as_ref() == font
            && previous.raster_font == raster
            && previous.image_sources.iter().all(|(source, identity)| {
                *identity == image_identity(asset, &assets, &asset_server, &image_assets, source)
            })
        {
            continue;
        }
        for &span in &previous.spans {
            commands.entity(span).despawn();
        }
        if let Some(images) = images {
            for image in images.iter() {
                commands.entity(image).despawn();
            }
        }
        let segments = if data.ubb_enabled || data.html_enabled {
            crate::text::layout_for(data, &text)
        } else {
            vec![crate::text::RichSegment::Text {
                text: text.clone(),
                color: None,
                size: None,
                bold: false,
                italic: false,
                underline: false,
                strikethrough: false,
                stroke: None,
                stroke_color: None,
                url: None,
                link_id: None,
            }]
        };
        let mut root = commands.entity(entity);
        // Reinserting Text invalidates the paragraph until its new span list has
        // been laid out. Old inline-box rectangles must never expose a new image early.
        root.insert((
            Text::new(String::new()),
            TextFont {
                font: raster.clone(),
                ..backend::text_font_for(gobject, data, font)
            },
            backend::text_layout_for_object(gobject, data),
            backend::text_line_height_for(data, None),
            backend::text_letter_spacing_for(data),
        ));
        let images = spawn_rich_content(
            &mut root,
            object.0,
            data,
            &text,
            &segments,
            asset,
            &assets,
            &asset_server,
            &image_assets,
            font,
            Some(&raster),
        );
        let parent = gobject.parent().and_then(|id| runtime.entity_of(id));
        for image in images {
            if let Some(parent) = parent {
                commands.entity(image).insert(ChildOf(parent));
            } else {
                commands.entity(image).remove::<ChildOf>();
            }
        }
    }
}

#[derive(Component, Debug)]
pub(crate) struct RichTextBlock {
    source: String,
    data: fairygui_core::widget::TextFieldData,
    font: Option<Handle<Font>>,
    raster_font: bevy::text::FontSource,
    package_id: String,
    res_key: String,
    spans: Vec<Entity>,
    image_sources: Vec<(String, Option<InlineImageIdentity>)>,
    links: Vec<(Entity, usize, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LinkHit {
    pub object: GObjectId,
    generation: Entity,
    group: usize,
    pub href: String,
}

impl LinkHit {
    pub(super) fn bitmap(
        object: GObjectId,
        generation: Entity,
        group: usize,
        href: String,
    ) -> Self {
        Self {
            object,
            generation,
            group,
            href,
        }
    }
}

impl RichTextBlock {
    pub(super) fn despawn_spans(&self, commands: &mut Commands) {
        for &span in &self.spans {
            commands.entity(span).despawn();
        }
    }
    /// Normalized coordinates have already passed Bevy's inverse UI transform.
    /// Run rectangles use physical content coordinates, including individual
    /// lines and nested styles, rather than a union spanning non-link gaps.
    pub(crate) fn link_at(
        &self,
        runtime: &GuiRuntime,
        object: GObjectId,
        normalized: Vec2,
        computed: &ComputedNode,
        block: &ComputedTextBlock,
        layout: &bevy::text::TextLayoutInfo,
    ) -> Option<LinkHit> {
        let data = runtime.store().object(object)?.control().as_text_field()?;
        if self.source != runtime.translate(&data.text)
            || self.data != *data
            || self.font.as_ref() != runtime.font.as_ref()
        {
            return None;
        }
        let local = normalized * computed.size() - computed.content_inset().min_inset;
        for run in &layout.run_geometry {
            if !run.bounds.contains(local) {
                continue;
            }
            let entity = block.entities().get(run.section_index as usize)?.entity;
            if let Some((_, group, href)) = self.links.iter().find(|(span, _, _)| *span == entity) {
                return Some(LinkHit {
                    object,
                    generation: *self.spans.first()?,
                    group: *group,
                    href: href.clone(),
                });
            }
        }
        for (entity, kind, bounds) in &layout.inline_boxes {
            if *kind != InlineBoxKind::InFlow || !bounds.contains(local) {
                continue;
            }
            if let Some((_, group, href)) = self.links.iter().find(|(span, _, _)| span == entity) {
                return Some(LinkHit {
                    object,
                    generation: *self.spans.first()?,
                    group: *group,
                    href: href.clone(),
                });
            }
        }
        None
    }
}

pub(super) fn image_source<'a>(
    own: &'a FuiAsset,
    assets: &'a Assets<FuiAsset>,
    source: &str,
) -> Option<(&'a FuiAsset, usize)> {
    if !source.starts_with("ui://") {
        return own
            .package
            .item_index_by_name(source)
            .map(|index| (own, index));
    }
    if let Some(index) = own.package.resolve_local_url(source) {
        return Some((own, index));
    }
    // Never borrow a duplicate copy of the owner's package from another directory.
    assets
        .iter()
        .filter(|(_, asset)| asset.package.id() != own.package.id())
        .filter_map(|(_, asset)| {
            asset
                .package
                .resolve_local_url(source)
                .map(|index| (asset, index))
        })
        .min_by(|(a, _), (b, _)| a.res_key.cmp(&b.res_key))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum InlineImageIdentity {
    Package {
        res_key: String,
        item: usize,
    },
    PackageMovieClip {
        res_key: String,
        item: usize,
    },
    External {
        image: Handle<Image>,
        size: Option<UVec2>,
    },
}

pub(super) fn image_identity(
    own: &FuiAsset,
    assets: &Assets<FuiAsset>,
    server: &AssetServer,
    images: &Assets<Image>,
    source: &str,
) -> Option<InlineImageIdentity> {
    if let Some((asset, item)) = image_source(own, assets, source) {
        let res_key = asset.res_key.clone();
        return Some(
            if asset.package.item(item).is_some_and(|item| {
                item.item_type == fairygui_core::package::PackageItemType::MovieClip
            }) {
                InlineImageIdentity::PackageMovieClip { res_key, item }
            } else {
                InlineImageIdentity::Package { res_key, item }
            },
        );
    }
    if source.is_empty() || source.starts_with("ui://") {
        return None;
    }
    let path = bevy::asset::AssetPath::try_parse(source).ok()?.into_owned();
    // Retained snapshot handles keep pending requests alive. Polling a failed
    // request reuses its handle rather than restarting a load every frame.
    let image = server
        .get_handle::<Image>(path.clone())
        .unwrap_or_else(|| server.load(path));
    let size = images.get(&image).map(Image::size);
    Some(InlineImageIdentity::External { image, size })
}

pub(super) struct InlineImageSource {
    pub canvas: Vec2,
    pub logical_size: Vec2,
    pub offset: Vec2,
    pub size: Vec2,
    pub image: ImageNode,
    pub pending: bool,
    pub movie: Option<crate::movie::MovieClipPlayer>,
}

pub(super) fn inline_image(
    own: &FuiAsset,
    assets: &Assets<FuiAsset>,
    server: &AssetServer,
    images: &Assets<Image>,
    source: &str,
    dimensions: fairygui_core::text::ImageDimensions,
) -> Option<InlineImageSource> {
    match image_identity(own, assets, server, images, source)? {
        InlineImageIdentity::PackageMovieClip { .. } => {
            let (asset, index) = image_source(own, assets, source)?;
            let player = super::movie_sources::player_for(asset, server, index, 0, true)?;
            let item = asset.package.item(index)?;
            let size = Vec2::new(item.width.max(0.0), item.height.max(0.0));
            let mut image = ImageNode {
                image_mode: bevy::ui::widget::NodeImageMode::Stretch,
                ..default()
            };
            player.apply_frame(&mut image);
            Some(InlineImageSource {
                canvas: Vec2::new(size.x + if size.x > 0.0 { 2.0 } else { 0.0 }, size.y),
                logical_size: size,
                offset: Vec2::new(if size.x > 0.0 { 1.0 } else { 0.0 }, 0.0),
                size,
                image,
                pending: false,
                movie: Some(player),
            })
        }
        InlineImageIdentity::Package { .. } => {
            let (asset, index) = image_source(own, assets, source)?;
            let sprite = asset.package.sprite_for(index)?;
            let file = asset.package.item(sprite.atlas_item)?.file.as_ref()?;
            Some(InlineImageSource {
                canvas: Vec2::new(sprite.original_size.x + 2.0, sprite.original_size.y),
                logical_size: Vec2::new(sprite.original_size.x, sprite.original_size.y),
                offset: Vec2::new(sprite.offset.x + 1.0, sprite.offset.y),
                size: Vec2::new(sprite.rect.width, sprite.rect.height),
                image: backend::image_for(
                    sprite,
                    server.load(asset.resolve_asset_path(file)),
                    None,
                    false,
                ),
                pending: false,
                movie: None,
            })
        }
        InlineImageIdentity::External { image, size } => {
            let pending = size.is_none();
            let natural = size.map(|s| fairygui_core::Size::new(s.x as f32, s.y as f32));
            let resolved = dimensions.resolve(natural);
            // Negative native Sprite sizes cannot be represented by Bevy nodes.
            let size = Vec2::new(resolved.width.max(0.0), resolved.height.max(0.0));
            let margin = if size.x > 0.0 { 1.0 } else { 0.0 };
            Some(InlineImageSource {
                canvas: Vec2::new(size.x + 2.0 * margin, size.y),
                logical_size: size,
                offset: Vec2::new(margin, 0.0),
                size,
                image: ImageNode {
                    image,
                    image_mode: bevy::ui::widget::NodeImageMode::Stretch,
                    ..default()
                },
                pending,
                movie: None,
            })
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_rich_content(
    entity: &mut bevy::ecs::system::EntityCommands<'_>,
    object: GObjectId,
    data: &fairygui_core::widget::TextFieldData,
    text: &str,
    segments: &[crate::text::RichSegment],
    asset: &FuiAsset,
    assets: &Assets<FuiAsset>,
    asset_server: &AssetServer,
    image_assets: &Assets<Image>,
    font: Option<&Handle<Font>>,
    raster: Option<&bevy::text::FontSource>,
) -> Vec<Entity> {
    let text_entity = entity.id();
    let mut spans = Vec::new();
    let mut images = Vec::new();
    let mut image_sources = Vec::new();
    let mut links = Vec::new();
    let raster = raster.cloned().unwrap_or_else(|| {
        font.cloned()
            .map(bevy::text::FontSource::Handle)
            .unwrap_or_default()
    });

    entity.with_children(|parent| {
        for (segment_index, segment) in segments.iter().enumerate() {
            let (url, link_id) = match segment {
                crate::text::RichSegment::Text { url, link_id, .. }
                | crate::text::RichSegment::Image { url, link_id, .. } => (url.as_deref(), link_id),
            };
            let group = link_id.unwrap_or(segment_index);
            match segment {
                crate::text::RichSegment::Text {
                    text,
                    color,
                    size,
                    bold,
                    italic,
                    underline,
                    strikethrough,
                    stroke,
                    stroke_color,
                    ..
                } => {
                    let mut span = parent.spawn((
                        TextSpan::new(text.clone()),
                        crate::text_stroke::TextStrokeSpan {
                            width: stroke.map(|w| w as f32),
                            color: *stroke_color,
                        },
                        appearance::TextTintSource {
                            owner: object,
                            color: *color,
                        },
                        bevy::picking::Pickable {
                            should_block_lower: false,
                            is_hoverable: true,
                        },
                    ));
                    span.insert((
                        TextColor(backend::to_bevy_color(color.unwrap_or(data.color))),
                        TextFont {
                            font: raster.clone(),
                            ..backend::text_font_for_span(data, font, *size, *bold, *italic)
                        },
                        backend::text_line_height_for(data, *size),
                        backend::text_letter_spacing_for(data),
                    ));
                    spans.push(span.id());
                    if let Some(url) = url {
                        links.push((span.id(), group, url.to_owned()));
                    }
                    if *strikethrough {
                        span.insert(bevy::text::Strikethrough);
                    }
                    if data.underline || *underline {
                        span.insert(Underline);
                    }
                }
                crate::text::RichSegment::Image {
                    source, dimensions, ..
                } => {
                    image_sources.push((
                        source.clone(),
                        image_identity(asset, assets, asset_server, image_assets, source),
                    ));
                    // 内联图片：支持标准 ui:// 地址和裸条目名。
                    match inline_image(
                        asset,
                        assets,
                        asset_server,
                        image_assets,
                        source,
                        *dimensions,
                    ) {
                        Some(source) => {
                            // Reserve the original canvas with a real Parley inline
                            // box. The atlas quad is separate so trimming does not
                            // change the paragraph's advance or line height.
                            let inline_box = parent
                                .spawn((
                                    InlineBox {
                                        kind: InlineBoxKind::InFlow,
                                        size: source.canvas,
                                    },
                                    bevy::picking::Pickable::IGNORE,
                                ))
                                .id();
                            spans.push(inline_box);
                            if let Some(url) = url {
                                links.push((inline_box, group, url.to_owned()));
                            }
                            let movie = source.movie;
                            let mut image = parent.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    width: Val::Px(source.size.x),
                                    height: Val::Px(source.size.y),
                                    display: Display::None,
                                    ..default()
                                },
                                source.image,
                                appearance::TextTintSource {
                                    owner: object,
                                    color: Some(fairygui_core::color::Color::WHITE),
                                },
                                bevy::picking::Pickable::IGNORE,
                                InlineImage {
                                    inline_box,
                                    offset: source.offset,
                                    size: source.size,
                                },
                                InlineImageOwner(text_entity),
                                ZIndex::default(),
                            ));
                            if source.pending {
                                image.insert(InlineImagePending);
                            }
                            if let Some(player) = movie {
                                image.insert(InlineMovieClipPlayer::new(object, player));
                            }
                            let image = image.id();
                            images.push(image);
                        }
                        None => tracing::debug!(source, "富文本内联图片未找到对应条目，已跳过"),
                    }
                }
            }
        }
    });
    entity.insert(RichTextBlock {
        source: text.into(),
        data: data.clone(),
        font: font.cloned(),
        raster_font: raster,
        package_id: asset.package.id().into(),
        res_key: asset.res_key.clone(),
        spans,
        image_sources,
        links,
    });
    images
}

/// Match the current text layout in the same frame. ImageNode remains a sibling
/// for atlas slicing; its final computed quad is derived from the text transform,
/// as Bevy's native inline-image extractor does. Run before UI clipping.
#[allow(clippy::type_complexity)]
pub(crate) fn finalize_inline_images(
    text: Query<
        (
            &bevy::text::TextLayoutInfo,
            &ComputedNode,
            &UiGlobalTransform,
        ),
        (With<RichTextBlock>, Without<InlineImage>),
    >,
    mut images: Query<
        (
            &InlineImageOwner,
            &InlineImage,
            &Node,
            &mut ComputedNode,
            &mut UiGlobalTransform,
        ),
        Without<RichTextBlock>,
    >,
) {
    for (owner, image, node, mut computed, mut global) in &mut images {
        let geometry = text
            .get(owner.0)
            .ok()
            .and_then(|(layout, text_node, text_global)| {
                if node.display == Display::None {
                    return None;
                }
                let (_, _, bounds) = layout.inline_boxes.iter().find(|(entity, kind, _)| {
                    *entity == image.inline_box && *kind == InlineBoxKind::InFlow
                })?;
                let scale = layout.scale_factor;
                let center = text_node.content_box().min
                    + bounds.min
                    + (image.offset + image.size * 0.5) * scale;
                Some((
                    image.size * scale,
                    text_node.inverse_scale_factor(),
                    UiGlobalTransform::from(
                        bevy::math::Affine2::from(text_global)
                            * bevy::math::Affine2::from_translation(center),
                    ),
                ))
            });
        let Some((size, inverse_scale_factor, transform)) = geometry else {
            if computed.size != Vec2::ZERO {
                computed.size = Vec2::ZERO;
            }
            continue;
        };
        if computed.size != size {
            computed.size = size;
        }
        if computed.unrounded_size != size {
            computed.unrounded_size = size;
        }
        if computed.content_size != size {
            computed.content_size = size;
        }
        if computed.inverse_scale_factor != inverse_scale_factor {
            computed.inverse_scale_factor = inverse_scale_factor;
        }
        if *global != transform {
            *global = transform;
        }
    }
}

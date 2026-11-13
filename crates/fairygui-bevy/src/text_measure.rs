//! Synchronous text sizing before native layout and view materialization.
use super::*;
use bevy::text::{FontCx, LayoutCx, TextBounds, TextElement, TextPipeline};
use std::collections::HashSet;

#[cfg(test)]
#[path = "text_measure_tests.rs"]
mod tests;

#[derive(Default)]
pub(crate) struct TextMeasurer {
    pipeline: TextPipeline,
    font_cx: FontCx,
    layout_cx: LayoutCx,
    fonts: Assets<Font>,
    font_handles: HashMap<bevy::asset::AssetId<Font>, Handle<Font>>,
    packages: Assets<FuiAsset>,
    images: HashMap<bevy::asset::AssetId<Image>, UVec2>,
    server: Option<AssetServer>,
    default_font: Option<Handle<Font>>,
    scale_factor: f32,
    requested: HashSet<GObjectId>,
    translations: fairygui_core::translation::TranslationTable,
    revision: u64,
    cache: HashMap<GObjectId, Cached>,
}

#[derive(PartialEq)]
struct Key {
    data: fairygui_core::widget::TextFieldData,
    text: String,
    width: Option<f32>,
    max: f32,
    revision: u64,
    font: Option<Handle<Font>>,
}
struct Cached {
    key: Key,
    size: Vec2,
}

impl TextMeasurer {
    /// Prepare native text geometry before its parent reads row/group bounds.
    pub(crate) fn ensure_subtree(&mut self, store: &mut ObjectStore, root: GObjectId) {
        // Low-level runtime users may materialize views without installing the
        // plugin's asset service. Their bitmap projection still measures itself.
        if self.server.is_none() {
            return;
        }
        let mut objects = vec![root];
        store.for_each_descendant(root, &mut |id| objects.push(id));
        for object in objects.into_iter().rev() {
            self.ensure(store, object);
        }
    }

    pub(crate) fn contains(&self, id: GObjectId) -> bool {
        self.requested.contains(&id)
    }

    /// Fixed/shrink/ellipsis fields keep their exported size. Pending fonts leave
    /// the current geometry intact and are retried once their assets arrive.
    pub(crate) fn ensure(&mut self, store: &mut ObjectStore, id: GObjectId) -> bool {
        let Some(object) = store.object(id) else {
            return false;
        };
        let Some(data) = object.control().as_text_field() else {
            return false;
        };
        if !matches!(data.auto_size, 1 | 2) {
            self.requested.remove(&id);
            self.cache.remove(&id);
            return false;
        }
        self.requested.insert(id);
        let text = text_fonts::display_text(object, &self.translations.translate(&data.text));
        let key = Key {
            data: data.clone(),
            text: text.clone(),
            width: (data.auto_size == 2).then_some(object.width()),
            max: object.max_size().width,
            revision: self.revision,
            font: self.default_font.clone(),
        };
        let size = if let Some(cached) = self.cache.get(&id).filter(|cached| cached.key == key) {
            cached.size
        } else {
            let Some(size) = self.measure(object, &text) else {
                return false;
            };
            self.cache.insert(id, Cached { key, size });
            size
        };
        if !size.is_finite() || size.min_element() < 0. {
            return false;
        }
        if (size - Vec2::new(object.width(), object.height()))
            .abs()
            .max_element()
            > 0.01
        {
            store.set_size(id, size.x, size.y);
        }
        true
    }

    fn image_size(
        &self,
        own: &FuiAsset,
        source: &str,
        dimensions: fairygui_core::text::ImageDimensions,
    ) -> Option<Vec2> {
        if let Some((asset, index)) = rich_text::image_source(own, &self.packages, source) {
            let item = asset.package.item(index)?;
            return if item.item_type == fairygui_core::package::PackageItemType::MovieClip {
                Some(Vec2::new(item.width.max(0.), item.height.max(0.)))
            } else {
                let sprite = asset.package.sprite_for(index)?;
                Some(Vec2::new(sprite.original_size.x, sprite.original_size.y))
            };
        }
        if source.is_empty() || source.starts_with("ui://") {
            return None;
        }
        let server = self.server.as_ref()?;
        let path = bevy::asset::AssetPath::try_parse(source).ok()?.into_owned();
        let handle = server
            .get_handle::<Image>(path.clone())
            .unwrap_or_else(|| server.load(path));
        let natural = self
            .images
            .get(&handle.id())
            .map(|size| fairygui_core::Size::new(size.x as f32, size.y as f32));
        let size = dimensions.resolve(natural);
        Some(Vec2::new(size.width.max(0.), size.height.max(0.)))
    }

    fn font_source(&self, source: bevy::text::FontSource) -> Option<bevy::text::FontSource> {
        use bevy::text::FontSource;
        Some(match source {
            FontSource::Handle(handle) => {
                FontSource::Handle(self.font_handles.get(&handle.id())?.clone())
            }
            FontSource::List(sources) => FontSource::List(
                sources
                    .into_iter()
                    .filter_map(|source| self.font_source(source))
                    .collect(),
            ),
            source => source,
        })
    }

    fn measure(&mut self, object: &fairygui_core::GObject, text: &str) -> Option<Vec2> {
        let data = object.control().as_text_field()?;
        let asset = self.packages.iter().find_map(|(_, asset)| {
            (Some(asset.package.id()) == object.source_package_id()).then_some(asset)
        });
        let parsed = if data.ubb_enabled || data.html_enabled {
            crate::text::parse_for(data, text)
        } else {
            fairygui_core::text::UbbDocument {
                spans: vec![fairygui_core::text::UbbSpan {
                    text: text.into(),
                    ..default()
                }],
            }
        };
        // Bitmap measurement shares the same native layout used to draw glyphs.
        if let Some((_, font)) = asset.and_then(|asset| {
            data.font
                .as_deref()
                .and_then(|name| text_fonts::resolve_bitmap_font(asset, &self.packages, name))
        }) {
            if !data.ubb_enabled && !data.html_enabled {
                let result = fairygui_core::bitmap_text::layout_bitmap_text(
                    &font,
                    data,
                    text,
                    object.size(),
                );
                return Some(Vec2::new(
                    result.object_size.width,
                    result.object_size.height,
                ));
            }
            let elements: Vec<_> = parsed
                .spans
                .iter()
                .filter_map(|span| {
                    let kind = if let Some(source) = &span.image {
                        let size = self.image_size(asset?, source, span.image_dimensions)?;
                        fairygui_core::bitmap_text::BitmapElementKind::Image(
                            fairygui_core::Size::new(size.x, size.y),
                        )
                    } else {
                        fairygui_core::bitmap_text::BitmapElementKind::Text(span.text.clone())
                    };
                    Some(fairygui_core::bitmap_text::BitmapElement {
                        kind,
                        font_size: span.size.unwrap_or(data.font_size),
                    })
                })
                .collect();
            let result = fairygui_core::bitmap_text::layout_bitmap_rich_text(
                &font,
                data,
                &elements,
                object.size(),
            );
            return Some(Vec2::new(
                result.object_size.width,
                result.object_size.height,
            ));
        }
        let server = self.server.as_ref()?;
        let (raster, _) = text_fonts::raster_font_with_presence(
            data.font.as_deref(),
            server,
            self.default_font.as_ref(),
            |handle| self.font_handles.contains_key(&handle.id()),
        );
        let raster = self.font_source(raster)?;
        let fonts: Vec<_> = parsed
            .spans
            .iter()
            .map(|span| TextFont {
                font: raster.clone(),
                ..backend::text_font_for_span(data, None, span.size, span.bold, span.italic)
            })
            .collect();
        let boxes: Vec<_> = parsed
            .spans
            .iter()
            .map(|span| {
                let source = span.image.as_ref()?;
                let size = self.image_size(asset?, source, span.image_dimensions)?;
                Some(bevy::text::InlineBox {
                    kind: bevy::text::InlineBoxKind::InFlow,
                    size: size + Vec2::new(if size.x > 0. { 2. } else { 0. }, 0.),
                })
            })
            .collect();
        let spans = parsed.spans.iter().enumerate().filter_map(|(index, span)| {
            let element = if span.image.is_some() {
                TextElement::Box(boxes[index].as_ref()?)
            } else {
                TextElement::Text {
                    text: &span.text,
                    font: &fonts[index],
                    color: Color::WHITE,
                    line_height: backend::text_line_height_for(data, span.size),
                    letter_spacing: backend::text_letter_spacing_for(data),
                }
            };
            Some((Entity::from_raw_u32(index as u32).unwrap(), 0, element))
        });
        let max = object.max_size().width;
        let width = if data.single_line {
            None
        } else if data.auto_size == 2 {
            Some((object.width() - 4.).max(0.))
        } else {
            (max > 0.).then_some((max - 4.).max(0.))
        };
        let layout = backend::text_layout_for_object(object, data);
        let mut computed = ComputedTextBlock::default();
        // Parley quantizes glyph advances at the render target's scale. Shaping
        // at 1x and scaling the result can make a short line wrap at fractional
        // DPI, even with the same font. Match rendering, then round outwards.
        self.pipeline
            .update_buffer(
                &self.fonts,
                spans,
                layout.linebreak,
                layout.justify,
                TextBounds {
                    width: width.map(|width| width * self.scale_factor.max(f32::EPSILON)),
                    height: None,
                },
                self.scale_factor.max(f32::EPSILON),
                &mut computed,
                &mut self.font_cx,
                &mut self.layout_cx,
                Vec2::ZERO,
                bevy::text::RemSize::default(),
            )
            .ok()?;
        let buffer = computed.buffer();
        let measured = if text.is_empty() {
            Vec2::new(0., 0.)
        } else {
            (Vec2::new(buffer.full_width(), buffer.height()).ceil()
                / self.scale_factor.max(f32::EPSILON))
            .ceil()
                + Vec2::splat(4.)
        };
        Some(Vec2::new(
            if data.auto_size == 1 {
                measured.x
            } else {
                object.width()
            },
            measured.y,
        ))
    }
}

/// Capture cheap font/package references and image dimensions, never image pixels.
#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_assets(
    runtime: NonSend<GuiRuntime>,
    fonts: Option<Res<Assets<Font>>>,
    packages: Res<Assets<FuiAsset>>,
    images: Option<Res<Assets<Image>>>,
    server: Res<AssetServer>,
    windows: Query<(Entity, &Window, Has<bevy::window::PrimaryWindow>)>,
    settings: Res<crate::GuiInputSettings>,
    ui_scale: Option<Res<bevy::ui::UiScale>>,
) {
    let mut measure = runtime.text_measurer.borrow_mut();
    measure.server = Some(server.clone());
    measure.default_font = runtime.font.clone();
    measure.translations = runtime.translations.clone();
    let scale = windows
        .iter()
        .find(|(entity, _, primary)| settings.window.map_or(*primary, |target| target == *entity))
        .map_or(1., |(_, window, _)| window.scale_factor())
        * ui_scale.map_or(1., |scale| scale.0);
    if measure.scale_factor != scale {
        measure.scale_factor = scale;
        measure.revision = measure.revision.wrapping_add(1);
    }
    if let Some(fonts) = fonts.filter(|fonts| fonts.is_changed()) {
        measure.fonts = Assets::default();
        measure.font_handles.clear();
        measure.font_cx = FontCx::default();
        measure.revision = measure.revision.wrapping_add(1);
        for (id, font) in fonts.iter() {
            let mut font = font.clone();
            let handle = measure.fonts.add(font.clone());
            font.alias = format!("asset_id:{:?}", handle.id());
            measure
                .font_cx
                .collection
                .register_fonts(font.data.clone(), None);
            measure.font_cx.collection.register_fonts(
                font.data.clone(),
                Some(parley::fontique::FontInfoOverride {
                    family_name: Some(&font.alias),
                    ..default()
                }),
            );
            *measure.fonts.get_mut(&handle).unwrap() = font;
            measure.font_handles.insert(id, handle);
        }
    }
    if packages.is_changed() {
        measure.packages = Assets::default();
        measure.revision = measure.revision.wrapping_add(1);
        for (_, package) in packages.iter() {
            measure.packages.add(FuiAsset {
                package: package.package.clone(),
                res_key: package.res_key.clone(),
            });
        }
    }
    if let Some(images) = images.filter(|images| images.is_changed()) {
        let next = images
            .iter()
            .map(|(id, image)| (id, image.size()))
            .collect();
        if measure.images != next {
            measure.images = next;
            measure.revision = measure.revision.wrapping_add(1);
        }
    }
    // Keep the service independent of Bevy's post-layout physical pixel data.
    measure.requested.retain(|&id| runtime.store.contains(id));
    measure.cache.retain(|&id, _| runtime.store.contains(id));
}

pub(crate) fn sync_sizes(mut runtime: NonSendMut<GuiRuntime>) {
    let service = runtime.text_measurer.clone();
    let mut measure = service.borrow_mut();
    let ids: Vec<_> = runtime
        .store
        .iter()
        .filter_map(|(id, object)| {
            (measure.contains(id)
                || object
                    .control()
                    .as_text_field()
                    .is_some_and(|data| matches!(data.auto_size, 1 | 2)))
            .then_some(id)
        })
        .collect();
    let mut lists = HashSet::new();
    for id in ids {
        let before = runtime.store.object(id).map(|object| object.size());
        measure.ensure(&mut runtime.store, id);
        if before != runtime.store.object(id).map(|object| object.size()) {
            let mut parent = runtime.store.object(id).and_then(|object| object.parent());
            while let Some(id) = parent {
                let object = runtime.store.object(id).unwrap();
                if matches!(
                    object.control(),
                    ControlData::List(_) | ControlData::Tree(_)
                ) {
                    lists.insert(id);
                }
                parent = object.parent();
            }
        }
    }
    for list in lists {
        if runtime.store.is_virtual_list(list) {
            runtime.store.invalidate_virtual_list(list, false);
        } else {
            runtime.store.layout_list(list);
        }
    }
}

pub(crate) fn sync_padding(
    runtime: NonSend<GuiRuntime>,
    mut nodes: Query<(&GObjectRef, &mut Node), With<Text>>,
) {
    let measure = runtime.text_measurer.borrow();
    for (object, mut node) in &mut nodes {
        if measure.contains(object.0) && node.padding == UiRect::ZERO {
            node.padding = UiRect::all(Val::Px(2.));
        }
    }
}

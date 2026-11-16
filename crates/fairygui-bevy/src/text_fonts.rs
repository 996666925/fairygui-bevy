//! Select and replace text renderers while keeping the logical object's view.
use super::*;
use fairygui_core::package::{BitmapFontData, PackageItemType};

#[cfg(test)]
#[path = "text_font_tests.rs"]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq)]
struct BitmapSource {
    package_id: String,
    res_key: String,
    item: usize,
}

#[derive(Component, Debug, Clone, PartialEq)]
pub(crate) struct TextFontBinding {
    requested: Option<String>,
    bitmap: Option<BitmapSource>,
    rich: bool,
    default_font: Option<Handle<Font>>,
    raster: bevy::text::FontSource,
    request: Option<Handle<Font>>,
    editing: bool,
}

impl TextFontBinding {
    pub(super) fn raster(&self) -> &bevy::text::FontSource {
        &self.raster
    }

    pub(super) fn editing(&self) -> bool {
        self.editing
    }

    fn for_input_focus(mut self, editing: bool) -> Self {
        self.editing = editing && (self.bitmap.is_some() || self.rich);
        if self.editing {
            self.bitmap = None;
            self.rich = false;
        }
        self
    }
}

/// A materialized native input can acquire focus before its canvas renderer is
/// replaced by the plain editing surface.
#[derive(Component, Debug)]
pub(crate) struct InputPresentation;

#[derive(Component)]
struct SuspendedInputEditor(bevy::text::EditableText);

fn prepare_editor(commands: &mut Commands, entity: Entity, source: &str) {
    let source = source.to_owned();
    commands.queue(move |world: &mut World| {
        if world.get::<bevy::text::EditableText>(entity).is_none() {
            let editor = world
                .entity_mut(entity)
                .take::<SuspendedInputEditor>()
                .map_or_else(|| bevy::text::EditableText::new(&source), |saved| saved.0);
            world.entity_mut(entity).insert(editor);
        }
        if let Some(mut editor) = world.get_mut::<bevy::text::EditableText>(entity) {
            if editor.editor.raw_text() != source {
                editor.editor.set_text(&source);
            }
            // A new TextLayoutInfo needs glyphs even when the editor's text and
            // selected range have not changed across a renderer replacement.
            let scale = editor.editor.get_scale();
            editor.editor.set_scale(scale);
        }
    });
}

fn raster_font(
    name: Option<&str>,
    server: &AssetServer,
    fonts: Option<&Assets<Font>>,
    default: Option<&Handle<Font>>,
) -> (bevy::text::FontSource, Option<Handle<Font>>) {
    raster_font_with_presence(name, server, default, |handle| {
        fonts.is_some_and(|fonts| fonts.contains(handle))
    })
}

pub(super) fn raster_font_with_presence(
    name: Option<&str>,
    server: &AssetServer,
    default: Option<&Handle<Font>>,
    is_loaded: impl Fn(&Handle<Font>) -> bool,
) -> (bevy::text::FontSource, Option<Handle<Font>>) {
    use bevy::text::FontSource;
    let fallback = default.cloned().map(FontSource::Handle).unwrap_or_default();
    let Some(name) = name.filter(|name| !name.is_empty() && !name.starts_with("ui://")) else {
        return (fallback, None);
    };
    if fairygui_core::text::is_font_resource(name) {
        let Ok(path) = bevy::asset::AssetPath::try_parse(name) else {
            return (fallback, None);
        };
        let path = path.into_owned();
        let handle = server
            .get_handle::<Font>(path.clone())
            .unwrap_or_else(|| server.load(path));
        let source = if is_loaded(&handle) {
            FontSource::Handle(handle.clone())
        } else {
            fallback
        };
        (source, Some(handle))
    } else {
        (
            FontSource::list([FontSource::families(name), fallback]),
            None,
        )
    }
}

/// Bare names remain local. An explicit URL selects only its declared package;
/// glyph item indices and atlas paths must be interpreted in that same package.
fn font_source<'a>(
    asset: &'a FuiAsset,
    assets: &'a Assets<FuiAsset>,
    name: &str,
) -> Option<(&'a FuiAsset, usize)> {
    let (source, index) = if name.starts_with("ui://") {
        if let Some(index) = asset.package.resolve_local_url(name) {
            (asset, index)
        } else {
            assets.iter().find_map(|(_, source)| {
                source
                    .package
                    .resolve_local_url(name)
                    .map(|index| (source, index))
            })?
        }
    } else {
        (asset, asset.package.item_index_by_name(name)?)
    };
    (source.package.item(index)?.item_type == PackageItemType::Font).then_some((source, index))
}

pub(super) fn resolve_bitmap_font<'a>(
    asset: &'a FuiAsset,
    assets: &'a Assets<FuiAsset>,
    name: &str,
) -> Option<(&'a FuiAsset, BitmapFontData)> {
    let (source, index) = font_source(asset, assets, name)?;
    let font = source
        .package
        .load_bitmap_font(index, &fairygui_core::package::LoadContext::none())
        .ok()?;
    Some((source, font))
}

fn binding(
    object: &fairygui_core::GObject,
    asset: &FuiAsset,
    assets: &Assets<FuiAsset>,
    font: Option<&Handle<Font>>,
    text: &str,
    server: &AssetServer,
    fonts: Option<&Assets<Font>>,
) -> TextFontBinding {
    let data = object.control().as_text_field().unwrap();
    let text = template_text(object, text);
    let text = text.as_str();
    let bitmap = data
        .font
        .as_deref()
        .and_then(|name| font_source(asset, assets, name))
        .map(|(source, item)| BitmapSource {
            package_id: source.package.id().into(),
            res_key: source.res_key.clone(),
            item,
        });
    let (raster, request) = raster_font(
        data.font.as_deref().filter(|_| bitmap.is_none()),
        server,
        fonts,
        font,
    );
    TextFontBinding {
        requested: data.font.clone(),
        rich: (data.ubb_enabled || data.html_enabled)
            && (bitmap.is_some()
                || !matches!(object.control(), ControlData::TextInput(_))
                || matches!(object.control(), ControlData::TextInput(input) if input.password)
                    && data.ubb_enabled
                    && fairygui_core::text::ubb_to_html(
                        &text.replace("\r\n", "\n"),
                        &fairygui_core::text::MarkupOptions::default(),
                    ) != text.replace("\r\n", "\n")
                || needs_rich_input(data, text)),
        bitmap,
        default_font: font.cloned(),
        raster,
        request,
        editing: false,
    }
}

/// Core values stay unmasked; only renderer sources receive this projection.
pub(super) fn display_text(object: &fairygui_core::GObject, text: &str) -> String {
    let text = template_text(object, text);
    if let ControlData::TextInput(input) = object.control()
        && input.password
    {
        fairygui_core::text::input::password_text(&text, input.text_field.ubb_enabled)
    } else {
        text
    }
}

pub(crate) fn template_text(object: &fairygui_core::GObject, text: &str) -> String {
    object
        .control()
        .as_text_field()
        .and_then(|data| data.template_vars.as_ref())
        .map_or_else(
            || text.to_owned(),
            |variables| fairygui_core::text::expand_template(text, variables),
        )
}

// Preserve the existing styled-input path. Plain UBB-enabled inputs still use
// EditableText, while actual multi-span/styles/images require a rich paragraph.
fn needs_rich_input(data: &fairygui_core::widget::TextFieldData, text: &str) -> bool {
    let segments = crate::text::layout_for(data, text);
    segments.len() > 1
        || segments.iter().any(|segment| match segment {
            crate::text::RichSegment::Image { .. } => true,
            crate::text::RichSegment::LinkStart { .. } | crate::text::RichSegment::LinkEnd => true,
            crate::text::RichSegment::Text {
                size,
                bold,
                italic,
                underline,
                strikethrough,
                stroke,
                stroke_color,
                url,
                ..
            } => {
                size.is_some()
                    || *bold
                    || *italic
                    || *underline
                    || *strikethrough
                    || url.is_some()
                    || stroke.is_some()
                    || stroke_color.is_some()
            }
        })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_text(
    commands: &mut Commands,
    entity: Entity,
    owner: GObjectId,
    object: &fairygui_core::GObject,
    asset: &FuiAsset,
    assets: &Assets<FuiAsset>,
    asset_server: &AssetServer,
    image_assets: &Assets<Image>,
    font: Option<&Handle<Font>>,
    text: &str,
    editing: bool,
) -> Vec<Entity> {
    let data = object.control().as_text_field().unwrap();
    let input_text = matches!(object.control(), ControlData::TextInput(_))
        .then(|| fairygui_core::text::input::normalize_input_lines(text, !data.single_line));
    let text = input_text.as_deref().unwrap_or(text);
    let selected = binding(object, asset, assets, font, text, asset_server, None)
        .for_input_focus(editing && matches!(object.control(), ControlData::TextInput(_)));
    if matches!(object.control(), ControlData::TextInput(_)) {
        commands.entity(entity).insert(InputPresentation);
    }
    let display = display_text(object, text);
    let source = template_text(object, text);
    let text = display.as_str();
    if !selected.editing
        && let Some((source, bitmap)) = data
            .font
            .as_deref()
            .and_then(|name| resolve_bitmap_font(asset, assets, name))
    {
        if data.ubb_enabled || data.html_enabled {
            bitmap_rich_text::spawn(
                &mut *commands,
                entity,
                owner,
                object,
                asset,
                source,
                assets,
                asset_server,
                image_assets,
                &bitmap,
                text,
            );
            commands.entity(entity).insert(selected);
            return Vec::new();
        }
        let (glyphs, metrics, underlines) = bitmap_layout(&bitmap, data, text, object.size());
        let decorations = underlines
            .into_iter()
            .map(|rect| spawn_text_decoration(commands, entity, owner, rect, None))
            .collect();
        let parts = glyphs
            .iter()
            .filter_map(|glyph| spawn_glyph(commands, entity, owner, glyph, source, asset_server))
            .collect();
        commands.entity(entity).insert((
            BitmapGlyphs(parts),
            BitmapDecorations(decorations),
            metrics,
            selected,
        ));
        return Vec::new();
    }
    // Keep the attempted source identity even when parsing fails, so invalid
    // metadata cannot trigger an identical renderer rebuild every frame.
    let mut root = commands.entity(entity);
    root.insert((
        TextFont {
            font: selected.raster.clone(),
            ..backend::text_font_for(object, data, font)
        },
        backend::text_layout_for_object(object, data),
        backend::text_line_height_for(data, None),
        crate::runtime::text_measure::fit::FontMetrics::new(data, None),
        backend::text_letter_spacing_for(data),
    ));
    if selected.rich {
        root.insert(Text::new(String::new()));
        let segments = crate::text::layout_for(data, text);
        let images = rich_text::spawn_rich_content(
            &mut root,
            owner,
            data,
            text,
            &segments,
            asset,
            assets,
            asset_server,
            image_assets,
            font,
            Some(&selected.raster),
        );
        root.insert(selected);
        images
    } else {
        root.insert((
            Text::new(text),
            TextColor(backend::to_bevy_color(data.color)),
            selected,
        ));
        if data.underline {
            root.insert(Underline);
        }
        if let ControlData::TextInput(input) = object.control() {
            root.insert((
                bevy::text::TextCursorStyle {
                    color: backend::to_bevy_color(data.color),
                    ..default()
                },
                InputEditPolicy::new(input),
                InputTextState {
                    source: data.text.clone(),
                    display: source.to_owned(),
                },
            ));
            // Text mirrors the native value and supplies extraction metadata,
            // while EditableText owns glyph/caret/selection geometry. Keeping
            // the static layout flags lets text_system clear the editor's
            // selection_rects whenever the node or text style changes.
            root.remove::<bevy::ui::widget::TextNodeFlags>();
            prepare_editor(commands, entity, &source);
        }
        Vec::new()
    }
}

/// Recheck selection independently of the dirty queue, including a requested
/// foreign font whose package arrived later. Obsolete requests cannot win.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn sync_text_fonts(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    assets: Res<Assets<FuiAsset>>,
    asset_server: Res<AssetServer>,
    image_assets: Res<Assets<Image>>,
    fonts: Res<Assets<Font>>,
    decorations: Query<&BitmapDecorations>,
    mut query: Query<(
        Entity,
        &GObjectRef,
        &TextFontBinding,
        &mut Node,
        Option<&BitmapGlyphs>,
        Option<&RichTextBlock>,
        Option<&InlineImageParts>,
        Option<&BitmapRichText>,
    )>,
) {
    for (entity, owner, previous, mut node, glyphs, rich, images, bitmap_rich) in &mut query {
        let Some(object) = runtime.store().object(owner.0) else {
            continue;
        };
        let Some(handle) = runtime.asset_handle_for(owner.0) else {
            continue;
        };
        let Some(asset) = assets.get(&handle) else {
            continue;
        };
        let text = runtime.translate(&object.control().as_text_field().unwrap().text);
        let next = binding(
            object,
            asset,
            &assets,
            runtime.font.as_ref(),
            &text,
            &asset_server,
            Some(&fonts),
        )
        .for_input_focus(runtime.focused_input() == Some(owner.0));
        if *previous == next {
            continue;
        }
        if previous.bitmap == next.bitmap && previous.rich == next.rich {
            // Aliases/default-font changes do not require a new content tree.
            // In particular, preserve an input's cursor/selection state.
            if next.bitmap.is_none() {
                let data = object.control().as_text_field().unwrap();
                commands.entity(entity).insert(TextFont {
                    font: next.raster.clone(),
                    ..backend::text_font_for(object, data, runtime.font.as_ref())
                });
            }
            commands.entity(entity).insert(next);
            continue;
        }
        if let Some(glyphs) = glyphs {
            for &glyph in &glyphs.0 {
                commands.entity(glyph).despawn();
            }
        }
        if let Ok(parts) = decorations.get(entity) {
            for &part in &parts.0 {
                commands.entity(part).despawn();
            }
        }
        if let Some(rich) = rich {
            rich.despawn_spans(&mut commands);
        }
        if let Some(rich) = bitmap_rich {
            rich.despawn_parts(&mut commands);
        }
        if let Some(images) = images {
            for image in images.iter() {
                commands.entity(image).despawn();
            }
        }
        // Bevy required components outlive removal of Text. Reclaim paragraph
        // geometry/measure state too, or old raster glyphs and intrinsic sizing
        // can remain active beside the new bitmap quads.
        let suspend = matches!(object.control(), ControlData::TextInput(_))
            && (next.bitmap.is_some() || next.rich);
        if suspend {
            commands.queue(move |world: &mut World| {
                if let Some(editor) = world.entity_mut(entity).take::<bevy::text::EditableText>() {
                    world
                        .entity_mut(entity)
                        .insert(SuspendedInputEditor(editor));
                }
            });
        }
        commands
            .entity(entity)
            .remove::<(
                Text,
                TextFont,
                TextColor,
                TextLayout,
                bevy::text::LineHeight,
                bevy::text::LetterSpacing,
                Underline,
                bevy::text::TextCursorStyle,
            )>()
            .remove::<(
                ComputedTextBlock,
                bevy::text::TextLayoutInfo,
                bevy::ui::widget::TextNodeFlags,
                FontHinting,
                BitmapGlyphs,
                BitmapDecorations,
                BitmapTextMetrics,
                RichTextBlock,
                BitmapRichText,
                crate::runtime::text_measure::fit::FontMetrics,
                crate::runtime::text_measure::fit::RasterFit,
                crate::runtime::text_measure::ellipsis::EllipsisState,
            )>()
            // Bevy's layout query requires ContentSize even for bitmap nodes.
            // Reset the old text measure without removing the node from layout.
            .insert(bevy::ui::ContentSize::default());
        node.padding = UiRect::ZERO;
        let images = spawn_text(
            &mut commands,
            entity,
            owner.0,
            object,
            asset,
            &assets,
            &asset_server,
            &image_assets,
            runtime.font.as_ref(),
            &text,
            runtime.focused_input() == Some(owner.0),
        );
        let parent = object.parent().and_then(|id| runtime.entity_of(id));
        for image in images {
            if let Some(parent) = parent {
                commands.entity(image).insert(ChildOf(parent));
            } else {
                commands.entity(image).remove::<ChildOf>();
            }
        }
    }
}

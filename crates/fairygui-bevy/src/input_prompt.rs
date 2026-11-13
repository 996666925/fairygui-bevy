//! Owned prompt views; prompt text never enters the model or editing buffer.
use super::*;

/// Logical input whose empty-value prompt is rendered by this decorative node.
#[derive(Component, Debug, Clone, Copy)]
pub struct InputPromptOwner(pub GObjectId);

#[derive(Component, Debug)]
pub(crate) struct InputPromptView(Entity);

#[derive(Component, Debug, Clone, PartialEq)]
pub(crate) struct PromptSnapshot {
    text: String,
    data: fairygui_core::widget::TextFieldData,
    color: fairygui_core::color::Color,
    size: fairygui_core::Size,
    font: bevy::text::FontSource,
    bitmap: Option<(String, String)>,
}

#[cfg(test)]
#[path = "input_prompt_tests.rs"]
mod tests;

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn sync_input_prompts(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    assets: Res<Assets<FuiAsset>>,
    server: Res<AssetServer>,
    owners: Query<
        (
            Entity,
            &GObjectRef,
            Option<&bevy::text::EditableText>,
            Option<&text_fonts::TextFontBinding>,
            Option<&InputPromptView>,
        ),
        Without<InputPromptOwner>,
    >,
    mut prompts: Query<
        (
            &PromptSnapshot,
            &mut Visibility,
            &mut Node,
            Option<&bevy::text::TextLayoutInfo>,
        ),
        With<InputPromptOwner>,
    >,
) {
    for (entity, owner, editor, font, previous) in &owners {
        let Some(object) = runtime.store().object(owner.0) else {
            continue;
        };
        let ControlData::TextInput(input) = object.control() else {
            continue;
        };
        let text = input
            .prompt_text
            .as_deref()
            .map(fairygui_core::text::strip_ubb_tags)
            .map(|text| runtime.translate(&text).replace("\r\n", "\n"))
            .unwrap_or_default();
        if text.is_empty() {
            if let Some(previous) = previous {
                commands.entity(previous.0).despawn();
                commands.entity(entity).remove::<InputPromptView>();
            }
            continue;
        }
        let empty = editor.map_or_else(
            || runtime.translate(&input.text_field.text).is_empty(),
            |editor| editor.value().to_string().is_empty() && !editor.is_composing(),
        );
        let visible = if empty {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        let own = runtime
            .asset_handle_for(owner.0)
            .and_then(|h| assets.get(&h));
        let bitmap = own.and_then(|asset| {
            input
                .text_field
                .font
                .as_deref()
                .and_then(|name| text_fonts::resolve_bitmap_font(asset, &assets, name))
        });
        let mut data = input.text_field.clone();
        data.text.clear();
        let snapshot = PromptSnapshot {
            text,
            data,
            color: input.prompt_color,
            size: object.size(),
            font: font
                .map(|binding| binding.raster().clone())
                .unwrap_or_else(|| {
                    runtime
                        .font
                        .clone()
                        .map(bevy::text::FontSource::Handle)
                        .unwrap_or_default()
                }),
            bitmap: bitmap
                .as_ref()
                .map(|(asset, _)| (asset.package.id().to_owned(), asset.res_key.clone())),
        };
        // Absolute children start at the owner's padding-box edge. Its content
        // padding must not displace the prompt, which aligns independently.
        let left = 0.;
        let top = 0.;
        if let Some(previous) = previous
            && let Ok((old, mut visibility, mut node, layout)) = prompts.get_mut(previous.0)
            && *old == snapshot
        {
            if *visibility != visible {
                *visibility = visible;
            }
            if node.left != Val::Px(left) {
                node.left = Val::Px(left);
            }
            if node.top != Val::Px(top) {
                node.top = Val::Px(top);
            }
            if snapshot.bitmap.is_none()
                && let Some(layout) = layout
            {
                let scale = if layout.scale_factor.is_finite() && layout.scale_factor > f32::EPSILON
                {
                    layout.scale_factor
                } else {
                    1.
                };
                let gutter = if runtime.text_measurer.borrow().contains(owner.0) {
                    2.
                } else {
                    0.
                };
                let remainder = (Vec2::new(snapshot.size.width, snapshot.size.height)
                    - layout.size / scale
                    - Vec2::splat(gutter * 2.))
                .max(Vec2::ZERO);
                let factor = |v| match v {
                    1 => 0.5,
                    2 => 1.,
                    _ => 0.,
                };
                let padding = UiRect {
                    left: Val::Px(
                        gutter
                            + if snapshot.data.single_line || snapshot.data.auto_size == 1 {
                                remainder.x * factor(snapshot.data.align)
                            } else {
                                0.
                            },
                    ),
                    top: Val::Px(gutter + remainder.y * factor(snapshot.data.vertical_align)),
                    right: Val::Px(gutter),
                    bottom: Val::Px(gutter),
                };
                if node.padding != padding {
                    node.padding = padding;
                }
            }
            continue;
        }
        if let Some(previous) = previous {
            commands.entity(previous.0).despawn();
        }
        let prompt = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(left),
                    top: Val::Px(top),
                    width: Val::Px(snapshot.size.width),
                    height: Val::Px(snapshot.size.height),
                    ..default()
                },
                visible,
                InputPromptOwner(owner.0),
                bevy::picking::Pickable::IGNORE,
                ChildOf(entity),
            ))
            .id();
        if let Some((asset, bitmap)) = bitmap {
            let (glyphs, _, underlines) =
                bitmap_layout(&bitmap, &snapshot.data, &snapshot.text, snapshot.size);
            for glyph in &glyphs {
                if let Some(part) =
                    spawn_glyph(&mut commands, prompt, owner.0, glyph, asset, &server)
                {
                    commands.entity(part).insert(appearance::TextTintSource {
                        owner: owner.0,
                        color: Some(if bitmap.tint {
                            snapshot.color
                        } else {
                            fairygui_core::color::Color::WHITE
                        }),
                    });
                }
            }
            for rect in underlines {
                spawn_text_decoration(&mut commands, prompt, owner.0, rect, Some(snapshot.color));
            }
        } else {
            commands.entity(prompt).insert((
                Text::new(&snapshot.text),
                TextFont {
                    font: snapshot.font.clone(),
                    ..backend::text_font_for(object, &snapshot.data, runtime.font.as_ref())
                },
                backend::text_layout_for(&snapshot.data),
                backend::text_line_height_for(&snapshot.data, None),
                backend::text_letter_spacing_for(&snapshot.data),
                appearance::TextTintSource {
                    owner: owner.0,
                    color: Some(snapshot.color),
                },
            ));
            if snapshot.data.underline {
                commands.entity(prompt).insert(Underline);
            }
        }
        commands.entity(prompt).insert(snapshot);
        commands.entity(entity).insert(InputPromptView(prompt));
    }
}

/// Editing occurs in PostUpdate; hide/reveal before extraction in that same frame.
pub(crate) fn update_prompt_visibility(
    owners: Query<&bevy::text::EditableText>,
    views: Query<(&InputPromptView, Entity)>,
    mut prompts: Query<&mut Visibility, With<InputPromptOwner>>,
) {
    for (view, entity) in &views {
        let Ok(editor) = owners.get(entity) else {
            continue;
        };
        if let Ok(mut visibility) = prompts.get_mut(view.0) {
            let next = if editor.value().to_string().is_empty() && !editor.is_composing() {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            if *visibility != next {
                *visibility = next;
            }
        }
    }
}

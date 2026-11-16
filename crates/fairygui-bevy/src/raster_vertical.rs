//! Render-only vertical command layout. Editable buffers retain Bevy geometry.
use super::*;
use bevy::text::{EditableText, TextLayoutInfo, TextReader};
#[cfg(test)]
#[path = "raster_vertical_tests.rs"]
mod tests;

#[derive(Component, Debug)]
#[require(backend::TextLineAlignment)]
pub(crate) struct NativeTextLayout {
    source: TextLayoutInfo,
    source_block: ComputedTextBlock,
    projected: TextLayoutInfo,
    projected_block: ComputedTextBlock,
    key: fit::Key,
    render_key: fit::Key,
    rows: rows::Rows,
}
impl NativeTextLayout {
    pub(crate) fn shift(&self, line: u32, section: u32) -> Vec2 {
        self.rows
            .lines
            .get(line as usize)
            .and_then(|row| row.shifts.iter().find(|(index, _, _)| *index == section))
            .map_or(Vec2::ZERO, |(_, shift, _)| *shift)
    }
}

pub(crate) fn restore(
    mut text: Query<(
        &NativeTextLayout,
        &mut TextLayoutInfo,
        &mut ComputedTextBlock,
        Has<EditableText>,
    )>,
) {
    for (layout, mut info, mut block, editable) in &mut text {
        if !editable {
            copy_info(info.bypass_change_detection(), &layout.source);
            *block.bypass_change_detection() = layout.source_block.clone();
        }
    }
}

fn copy_info(target: &mut TextLayoutInfo, source: &TextLayoutInfo) {
    target.size = source.size;
    target.scale_factor = source.scale_factor;
    target.glyphs.clone_from(&source.glyphs);
    target.run_geometry.clone_from(&source.run_geometry);
    target.inline_boxes.clone_from(&source.inline_boxes);
    target.cursor = source.cursor;
    target.selection_rects.clone_from(&source.selection_rects);
    target
        .preedit_underline_rects
        .clone_from(&source.preedit_underline_rects);
}

fn same_info(a: &TextLayoutInfo, b: &TextLayoutInfo) -> bool {
    a.size == b.size
        && a.scale_factor == b.scale_factor
        && a.cursor == b.cursor
        && a.selection_rects == b.selection_rects
        && a.preedit_underline_rects == b.preedit_underline_rects
        && a.inline_boxes == b.inline_boxes
        && a.glyphs.len() == b.glyphs.len()
        && a.glyphs.iter().zip(&b.glyphs).all(|(a, b)| {
            a.position == b.position
                && a.section_index == b.section_index
                && a.line_index == b.line_index
                && a.atlas_info.texture == b.atlas_info.texture
                && a.atlas_info.rect == b.atlas_info.rect
                && a.atlas_info.offset == b.atlas_info.offset
                && a.atlas_info.is_alpha_mask == b.atlas_info.is_alpha_mask
        })
        && a.run_geometry.len() == b.run_geometry.len()
        && a.run_geometry.iter().zip(&b.run_geometry).all(|(a, b)| {
            a.section_index == b.section_index
                && a.bounds == b.bounds
                && a.underline_y == b.underline_y
                && a.underline_thickness == b.underline_thickness
                && a.strikethrough_y == b.strikethrough_y
                && a.strikethrough_thickness == b.strikethrough_thickness
        })
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn sync(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    mut reader: TextReader<Text>,
    boundaries: Query<&rich_text::RichLinkBoundary>,
    mut atlas: ResMut<bevy::text::FontAtlasSet>,
    mut images: ResMut<Assets<Image>>,
    mut scale_cx: ResMut<bevy::text::ScaleCx>,
    mut text: Query<
        (
            Entity,
            &GObjectRef,
            &mut ComputedTextBlock,
            &mut TextLayoutInfo,
            Option<&mut NativeTextLayout>,
            Option<&fit::RasterFit>,
            Option<&ellipsis::EllipsisState>,
            Option<&mut RichTextBlock>,
            Option<&bevy::text::FontHinting>,
        ),
        (With<Text>, Without<EditableText>),
    >,
    obsolete: Query<
        Entity,
        (
            With<NativeTextLayout>,
            Or<(Without<Text>, With<EditableText>)>,
        ),
    >,
) {
    let mut service = runtime.text_measurer.borrow_mut();
    for (entity, reference, mut block, mut info, previous, fit, ellipsis, mut rich, hint) in
        &mut text
    {
        let Some(object) = runtime.store().object(reference.0) else {
            continue;
        };
        let Some(data) = object.control().as_text_field() else {
            continue;
        };
        if matches!(object.control(), ControlData::TextInput(_)) {
            commands
                .entity(entity)
                .remove::<(NativeTextLayout, backend::TextLineAlignment)>();
            continue;
        }
        let key = ellipsis
            .map(|state| state.render_key().clone())
            .unwrap_or_else(|| fit::Key {
                size: Vec2::new(object.width(), object.height()),
                gutter: 2.,
                font_size: data.font_size,
                leading: data.leading,
                native_vertical: true,
                wrap: super::wrapping::Config::for_object(object, data),
                commands: None,
                layout: backend::text_layout_for_object(object, data),
                revision: service.revision,
                runs: reader
                    .iter(entity)
                    .map(|(entity, _, element)| match element {
                        TextElement::Text {
                            text,
                            font,
                            line_height,
                            letter_spacing,
                            ..
                        } => fit::Run::Text {
                            entity,
                            text: text.into(),
                            font: font.clone(),
                            height: line_height,
                            spacing: letter_spacing,
                            metrics: None,
                        },
                        TextElement::Box(value) => fit::Run::Box {
                            entity,
                            value: *value,
                            boundary: boundaries.get(entity).copied().ok(),
                        },
                    })
                    .collect(),
            });
        if let Some(previous) = previous
            .as_ref()
            .filter(|previous| previous.key == key && same_info(&info, &previous.source))
        {
            copy_info(info.bypass_change_detection(), &previous.projected);
            *block.bypass_change_detection() = previous.projected_block.clone();
            continue;
        }
        if let Some(rich) = &mut rich {
            rich.set_image_rows(Vec::new());
        }
        let source_info = info.clone();
        let source_block = block.clone();
        let Some((render_key, mut projected_block)) = service.shape_native(&key, 1.) else {
            continue;
        };
        let mut projected_info = TextLayoutInfo::default();
        if service
            .pipeline
            .update_text_layout_info(
                &mut projected_info,
                &mut atlas,
                &mut images,
                &mut projected_block,
                &mut scale_cx,
                TextBounds::UNBOUNDED,
                Justify::Left,
                hint.copied().unwrap_or_default(),
            )
            .is_err()
        {
            continue;
        }
        *block = projected_block;
        *info = projected_info;
        let Some(rows) =
            service.raster_rows(&render_key, &block, fit.map_or(1., fit::RasterFit::scale))
        else {
            commands
                .entity(entity)
                .remove::<(NativeTextLayout, backend::TextLineAlignment)>();
            continue;
        };
        let layout = NativeTextLayout {
            source: source_info,
            source_block,
            projected: TextLayoutInfo::default(),
            projected_block: block.clone(),
            key: key.clone(),
            render_key,
            rows,
        };
        if layout.rows.lines.is_empty() {
            info.clear();
        }
        for glyph in &mut info.glyphs {
            glyph.position += layout.shift(glyph.line_index, glyph.section_index);
        }
        let dpi = info.scale_factor.max(f32::EPSILON);
        for run in &mut info.run_geometry {
            let Some(row) = layout.rows.lines.iter().find(|row| {
                (row.source_top - run.bounds.min.y).abs() < 0.01
                    && (row.source_bottom - run.bounds.max.y).abs() < 0.01
            }) else {
                continue;
            };
            let size = row
                .shifts
                .iter()
                .find(|(section, _, _)| *section == run.section_index)
                .map_or(key.font_size as f32, |(_, _, size)| *size);
            run.bounds.min.y = row.top;
            run.bounds.max.y = row.top + row.height;
            if let Some((_, shift, _)) = row
                .shifts
                .iter()
                .find(|(section, _, _)| *section == run.section_index)
            {
                run.bounds.min.x += shift.x;
                run.bounds.max.x += shift.x;
            }
            crate::text_decorations::project_geometry(
                run,
                row.logical_top,
                row.logical_height,
                size,
                dpi,
            );
        }
        if let Some(geometry) = &layout.render_key.commands {
            let mut visited = HashSet::new();
            info.run_geometry.retain_mut(|run| {
                let Some((line, command)) = geometry.lines.iter().find_map(|line| {
                    line.commands
                        .iter()
                        .find(|command| command.section == run.section_index as usize)
                        .map(|cmd| (line, cmd))
                }) else {
                    return false;
                };
                if !visited.insert(run.section_index) || command.size.x <= 0. {
                    return false;
                }
                run.bounds = Rect::new(
                    command.position.x * dpi,
                    line.top * dpi,
                    (command.position.x + command.size.x) * dpi,
                    (line.top + line.height) * dpi,
                );
                true
            });
        }
        for row in &layout.rows.lines {
            for (entity, canvas, _) in &row.images {
                if let Some((_, _, bounds)) =
                    info.inline_boxes.iter_mut().find(|(id, _, _)| id == entity)
                {
                    *bounds = *canvas;
                }
            }
        }
        info.inline_boxes.retain(|(entity, _, _)| {
            !layout.render_key.runs.iter().any(|run| {
                matches!(run, fit::Run::Box { entity: id, boundary: Some(_), .. } if id == entity)
            })
        });
        info.size.y = layout.rows.height;
        if let Some(geometry) = &layout.render_key.commands {
            info.size.x = geometry.width * dpi;
        }
        if let Some(rich) = &mut rich {
            rich.set_image_rows(
                layout
                    .rows
                    .lines
                    .iter()
                    .flat_map(|row| {
                        row.images
                            .iter()
                            .map(|(entity, _, bounds)| (*entity, *bounds))
                    })
                    .collect(),
            );
        }
        let mut layout = layout;
        layout.projected = info.clone();
        if let Some(mut previous) = previous {
            *previous = layout;
        } else {
            commands.entity(entity).insert(layout);
        }
    }
    for entity in &obsolete {
        commands
            .entity(entity)
            .remove::<(NativeTextLayout, backend::TextLineAlignment)>();
    }
}

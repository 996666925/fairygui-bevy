//! Render-only Text.ELLIPSIS projection. Source strings and editors stay intact.
use super::fit::{Key, Run};
use super::*;
use bevy::text::{
    EditableText, FontAtlasSet, FontHinting, LetterSpacing, LineHeight, ScaleCx, TextLayoutInfo,
    TextReader,
};
use parley::PositionedLayoutItem;

#[cfg(test)]
#[path = "raster_ellipsis_tests.rs"]
mod tests;

#[derive(Component, Debug)]
pub(crate) struct EllipsisState {
    key: Key,
    hint: FontHinting,
    source: ComputedTextBlock,
    source_info: TextLayoutInfo,
    projected: Option<(ComputedTextBlock, TextLayoutInfo, String)>,
    truncated: bool,
    markers: Vec<(Entity, Entity)>,
}
impl EllipsisState {
    pub(crate) fn is_projected(&self) -> bool {
        self.truncated
    }
}

/// Input uses the preceding frame's visible geometry. Restore the original
/// paragraph only after input, before Bevy measures/renders this frame.
pub(crate) fn restore(
    mut query: Query<(&EllipsisState, &mut ComputedTextBlock, &mut TextLayoutInfo)>,
) {
    for (state, mut block, mut info) in &mut query {
        if state.projected.is_some() {
            *block = state.source.clone();
            *info = state.source_info.clone();
        }
    }
}

#[derive(Debug)]
struct Command {
    run: usize,
    text: Option<String>,
    start: usize,
    end: usize,
    x: f32,
    width: f32,
    style: usize,
}

fn entity(run: &Run) -> Entity {
    match run {
        Run::Text { entity, .. } | Run::Box { entity, .. } => *entity,
    }
}

/// Keep native's ten-pixel reserve and scalar-safe UTF-16 character removal.
fn trim_marker(text: &mut String, space: f32, first: bool) {
    let remove = if space < 5. {
        2
    } else if space < 10. {
        1
    } else {
        0
    };
    for _ in 0..remove {
        if text.encode_utf16().count() > usize::from(first) {
            text.pop();
        }
    }
    text.push('…');
}

struct Plan {
    runs: Vec<Run>,
    marker_styles: Vec<(Entity, Entity)>,
}

fn plan(key: &Key, shaped: &ComputedTextBlock, dpi: f32) -> Option<Plan> {
    let layout = shaped.buffer();
    let available = (key.size - Vec2::splat(key.gutter * 2.)).max(Vec2::ZERO);
    let width = layout.full_width() / dpi;
    let height = layout.height() / dpi;
    let multiline = key.layout.linebreak != LineBreak::NoWrap;
    let lines: Vec<_> = layout.lines().collect();
    if lines.is_empty()
        || !(width + if width > 0. { key.gutter * 2. } else { 0. } > available.x
            || height + if height > 0. { key.gutter * 2. } else { 0. } > available.y
            || (!multiline && lines.len() > 1))
    {
        return None;
    }
    let count = if !multiline && lines.len() > 1 {
        1
    } else {
        lines
            .iter()
            .position(|line| line.metrics().block_max_coord / dpi > available.y)
            .map_or(lines.len(), |index| index.max(1))
    };
    let deleted = count < lines.len();
    let mut cursor = 0;
    let ranges: Vec<_> = key
        .runs
        .iter()
        .map(|run| {
            let start = cursor;
            if let Run::Text { text, .. } = run {
                cursor += text.len();
            }
            start..cursor
        })
        .collect();
    let fallback = key
        .runs
        .iter()
        .rposition(|run| matches!(run, Run::Text { text, .. } if !text.is_empty()))
        .unwrap_or(0);
    let mut commands = Vec::new();
    for line in lines.iter().take(count) {
        let mut parts: Vec<Command> = Vec::new();
        for item in line.items() {
            match item {
                PositionedLayoutItem::GlyphRun(glyphs) => {
                    let index = glyphs.style().brush.section_index as usize;
                    let Run::Text { text, .. } = &key.runs[index] else {
                        continue;
                    };
                    let run_range = glyphs.run().text_range();
                    let line_range = line.text_range();
                    let start = ranges[index]
                        .start
                        .max(run_range.start)
                        .max(line_range.start);
                    let end = ranges[index].end.min(run_range.end).min(line_range.end);
                    if start >= end {
                        continue;
                    }
                    let value = text[start - ranges[index].start..end - ranges[index].start]
                        .trim_end_matches(['\r', '\n']);
                    if value.is_empty() {
                        continue;
                    }
                    if let Some(previous) = parts.last_mut().filter(|previous| {
                        previous.run == index && previous.end == start && previous.text.is_some()
                    }) {
                        previous.text.as_mut().unwrap().push_str(value);
                        previous.end = end;
                        previous.width += glyphs.advance() / dpi;
                    } else {
                        parts.push(Command {
                            run: index,
                            text: Some(value.into()),
                            start,
                            end,
                            x: glyphs.offset() / dpi,
                            width: glyphs.advance() / dpi,
                            style: index,
                        });
                    }
                }
                PositionedLayoutItem::InlineBox(value) => {
                    let index = key
                        .runs
                        .iter()
                        .position(|run| entity(run).to_bits() == value.id)?;
                    parts.push(Command {
                        run: index,
                        text: None,
                        start: ranges[index].start,
                        end: 0,
                        x: value.x / dpi,
                        width: value.width / dpi,
                        style: fallback,
                    });
                }
            }
        }
        // Parley yields visual bidi order; native commands and their source
        // prefixes are logical. Keep logical order before reshaping the result.
        parts.sort_by_key(|part| (part.run, part.start));
        let mut x = 0.;
        for part in &mut parts {
            part.x = x;
            x += part.width;
        }
        commands.push(parts);
    }
    let last = commands.last_mut()?;
    let mut previous_text = None;
    let mut modified = deleted;
    let length = last.len();
    let mut stop = None;
    for (index, command) in last.iter_mut().enumerate() {
        if command.text.is_some() {
            previous_text = Some(command.run);
        }
        if (index + 1 == length && deleted) || command.x + command.width > available.x - 10. {
            if let Some(text) = &mut command.text {
                trim_marker(text, command.x + command.width - available.x, index == 0);
            } else {
                command.text = Some("…".into());
                command.style = previous_text.unwrap_or(fallback);
            }
            stop = Some(index + 1);
            modified = true;
            break;
        }
    }
    if let Some(stop) = stop {
        last.truncate(stop);
    }
    if !modified {
        return None;
    }
    // Freeze the chosen line breaks: inserting the marker must not wrap again.
    let mut output = vec![key.runs.first()?.clone()];
    if let Run::Text { text, .. } = &mut output[0] {
        text.clear();
    }
    let mut markers = Vec::new();
    let mut newline_style = 0;
    for (index, line) in commands.iter().enumerate() {
        // Only an authored hard break contributes its original font metrics.
        // A soft wrap has no glyph; preserve the measured height of its row.
        let end = lines[index].text_range().end;
        let hard_break = key.runs.iter().enumerate().find_map(|(run, value)| {
            let Run::Text { text, .. } = value else {
                return None;
            };
            let range = &ranges[run];
            (end > range.start
                && end <= range.end
                && text.as_bytes().get(end - range.start - 1) == Some(&b'\n'))
            .then_some(run)
        });
        for command in line {
            match (&key.runs[command.run], &command.text) {
                (Run::Box { entity: target, .. }, Some(value)) => {
                    let mut run = key.runs[command.style].clone();
                    if let Run::Text { entity, text, .. } = &mut run {
                        markers.push((*target, *entity));
                        *entity = *target;
                        *text = value.clone();
                    }
                    newline_style = command.style;
                    output.push(run);
                }
                (_, Some(value)) => {
                    let mut run = key.runs[command.run].clone();
                    if let Run::Text { text, .. } = &mut run {
                        *text = value.clone();
                    }
                    newline_style = command.run;
                    output.push(run);
                }
                (_, None) => output.push(key.runs[command.run].clone()),
            }
        }
        let more_lines = index + 1 < commands.len();
        if more_lines || line.is_empty() {
            let mut run = key.runs[hard_break.unwrap_or(newline_style)].clone();
            if let Run::Text {
                text,
                font,
                height,
                spacing,
                metrics,
                ..
            } = &mut run
            {
                *text = if more_lines { "\n" } else { "\u{200b}" }.into();
                if hard_break.is_none() && !line.is_empty() {
                    font.font_size = FontSize::Px(1.);
                    *height = LineHeight::Px(lines[index].metrics().line_height / dpi);
                    *spacing = LetterSpacing::Px(0.);
                    *metrics = None;
                }
            }
            output.push(run.clone());
            if more_lines && hard_break.is_none() {
                // Parley 0.11 takes a shaped run's line height from the next
                // style at a font boundary. End the break run with a separate
                // zero-width carrier, so the next line's font cannot enlarge
                // the preceding row. Its metrics round to zero in UI layout.
                if let Run::Text { text, font, .. } = &mut run {
                    *text = "\u{200b}".into();
                    font.font_size = FontSize::Px(0.001);
                }
                output.push(run);
            }
        }
    }
    Some(Plan {
        runs: output,
        marker_styles: markers,
    })
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn sync(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    mut reader: TextReader<Text>,
    mut roots: Query<
        (
            Entity,
            Option<&GObjectRef>,
            Option<&InputPromptOwner>,
            Option<&mut EllipsisState>,
            &mut ComputedTextBlock,
            &mut TextLayoutInfo,
            Option<&FontHinting>,
            Option<&EditableText>,
            &mut Node,
            &mut ComputedNode,
        ),
        With<Text>,
    >,
    styles: Query<(
        Option<&TextFont>,
        Option<&TextColor>,
        Option<&LineHeight>,
        Option<&LetterSpacing>,
        Option<&Underline>,
        Option<&bevy::text::Strikethrough>,
        Option<&crate::text_stroke::TextStrokeSpan>,
        Option<&appearance::TextTintSource>,
    )>,
    focus: Option<Res<bevy::input_focus::InputFocus>>,
    mut atlas: ResMut<FontAtlasSet>,
    mut images: ResMut<Assets<Image>>,
    mut scale_cx: ResMut<ScaleCx>,
) {
    let mut service = runtime.text_measurer.borrow_mut();
    for (
        root,
        reference,
        prompt,
        previous,
        mut block,
        mut info,
        hint,
        editor,
        mut node,
        mut computed_node,
    ) in &mut roots
    {
        let Some(id) = reference
            .map(|value| value.0)
            .or_else(|| prompt.map(|value| value.0))
        else {
            continue;
        };
        let Some(object) = runtime.store().object(id) else {
            continue;
        };
        let Some(data) = object.control().as_text_field() else {
            continue;
        };
        if data.auto_size != 4
            || (editor.is_some()
                && focus
                    .as_ref()
                    .is_some_and(|focus| focus.get() == Some(root)))
        {
            if previous.is_some() {
                commands.entity(root).remove::<EllipsisState>();
            }
            continue;
        }
        let key = Key {
            size: Vec2::new(object.width(), object.height()),
            gutter: if matches!(object.control(), ControlData::TextInput(_)) {
                0.
            } else {
                2.
            },
            font_size: data.font_size,
            layout: backend::text_layout_for_object(object, data),
            revision: service.revision,
            runs: reader
                .iter(root)
                .map(|(entity, _, element)| match element {
                    TextElement::Text {
                        text,
                        font,
                        line_height,
                        letter_spacing,
                        ..
                    } => Run::Text {
                        entity,
                        text: text.into(),
                        font: font.clone(),
                        height: line_height,
                        spacing: letter_spacing,
                        metrics: None,
                    },
                    TextElement::Box(value) => Run::Box {
                        entity,
                        value: *value,
                    },
                })
                .collect(),
        };
        let hint = hint.copied().unwrap_or_default();
        let state = if let Some(previous) = previous
            .as_ref()
            .filter(|previous| previous.key == key && previous.hint == hint)
        {
            EllipsisState {
                key,
                hint,
                source: block.clone(),
                source_info: info.clone(),
                projected: previous.projected.clone(),
                truncated: previous.truncated,
                markers: previous.markers.clone(),
            }
        } else {
            let mut measurement = key.clone();
            measurement.layout.justify = Justify::Left;
            if measurement.layout.linebreak == LineBreak::NoWrap {
                measurement.layout.linebreak = LineBreak::AnyCharacter;
            }
            let Some(shaped) = service.shape_block(&measurement, 1.) else {
                info.clear();
                continue;
            };
            let mut markers = Vec::new();
            let planned = plan(&key, &shaped, service.scale_factor.max(f32::EPSILON));
            let truncated = planned.is_some();
            let projected = {
                let mut projected = key.clone();
                if let Some(plan) = planned {
                    markers = plan.marker_styles;
                    projected.layout.linebreak = LineBreak::NoWrap;
                    projected.runs = plan.runs;
                }
                let visible_text: String = projected
                    .runs
                    .iter()
                    .filter_map(|run| {
                        if let Run::Text { text, .. } = run {
                            Some(text.as_str())
                        } else {
                            None
                        }
                    })
                    .collect();
                let Some(mut computed) = service.shape_block(&projected, 1.) else {
                    info.clear();
                    continue;
                };
                let mut projected_info = TextLayoutInfo::default();
                let dpi = service.scale_factor.max(f32::EPSILON);
                if service
                    .pipeline
                    .update_text_layout_info(
                        &mut projected_info,
                        &mut atlas,
                        &mut images,
                        &mut computed,
                        &mut scale_cx,
                        TextBounds::new_horizontal((key.size.x - key.gutter * 2.).max(0.) * dpi),
                        key.layout.justify,
                        hint,
                    )
                    .is_err()
                {
                    info.clear();
                    continue;
                }
                // Newline/empty-line carriers have no interactive area.
                projected_info
                    .run_geometry
                    .retain(|run| run.bounds.width() > 0.);
                Some((computed, projected_info, visible_text))
            };
            EllipsisState {
                key,
                hint,
                source: block.clone(),
                source_info: info.clone(),
                projected,
                truncated,
                markers,
            }
        };
        for &(target, source) in &state.markers {
            let (
                Ok((font, color, height, spacing, underline, strike, stroke, tint)),
                Ok((
                    old_font,
                    old_color,
                    old_height,
                    old_spacing,
                    old_underline,
                    old_strike,
                    old_stroke,
                    old_tint,
                )),
            ) = (styles.get(source), styles.get(target))
            else {
                continue;
            };
            let mut target = commands.entity(target);
            if font != old_font
                && let Some(font) = font
            {
                target.insert(font.clone());
            }
            if color != old_color
                && let Some(color) = color
            {
                target.insert(*color);
            }
            if height != old_height
                && let Some(height) = height
            {
                target.insert(*height);
            }
            if spacing != old_spacing
                && let Some(spacing) = spacing
            {
                target.insert(*spacing);
            }
            let underlined = underline.is_some() || (source == root && data.underline);
            if underlined != old_underline.is_some() {
                if underlined {
                    target.insert(Underline);
                } else {
                    target.remove::<Underline>();
                }
            }
            if strike.is_some() != old_strike.is_some() {
                if strike.is_some() {
                    target.insert(bevy::text::Strikethrough);
                } else {
                    target.remove::<bevy::text::Strikethrough>();
                }
            }
            if let Some(stroke) = stroke {
                if old_stroke
                    .is_none_or(|old| old.width != stroke.width || old.color != stroke.color)
                {
                    target.insert(*stroke);
                }
            } else if old_stroke.is_some() {
                target.remove::<crate::text_stroke::TextStrokeSpan>();
            }
            if old_tint
                .is_none_or(|old| old.owner != id || old.color != tint.and_then(|tint| tint.color))
            {
                target.insert(appearance::TextTintSource {
                    owner: id,
                    color: tint.and_then(|tint| tint.color),
                });
            }
        }
        if let Some((projected, projected_info, _)) = &state.projected {
            *block = projected.clone();
            *info = projected_info.clone();
            // UI extraction subtracts the source editor's scroll offset. The
            // unfocused static projection starts at the field origin instead.
            if let Some(editor) = editor {
                let offset = editor.viewport.offset;
                for glyph in &mut info.glyphs {
                    glyph.position += offset;
                }
                for run in &mut info.run_geometry {
                    run.bounds.min += offset;
                    run.bounds.max += offset;
                }
                for (_, _, rect) in &mut info.inline_boxes {
                    rect.min += offset;
                    rect.max += offset;
                }
            }
            let mut alignment = state.key.layout;
            if state.is_projected() {
                alignment.linebreak = LineBreak::NoWrap;
            }
            backend::align_text_block(
                object,
                data,
                &info,
                &alignment,
                &mut node,
                &mut computed_node,
                state.key.gutter,
            );
        }
        if let Some(mut previous) = previous {
            *previous = state;
        } else {
            commands.entity(root).insert(state);
        }
    }
}

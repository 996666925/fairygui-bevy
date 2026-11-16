//! Vertical command layout shared by sizing, fitting, ellipsis and rendering.
use super::fit::{Key, Run};
use super::*;

#[derive(Debug, Clone)]
pub(super) struct Row {
    pub top: f32,
    pub logical_top: f32,
    pub logical_height: f32,
    pub height: f32,
    pub shifts: Vec<(u32, Vec2, f32)>, // section, glyph offset, font size
    pub images: Vec<(Entity, Rect, Rect)>, // canvas, link area
    pub source_top: f32,
    pub source_bottom: f32,
}

#[derive(Debug, Clone)]
pub(super) struct Rows {
    pub lines: Vec<Row>,
    pub height: f32,
    pub logical_height: f32,
}

impl TextMeasurer {
    pub(super) fn raster_rows(
        &mut self,
        key: &Key,
        block: &ComputedTextBlock,
        font_scale: f64,
    ) -> Option<Rows> {
        if key
            .runs
            .iter()
            .all(|run| matches!(run, Run::Text { text, .. } if text.is_empty()))
        {
            return Some(Rows {
                lines: Vec::new(),
                height: 0.,
                logical_height: 0.,
            });
        }
        let dpi = block.buffer().scale().max(f32::EPSILON);
        let leading = (f64::from(key.leading) * font_scale).floor() as f32;
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
        let mut top = 0_f64;
        let mut lines = Vec::new();
        let mut last_font_height = None;
        for (line_index, line) in block.buffer().lines().enumerate() {
            let native = key.commands.as_ref().and_then(|g| g.lines.get(line_index));
            let source_lefts: HashMap<_, f32> = line
                .items()
                .filter_map(|item| {
                    let parley::PositionedLayoutItem::GlyphRun(glyphs) = item else {
                        return None;
                    };
                    Some((glyphs.style().brush.section_index, glyphs.offset()))
                })
                .fold(HashMap::new(), |mut map, (section, x)| {
                    map.entry(section)
                        .and_modify(|value| *value = value.min(x))
                        .or_insert(x);
                    map
                });
            let mut content = 0_f32;
            let mut text = Vec::new();
            let mut boxes = Vec::new();
            let mut fallback = None;
            let mut commands = false;
            for item in line.items() {
                match item {
                    parley::PositionedLayoutItem::InlineBox(value)
                        if value.kind == parley::InlineBoxKind::InFlow =>
                    {
                        content = content.max((value.height / dpi).round());
                        boxes.push(value);
                        commands = true;
                    }
                    parley::PositionedLayoutItem::GlyphRun(glyphs) => {
                        let index = glyphs.style().brush.section_index as usize;
                        let Run::Text {
                            font, text: source, ..
                        } = key.runs.get(index)?
                        else {
                            continue;
                        };
                        let size = glyphs.run().font_size() / dpi;
                        // Ellipsis' soft-break carriers are not authored commands.
                        if size < 1. {
                            continue;
                        }
                        let metrics = self.font_geometry(font)?;
                        let size = size.round();
                        let height = metrics.height(size);
                        fallback = Some(height);
                        last_font_height = Some(height);
                        let run_range = glyphs.run().text_range();
                        let start = ranges[index]
                            .start
                            .max(run_range.start)
                            .max(line.text_range().start);
                        let end = ranges[index]
                            .end
                            .min(run_range.end)
                            .min(line.text_range().end);
                        let value = source.get(
                            start.saturating_sub(ranges[index].start)
                                ..end.saturating_sub(ranges[index].start),
                        )?;
                        if value.trim_end_matches(['\r', '\n']).is_empty() {
                            continue;
                        }
                        content = content.max(height);
                        commands = true;
                        text.push((
                            index as u32,
                            height,
                            metrics.baseline(size),
                            glyphs.baseline(),
                            size,
                        ));
                    }
                    _ => {}
                }
            }
            if content == 0. {
                content = if let Some(height) = fallback.or(last_font_height) {
                    height
                } else if !commands {
                    let Run::Text { font, metrics, .. } = key.runs.first()? else {
                        return None;
                    };
                    let size =
                        metrics.map_or(font.font_size, |metrics| metrics.scaled(font_scale).0);
                    let FontSize::Px(size) = size else {
                        return None;
                    };
                    self.font_geometry(font)?.height(size)
                } else {
                    0.
                };
            }
            let row_top = native.map_or(top as f32, |line| line.top);
            let height = native.map_or(content + 1., |line| line.height);
            let shifts = text
                .into_iter()
                .map(|(section, own_height, baseline, source, size)| {
                    let command = native.and_then(|line| {
                        line.commands
                            .iter()
                            .find(|cmd| cmd.section == section as usize)
                    });
                    let y = command
                        .map_or(((content - own_height) * 0.5).floor(), |cmd| cmd.position.y);
                    let x = command.map_or(0., |cmd| {
                        cmd.position.x * dpi - source_lefts.get(&section).copied().unwrap_or(0.)
                    });
                    (
                        section,
                        Vec2::new(x, (row_top + y + baseline) * dpi - source),
                        size,
                    )
                })
                .collect();
            let images = boxes
                .into_iter()
                .map(|image| {
                    let own_height = (image.height / dpi).round();
                    let command = native.and_then(|line| line.commands.iter().find(|cmd| {
                        matches!(key.runs.get(cmd.section), Some(Run::Box { entity, .. }) if entity.to_bits() == image.id)
                    }));
                    let y = (row_top + command.map_or(((content - own_height) * 0.5).floor(), |cmd| cmd.position.y)) * dpi;
                    let x = command.map_or(image.x, |cmd| cmd.position.x * dpi);
                    let canvas = Rect::from_corners(
                        Vec2::new(x, y),
                        Vec2::new(x + image.width, y + image.height),
                    );
                    let link = Rect::new(
                        x,
                        row_top * dpi,
                        x + image.width,
                        (row_top + height) * dpi,
                    );
                    (Entity::from_bits(image.id), canvas, link)
                })
                .collect();
            lines.push(Row {
                top: row_top * dpi,
                logical_top: row_top,
                logical_height: height,
                height: height * dpi,
                shifts,
                images,
                source_top: line.metrics().block_min_coord,
                source_bottom: line.metrics().block_max_coord,
            });
            top += f64::from(height + leading);
        }
        let logical_height = lines
            .last()
            .map_or(0., |line| line.logical_top + line.logical_height);
        Some(Rows {
            lines,
            height: logical_height * dpi,
            logical_height,
        })
    }
}

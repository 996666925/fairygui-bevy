//! Native command wrapping; virtual spans preserve the application's entities.
use super::{
    fit::{Key, Run},
    *,
};
use fairygui_core::text::wrapping::{WrapElement, WrapKind, WrapOptions, wrap_commands};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Config {
    pub width: Option<f32>,
    pub wrap: bool,
    pub no_break_word: bool,
    pub align: bool,
}
impl Config {
    pub(super) fn for_object(
        object: &fairygui_core::GObject,
        data: &fairygui_core::widget::TextFieldData,
    ) -> Option<Self> {
        if matches!(object.control(), ControlData::TextInput(_)) {
            return None;
        }
        let word_wrap = !data.single_line && data.auto_size != 1;
        let max = object.max_size().width;
        let width = if data.auto_size == 1 {
            (max > 0.).then_some((max - 4.).max(0.))
        } else {
            Some((object.width() - 4.).max(0.))
        };
        Some(Self {
            width,
            wrap: word_wrap || data.auto_size == 4 || max > 0.,
            no_break_word: word_wrap,
            align: data.auto_size != 1,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Placement {
    pub section: usize,
    pub position: Vec2,
    pub size: Vec2,
    pub font_size: f32,
}
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Line {
    pub top: f32,
    pub height: f32,
    pub width: f32,
    pub commands: Vec<Placement>,
}
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Geometry {
    pub lines: Vec<Line>,
    pub width: f32,
    pub height: f32,
}

/// Ellipsis only re-finalizes its final row. Earlier rows retain the original
/// pre-carry width/height and command positions, including mixed-size words.
pub(super) fn preserve_prefix(source: &Key, target: &mut Key) {
    let (Some(original), Some(projected)) = (&source.commands, &mut target.commands) else {
        return;
    };
    let geometry = Arc::make_mut(projected);
    let last = geometry.lines.len().saturating_sub(1);
    for (index, line) in geometry.lines.iter_mut().enumerate() {
        let Some(old) = original.lines.get(index) else {
            continue;
        };
        line.top = old.top;
        if index == last {
            continue;
        }
        line.width = old.width;
        line.height = old.height;
        for command in &mut line.commands {
            let current = &target.runs[command.section];
            let Some(previous) = old.commands.iter().find(|previous| {
                match (&source.runs[previous.section], current) {
                    (
                        Run::Text {
                            entity: a, text: x, ..
                        },
                        Run::Text {
                            entity: b, text: y, ..
                        },
                    ) => a == b && x == y,
                    (Run::Box { entity: a, .. }, Run::Box { entity: b, .. }) => a == b,
                    _ => false,
                }
            }) else {
                continue;
            };
            command.position = previous.position;
            command.size = previous.size;
        }
    }
    geometry.width = geometry
        .lines
        .iter()
        .map(|line| line.width)
        .fold(0., f32::max);
    geometry.height = geometry
        .lines
        .last()
        .map_or(0., |line| line.top + line.height);
}

impl TextMeasurer {
    /// Ordinary-font widths are measured as complete substrings at logical 1x,
    /// including the native prefix estimate and UTF-16 character accounting.
    pub(super) fn shape_native(
        &mut self,
        key: &Key,
        scale: f64,
    ) -> Option<(Key, ComputedTextBlock)> {
        let Some(config) = key.wrap.as_ref().filter(|_| key.commands.is_none()) else {
            return Some((key.clone(), self.shape_block(key, scale)?));
        };
        if key
            .runs
            .iter()
            .all(|run| matches!(run, Run::Text { text, .. } if text.is_empty()))
        {
            let mut empty = key.clone();
            empty.commands = Some(Arc::new(Geometry {
                lines: Vec::new(),
                width: 0.,
                height: 0.,
            }));
            return Some((empty, ComputedTextBlock::default()));
        }
        let mut source = key.clone();
        for run in &mut source.runs {
            if let Run::Text {
                font,
                height,
                metrics,
                ..
            } = run
                && let Some(metrics) = metrics.take()
            {
                (font.font_size, *height) = metrics.scaled(scale);
            }
        }
        let base_font = source.runs.iter().find_map(|run| match run {
            Run::Text { font, .. } if matches!(font.font_size, FontSize::Px(size) if size >= 1.) => Some(font.clone()), _ => None,
        });
        // Frozen soft breaks carry no style. When a projection is reshaped,
        // process them as line boundaries using the field's default font for
        // genuinely empty rows; they never add a visible command.
        if let Some(base_font) = base_font {
            for run in &mut source.runs {
                if let Run::Text { text, font, .. } = run
                    && matches!(font.font_size, FontSize::Px(size) if size < 1.)
                {
                    *font = base_font.clone();
                    *text = text.replace('\u{200b}', "");
                }
            }
        }
        let mut indices = Vec::new();
        let mut elements = Vec::new();
        for (index, run) in source.runs.iter().enumerate() {
            let (kind, size, height) = match run {
                Run::Text { text, font, .. } => {
                    if text.is_empty() {
                        continue;
                    }
                    let FontSize::Px(size) = font.font_size else {
                        return None;
                    };
                    (
                        WrapKind::Text(text.clone()),
                        size,
                        self.font_geometry(font)?.height(size),
                    )
                }
                Run::Box { value, .. } => {
                    let raw_width = if value.size.x > 0. {
                        (value.size.x - 2.).max(0.)
                    } else {
                        0.
                    };
                    (
                        WrapKind::Object(fairygui_core::Size::new(raw_width, value.size.y)),
                        0.,
                        value.size.y,
                    )
                }
            };
            indices.push(index);
            elements.push(WrapElement {
                kind,
                font_size: f64::from(size),
                height: f64::from(height),
            });
        }
        let mut measurements = HashMap::new();
        let mut failed = false;
        let lines = wrap_commands(
            &elements,
            WrapOptions {
                width: config.width.map_or(f64::MAX, f64::from),
                leading: (f64::from(key.leading) * scale).floor(),
                wrap: config.wrap,
                no_break_word: config.no_break_word,
                raster: true,
            },
            |index, text, _| {
                let cache_key = (index, text.to_vec());
                if let Some(width) = measurements.get(&cache_key) {
                    return *width;
                }
                let mut fragment = source.clone();
                fragment.layout = TextLayout::new(Justify::Left, LineBreak::NoWrap);
                fragment.gutter = 0.;
                fragment.size = Vec2::splat(1e8);
                fragment.runs = vec![source.runs[indices[index]].clone()];
                let Run::Text { text: target, .. } = &mut fragment.runs[0] else {
                    return 0.;
                };
                *target = String::from_utf16_lossy(text);
                // Widths must not change with the render target's DPI.
                let dpi = self.scale_factor;
                self.scale_factor = 1.;
                let measured = self.shape_block(&fragment, 1.);
                self.scale_factor = dpi;
                let width = measured.map_or_else(
                    || {
                        failed = true;
                        0.
                    },
                    |block| f64::from(block.buffer().full_width()),
                );
                measurements.insert(cache_key, width);
                width
            },
        );
        if failed {
            return None;
        }
        let mut output = source.clone();
        output.layout = TextLayout::new(Justify::Left, LineBreak::NoWrap);
        output.runs = source.runs.first().cloned().into_iter().collect();
        if let Some(Run::Text { text, .. }) = output.runs.first_mut() {
            text.clear();
        }
        let mut geometry = Geometry {
            lines: Vec::new(),
            width: 0.,
            height: 0.,
        };
        for (line_index, line) in lines.iter().enumerate() {
            let mut placements = Vec::new();
            let remainder =
                f64::from(config.width.unwrap_or(key.size.x - 2. * key.gutter)) - line.width;
            let offset = if config.align {
                match key.layout.justify {
                    Justify::Center => (remainder * 0.5).floor().max(0.),
                    Justify::Right => remainder.max(0.),
                    _ => 0.,
                }
            } else {
                0.
            };
            for cmd in &line.cmds {
                let mut run = source.runs[indices[cmd.element]].clone();
                if let (Run::Text { text, .. }, Some(value)) = (&mut run, &cmd.text) {
                    *text = String::from_utf16_lossy(value);
                }
                let margin = if matches!(run, Run::Box { value, .. } if value.size.x > 0.) {
                    1.
                } else {
                    0.
                };
                placements.push(Placement {
                    section: output.runs.len(),
                    position: Vec2::new((offset + cmd.x - margin) as f32, cmd.y as f32),
                    size: Vec2::new(cmd.width as f32, cmd.height as f32),
                    font_size: cmd.font_size as f32,
                });
                output.runs.push(run);
            }
            geometry.width = geometry.width.max(line.width as f32);
            geometry.height = (line.y + line.height) as f32;
            geometry.lines.push(Line {
                top: line.y as f32,
                height: line.height as f32,
                width: line.width as f32,
                commands: placements,
            });
            // Freeze native line breaks without injecting font-height struts.
            if line_index + 1 < lines.len() || line.cmds.is_empty() {
                let mut carrier = source
                    .runs
                    .iter()
                    .find(|run| matches!(run, Run::Text { .. }))?
                    .clone();
                if let Run::Text {
                    text,
                    font,
                    height,
                    spacing,
                    metrics,
                    ..
                } = &mut carrier
                {
                    *text = if line_index + 1 < lines.len() {
                        "\n"
                    } else {
                        "\u{200b}"
                    }
                    .into();
                    font.font_size = FontSize::Px(0.001);
                    *height = bevy::text::LineHeight::Px(0.);
                    *spacing = bevy::text::LetterSpacing::Px(0.);
                    *metrics = None;
                }
                output.runs.push(carrier);
            }
        }
        output.commands = Some(Arc::new(geometry));
        let block = self.shape_block(&output, 1.)?;
        Some((output, block))
    }
}

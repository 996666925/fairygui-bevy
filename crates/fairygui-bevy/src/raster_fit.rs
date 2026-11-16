//! Raster Text.SHRINK policy using the same Parley shaping as the visible text.
use super::*;
use bevy::text::{InlineBox, LetterSpacing, LineHeight, TextReader};

#[cfg(test)]
#[path = "raster_fit_tests.rs"]
mod tests;

/// Keep requested metrics separate from the projected (possibly smaller) font.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub(crate) struct FontMetrics {
    size: i32,
    leading: i32,
}
impl FontMetrics {
    pub(crate) fn new(data: &fairygui_core::widget::TextFieldData, size: Option<i32>) -> Self {
        Self {
            size: size.unwrap_or(data.font_size),
            leading: data.leading,
        }
    }
    pub(super) fn scaled(self, scale: f64) -> (FontSize, LineHeight) {
        let size = (f64::from(self.size) * scale).floor().max(1.) as f32;
        let leading = (f64::from(self.leading) * scale).floor() as f32;
        (FontSize::Px(size), LineHeight::Px((size + leading).max(1.)))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Run {
    Text {
        entity: Entity,
        text: String,
        font: TextFont,
        height: LineHeight,
        spacing: LetterSpacing,
        metrics: Option<FontMetrics>,
    },
    Box {
        entity: Entity,
        value: InlineBox,
        boundary: Option<rich_text::RichLinkBoundary>,
    },
}

#[derive(Debug, Clone)]
pub(super) struct Key {
    pub(super) size: Vec2,
    pub(super) gutter: f32,
    pub(super) font_size: i32,
    pub(super) leading: i32,
    pub(super) native_vertical: bool,
    pub(super) wrap: Option<super::wrapping::Config>,
    pub(super) commands: Option<std::sync::Arc<super::wrapping::Geometry>>,
    pub(super) layout: TextLayout,
    pub(super) revision: u64,
    pub(super) runs: Vec<Run>,
}
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.size == other.size
            && self.gutter == other.gutter
            && self.font_size == other.font_size
            && self.leading == other.leading
            && self.native_vertical == other.native_vertical
            && self.wrap == other.wrap
            && self.commands == other.commands
            && self.layout.linebreak == other.layout.linebreak
            && self.layout.justify == other.layout.justify
            && self.revision == other.revision
            && self.runs == other.runs
    }
}

#[derive(Component, Debug)]
pub(crate) struct RasterFit {
    key: Key,
    scale: f64,
}
impl RasterFit {
    pub(super) fn scale(&self) -> f64 {
        self.scale
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn sync(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    roots: Query<(Entity, &GObjectRef, Option<&RasterFit>), With<Text>>,
    metrics: Query<&FontMetrics>,
    boundaries: Query<&rich_text::RichLinkBoundary>,
    mut text: ParamSet<(TextReader<Text>, Query<(&mut TextFont, &mut LineHeight)>)>,
) {
    let mut service = runtime.text_measurer.borrow_mut();
    for (entity, object, previous) in &roots {
        let Some(object) = runtime.store().object(object.0) else {
            continue;
        };
        let Some(data) = object.control().as_text_field() else {
            continue;
        };
        if data.auto_size != 3 && previous.is_none() {
            continue;
        }
        let mut reader = text.p0();
        let runs = reader
            .iter(entity)
            .map(|(entity, _, element)| match element {
                TextElement::Text {
                    text,
                    font,
                    line_height,
                    letter_spacing,
                    ..
                } => {
                    let original = metrics.get(entity).ok().copied();
                    let mut font = font.clone();
                    let mut height = line_height;
                    if let Some(original) = original {
                        (font.font_size, height) = original.scaled(1.);
                    }
                    Run::Text {
                        entity,
                        text: text.to_owned(),
                        font,
                        height,
                        spacing: letter_spacing,
                        metrics: original,
                    }
                }
                TextElement::Box(value) => Run::Box {
                    entity,
                    value: *value,
                    boundary: boundaries.get(entity).copied().ok(),
                },
            })
            .collect();
        let key = Key {
            size: Vec2::new(object.width(), object.height()),
            gutter: if matches!(object.control(), ControlData::TextInput(_)) {
                0.
            } else {
                2.
            },
            font_size: data.font_size,
            leading: data.leading,
            native_vertical: !matches!(object.control(), ControlData::TextInput(_)),
            wrap: super::wrapping::Config::for_object(object, data),
            commands: None,
            layout: backend::text_layout_for_object(object, data),
            revision: service.revision,
            runs,
        };
        let fitted = if data.auto_size != 3 {
            Some(1.)
        } else if let Some(previous) = previous.filter(|previous| previous.key == key) {
            Some(previous.scale)
        } else {
            service.fit_raster(&key)
        };
        let scale = fitted.unwrap_or(1.);
        let mut output = text.p1();
        for run in &key.runs {
            if let Run::Text {
                entity,
                metrics: Some(metrics),
                ..
            } = run
                && let Ok((mut font, mut height)) = output.get_mut(*entity)
            {
                let (size, leading) = metrics.scaled(scale);
                if font.font_size != size {
                    font.font_size = size;
                }
                if *height != leading {
                    *height = leading;
                }
            }
        }
        if data.auto_size == 3 && fitted.is_some() {
            if previous.is_none_or(|previous| previous.key != key || previous.scale != scale) {
                commands.entity(entity).insert(RasterFit { key, scale });
            }
        } else if previous.is_some() {
            commands.entity(entity).remove::<RasterFit>();
        }
    }
}

impl TextMeasurer {
    pub(super) fn shape_block(&mut self, key: &Key, scale: f64) -> Option<ComputedTextBlock> {
        let fonts: Vec<_> = key
            .runs
            .iter()
            .map(|run| match run {
                Run::Text {
                    font,
                    height,
                    metrics,
                    ..
                } => {
                    let mut font = font.clone();
                    font.font = self.font_source(font.font)?;
                    let mut height = *height;
                    if let Some(metrics) = metrics {
                        (font.font_size, height) = metrics.scaled(scale);
                    }
                    Some((font, height))
                }
                Run::Box { .. } => Some((TextFont::default(), LineHeight::default())),
            })
            .collect::<Option<_>>()?;
        let runs = key.runs.iter().enumerate().map(|(index, run)| match run {
            Run::Text {
                entity,
                text,
                spacing,
                ..
            } => (
                *entity,
                0,
                TextElement::Text {
                    text,
                    font: &fonts[index].0,
                    color: Color::WHITE,
                    line_height: fonts[index].1,
                    letter_spacing: *spacing,
                },
            ),
            Run::Box { entity, value, .. } => (*entity, 0, TextElement::Box(value)),
        });
        let mut computed = ComputedTextBlock::default();
        let dpi = self.scale_factor.max(f32::EPSILON);
        self.pipeline
            .update_buffer(
                &self.fonts,
                runs,
                key.layout.linebreak,
                key.layout.justify,
                TextBounds {
                    // Native command fragments already contain every break.
                    // Parley can otherwise wrap inline boxes again even with
                    // NoWrap, losing the native one-pixel fit tolerance.
                    width: key
                        .commands
                        .is_none()
                        .then_some((key.size.x - key.gutter * 2.).max(0.) * dpi),
                    height: None,
                },
                dpi,
                &mut computed,
                &mut self.font_cx,
                &mut self.layout_cx,
                Vec2::ZERO,
                bevy::text::RemSize::default(),
            )
            .ok()?;
        Some(computed)
    }

    fn shape_raster(&mut self, key: &Key, scale: f64) -> Option<(Vec2, usize)> {
        let (render_key, computed) = self.shape_native(key, scale)?;
        let layout = computed.buffer();
        let native = if key.native_vertical {
            self.raster_rows(&render_key, &computed, scale)
        } else {
            None
        };
        let dpi = self.scale_factor.max(f32::EPSILON);
        Some((
            Vec2::new(
                render_key
                    .commands
                    .as_ref()
                    .map_or(layout.full_width() / dpi, |geometry| geometry.width),
                native.map_or(layout.height() / dpi, |rows| rows.logical_height),
            ),
            layout.lines().count(),
        ))
    }

    fn fit_metrics(&mut self, key: &Key, scale: f64) -> Option<(Vec2, usize)> {
        let (mut size, lines) = self.shape_raster(key, scale)?;
        // Text.calcTextSize includes padding in nonempty textWidth/textHeight;
        // Text.SHRINK compares those values with the content rectangle.
        if size.x > 0. {
            size.x += key.gutter * 2.;
        }
        if size.y > 0. {
            size.y += key.gutter * 2.;
        }
        Some((size, lines))
    }

    fn fit_raster(&mut self, key: &Key) -> Option<f64> {
        if !key.size.is_finite() {
            return None;
        }
        let available = (key.size - Vec2::splat(key.gutter * 2.)).max(Vec2::ZERO);
        let (mut measured, lines) = self.fit_metrics(key, 1.)?;
        let base = f64::from(key.font_size.max(1));
        let mut scale = 1.;
        // Follow Text.doLayout's multiline search and single-line width policy.
        // In particular, one tall line alone does not trigger height shrinking.
        if lines > 1 && measured.y > available.y {
            let (mut low, mut high) = (0., base);
            scale = (f64::from(available.y) / f64::from(measured.y)).sqrt();
            let mut current = (scale * base).floor();
            // Fixed-size inline objects cannot always fit, even at 1px text.
            for _ in 0..128 {
                measured = self.fit_metrics(key, scale)?.0;
                if measured.x > available.x || measured.y > available.y {
                    high = current;
                } else {
                    low = current;
                }
                if high - low > 1. || (high != low && current == high) {
                    current = low + (high - low) / 2.;
                    scale = current / base;
                } else {
                    break;
                }
            }
        } else if measured.x > available.x {
            scale = f64::from(available.x) / f64::from(measured.x);
            if self.fit_metrics(key, scale)?.0.x > available.x {
                scale = ((base * scale).floor() - 1.) / base;
            }
        }
        Some(scale)
    }
}

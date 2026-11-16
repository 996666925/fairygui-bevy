//! Laya's fixed-size font probe, using the fonts selected by Bevy/Parley.
use super::*;
use bevy::text::{FontSize, FontSource, FontStyle, GenericFontFamily, LetterSpacing, LineHeight};

#[derive(Debug, PartialEq)]
enum SourceKey {
    Asset(bevy::asset::AssetId<Font>),
    Family(String),
    Families(String),
    List(Vec<SourceKey>),
    Generic(GenericFontFamily),
}
impl SourceKey {
    fn new(source: &FontSource) -> Self {
        match source {
            FontSource::Handle(handle) => Self::Asset(handle.id()),
            FontSource::Family(name) => Self::Family(name.to_string()),
            FontSource::Families(names) => Self::Families(names.to_string()),
            FontSource::List(sources) => Self::List(sources.iter().map(Self::new).collect()),
            FontSource::Generic(family) => Self::Generic(*family),
        }
    }
}

#[derive(Debug, PartialEq)]
struct FontKey {
    source: SourceKey,
    style: TextFont,
}
impl FontKey {
    fn new(font: &TextFont) -> Self {
        let mut style = font.clone();
        let source = SourceKey::new(&style.font);
        // Cache identity must not pin old application font assets in memory.
        style.font = FontSource::default();
        Self { source, style }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct FontGeometry {
    height: f32,
    y_offset: f32,
    middle: f32,
}

impl FontGeometry {
    pub(super) fn height(self, size: f32) -> f32 {
        (self.height * size / 32.).ceil()
    }

    pub(super) fn baseline(self, size: f32) -> f32 {
        size * 0.5 + (self.middle * size * 64.).round() / 64. + (self.y_offset * size / 32.).ceil()
    }
}

#[derive(Default)]
pub(super) struct FontGeometryCache {
    entries: Vec<(FontKey, FontGeometry)>,
    scaler: swash::scale::ScaleContext,
}

impl FontGeometryCache {
    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }
}

fn middle(font: swash::FontRef<'_>, coords: &[i16]) -> f32 {
    let base = font.metrics(&[]);
    let metrics = font.metrics(coords);
    let typo = font
        .table(swash::tag_from_bytes(b"OS/2"))
        .and_then(|table| {
            Some((
                i16::from_be_bytes(table.get(68..70)?.try_into().ok()?) as f32,
                i16::from_be_bytes(table.get(70..72)?.try_into().ok()?) as f32,
            ))
        });
    let (ascent, descent) = typo
        .filter(|(ascent, descent)| ascent - descent > 0.)
        .unwrap_or((base.ascent, -base.descent));
    (ascent + descent + metrics.ascent - base.ascent - metrics.descent + base.descent)
        / (2. * f32::from(metrics.units_per_em))
}

/// Preserve MeasureFont.updateBbx's search and its initial [16,32] bounds.
/// It deliberately finds a boundary row rather than tight outline bounds.
fn extend_bounds(rows: &[bool; 64], bounds: &mut [usize; 2]) {
    let (mut start, mut end) = (bounds[0], 0);
    if rows[start] {
        loop {
            let row = (start + end) / 2;
            if row + 1 >= start {
                bounds[0] = row;
                break;
            }
            if rows[row] {
                start = row;
            } else {
                end = row;
            }
        }
    }
    let (mut start, mut end) = (bounds[1], 64);
    if rows[start] {
        loop {
            let row = (start + end) / 2;
            if row <= start + 1 {
                bounds[1] = row;
                break;
            }
            if rows[row] {
                start = row;
            } else {
                end = row;
            }
        }
    }
}

impl TextMeasurer {
    pub(super) fn font_geometry(&mut self, font: &TextFont) -> Option<FontGeometry> {
        let mut font = font.clone();
        font.font_size = FontSize::Px(32.);
        // Native measurement distinguishes bold, but not italic.
        font.style = FontStyle::Normal;
        let key = FontKey::new(&font);
        if let Some((_, result)) = self
            .font_geometry
            .entries
            .iter()
            .find(|(cached, _)| *cached == key)
        {
            return Some(*result);
        }
        let mut probe_font = font.clone();
        probe_font.font = self.font_source(probe_font.font)?;
        let mut block = ComputedTextBlock::default();
        self.pipeline
            .update_buffer(
                &self.fonts,
                std::iter::once((
                    Entity::PLACEHOLDER,
                    0,
                    TextElement::Text {
                        text: "g有",
                        font: &probe_font,
                        color: Color::WHITE,
                        line_height: LineHeight::Px(32.),
                        letter_spacing: LetterSpacing::Px(0.),
                    },
                )),
                LineBreak::NoWrap,
                Justify::Left,
                TextBounds::UNBOUNDED,
                1.,
                &mut block,
                &mut self.font_cx,
                &mut self.layout_cx,
                Vec2::ZERO,
                bevy::text::RemSize::default(),
            )
            .ok()?;
        let mut bounds = [16, 32];
        let mut middle_ratio = None;
        for line in block.buffer().lines() {
            for item in line.items() {
                let parley::PositionedLayoutItem::GlyphRun(glyphs) = item else {
                    continue;
                };
                let run = glyphs.run();
                let data = run.font();
                let face = swash::FontRef::from_index(data.data.as_ref(), data.index as usize)?;
                let ratio =
                    *middle_ratio.get_or_insert_with(|| middle(face, run.normalized_coords()));
                let baseline = 32. + (ratio * 32. * 64.).round() / 64.;
                let mut scaler = self
                    .font_geometry
                    .scaler
                    .builder_with_id(face, [data.data.id(), data.index.into()])
                    .size(32.)
                    .hint(true)
                    .normalized_coords(run.normalized_coords())
                    .build();
                for glyph in glyphs.glyphs() {
                    let canvas_width = (glyph.advance + 32.).floor() as i32;
                    let glyph = u16::try_from(glyph.id).ok()?;
                    let mask = swash::scale::Render::new(&[swash::scale::Source::Outline])
                        .offset(swash::zeno::Vector::new(0., baseline.fract()))
                        .render(&mut scaler, glyph)?;
                    let mut rows = [false; 64];
                    let width = mask.placement.width as usize;
                    if width == 0 {
                        continue;
                    }
                    let left = 16 + mask.placement.left;
                    let start = (-left).max(0).min(width as i32) as usize;
                    let end = (canvas_width - left).max(0).min(width as i32) as usize;
                    for (row, pixels) in mask.data.chunks_exact(width).enumerate() {
                        let y = baseline.floor() as i32 - mask.placement.top + row as i32;
                        if (0..64).contains(&y)
                            && start < end
                            && pixels[start..end].iter().any(|&alpha| alpha != 0)
                        {
                            rows[y as usize] = true;
                        }
                    }
                    extend_bounds(&rows, &mut bounds);
                }
            }
        }
        let result = FontGeometry {
            height: (bounds[1] - bounds[0] + 1) as f32,
            y_offset: 16_usize.saturating_sub(bounds[0]) as f32,
            middle: middle_ratio?,
        };
        self.font_geometry.entries.push((key, result));
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probing_and_cache_hits_do_not_keep_the_application_font_handle_alive() {
        let face = Font::from_bytes(
            include_bytes!("../../fairygui-demo/assets/fonts/NotoSansSC-VF.ttf").to_vec(),
        );
        let mut assets = Assets::<Font>::default();
        let handle = assets.add(face.clone());
        let mut service = TextMeasurer::default();
        let copy = service.fonts.add(face.clone());
        service.fonts.get_mut(&copy).unwrap().alias = "Geometry Cache Test".into();
        service.font_cx.collection.register_fonts(
            face.data,
            Some(parley::fontique::FontInfoOverride {
                family_name: Some("Geometry Cache Test"),
                ..default()
            }),
        );
        service.font_handles.insert(handle.id(), copy);
        let font = TextFont {
            font: FontSource::Handle(handle.clone()),
            ..default()
        };
        let Handle::Strong(strong) = &handle else {
            panic!("indexed font");
        };
        let before = std::sync::Arc::strong_count(strong);
        assert!(service.font_geometry(&font).is_some());
        assert!(service.font_geometry(&font).is_some());
        assert_eq!(std::sync::Arc::strong_count(strong), before);
    }
}

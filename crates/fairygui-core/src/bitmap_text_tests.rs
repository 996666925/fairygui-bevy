use super::*;
use crate::{
    geom::Rect,
    package::{BmGlyph, GlyphTexture},
};
use std::collections::HashMap;

#[test]
fn executable_native_bitmap_measurement_layout_and_draw_geometry_match() {
    assert_eq!(
        check_reference(
            include_str!("../tests/fixtures/bitmap-text-reference.txt"),
            false
        ),
        (422, 2391, 0)
    );
}

#[test]
fn executable_native_bitmap_underlines_and_strike_bounds_match() {
    assert_eq!(
        check_reference(
            include_str!("../tests/fixtures/bitmap-decoration-reference.txt"),
            true
        ),
        (422, 2391, 757)
    );
}

fn check_reference(reference: &str, decorated: bool) -> (usize, usize, usize) {
    let mut fonts = HashMap::new();
    let mut current_font = None;
    let mut current = None;
    let mut expected = Vec::new();
    let mut cases = 0;
    let mut quads = 0;
    let mut name = "";
    let mut underlines = 0;
    let mut current_underlines = 0;
    for line in reference.lines() {
        if line.starts_with('#') {
            continue;
        }
        let p: Vec<_> = line.split_whitespace().collect();
        let number = |i: usize| p[i].parse::<f32>().unwrap();
        match p[0] {
            "font" => {
                let id = p[1].parse::<u32>().unwrap();
                current_font = Some(id);
                fonts.insert(
                    id,
                    BitmapFontData {
                        ttf: false,
                        tint: p[3] == "1",
                        auto_scale_size: p[2] == "1",
                        font_size: number(4) as i32,
                        line_height: number(5) as i32,
                        x_advance: number(6),
                        glyphs: HashMap::new(),
                    },
                );
            }
            "glyph" => {
                let code = number(1) as u32;
                fonts
                    .get_mut(&current_font.unwrap())
                    .unwrap()
                    .glyphs
                    .insert(
                        code,
                        BmGlyph {
                            char_code: code,
                            x: number(2),
                            y: number(3),
                            width: number(4),
                            height: number(5),
                            advance: number(6),
                            texture: (p[7] == "1").then_some(GlyphTexture::FontAtlas {
                                atlas_item: 0,
                                rect: Rect::ZERO,
                            }),
                        },
                    );
            }
            "case" => {
                if let Some(previous) = current.take() {
                    let previous: BitmapTextLayout = previous;
                    assert_eq!(previous.glyphs, expected, "{name}");
                }
                expected.clear();
                current_underlines = 0;
                name = p[1];
                let font = &fonts[&(number(2) as u32)];
                let data = TextFieldData {
                    underline: decorated,
                    font_size: number(3) as i32,
                    auto_size: number(6) as i32,
                    single_line: p[7] == "1",
                    align: number(8) as i32,
                    vertical_align: number(9) as i32,
                    leading: number(10) as i32,
                    ..Default::default()
                };
                let text = String::from_utf16(
                    &p[11..]
                        .iter()
                        .map(|s| s.parse::<u16>().unwrap())
                        .collect::<Vec<_>>(),
                )
                .unwrap();
                current = Some(layout_bitmap_text(
                    font,
                    &data,
                    &text,
                    Size::new(number(4), number(5)),
                ));
                cases += 1;
            }
            "metrics" => {
                let got = current.as_ref().unwrap();
                for (value, want) in [
                    got.text_size.width as f64,
                    got.text_size.height as f64,
                    got.object_size.width as f64,
                    got.object_size.height as f64,
                    got.font_scale,
                ]
                .into_iter()
                .zip(p[1..].iter().map(|s| s.parse::<f64>().unwrap()))
                {
                    assert!(
                        (value - want).abs() < 0.00005,
                        "{name}: metrics {value} != {want}"
                    );
                }
            }
            "quad" => {
                let glyph = BitmapTextGlyph {
                    char_code: number(1) as u32,
                    x: number(2),
                    y: number(3),
                    width: number(4),
                    height: number(5),
                };
                let got = &current.as_ref().unwrap().glyphs[expected.len()];
                assert_eq!(got.char_code, glyph.char_code, "{name}");
                for (value, want) in [got.x, got.y, got.width, got.height].into_iter().zip([
                    glyph.x,
                    glyph.y,
                    glyph.width,
                    glyph.height,
                ]) {
                    assert!(
                        (value - want).abs() < 0.00005,
                        "{name}: quad {value} != {want}"
                    );
                }
                expected.push(got.clone());
                quads += 1;
            }
            "underline" => {
                let got = current.as_ref().unwrap().underlines[current_underlines];
                for (got, want) in [got.x, got.y, got.width, got.height]
                    .into_iter()
                    .zip(p[1..].iter().map(|s| s.parse::<f32>().unwrap()))
                {
                    assert!(
                        (got - want).abs() < 0.00005,
                        "{name}: {line}: {got} != {want}"
                    );
                }
                current_underlines += 1;
                underlines += 1;
            }
            "strike" => {
                let got = bitmap_decoration_rect(
                    number(1) as f64,
                    number(2) as f64,
                    number(3) as f64,
                    number(4) as f64,
                    number(5) as f64,
                    true,
                );
                for (got, want) in [got.x, got.y, got.width, got.height]
                    .into_iter()
                    .zip(p[6..].iter().map(|s| s.parse::<f32>().unwrap()))
                {
                    assert!(
                        (got - want).abs() < 0.00005,
                        "{name}: {line}: {got} != {want}"
                    );
                }
            }
            "end" => {
                assert_eq!(
                    current.as_ref().unwrap().underlines.len(),
                    current_underlines,
                    "{name}"
                );
                assert_eq!(
                    current.as_ref().unwrap().glyphs.len(),
                    expected.len(),
                    "{name}"
                );
            }
            _ => panic!("{line}"),
        }
    }
    (cases, quads, underlines)
}

#[test]
fn native_measurement_rounds_each_step_ignores_missing_utf16_and_respects_fixed_size() {
    let mut font = BitmapFontData {
        ttf: false,
        tint: false,
        auto_scale_size: true,
        font_size: 20,
        line_height: 24,
        x_advance: 99.0,
        glyphs: HashMap::new(),
    };
    for (code, advance) in [(65, 11.0), (66, 12.0)] {
        font.glyphs.insert(
            code,
            BmGlyph {
                char_code: code,
                x: 0.0,
                y: 0.0,
                width: 9.0,
                height: 17.0,
                advance,
                texture: None,
            },
        );
    }
    assert_eq!(font.measure_width_at("A?😀B", 17), 19.0);
    font.auto_scale_size = false;
    assert_eq!(font.measure_width_at("A?😀B", 17), 23.0);
    assert_eq!(font.measure_width("?😀?"), 0.0);
}

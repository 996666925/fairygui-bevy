use super::*;
use crate::package::{BmGlyph, GlyphTexture};
use std::collections::HashMap;

#[test]
fn executable_native_multi_command_layout_glyphs_images_and_link_rectangles_match() {
    let mut fonts = HashMap::new();
    let mut font_id = 0;
    let mut selected = 0;
    let mut elements = Vec::new();
    let mut data = TextFieldData::default();
    let mut size = Size::ZERO;
    let mut current = None;
    let mut name = "";
    let mut counts = [0; 4];
    let mut totals = [0; 4];
    let mut cases = 0;
    for line in include_str!("../tests/fixtures/bitmap-rich-reference.txt").lines() {
        if line.starts_with('#') {
            continue;
        }
        let p: Vec<_> = line.split_whitespace().collect();
        let n = |i: usize| p[i].parse::<f32>().unwrap();
        let close = |got: f32, want: f32| {
            assert!(
                (got.is_nan() && want.is_nan()) || (got - want).abs() < 0.0001,
                "{name}: {line}: {got} != {want}"
            )
        };
        match p[0] {
            "font" => {
                font_id = n(1) as u32;
                fonts.insert(
                    font_id,
                    BitmapFontData {
                        ttf: false,
                        tint: p[3] == "1",
                        auto_scale_size: p[2] == "1",
                        font_size: n(4) as i32,
                        line_height: n(5) as i32,
                        x_advance: n(6),
                        glyphs: HashMap::new(),
                    },
                );
            }
            "glyph" => {
                let code = n(1) as u32;
                fonts.get_mut(&font_id).unwrap().glyphs.insert(
                    code,
                    BmGlyph {
                        char_code: code,
                        x: n(2),
                        y: n(3),
                        width: n(4),
                        height: n(5),
                        advance: n(6),
                        texture: (p[7] == "1").then_some(GlyphTexture::FontAtlas {
                            atlas_item: 0,
                            rect: Rect::ZERO,
                        }),
                    },
                );
            }
            "case" => {
                name = p[1];
                selected = n(2) as u32;
                data = TextFieldData {
                    font_size: n(3) as i32,
                    auto_size: n(6) as i32,
                    single_line: p[7] == "1",
                    align: n(8) as i32,
                    vertical_align: n(9) as i32,
                    leading: n(10) as i32,
                    ..Default::default()
                };
                size = Size::new(n(4), n(5));
                elements.clear();
                counts = [0; 4];
                cases += 1;
            }
            "element" => {
                elements.push(BitmapElement {
                    font_size: n(2) as i32,
                    kind: match p[1] {
                        "T" => BitmapElementKind::Text(
                            String::from_utf16(
                                &p[3..]
                                    .iter()
                                    .map(|s| s.parse().unwrap())
                                    .collect::<Vec<u16>>(),
                            )
                            .unwrap(),
                        ),
                        "I" => BitmapElementKind::Image(Size::new(n(3), n(4))),
                        "S" => BitmapElementKind::LinkStart,
                        "E" => BitmapElementKind::LinkEnd,
                        _ => panic!("{line}"),
                    },
                });
            }
            "metrics" => {
                let got = layout_bitmap_rich_text(&fonts[&selected], &data, &elements, size);
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
                        (value - want).abs() < 0.0001,
                        "{name}: {line}: {value} != {want}"
                    );
                }
                current = Some(got);
            }
            "run" => {
                let r = &current.as_ref().unwrap().runs[counts[0]];
                if p[1] != "-1" {
                    assert_eq!(r.element, n(1) as usize, "{name}: {line}");
                }
                for (v, w) in [r.rect.x, r.rect.y, r.rect.width, r.rect.height]
                    .into_iter()
                    .zip((2..6).map(n))
                {
                    close(v, w);
                }
                if matches!(elements[r.element].kind, BitmapElementKind::Text(_)) {
                    close(r.font_size, n(6));
                }
                close(r.line_height, n(7));
                counts[0] += 1;
                totals[0] += 1;
            }
            "quad" => {
                let g = &current.as_ref().unwrap().glyphs[counts[1]].glyph;
                assert_eq!(g.char_code, n(1) as u32, "{name}: {line}");
                for (v, w) in [g.x, g.y, g.width, g.height].into_iter().zip((2..6).map(n)) {
                    close(v, w);
                }
                counts[1] += 1;
                totals[1] += 1;
            }
            "image" | "link" => {
                let slot = if p[0] == "image" { 2 } else { 3 };
                let list = if slot == 2 {
                    &current.as_ref().unwrap().images
                } else {
                    &current.as_ref().unwrap().links
                };
                let (element, rect) = list[counts[slot]];
                assert_eq!(element, n(1) as usize, "{name}: {line}");
                for (v, w) in [rect.x, rect.y, rect.width, rect.height]
                    .into_iter()
                    .zip((2..6).map(n))
                {
                    close(v, w);
                }
                counts[slot] += 1;
                totals[slot] += 1;
            }
            "end" => {
                let got = current.as_ref().unwrap();
                assert_eq!(
                    counts,
                    [
                        got.runs.len(),
                        got.glyphs.len(),
                        got.images.len(),
                        got.links.len()
                    ],
                    "{name}"
                );
            }
            _ => panic!("{line}"),
        }
    }
    assert_eq!(
        (cases, totals[1], totals[2], totals[3]),
        (955, 6208, 406, 287)
    );
}

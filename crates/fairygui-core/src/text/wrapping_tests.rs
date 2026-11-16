use super::*;

fn units(value: &str) -> Vec<u16> {
    if value == "_" {
        return Vec::new();
    }
    value
        .as_bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|bytes| {
            let bytes = std::str::from_utf8(bytes).unwrap();
            u16::from_le_bytes([
                u8::from_str_radix(&bytes[..2], 16).unwrap(),
                u8::from_str_radix(&bytes[2..], 16).unwrap(),
            ])
        })
        .collect()
}
fn width(text: &[u16], size: f64) -> f64 {
    String::from_utf16_lossy(text)
        .chars()
        .map(|ch| {
            if ch as u32 > 0xffff {
                23.
            } else if ch as u32 > 127 {
                19.
            } else if ch == ' ' {
                5.
            } else if "il!.,".contains(ch) {
                4.
            } else if ch == 'W' {
                18.
            } else {
                11.
            }
        })
        .sum::<f64>()
        * size
        / 20.
}

#[test]
fn raster_command_wrapping_matches_unmodified_native_layout_in_132_cases() {
    let mut elements = Vec::new();
    let mut expected = Vec::<WrappedLine>::new();
    let mut options = WrapOptions {
        width: 0.,
        leading: 3.,
        wrap: true,
        no_break_word: true,
        raster: true,
    };
    let mut name = "";
    let mut count = 0;
    for line in include_str!("../../tests/fixtures/raster-wrap-reference.txt").lines() {
        let p: Vec<_> = line.split_whitespace().collect();
        match p.first().copied() {
            Some("case") => {
                name = p[1];
                elements.clear();
                expected.clear();
                options.width = p[2].parse().unwrap();
                options.wrap = p[3] == "1";
                options.no_break_word = options.wrap;
            }
            Some("element") => {
                let size: f64 = p[2].parse().unwrap();
                elements.push(WrapElement {
                    font_size: size,
                    height: (size * 1.2).ceil(),
                    kind: if p[1] == "T" {
                        WrapKind::Text(String::from_utf16(&units(p[3])).unwrap())
                    } else {
                        WrapKind::Object(Size::new(p[3].parse().unwrap(), p[4].parse().unwrap()))
                    },
                });
            }
            Some("line") => expected.push(WrappedLine {
                width: p[1].parse().unwrap(),
                height: p[2].parse().unwrap(),
                y: p[3].parse().unwrap(),
                ..Default::default()
            }),
            Some("cmd") => expected.last_mut().unwrap().cmds.push(WrappedCommand {
                element: p[1].parse().unwrap(),
                x: p[2].parse().unwrap(),
                y: p[3].parse().unwrap(),
                width: p[4].parse().unwrap(),
                height: p[5].parse().unwrap(),
                font_size: p[6].parse().unwrap(),
                text: (p[7] != "object").then(|| units(p[7])),
                link_end: false,
            }),
            Some("end") => {
                let actual = wrap_commands(&elements, options, |_, text, size| width(text, size));
                assert_eq!(
                    actual.len(),
                    expected.len(),
                    "case {name}: {actual:?} != {expected:?}"
                );
                for (actual, expected) in actual.iter().zip(&expected) {
                    for (a, b) in [
                        (actual.width, expected.width),
                        (actual.height, expected.height),
                        (actual.y, expected.y),
                    ] {
                        assert!(
                            (a - b).abs() < 0.001,
                            "case {name}: {actual:?} != {expected:?}"
                        );
                    }
                    assert_eq!(actual.cmds.len(), expected.cmds.len(), "case {name}");
                    for (a, b) in actual.cmds.iter().zip(&expected.cmds) {
                        assert_eq!((a.element, &a.text), (b.element, &b.text), "case {name}");
                        for (a, b) in [
                            (a.x, b.x),
                            (a.y, b.y),
                            (a.width, b.width),
                            (a.height, b.height),
                        ] {
                            assert!((a - b).abs() < 0.001, "case {name}: {a} != {b}");
                        }
                        if a.text.is_some() {
                            assert_eq!(a.font_size, b.font_size, "case {name}");
                        }
                    }
                }
                count += 1;
            }
            _ => {}
        }
    }
    assert_eq!(count, 132);
}

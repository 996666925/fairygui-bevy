use super::*;

fn decode(value: &str) -> Option<String> {
    if value == "-" {
        return None;
    }
    if value == "_" {
        return Some(String::new());
    }
    let bytes: Vec<_> = value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect();
    Some(String::from_utf8(bytes).unwrap())
}
#[test]
fn executable_native_ubb_html_xml_styles_attributes_and_element_boundaries_match() {
    let mut options = MarkupOptions::default();
    let mut ubb = false;
    let mut input = String::new();
    let mut current = MarkupDocument::default();
    let mut expected = Vec::new();
    let mut name = "";
    let mut cases = 0;
    let mut elements = 0;
    for line in include_str!("../../tests/fixtures/markup-reference.txt").lines() {
        if line.starts_with('#') {
            continue;
        }
        let p: Vec<_> = line.split_whitespace().collect();
        let integer = |i: usize| p[i].parse::<i32>().unwrap();
        match p[0] {
            "case" => {
                name = p[1];
                ubb = p[2] == "1";
                options = MarkupOptions {
                    style: MarkupStyle {
                        font_size: integer(3),
                        color: "#326496".into(),
                        bold: p[4] == "1",
                        ..Default::default()
                    },
                    link_underline: p[5] == "1",
                    link_color: decode(p[6]),
                    ignore_whitespace: p[7] == "1",
                    default_image_width: integer(8),
                    default_image_height: integer(9),
                };
                expected.clear();
                cases += 1;
            }
            "input" => input = decode(p[1]).unwrap(),
            "html" => {
                let html = if ubb {
                    ubb_to_html(&input, &options)
                } else {
                    input.clone()
                };
                assert_eq!(html, decode(p[1]).unwrap(), "{name}: UBB conversion");
                current = parse_markup(&input, ubb, &options);
            }
            "element" => {
                expected.push(MarkupElement {
                    kind: match p[1] {
                        "0" => MarkupKind::Text,
                        "1" => MarkupKind::Link,
                        "2" => MarkupKind::Image,
                        "3" => MarkupKind::Input,
                        "4" => MarkupKind::Select,
                        "6" => MarkupKind::LinkEnd,
                        _ => panic!("{line}"),
                    },
                    text: decode(p[12]).unwrap(),
                    style: MarkupStyle {
                        font_size: integer(2),
                        color: decode(p[3]).unwrap(),
                        bold: p[4] == "1",
                        italic: p[5] == "1",
                        underline: p[6] == "1",
                        strikethrough: p[7] == "1",
                        stroke: integer(8),
                        stroke_color: decode(p[9]).unwrap(),
                        color_changed: p[10] == "1",
                        align: decode(p[11]),
                    },
                    attributes: BTreeMap::new(),
                    items: Vec::new(),
                    values: Vec::new(),
                });
                elements += 1;
            }
            "attr" => {
                expected
                    .last_mut()
                    .unwrap()
                    .attributes
                    .insert(decode(p[1]).unwrap(), decode(p[2]).unwrap());
            }
            "item" => expected
                .last_mut()
                .unwrap()
                .items
                .push(decode(p[1]).unwrap()),
            "value" => expected
                .last_mut()
                .unwrap()
                .values
                .push(decode(p[1]).unwrap()),
            "color" => {
                let c = markup_color(&decode(p[1]).unwrap());
                for (got, want) in [c.r, c.g, c.b, c.a]
                    .into_iter()
                    .zip(p[2..].iter().map(|s| s.parse::<f32>().unwrap()))
                {
                    assert!(
                        (got.is_nan() && want.is_nan()) || (got - want).abs() < 1e-6,
                        "{line}: {got} != {want}"
                    );
                }
            }
            "end" => assert_eq!(current.elements, expected, "{name}: source={input:?}"),
            _ => panic!("{line}"),
        }
    }
    assert_eq!((cases, elements), (783, 1851));
}

#[test]
fn native_html_image_explicit_and_automatic_dimensions_match_cached_deferred_and_failed_textures() {
    let mut dimensions = ImageDimensions::default();
    let mut mode = "";
    let mut natural = crate::geom::Size::ZERO;
    let mut cases = 0;
    let mut states = 0;
    let mut callbacks = 0;
    for line in include_str!("../../tests/fixtures/inline-image-reference.txt").lines() {
        if line.starts_with('#') {
            continue;
        }
        let p: Vec<_> = line.split_whitespace().collect();
        match p[0] {
            "case" => {
                let mut attrs = BTreeMap::new();
                if let Some(w) = decode(p[2]) {
                    attrs.insert("width".into(), w);
                }
                if let Some(h) = decode(p[3]) {
                    attrs.insert("height".into(), h);
                }
                dimensions = ImageDimensions::from_attributes(&attrs);
                mode = p[4];
                natural = crate::geom::Size::new(p[5].parse().unwrap(), p[6].parse().unwrap());
                cases += 1;
            }
            "before" | "after" => {
                let loaded = mode == "cached" || (p[0] == "after" && mode == "success");
                let size = dimensions.resolve(loaded.then_some(natural));
                assert_eq!(
                    size,
                    crate::geom::Size::new(p[1].parse().unwrap(), p[2].parse().unwrap()),
                    "{line}"
                );
                let refresh = usize::from(p[0] == "after" && mode != "cached");
                assert_eq!(refresh, p[3].parse::<usize>().unwrap());
                states += 1;
            }
            "lifecycle" => {
                match p[1] {
                    "replacement" => assert_eq!(&p[2..], ["20", "45", "1", "1"]),
                    "release" | "destroy" => assert_eq!(&p[2..], ["10", "0", "0", "0"]),
                    _ => panic!("{line}"),
                };
                callbacks += 1;
            }
            _ => panic!("{line}"),
        }
    }
    assert_eq!((cases, states, callbacks), (33, 66, 3));
}

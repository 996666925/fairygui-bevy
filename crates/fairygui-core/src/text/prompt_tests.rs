use super::*;

fn decode(s: &str) -> Option<String> {
    match s {
        "-" => None,
        "_" => Some(String::new()),
        _ => Some(
            String::from_utf8(
                s.as_bytes()
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|v| u8::from_str_radix(std::str::from_utf8(v).unwrap(), 16).unwrap())
                    .collect(),
            )
            .unwrap(),
        ),
    }
}

#[test]
fn native_prompt_removal_and_projection_fixture_preserves_literals_default_color_and_real_text() {
    let mut counts = [0; 3];
    for line in include_str!("../../tests/fixtures/input-prompt-reference.txt").lines() {
        if line.starts_with('#') {
            continue;
        }
        let p: Vec<_> = line.split_whitespace().collect();
        match p[0] {
            "strip" => {
                assert_eq!(
                    strip_ubb_tags(&decode(p[1]).unwrap()),
                    decode(p[2]).unwrap(),
                    "{line}"
                );
                assert_eq!(decode(p[3]).unwrap(), "#a9a9a9");
                assert_eq!(decode(p[4]), None);
                counts[0] += 1;
            }
            "project" => {
                let source = decode(p[1]).unwrap();
                let value = decode(p[2]).unwrap();
                let hidden = p[3] == "1";
                let prompt = strip_ubb_tags(&source);
                let prompt = if prompt == "!hint!" {
                    "Localized hint".to_owned()
                } else {
                    prompt
                };
                let is_prompt = value.is_empty() && !prompt.is_empty();
                let rendered = if hidden {
                    String::new()
                } else {
                    (if is_prompt { prompt } else { value }).replace("\r\n", "\n")
                };
                assert_eq!(rendered, decode(p[4]).unwrap(), "{line}");
                let color = (!rendered.is_empty())
                    .then_some(if is_prompt { "#a9a9a9" } else { "#123456" }.to_owned());
                assert_eq!(color, decode(p[5]), "{line}");
                assert_eq!(decode(p[6]).unwrap(), "#123456");
                counts[1] += 1;
            }
            "editable" => {
                counts[2] += 1;
                if p[1] == "default" {
                    assert!(crate::widget::TextInputData::default().editable);
                    assert_eq!(p[2], "1");
                }
            }
            _ => panic!("{line}"),
        }
    }
    assert_eq!(counts, [24, 96, 4]);
}

use super::*;

fn decode(s: &str) -> String {
    if s == "_" {
        return String::new();
    }
    String::from_utf8(
        s.as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|v| u8::from_str_radix(std::str::from_utf8(v).unwrap(), 16).unwrap())
            .collect(),
    )
    .unwrap()
}

#[test]
fn native_password_projection_counts_utf16_after_ubb_and_crlf_but_never_masks_prompts() {
    let mut counts = [0; 3];
    for line in include_str!("../../tests/fixtures/input-password-reference.txt").lines() {
        if line.starts_with('#') {
            continue;
        }
        let p: Vec<_> = line.split_whitespace().collect();
        match p[0] {
            "project" => {
                let source = decode(p[1]);
                let value = if source == "!value!" {
                    "译😀"
                } else {
                    &source
                };
                let expected = if value.is_empty() {
                    "Readable hint".to_owned()
                } else if p[4] == "1" {
                    input::password_text(value, p[2] == "1")
                } else if p[2] == "1" {
                    ubb_to_html(&value.replace("\r\n", "\n"), &MarkupOptions::default())
                } else {
                    value.replace("\r\n", "\n")
                };
                assert_eq!(expected, decode(p[5]), "{line}");
                assert_eq!(
                    source,
                    decode(p[6]),
                    "projection mutated native value: {line}"
                );
                counts[0] += 1;
            }
            "type" => {
                assert_eq!(p[1], p[3]);
                assert_eq!(p[1], p[4]);
                assert_eq!(decode(p[2]), if p[1] == "1" { "password" } else { "text" });
                counts[1] += 1;
                assert_eq!(p[5].parse::<usize>().unwrap(), counts[1]);
            }
            "element" => {
                assert_eq!(
                    p[3],
                    if p[1] == "1" {
                        "password"
                    } else if p[2] == "1" {
                        "textarea"
                    } else {
                        "input"
                    }
                );
                counts[2] += 1;
            }
            _ => panic!("{line}"),
        }
    }
    assert_eq!(counts, [128, 5, 4]);
}

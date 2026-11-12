//! Native Text.font resource-name detection (Utils.getFileExtension).

/// Resource-like names go through the loader; other names select a font family.
/// Native extension detection uses the last dot, including dots in directories,
/// and removes the extension's query suffix. Bitmap registration takes precedence.
pub fn is_font_resource(name: &str) -> bool {
    name.starts_with("res://")
        || name.rsplit_once('.').is_some_and(|(_, extension)| {
            !extension.split('?').next().unwrap_or_default().is_empty()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_text_font_classification_matches_executable_reference() {
        let mut cases = 0;
        for line in include_str!("../../tests/fixtures/font-selection-reference.txt").lines() {
            if !line.starts_with("classify ") {
                continue;
            }
            let parts: Vec<_> = line.split_whitespace().collect();
            let name = if parts[1] == "_" {
                String::new()
            } else {
                String::from_utf8(
                    parts[1]
                        .as_bytes()
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|hex| {
                            u8::from_str_radix(std::str::from_utf8(hex).unwrap(), 16).unwrap()
                        })
                        .collect(),
                )
                .unwrap()
            };
            assert_eq!(is_font_resource(&name), parts[2] == "1", "{name}");
            cases += 1;
        }
        assert_eq!(cases, 25);
    }
}

//! Laya Input/TextInputAdapter's committed-text validation.

#[derive(Debug, Clone)]
pub struct InputRestriction(Option<regress::Regex>);

impl InputRestriction {
    /// Compile the native inverted character class, including its first ^^ removal.
    pub fn new(source: Option<&str>) -> Result<Self, regress::Error> {
        let Some(source) = source.filter(|s| !s.is_empty()) else {
            return Ok(Self(None));
        };
        let pattern = format!("[^{source}]").replacen("^^", "", 1);
        regress::Regex::from_unicode(
            pattern.encode_utf16().map(u32::from),
            regress::Flags::default(),
        )
        .map(|regex| Self(Some(regex)))
    }

    /// Restriction validates the entire committed value, not individual keystrokes.
    /// Native non-Unicode regexes operate on UTF-16 units. Unpaired output units
    /// cross Rust's String boundary as replacement characters.
    pub fn validate(&self, text: &str, multiline: bool) -> String {
        let text = normalize_input_lines(text, multiline);
        let Some(regex) = &self.0 else {
            return text;
        };
        let clean: String = text
            .chars()
            .filter(|&c| c != '\u{2006}' && c != '\'')
            .collect();
        let units: Vec<_> = clean.encode_utf16().collect();
        let mut output = Vec::new();
        let mut start = 0;
        for found in regex.find_from_ucs2(&units, 0) {
            let range = found.range();
            output.extend_from_slice(&units[start..range.start]);
            start = range.end;
        }
        output.extend_from_slice(&units[start..]);
        String::from_utf16_lossy(&output)
    }
}

/// Input.text removes LF and its optional preceding CR only in single-line mode.
pub fn normalize_input_lines(text: &str, multiline: bool) -> String {
    if multiline {
        return text.to_owned();
    }
    text.replace("\r\n", "").replace('\n', "")
}

/// Laya Text masks after CRLF normalization and UBB expansion, before HTML parsing.
/// Its JavaScript length counts UTF-16 units, including both halves of astral text.
/// Prompts bypass this projection and must be rendered separately.
pub fn password_text(text: &str, ubb: bool) -> String {
    let text = text.replace("\r\n", "\n");
    let text = if ubb {
        super::ubb_to_html(&text, &super::MarkupOptions::default())
    } else {
        text
    };
    "●".repeat(text.encode_utf16().count())
}

/// The browser adapter uses 100000 when maxChars is zero/negative.
pub fn input_max_length(max: i32) -> usize {
    if max > 0 { max as usize } else { 100_000 }
}

/// Bound an incoming edit while preserving unselected existing text. Unlike a
/// programmatic text assignment, user insertion obeys the UTF-16 length budget.
/// Do not split a Unicode scalar when the remaining budget is one code unit.
pub fn limit_input_insert<'a>(
    current: &str,
    selected: &str,
    incoming: &'a str,
    max: i32,
) -> &'a str {
    let remaining = input_max_length(max).saturating_sub(
        current
            .encode_utf16()
            .count()
            .saturating_sub(selected.encode_utf16().count()),
    );
    let mut used = 0;
    for (index, c) in incoming.char_indices() {
        used += c.len_utf16();
        if used > remaining {
            return &incoming[..index];
        }
    }
    incoming
}

#[cfg(test)]
mod tests {
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
                        .map(|hex| {
                            u8::from_str_radix(std::str::from_utf8(hex).unwrap(), 16).unwrap()
                        })
                        .collect(),
                )
                .unwrap(),
            ),
        }
    }
    #[test]
    fn executable_native_restrictions_newlines_programmatic_assignments_and_limit_properties_match()
    {
        let mut counts = [0; 4];
        for line in include_str!("../../tests/fixtures/input-reference.txt").lines() {
            if line.starts_with('#') {
                continue;
            }
            let p: Vec<_> = line.split_whitespace().collect();
            match p[0] {
                "validate" => {
                    let r = decode(p[1]);
                    let policy = InputRestriction::new(r.as_deref()).unwrap();
                    assert_eq!(
                        policy.validate(&decode(p[3]).unwrap(), p[2] == "1"),
                        decode(p[4]).unwrap(),
                        "{line}"
                    );
                    counts[0] += 1;
                }
                "invalid" => {
                    assert!(
                        InputRestriction::new(decode(p[1]).as_deref()).is_err(),
                        "{line}"
                    );
                    counts[1] += 1;
                }
                "program" => {
                    assert_eq!(
                        normalize_input_lines(&decode(p[2]).unwrap(), p[1] == "1"),
                        decode(p[3]).unwrap(),
                        "{line}"
                    );
                    counts[2] += 1;
                }
                "limit" => {
                    assert_eq!(
                        input_max_length(p[1].parse().unwrap()),
                        p[2].parse::<usize>().unwrap()
                    );
                    counts[3] += 1;
                }
                _ => panic!("{line}"),
            }
        }
        assert_eq!(counts, [576, 4, 24, 6]);
    }
    #[test]
    fn insert_budget_counts_utf16_preserves_selection_and_never_splits_a_scalar() {
        assert_eq!(limit_input_insert("a😀b", "", "XYZ", 5), "X");
        assert_eq!(limit_input_insert("a😀b", "😀", "XYZ", 5), "XYZ");
        assert_eq!(limit_input_insert("a", "", "😀Z", 2), "");
        assert_eq!(limit_input_insert("a", "", "😀Z", 3), "😀");
        assert_eq!(limit_input_insert("existing over limit", "", "a", 2), "");
    }
}

//! Native Laya Utils.parseTemplate syntax. Values are expanded once, before markup.
use std::collections::BTreeMap;

pub type TextVariables = BTreeMap<String, String>;

pub fn expand_template(template: &str, variables: &TextVariables) -> String {
    let mut result = String::with_capacity(template.len());
    let mut cursor = 0;
    while let Some(relative) = template[cursor..].find('{') {
        let open = cursor + relative;
        if open > 0 && template.as_bytes()[open - 1] == b'\\' {
            result.push_str(&template[cursor..open - 1]);
            result.push('{');
            cursor = open + 1;
            continue;
        }
        result.push_str(&template[cursor..open]);
        cursor = open;
        let Some(relative) = template[open..].find('}') else {
            break;
        };
        let close = open + relative;
        if close == open + 1 {
            result.push_str("{}");
        } else {
            let tag = &template[open + 1..close];
            let (name, fallback) = tag.split_once('=').unwrap_or((tag, ""));
            result.push_str(variables.get(name).map_or(fallback, String::as_str));
        }
        cursor = close + 1;
    }
    result.push_str(&template[cursor..]);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn decode(hex: &str) -> String {
        if hex == "-" {
            return String::new();
        }
        String::from_utf8(
            hex.as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap())
                .collect(),
        )
        .unwrap()
    }
    #[test]
    fn defaults_escapes_missing_unicode_and_nonrecursive_values_match_original() {
        let mut variables = TextVariables::new();
        let mut count = 0;
        for line in include_str!("../../tests/fixtures/text-template-reference.txt").lines() {
            if line.starts_with('#') {
                continue;
            }
            let mut parts = line.split_whitespace();
            let kind = parts.next().unwrap();
            let key = decode(parts.next().unwrap());
            let value = decode(parts.next().unwrap());
            if kind == "var" {
                variables.insert(key, value);
            } else {
                assert_eq!(expand_template(&key, &variables), value, "{key}");
                count += 1;
            }
        }
        assert_eq!(count, 30);
    }
}

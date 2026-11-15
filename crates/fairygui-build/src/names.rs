use std::collections::BTreeSet;

#[derive(Debug, Default)]
pub(crate) struct Names(BTreeSet<String>);
impl Names {
    pub fn reserve(&mut self, name: &str) {
        self.0.insert(name.into());
    }
    pub fn take(&mut self, base: &str) -> String {
        if self.0.insert(base.into()) {
            return base.into();
        }
        for i in 2.. {
            let candidate = format!("{base}_{i}");
            if self.0.insert(candidate.clone()) {
                return candidate;
            }
        }
        unreachable!()
    }
}

fn words(name: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let chars: Vec<_> = name.chars().collect();
    for (i, &ch) in chars.iter().enumerate() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase()
                && !word.is_empty()
                && (chars[i - 1].is_ascii_lowercase()
                    || chars[i - 1].is_ascii_digit()
                    || chars.get(i + 1).is_some_and(char::is_ascii_lowercase))
            {
                words.push(std::mem::take(&mut word));
            }
            word.push(ch.to_ascii_lowercase());
        } else {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            if !ch.is_ascii() {
                words.push(format!("u{:x}", ch as u32));
            }
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}
fn valid(mut name: String) -> String {
    if name.is_empty() {
        name = "unnamed".into();
    }
    if name.starts_with(|ch: char| ch.is_ascii_digit()) {
        name.insert_str(0, "n_");
    }
    if matches!(
        name.as_str(),
        "as" | "async"
            | "await"
            | "break"
            | "const"
            | "continue"
            | "crate"
            | "dyn"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "abstract"
            | "become"
            | "box"
            | "do"
            | "final"
            | "gen"
            | "macro"
            | "override"
            | "priv"
            | "try"
            | "typeof"
            | "unsized"
            | "virtual"
            | "yield"
    ) {
        name.push('_');
    }
    name
}
pub(crate) fn snake(name: &str) -> String {
    valid(words(name).join("_"))
}
pub(crate) fn pascal(name: &str) -> String {
    valid(
        words(name)
            .into_iter()
            .map(|mut word| {
                word[..1].make_ascii_uppercase();
                word
            })
            .collect(),
    )
}

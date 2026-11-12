//! 多语言翻译表。
//!
//! 官方用一个外部表把界面里的文本按 key 改写：编辑器里把文本写成 `!key!`，
//! 载入后由翻译表替换成实际文案。表本身不在包内，由使用方提供，这里只负责
//! 改写规则。
//!
//! 缺翻译时**保留原样**而不是置空——否则界面会突然出现空白，
//! 保留 `!key!` 更便于定位漏翻的条目。

use std::collections::HashMap;

/// 翻译表。
#[derive(Debug, Clone, Default)]
pub struct TranslationTable {
    entries: HashMap<String, String>,
}

impl TranslationTable {
    /// 空表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 条目数量。
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 是否没有任何条目。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 登记一条翻译。重复登记会覆盖旧值。
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.entries.insert(key.into(), value.into());
    }

    /// 批量登记。
    pub fn extend<I, K, V>(&mut self, entries: I)
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        for (key, value) in entries {
            self.set(key, value);
        }
    }

    /// 取一条翻译。
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(String::as_str)
    }

    /// 是否登记了某个 key。
    pub fn contains(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }

    /// 清空全部条目。
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// 改写文本：把 `!key!` 形式的占位符替换成译文。
    pub fn translate(&self, text: &str) -> String {
        if self.entries.is_empty() || !text.contains('!') {
            return text.to_string();
        }

        let mut out = String::with_capacity(text.len());
        let mut rest = text;

        while let Some(start) = rest.find('!') {
            out.push_str(&rest[..start]);
            let after = &rest[start + 1..];

            let Some(end) = after.find('!') else {
                // 只有半个标记，原样保留剩余内容。
                out.push('!');
                out.push_str(after);
                return out;
            };

            let key = &after[..end];
            match self.get(key) {
                Some(value) => out.push_str(value),
                None => {
                    // 缺翻译时保留原样，便于定位。
                    out.push('!');
                    out.push_str(key);
                    out.push('!');
                }
            }

            rest = &after[end + 1..];
        }

        out.push_str(rest);
        out
    }

    /// 文本里是否含可替换的占位符。
    pub fn has_placeholder(text: &str) -> bool {
        let mut rest = text;
        while let Some(start) = rest.find('!') {
            let after = &rest[start + 1..];
            match after.find('!') {
                Some(end) if end > 0 => return true,
                Some(end) => rest = &after[end + 1..],
                None => return false,
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> TranslationTable {
        let mut table = TranslationTable::new();
        table.set("title", "开始游戏");
        table.set("exit", "退出");
        table
    }

    #[test]
    fn replaces_placeholders() {
        let table = table();
        assert_eq!(table.translate("!title!"), "开始游戏");
        assert_eq!(table.translate("!title! / !exit!"), "开始游戏 / 退出");
    }

    #[test]
    fn surrounding_text_is_preserved() {
        let table = table();
        assert_eq!(
            table.translate("请点击 !title! 按钮"),
            "请点击 开始游戏 按钮"
        );
    }

    #[test]
    fn missing_keys_are_kept_verbatim() {
        let table = table();
        assert_eq!(
            table.translate("!missing!"),
            "!missing!",
            "缺翻译时应当保留占位符，而不是留空"
        );
    }

    #[test]
    fn text_without_markers_is_returned_as_is() {
        let table = table();
        assert_eq!(table.translate("普通文本"), "普通文本");
        assert_eq!(table.translate(""), "");
    }

    #[test]
    fn lone_exclamation_is_preserved() {
        let table = table();
        assert_eq!(table.translate("小心!"), "小心!");
        assert_eq!(table.translate("!title"), "!title");
    }

    #[test]
    fn empty_table_is_a_passthrough() {
        let table = TranslationTable::new();
        assert!(table.is_empty());
        assert_eq!(table.translate("!title!"), "!title!", "空表不做任何替换");
    }

    #[test]
    fn placeholder_detection() {
        assert!(TranslationTable::has_placeholder("!key!"));
        assert!(TranslationTable::has_placeholder("前缀!key!后缀"));
        assert!(!TranslationTable::has_placeholder("普通文本"));
        assert!(!TranslationTable::has_placeholder("单个!"));
        assert!(!TranslationTable::has_placeholder("!!"));
    }

    #[test]
    fn setting_again_overwrites() {
        let mut table = table();
        table.set("title", "重新开始");
        assert_eq!(table.translate("!title!"), "重新开始");
        assert_eq!(table.len(), 2);
    }

    #[test]
    fn extend_adds_many() {
        let mut table = TranslationTable::new();
        table.extend([("a", "1"), ("b", "2")]);

        assert_eq!(table.translate("!a!-!b!"), "1-2");
        assert!(table.contains("a"));
    }

    #[test]
    fn clearing_restores_passthrough() {
        let mut table = table();
        table.clear();
        assert!(table.is_empty());
        assert_eq!(table.translate("!title!"), "!title!");
    }
}

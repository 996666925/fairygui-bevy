//! 包内字符串表。
//!
//! 包体把所有字符串集中存放一次，其余位置只写 2 字节索引，以压缩体积。
//! 表项允许为 `None`：官方实现中的索引 `65534` 表示 `null`，`65533` 表示空串。

/// 包内字符串表。
#[derive(Debug, Default, Clone)]
pub struct StringTable {
    entries: Vec<Option<String>>,
}

impl StringTable {
    /// 空表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 预留容量的空表。
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
        }
    }

    /// 表项数量。
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 表是否为空。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 按索引取值。越界或该槽位为 `null` 时返回 `None`。
    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).and_then(|entry| entry.as_deref())
    }

    /// 追加一项。
    pub fn push(&mut self, value: Option<String>) {
        self.entries.push(value);
    }

    /// 覆盖指定槽位。索引越界时忽略，用于「自定义字符串」对表的修补。
    pub fn set(&mut self, index: usize, value: Option<String>) {
        if index < self.entries.len() {
            self.entries[index] = value;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_returns_none_for_null_and_out_of_range() {
        let mut table = StringTable::new();
        table.push(Some("first".to_string()));
        table.push(None);
        table.push(Some(String::new()));

        assert_eq!(table.get(0), Some("first"));
        assert_eq!(table.get(1), None);
        assert_eq!(table.get(2), Some(""));
        assert_eq!(table.get(3), None);
        assert_eq!(table.len(), 3);
    }

    #[test]
    fn set_ignores_out_of_range_index() {
        let mut table = StringTable::new();
        table.push(Some("a".to_string()));
        table.set(5, Some("b".to_string()));
        assert_eq!(table.len(), 1);
        assert_eq!(table.get(0), Some("a"));
    }
}

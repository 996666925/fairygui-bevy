//! 核心层的统一错误类型。
//!
//! 解析类错误都带上出错位置，便于定位是包体的哪一块、哪个偏移出了问题。

use thiserror::Error;

/// 核心层的错误。
#[derive(Debug, Error)]
pub enum Error {
    /// 包体魔数不是 `FGUI`，通常意味着用错了格式（例如误把旧格式包当新格式解析）。
    #[error("invalid package magic: expected \"FGUI\" (0x46475549), found {found:#010x}")]
    InvalidMagic { found: u32 },

    /// 包格式版本不受支持。
    #[error("unsupported package format version {version}")]
    UnsupportedVersion { version: i32 },

    /// 读取越过数据末尾。
    #[error("truncated data at offset {offset}: need {needed} more byte(s)")]
    UnexpectedEof { offset: usize, needed: usize },

    /// 字符串不是合法 UTF-8。
    #[error("invalid utf-8 string at offset {offset}")]
    InvalidUtf8 { offset: usize },

    /// 字符串表索引越界。
    #[error("invalid string table index {index} (table length {len})")]
    InvalidStringIndex { index: usize, len: usize },

    /// 解压失败。
    #[error("failed to decompress package body: {0}")]
    Decompress(#[source] std::io::Error),

    /// 索引表里某一块声明了偏移，但块本身不存在。
    #[error("index table block {block} is missing")]
    MissingBlock { block: u8 },

    /// 找不到指定的包条目。
    #[error("package item not found: {0}")]
    ItemNotFound(String),

    /// 找不到指定的包。
    #[error("package not found: {0}")]
    PackageNotFound(String),

    /// 条目类型与预期不符。
    #[error("unexpected package item type: expected {expected}, found {found}")]
    UnexpectedItemType {
        expected: &'static str,
        found: &'static str,
    },

    /// 数据在语义上不合法（长度不一致、枚举值未知等）。
    #[error("invalid data at offset {offset}: {reason}")]
    InvalidData { offset: usize, reason: String },
}

impl Error {
    /// 构造一个语义错误，带上出错偏移。
    pub fn invalid_data(offset: usize, reason: impl Into<String>) -> Self {
        Self::InvalidData {
            offset,
            reason: reason.into(),
        }
    }
}

/// 核心层的 `Result` 别名。
pub type Result<T, E = Error> = std::result::Result<T, E>;

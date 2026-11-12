//! 二进制字节流读取。
//!
//! 对应官方运行时的 `utils/ByteBuffer`。有两处与常见习惯不同，是最容易踩坑的地方：
//!
//! 1. **大端序**：官方 `ByteBuffer` 在构造时强制 `Laya.Byte.BIG_ENDIAN`，
//!    因此包体内所有多字节整数都是网络序，而不是通常的小端。
//! 2. **索引表寻址**：`seek` 的步长不是固定的。索引表头部先有 1 字节段数，
//!    再 1 字节宽度标志决定条目为 2 字节还是 4 字节。
//!
//! 另外字符串解码是两段式的：字符串表本身用长度前缀的 UTF-8 读取，
//! 而表建立之后，其余位置一律只写 2 字节的**索引**。

mod reader;
mod string_table;

pub use reader::ByteBuffer;
pub use string_table::StringTable;

//! 大端字节流读取器，对应官方运行时的 `utils/ByteBuffer`。
//!
//! 所有读取方法都做边界检查并返回 [`Result`]，越界会给出出错偏移，
//! 而不是像官方实现那样依赖底层 `DataView` 抛异常。

use std::ops::Range;

use tracing::warn;

use crate::binary::StringTable;
use crate::error::{Error, Result};

/// 字符串表索引 `65534`，官方实现中表示 `null`。
const STRING_INDEX_NULL: u16 = 65534;
/// 字符串表索引 `65533`，官方实现中表示空串。
const STRING_INDEX_EMPTY: u16 = 65533;

/// 大端字节流读取器。
///
/// 不持有字符串表：表在解析过程中会被「自定义字符串」块修补，
/// 因此作为参数传给 [`ByteBuffer::read_s`] 一族方法，避免借用冲突。
#[derive(Debug, Clone, Copy)]
pub struct ByteBuffer<'a> {
    data: &'a [u8],
    pos: usize,
    version: i32,
}

impl<'a> ByteBuffer<'a> {
    /// 以整段数据创建读取器。
    pub const fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            version: 0,
        }
    }

    /// 包格式版本。子缓冲区（如组件布局、位图字体）会继承该版本。
    pub const fn version(&self) -> i32 {
        self.version
    }

    /// 设置包格式版本。
    pub fn set_version(&mut self, version: i32) {
        self.version = version;
    }

    /// 当前读取位置。
    pub const fn position(&self) -> usize {
        self.pos
    }

    /// 直接跳转到指定位置。
    pub fn set_position(&mut self, position: usize) {
        self.pos = position;
    }

    /// 前进若干字节。与官方一致，不做越界检查，越界会在后续读取时暴露。
    pub fn skip(&mut self, count: usize) {
        self.pos = self.pos.saturating_add(count);
    }

    /// 数据总长度。
    pub const fn total_len(&self) -> usize {
        self.data.len()
    }

    /// 剩余可读字节数。
    pub const fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    // ---- 基础读取 ----

    /// 读取 1 字节无符号整数。
    pub fn read_u8(&mut self) -> Result<u8> {
        let byte = *self.data.get(self.pos).ok_or(Error::UnexpectedEof {
            offset: self.pos,
            needed: 1,
        })?;
        self.pos += 1;
        Ok(byte)
    }

    /// 读取 1 字节有符号整数，对应官方的 `readByte()`。
    pub fn read_i8(&mut self) -> Result<i8> {
        Ok(self.read_u8()? as i8)
    }

    /// 读取 1 字节布尔值。官方只认 `1` 为真。
    pub fn read_bool(&mut self) -> Result<bool> {
        Ok(self.read_u8()? == 1)
    }

    /// 读取 2 字节无符号整数（大端）。
    pub fn read_u16(&mut self) -> Result<u16> {
        let bytes = self.read_bytes(2)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    /// 读取 2 字节有符号整数（大端）。
    pub fn read_i16(&mut self) -> Result<i16> {
        let bytes = self.read_bytes(2)?;
        Ok(i16::from_be_bytes([bytes[0], bytes[1]]))
    }

    /// 读取 4 字节无符号整数（大端）。
    pub fn read_u32(&mut self) -> Result<u32> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// 读取 4 字节有符号整数（大端）。
    pub fn read_i32(&mut self) -> Result<i32> {
        let bytes = self.read_bytes(4)?;
        Ok(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// 读取 4 字节浮点数（大端）。
    pub fn read_f32(&mut self) -> Result<f32> {
        let bytes = self.read_bytes(4)?;
        Ok(f32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// 读取 2 字节码点并转为字符，对应官方的 `readChar()`。
    pub fn read_char(&mut self) -> Result<char> {
        let code = self.read_u16()?;
        Ok(char::from_u32(u32::from(code)).unwrap_or(char::REPLACEMENT_CHARACTER))
    }

    /// 读取 `len` 个字节并借用返回。
    pub fn read_bytes(&mut self, len: usize) -> Result<&'a [u8]> {
        let start = self.pos;
        let end = start.checked_add(len).ok_or(Error::UnexpectedEof {
            offset: start,
            needed: len,
        })?;
        if end > self.data.len() {
            return Err(Error::UnexpectedEof {
                offset: start,
                needed: len,
            });
        }
        self.pos = end;
        let data = self.data;
        Ok(&data[start..end])
    }

    // ---- 字符串 ----

    /// 读取长度前缀（16 位）的 UTF-8 字符串，对应官方的 `readUTFString()`。
    ///
    /// 只用于包头部与字符串表本身；表建立之后一律用索引编码。
    pub fn read_utf_string(&mut self) -> Result<String> {
        let len = usize::from(self.read_u16()?);
        let start = self.pos;
        let bytes = self.read_bytes(len)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| Error::InvalidUtf8 { offset: start })
    }

    /// 读取字符串表索引，对应官方的 `readS()`。
    ///
    /// `65534` 映射为 `None`（官方为 `null`），`65533` 映射为空串。
    pub fn read_s(&mut self, table: &StringTable) -> Result<Option<String>> {
        let index = self.read_u16()?;
        match index {
            STRING_INDEX_NULL => Ok(None),
            STRING_INDEX_EMPTY => Ok(Some(String::new())),
            _ => {
                let index = usize::from(index);
                match table.get(index) {
                    Some(value) => Ok(Some(value.to_string())),
                    None => {
                        if index >= table.len() {
                            warn!(
                                index,
                                table_len = table.len(),
                                "string table index out of range, treated as null"
                            );
                        }
                        Ok(None)
                    }
                }
            }
        }
    }

    /// 连续读取 `count` 个字符串表索引，对应官方的 `readSArray()`。
    pub fn read_s_array(
        &mut self,
        table: &StringTable,
        count: usize,
    ) -> Result<Vec<Option<String>>> {
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            values.push(self.read_s(table)?);
        }
        Ok(values)
    }

    /// 读取自定义字符串，对应官方的 `getCustomString(len)`。
    ///
    /// `len` 的语义由首个标记字节决定，这是官方实现里比较隐晦的一点：
    ///
    /// | 标记 | 含义 | `len` 的单位 |
    /// | --- | --- | --- |
    /// | `0` | UTF-16 码点逐字符 | 字符数 + 1 |
    /// | `1` | 字符串表索引 | 不适用 |
    /// | `2` | UTF-8 字节 | 字节数 + 1 |
    pub fn read_custom_string(
        &mut self,
        len: usize,
        table: &StringTable,
    ) -> Result<Option<String>> {
        if len == 0 {
            return Ok(None);
        }

        let marker = self.read_u8()?;
        match marker {
            0 => {
                let count = len - 1;
                let mut text = String::with_capacity(count);
                for _ in 0..count {
                    text.push(self.read_char()?);
                }
                Ok(Some(text))
            }
            1 => self.read_s(table),
            2 => {
                let count = len - 1;
                let bytes = self.read_bytes(count)?;
                match std::str::from_utf8(bytes) {
                    Ok(text) => Ok(Some(text.to_string())),
                    Err(_) => {
                        warn!("custom string is not valid utf-8, replaced with lossy text");
                        Ok(Some(String::from_utf8_lossy(bytes).into_owned()))
                    }
                }
            }
            other => {
                // 未知标记无法推断长度单位，按单字节处理以尽量保持流位置正确。
                warn!(
                    marker = other,
                    "unknown custom string marker, skipping remaining bytes"
                );
                self.skip(len - 1);
                Ok(None)
            }
        }
    }

    // ---- 结构定位 ----

    /// 读取一个长度前缀（32 位）的子缓冲区，对应官方的 `readBuffer()`。
    ///
    /// 子缓冲区是原数据的借用，并继承包格式版本。
    pub fn read_buffer(&mut self) -> Result<Self> {
        let range = self.read_buffer_range()?;
        let data = self.data;
        Ok(Self {
            data: &data[range],
            pos: 0,
            version: self.version,
        })
    }

    /// 读取长度前缀（32 位）的子缓冲区，只返回它在当前缓冲区中的位置区间。
    ///
    /// 用于把「原始数据」记录为可跨线程传递的区间，而不是借用。
    pub fn read_buffer_range(&mut self) -> Result<Range<usize>> {
        let count = self.read_u32()? as usize;
        let start = self.pos;
        let end = start.checked_add(count).ok_or(Error::UnexpectedEof {
            offset: start,
            needed: count,
        })?;
        if end > self.data.len() {
            return Err(Error::UnexpectedEof {
                offset: start,
                needed: count,
            });
        }
        self.pos = end;
        Ok(start..end)
    }

    /// 把「相对当前读取位置」的偏移解析为绝对位置。
    ///
    /// 官方在条目表、精灵表、帧表、字形表里都用「先读一个偏移字段，再加上当前位置」
    /// 的方式跳过不认识的字段，但各处的偏移宽度并不一致（32 位、16 位无符号、
    /// 16 位有符号），因此这里统一接收已经拓宽为 32 位的值。
    pub(crate) fn resolve_relative(&self, offset: i32) -> Result<usize> {
        let end = self.pos as i64 + i64::from(offset);
        if end < 0 || end as u64 > self.data.len() as u64 {
            return Err(Error::invalid_data(
                self.pos,
                format!("relative offset {offset} points outside the buffer"),
            ));
        }
        Ok(end as usize)
    }

    /// 按索引表定位到第 `block_index` 块，对应官方的 `seek(indexTablePos, blockIndex)`。
    ///
    /// 索引表布局为：1 字节段数 + 1 字节宽度标志 + 各块偏移条目。
    /// 宽度标志为 `1` 时条目是 2 字节，否则是 4 字节；条目值是**相对表起始**的偏移。
    /// 返回 `Ok(false)` 表示该块不存在（偏移为 0 或索引超出段数），此时读取位置保持不变。
    pub fn seek(&mut self, index_table_pos: usize, block_index: u8) -> Result<bool> {
        let saved = self.pos;
        self.pos = index_table_pos;

        let seg_count = self.read_u8()?;
        if block_index >= seg_count {
            self.pos = saved;
            return Ok(false);
        }

        let use_short = self.read_u8()? == 1;
        let new_pos = if use_short {
            self.pos += 2 * usize::from(block_index);
            usize::from(self.read_u16()?)
        } else {
            self.pos += 4 * usize::from(block_index);
            self.read_u32()? as usize
        };

        if new_pos > 0 {
            self.pos = index_table_pos + new_pos;
            Ok(true)
        } else {
            self.pos = saved;
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table_with(entries: &[&str]) -> StringTable {
        let mut table = StringTable::new();
        for entry in entries {
            table.push(Some((*entry).to_string()));
        }
        table
    }

    #[test]
    fn integers_are_big_endian() {
        let data = [0x00, 0x00, 0x00, 0x06, 0x01, 0x02];
        let mut buffer = ByteBuffer::new(&data);
        assert_eq!(buffer.read_u32().unwrap(), 6);
        assert_eq!(buffer.read_u16().unwrap(), 0x0102);
    }

    #[test]
    fn read_bool_only_treats_one_as_true() {
        let data = [0x01, 0x02, 0x00];
        let mut buffer = ByteBuffer::new(&data);
        assert!(buffer.read_bool().unwrap());
        assert!(!buffer.read_bool().unwrap());
        assert!(!buffer.read_bool().unwrap());
    }

    #[test]
    fn utf_string_uses_16_bit_length_prefix() {
        // 长度 0x0008 后面跟 "rbw1tv9t"
        let mut data = vec![0x00, 0x08];
        data.extend_from_slice(b"rbw1tv9t");
        let mut buffer = ByteBuffer::new(&data);
        assert_eq!(buffer.read_utf_string().unwrap(), "rbw1tv9t");
        assert_eq!(buffer.remaining(), 0);
    }

    #[test]
    fn read_s_maps_sentinel_indices() {
        let table = table_with(&["alpha", "beta"]);
        // 0 -> "alpha"，65533 -> 空串，65534 -> null
        let data = [0x00, 0x00, 0xFF, 0xFD, 0xFF, 0xFE];
        let mut buffer = ByteBuffer::new(&data);
        assert_eq!(buffer.read_s(&table).unwrap().as_deref(), Some("alpha"));
        assert_eq!(buffer.read_s(&table).unwrap().as_deref(), Some(""));
        assert_eq!(buffer.read_s(&table).unwrap(), None);
    }

    #[test]
    fn read_s_out_of_range_becomes_none() {
        let table = table_with(&["alpha"]);
        let data = [0x00, 0x05];
        let mut buffer = ByteBuffer::new(&data);
        assert_eq!(buffer.read_s(&table).unwrap(), None);
    }

    #[test]
    fn read_buffer_is_a_borrowed_view_and_skips_parent() {
        // [len=3]["abc"] 后面跟 0xFF
        let data = [0x00, 0x00, 0x00, 0x03, b'a', b'b', b'c', 0xFF];
        let mut buffer = ByteBuffer::new(&data);
        let mut sub = buffer.read_buffer().unwrap();
        assert_eq!(sub.total_len(), 3);
        assert_eq!(sub.read_u8().unwrap(), b'a');
        assert_eq!(buffer.read_u8().unwrap(), 0xFF);
    }

    #[test]
    fn read_buffer_inherits_version() {
        let data = [0x00, 0x00, 0x00, 0x00];
        let mut buffer = ByteBuffer::new(&data);
        buffer.set_version(6);
        let sub = buffer.read_buffer().unwrap();
        assert_eq!(sub.version(), 6);
    }

    #[test]
    fn seek_resolves_short_and_int_entries() {
        // segCount=3, useShort=1
        // 块 0 -> 10, 块 1 -> 20, 块 2 -> 0（不存在）
        let mut data = vec![3, 1];
        data.extend_from_slice(&[0x00, 0x0A]);
        data.extend_from_slice(&[0x00, 0x14]);
        data.extend_from_slice(&[0x00, 0x00]);
        data.resize(32, 0);

        let mut buffer = ByteBuffer::new(&data);
        assert!(buffer.seek(0, 0).unwrap());
        assert_eq!(buffer.position(), 10);
        assert!(buffer.seek(0, 1).unwrap());
        assert_eq!(buffer.position(), 20);
        assert!(!buffer.seek(0, 2).unwrap());
        assert_eq!(buffer.position(), 20, "失败的定位不应改变读取位置");
    }

    #[test]
    fn seek_handles_int_entries_and_index_beyond_segment_count() {
        // segCount=2, useShort=0, 块 1 -> 16
        let mut data = vec![2, 0];
        data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
        data.extend_from_slice(&[0x00, 0x00, 0x00, 0x10]);
        data.resize(32, 0);

        let mut buffer = ByteBuffer::new(&data);
        assert!(!buffer.seek(0, 0).unwrap(), "偏移为 0 视为不存在");
        assert!(buffer.seek(0, 1).unwrap());
        assert_eq!(buffer.position(), 16);
        assert!(!buffer.seek(0, 2).unwrap(), "索引超出段数");
    }

    #[test]
    fn seek_with_zero_segments_does_not_read_width_flag() {
        let data = [0x00, 0xAA, 0xBB];
        let mut buffer = ByteBuffer::new(&data);
        assert!(!buffer.seek(0, 0).unwrap());
        assert_eq!(buffer.position(), 0);
    }

    #[test]
    fn custom_string_markers() {
        let table = table_with(&["shared"]);

        // marker=1：走字符串表索引
        let data = [1, 0x00, 0x00];
        let mut buffer = ByteBuffer::new(&data);
        assert_eq!(
            buffer.read_custom_string(3, &table).unwrap().as_deref(),
            Some("shared")
        );

        // marker=2：UTF-8，len 为 字节数 + 1
        let mut data = vec![2];
        data.extend_from_slice("字体".as_bytes());
        let mut buffer = ByteBuffer::new(&data);
        assert_eq!(
            buffer
                .read_custom_string(data.len(), &table)
                .unwrap()
                .as_deref(),
            Some("字体")
        );

        // marker=0：UTF-16 码点，"AB" 共 2 个字符，len 为 字符数 + 1
        let data = [0, 0x00, 0x41, 0x00, 0x42];
        let mut buffer = ByteBuffer::new(&data);
        assert_eq!(
            buffer.read_custom_string(3, &table).unwrap().as_deref(),
            Some("AB")
        );
    }

    #[test]
    fn truncated_input_reports_offset() {
        let data = [0x00, 0x01];
        let mut buffer = ByteBuffer::new(&data);
        let err = buffer.read_u32().unwrap_err();
        assert!(matches!(
            err,
            Error::UnexpectedEof {
                offset: 0,
                needed: 4
            }
        ));
    }
}

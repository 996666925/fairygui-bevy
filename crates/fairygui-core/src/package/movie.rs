//! 序列帧（影片剪辑）解析。
//!
//! 与位图字体类似，序列帧条目的原始数据也是「索引表 + 两块」：
//! 块 0 是播放参数，块 1 是帧表，每帧指向精灵表里的一个切片。

use tracing::warn;

use crate::error::{Error, Result};
use crate::geom::{Point, Rect, Size};
use crate::package::{FuiPackage, LoadContext, PackageItemType};

/// 帧的纹理来源。
#[derive(Debug, Clone, PartialEq)]
pub struct FrameTexture {
    /// 图集条目在包内的下标。
    pub atlas_item: usize,
    /// 在图集中的矩形。
    pub rect: Rect,
    /// 相对原图左上角的偏移。
    pub offset: Point,
    /// 原图尺寸，用于还原播放区域。
    pub original_size: Size,
}

/// 序列帧的单帧。
#[derive(Debug, Clone)]
pub struct MovieFrame {
    /// 相对上一帧的额外延迟（毫秒）。
    pub add_delay: i32,
    /// 帧引用的图片条目编号。为空表示该帧本来就没有图。
    pub sprite_id: Option<String>,
    /// 纹理来源。为空但 [`Self::sprite_id`] 不为空时，说明引用的资源缺失。
    pub texture: Option<FrameTexture>,
}

/// 序列帧数据。
#[derive(Debug, Clone)]
pub struct MovieClipData {
    /// 帧间隔（毫秒）。
    pub interval: i32,
    /// 是否往复播放。
    pub swing: bool,
    /// 循环前的延迟（毫秒）。
    pub repeat_delay: i32,
    /// 帧列表。
    pub frames: Vec<MovieFrame>,
}

impl MovieClipData {
    /// 帧数量。
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// 取指定帧，超出范围时按循环取模。
    pub fn frame(&self, index: usize) -> Option<&MovieFrame> {
        if self.frames.is_empty() {
            None
        } else {
            self.frames.get(index % self.frames.len())
        }
    }
}

impl FuiPackage {
    /// 解析序列帧条目。
    pub fn load_movie_clip(&self, item_index: usize, ctx: &LoadContext) -> Result<MovieClipData> {
        let index = self.resolve_branch(item_index, ctx);
        let item = self
            .item(index)
            .ok_or_else(|| Error::ItemNotFound(format!("#{item_index}")))?;
        item.expect_type(PackageItemType::MovieClip)?;

        let mut buffer = self
            .raw_data(item)
            .ok_or_else(|| Error::invalid_data(0, "movie clip item has no raw data"))?;

        // ---- 块 0：播放参数 ----
        if !buffer.seek(0, 0)? {
            return Err(Error::MissingBlock { block: 0 });
        }
        let interval = buffer.read_i32()?;
        let swing = buffer.read_bool()?;
        let repeat_delay = buffer.read_i32()?;

        // ---- 块 1：帧表 ----
        if !buffer.seek(0, 1)? {
            return Err(Error::MissingBlock { block: 1 });
        }

        let table = self.string_table();
        let frame_count = buffer.read_i16()?;
        let mut frames = Vec::with_capacity(frame_count.max(0) as usize);

        for _ in 0..frame_count {
            // 帧表的偏移也是 16 位有符号。
            let end_offset = i32::from(buffer.read_i16()?);
            let entry_end = buffer.resolve_relative(end_offset)?;

            let fx = buffer.read_i32()? as f32;
            let fy = buffer.read_i32()? as f32;
            buffer.read_i32()?; // 宽，官方读后丢弃
            buffer.read_i32()?; // 高，官方读后丢弃
            let add_delay = buffer.read_i32()?;
            let sprite_id = buffer.read_s(table)?;

            let texture = sprite_id
                .as_deref()
                .and_then(|id| match self.sprite_by_id(id) {
                    Some(sprite) => Some(FrameTexture {
                        atlas_item: sprite.atlas_item,
                        rect: sprite.rect,
                        offset: Point::new(fx, fy),
                        original_size: Size::new(item.width, item.height),
                    }),
                    None => {
                        warn!(sprite_id = id, "movie clip frame item has no atlas sprite");
                        None
                    }
                });

            frames.push(MovieFrame {
                add_delay,
                sprite_id,
                texture,
            });
            buffer.set_position(entry_end);
        }

        Ok(MovieClipData {
            interval,
            swing,
            repeat_delay,
            frames,
        })
    }
}

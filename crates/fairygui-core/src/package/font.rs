//! 位图字体解析。
//!
//! 字体条目的原始数据本身又是一个「索引表 + 两块」结构：
//! 块 0 是字体头（字号、行高、字形来源等），块 1 是字形表。
//!
//! 字形的图有两种来源：包内标记为内嵌字体时，字形从字体自带的图集里裁剪；
//! 否则每个字形指向一个独立的图片条目。

use std::collections::HashMap;

use tracing::warn;

use crate::error::{Error, Result};
use crate::geom::Rect;
use crate::package::{FuiPackage, LoadContext, PackageItemType};

/// 字形的纹理来源。
#[derive(Debug, Clone, PartialEq)]
pub enum GlyphTexture {
    /// 从字体自带图集中裁剪，矩形已是图集内的绝对位置。
    FontAtlas {
        /// 图集条目在包内的下标。
        atlas_item: usize,
        /// 在图集中的矩形。
        rect: Rect,
    },
    /// 使用另一个图片条目的纹理。
    Item {
        /// 图片条目在包内的下标。
        item: usize,
    },
}

/// 位图字体的单个字形。
#[derive(Debug, Clone)]
pub struct BmGlyph {
    /// 字符码点。
    pub char_code: u32,
    /// 相对基线的横向偏移。
    pub x: f32,
    /// 相对基线的纵向偏移。
    pub y: f32,
    /// 字形宽度。
    pub width: f32,
    /// 字形高度。
    pub height: f32,
    /// 步进宽度。
    pub advance: f32,
    /// 纹理来源。为空表示该字形没有图（例如空格）。
    pub texture: Option<GlyphTexture>,
}

/// 位图字体数据。
#[derive(Debug, Clone)]
pub struct BitmapFontData {
    /// 字形是否由字体自带的图集承载。
    pub ttf: bool,
    /// 是否按指定颜色着色。
    pub tint: bool,
    /// 是否随字号缩放。
    pub auto_scale_size: bool,
    /// 设计字号。
    pub font_size: i32,
    /// 默认步进宽度，为 0 时按字形自身宽度推算。
    pub x_advance: f32,
    /// 行高。
    pub line_height: i32,
    /// 字形表，按码点索引。
    pub glyphs: HashMap<u32, BmGlyph>,
}

impl BitmapFontData {
    /// 按码点取字形。
    pub fn glyph(&self, char_code: u32) -> Option<&BmGlyph> {
        self.glyphs.get(&char_code)
    }

    /// Native design-size width: missing UTF-16 glyphs have zero advance.
    pub fn measure_width(&self, text: &str) -> f32 {
        self.measure_width_at(text, self.font_size)
    }

    /// BitmapFont.getTextWidth: round each scaled advance independently.
    pub fn measure_width_at(&self, text: &str, font_size: i32) -> f32 {
        let scale = if self.auto_scale_size {
            f64::from(font_size) / f64::from(self.font_size)
        } else {
            1.0
        };
        text.encode_utf16()
            .filter_map(|code| self.glyph(u32::from(code)))
            .map(|g| (f64::from(g.advance) * scale + 0.5).floor())
            .sum::<f64>() as f32
    }
}

impl FuiPackage {
    /// 解析位图字体条目。
    pub fn load_bitmap_font(&self, item_index: usize, ctx: &LoadContext) -> Result<BitmapFontData> {
        let index = self.resolve_branch(item_index, ctx);
        let item = self
            .item(index)
            .ok_or_else(|| Error::ItemNotFound(format!("#{item_index}")))?;
        item.expect_type(PackageItemType::Font)?;

        let mut buffer = self
            .raw_data(item)
            .ok_or_else(|| Error::invalid_data(0, "bitmap font item has no raw data"))?;

        // ---- 块 0：字体头 ----
        if !buffer.seek(0, 0)? {
            return Err(Error::MissingBlock { block: 0 });
        }
        let ttf = buffer.read_bool()?;
        let tint = buffer.read_bool()?;
        let auto_scale_size = buffer.read_bool()?;
        buffer.read_bool()?; // has channel，无实际用途
        let font_size = buffer.read_i32()?.max(1);
        let x_advance = buffer.read_i32()? as f32;
        let line_height = buffer.read_i32()?.max(font_size);

        // 内嵌字体的字形要从字体条目自己的图集切片里裁剪。
        let main_sprite = self.sprite_for(index).cloned();

        // ---- 块 1：字形表 ----
        if !buffer.seek(0, 1)? {
            return Err(Error::MissingBlock { block: 1 });
        }

        let table = self.string_table();
        let count = buffer.read_i32()?.max(0) as usize;
        let mut glyphs = HashMap::with_capacity(count);

        for _ in 0..count {
            // 字形表的偏移是 16 位有符号。
            let end_offset = i32::from(buffer.read_i16()?);
            let entry_end = buffer.resolve_relative(end_offset)?;

            let char_code = u32::from(buffer.read_u16()?);
            let image_id = buffer.read_s(table)?.unwrap_or_default();
            let bx = buffer.read_i32()? as f32;
            let by = buffer.read_i32()? as f32;
            let x = buffer.read_i32()? as f32;
            let y = buffer.read_i32()? as f32;
            let mut width = buffer.read_i32()? as f32;
            let mut height = buffer.read_i32()? as f32;
            let mut advance = buffer.read_i32()? as f32;
            buffer.read_i8()?; // channel，无实际用途

            let texture = if ttf {
                main_sprite.as_ref().map(|sprite| GlyphTexture::FontAtlas {
                    atlas_item: sprite.atlas_item,
                    rect: Rect::new(sprite.rect.x + bx, sprite.rect.y + by, width, height),
                })
            } else {
                let resolved = self
                    .item_index_by_id(&image_id)
                    .map(|index| self.resolve_branch(index, ctx));

                match resolved {
                    Some(char_index) => {
                        if let Some(char_item) = self.item(char_index) {
                            // 非内嵌模式下，字形尺寸以图片条目为准。
                            width = char_item.width;
                            height = char_item.height;
                        }
                        Some(GlyphTexture::Item {
                            item: self.resolve_high_resolution(char_index, ctx),
                        })
                    }
                    None => {
                        if !image_id.is_empty() {
                            warn!(image_id, "bitmap font glyph references an unknown item");
                        }
                        None
                    }
                }
            };

            // 非内嵌模式下包内步进可能为 0，此时用默认步进或字形自身宽度补齐。
            if !ttf && advance == 0.0 {
                advance = if x_advance == 0.0 {
                    x + width
                } else {
                    x_advance
                };
            }

            glyphs.insert(
                char_code,
                BmGlyph {
                    char_code,
                    x,
                    y,
                    width,
                    height,
                    advance,
                    texture,
                },
            );

            buffer.set_position(entry_end);
        }

        Ok(BitmapFontData {
            ttf,
            tint,
            auto_scale_size,
            font_size,
            x_advance,
            line_height,
            glyphs,
        })
    }
}

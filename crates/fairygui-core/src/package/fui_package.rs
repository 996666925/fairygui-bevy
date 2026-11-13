//! `.fui` 包的解析。
//!
//! 包体布局为「20 字节之后的头部 + 索引表 + 六个数据块」：
//!
//! ```text
//! 魔数(4) 版本(4) 压缩标志(1) 包编号(变长) 包名(变长) 保留区(20)
//! ── 索引表 ── 段数(1) 宽度标志(1) 各块偏移(2 或 4 字节 × 段数)
//! ── 数据块 ── 0 依赖与分支 / 1 包条目 / 2 精灵表 / 3 像素命中 / 4 字符串表 / 5 自定义字符串
//! ```
//!
//! 压缩包只在头部之后压缩，因此解压得到的缓冲区对应「索引表 + 数据块」，
//! 其索引表起点为 0；未压缩时索引表起点紧跟在头部之后。

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use tracing::warn;

use crate::binary::{ByteBuffer, StringTable};
use crate::error::{Error, Result};
use crate::geom::{Point, Rect};
use crate::package::item::{
    AtlasSprite, ObjectType, PackageItem, PackageItemType, PixelHitTestData,
};
use crate::scroll::ScrollDefaults;

/// 包体魔数，字节序列为 `F G U I`。
const PACKAGE_MAGIC: u32 = 0x46475549;

/// 索引表内的块编号。编号由官方 `seek` 调用硬编码，与实际存储顺序无关。
mod block {
    pub const DEPENDENCIES: u8 = 0;
    pub const ITEMS: u8 = 1;
    pub const SPRITES: u8 = 2;
    pub const PIXEL_HIT_TEST: u8 = 3;
    pub const STRING_TABLE: u8 = 4;
    pub const CUSTOM_STRINGS: u8 = 5;
}

/// 加载上下文。
///
/// 分支与内容缩放等级会影响资源的实际取用，因此解析依赖资源的接口都要带上它。
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LoadContext {
    /// 当前分支编号，`-1` 表示未启用分支。
    pub branch_index: i32,
    /// 内容缩放等级，`0` 表示标准分辨率。
    pub content_scale_level: i32,
    /// FairyGUI global fallbacks used by scroll panes with no exported override.
    pub scroll_defaults: ScrollDefaults,
}

impl LoadContext {
    /// 不做任何重定向的上下文。
    pub const fn none() -> Self {
        Self {
            branch_index: -1,
            content_scale_level: 0,
            scroll_defaults: ScrollDefaults::new(),
        }
    }
}

/// 包之间的依赖。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDependency {
    /// 被依赖包的编号。
    pub id: String,
    /// 被依赖包的名称。
    pub name: String,
}

/// 解析后的 `.fui` 包。
///
/// 这是纯数据：不借用输入缓冲，也不含运行时状态，可以安全地跨线程传递。
#[derive(Debug, Clone)]
pub struct FuiPackage {
    id: String,
    name: String,
    version: i32,
    res_key: String,
    data: Arc<[u8]>,
    string_table: StringTable,
    items: Vec<PackageItem>,
    items_by_id: HashMap<String, usize>,
    items_by_name: HashMap<String, usize>,
    sprites: Vec<Option<AtlasSprite>>,
    sprites_by_id: HashMap<String, AtlasSprite>,
    dependencies: Vec<PackageDependency>,
    branches: Vec<String>,
}

/// 解析中途收集的数据，用于在借用结束后组装 [`FuiPackage`]。
struct ParsedBody {
    string_table: StringTable,
    items: Vec<PackageItem>,
    items_by_id: HashMap<String, usize>,
    items_by_name: HashMap<String, usize>,
    sprites: Vec<Option<AtlasSprite>>,
    sprites_by_id: HashMap<String, AtlasSprite>,
    dependencies: Vec<PackageDependency>,
    branches: Vec<String>,
}

impl FuiPackage {
    /// 解析包体。
    ///
    /// `res_key` 是加载该包时使用的资源键，用于拼接图集与声音的文件名前缀。
    pub fn parse(res_key: impl Into<String>, data: &[u8]) -> Result<Self> {
        let res_key = res_key.into();

        let mut header = ByteBuffer::new(data);
        let magic = header.read_u32()?;
        if magic != PACKAGE_MAGIC {
            return Err(Error::InvalidMagic { found: magic });
        }

        let version = header.read_i32()?;
        let compressed = header.read_bool()?;
        let id = header.read_utf_string()?;
        let name = header.read_utf_string()?;
        header.skip(20);

        let header_end = header.position();

        let (body, index_table_pos): (Arc<[u8]>, usize) = if compressed {
            let inflated = inflate(&data[header_end..])?;
            (Arc::from(inflated.into_boxed_slice()), 0)
        } else {
            (Arc::from(data), header_end)
        };

        let parsed = parse_body(&res_key, &body, index_table_pos, version)?;

        Ok(Self {
            id,
            name,
            version,
            res_key,
            data: body,
            string_table: parsed.string_table,
            items: parsed.items,
            items_by_id: parsed.items_by_id,
            items_by_name: parsed.items_by_name,
            sprites: parsed.sprites,
            sprites_by_id: parsed.sprites_by_id,
            dependencies: parsed.dependencies,
            branches: parsed.branches,
        })
    }

    /// 包编号。
    pub fn id(&self) -> &str {
        &self.id
    }

    /// 包名。
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 包格式版本。
    pub const fn version(&self) -> i32 {
        self.version
    }

    /// 加载时使用的资源键。
    pub fn res_key(&self) -> &str {
        &self.res_key
    }

    /// 包体数据。压缩包返回的是解压后的数据。
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// 包内字符串表。
    pub fn string_table(&self) -> &StringTable {
        &self.string_table
    }

    /// 全部条目。
    pub fn items(&self) -> &[PackageItem] {
        &self.items
    }

    /// 条目数量。
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// 按下标取条目。
    pub fn item(&self, index: usize) -> Option<&PackageItem> {
        self.items.get(index)
    }

    /// 按编号取条目。
    pub fn item_by_id(&self, id: &str) -> Option<&PackageItem> {
        self.items_by_id
            .get(id)
            .and_then(|&index| self.items.get(index))
    }

    /// 按名称取条目。
    pub fn item_by_name(&self, name: &str) -> Option<&PackageItem> {
        self.items_by_name
            .get(name)
            .and_then(|&index| self.items.get(index))
    }

    /// 按编号取条目下标。
    pub fn item_index_by_id(&self, id: &str) -> Option<usize> {
        self.items_by_id.get(id).copied()
    }

    /// 从本包内解析 `ui://` 地址，返回条目下标。
    ///
    /// 两种形式都支持：`ui://<包名>/<条目名>` 与 `ui://<包编号><条目编号>`
    /// （编号固定 8 个字符）。指向**其他包**的地址需要运行时注册表才能解析，
    /// 这里返回 `None`——滚动条一类的资源引用绝大多数是同包内的。
    pub fn resolve_local_url(&self, url: &str) -> Option<usize> {
        let rest = url.strip_prefix("ui://")?;

        match rest.find('/') {
            Some(slash) => {
                if &rest[..slash] != self.name() {
                    return None;
                }
                self.item_index_by_name(&rest[slash + 1..])
            }
            None => {
                if rest.len() <= 8 || rest.get(..8)? != self.id() {
                    return None;
                }
                self.item_index_by_id(&rest[8..])
            }
        }
    }

    /// 按名称取条目下标。
    pub fn item_index_by_name(&self, name: &str) -> Option<usize> {
        self.items_by_name.get(name).copied()
    }

    /// 取图片条目的图集切片信息。返回 `None` 表示该图片没有可渲染的切片。
    pub fn sprite_for(&self, index: usize) -> Option<&AtlasSprite> {
        self.sprites.get(index).and_then(Option::as_ref)
    }

    /// 按精灵编号取切片。序列帧切片不一定对应独立的包条目。
    pub fn sprite_by_id(&self, id: &str) -> Option<&AtlasSprite> {
        self.sprites_by_id.get(id)
    }

    /// 依赖包列表。
    pub fn dependencies(&self) -> &[PackageDependency] {
        &self.dependencies
    }

    /// 分支名列表。
    pub fn branches(&self) -> &[String] {
        &self.branches
    }

    /// 分支名对应的编号，找不到时返回 `-1`（与官方 `indexOf` 的行为一致）。
    pub fn branch_index_of(&self, branch: &str) -> i32 {
        self.branches
            .iter()
            .position(|candidate| candidate == branch)
            .map_or(-1, |index| index as i32)
    }

    /// 取条目原始数据（组件布局、位图字体、序列帧）的可读视图。
    pub fn raw_data<'a>(&'a self, item: &PackageItem) -> Option<ByteBuffer<'a>> {
        let range: Range<usize> = item.raw_data.clone()?;
        let mut buffer = ByteBuffer::new(&self.data[range]);
        buffer.set_version(self.version);
        Some(buffer)
    }

    /// 应用分支重定向，得到实际生效的条目下标。
    pub fn resolve_branch(&self, index: usize, ctx: &LoadContext) -> usize {
        let Some(item) = self.item(index) else {
            return index;
        };
        let branch_id = item.branch_id(ctx.branch_index);
        if branch_id == item.id {
            return index;
        }
        self.item_index_by_id(branch_id).unwrap_or(index)
    }

    /// 应用高清重定向，得到实际生效的条目下标。
    pub fn resolve_high_resolution(&self, index: usize, ctx: &LoadContext) -> usize {
        let Some(item) = self.item(index) else {
            return index;
        };
        let hd_id = item.high_resolution_id(ctx.content_scale_level);
        if hd_id == item.id {
            return index;
        }
        self.item_index_by_id(hd_id).unwrap_or(index)
    }

    /// 依次应用分支与高清重定向。
    pub fn resolve(&self, index: usize, ctx: &LoadContext) -> usize {
        self.resolve_high_resolution(self.resolve_branch(index, ctx), ctx)
    }
}

fn parse_body(
    res_key: &str,
    body: &[u8],
    index_table_pos: usize,
    version: i32,
) -> Result<ParsedBody> {
    let mut buffer = ByteBuffer::new(body);
    buffer.set_version(version);
    buffer.set_position(index_table_pos);

    // 只有格式 2 及以上才有分支与高清机制，其余差异靠「块是否存在」探测。
    let ver2 = version >= 2;

    // ---- 块 4：字符串表 ----
    // 此块必须先解析：其余各块里的字符串都只是它的索引。
    if !buffer.seek(index_table_pos, block::STRING_TABLE)? {
        return Err(Error::MissingBlock {
            block: block::STRING_TABLE,
        });
    }
    let count = buffer.read_i32()?.max(0) as usize;
    let mut string_table = StringTable::with_capacity(count);
    for _ in 0..count {
        // 此处字符串表尚未生效，因此读的是长度前缀的原文。
        string_table.push(Some(buffer.read_utf_string()?));
    }

    // ---- 块 5：自定义字符串 ----
    // 用索引覆盖表中对应的槽位，承载超长字符串与需要按字符编码的内容。
    if buffer.seek(index_table_pos, block::CUSTOM_STRINGS)? {
        let count = buffer.read_i32()?.max(0) as usize;
        for _ in 0..count {
            let index = usize::from(buffer.read_u16()?);
            let len = buffer.read_i32()?.max(0) as usize;
            let value = buffer.read_custom_string(len, &string_table)?;
            string_table.set(index, value);
        }
    }

    // ---- 块 0：依赖与分支 ----
    let mut dependencies = Vec::new();
    let mut branches = Vec::new();
    let mut branch_included = false;

    if buffer.seek(index_table_pos, block::DEPENDENCIES)? {
        let count = buffer.read_i16()?;
        for _ in 0..count {
            let id = buffer.read_s(&string_table)?.unwrap_or_default();
            let name = buffer.read_s(&string_table)?.unwrap_or_default();
            dependencies.push(PackageDependency { id, name });
        }

        if ver2 {
            let count = buffer.read_i16()?;
            if count > 0 {
                branches = buffer
                    .read_s_array(&string_table, count as usize)?
                    .into_iter()
                    .map(Option::unwrap_or_default)
                    .collect();
                branch_included = true;
            }
        }
    } else {
        warn!("package has no dependency block, treated as having none");
    }

    // ---- 块 1：包条目 ----
    if !buffer.seek(index_table_pos, block::ITEMS)? {
        return Err(Error::MissingBlock {
            block: block::ITEMS,
        });
    }

    // 图集与声音的文件名要带上包名前缀；骨骼动画用的是目录前缀。
    let path_prefix = format!("{res_key}_");
    let short_path = match res_key.rfind('/') {
        Some(pos) => &res_key[..=pos],
        None => "",
    };

    let mut items: Vec<PackageItem> = Vec::new();
    let mut items_by_id: HashMap<String, usize> = HashMap::new();
    let mut items_by_name: HashMap<String, usize> = HashMap::new();

    let count = buffer.read_u16()?;
    for _ in 0..count {
        let end_offset = buffer.read_i32()?;
        let entry_end = buffer.resolve_relative(end_offset)?;

        let item_type = PackageItemType::from_byte(buffer.read_i8()?);
        let id = buffer.read_s(&string_table)?.unwrap_or_default();
        let name = buffer.read_s(&string_table)?.unwrap_or_default();
        buffer.read_s(&string_table)?; // path，已废弃
        let file = buffer.read_s(&string_table)?;
        buffer.read_bool()?; // exported，无实际用途
        let width = buffer.read_i32()? as f32;
        let height = buffer.read_i32()? as f32;

        let mut item = PackageItem::new(id, name, item_type);
        item.file = file;
        item.width = width;
        item.height = height;

        match item_type {
            PackageItemType::Image => {
                item.object_type = ObjectType::Image;
                match buffer.read_i8()? {
                    1 => {
                        let x = buffer.read_i32()? as f32;
                        let y = buffer.read_i32()? as f32;
                        let w = buffer.read_i32()? as f32;
                        let h = buffer.read_i32()? as f32;
                        item.scale9_grid = Some(Rect::new(x, y, w, h));
                        item.tile_grid_indice = Some(buffer.read_i32()?);
                    }
                    2 => item.scale_by_tile = true,
                    _ => {}
                }
                item.smoothing = buffer.read_bool()?;
            }
            PackageItemType::MovieClip => {
                item.smoothing = buffer.read_bool()?;
                item.object_type = ObjectType::MovieClip;
                item.raw_data = Some(buffer.read_buffer_range()?);
            }
            PackageItemType::Font => {
                item.raw_data = Some(buffer.read_buffer_range()?);
            }
            PackageItemType::Component => {
                let extension = buffer.read_i8()?;
                item.object_type = if extension > 0 {
                    ObjectType::from_code(extension as u8)
                } else {
                    ObjectType::Component
                };
                item.raw_data = Some(buffer.read_buffer_range()?);
            }
            PackageItemType::Atlas | PackageItemType::Sound | PackageItemType::Misc => {
                item.file = item.file.map(|file| format!("{path_prefix}{file}"));
            }
            PackageItemType::Spine | PackageItemType::DragonBones => {
                item.file = item.file.map(|file| format!("{short_path}{file}"));
                let x = buffer.read_f32()?;
                let y = buffer.read_f32()?;
                item.skeleton_anchor = Some(Point::new(x, y));
            }
            PackageItemType::Swf | PackageItemType::Unknown => {}
        }

        if ver2 {
            // 分支条目的名称会被加上 `分支名/` 前缀。
            if let Some(branch) = buffer.read_s(&string_table)? {
                item.name = format!("{branch}/{}", item.name);
            }

            let branch_count = buffer.read_u8()?;
            if branch_count > 0 {
                if branch_included {
                    item.branches =
                        Some(buffer.read_s_array(&string_table, usize::from(branch_count))?);
                } else {
                    // 包里只记了单个分支编号，官方把它登记为一个指向同一条目的别名。
                    let alias = buffer.read_s(&string_table)?.unwrap_or_default();
                    items_by_id.insert(alias, items.len());
                }
            }

            let high_res_count = buffer.read_u8()?;
            if high_res_count > 0 {
                item.high_resolution =
                    Some(buffer.read_s_array(&string_table, usize::from(high_res_count))?);
            }
        }

        let index = items.len();
        items_by_id.insert(item.id.clone(), index);
        if !item.name.is_empty() {
            items_by_name.insert(item.name.clone(), index);
        }
        items.push(item);

        buffer.set_position(entry_end);
    }

    // ---- 块 2：精灵表 ----
    // 把图片条目映射到图集条目内的矩形，是图集零拷贝渲染的依据。
    let mut sprites: Vec<Option<AtlasSprite>> = vec![None; items.len()];
    let mut sprites_by_id = HashMap::new();

    if buffer.seek(index_table_pos, block::SPRITES)? {
        let count = buffer.read_u16()?;
        for _ in 0..count {
            // 注意：这一块的偏移是 16 位无符号，与其它块都不同。
            let end_offset = i32::from(buffer.read_u16()?);
            let entry_end = buffer.resolve_relative(end_offset)?;

            let image_id = buffer.read_s(&string_table)?.unwrap_or_default();
            let atlas_id = buffer.read_s(&string_table)?.unwrap_or_default();

            let x = buffer.read_i32()? as f32;
            let y = buffer.read_i32()? as f32;
            let width = buffer.read_i32()? as f32;
            let height = buffer.read_i32()? as f32;
            let rotated = buffer.read_bool()?;

            let (offset, original_size) = if ver2 && buffer.read_bool()? {
                let ox = buffer.read_i32()? as f32;
                let oy = buffer.read_i32()? as f32;
                let ow = buffer.read_i32()? as f32;
                let oh = buffer.read_i32()? as f32;
                (Point::new(ox, oy), Point::new(ow, oh))
            } else {
                // 旧格式没有独立记录，原尺寸即切片尺寸。
                (Point::ZERO, Point::new(width, height))
            };

            match items_by_id.get(&atlas_id).copied() {
                Some(atlas_item) => {
                    let sprite = AtlasSprite {
                        atlas_item,
                        rect: Rect::new(x, y, width, height),
                        offset,
                        original_size,
                        rotated,
                    };
                    if let Some(&image_index) = items_by_id.get(&image_id) {
                        sprites[image_index] = Some(sprite.clone());
                    }
                    sprites_by_id.insert(image_id, sprite);
                }
                _ => warn!(
                    image_id,
                    atlas_id, "sprite entry references an unknown item, skipped"
                ),
            }

            buffer.set_position(entry_end);
        }
    } else {
        warn!("package has no sprite block");
    }

    // ---- 块 3：像素命中测试 ----
    if buffer.seek(index_table_pos, block::PIXEL_HIT_TEST)? {
        let count = buffer.read_u16()?;
        for _ in 0..count {
            let end_offset = buffer.read_i32()?;
            let entry_end = buffer.resolve_relative(end_offset)?;

            let id = buffer.read_s(&string_table)?.unwrap_or_default();
            if let Some(&index) = items_by_id.get(&id)
                && items[index].item_type == PackageItemType::Image
            {
                items[index].pixel_hit_test_data = Some(PixelHitTestData::load(&mut buffer)?);
            }

            buffer.set_position(entry_end);
        }
    }

    Ok(ParsedBody {
        string_table,
        items,
        items_by_id,
        items_by_name,
        sprites,
        sprites_by_id,
        dependencies,
        branches,
    })
}

/// 解压包体。
///
/// 官方使用的是 Laya 的 `Zlib.RawInflate`，不同版本对「是否带 zlib 容器头」
/// 存在差异，因此先按 zlib 容器解，失败再按裸 deflate 解。
fn inflate(data: &[u8]) -> Result<Vec<u8>> {
    use std::io::Read;

    let mut decoder = flate2::read::ZlibDecoder::new(data);
    let mut out = Vec::new();
    match decoder.read_to_end(&mut out) {
        Ok(_) => return Ok(out),
        Err(err) => {
            tracing::debug!(%err, "zlib container decode failed, retrying as raw deflate");
        }
    }

    out.clear();
    let mut decoder = flate2::read::DeflateDecoder::new(data);
    decoder.read_to_end(&mut out).map_err(Error::Decompress)?;
    Ok(out)
}

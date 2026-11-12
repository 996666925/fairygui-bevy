//! 包条目：包内单个资源的描述。
//!
//! 官方实现用一个类承载所有类型的资源，字段按类型复用。这里保持一致，
//! 但把「原始数据」表示为在包数据中的位置区间而非借用，以保证解析产物可跨线程传递。

use std::ops::Range;

use crate::binary::ByteBuffer;
use crate::error::{Error, Result};
use crate::geom::{Point, Rect};

/// 包条目的大类，对应官方的 `PackageItemType`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageItemType {
    Image,
    MovieClip,
    Sound,
    Component,
    Atlas,
    Font,
    Swf,
    Misc,
    Unknown,
    Spine,
    DragonBones,
}

impl PackageItemType {
    /// 由包内的单字节编号构造。官方用有符号字节读取，因此未知编号统一归为 [`Self::Unknown`]。
    pub const fn from_byte(value: i8) -> Self {
        match value {
            0 => Self::Image,
            1 => Self::MovieClip,
            2 => Self::Sound,
            3 => Self::Component,
            4 => Self::Atlas,
            5 => Self::Font,
            6 => Self::Swf,
            7 => Self::Misc,
            9 => Self::Spine,
            10 => Self::DragonBones,
            _ => Self::Unknown,
        }
    }

    /// 稳定的名称，用于日志与调试输出。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Image => "Image",
            Self::MovieClip => "MovieClip",
            Self::Sound => "Sound",
            Self::Component => "Component",
            Self::Atlas => "Atlas",
            Self::Font => "Font",
            Self::Swf => "Swf",
            Self::Misc => "Misc",
            Self::Unknown => "Unknown",
            Self::Spine => "Spine",
            Self::DragonBones => "DragonBones",
        }
    }
}

/// 对象类型，对应官方的 `ObjectType`。
///
/// 组件条目的对象类型来自包内的 `extension` 字节：编辑器把「扩展」选项编号写进包里，
/// 编号为 0 时表示普通组件，其余编号既可能是内置控件，也可能是用户注册的扩展类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectType {
    Image,
    MovieClip,
    Swf,
    Graph,
    Loader,
    Group,
    Text,
    RichText,
    InputText,
    Component,
    List,
    Label,
    Button,
    ComboBox,
    ProgressBar,
    Slider,
    ScrollBar,
    Tree,
    Loader3D,
    /// 未被内置类型占用的扩展编号，交由上层注册表解析。
    Extension(u8),
}

impl ObjectType {
    /// 由编号构造，编号含义见官方 `ObjectType` 枚举。
    pub const fn from_code(code: u8) -> Self {
        match code {
            0 => Self::Image,
            1 => Self::MovieClip,
            2 => Self::Swf,
            3 => Self::Graph,
            4 => Self::Loader,
            5 => Self::Group,
            6 => Self::Text,
            7 => Self::RichText,
            8 => Self::InputText,
            9 => Self::Component,
            10 => Self::List,
            11 => Self::Label,
            12 => Self::Button,
            13 => Self::ComboBox,
            14 => Self::ProgressBar,
            15 => Self::Slider,
            16 => Self::ScrollBar,
            17 => Self::Tree,
            18 => Self::Loader3D,
            other => Self::Extension(other),
        }
    }

    /// 稳定的名称，用于日志与调试输出。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Image => "Image",
            Self::MovieClip => "MovieClip",
            Self::Swf => "Swf",
            Self::Graph => "Graph",
            Self::Loader => "Loader",
            Self::Group => "Group",
            Self::Text => "Text",
            Self::RichText => "RichText",
            Self::InputText => "InputText",
            Self::Component => "Component",
            Self::List => "List",
            Self::Label => "Label",
            Self::Button => "Button",
            Self::ComboBox => "ComboBox",
            Self::ProgressBar => "ProgressBar",
            Self::Slider => "Slider",
            Self::ScrollBar => "ScrollBar",
            Self::Tree => "Tree",
            Self::Loader3D => "Loader3D",
            Self::Extension(_) => "Extension",
        }
    }
}

/// 图集切片的映射关系，对应官方的 `AtlasSprite`。
///
/// 它把一张图片条目映射到某个图集条目内的矩形区域，是「图集零拷贝」的依据：
/// 渲染侧只需按 `rect` 截取图集纹理，无需切出子纹理。
#[derive(Debug, Clone)]
pub struct AtlasSprite {
    /// 所属图集条目在包内的下标。
    pub atlas_item: usize,
    /// 在图集中的矩形，单位与图集一致。
    pub rect: Rect,
    /// 相对原图左上角的偏移，用于九宫格与原尺寸还原。
    pub offset: Point,
    /// 原图尺寸，用于九宫格与原尺寸还原。
    pub original_size: Point,
    /// 是否被旋转打包。
    pub rotated: bool,
}

/// 像素级命中测试数据。
///
/// 位图按行优先、每像素 1 位存储，低位在前（与官方实现的位序一致）。
#[derive(Debug, Clone, Default)]
pub struct PixelHitTestData {
    /// 位图宽度（像素）。
    pub pixel_width: i32,
    /// 坐标缩放系数，等于包内比例的倒数。
    pub scale: f32,
    /// 位图数据。
    pub pixels: Vec<u8>,
}

impl PixelHitTestData {
    /// 从索引表的像素命中块读取一份数据。
    ///
    /// 包内格式为：精灵编号（丢弃）、位图宽度、比例分母、字节长度、若干字节。
    pub fn load(buffer: &mut ByteBuffer<'_>) -> Result<Self> {
        buffer.read_i32()?; // 精灵编号，无实际用途
        let pixel_width = buffer.read_i32()?;

        let denominator = buffer.read_i8()?;
        let scale = if denominator == 0 {
            tracing::warn!("pixel hit test scale divisor is 0, treated as 1");
            1.0
        } else {
            1.0 / f32::from(denominator)
        };

        let len = buffer.read_i32()?.max(0) as usize;
        let mut pixels = Vec::with_capacity(len);
        for _ in 0..len {
            // 官方按有符号字节读取后把负值加回 256，等价于按无符号读取。
            pixels.push(buffer.read_u8()?);
        }

        Ok(Self {
            pixel_width,
            scale,
            pixels,
        })
    }

    /// 判断本地坐标是否落在非透明像素上。
    ///
    /// 传入的坐标应当已经扣除对象偏移并除以对象缩放，即官方 `PixelHitTest.contains`
    /// 中括号内的值。
    pub fn contains(&self, local_x: f32, local_y: f32) -> bool {
        let x = (local_x * self.scale).floor();
        let y = (local_y * self.scale).floor();
        if x < 0.0 || y < 0.0 || self.pixel_width <= 0 || x >= self.pixel_width as f32 {
            return false;
        }

        let index = y as i32 * self.pixel_width + x as i32;
        let byte_index = index / 8;
        let bit = index % 8;

        match self.pixels.get(byte_index as usize) {
            Some(byte) => (byte >> bit) & 1 == 1,
            None => false,
        }
    }
}

/// 包内的单个资源描述。
#[derive(Debug, Clone)]
pub struct PackageItem {
    /// 资源编号，包内唯一。`ui://` 协议的 URL 由包编号与它拼接而成。
    pub id: String,
    /// 资源名称。分支条目的名称会带 `分支名/` 前缀。
    pub name: String,
    /// 资源大类。
    pub item_type: PackageItemType,
    /// 对象类型，决定实例化为哪个控件。
    pub object_type: ObjectType,
    /// 关联的外部文件名。图集与声音会带上包名前缀。
    pub file: Option<String>,
    /// 原始宽度。
    pub width: f32,
    /// 原始高度。
    pub height: f32,
    /// 原始数据在包数据中的区间。
    ///
    /// 组件布局、位图字体、序列帧都以子缓冲区形式存放，用 [`crate::package::FuiPackage::raw_data`]
    /// 取回可读的视图。
    pub raw_data: Option<Range<usize>>,
    /// 九宫格切分区域，对应官方的 `scale9Grid`。
    pub scale9_grid: Option<Rect>,
    /// 平铺网格索引，对应官方的 `tileGridIndice`。
    pub tile_grid_indice: Option<i32>,
    /// 是否按平铺缩放。
    pub scale_by_tile: bool,
    /// 纹理是否使用平滑过滤。
    pub smoothing: bool,
    /// 分支条目的编号列表。
    pub branches: Option<Vec<Option<String>>>,
    /// 高清替代资源的编号列表。
    pub high_resolution: Option<Vec<Option<String>>>,
    /// 像素级命中数据。
    pub pixel_hit_test_data: Option<PixelHitTestData>,
    /// 骨骼动画锚点。
    pub skeleton_anchor: Option<Point>,
}

impl PackageItem {
    /// 创建除类型外的所有字段均为默认值的条目。
    pub(crate) fn new(id: String, name: String, item_type: PackageItemType) -> Self {
        Self {
            id,
            name,
            item_type,
            object_type: ObjectType::Component,
            file: None,
            width: 0.0,
            height: 0.0,
            raw_data: None,
            scale9_grid: None,
            tile_grid_indice: None,
            scale_by_tile: false,
            smoothing: false,
            branches: None,
            high_resolution: None,
            pixel_hit_test_data: None,
            skeleton_anchor: None,
        }
    }

    /// 按分支编号取出实际生效的条目编号。
    ///
    /// 条件不满足时返回 `self.id`，与官方「查不到就回退自身」的行为一致。
    pub fn branch_id(&self, branch_index: i32) -> &str {
        if branch_index >= 0
            && let Some(branches) = &self.branches
            && let Some(Some(id)) = branches.get(branch_index as usize)
        {
            return id;
        }
        &self.id
    }

    /// 按内容缩放等级取出实际生效的高清条目编号。
    ///
    /// 等级 0 表示标准分辨率，此时不替换。
    pub fn high_resolution_id(&self, content_scale_level: i32) -> &str {
        if content_scale_level > 0
            && let Some(entries) = &self.high_resolution
            && let Some(Some(id)) = entries.get(content_scale_level as usize - 1)
        {
            return id;
        }
        &self.id
    }

    /// 校验是该类型并返回自身，否则报错。
    pub fn expect_type(&self, expected: PackageItemType) -> Result<&Self> {
        if self.item_type == expected {
            Ok(self)
        } else {
            Err(Error::UnexpectedItemType {
                expected: expected.as_str(),
                found: self.item_type.as_str(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_item_type_maps_all_known_codes() {
        assert_eq!(PackageItemType::from_byte(0), PackageItemType::Image);
        assert_eq!(PackageItemType::from_byte(3), PackageItemType::Component);
        assert_eq!(PackageItemType::from_byte(7), PackageItemType::Misc);
        assert_eq!(PackageItemType::from_byte(8), PackageItemType::Unknown);
        assert_eq!(PackageItemType::from_byte(10), PackageItemType::DragonBones);
        assert_eq!(PackageItemType::from_byte(99), PackageItemType::Unknown);
    }

    #[test]
    fn object_type_keeps_unknown_extension_code() {
        assert_eq!(ObjectType::from_code(9), ObjectType::Component);
        assert_eq!(ObjectType::from_code(17), ObjectType::Tree);
        assert_eq!(ObjectType::from_code(42), ObjectType::Extension(42));
    }

    #[test]
    fn branch_id_falls_back_to_self() {
        let mut item = PackageItem::new("a".into(), "n".into(), PackageItemType::Image);
        assert_eq!(item.branch_id(0), "a");

        item.branches = Some(vec![Some("b".into()), None]);
        assert_eq!(item.branch_id(0), "b");
        assert_eq!(item.branch_id(1), "a", "空槽位回退自身");
        assert_eq!(item.branch_id(5), "a", "越界回退自身");
        assert_eq!(item.branch_id(-1), "a", "未启用分支");
    }

    #[test]
    fn high_resolution_id_uses_level_minus_one() {
        let mut item = PackageItem::new("a".into(), "n".into(), PackageItemType::Image);
        item.high_resolution = Some(vec![Some("hd1".into()), Some("hd2".into())]);

        assert_eq!(item.high_resolution_id(0), "a", "等级 0 不替换");
        assert_eq!(item.high_resolution_id(1), "hd1");
        assert_eq!(item.high_resolution_id(2), "hd2");
        assert_eq!(item.high_resolution_id(3), "a", "越界回退自身");
    }

    #[test]
    fn pixel_hit_test_reads_big_endian_and_signed_scale() {
        // 精灵编号、宽度 8、比例分母 1、长度 2、位数据 [0b0000_0001, 0b1000_0000]
        let data = [
            0x00, 0x00, 0x00, 0x01, // spriteId
            0x00, 0x00, 0x00, 0x08, // pixelWidth
            0x01, // scale divisor
            0x00, 0x00, 0x00, 0x02, // len
            0x01, 0x80,
        ];
        let mut buffer = ByteBuffer::new(&data);
        let hit = PixelHitTestData::load(&mut buffer).unwrap();

        assert_eq!(hit.pixel_width, 8);
        assert_eq!(hit.scale, 1.0);
        assert_eq!(hit.pixels, vec![0x01, 0x80]);
        assert!(hit.contains(0.0, 0.0), "第一个像素为 1");
        assert!(!hit.contains(1.0, 0.0), "第二个像素为 0");
        assert!(hit.contains(7.0, 1.0), "第二字节的最高位");
        assert!(!hit.contains(8.0, 0.0), "超出宽度");
        assert!(!hit.contains(0.0, 2.0), "超出数据范围");
    }
}

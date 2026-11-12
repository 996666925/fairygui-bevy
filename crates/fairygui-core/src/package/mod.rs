//! `.fui` 包的解析与数据描述。
//!
//! 解析产物 [`FuiPackage`] 是**纯数据**：不含任何运行时状态，也不借用输入缓冲，
//! 因此可以安全地在线程间传递（例如作为 Bevy 的资产在线程池解析）。
//!
//! 包体结构为「头部 + 索引表 + 六个数据块」。索引表记录了各块的相对偏移，
//! 块本身可以缺失（偏移为 0），这是官方实现向前兼容新格式的方式。

mod font;
mod fui_package;
mod item;
mod movie;
mod registry;

pub use font::{BitmapFontData, BmGlyph, GlyphTexture};
pub use fui_package::{FuiPackage, LoadContext, PackageDependency};
pub use item::{AtlasSprite, ObjectType, PackageItem, PackageItemType, PixelHitTestData};
pub use movie::{FrameTexture, MovieClipData, MovieFrame};
pub use registry::PackageRegistry;

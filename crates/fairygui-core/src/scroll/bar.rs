//! 滚动条。
//!
//! 滚动条本身是个由容器派生的控件，其扩展数据只有一个「滑块尺寸是否固定」
//! 的开关；滑块的伸缩与位置由所属滚动容器计算。

// 滚动条的实例化依赖运行时注册表（按资源地址创建对象），此处先备好解析入口。
#![allow(dead_code)]

use crate::binary::{ByteBuffer, StringTable};
use crate::error::{Error, Result};
use crate::geom::{Point, Rect, Size};
use crate::package::PackageItemType;
use crate::widget::ComponentData;

/// 滑块在滚动条内的名称。
pub const GRIP_NAME: &str = "grip";
/// 轨道在滚动条内的名称。
pub const BAR_NAME: &str = "bar";
/// 起始箭头在滚动条内的名称。
pub const ARROW1_NAME: &str = "arrow1";
/// 末尾箭头在滚动条内的名称。
pub const ARROW2_NAME: &str = "arrow2";

/// 滚动条的数据。
#[derive(Debug, Clone, Default)]
pub struct ScrollBarData {
    /// 容器部分的数据。
    pub component: ComponentData,
    /// 滑块尺寸是否固定。
    pub fixed_grip_size: bool,
}

/// 摆放滑块之后的结果。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GripLayout {
    /// 滑块的位置（滚动条容器内的局部坐标）。
    pub pos: Point,
    /// 滑块的尺寸。
    pub size: Size,
    /// 滑块是否可见。
    pub visible: bool,
}

/// 摆放滑块所依据的滚动状态。
#[derive(Debug, Clone, Copy)]
pub struct GripState {
    /// 是否为纵向滚动条。
    pub vertical: bool,
    /// 滑块尺寸是否固定。
    pub fixed_grip_size: bool,
    /// 滑块占轨道的比例（视口 / 内容）。
    pub ratio: f32,
    /// 当前滚动比例，0 在起点、1 在终点。
    pub perc: f32,
}

/// 摆放滑块所依据的几何量，都是滚动条容器内的局部值。
#[derive(Debug, Clone, Copy, Default)]
pub struct GripGeometry {
    /// 轨道的位置与尺寸。
    pub bar: Rect,
    /// 滑块的当前位置与尺寸。
    pub grip: Rect,
}

/// 按滚动比例摆放滑块，对应官方的 `setDisplayPerc` 与 `setScrollPerc`。
///
/// 滑块尺寸固定时只挪位置、不改尺寸——这正是「固定滑块」开关的含义。
pub fn apply_grip_layout(state: GripState, geometry: GripGeometry) -> GripLayout {
    let mut pos = Point::new(geometry.grip.x, geometry.grip.y);
    let mut size = Size::new(geometry.grip.width, geometry.grip.height);

    if state.vertical {
        if !state.fixed_grip_size {
            size.height = (state.ratio * geometry.bar.height).floor();
        }
        pos.y = geometry.bar.y + (geometry.bar.height - size.height) * state.perc;
    } else {
        if !state.fixed_grip_size {
            size.width = (state.ratio * geometry.bar.width).floor();
        }
        pos.x = geometry.bar.x + (geometry.bar.width - size.width) * state.perc;
    }

    // 滑块占满（内容不超出）或占零时都没有可拖动的余地，官方此时隐藏它。
    GripLayout {
        pos,
        size,
        visible: state.ratio != 0.0 && state.ratio != 1.0,
    }
}

impl ScrollBarData {
    /// 计算滑块在滚动条上占据的比例。
    ///
    /// 内容不超过视口时滑块占满整条。
    pub fn grip_ratio(view_size: f32, content_size: f32) -> f32 {
        if content_size <= view_size || content_size <= 0.0 {
            1.0
        } else {
            view_size / content_size
        }
    }

    /// 计算滑块在滚动条上的起始比例。
    pub fn grip_start(offset: f32, view_size: f32, content_size: f32) -> f32 {
        let max = content_size - view_size;
        if max <= 0.0 {
            0.0
        } else {
            (offset / max).clamp(0.0, 1.0)
        }
    }
}

/// 读取滚动条的扩展数据（组件描述块 6）。
pub(crate) fn read_scroll_bar_extension(
    buffer: &mut ByteBuffer<'_>,
    table: &StringTable,
) -> Result<bool> {
    let _ = table;
    if !buffer.seek(0, 6)? {
        return Ok(false);
    }
    buffer.read_bool()
}

/// 校验条目确实是滚动条，并返回它。
pub(crate) fn expect_scroll_bar(
    package: &crate::package::FuiPackage,
    item_index: usize,
) -> Result<&crate::package::PackageItem> {
    let item = package
        .item(item_index)
        .ok_or_else(|| Error::ItemNotFound(format!("#{item_index}")))?;
    item.expect_type(PackageItemType::Component)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vertical_geometry() -> GripGeometry {
        GripGeometry {
            bar: Rect::new(0.0, 0.0, 10.0, 200.0),
            grip: Rect::new(0.0, 0.0, 10.0, 100.0),
        }
    }

    #[test]
    fn grip_shrinks_with_ratio() {
        let layout = apply_grip_layout(
            GripState {
                vertical: true,
                fixed_grip_size: false,
                ratio: 0.25,
                perc: 0.0,
            },
            vertical_geometry(),
        );
        assert_eq!(layout.size.height, 50.0, "四分之一的比例占轨道四分之一");
        assert!(layout.visible);
    }

    #[test]
    fn fixed_grip_size_keeps_dimensions() {
        let layout = apply_grip_layout(
            GripState {
                vertical: true,
                fixed_grip_size: true,
                ratio: 0.25,
                perc: 0.0,
            },
            vertical_geometry(),
        );
        assert_eq!(layout.size.height, 100.0, "固定尺寸时不应改变滑块高度");
    }

    #[test]
    fn grip_position_follows_perc() {
        let layout = apply_grip_layout(
            GripState {
                vertical: true,
                fixed_grip_size: false,
                ratio: 0.5,
                perc: 1.0,
            },
            vertical_geometry(),
        );
        // 滑块高 100、轨道高 200，滚到终点时偏移 100。
        assert_eq!(layout.pos.y, 100.0);
    }

    #[test]
    fn horizontal_axis_uses_width() {
        let layout = apply_grip_layout(
            GripState {
                vertical: false,
                fixed_grip_size: false,
                ratio: 0.5,
                perc: 0.5,
            },
            GripGeometry {
                bar: Rect::new(0.0, 0.0, 200.0, 10.0),
                grip: Rect::new(0.0, 0.0, 100.0, 10.0),
            },
        );
        assert_eq!(layout.size.width, 100.0);
        assert_eq!(layout.pos.x, 50.0);
    }

    #[test]
    fn grip_hides_when_content_fits() {
        let layout = apply_grip_layout(
            GripState {
                vertical: true,
                fixed_grip_size: false,
                ratio: 1.0,
                perc: 0.0,
            },
            vertical_geometry(),
        );
        assert!(!layout.visible, "内容不超出时滑块没有拖动余地，应当隐藏");
    }

    #[test]
    fn grip_ratio_fills_when_content_fits() {
        assert_eq!(ScrollBarData::grip_ratio(100.0, 40.0), 1.0);
        assert_eq!(ScrollBarData::grip_ratio(100.0, 100.0), 1.0);
        assert_eq!(ScrollBarData::grip_ratio(100.0, 0.0), 1.0);
    }

    #[test]
    fn grip_ratio_shrinks_with_content() {
        assert!((ScrollBarData::grip_ratio(100.0, 400.0) - 0.25).abs() < 1e-6);
    }

    #[test]
    fn grip_start_is_clamped() {
        assert_eq!(ScrollBarData::grip_start(-10.0, 100.0, 300.0), 0.0);
        assert_eq!(ScrollBarData::grip_start(100.0, 100.0, 300.0), 0.5);
        assert_eq!(ScrollBarData::grip_start(999.0, 100.0, 300.0), 1.0);
        assert_eq!(
            ScrollBarData::grip_start(50.0, 100.0, 50.0),
            0.0,
            "内容不超出时为 0"
        );
    }
}

//! 滚动容器与滚动条。
//!
//! 滚动位置由核心层掌管（边界、惯性、回弹都在这里算），渲染层只负责把位置
//! 映射到引擎的滚动机制上。这样做的原因是：惯性与回弹需要跨帧累积状态，
//! 放在核心层才能与对象模型一起被保存与恢复。

pub(crate) mod bar;
mod pane;
mod visibility;

pub use bar::{
    ARROW1_NAME, ARROW2_NAME, BAR_NAME, GRIP_NAME, GripGeometry, GripLayout, GripState,
    ScrollBarData, apply_grip_layout,
};
pub(crate) use pane::snapping_position;
pub use pane::{
    PullRelease, RefreshBarLayout, SCROLL_STEP, ScrollBarDisplayType, ScrollDefaults, ScrollLoop,
    ScrollPane, ScrollPhysics, ScrollType,
};

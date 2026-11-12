//! UI 事件。
//!
//! 官方实现直接把引擎的事件系统当作自己的事件系统，并靠对象树冒泡。
//! 这里改为**事件队列**：核心层在派发时把事件连同冒泡路径推进队列，
//! 由上层（例如 Bevy 的 ECS 系统）取出后转成自己的事件。
//!
//! 这样做是因为 Rust 下「在回调里修改对象树」会与派发过程产生借用冲突，
//! 而队列可以把「派发」与「响应」彻底分开。核心层内部的联动（按钮状态、
//! 列表选择等）不走队列，直接在对象方法里完成。

use crate::geom::Point;
use crate::object::GObjectId;

/// 事件类型。
///
/// 前半部分对应引擎的原生事件，后半部分是官方 `Events` 里定义的自定义事件。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EventKind {
    // ---- 指针 ----
    Click,
    RightClick,
    /// Rich-text href, preserved verbatim in EventData::Text.
    Link,
    MouseDown,
    MouseUp,
    RollOver,
    RollOut,
    MouseWheel,

    // ---- 触摸 ----
    TouchBegin,
    TouchMove,
    TouchEnd,

    // ---- 键盘 ----
    KeyDown,
    KeyUp,

    // ---- 焦点 ----
    FocusIn,
    FocusOut,
    /// Native input events are delivered to the input itself, without bubbling.
    Input,
    Change,
    Enter,

    // ---- 窗口 ----
    /// 窗口已显示。
    WindowShown,
    /// 窗口已隐藏或关闭。与官方一致，两种情况共用同一个事件。
    WindowHide,

    // ---- 布局 ----
    /// 坐标变化。
    XyChanged,
    /// 尺寸变化。
    SizeChanged,
    /// 尺寸的延迟变化（容器在下一帧统一处理）。
    SizeDelayChange,

    // ---- 状态 ----
    /// 按钮或控制器状态变化。
    StateChanged,
    /// 齿轮动画结束。
    GearStop,

    // ---- 列表与滚动 ----
    /// 列表项被点击。
    ClickItem,
    Scroll,
    ScrollEnd,

    // ---- 拖拽 ----
    DragStart,
    DragMove,
    DragEnd,
    Drop,

    // ---- 下拉刷新 ----
    PullDownRelease,
    PullUpRelease,

    /// 其它自定义事件，携带事件名。
    Other(String),
}

impl EventKind {
    /// 官方定义的自定义事件名，用于和外部系统对接。
    pub fn as_fairygui_name(&self) -> Option<&str> {
        match self {
            Self::StateChanged => Some("fui_state_changed"),
            Self::XyChanged => Some("fui_xy_changed"),
            Self::SizeChanged => Some("fui_size_changed"),
            Self::SizeDelayChange => Some("fui_size_delay_change"),
            Self::ClickItem => Some("fui_click_item"),
            Self::Scroll => Some("fui_scroll"),
            Self::ScrollEnd => Some("fui_scroll_end"),
            Self::Drop => Some("fui_drop"),
            Self::DragStart => Some("fui_drag_start"),
            Self::DragMove => Some("fui_drag_move"),
            Self::DragEnd => Some("fui_drag_end"),
            Self::PullDownRelease => Some("fui_pull_down_release"),
            Self::PullUpRelease => Some("fui_pull_up_release"),
            Self::GearStop => Some("fui_gear_stop"),
            _ => None,
        }
    }
}

/// 事件携带的附加数据。
#[derive(Debug, Clone, PartialEq, Default)]
pub enum EventData {
    /// 无附加数据。
    #[default]
    None,
    /// 列表项编号。
    Item(String),
    /// 滚轮或滚动的增量。
    Delta(f32),
    /// 按键标识。
    Key(String),
    /// 文本内容。
    Text(String),
}

/// 一次事件投递。
///
/// 冒泡过程中，同一个事件会为目标及其每个祖先各产生一条记录，
/// `current_target` 指出当前是哪一个对象在接收。
#[derive(Debug, Clone)]
pub struct UiEvent {
    /// 事件类型。
    pub kind: EventKind,
    /// 事件的原始目标。
    pub target: GObjectId,
    /// 当前接收该事件的对象。冒泡过程中从目标逐级向上。
    pub current_target: GObjectId,
    /// 指针位置，来自输入系统时才有值。
    pub pointer: Option<Point>,
    /// 附加数据。
    pub data: EventData,
    /// 是否为冒泡阶段。
    pub bubbling: bool,
}

impl UiEvent {
    /// 创建一条面向指定对象的事件。
    pub fn new(kind: EventKind, target: GObjectId) -> Self {
        Self {
            kind,
            target,
            current_target: target,
            pointer: None,
            data: EventData::None,
            bubbling: false,
        }
    }

    /// 设置指针位置。
    pub fn with_pointer(mut self, pointer: Point) -> Self {
        self.pointer = Some(pointer);
        self
    }

    /// 设置附加数据。
    pub fn with_data(mut self, data: EventData) -> Self {
        self.data = data;
        self
    }

    /// 当前接收者是否就是事件目标。
    pub fn is_at_target(&self) -> bool {
        self.target == self.current_target
    }
}

/// 待处理的事件队列。
#[derive(Debug, Default)]
pub struct EventQueue {
    events: Vec<UiEvent>,
}

impl EventQueue {
    /// 空队列。
    pub fn new() -> Self {
        Self::default()
    }

    /// 追加一条事件。
    pub fn push(&mut self, event: UiEvent) {
        self.events.push(event);
    }

    /// 取走全部事件。
    pub fn drain(&mut self) -> impl Iterator<Item = UiEvent> + '_ {
        self.events.drain(..)
    }

    /// 队列长度。
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// 队列是否为空。
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// 清空队列。
    pub fn clear(&mut self) {
        self.events.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slotmap::Key;

    fn dummy_id() -> GObjectId {
        GObjectId::null()
    }

    #[test]
    fn custom_event_names_match_official_constants() {
        assert_eq!(
            EventKind::StateChanged.as_fairygui_name(),
            Some("fui_state_changed")
        );
        assert_eq!(
            EventKind::ClickItem.as_fairygui_name(),
            Some("fui_click_item")
        );
        assert_eq!(EventKind::Click.as_fairygui_name(), None);
    }

    #[test]
    fn queue_drains_in_order() {
        let mut queue = EventQueue::new();
        assert!(queue.is_empty());

        let id = dummy_id();
        queue.push(UiEvent::new(EventKind::Click, id));
        queue.push(UiEvent::new(EventKind::MouseUp, id).with_data(EventData::Delta(2.0)));
        assert_eq!(queue.len(), 2);

        let drained: Vec<_> = queue.drain().collect();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].kind, EventKind::Click);
        assert!(drained[0].is_at_target());
        assert_eq!(drained[1].data, EventData::Delta(2.0));
        assert!(queue.is_empty());
    }
}

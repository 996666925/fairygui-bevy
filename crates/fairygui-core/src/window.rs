//! 窗口。
//!
//! 窗口是挂在根节点弹窗层上的容器，通常包含内容组件与可选的窗框
//! （窗框里带标题与关闭按钮）。显示与隐藏都要经过根节点，这样层级顺序
//! 与模态状态才能统一维护。

use crate::geom::Point;
use crate::object::{GObjectId, ObjectStore};
use crate::root::Root;

/// 窗框组件在窗口里的名字。
pub const FRAME_NAME: &str = "frame";
/// 内容组件在窗口里的名字。
pub const CONTENT_NAME: &str = "content";
/// 关闭按钮在窗框里的名字。
pub const CLOSE_BUTTON_NAME: &str = "closeButton";

/// 窗口。
#[derive(Debug, Clone)]
pub struct Window {
    /// 窗口自身对象。
    owner: GObjectId,
    /// 内容组件。
    content: Option<GObjectId>,
    /// 窗框组件。
    frame: Option<GObjectId>,
    /// 关闭按钮。
    close_button: Option<GObjectId>,
    /// 拖拽区域，通常是窗框的标题栏。
    drag_area: Option<GObjectId>,
    /// 模态等待时盖在窗口上的面板。
    modal_wait_pane: Option<GObjectId>,
    /// 发起模态等待时所带的命令编号。0 表示不限定。
    requesting_cmd: u32,
    /// 是否模态。
    modal: bool,
    /// 是否允许拖拽移动窗口。
    draggable: bool,
    /// 是否正在显示。
    showing: bool,
    /// 是否正在拖拽。
    dragging: bool,
    /// 是否正在模态等待。
    modal_waiting: bool,
    /// 按下时光标相对窗口左上角的偏移，拖拽期间保持不变。
    drag_offset: Point,
}

impl Window {
    /// 以指定对象为窗口主体创建。
    pub fn new(owner: GObjectId) -> Self {
        Self {
            owner,
            content: None,
            frame: None,
            close_button: None,
            drag_area: None,
            modal_wait_pane: None,
            requesting_cmd: 0,
            modal: false,
            draggable: true,
            showing: false,
            dragging: false,
            modal_waiting: false,
            drag_offset: Point::ZERO,
        }
    }

    /// 窗口自身对象。
    pub fn owner(&self) -> GObjectId {
        self.owner
    }

    /// 内容组件。
    pub fn content(&self) -> Option<GObjectId> {
        self.content
    }

    /// 窗框组件。
    pub fn frame(&self) -> Option<GObjectId> {
        self.frame
    }

    /// 关闭按钮。
    pub fn close_button(&self) -> Option<GObjectId> {
        self.close_button
    }

    /// 设置内容组件。内容会成为窗口的子对象。
    pub fn set_content(&mut self, store: &mut ObjectStore, content: GObjectId) {
        store.set_parent(content, Some(self.owner));
        self.content = Some(content);
    }

    /// 设置窗框组件。
    pub fn set_frame(&mut self, store: &mut ObjectStore, frame: GObjectId) {
        store.set_parent(frame, Some(self.owner));
        self.frame = Some(frame);
    }

    /// 记录关闭按钮，便于上层把它与关闭行为关联起来。
    pub fn set_close_button(&mut self, button: GObjectId) {
        self.close_button = Some(button);
    }

    /// 按约定名识别窗口的各个部分。
    ///
    /// 编辑器里的约定是：窗框组件叫 `frame`，内容组件叫 `content`，关闭按钮
    /// 在窗框里叫 `closeButton`，而拖拽区域就是窗框本身（也就是标题栏所在）。
    /// 认不出来的部分留空，窗口依然可用，只是少了对应的行为。
    pub fn attach_parts(&mut self, store: &ObjectStore) {
        let owner = self.owner;

        if let Some(frame) = store.child_by_name(owner, FRAME_NAME) {
            self.frame = Some(frame);
            self.drag_area = Some(frame);
            if let Some(button) = store.child_by_name(frame, CLOSE_BUTTON_NAME) {
                self.close_button = Some(button);
            }
        }

        if let Some(content) = store.child_by_name(owner, CONTENT_NAME) {
            self.content = Some(content);
        }
    }

    /// 是否可拖拽。
    pub fn is_draggable(&self) -> bool {
        self.draggable
    }

    /// 设置是否可拖拽。
    pub fn set_draggable(&mut self, draggable: bool) {
        self.draggable = draggable;
    }

    /// 是否为模态窗口。
    pub fn is_modal(&self) -> bool {
        self.modal
    }

    /// 是否正在显示。
    pub fn is_showing(&self) -> bool {
        self.showing
    }

    /// 显示窗口。已经显示时不做任何事。
    pub fn show(&mut self, store: &mut ObjectStore, root: &mut Root, modal: bool) {
        if self.showing {
            return;
        }
        self.modal = modal;
        root.show_popup(store, self.owner, modal);
        store.set_visible(self.owner, true);
        self.showing = true;
    }

    /// 隐藏窗口。未显示时不做任何事。
    pub fn hide(&mut self, store: &mut ObjectStore, root: &mut Root) {
        if !self.showing {
            return;
        }
        root.detach(store, self.owner);
        self.showing = false;
    }

    /// 切换显示状态。已显示则隐藏，否则显示。
    pub fn toggle(&mut self, store: &mut ObjectStore, root: &mut Root, modal: bool) {
        if self.showing {
            self.hide(store, root);
        } else {
            self.show(store, root, modal);
        }
    }

    /// 让窗口在根容器上居中。
    pub fn center_on(&self, store: &mut ObjectStore, root: &Root) {
        let Some(root_object) = store.object(root.owner()) else {
            return;
        };
        let (root_width, root_height) = (root_object.width(), root_object.height());

        let Some(window) = store.object(self.owner) else {
            return;
        };
        let (width, height) = (window.width(), window.height());

        store.set_xy(
            self.owner,
            (root_width - width) / 2.0,
            (root_height - height) / 2.0,
        );
    }

    /// 把窗口提到最上方。窗框被点击时通常需要这么做。
    pub fn bring_to_front(&self, store: &mut ObjectStore, root: &mut Root) -> bool {
        if !self.showing {
            return false;
        }
        root.bring_to_front(store, self.owner)
    }

    // ---- 拖拽 ----

    /// 设置拖拽区域，通常是窗框的标题栏。
    pub fn set_drag_area(&mut self, area: GObjectId) {
        self.drag_area = Some(area);
    }

    /// 拖拽区域。
    pub fn drag_area(&self) -> Option<GObjectId> {
        self.drag_area
    }

    /// 该对象是否落在拖拽区域内（即是不是标题栏自己或它的子对象）。
    pub fn is_in_drag_area(&self, store: &ObjectStore, object: GObjectId) -> bool {
        let Some(area) = self.drag_area else {
            return false;
        };

        let mut current = Some(object);
        while let Some(id) = current {
            if id == area {
                return true;
            }
            current = store.object(id).and_then(|item| item.parent());
        }
        false
    }

    /// 开始拖拽。`cursor_x`/`cursor_y` 是按下时光标在根坐标系里的位置。
    ///
    /// 窗口不可拖拽、尚未显示、或传入的偏移不成立时返回 `false`。
    pub fn begin_drag(&mut self, store: &ObjectStore, cursor_x: f32, cursor_y: f32) -> bool {
        if !self.draggable || !self.showing {
            return false;
        }

        let Some(window) = store.object(self.owner) else {
            return false;
        };
        // 记下光标相对窗口左上角的偏移，拖拽期间它保持不变，
        // 这样窗口不会在按下的瞬间跳到光标中心。
        self.drag_offset = Point::new(cursor_x - window.x(), cursor_y - window.y());
        self.dragging = true;
        true
    }

    /// 拖拽到光标所在位置。返回窗口位置是否变化。
    pub fn drag_to(&mut self, store: &mut ObjectStore, cursor_x: f32, cursor_y: f32) -> bool {
        if !self.dragging {
            return false;
        }

        let x = cursor_x - self.drag_offset.x;
        let y = cursor_y - self.drag_offset.y;

        let changed = store
            .object(self.owner)
            .is_some_and(|window| window.x() != x || window.y() != y);
        if changed {
            store.set_xy(self.owner, x, y);
        }
        changed
    }

    /// 结束拖拽。
    pub fn end_drag(&mut self) {
        self.dragging = false;
    }

    /// 直接设置拖拽状态。
    ///
    /// 渲染层按「相对按下点的位移」拖动窗口时不需要起点偏移，但窗口自身
    /// 仍要知道自己正在被拖。
    pub fn set_dragging(&mut self, dragging: bool) {
        self.dragging = dragging;
    }

    /// 是否正在拖拽。
    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    // ---- 模态等待 ----

    /// 设置模态等待面板。它会被盖在内容区上。
    pub fn set_modal_wait_pane(&mut self, pane: GObjectId) {
        self.modal_wait_pane = Some(pane);
    }

    /// 模态等待面板。
    pub fn modal_wait_pane(&self) -> Option<GObjectId> {
        self.modal_wait_pane
    }

    /// 是否正在模态等待。
    pub fn is_modal_waiting(&self) -> bool {
        self.modal_waiting
    }

    /// 开始模态等待：在内容区上盖一块面板，挡住窗口内的操作。
    ///
    /// `cmd` 是发起方自定的命令编号，结束时据此判断「这次等待是不是我要等的
    /// 那一个」。传 0 表示不限定。没有配置等待面板时只记录状态。
    pub fn show_modal_wait(&mut self, store: &mut ObjectStore, cmd: u32) {
        self.requesting_cmd = cmd;
        self.modal_waiting = true;

        let Some(pane) = self.modal_wait_pane else {
            return;
        };

        // 面板盖住内容区；没有内容区时盖住整个窗口。
        match self.content.and_then(|id| {
            store
                .object(id)
                .map(|object| (object.position(), object.size()))
        }) {
            Some((pos, size)) => {
                store.set_xy(pane, pos.x, pos.y);
                store.set_size(pane, size.width, size.height);
            }
            None => {
                let (width, height) = store
                    .object(self.owner)
                    .map_or((0.0, 0.0), |object| (object.width(), object.height()));
                store.set_size(pane, width, height);
            }
        }

        store.set_parent(pane, Some(self.owner));
        store.set_visible(pane, true);
    }

    /// 结束模态等待。
    ///
    /// 传入了非零命令编号且与发起时不一致时不做任何事并返回 `false`——
    /// 这样多个并存的请求不会把彼此关掉。
    pub fn close_modal_wait(&mut self, store: &mut ObjectStore, cmd: u32) -> bool {
        if cmd != 0 && self.requesting_cmd != cmd {
            return false;
        }

        self.requesting_cmd = 0;
        self.modal_waiting = false;

        if let Some(pane) = self.modal_wait_pane {
            store.detach_from_parent(pane);
            store.set_visible(pane, false);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::ObjectType;

    fn setup() -> (ObjectStore, Root, Window) {
        let mut store = ObjectStore::new();
        let root_object = store.create(ObjectType::Component);
        store.set_size(root_object, 1000.0, 800.0);
        let root = Root::new(root_object);

        let owner = store.create(ObjectType::Component);
        store.set_size(owner, 400.0, 300.0);
        let window = Window::new(owner);
        (store, root, window)
    }

    #[test]
    fn show_and_hide_go_through_the_root() {
        let (mut store, mut root, mut window) = setup();
        let owner = window.owner();

        assert!(!window.is_showing());
        window.show(&mut store, &mut root, false);

        assert!(window.is_showing());
        assert_eq!(root.len(crate::root::RootLayer::Popup), 1);
        assert_eq!(store.object(owner).unwrap().parent(), Some(root.owner()));

        window.hide(&mut store, &mut root);
        assert!(!window.is_showing());
        assert_eq!(root.len(crate::root::RootLayer::Popup), 0);
    }

    #[test]
    fn showing_twice_is_a_no_op() {
        let (mut store, mut root, mut window) = setup();
        window.show(&mut store, &mut root, false);
        window.show(&mut store, &mut root, false);
        assert_eq!(root.len(crate::root::RootLayer::Popup), 1);
    }

    #[test]
    fn modal_window_blocks_lower_layers() {
        let (mut store, mut root, mut window) = setup();

        window.show(&mut store, &mut root, false);
        assert!(!root.is_modal());

        window.hide(&mut store, &mut root);
        window.show(&mut store, &mut root, true);
        assert!(root.is_modal());
        assert_eq!(root.top_modal(), Some(window.owner()));
    }

    #[test]
    fn toggle_switches_between_states() {
        let (mut store, mut root, mut window) = setup();

        window.toggle(&mut store, &mut root, false);
        assert!(window.is_showing());

        window.toggle(&mut store, &mut root, false);
        assert!(!window.is_showing());
    }

    #[test]
    fn centering_places_the_window_in_the_middle() {
        let (mut store, root, window) = setup();
        window.center_on(&mut store, &root);

        let object = store.object(window.owner()).unwrap();
        assert_eq!(object.position().x, 300.0);
        assert_eq!(object.position().y, 250.0);
    }

    #[test]
    fn content_and_frame_become_children() {
        let (mut store, _root, mut window) = setup();
        let owner = window.owner();

        let content = store.create(ObjectType::Component);
        let frame = store.create(ObjectType::Component);
        window.set_content(&mut store, content);
        window.set_frame(&mut store, frame);

        assert_eq!(store.object(content).unwrap().parent(), Some(owner));
        assert_eq!(store.object(frame).unwrap().parent(), Some(owner));
        assert_eq!(window.content(), Some(content));
        assert_eq!(window.frame(), Some(frame));
    }

    #[test]
    fn draggable_defaults_to_true() {
        let (_store, _root, window) = setup();
        assert!(window.is_draggable());
    }

    /// 拖拽只对已显示的窗口有意义，因此测试统一从这里取。
    fn setup_shown() -> (ObjectStore, Root, Window) {
        let (mut store, mut root, mut window) = setup();
        window.show(&mut store, &mut root, false);
        (store, root, window)
    }

    #[test]
    fn dragging_moves_the_window_by_the_cursor_delta() {
        let (mut store, _root, mut window) = setup_shown();

        // 光标在窗口内的 (10, 20) 处按下。
        assert!(window.begin_drag(&store, 10.0, 20.0));
        assert!(window.is_dragging());

        // 光标移到 (110, 120)，窗口跟着走同样的位移。
        assert!(window.drag_to(&mut store, 110.0, 120.0));
        let object = store.object(window.owner()).unwrap();
        assert_eq!(object.position().x, 100.0);
        assert_eq!(object.position().y, 100.0);

        window.end_drag();
        assert!(!window.is_dragging());
        assert!(
            !window.drag_to(&mut store, 500.0, 500.0),
            "结束拖拽后不应再跟随光标"
        );
    }

    #[test]
    fn drag_keeps_the_grab_offset() {
        let (mut store, _root, mut window) = setup_shown();

        // 光标没动时窗口也不该动——否则按下的瞬间窗口会跳到光标位置。
        window.begin_drag(&store, 200.0, 150.0);
        window.drag_to(&mut store, 200.0, 150.0);

        assert_eq!(
            store.object(window.owner()).unwrap().position(),
            Point::ZERO
        );
    }

    #[test]
    fn non_draggable_window_refuses_drag() {
        let (store, _root, mut window) = setup_shown();
        window.set_draggable(false);
        assert!(
            !window.begin_drag(&store, 0.0, 0.0),
            "明确禁止拖拽时不应进入拖拽状态"
        );
    }

    #[test]
    fn hidden_window_cannot_be_dragged() {
        let (store, _root, mut window) = setup();
        assert!(
            !window.begin_drag(&store, 0.0, 0.0),
            "未显示的窗口不应可拖拽"
        );
    }

    #[test]
    fn drag_area_recognises_descendants() {
        let (mut store, _root, mut window) = setup_shown();
        let owner = window.owner();

        let title = store.create(ObjectType::Component);
        let label = store.create(ObjectType::Image);
        store.set_parent(title, Some(owner));
        store.set_parent(label, Some(title));
        window.set_drag_area(title);

        assert!(window.is_in_drag_area(&store, title));
        assert!(
            window.is_in_drag_area(&store, label),
            "标题栏里的子对象也算在拖拽区域内"
        );
        assert!(!window.is_in_drag_area(&store, owner), "窗口自身不算");
    }

    #[test]
    fn modal_wait_toggle() {
        let (mut store, _root, mut window) = setup();

        assert!(!window.is_modal_waiting());
        window.show_modal_wait(&mut store, 7);
        assert!(window.is_modal_waiting());

        // 命令编号不匹配时不会误关别人的等待。
        assert!(!window.close_modal_wait(&mut store, 8));
        assert!(window.is_modal_waiting());

        assert!(window.close_modal_wait(&mut store, 7));
        assert!(!window.is_modal_waiting());
    }

    #[test]
    fn modal_wait_pane_covers_the_content_area() {
        let (mut store, _root, mut window) = setup_shown();
        let owner = window.owner();

        let content = store.create(ObjectType::Component);
        store.set_xy(content, 20.0, 60.0);
        store.set_size(content, 360.0, 200.0);
        window.set_content(&mut store, content);

        let pane = store.create(ObjectType::Image);
        window.set_modal_wait_pane(pane);
        window.show_modal_wait(&mut store, 0);

        // 面板对齐内容区，并成为窗口的子对象。
        let object = store.object(pane).unwrap();
        assert_eq!(object.parent(), Some(owner));
        assert_eq!(object.position().x, 20.0);
        assert_eq!(object.position().y, 60.0);
        assert_eq!(object.width(), 360.0);
        assert_eq!(object.height(), 200.0);

        window.close_modal_wait(&mut store, 0);
        assert_eq!(store.object(pane).unwrap().parent(), None);
    }
}

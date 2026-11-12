//! 弹出菜单。
//!
//! 菜单只维护「有哪些项」以及各项的状态，实际的列表控件由上层根据这些数据
//! 填充。这样菜单逻辑与渲染完全解耦，也便于在没有界面资源时单独测试。

/// 菜单项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItem {
    /// Stable application name, independent of the displayed caption.
    pub name: String,
    /// Application action identifier. The renderer never interprets this value.
    pub action: Option<u64>,
    /// Package icon URL, when present.
    pub icon: Option<String>,
    /// 显示文本。
    pub caption: String,
    /// 是否为分隔线。分隔线不参与点击。
    pub separator: bool,
    /// 是否被勾选。
    pub checked: bool,
    /// Whether the template's checked controller is enabled.
    pub checkable: bool,
    /// 是否可见。
    pub visible: bool,
    /// 是否可点击。
    pub enabled: bool,
}

impl MenuItem {
    /// 普通菜单项。
    pub fn new(caption: impl Into<String>) -> Self {
        let caption = caption.into();
        Self {
            name: caption.clone(),
            action: None,
            icon: None,
            caption,
            separator: false,
            checked: false,
            checkable: false,
            visible: true,
            enabled: true,
        }
    }

    /// A named action. Names remain unchanged when captions are edited.
    pub fn action(name: impl Into<String>, caption: impl Into<String>, action: u64) -> Self {
        Self {
            name: name.into(),
            action: Some(action),
            ..Self::new(caption)
        }
    }

    /// 分隔线。
    pub fn separator() -> Self {
        Self {
            separator: true,
            enabled: false,
            ..Self::new("")
        }
    }

    /// 该项是否可以被点击。
    pub fn is_actionable(&self) -> bool {
        !self.separator && self.visible && self.enabled
    }
}

/// 弹出菜单。
#[derive(Debug, Clone, Default)]
pub struct PopupMenu {
    items: Vec<MenuItem>,
    /// 菜单展开时承载它的对象。菜单项是它的子对象。
    content: Option<crate::object::GObjectId>,
    /// 菜单项模板地址，供适配器更新内容时复用列表项。
    item_url: Option<String>,
}

impl PopupMenu {
    /// 空菜单。
    pub fn new() -> Self {
        Self::default()
    }

    /// 承载菜单的对象。未展开时为 `None`。
    pub fn content(&self) -> Option<crate::object::GObjectId> {
        self.content
    }

    /// 菜单是否展开着。
    pub fn is_open(&self) -> bool {
        self.content.is_some()
    }

    /// 菜单项的模板地址。
    pub fn item_url(&self) -> Option<&str> {
        self.item_url.as_deref()
    }

    /// 记下菜单已展开。
    pub fn set_content(&mut self, content: crate::object::GObjectId, item_url: Option<String>) {
        self.content = Some(content);
        self.item_url = item_url;
    }

    /// 记下菜单已收起。数据部分原样保留，下次展开可以直接复用。
    pub fn set_closed(&mut self) {
        self.content = None;
    }

    /// 全部菜单项。
    pub fn items(&self) -> &[MenuItem] {
        &self.items
    }

    /// 项的数量（含分隔线）。
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// 是否没有任何项。
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// 追加一个菜单项。
    pub fn add_item(&mut self, caption: impl Into<String>) {
        self.items.push(MenuItem::new(caption));
    }

    /// 追加一条分隔线。
    pub fn add_separator(&mut self) {
        self.items.push(MenuItem::separator());
    }

    /// 清空全部项。
    pub fn clear_items(&mut self) {
        self.items.clear();
    }

    /// 按文本修改菜单项。
    ///
    /// 找不到时返回 `false`。
    pub fn set_item_text(&mut self, caption: &str, new_caption: impl Into<String>) -> bool {
        match self.items.iter_mut().find(|item| item.caption == caption) {
            Some(item) => {
                item.caption = new_caption.into();
                true
            }
            None => false,
        }
    }

    /// 设置某一项的勾选状态。分隔线不受影响。
    pub fn set_item_checked(&mut self, caption: &str, checked: bool) -> bool {
        match self
            .items
            .iter_mut()
            .find(|item| !item.separator && item.caption == caption)
        {
            Some(item) => {
                item.checked = checked;
                item.checkable = true;
                true
            }
            None => false,
        }
    }

    /// 设置某一项的可见性。
    pub fn set_item_visible(&mut self, caption: &str, visible: bool) -> bool {
        match self.items.iter_mut().find(|item| item.caption == caption) {
            Some(item) => {
                item.visible = visible;
                true
            }
            None => false,
        }
    }

    /// 设置某一项是否可点击。
    pub fn set_item_enabled(&mut self, caption: &str, enabled: bool) -> bool {
        match self.items.iter_mut().find(|item| item.caption == caption) {
            Some(item) => {
                item.enabled = enabled;
                true
            }
            None => false,
        }
    }

    /// 按序号取项的文本。用于把点击结果映射回菜单项。
    pub fn caption_at(&self, index: usize) -> Option<&str> {
        self.items.get(index).map(|item| item.caption.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_tracks_open_and_closed_state() {
        let mut menu = PopupMenu::new();
        assert!(!menu.is_open());
        assert_eq!(menu.content(), None);

        // 展开：记下承载对象与项模板地址。
        let mut store = crate::object::ObjectStore::new();
        let content = store.create(crate::package::ObjectType::Component);
        menu.set_content(content, Some("ui://pkg/item".to_string()));

        assert!(menu.is_open());
        assert_eq!(menu.content(), Some(content));
        assert_eq!(menu.item_url(), Some("ui://pkg/item"));

        // 收起：承载对象清空，但项模板地址保留，下次展开可直接复用。
        menu.set_closed();
        assert!(!menu.is_open());
        assert_eq!(menu.content(), None);
        assert_eq!(
            menu.item_url(),
            Some("ui://pkg/item"),
            "收起不该丢掉模板地址"
        );
    }

    #[test]
    fn items_and_separators_are_appended_in_order() {
        let mut menu = PopupMenu::new();
        menu.add_item("打开");
        menu.add_separator();
        menu.add_item("退出");

        assert_eq!(menu.item_count(), 3);
        assert_eq!(menu.caption_at(0), Some("打开"));
        assert_eq!(menu.caption_at(1), Some(""));
        assert_eq!(menu.caption_at(2), Some("退出"));
        assert!(menu.items()[1].separator);
    }

    #[test]
    fn separator_is_not_actionable() {
        let separator = MenuItem::separator();
        assert!(!separator.is_actionable());

        let mut item = MenuItem::new("项");
        assert!(item.is_actionable());
        item.enabled = false;
        assert!(!item.is_actionable());
        item.enabled = true;
        item.visible = false;
        assert!(!item.is_actionable());
    }

    #[test]
    fn modifications_report_whether_they_applied() {
        let mut menu = PopupMenu::new();
        menu.add_item("A");

        assert!(menu.set_item_text("A", "B"));
        assert_eq!(menu.caption_at(0), Some("B"));
        assert!(!menu.set_item_text("A", "C"), "旧文本已不存在");

        assert!(menu.set_item_checked("B", true));
        assert!(menu.set_item_visible("B", false));
        assert!(menu.set_item_enabled("B", false));
        assert!(!menu.set_item_checked("不存在", true));
    }

    #[test]
    fn separators_are_not_selectable_by_name() {
        let mut menu = PopupMenu::new();
        menu.add_separator();
        // 分隔线的文本为空，不应被勾选操作命中。
        assert!(!menu.set_item_checked("", true));
    }

    #[test]
    fn clearing_removes_everything() {
        let mut menu = PopupMenu::new();
        menu.add_item("A");
        menu.add_item("B");
        assert!(!menu.is_empty());

        menu.clear_items();
        assert!(menu.is_empty());
        assert_eq!(menu.caption_at(0), None);
    }
}

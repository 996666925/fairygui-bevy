//! 界面根节点。
//!
//! 根节点把界面分成几层：内容层放普通界面，弹窗层放窗口与提示，
//! 弹窗层始终盖在内容层之上。同一层内后加入的在上方，因此绘制顺序
//! 与加入顺序一致。有模态弹窗时，下层输入需要被挡住。

use crate::object::{GObjectId, ObjectStore};

/// 根节点管理的层。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootLayer {
    /// 内容层：普通界面。
    Content,
    /// 弹窗层：窗口与提示。
    Popup,
}

/// 界面根节点。
///
/// 它自身是一个容器对象，各层界面都是它的子对象。
#[derive(Debug, Clone)]
pub struct Root {
    /// 根容器对象。
    owner: GObjectId,
    /// 内容层的项，按加入顺序排列。
    content: Vec<GObjectId>,
    /// 弹窗层的项，附带是否为模态。
    popups: Vec<(GObjectId, bool)>,
    /// 整体缩放系数。
    scale: f32,
}

impl Root {
    /// 以指定对象为根容器创建。
    pub fn new(owner: GObjectId) -> Self {
        Self {
            owner,
            content: Vec::new(),
            popups: Vec::new(),
            scale: 1.0,
        }
    }

    /// 根容器对象。
    pub fn owner(&self) -> GObjectId {
        self.owner
    }

    /// 内容层的项。
    pub fn content(&self) -> &[GObjectId] {
        &self.content
    }

    /// 弹窗层的项。
    pub fn popups(&self) -> impl Iterator<Item = GObjectId> + '_ {
        self.popups.iter().map(|(id, _)| *id)
    }

    /// 某一层上的界面数量。
    pub fn len(&self, layer: RootLayer) -> usize {
        match layer {
            RootLayer::Content => self.content.len(),
            RootLayer::Popup => self.popups.len(),
        }
    }

    /// 是否没有任何界面。
    pub fn is_empty(&self) -> bool {
        self.content.is_empty() && self.popups.is_empty()
    }

    /// 是否处于模态状态。
    ///
    /// 只要还有一个模态弹窗存在就为真，此时下层不应响应输入。
    pub fn is_modal(&self) -> bool {
        self.popups.iter().any(|(_, modal)| *modal)
    }

    /// 最上方的模态弹窗。若有它，输入只应交给它。
    pub fn top_modal(&self) -> Option<GObjectId> {
        self.popups
            .iter()
            .rev()
            .find(|(_, modal)| *modal)
            .map(|(id, _)| *id)
    }

    /// 整体缩放系数。
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// 设置整体缩放系数。过小的值没有意义，会被夹住。
    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale.max(0.01);
    }

    /// 把一个界面加入内容层。
    pub fn add_content(&mut self, store: &mut ObjectStore, id: GObjectId) {
        store.set_parent(id, Some(self.owner));
        self.content.push(id);
    }

    /// 把一个界面加入弹窗层。
    ///
    /// `modal` 为真时会挡住下层输入，直到它被移除。
    pub fn show_popup(&mut self, store: &mut ObjectStore, id: GObjectId, modal: bool) {
        store.set_parent(id, Some(self.owner));
        self.popups.push((id, modal));
    }

    /// Change modality without detaching or reordering an existing popup.
    pub fn set_popup_modal(&mut self, id: GObjectId, modal: bool) -> bool {
        let Some(entry) = self.popups.iter_mut().find(|(object, _)| *object == id) else {
            return false;
        };
        entry.1 = modal;
        true
    }

    /// 关闭某一层上的界面。返回是否确实存在。
    pub fn remove(&mut self, store: &mut ObjectStore, id: GObjectId) -> bool {
        if self.detach(store, id) {
            store.remove(id);
            true
        } else {
            false
        }
    }

    /// 从层中撤下但保留对象，供窗口隐藏与再次挂载使用。
    pub fn detach(&mut self, store: &mut ObjectStore, id: GObjectId) -> bool {
        if let Some(index) = self.content.iter().position(|item| *item == id) {
            self.content.remove(index);
            store.detach_from_parent(id);
            return true;
        }

        if let Some(index) = self.popups.iter().position(|(item, _)| *item == id) {
            self.popups.remove(index);
            store.detach_from_parent(id);
            return true;
        }

        false
    }

    /// 把某个弹窗提到最上方。
    ///
    /// 顺序即绘制顺序，因此重排之后要把根容器的子对象列表也同步过去。
    pub fn bring_to_front(&mut self, store: &mut ObjectStore, id: GObjectId) -> bool {
        let Some(index) = self.popups.iter().position(|(item, _)| *item == id) else {
            return false;
        };

        let entry = self.popups.remove(index);
        self.popups.push(entry);
        self.resync_order(store);
        true
    }

    /// 按当前层内顺序重建根容器的子对象列表。
    fn resync_order(&mut self, store: &mut ObjectStore) {
        let ordered: Vec<GObjectId> = self
            .content
            .iter()
            .copied()
            .chain(self.popups.iter().map(|(id, _)| *id))
            .collect();

        let extra: Vec<_> = store
            .object(self.owner)
            .map(|object| {
                object
                    .children()
                    .iter()
                    .copied()
                    .filter(|id| !ordered.contains(id))
                    .collect()
            })
            .unwrap_or_default();
        for (index, child) in extra.into_iter().chain(ordered).enumerate() {
            store.add_child_at(self.owner, child, index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::ObjectType;

    fn root_with_store() -> (ObjectStore, Root) {
        let mut store = ObjectStore::new();
        let owner = store.create(ObjectType::Component);
        store.set_size(owner, 1280.0, 720.0);
        (store, Root::new(owner))
    }

    #[test]
    fn content_and_popups_are_attached_to_the_root() {
        let (mut store, mut root) = root_with_store();
        let owner = root.owner();

        let content = store.create(ObjectType::Component);
        let popup = store.create(ObjectType::Component);
        root.add_content(&mut store, content);
        root.show_popup(&mut store, popup, false);

        assert_eq!(store.object(content).unwrap().parent(), Some(owner));
        assert_eq!(store.object(popup).unwrap().parent(), Some(owner));
        assert_eq!(root.len(RootLayer::Content), 1);
        assert_eq!(root.len(RootLayer::Popup), 1);
        assert!(!root.is_empty());
    }

    #[test]
    fn modal_popups_block_the_lower_layers() {
        let (mut store, mut root) = root_with_store();
        let popup = store.create(ObjectType::Component);

        assert!(!root.is_modal());
        assert_eq!(root.top_modal(), None);

        root.show_popup(&mut store, popup, true);
        assert!(root.is_modal());
        assert_eq!(root.top_modal(), Some(popup));

        root.remove(&mut store, popup);
        assert!(!root.is_modal(), "模态弹窗关闭后应解除阻挡");
    }

    #[test]
    fn top_modal_is_the_last_modal_popup() {
        let (mut store, mut root) = root_with_store();

        let first = store.create(ObjectType::Component);
        let second = store.create(ObjectType::Component);
        let plain = store.create(ObjectType::Component);

        root.show_popup(&mut store, first, true);
        root.show_popup(&mut store, plain, false);
        root.show_popup(&mut store, second, true);

        assert_eq!(
            root.top_modal(),
            Some(second),
            "应当返回最上方的那个模态弹窗"
        );
    }

    #[test]
    fn bring_to_front_reorders_children() {
        let (mut store, mut root) = root_with_store();

        let first = store.create(ObjectType::Component);
        let second = store.create(ObjectType::Component);
        root.show_popup(&mut store, first, false);
        root.show_popup(&mut store, second, false);

        assert_eq!(root.popups().collect::<Vec<_>>(), vec![first, second]);
        assert!(root.bring_to_front(&mut store, first));
        assert_eq!(root.popups().collect::<Vec<_>>(), vec![second, first]);

        // 根容器的子对象顺序也要跟着变，否则绘制顺序不会更新。
        let owner = root.owner();
        let children = store.object(owner).unwrap().children().to_vec();
        assert_eq!(children.last(), Some(&first));
    }

    #[test]
    fn removing_unknown_object_reports_failure() {
        let (mut store, mut root) = root_with_store();
        let orphan = store.create(ObjectType::Component);

        assert!(!root.remove(&mut store, orphan));
        assert!(!root.bring_to_front(&mut store, orphan));
        assert!(store.contains(orphan), "未加入根节点的对象不应被销毁");
    }

    #[test]
    fn scale_is_clamped_to_a_sane_minimum() {
        let (_store, mut root) = root_with_store();
        root.set_scale(0.0);
        assert!(root.scale() > 0.0);

        root.set_scale(2.0);
        assert!((root.scale() - 2.0).abs() < 1e-6);
    }
}

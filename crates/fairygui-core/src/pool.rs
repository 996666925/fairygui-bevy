//! 对象池。
//!
//! 列表滚动时会反复创建与回收项对象，池化能避免重复的构建开销。
//! 池按资源地址分组：同一个项模板的实例可以互相复用。

use std::collections::HashMap;

use crate::object::{GObjectId, ObjectStore};

/// 按资源地址缓存的对象池。
#[derive(Debug, Default)]
pub struct ObjectPool {
    /// 资源地址 -> 空闲对象。
    free: HashMap<String, Vec<GObjectId>>,
}

impl ObjectPool {
    /// 空池。
    pub fn new() -> Self {
        Self::default()
    }

    /// 池中空闲对象总数。
    pub fn len(&self) -> usize {
        self.free.values().map(Vec::len).sum()
    }

    /// 是否没有空闲对象。
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 取一个空闲对象。池中没有时返回 `None`，由调用方负责创建。
    pub fn acquire(&mut self, url: &str) -> Option<GObjectId> {
        self.free.get_mut(url)?.pop()
    }

    /// 取一个仍然有效的空闲对象。
    ///
    /// 与 [`Self::acquire`] 的区别是会校验句柄，失效的直接丢弃。
    pub fn acquire_checked(&mut self, store: &ObjectStore, url: &str) -> Option<GObjectId> {
        loop {
            let id = self.acquire(url)?;
            if store.contains(id) {
                return Some(id);
            }
        }
    }

    /// 归还一个对象。
    ///
    /// 会先把它从父节点上摘下并隐藏，这样复用时不必再处理残留状态。
    pub fn release(&mut self, store: &mut ObjectStore, url: impl Into<String>, id: GObjectId) {
        if !store.contains(id) {
            return;
        }
        store.detach_from_parent(id);
        store.set_visible(id, false);
        self.free.entry(url.into()).or_default().push(id);
    }

    /// 某一类对象的空闲数量。
    pub fn free_count(&self, url: &str) -> usize {
        self.free.get(url).map_or(0, Vec::len)
    }

    /// 清空池，并销毁其中的对象。
    pub fn clear(&mut self, store: &mut ObjectStore) {
        for (_, ids) in self.free.drain() {
            for id in ids {
                store.remove(id);
            }
        }
    }

    /// 只清空记录，不销毁对象。
    pub fn forget_all(&mut self) {
        self.free.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::ObjectType;

    #[test]
    fn release_then_acquire_returns_same_object() {
        let mut store = ObjectStore::new();
        let mut pool = ObjectPool::new();

        let id = store.create(ObjectType::Image);
        pool.release(&mut store, "ui://pkg/item", id);

        assert_eq!(pool.len(), 1);
        assert_eq!(pool.free_count("ui://pkg/item"), 1);
        assert_eq!(pool.acquire("ui://pkg/item"), Some(id));
        assert!(pool.is_empty());
    }

    #[test]
    fn released_object_is_detached_and_hidden() {
        let mut store = ObjectStore::new();
        let mut pool = ObjectPool::new();

        let parent = store.create(ObjectType::Component);
        let child = store.create(ObjectType::Image);
        store.set_parent(child, Some(parent));
        store.set_visible(child, true);

        pool.release(&mut store, "item", child);

        assert_eq!(store.object(child).unwrap().parent(), None);
        assert!(!store.object(child).unwrap().visible());
        assert!(store.object(parent).unwrap().children().is_empty());
    }

    #[test]
    fn acquire_on_unknown_url_returns_none() {
        let mut pool = ObjectPool::new();
        assert_eq!(pool.acquire("nothing"), None);
    }

    #[test]
    fn releasing_a_dead_handle_is_ignored() {
        let mut store = ObjectStore::new();
        let mut pool = ObjectPool::new();

        let id = store.create(ObjectType::Image);
        store.remove(id);
        pool.release(&mut store, "item", id);

        assert!(pool.is_empty());
    }

    #[test]
    fn checked_acquire_skips_dead_handles() {
        let mut store = ObjectStore::new();
        let mut pool = ObjectPool::new();

        let alive = store.create(ObjectType::Image);
        let dead = store.create(ObjectType::Image);
        pool.release(&mut store, "item", dead);
        pool.release(&mut store, "item", alive);
        store.remove(dead);

        assert_eq!(pool.acquire_checked(&store, "item"), Some(alive));
        assert!(pool.acquire_checked(&store, "item").is_none());
    }

    #[test]
    fn clear_destroys_pooled_objects() {
        let mut store = ObjectStore::new();
        let mut pool = ObjectPool::new();

        let id = store.create(ObjectType::Image);
        pool.release(&mut store, "item", id);
        pool.clear(&mut store);

        assert!(pool.is_empty());
        assert!(!store.contains(id));
    }

    #[test]
    fn forget_all_keeps_objects_alive() {
        let mut store = ObjectStore::new();
        let mut pool = ObjectPool::new();

        let id = store.create(ObjectType::Image);
        pool.release(&mut store, "item", id);
        pool.forget_all();

        assert!(pool.is_empty());
        assert!(store.contains(id));
    }
}

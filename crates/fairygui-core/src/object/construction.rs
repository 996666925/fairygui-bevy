//! Per-list FIFO pools, including rows prepared under AsyncOperation's budget.
use super::{GObjectId, ObjectStore};

#[cfg(test)]
#[path = "list_pool_tests.rs"]
mod tests;

impl ObjectStore {
    /// Unpublished asynchronous objects and unused prepared rows. Runtime
    /// services must defer dynamic content creation until they are released.
    pub fn is_constructing(&self, object: GObjectId) -> bool {
        self.constructing.contains(&object)
    }

    pub(crate) fn mark_constructing(&mut self, object: GObjectId) {
        self.constructing.insert(object);
    }

    pub(crate) fn finish_construction(&mut self, root: GObjectId) {
        let mut objects = vec![root];
        self.for_each_descendant(root, &mut |object| objects.push(object));
        for object in objects {
            self.constructing.remove(&object);
        }
    }

    pub(crate) fn cache_list_item_pool(
        &mut self,
        list: GObjectId,
        items: Vec<(String, GObjectId)>,
    ) {
        if !items.is_empty() {
            self.list_item_pools.insert(list, items.into());
        }
    }

    /// Take a cached row by canonical ui:// package/item IDs, consuming equal
    /// templates FIFO. Also publishes any asynchronously constructed row.
    /// Native getFromPool makes the row visible; the caller attaches it.
    pub fn take_list_item_from_pool(&mut self, list: GObjectId, url: &str) -> Option<GObjectId> {
        loop {
            let items = self.list_item_pools.get_mut(&list)?;
            let index = items.iter().position(|(key, _)| key == url)?;
            let (_, object) = items.remove(index)?;
            if items.is_empty() {
                self.list_item_pools.remove(&list);
            }
            if self.contains(object) {
                self.finish_construction(object);
                self.set_visible(object, true);
                return Some(object);
            }
        }
    }

    /// Detach a packaged list row and retain its complete subtree for FIFO
    /// reuse. The list owns pooled rows until disposal. Explicit visibility,
    /// child state and row entities survive pooling, matching native returnToPool.
    /// Reject overlays, foreign children, ownership cycles and duplicate returns.
    pub fn return_list_item_to_pool(&mut self, list: GObjectId, row: GObjectId) -> bool {
        if !matches!(
            self.object(list).map(|o| o.control()),
            Some(crate::widget::ControlData::List(_))
        ) || self.is_virtual_list(list)
            || row == list
            || self.is_scroll_overlay(row)
            || self
                .object(row)
                .is_none_or(|o| o.parent().is_some_and(|parent| parent != list))
            || self
                .list_item_pools
                .values()
                .any(|items| items.iter().any(|(_, item)| *item == row))
            || self
                .virtual_states
                .values()
                .any(|state| state.pool.iter().any(|(_, item)| *item == row))
        {
            return false;
        }
        let mut owned = Vec::new();
        self.collect_subtree(row, &mut owned);
        if owned.contains(&list) {
            return false;
        }
        let Some(object) = self.object(row) else {
            return false;
        };
        let Some((package, item)) = object.source_package_id().zip(object.package_item_id()) else {
            return false;
        };
        let url = format!("ui://{package}{item}");
        self.detach_from_parent(row);
        self.detach_from_stage(row);
        self.list_item_pools
            .entry(list)
            .or_default()
            .push_back((url, row));
        true
    }
}

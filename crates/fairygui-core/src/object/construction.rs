//! Ownership of detached rows prepared by AsyncOperation's list-item pool.
use super::{GObjectId, ObjectStore};

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

    pub(crate) fn cache_prepared_list_items(
        &mut self,
        list: GObjectId,
        items: Vec<(String, GObjectId)>,
    ) {
        if !items.is_empty() {
            self.prepared_list_items.insert(list, items.into());
        }
    }

    /// Take a row built under the asynchronous object's budget. Keys are
    /// canonical ui:// package/item IDs. Equal templates are consumed FIFO.
    /// Taking publishes the row's model; the caller attaches it to the list.
    pub fn take_prepared_list_item(&mut self, list: GObjectId, url: &str) -> Option<GObjectId> {
        loop {
            let items = self.prepared_list_items.get_mut(&list)?;
            let index = items.iter().position(|(key, _)| key == url)?;
            let (_, object) = items.remove(index)?;
            if items.is_empty() {
                self.prepared_list_items.remove(&list);
            }
            if self.contains(object) {
                self.finish_construction(object);
                return Some(object);
            }
        }
    }
}

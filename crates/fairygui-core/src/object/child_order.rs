//! Native GComponent child indices and GObject.sortingOrder.
use super::*;

#[cfg(test)]
#[path = "child_order_tests.rs"]
mod tests;

impl ObjectStore {
    fn ordinary_child_count(&self, parent: GObjectId) -> usize {
        self.objects[parent]
            .children
            .iter()
            .filter(|&&id| self.objects[id].sorting_order == 0)
            .count()
    }

    pub(super) fn child_insertion_index(&self, parent: GObjectId, child: GObjectId) -> usize {
        let order = self.objects[child].sorting_order;
        self.objects[parent]
            .children
            .iter()
            .position(|&id| self.objects[id].sorting_order > order)
            .unwrap_or(self.objects[parent].children.len())
    }

    fn move_child_index(
        &mut self,
        parent: GObjectId,
        child: GObjectId,
        index: usize,
    ) -> Option<usize> {
        let current = self.child_index(child)?;
        let index = index.min(self.objects[parent].children.len().saturating_sub(1));
        if current != index {
            self.objects[parent].children.remove(current);
            self.objects[parent].children.insert(index, child);
            self.touch(parent, dirty::CHILDREN);
            self.refresh_list_selection_snapshot(parent);
            self.layout_list(parent);
            self.refresh_stage();
        }
        Some(index)
    }

    /// Move an ordinary child within the ordinary prefix. A nonzero
    /// sortingOrder ignores explicit indices, matching GComponent.setChildIndex.
    /// Returns the final index, or None for an unparented/missing object.
    pub fn set_child_index(&mut self, child: GObjectId, index: usize) -> Option<usize> {
        let object = self.object(child)?;
        let parent = object.parent()?;
        if object.sorting_order != 0 {
            return self.child_index(child);
        }
        let index = index.min(self.ordinary_child_count(parent).saturating_sub(1));
        self.move_child_index(parent, child, index)
    }

    /// Index before removal of this child, as in setChildIndexBefore. Forward
    /// moves subtract one after clamping to the ordinary-child boundary.
    pub fn set_child_index_before(&mut self, child: GObjectId, index: usize) -> Option<usize> {
        let object = self.object(child)?;
        let parent = object.parent()?;
        let old = self.child_index(child)?;
        if object.sorting_order != 0 {
            return Some(old);
        }
        let count = self.objects[parent].children.len();
        let ordinary = self.ordinary_child_count(parent);
        let index = if ordinary < count {
            index.min(ordinary.saturating_sub(1))
        } else {
            index
        };
        self.move_child_index(
            parent,
            child,
            if old < index {
                index.saturating_sub(1)
            } else {
                index
            },
        )
    }

    /// Native swapChildren is two setChildIndex calls. Sorted children keep
    /// their slots; swapping one with an ordinary child can still move that one.
    pub fn swap_children(&mut self, first: GObjectId, second: GObjectId) -> bool {
        let Some(parent) = self.object(first).and_then(|o| o.parent()) else {
            return false;
        };
        if self.object(second).and_then(|o| o.parent()) != Some(parent) {
            return false;
        }
        let (Some(a), Some(b)) = (self.child_index(first), self.child_index(second)) else {
            return false;
        };
        self.set_child_index(first, b);
        self.set_child_index(second, a);
        self.child_index(first) != Some(a) || self.child_index(second) != Some(b)
    }

    /// Nonnegative priority; 0 rejoins the end of ordinary siblings. Equal
    /// nonzero priorities follow existing peers when added or changed.
    pub fn set_sorting_order(&mut self, child: GObjectId, order: i32) -> bool {
        let order = order.max(0);
        let Some(object) = self.objects.get_mut(child) else {
            return false;
        };
        if object.sorting_order == order {
            return false;
        }
        object.sorting_order = order;
        let Some(parent) = object.parent else {
            return true;
        };
        if order == 0 {
            self.set_child_index(child, usize::MAX);
        } else {
            let old = self.child_index(child).unwrap();
            let index = self.objects[parent]
                .children
                .iter()
                .enumerate()
                .find_map(|(index, &id)| {
                    (id != child && self.objects[id].sorting_order > order).then_some(index)
                })
                .unwrap_or(self.objects[parent].children.len());
            self.move_child_index(
                parent,
                child,
                if old < index {
                    index.saturating_sub(1)
                } else {
                    index
                },
            );
        }
        true
    }
}

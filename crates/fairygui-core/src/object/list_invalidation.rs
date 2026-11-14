//! Deferred materialized-list bounds, matching native setBoundsChangedFlag.
use super::{GObjectId, ObjectStore};

#[cfg(test)]
#[path = "list_invalidation_tests.rs"]
mod tests;

impl ObjectStore {
    pub(super) fn invalidate_list_layout(&mut self, list: GObjectId) {
        if !self.laying_out_lists.contains(&list)
            && !self.is_virtual_list(list)
            && self.list_data(list).is_some()
        {
            self.pending_list_layouts.insert(list);
        }
    }

    pub(super) fn invalidate_parent_list_layout(&mut self, row: GObjectId) {
        if !self.is_scroll_overlay(row)
            && let Some(parent) = self.object(row).and_then(|o| o.parent())
        {
            self.invalidate_list_layout(parent);
        }
    }

    /// Reflow lists whose viewport, row size, visibility or membership changed.
    /// Call after a batch of property edits, before reading layout geometry.
    /// Returns the number of lists laid out, including empty lists.
    pub fn flush_list_layouts(&mut self) -> usize {
        let mut count = 0;
        while !self.pending_list_layouts.is_empty() {
            let mut lists: Vec<_> = self.pending_list_layouts.iter().copied().collect();
            // Outer lists assign nested-list viewports before those lists lay
            // out their own rows. A completed layout consumes its invalidation.
            lists.sort_by_key(|&list| {
                let mut depth = 0;
                let mut parent = self.object(list).and_then(|o| o.parent());
                while let Some(id) = parent {
                    depth += 1;
                    parent = self.object(id).and_then(|o| o.parent());
                }
                depth
            });
            for list in lists {
                if self.pending_list_layouts.contains(&list) {
                    self.layout_list(list);
                    count += 1;
                }
            }
        }
        count
    }
}

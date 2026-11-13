//! Ordinary list configuration with package-aware row construction.
use super::*;
use crate::list::GuiListContentError;

impl GuiRuntime {
    pub(crate) fn configure_list_content(
        &mut self,
        list: GObjectId,
        count: usize,
        mut render: impl FnMut(&mut ObjectStore, usize, GObjectId),
    ) -> Result<(), GuiListContentError> {
        let Some(object) = self.store.object(list) else {
            return Err(GuiListContentError::NotList);
        };
        let ControlData::List(data) = object.control() else {
            return Err(GuiListContentError::NotList);
        };
        if self.store.is_virtual_list(list) {
            return Err(GuiListContentError::VirtualList);
        }
        let mut rows = self.store.list_items(list).unwrap();
        // Resolve only when growing: zero, shrinking and editing existing rows
        // remain valid even if there is no default template.
        if count > rows.len() {
            let package = object
                .source_package_id()
                .and_then(|id| self.registry.get_by_id(id))
                .cloned()
                .ok_or(GuiListContentError::MissingDefaultItem)?;
            let reference = data
                .default_item
                .as_deref()
                .ok_or(GuiListContentError::MissingDefaultItem)?;
            let template = self
                .resolve_template(&package, reference)
                .ok_or(GuiListContentError::MissingDefaultItem)?;
            let mut created = Vec::new();
            for _ in rows.len()..count {
                let Some(row) = self.create_template(&template) else {
                    for row in created {
                        self.store.remove(row);
                    }
                    return Err(GuiListContentError::CreationFailed(template.url));
                };
                self.prepare_subtree(&template.package, row);
                created.push(row);
            }
            for row in created {
                self.store.set_parent(row, Some(list));
                rows.push(row);
            }
        }
        for row in rows.drain(count.min(rows.len())..) {
            self.store.remove(row);
        }
        for (index, row) in rows.into_iter().enumerate() {
            render(&mut self.store, index, row);
        }
        self.store.layout_list(list);
        self.store.layout_scroll_bars(list);
        self.queue_subtree(list);
        Ok(())
    }
}

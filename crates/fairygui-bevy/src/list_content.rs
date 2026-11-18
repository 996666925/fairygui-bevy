//! Ordinary list configuration with package-aware row construction.
use super::*;
use crate::list::GuiListContentError;

impl GuiRuntime {
    pub(crate) fn configure_list_content(
        &mut self,
        list: GObjectId,
        count: usize,
        mut provider: impl FnMut(usize) -> Option<String>,
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
                .cloned();
            let default = data.default_item.clone();
            // Resolve every new row before acquiring objects. A missing late
            // package or a panicking provider cannot leak detached candidates
            // or partially replace the existing content.
            let mut templates = Vec::new();
            for index in rows.len()..count {
                let provided = provider(index).filter(|url| !url.is_empty());
                let reference = provided
                    .as_ref()
                    .or(default.as_ref())
                    .ok_or(GuiListContentError::MissingDefaultItem)?;
                let owner = package
                    .as_ref()
                    .or_else(|| self.registry.resolve_url(reference).map(|(owner, _)| owner));
                let template = owner
                    .and_then(|owner| self.resolve_template(owner, reference))
                    .ok_or_else(|| {
                        if provided.is_some() {
                            GuiListContentError::MissingResource(reference.clone())
                        } else {
                            GuiListContentError::MissingDefaultItem
                        }
                    })?;
                templates.push(template);
            }
            // Keep pool acquisition and its visible=true side effect pending
            // until every fresh template is constructed. Returning candidates
            // after failure would reorder the FIFO and change hidden rows.
            let mut available = HashMap::new();
            let mut pending: Vec<(templates::ResolvedTemplate, Option<GObjectId>)> = Vec::new();
            for template in templates {
                let cached = available
                    .entry(template.url.clone())
                    .or_insert_with(|| self.store.list_item_pool_count(list, &template.url));
                if *cached > 0 {
                    *cached -= 1;
                    pending.push((template, None));
                    continue;
                }
                let Some(row) = self.create_template(&template) else {
                    for (_, row) in pending {
                        if let Some(row) = row {
                            for removed in self.store.remove(row) {
                                self.object_assets.remove(&removed);
                            }
                        }
                    }
                    return Err(GuiListContentError::CreationFailed(template.url));
                };
                self.prepare_subtree(&template.package, row);
                pending.push((template, Some(row)));
            }
            for (template, fresh) in pending {
                // No application callbacks run between reservation and commit;
                // construction only touches newly allocated, detached subtrees.
                let row = fresh.unwrap_or_else(|| {
                    self.store
                        .take_list_item_from_pool(list, &template.url)
                        .expect("reserved list row remains in its pool")
                });
                if !self.object_assets.contains_key(&row) {
                    self.remember_asset_tree(row, &template.handle);
                    self.prepare_subtree(&template.package, row);
                }
                self.store.set_visible(row, true);
                self.store.set_parent(row, Some(list));
                rows.push(row);
            }
        }
        for row in rows.drain(count.min(rows.len())..) {
            if !self.store.return_list_item_to_pool(list, row) {
                self.store.remove(row);
            }
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

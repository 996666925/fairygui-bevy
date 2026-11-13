//! Transactional keyed tree row construction; package semantics stay in core.
use super::*;
use crate::tree::{ContentBinding, ContentRow, GuiTreeContent, GuiTreeContentError, GuiTreeItem};

impl GuiRuntime {
    pub(crate) fn configure_tree_content(
        &mut self,
        tree: GObjectId,
        config: &GuiTreeContent,
        binding: Option<&ContentBinding>,
    ) -> Result<ContentBinding, GuiTreeContentError> {
        use GuiTreeContentError as Error;
        fn flatten<'a>(
            items: &'a [GuiTreeItem],
            level: u16,
            keys: &mut std::collections::HashSet<&'a str>,
            flat: &mut Vec<(u8, &'a GuiTreeItem)>,
        ) -> Result<(), Error> {
            if !items.is_empty() && level > u16::from(u8::MAX) {
                return Err(Error::TooDeep);
            }
            for item in items {
                if item.key.is_empty() {
                    return Err(Error::EmptyKey);
                }
                if !keys.insert(&item.key) {
                    return Err(Error::DuplicateKey(item.key.clone()));
                }
                if !item.is_folder && !item.children.is_empty() {
                    return Err(Error::LeafChildren(item.key.clone()));
                }
                flat.push((level as u8, item));
                flatten(&item.children, level + 1, keys, flat)?;
            }
            Ok(())
        }
        let Some(object) = self.store.object(tree) else {
            return Err(Error::NotTree);
        };
        let ControlData::Tree(data) = object.control() else {
            return Err(Error::NotTree);
        };
        let default_item = data.list.default_item.clone();
        let package = object
            .source_package_id()
            .and_then(|id| self.registry.get_by_id(id))
            .cloned();
        let mut flat = Vec::new();
        flatten(
            &config.items,
            0,
            &mut std::collections::HashSet::new(),
            &mut flat,
        )?;
        let old_rows = self.store.list_items(tree).unwrap_or_default();
        let mut prior = HashMap::new();
        if let Some(binding) = binding.filter(|binding| binding.object == tree) {
            for row in &binding.rows {
                if let Some(index) = old_rows.iter().position(|&object| object == row.object) {
                    prior.insert(row.key.as_str(), (row, data.is_expanded(index)));
                }
            }
        }
        // Resolve every requested template before constructing/mutating rows.
        let mut templates = Vec::new();
        for (_, item) in &flat {
            let reference = item
                .template
                .as_deref()
                .or(default_item.as_deref())
                .unwrap_or_default();
            let template = package
                .as_ref()
                .and_then(|package| self.resolve_template(package, reference))
                .ok_or_else(|| Error::MissingTemplate(reference.to_owned()))?;
            templates.push(template);
        }
        let mut rows = Vec::new();
        let mut expanded = Vec::new();
        let mut created = Vec::new();
        for ((_, item), template) in flat.iter().zip(&templates) {
            let previous = prior
                .get(item.key.as_str())
                .filter(|(row, _)| row.template == template.url && row.is_folder == item.is_folder);
            let row = if let Some((row, _)) = previous {
                row.object
            } else {
                let Some(row) = self.create_template(template) else {
                    for row in created {
                        for removed in self.store.remove(row) {
                            self.object_assets.remove(&removed);
                        }
                    }
                    return Err(Error::CreationFailed(template.url.clone()));
                };
                created.push(row);
                if self
                    .store
                    .object(row)
                    .is_none_or(|object| object.control().component().is_none())
                {
                    for row in created {
                        for removed in self.store.remove(row) {
                            self.object_assets.remove(&removed);
                        }
                    }
                    return Err(Error::InvalidTemplate(template.url.clone()));
                }
                self.prepare_subtree(&template.package, row);
                row
            };
            rows.push(ContentRow {
                key: item.key.clone(),
                template: template.url.clone(),
                object: row,
                is_folder: item.is_folder,
            });
            expanded.push(item.expanded.unwrap_or_else(|| {
                previous.is_some_and(|(row, expanded)| row.is_folder == item.is_folder && *expanded)
            }));
        }
        let retained: std::collections::HashSet<_> = rows.iter().map(|row| row.object).collect();
        for row in old_rows {
            if !retained.contains(&row) {
                for removed in self.store.remove(row) {
                    self.object_assets.remove(&removed);
                }
            }
        }
        for (index, row) in rows.iter().enumerate() {
            self.store.add_child_at(tree, row.object, index);
        }
        let nodes = flat
            .iter()
            .zip(&templates)
            .map(
                |((level, item), template)| fairygui_core::widget::TreeNodeData {
                    url: Some(template.url.clone()),
                    is_folder: item.is_folder,
                    level: *level,
                },
            )
            .collect();
        let items = flat
            .iter()
            .zip(&templates)
            .map(
                |((_, item), template)| fairygui_core::widget::ListItemData {
                    url: Some(template.url.clone()),
                    title: Some(item.title.clone()),
                    icon: item.icon.clone(),
                    ..default()
                },
            )
            .collect();
        let valid = self.store.replace_tree_nodes(tree, nodes, items, expanded);
        debug_assert!(
            valid,
            "validated nested tree must produce valid preorder metadata"
        );
        for index in 0..rows.len() {
            self.store.apply_tree_item(tree, index);
        }
        self.store.initialize_tree_cells(tree);
        self.store.layout_scroll_bars(tree);
        self.queue_subtree(tree);
        Ok(ContentBinding { object: tree, rows })
    }
}

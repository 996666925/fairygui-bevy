use super::*;
use fairygui_core::{geom::Size, package::PackageItemType};

#[derive(Debug)]
pub(super) struct LoadedComponent {
    content: GObjectId,
    url: String,
    source: Size,
    // Retain the content's package independently of the loader's source package.
    _handle: Handle<FuiAsset>,
}

impl GuiRuntime {
    /// Component currently owned by this loader, equivalent to GLoader.component.
    /// Its lifetime is controlled by the loader's URL and disposal. Use its ID
    /// to inspect or customize the loaded controls, controllers and transitions.
    pub fn loader_component(&self, loader: GObjectId) -> Option<GObjectId> {
        self.loader_components
            .get(&loader)
            .map(|loaded| loaded.content)
            .filter(|content| self.store.contains(*content))
    }

    fn loader_url(&self, loader: GObjectId) -> Option<&str> {
        match self.store.object(loader)?.control() {
            ControlData::Loader(data) => Some(data.url.as_deref().unwrap_or("")),
            _ => None,
        }
    }

    fn clear_loader_component(&mut self, commands: &mut Commands, loader: GObjectId) {
        if let Some(loaded) = self.loader_components.remove(&loader)
            && self.store.contains(loaded.content)
        {
            self.despawn_subtree(commands, loaded.content);
        }
    }

    pub(super) fn initialize_loaded_subtree(
        &mut self,
        package: &FuiPackage,
        content: GObjectId,
        handle: &Handle<FuiAsset>,
    ) {
        self.materialize_list_items(package, content);
        attach_scroll_bars(&mut self.store, &self.registry, package, content);
        let mut initialized = vec![content];
        self.store
            .for_each_descendant(content, &mut |id| initialized.push(id));
        for &id in &initialized {
            self.store.apply_title_of(id);
        }
        for &id in &initialized {
            let count = self
                .store
                .object(id)
                .map_or(0, |object| object.children().len());
            for index in 0..count {
                self.store.apply_list_item(id, index);
            }
        }
        for &id in &initialized {
            self.store.layout_group(id);
            self.store.layout_list(id);
        }
        layout_scroll_bars(&mut self.store, content);
        self.remember_asset_tree(content, handle);
    }

    pub(super) fn loader_would_recurse(&self, loader: GObjectId, url: &str) -> bool {
        let mut current = Some(loader);
        while let Some(id) = current {
            if self.resource_url_for(id).as_deref() == Some(url) {
                return true;
            }
            current = self.store.object(id).and_then(|object| object.parent());
        }
        false
    }
}

/// Work from the model rather than ECS views, so initial and nested content is
/// created before its first view frame. URL replacement destroys old ownership
/// even if the new resource is missing. Retained hidden owners keep their content.
pub(crate) fn sync_loader_components(
    mut commands: Commands,
    mut runtime: NonSendMut<GuiRuntime>,
    mut assets: ResMut<Assets<FuiAsset>>,
) {
    let gui = &mut *runtime;
    let obsolete: Vec<_> = gui
        .loader_components
        .iter()
        .filter_map(|(loader, loaded)| {
            (gui.loader_retries.contains(loader)
                || gui.loader_url(*loader) != Some(&loaded.url)
                || gui
                    .store
                    .object(loaded.content)
                    .is_none_or(|content| content.parent() != Some(*loader)))
            .then_some(*loader)
        })
        .collect();
    for loader in obsolete {
        gui.clear_loader_component(&mut commands, loader);
    }
    gui.failed_loader_components.retain(|(loader, url)| {
        matches!(gui.store.object(*loader).map(|object| object.control()), Some(ControlData::Loader(data)) if data.url.as_deref() == Some(url))
    });
    let mut pending: std::collections::VecDeque<_> = gui
        .store
        .iter()
        .filter(|(_, object)| matches!(object.control(), ControlData::Loader(_)))
        .map(|(id, _)| id)
        .collect();
    let mut visited = std::collections::HashSet::new();
    while let Some(loader) = pending.pop_front() {
        if !visited.insert(loader) || gui.loader_components.contains_key(&loader) {
            continue;
        }
        let Some(url) = gui
            .loader_url(loader)
            .filter(|url| url.starts_with("ui://"))
            .map(str::to_owned)
        else {
            continue;
        };
        let Some((asset_id, package, index)) = assets.iter().find_map(|(id, asset)| {
            let index = asset.package.resolve_local_url(&url)?;
            (asset.package.item(index)?.item_type == PackageItemType::Component)
                .then(|| (id, asset.package.clone(), index))
        }) else {
            continue;
        };
        let Some(handle) = assets.get_strong_handle(asset_id) else {
            continue;
        };
        gui.register_package(handle.clone(), &package);
        let template = templates::ResolvedTemplate {
            package: std::sync::Arc::new(package.clone()),
            index,
            url: format!("ui://{}{}", package.id(), package.item(index).unwrap().id),
            handle: handle.clone(),
        };
        if gui.loader_would_recurse(loader, &template.url) {
            if gui.failed_loader_components.insert((loader, url.clone())) {
                tracing::warn!(?loader, %url, "recursive loader component reference rejected");
            }
            continue;
        }
        let Some(content) = gui.create_template(&template) else {
            continue;
        };
        if gui
            .store
            .object(content)
            .is_none_or(|object| object.control().component().is_none())
        {
            gui.store.remove(content);
            continue;
        }
        let index = package.resolve_branch(index, &gui.registry.load_context(&package));
        let item = package.item(index).unwrap();
        let source = Size::new(item.width, item.height);
        gui.initialize_loaded_subtree(&package, content, &handle);
        gui.store.set_parent(content, Some(loader));
        gui.loader_components.insert(
            loader,
            LoadedComponent {
                content,
                url,
                source,
                _handle: handle,
            },
        );
        gui.store.layout_loader_component(loader, content, source);
        gui.queue_subtree(content);
        gui.store
            .for_each_descendant(content, &mut |id| pending.push_back(id));
    }
    // Replacing an outer component also disposed its nested loader owners.
    // Drop those ownership records in this synchronization, not a later frame.
    let dead: Vec<_> = gui
        .loader_components
        .keys()
        .copied()
        .filter(|loader| !gui.store.contains(*loader))
        .collect();
    for loader in dead {
        gui.clear_loader_component(&mut commands, loader);
    }
    // Parent resizing may change a nested loader through relations. Layout outer
    // owners before inner ones, keeping all resulting geometry in this frame.
    let mut layouts: Vec<_> = gui
        .loader_components
        .iter()
        .map(|(loader, loaded)| {
            let mut depth = 0;
            let mut current = gui.store.object(*loader).and_then(|object| object.parent());
            while let Some(id) = current {
                depth += 1;
                current = gui.store.object(id).and_then(|object| object.parent());
            }
            (depth, *loader, loaded.content, loaded.source)
        })
        .collect();
    layouts.sort_by_key(|(depth, ..)| *depth);
    for (_, loader, content, source) in layouts {
        gui.store.layout_loader_component(loader, content, source);
        layout_scroll_bars(&mut gui.store, content);
    }
}

#[cfg(test)]
#[path = "loader_component_tests.rs"]
mod tests;

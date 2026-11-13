//! Automatic, synchronous visible-row rendering with retained per-list pools.
use super::*;
use fairygui_core::object::{VirtualItemProvider, VirtualItemRenderer, VirtualListError};

pub(super) struct VirtualCallbacks {
    renderer: Box<VirtualItemRenderer<'static>>,
    provider: Box<VirtualItemProvider<'static>>,
}

impl GuiRuntime {
    /// Enable native virtual layout. The callback must finish updating
    /// the row's size synchronously; only visible rows are instantiated.
    pub fn enable_virtual_list(
        &mut self,
        list: GObjectId,
        renderer: impl FnMut(&mut ObjectStore, usize, GObjectId) + 'static,
    ) -> Result<(), VirtualListError> {
        self.enable_virtual_renderer(list, false, renderer)
    }

    /// Native setVirtualAndLoop: six internal copies, logical renderer indices
    /// and ScrollPane recentering. Flow layouts are rejected before mutation.
    pub fn enable_loop_virtual_list(
        &mut self,
        list: GObjectId,
        renderer: impl FnMut(&mut ObjectStore, usize, GObjectId) + 'static,
    ) -> Result<(), VirtualListError> {
        self.enable_virtual_renderer(list, true, renderer)
    }

    fn enable_virtual_renderer(
        &mut self,
        list: GObjectId,
        looping: bool,
        renderer: impl FnMut(&mut ObjectStore, usize, GObjectId) + 'static,
    ) -> Result<(), VirtualListError> {
        let mut callbacks = VirtualCallbacks {
            renderer: Box::new(renderer),
            provider: Box::new(|_| None),
        };
        let result = self.render_virtual_rows(list, &mut callbacks, Some(looping), None);
        if self.store.is_virtual_list(list)
            && !matches!(result, Err(VirtualListError::LoopModeMismatch))
        {
            self.virtual_renderers.insert(list, callbacks);
        }
        result.map(|_| ())
    }

    pub(crate) fn configure_ecs_virtual_list(
        &mut self,
        list: GObjectId,
        count: usize,
        looping: bool,
        renderer: impl FnMut(&mut ObjectStore, usize, GObjectId) + 'static,
    ) -> Result<(), VirtualListError> {
        let mut callbacks = VirtualCallbacks {
            renderer: Box::new(renderer),
            provider: Box::new(|_| None),
        };
        let result = self.render_virtual_rows(list, &mut callbacks, Some(looping), Some(count));
        if self.store.is_virtual_list(list)
            && !matches!(result, Err(VirtualListError::LoopModeMismatch))
        {
            self.virtual_renderers.insert(list, callbacks);
        }
        result.map(|_| ())
    }

    pub(crate) fn clear_ecs_virtual_list(&mut self, list: GObjectId) {
        if self.store.is_virtual_list(list) {
            let _ = self.set_virtual_item_count(list, 0);
        }
        self.virtual_renderers.remove(&list);
    }

    /// Provider URLs accept the same aliases and cross-package forms as normal
    /// package construction; pooled identities use normalized package/item IDs.
    pub fn set_virtual_item_provider(
        &mut self,
        list: GObjectId,
        provider: impl FnMut(usize) -> Option<String> + 'static,
    ) -> Result<(), VirtualListError> {
        let callbacks = self
            .virtual_renderers
            .get_mut(&list)
            .ok_or(VirtualListError::MissingPane)?;
        callbacks.provider = Box::new(provider);
        self.store.invalidate_virtual_list(list, false);
        self.refresh_virtual_list(list).map(|_| ())
    }

    /// Set the logical count and immediately render the visible range.
    pub fn set_virtual_item_count(
        &mut self,
        list: GObjectId,
        count: usize,
    ) -> Result<(), VirtualListError> {
        self.store.set_virtual_item_count(list, count)?;
        self.refresh_virtual_list(list).map(|_| ())
    }

    /// Force renderer calls after application data changes.
    pub fn invalidate_virtual_list(&mut self, list: GObjectId) -> Result<(), VirtualListError> {
        if !self.store.invalidate_virtual_list(list, false) {
            return Err(VirtualListError::MissingPane);
        }
        self.refresh_virtual_list(list).map(|_| ())
    }

    /// Refresh after direct scroll/layout mutations. The frame pipeline also
    /// does this automatically before spawning and mirroring row views.
    pub fn refresh_virtual_list(&mut self, list: GObjectId) -> Result<usize, VirtualListError> {
        let mut callbacks = self
            .virtual_renderers
            .remove(&list)
            .ok_or(VirtualListError::MissingPane)?;
        let result = self.render_virtual_rows(list, &mut callbacks, None, None);
        if self.store.is_virtual_list(list) {
            self.virtual_renderers.insert(list, callbacks);
        }
        result
    }

    fn render_virtual_rows(
        &mut self,
        list: GObjectId,
        callbacks: &mut VirtualCallbacks,
        enable: Option<bool>,
        count: Option<usize>,
    ) -> Result<usize, VirtualListError> {
        self.store.layout_scroll_bars(list);
        let object = self.store.object(list).ok_or(VirtualListError::Removed)?;
        let package = object
            .source_package_id()
            .and_then(|id| self.registry.get_by_id(id))
            .cloned()
            .ok_or(VirtualListError::MissingDefaultItem)?;
        let ControlData::List(data) = object.control() else {
            return Err(VirtualListError::MissingPane);
        };
        let reference = data
            .default_item
            .as_deref()
            .ok_or(VirtualListError::MissingDefaultItem)?;
        let registry = &self.registry;
        let handles = &self.package_handles;
        let resolve = |reference: &str| {
            let (owner, index) = if let Some(index) = package.resolve_local_url(reference) {
                (package.clone(), index)
            } else if reference.starts_with("ui://") {
                let (owner, index) = registry.resolve_url(reference)?;
                (owner.clone(), index)
            } else {
                (package.clone(), package.item_index_by_name(reference)?)
            };
            let url = format!("ui://{}{}", owner.id(), owner.item(index)?.id);
            Some(templates::ResolvedTemplate {
                handle: handles.get(owner.id())?.clone(),
                package: owner,
                index,
                url,
            })
        };
        let default_url = resolve(reference)
            .ok_or_else(|| VirtualListError::MissingResource(reference.into()))?
            .url;
        let mut created = Vec::new();
        let mut factory = |store: &mut ObjectStore, url: &str| {
            let resource = resolve(url)?;
            let id = fairygui_core::widget::create_from_package_with_registry(
                store,
                &resource.package,
                resource.index,
                &registry.load_context(&resource.package),
                registry,
            )
            .ok()?;
            created.push((id, resource));
            Some(id)
        };
        let mut provider = |index| {
            let reference = (callbacks.provider)(index)?;
            if reference.is_empty() {
                return None;
            }
            Some(resolve(&reference).map_or(reference, |resource| resource.url))
        };
        let result = (|| {
            if let Some(looping) = enable {
                if looping {
                    self.store
                        .enable_loop_virtual_list(list, &default_url, &mut factory)?;
                } else {
                    self.store
                        .enable_virtual_list(list, &default_url, &mut factory)?;
                }
            }
            if let Some(count) = count {
                self.store.set_virtual_item_count(list, count)?;
            }
            self.store.refresh_virtual_list(
                list,
                &mut factory,
                &mut provider,
                callbacks.renderer.as_mut(),
            )
        })();
        // Asset ownership must survive detachment and cross-package pooling.
        for (id, resource) in created {
            if !self.store.contains(id) {
                continue;
            }
            self.object_assets.insert(id, resource.handle.clone());
            self.remember_asset_tree(id, &resource.handle);
            self.prepare_subtree(&resource.package, id);
        }
        if self.store.contains(list) {
            self.queue_subtree(list);
        }
        result
    }
}

pub(crate) fn sync_virtual_lists(mut runtime: NonSendMut<GuiRuntime>) {
    let removed: Vec<_> = runtime
        .virtual_renderers
        .keys()
        .copied()
        .filter(|&list| !runtime.store.is_virtual_list(list))
        .collect();
    for list in removed {
        runtime.virtual_renderers.remove(&list);
    }
    let lists: Vec<_> = runtime.virtual_renderers.keys().copied().collect();
    for list in lists {
        if let Err(error) = runtime.refresh_virtual_list(list) {
            tracing::warn!(?list, %error, "virtual list refresh failed");
        }
    }
}

#[cfg(test)]
#[path = "virtual_list_tests.rs"]
mod tests;

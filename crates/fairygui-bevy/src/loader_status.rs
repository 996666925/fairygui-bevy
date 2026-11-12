use super::*;
use bevy::asset::LoadState;
use fairygui_core::package::PackageItemType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiLoaderFailure {
    MissingResource,
    UnsupportedResource,
    ComponentConstruction,
    ImageLoad,
}

/// Current adapter load state, updated during Sync. Missing ui:// packages fail
/// immediately, as native lookup does, and may recover when registered/loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiLoaderState {
    Empty,
    Loading,
    Ready,
    Failed(GuiLoaderFailure),
}

pub(super) struct LoaderStatus {
    url: String,
    state: GuiLoaderState,
    sign: Option<(GObjectId, String)>,
}

impl GuiRuntime {
    /// Native UIConfig.loaderErrorSign URL. It must identify an exported package
    /// resource. Changes affect subsequent failures, not already displayed signs.
    pub fn set_loader_error_sign(&mut self, url: Option<&str>) {
        self.loader_error_template = url.filter(|url| !url.is_empty()).map(str::to_owned);
    }

    pub fn loader_error_sign_url(&self) -> Option<&str> {
        self.loader_error_template.as_deref()
    }

    pub fn loader_load_state(&self, loader: GObjectId) -> Option<GuiLoaderState> {
        self.loader_status.get(&loader).map(|status| status.state)
    }

    pub fn loader_error_sign(&self, loader: GObjectId) -> Option<GObjectId> {
        self.loader_status
            .get(&loader)?
            .sign
            .as_ref()
            .map(|(sign, _)| *sign)
            .filter(|sign| self.store.contains(*sign))
    }

    /// Retry the current URL without changing the loader object. This adapter
    /// convenience also reloads failed image requests. Existing error signs are
    /// returned to the pool; a fresh failure evaluates showErrorSign/config again.
    pub fn retry_loader(&mut self, loader: GObjectId) -> bool {
        if !matches!(
            self.store.object(loader).map(|object| object.control()),
            Some(ControlData::Loader(_))
        ) {
            return false;
        }
        self.loader_retries.insert(loader);
        true
    }

    fn release_loader_error(&mut self, sign: (GObjectId, String)) {
        if self.store.contains(sign.0) {
            self.store.detach_from_parent(sign.0);
            self.store.detach_from_stage(sign.0);
            self.loader_error_pool
                .entry(sign.1)
                .or_default()
                .push_back(sign.0);
        }
    }

    fn acquire_loader_error(
        &mut self,
        template: &templates::ResolvedTemplate,
    ) -> Option<GObjectId> {
        // Native error-sign pools are FIFO and preserve all state except size.
        while let Some(sign) = self
            .loader_error_pool
            .get_mut(&template.url)
            .and_then(|pool| pool.pop_front())
        {
            if self.store.contains(sign) {
                return Some(sign);
            }
        }
        let sign = self.create_template(template)?;
        self.initialize_loaded_subtree(&template.package, sign, &template.handle);
        Some(sign)
    }
}

fn image_state(
    server: &AssetServer,
    images: Option<&Assets<Image>>,
    path: String,
    retry: bool,
) -> GuiLoaderState {
    // Headless/input-only plugin hosts need not install ImagePlugin.
    let Some(images) = images else {
        return GuiLoaderState::Loading;
    };
    // AssetServer::load retries Failed requests. Observe the retained handle
    // first, otherwise polling would continually reset failure to Loading.
    let handle: Handle<Image> = server
        .get_handle(path.clone())
        .unwrap_or_else(|| server.load(path.clone()));
    if images.contains(handle.id()) {
        return GuiLoaderState::Ready;
    }
    if matches!(
        server.get_load_state(handle.id()),
        Some(LoadState::Failed(_))
    ) {
        if retry {
            server.reload(path);
            return GuiLoaderState::Loading;
        }
        return GuiLoaderState::Failed(GuiLoaderFailure::ImageLoad);
    }
    GuiLoaderState::Loading
}

fn load_state(
    gui: &GuiRuntime,
    assets: &Assets<FuiAsset>,
    server: &AssetServer,
    images: Option<&Assets<Image>>,
    loader: GObjectId,
    url: &str,
) -> GuiLoaderState {
    use GuiLoaderFailure as Failure;
    use GuiLoaderState as State;
    if url.is_empty() {
        return State::Empty;
    }
    let retry = gui.loader_retries.contains(&loader);
    if !url.starts_with("ui://") {
        return gui
            .asset_handle_for(loader)
            .and_then(|handle| assets.get(&handle))
            .map_or(State::Failed(Failure::MissingResource), |asset| {
                image_state(server, images, asset.resolve_asset_path(url), retry)
            });
    }
    let Some((asset, index)) = assets.iter().find_map(|(_, asset)| {
        asset
            .package
            .resolve_local_url(url)
            .map(|index| (asset, index))
    }) else {
        return State::Failed(Failure::MissingResource);
    };
    let index = asset
        .package
        .resolve_branch(index, &gui.registry.load_context(&asset.package));
    let item = asset.package.item(index).unwrap();
    match item.item_type {
        PackageItemType::Component => {
            if gui.loader_component(loader).is_some() {
                State::Ready
            } else {
                State::Failed(Failure::ComponentConstruction)
            }
        }
        PackageItemType::Image => resolve_loader_url(asset, url)
            .map_or(State::Failed(Failure::MissingResource), |source| {
                image_state(server, images, source.image, retry)
            }),
        PackageItemType::MovieClip => {
            let Ok(clip) = asset
                .package
                .load_movie_clip(index, &gui.registry.load_context(&asset.package))
            else {
                return State::Failed(Failure::MissingResource);
            };
            let mut loading = false;
            let mut checked = std::collections::HashSet::new();
            for frame in &clip.frames {
                let Some(texture) = &frame.texture else {
                    continue;
                };
                if !checked.insert(texture.atlas_item) {
                    continue;
                }
                let Some(file) = asset
                    .package
                    .item(texture.atlas_item)
                    .and_then(|item| item.file.as_deref())
                else {
                    return State::Failed(Failure::MissingResource);
                };
                match image_state(server, images, asset.resolve_asset_path(file), retry) {
                    State::Failed(error) => return State::Failed(error),
                    State::Loading => loading = true,
                    _ => {}
                }
            }
            if loading {
                State::Loading
            } else {
                State::Ready
            }
        }
        _ => State::Failed(Failure::UnsupportedResource),
    }
}

pub(crate) fn sync_loader_status(
    mut runtime: NonSendMut<GuiRuntime>,
    mut assets: ResMut<Assets<FuiAsset>>,
    images: Option<Res<Assets<Image>>>,
    server: Res<AssetServer>,
) {
    let gui = &mut *runtime;
    let dead: Vec<_> = gui
        .loader_status
        .keys()
        .copied()
        .filter(|id| !gui.store.contains(*id))
        .collect();
    for id in dead {
        if let Some(status) = gui.loader_status.remove(&id)
            && let Some(sign) = status.sign
        {
            gui.release_loader_error(sign);
        }
    }
    let loaders: Vec<_> = gui
        .store
        .iter()
        .filter_map(|(id, object)| {
            if let ControlData::Loader(data) = object.control() {
                Some((
                    id,
                    data.url.clone().unwrap_or_default(),
                    data.show_error_sign,
                    object.size(),
                ))
            } else {
                None
            }
        })
        .collect();
    for (loader, url, show_error, size) in loaders {
        let state = load_state(gui, &assets, &server, images.as_deref(), loader, &url);
        let previous = gui.loader_status.remove(&loader);
        let changed = previous
            .as_ref()
            .is_none_or(|previous| previous.url != url || previous.state != state)
            || gui.loader_retries.contains(&loader);
        let mut sign = previous.and_then(|previous| previous.sign);
        if changed && let Some(old) = sign.take() {
            gui.release_loader_error(old);
        }
        let error_template = if changed
            && matches!(state, GuiLoaderState::Failed(_))
            && show_error
            && let Some(reference) = gui.loader_error_template.clone()
        {
            assets.iter().find_map(|(id, asset)| {
                asset
                    .package
                    .resolve_local_url(&reference)
                    .map(|index| (id, asset.package.clone(), index))
            })
        } else {
            None
        };
        if let Some((asset_id, package, index)) = error_template
            && let Some(handle) = assets.get_strong_handle(asset_id)
        {
            gui.register_package(handle.clone(), &package);
            let template = templates::ResolvedTemplate {
                url: format!("ui://{}{}", package.id(), package.item(index).unwrap().id),
                package: std::sync::Arc::new(package),
                index,
                handle,
            };
            if !gui.loader_would_recurse(loader, &template.url)
                && let Some(object) = gui.acquire_loader_error(&template)
            {
                gui.store.set_size(object, size.width, size.height);
                gui.store.set_parent(object, Some(loader));
                gui.queue_subtree(object);
                sign = Some((object, template.url));
            }
        }
        gui.loader_status
            .insert(loader, LoaderStatus { url, state, sign });
    }
    gui.loader_retries.clear();
}

#[cfg(test)]
#[path = "loader_status_tests.rs"]
mod tests;

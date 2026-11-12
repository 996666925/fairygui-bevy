use super::*;
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct ResolvedTemplate {
    pub package: Arc<FuiPackage>,
    pub index: usize,
    pub url: String,
    pub handle: Handle<FuiAsset>,
}

impl GuiRuntime {
    /// Resolve a local resource name or either native ui:// URL form. Pool
    /// identities always use package/item IDs, independent of the input alias.
    pub(super) fn resolve_template(
        &self,
        default: &FuiPackage,
        reference: &str,
    ) -> Option<ResolvedTemplate> {
        let (package, index) = if reference.starts_with("ui://") {
            if let Some(index) = default.resolve_local_url(reference) {
                (Arc::new(default.clone()), index)
            } else {
                let (package, index) = self.registry.resolve_url(reference)?;
                (package.clone(), index)
            }
        } else if !reference.is_empty() && !reference.contains("://") {
            (
                Arc::new(default.clone()),
                default.item_index_by_name(reference)?,
            )
        } else {
            return None;
        };
        let handle = self.package_handles.get(package.id())?.clone();
        let url = format!("ui://{}{}", package.id(), package.item(index)?.id);
        Some(ResolvedTemplate {
            package,
            index,
            url,
            handle,
        })
    }

    pub(super) fn create_template(&mut self, template: &ResolvedTemplate) -> Option<GObjectId> {
        let object = fairygui_core::widget::create_from_package_with_registry(
            &mut self.store,
            &template.package,
            template.index,
            &self.registry.load_context(&template.package),
            &self.registry,
        )
        .ok()?;
        self.object_assets.insert(object, template.handle.clone());
        self.remember_asset_tree(object, &template.handle);
        Some(object)
    }

    pub(super) fn acquire_template(&mut self, template: &ResolvedTemplate) -> Option<GObjectId> {
        if let Some(object) = self.pool.acquire_checked(&self.store, &template.url) {
            return Some(object);
        }
        self.create_template(template)
    }

    pub(super) fn resource_url_for(&self, object: GObjectId) -> Option<String> {
        let object = self.store.object(object)?;
        Some(format!(
            "ui://{}{}",
            object.source_package_id()?,
            object.package_item_id()?
        ))
    }
}

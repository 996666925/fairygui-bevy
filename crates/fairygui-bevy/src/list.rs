//! ECS list content, row membership and synchronous virtual-row rendering.
use crate::{GObjectRef, GuiRuntime};

#[cfg(test)]
#[path = "ecs_list_tests.rs"]
mod tests;
use bevy::prelude::*;
use fairygui_core::object::{GObjectId, ObjectStore, VirtualListError, VirtualListInfo};
use std::{collections::HashMap, sync::Arc};

/// Temporary access to a row during native virtual layout. Sizes are committed
/// before the visible range is finalized. No runtime borrow or object IDs are
/// exposed to application renderers.
pub struct GuiListItem<'a> {
    store: &'a mut ObjectStore,
    object: GObjectId,
}

impl std::fmt::Debug for GuiListItem<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuiListItem").finish_non_exhaustive()
    }
}

impl GuiListItem<'_> {
    /// Reset a retained nested scroll pane when its virtual row is rebound.
    pub fn set_scroll_position(&mut self, position: Vec2) {
        if position.is_finite() {
            self.store
                .set_scroll_pos(self.object, position.x, position.y, false);
        }
    }
    pub fn set_text(&mut self, text: &str) {
        self.store.set_object_text(self.object, text);
    }
    pub fn set_icon(&mut self, icon: &str) {
        self.store.apply_icon_to(self.object, icon);
    }
    pub fn set_controller_page(&mut self, name: &str, index: i32) {
        self.store
            .set_controller_page_index(self.object, name, index);
    }
    pub fn size(&self) -> Vec2 {
        self.store.object(self.object).map_or(Vec2::ZERO, |object| {
            Vec2::new(object.width(), object.height())
        })
    }
    pub fn set_size(&mut self, size: Vec2) {
        if size.is_finite() && size.min_element() >= 0. {
            self.store.set_size(self.object, size.x, size.y);
        }
    }
    pub fn set_pivot(&mut self, pivot: Vec2, as_anchor: bool) {
        if pivot.is_finite() {
            self.store
                .set_pivot(self.object, pivot.x, pivot.y, as_anchor);
        }
    }
    pub fn child_by_path(&mut self, path: &str) -> Option<GuiListItem<'_>> {
        let mut object = self.object;
        if !path.is_empty() {
            for name in path.split('/') {
                object = self.store.child_by_name(object, name)?;
            }
        }
        Some(GuiListItem {
            store: self.store,
            object,
        })
    }
}

type ItemRenderer = dyn Fn(usize, &mut GuiListItem<'_>) + Send + Sync;

/// Configure every row of an ordinary list using its default item template.
/// Existing leading rows keep their identity; shrinking disposes trailing rows.
/// Removing the component stops configuration and keeps the current content.
#[derive(Component, Clone)]
pub struct GuiListContent {
    pub item_count: usize,
    renderer: Arc<ItemRenderer>,
}
impl std::fmt::Debug for GuiListContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuiListContent")
            .field("item_count", &self.item_count)
            .finish_non_exhaustive()
    }
}
impl GuiListContent {
    pub fn new(
        item_count: usize,
        renderer: impl Fn(usize, &mut GuiListItem<'_>) + Send + Sync + 'static,
    ) -> Self {
        Self {
            item_count,
            renderer: Arc::new(renderer),
        }
    }
    pub fn set_renderer(
        &mut self,
        renderer: impl Fn(usize, &mut GuiListItem<'_>) + Send + Sync + 'static,
    ) {
        self.renderer = Arc::new(renderer);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GuiListContentError {
    #[error("ordinary list configuration requires a List control")]
    NotList,
    #[error("ordinary list content cannot configure a virtual list")]
    VirtualList,
    #[error("list default item cannot be resolved")]
    MissingDefaultItem,
    #[error("cannot construct list default item {0}")]
    CreationFailed(String),
}
/// Last ordinary list configuration result. The actual count and row entities
/// remain available in GuiListState even if configuration fails.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct GuiListContentState {
    error: Option<GuiListContentError>,
}
impl GuiListContentState {
    pub fn error(&self) -> Option<&GuiListContentError> {
        self.error.as_ref()
    }
}
#[derive(Component, Debug)]
pub(crate) struct ContentBinding(GObjectId);

#[allow(clippy::type_complexity)]
pub(crate) fn sync_list_content(
    mut commands: Commands,
    mut runtime: NonSendMut<GuiRuntime>,
    configs: Query<(
        Entity,
        &GObjectRef,
        Ref<GuiListContent>,
        Option<&ContentBinding>,
        Option<&GuiVirtualList>,
        Option<&GuiListContentState>,
    )>,
    removed: Query<Entity, (With<ContentBinding>, Without<GuiListContent>)>,
) {
    for entity in &removed {
        commands
            .entity(entity)
            .remove::<(ContentBinding, GuiListContentState)>();
    }
    for (entity, reference, config, binding, virtual_config, current) in &configs {
        if runtime.entity_of(reference.0) != Some(entity) {
            continue;
        }
        let conflict = virtual_config.is_some() || runtime.store().is_virtual_list(reference.0);
        if !config.is_changed()
            && binding.is_some_and(|binding| binding.0 == reference.0)
            && current.is_some_and(|current| {
                matches!(current.error, Some(GuiListContentError::VirtualList)) == conflict
            })
        {
            continue;
        }
        let result = if conflict {
            Err(GuiListContentError::VirtualList)
        } else {
            runtime.configure_list_content(
                reference.0,
                config.item_count,
                |store, index, object| {
                    (config.renderer)(index, &mut GuiListItem { store, object });
                },
            )
        };
        let next = GuiListContentState {
            error: result.err(),
        };
        commands.entity(entity).insert(ContentBinding(reference.0));
        if current != Some(&next) {
            commands.entity(entity).insert(next);
        }
    }
}

/// Materialized content rows of a native list or tree, in logical child order.
/// Pane bars and pull headers/footers are excluded. Pooled rows are not active
/// items, even though their view entities may still exist.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct GuiListState {
    item_count: usize,
    items: Vec<Entity>,
    first_item_in_view: Option<usize>,
}

impl GuiListState {
    /// Logical data count; a virtual list may materialize only a few rows.
    pub fn item_count(&self) -> usize {
        self.item_count
    }
    pub fn items(&self) -> &[Entity] {
        &self.items
    }
    /// Native logical index, accounting for looping and the current display offset.
    pub fn first_item_in_view(&self) -> Option<usize> {
        self.first_item_in_view
    }
}

/// Current membership of a materialized list row. Removed while the row is
/// pooled; updated when the same view is rebound to another data index.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuiListRow {
    list: Entity,
    index: usize,
    record_index: Option<usize>,
}

impl GuiListRow {
    pub fn list(&self) -> Entity {
        self.list
    }
    pub fn index(&self) -> usize {
        self.index
    }
    /// Internal record in a virtual list, including the six copies in loop mode.
    /// Ordinary rows have no virtual record.
    pub fn record_index(&self) -> Option<usize> {
        self.record_index
    }
}

/// Attach to a list view to configure virtual rows through ECS. Only visible rows
/// are rendered. Change `item_count` or replace the component/renderer to refresh.
/// Removing it empties the list and drops the callback, retaining the native pool
/// until its list owner is disposed. Virtual/loop mode is fixed at initialization.
#[derive(Component, Clone)]
pub struct GuiVirtualList {
    pub item_count: usize,
    looping: bool,
    renderer: Arc<ItemRenderer>,
}

impl std::fmt::Debug for GuiVirtualList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuiVirtualList")
            .field("item_count", &self.item_count)
            .field("looping", &self.looping)
            .finish_non_exhaustive()
    }
}

impl GuiVirtualList {
    pub fn new(
        item_count: usize,
        renderer: impl Fn(usize, &mut GuiListItem<'_>) + Send + Sync + 'static,
    ) -> Self {
        Self {
            item_count,
            looping: false,
            renderer: Arc::new(renderer),
        }
    }
    /// Configure native sixfold loop records. Apply before the list initializes.
    pub fn looping(mut self) -> Self {
        self.looping = true;
        self
    }
    pub fn is_looping(&self) -> bool {
        self.looping
    }
    pub fn set_renderer(
        &mut self,
        renderer: impl Fn(usize, &mut GuiListItem<'_>) + Send + Sync + 'static,
    ) {
        self.renderer = Arc::new(renderer);
    }
}

/// Current virtual layout diagnostics or the last configuration error.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct GuiVirtualListState {
    info: Option<VirtualListInfo>,
    error: Option<VirtualListError>,
}
impl GuiVirtualListState {
    pub fn info(&self) -> Option<&VirtualListInfo> {
        self.info.as_ref()
    }
    pub fn error(&self) -> Option<&VirtualListError> {
        self.error.as_ref()
    }
}

#[derive(Component, Debug, Default)]
pub(crate) struct Binding {
    object: Option<GObjectId>,
    error: Option<VirtualListError>,
}

#[allow(clippy::type_complexity)]
pub(crate) fn sync_virtual_list_bindings(
    mut commands: Commands,
    mut runtime: NonSendMut<GuiRuntime>,
    mut removed: RemovedComponents<GuiVirtualList>,
    mut applied: Local<HashMap<Entity, GObjectId>>,
    mut lists: Query<(
        Entity,
        &GObjectRef,
        Ref<GuiVirtualList>,
        Option<&mut Binding>,
    )>,
    existing: Query<()>,
) {
    for entity in removed.read() {
        if let Some(object) = applied.remove(&entity) {
            runtime.clear_ecs_virtual_list(object);
        }
        if existing.contains(entity) && !lists.contains(entity) {
            commands
                .entity(entity)
                .remove::<(Binding, GuiVirtualListState)>();
        }
    }
    for (entity, reference, config, binding) in &mut lists {
        if runtime.entity_of(reference.0) != Some(entity) {
            continue;
        }
        if !config.is_changed()
            && binding
                .as_ref()
                .is_some_and(|binding| binding.object == Some(reference.0))
        {
            continue;
        }
        let renderer = config.renderer.clone();
        let result = runtime.configure_ecs_virtual_list(
            reference.0,
            config.item_count,
            config.looping,
            move |store, index, object| {
                renderer(index, &mut GuiListItem { store, object });
            },
        );
        applied.insert(entity, reference.0);
        let next = Binding {
            object: Some(reference.0),
            error: result.err(),
        };
        if let Some(mut binding) = binding {
            *binding = next;
        } else {
            commands.entity(entity).insert(next);
        }
    }
    applied.retain(|&entity, _| existing.contains(entity));
}

pub(crate) fn sync_virtual_list_state(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    lists: Query<
        (Entity, &GObjectRef, &Binding, Option<&GuiVirtualListState>),
        With<GuiVirtualList>,
    >,
) {
    for (entity, reference, binding, current) in &lists {
        let next = GuiVirtualListState {
            info: runtime.store().virtual_list_info(reference.0),
            error: binding.error.clone(),
        };
        if current != Some(&next) {
            commands.entity(entity).insert(next);
        }
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn sync_list_state(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    objects: Query<(
        Entity,
        &GObjectRef,
        Option<&GuiListState>,
        Option<&GuiListRow>,
    )>,
) {
    let store = runtime.store();
    let mut rows = HashMap::new();
    for (entity, reference, current, _) in &objects {
        if runtime.entity_of(reference.0) != Some(entity) {
            continue;
        }
        let Some(items) = store.list_items(reference.0) else {
            if current.is_some() {
                commands.entity(entity).remove::<GuiListState>();
            }
            continue;
        };
        let items = items
            .into_iter()
            .enumerate()
            .filter_map(|(child_index, object)| {
                let row = runtime.entity_of(object)?;
                let record_index = store.virtual_item_record_index(reference.0, object);
                let index = if store.is_virtual_list(reference.0) {
                    store.virtual_item_index(reference.0, object)?
                } else {
                    child_index
                };
                rows.insert(
                    row,
                    GuiListRow {
                        list: entity,
                        index,
                        record_index,
                    },
                );
                Some(row)
            })
            .collect();
        let next = GuiListState {
            item_count: store.list_num_items(reference.0),
            items,
            first_item_in_view: store.list_first_item_in_view(reference.0),
        };
        if current != Some(&next) {
            commands.entity(entity).insert(next);
        }
    }
    for (entity, _, _, current) in &objects {
        match rows.get(&entity) {
            Some(next) if current != Some(next) => {
                commands.entity(entity).insert(*next);
            }
            None if current.is_some() => {
                commands.entity(entity).remove::<GuiListRow>();
            }
            _ => {}
        }
    }
}

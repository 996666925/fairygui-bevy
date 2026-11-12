//! Native virtual rows: retained measurements, ordered reuse and per-list pools.
use super::{GObjectId, ObjectStore, dirty};
use crate::{
    geom::{Point, Size},
    widget::{ControlData, ListLayoutType, VirtualGeometry, VirtualLayout},
};
use std::collections::VecDeque;

#[cfg(test)]
#[path = "loop_list_tests.rs"]
mod loop_tests;
#[cfg(test)]
#[path = "virtual_list_tests.rs"]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VirtualListError {
    #[error("virtual list requires a List with a ScrollPane")]
    MissingPane,
    #[error("loop virtual lists do not support flow layouts")]
    UnsupportedLoopLayout,
    #[error("virtual list requires a default item resource")]
    MissingDefaultItem,
    #[error("virtual item dimensions or layout counts are invalid")]
    InvalidGeometry,
    #[error("cannot create virtual item resource {0}")]
    MissingResource(String),
    #[error("virtual item count exceeds available capacity")]
    CountTooLarge,
    #[error("item renderer did not converge after native refill limit")]
    UnstableRenderer,
    #[error("virtual list was removed during refresh")]
    Removed,
    #[error("virtual list cannot be refreshed recursively from its renderer")]
    ReentrantRefresh,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VirtualListInfo {
    pub item_count: usize,
    pub real_item_count: usize,
    pub looping: bool,
    pub first_index: usize,
    pub active_count: usize,
    pub pooled_count: usize,
    pub measured_count: usize,
    pub alignment: Point,
}

#[derive(Debug, Clone, Default)]
pub(super) struct VirtualEntry {
    pub object: Option<GObjectId>,
    pub selected: bool,
    pub url: String,
    pub version: u64,
}

#[derive(Debug, Clone)]
pub(super) struct VirtualState {
    pub last_scroll_revision: Option<u64>,
    pub num_items: usize,
    pub looping: bool,
    pub geometry: VirtualGeometry,
    pub entries: Vec<VirtualEntry>,
    pub default_url: String,
    pub pool: VecDeque<(String, GObjectId)>,
    pub version: u64,
    pub force: bool,
    pub layout_dirty: bool,
    pub alignment: Point,
    pub measured: Vec<bool>,
}

pub type VirtualItemFactory<'a> = dyn FnMut(&mut ObjectStore, &str) -> Option<GObjectId> + 'a;
pub type VirtualItemRenderer<'a> = dyn FnMut(&mut ObjectStore, usize, GObjectId) + 'a;
pub type VirtualItemProvider<'a> = dyn FnMut(usize) -> Option<String> + 'a;

impl ObjectStore {
    pub fn is_virtual_list(&self, list: GObjectId) -> bool {
        self.virtual_states.contains_key(&list)
    }
    pub fn virtual_list_info(&self, list: GObjectId) -> Option<VirtualListInfo> {
        let s = self.virtual_states.get(&list)?;
        Some(VirtualListInfo {
            item_count: s.num_items,
            real_item_count: s.geometry.count,
            looping: s.looping,
            first_index: s.geometry.first,
            active_count: s.entries.iter().filter(|e| e.object.is_some()).count(),
            pooled_count: s.pool.len(),
            measured_count: s
                .measured
                .iter()
                .take(s.geometry.count)
                .filter(|&&v| v)
                .count(),
            alignment: s.alignment,
        })
    }
    pub fn virtual_item_object(&self, list: GObjectId, index: usize) -> Option<GObjectId> {
        let s = self.virtual_states.get(&list)?;
        if s.looping && s.geometry.config.layout != ListLayoutType::Pagination {
            if index >= s.num_items {
                return None;
            }
            let first = s.geometry.first % s.num_items;
            let child = if index >= first {
                index - first
            } else {
                s.num_items - first + index
            };
            return self.list_item_at(list, child);
        }
        self.virtual_states
            .get(&list)?
            .entries
            .get(index)?
            .object
            .filter(|&id| self.contains(id))
    }
    pub fn virtual_item_index(&self, list: GObjectId, object: GObjectId) -> Option<usize> {
        let raw = self.virtual_item_record_index(list, object)?;
        let s = &self.virtual_states[&list];
        Some(
            if s.looping
                && s.num_items > 0
                && s.geometry.config.layout != ListLayoutType::Pagination
            {
                raw % s.num_items
            } else {
                raw
            },
        )
    }
    /// Internal sixfold record index, useful for diagnostics and native traces.
    pub fn virtual_item_record_index(&self, list: GObjectId, object: GObjectId) -> Option<usize> {
        let s = self.virtual_states.get(&list)?;
        s.entries
            .iter()
            .take(s.geometry.count)
            .position(|e| e.object == Some(object))
    }
    pub fn virtual_list_alignment(&self, list: GObjectId) -> Point {
        self.virtual_states
            .get(&list)
            .map_or(Point::ZERO, |s| s.alignment)
    }
    pub fn list_num_items(&self, list: GObjectId) -> usize {
        self.virtual_states
            .get(&list)
            .map_or_else(|| self.list_item_count(list), |s| s.num_items)
    }
    pub fn list_first_item_in_view(&self, list: GObjectId) -> Option<usize> {
        let rows = self.list_items(list)?;
        let child = rows.iter().position(|&id| {
            self.object(id).is_some_and(|o| {
                self.scroll_pane(list).is_none_or(|p| {
                    p.is_child_in_view(crate::geom::Rect::from_size(o.position(), o.size()))
                })
            })
        })?;
        self.list_child_index_to_item_index(list, child)
    }
    pub fn list_child_index_to_item_index(&self, list: GObjectId, index: usize) -> Option<usize> {
        let child = self.list_item_at(list, index)?;
        if self.is_virtual_list(list) {
            self.virtual_item_index(list, child)
        } else {
            Some(index)
        }
    }
    pub fn list_item_index_to_child_index(&self, list: GObjectId, index: usize) -> Option<usize> {
        if self.is_virtual_list(list) {
            self.list_item_index(list, self.virtual_item_object(list, index)?)
        } else {
            self.list_item_at(list, index).map(|_| index)
        }
    }
    fn virtual_config(&self, list: GObjectId) -> Option<VirtualLayout> {
        let ControlData::List(d) = self.object(list)?.control() else {
            return None;
        };
        Some(VirtualLayout::from_list(
            d,
            d.component.scroll_pane.as_ref()?.view_size(),
        ))
    }
    fn virtual_resource_url(&self, object: GObjectId, fallback: &str) -> String {
        self.object(object)
            .and_then(|o| o.source_package_id().zip(o.package_item_id()))
            .map_or_else(
                || fallback.to_owned(),
                |(package, item)| format!("ui://{package}{item}"),
            )
    }
    pub fn enable_virtual_list(
        &mut self,
        list: GObjectId,
        default_url: &str,
        create: &mut VirtualItemFactory<'_>,
    ) -> Result<(), VirtualListError> {
        self.enable_virtual_list_mode(list, default_url, false, create)
    }

    pub fn enable_loop_virtual_list(
        &mut self,
        list: GObjectId,
        default_url: &str,
        create: &mut VirtualItemFactory<'_>,
    ) -> Result<(), VirtualListError> {
        self.enable_virtual_list_mode(list, default_url, true, create)
    }

    fn enable_virtual_list_mode(
        &mut self,
        list: GObjectId,
        default_url: &str,
        looping: bool,
        create: &mut VirtualItemFactory<'_>,
    ) -> Result<(), VirtualListError> {
        if self.is_virtual_list(list) {
            return Ok(());
        }
        let config = self
            .virtual_config(list)
            .ok_or(VirtualListError::MissingPane)?;
        if looping
            && matches!(
                config.layout,
                ListLayoutType::FlowHorizontal | ListLayoutType::FlowVertical
            )
        {
            return Err(VirtualListError::UnsupportedLoopLayout);
        }
        if default_url.is_empty() {
            return Err(VirtualListError::MissingDefaultItem);
        }
        // Validate the sample before detaching authored children. A failed
        // factory or malformed item must leave the original list intact.
        let rows = self.list_items(list).unwrap_or_default();
        let existing = rows
            .iter()
            .copied()
            .find(|&row| self.virtual_resource_url(row, default_url) == default_url);
        let sample = existing
            .or_else(|| create(self, default_url))
            .ok_or_else(|| VirtualListError::MissingResource(default_url.into()))?;
        if !self.contains(list) {
            if existing.is_none() {
                self.remove(sample);
            }
            return Err(VirtualListError::Removed);
        }
        let item = self.object(sample).ok_or(VirtualListError::Removed)?.size();
        if !config.valid(item) {
            if existing.is_none() {
                self.remove(sample);
            }
            return Err(VirtualListError::InvalidGeometry);
        }
        self.set_visible(sample, true);
        let mut pool = VecDeque::new();
        for row in rows {
            let url = self.virtual_resource_url(row, default_url);
            self.detach_from_parent(row);
            if row != sample {
                pool.push_back((url, row));
            }
        }
        self.detach_from_parent(sample);
        pool.push_back((default_url.into(), sample));
        let (line_items, page_rows) = config.counts(item);
        let geometry = VirtualGeometry {
            config,
            item_size: item,
            sizes: Vec::new(),
            count: 0,
            line_items,
            page_rows,
            first: 0,
            first_position: None,
        };
        self.virtual_states.insert(
            list,
            VirtualState {
                last_scroll_revision: None,
                num_items: 0,
                looping,
                geometry,
                entries: Vec::new(),
                default_url: default_url.into(),
                pool,
                version: 0,
                force: true,
                layout_dirty: true,
                alignment: Point::ZERO,
                measured: Vec::new(),
            },
        );
        if let Some(p) = self.objects[list]
            .control
            .component_mut()
            .and_then(|c| c.scroll_pane.as_mut())
        {
            if looping {
                p.bounceback_effect = false;
                p.set_loop_mode(
                    if config.vertical() {
                        crate::scroll::ScrollLoop::Vertical
                    } else {
                        crate::scroll::ScrollLoop::Horizontal
                    },
                    Point::new(config.column_gap as f32, config.line_gap as f32),
                );
            }
            p.set_scroll_step(if config.vertical() {
                item.height
            } else {
                item.width
            });
        }
        self.refresh_list_selection_snapshot(list);
        Ok(())
    }
    pub fn set_virtual_item_count(
        &mut self,
        list: GObjectId,
        count: usize,
    ) -> Result<(), VirtualListError> {
        let s = self
            .virtual_states
            .get_mut(&list)
            .ok_or(VirtualListError::MissingPane)?;
        let real_count = count
            .checked_mul(if s.looping { 6 } else { 1 })
            .ok_or(VirtualListError::CountTooLarge)?;
        let extra = real_count.saturating_sub(s.entries.len());
        s.entries
            .try_reserve(extra)
            .map_err(|_| VirtualListError::CountTooLarge)?;
        s.geometry
            .sizes
            .try_reserve(extra)
            .map_err(|_| VirtualListError::CountTooLarge)?;
        s.measured
            .try_reserve(extra)
            .map_err(|_| VirtualListError::CountTooLarge)?;
        if extra > 0 {
            s.entries.resize_with(real_count, VirtualEntry::default);
            s.geometry.sizes.resize(real_count, s.geometry.item_size);
            s.measured.resize(real_count, false);
        }
        for e in s.entries.iter_mut().skip(real_count) {
            e.selected = false;
        }
        s.num_items = count;
        s.geometry.count = real_count;
        s.force = true;
        Ok(())
    }
    pub fn invalidate_virtual_list(&mut self, list: GObjectId, layout: bool) -> bool {
        let Some(s) = self.virtual_states.get_mut(&list) else {
            return false;
        };
        s.force = true;
        s.layout_dirty |= layout;
        true
    }
    fn virtual_sync_anchor(&mut self, list: GObjectId) {
        let first = self
            .content_children(list)
            .and_then(|v| v.first().copied())
            .and_then(|id| self.object(id))
            .map(|o| o.position());
        if let Some(s) = self.virtual_states.get_mut(&list) {
            s.geometry.first_position = first;
        }
    }
    pub fn refresh_virtual_list(
        &mut self,
        list: GObjectId,
        create: &mut VirtualItemFactory<'_>,
        provider: &mut VirtualItemProvider<'_>,
        render: &mut VirtualItemRenderer<'_>,
    ) -> Result<usize, VirtualListError> {
        if !self.refreshing_virtual_lists.insert(list) {
            return Err(VirtualListError::ReentrantRefresh);
        }
        let result = self.refresh_virtual_list_inner(list, create, provider, render);
        self.refreshing_virtual_lists.remove(&list);
        result
    }

    fn refresh_virtual_list_inner(
        &mut self,
        list: GObjectId,
        create: &mut VirtualItemFactory<'_>,
        provider: &mut VirtualItemProvider<'_>,
        render: &mut VirtualItemRenderer<'_>,
    ) -> Result<usize, VirtualListError> {
        self.layout_scroll_bars(list);
        let mut config = self
            .virtual_config(list)
            .ok_or(VirtualListError::MissingPane)?;
        let revision = self
            .scroll_pane(list)
            .ok_or(VirtualListError::MissingPane)?
            .scroll_revision();
        let s = self
            .virtual_states
            .get_mut(&list)
            .ok_or(VirtualListError::MissingPane)?;
        let changed = config != s.geometry.config;
        if !config.valid(s.geometry.item_size) {
            return Err(VirtualListError::InvalidGeometry);
        }
        s.layout_dirty |= changed;
        s.force |= changed;
        if !s.force && !s.layout_dirty && s.last_scroll_revision == Some(revision) {
            return Ok(0);
        }
        let looping = s.looping;
        if s.layout_dirty {
            s.geometry.config = config;
            (s.geometry.line_items, s.geometry.page_rows) = config.counts(s.geometry.item_size);
            if s.geometry.line_items == 0
                || s.geometry.page_rows == 0
                || s.geometry.line_items > i32::MAX as usize
                || s.geometry.page_rows > i32::MAX as usize
            {
                return Err(VirtualListError::InvalidGeometry);
            }
        }
        let mut force = s.force;
        let content = force.then(|| s.geometry.content_size());
        s.force = false;
        s.layout_dirty = false;
        if looping {
            let pane = self.objects[list]
                .control
                .component_mut()
                .and_then(|c| c.scroll_pane.as_mut())
                .unwrap();
            pane.set_loop_mode(
                pane.loop_mode(),
                Point::new(config.column_gap as f32, config.line_gap as f32),
            );
        }
        let batch = self.list_selection_batches.insert(list);
        if let Some(content) = content {
            let alignment = config.alignment(content);
            self.virtual_states.get_mut(&list).unwrap().alignment = alignment;
            self.update_scroll_content_size(list, content);
        }
        let result = (|| {
            let mut rendered = 0;
            for pass in 0..=20 {
                self.virtual_sync_anchor(list);
                let (count, again) = if config.layout == ListLayoutType::Pagination {
                    (
                        self.virtual_scroll_pages(list, force, create, provider, render)?,
                        false,
                    )
                } else {
                    self.virtual_scroll_axis(list, force, create, provider, render)?
                };
                rendered += count;
                // Render callbacks receive &mut ObjectStore and may change the
                // logical count, invalidate their data, or resize the list.
                // Reconcile those changes before returning a stable row window.
                self.layout_scroll_bars(list);
                let next_config = self.virtual_config(list).ok_or(VirtualListError::Removed)?;
                let state = self
                    .virtual_states
                    .get(&list)
                    .ok_or(VirtualListError::Removed)?;
                let restart =
                    state.force || state.layout_dirty || next_config != state.geometry.config;
                if restart {
                    let item_size = self.virtual_states[&list].geometry.item_size;
                    if !next_config.valid(item_size) {
                        return Err(VirtualListError::InvalidGeometry);
                    }
                    let (line_items, page_rows) = next_config.counts(item_size);
                    if line_items == 0
                        || page_rows == 0
                        || line_items > i32::MAX as usize
                        || page_rows > i32::MAX as usize
                    {
                        return Err(VirtualListError::InvalidGeometry);
                    }
                    let looping = self.virtual_states[&list].looping;
                    let content = {
                        let state = self.virtual_states.get_mut(&list).unwrap();
                        state.geometry.config = next_config;
                        state.geometry.line_items = line_items;
                        state.geometry.page_rows = page_rows;
                        state.layout_dirty = false;
                        state.force = false;
                        let content = state.geometry.content_size();
                        state.alignment = next_config.alignment(content);
                        content
                    };
                    if looping {
                        let pane = self.objects[list]
                            .control
                            .component_mut()
                            .and_then(|c| c.scroll_pane.as_mut())
                            .ok_or(VirtualListError::Removed)?;
                        pane.set_loop_mode(
                            pane.loop_mode(),
                            Point::new(next_config.column_gap as f32, next_config.line_gap as f32),
                        );
                    }
                    self.update_scroll_content_size(list, content);
                    config = next_config;
                    force = true;
                } else {
                    force = false;
                }
                if !again && !restart {
                    self.virtual_update_arch(list);
                    return Ok(rendered);
                }
                if pass == 20 {
                    return Err(VirtualListError::UnstableRenderer);
                }
            }
            unreachable!()
        })();
        if batch {
            self.list_selection_batches.remove(&list);
        }
        self.virtual_sync_anchor(list);
        self.refresh_list_selection_snapshot(list);
        let revision = self.scroll_pane(list).map(|p| p.scroll_revision());
        if let Some(state) = self.virtual_states.get_mut(&list) {
            state.last_scroll_revision = revision;
        }
        if result.is_err() && self.virtual_states.contains_key(&list) {
            // A failed factory can interrupt directional reuse mid-pass. Return
            // all live rows before retrying so physical children remain a valid
            // contiguous range rather than mixing two logical windows.
            let active: Vec<_> = self.virtual_states[&list]
                .entries
                .iter()
                .enumerate()
                .filter_map(|(i, row)| row.object.map(|_| i))
                .collect();
            for index in active {
                self.virtual_release_row(list, index);
            }
            let state = self.virtual_states.get_mut(&list).unwrap();
            state.force = true;
            state.geometry.first = 0;
            state.geometry.first_position = None;
            self.refresh_list_selection_snapshot(list);
        }
        result
    }
    fn virtual_take_object(
        &mut self,
        list: GObjectId,
        url: &str,
        create: &mut VirtualItemFactory<'_>,
    ) -> Result<GObjectId, VirtualListError> {
        loop {
            let s = self
                .virtual_states
                .get_mut(&list)
                .ok_or(VirtualListError::Removed)?;
            let Some(index) = s.pool.iter().position(|(key, _)| key == url) else {
                break;
            };
            let object = s.pool.remove(index).unwrap().1;
            if self.contains(object) {
                return Ok(object);
            }
        }
        let object =
            create(self, url).ok_or_else(|| VirtualListError::MissingResource(url.into()))?;
        if !self.is_virtual_list(list) || !self.contains(object) {
            return Err(VirtualListError::Removed);
        }
        Ok(object)
    }
    fn virtual_release_row(&mut self, list: GObjectId, index: usize) {
        let Some(row) = self
            .virtual_states
            .get(&list)
            .and_then(|s| s.entries.get(index))
            .cloned()
        else {
            return;
        };
        let Some(object) = row.object else {
            return;
        };
        let selected = self.button_selected(object);
        if let Some(s) = self.virtual_states.get_mut(&list) {
            s.entries[index].object = None;
            if let Some(selected) = selected {
                s.entries[index].selected = selected;
            }
            s.pool.push_back((row.url, object));
        }
        self.detach_from_parent(object);
    }
    fn virtual_place_child(&mut self, list: GObjectId, row: GObjectId, index: usize) {
        let raw = self
            .content_children(list)
            .and_then(|v| v.get(index).copied())
            .and_then(|id| self.child_index(id))
            .unwrap_or_else(|| self.objects[list].children.len());
        self.add_child_at(list, row, raw);
    }
    fn virtual_scroll_axis(
        &mut self,
        list: GObjectId,
        force: bool,
        create: &mut VirtualItemFactory<'_>,
        provider: &mut VirtualItemProvider<'_>,
        render: &mut VirtualItemRenderer<'_>,
    ) -> Result<(usize, bool), VirtualListError> {
        let p = self.scroll_pane(list).ok_or(VirtualListError::Removed)?;
        let s = &self.virtual_states[&list];
        let c = s.geometry.config;
        let vertical = c.vertical();
        let pos = if vertical {
            p.scrolling_pos().y
        } else {
            p.scrolling_pos().x
        };
        let view = if vertical {
            c.view.height
        } else {
            c.view.width
        };
        let mut end_pos = pos + view;
        // Native horizontal end test deliberately differs from its vertical one.
        let end = if vertical {
            end_pos == p.content_size().height
        } else {
            pos == p.content_size().width
        };
        let (first, position) = s.geometry.index_on_position(pos, force);
        let old_first = s.geometry.first;
        if first == old_first && !force {
            return Ok((0, false));
        }
        let child_count = self.list_item_count(list);
        let last = old_first + child_count;
        let forward = old_first > first;
        let mut reuse = if forward {
            last as isize - 1
        } else {
            old_first as isize
        };
        let version = s.version.wrapping_add(1);
        let n = s.geometry.line_items;
        let count = s.geometry.count;
        let logical_count = s.num_items;
        let default_url = s.default_url.clone();
        let s = self.virtual_states.get_mut(&list).unwrap();
        s.version = version;
        s.geometry.first = first;
        let part = if vertical {
            (c.view.width - c.column_gap as f32 * (n - 1) as f32) / n as f32
        } else {
            (c.view.height - c.line_gap as f32 * (n - 1) as f32) / n as f32
        };
        let mut index = first;
        let (mut cross, mut main) = (0., position);
        let (mut delta, mut first_delta) = (0., 0.);
        let mut rendered = 0;
        while index < count && (end || main < end_pos) {
            let mut row = self.virtual_states[&list].entries[index].clone();
            let mut url = row.url.clone();
            if row.object.is_none() || force {
                url = provider(index % logical_count).unwrap_or_else(|| default_url.clone());
                if row.object.is_some() && row.url != url {
                    self.virtual_release_row(list, index);
                    row = self.virtual_states[&list].entries[index].clone();
                }
            }
            let mut need_render = force;
            if row.object.is_none() {
                let mut candidate = None;
                let mut j = reuse;
                while j >= old_first as isize && j < last as isize {
                    let e = &self.virtual_states[&list].entries[j as usize];
                    if e.object.is_some() && e.version != version && e.url == url {
                        candidate = Some(j as usize);
                        break;
                    }
                    j += if forward { -1 } else { 1 };
                }
                let object = if let Some(j) = candidate {
                    let object = self.virtual_states[&list].entries[j].object.unwrap();
                    let selected = self.button_selected(object);
                    let s = self.virtual_states.get_mut(&list).unwrap();
                    s.entries[j].object = None;
                    if let Some(selected) = selected {
                        s.entries[j].selected = selected;
                    }
                    if j as isize == reuse {
                        reuse += if forward { -1 } else { 1 };
                    }
                    object
                } else {
                    self.virtual_take_object(list, &url, create)?
                };
                self.virtual_states.get_mut(&list).unwrap().entries[index].object = Some(object);
                self.virtual_states.get_mut(&list).unwrap().entries[index].url = url;
                let insertion = if forward {
                    index - first
                } else {
                    self.list_item_count(list)
                };
                self.virtual_place_child(list, object, insertion);
                self.set_button_selected(object, row.selected);
                row.object = Some(object);
                need_render = true;
            }
            let object = row.object.unwrap();
            if need_render {
                let before = self.object(object).ok_or(VirtualListError::Removed)?.size();
                if c.auto_resize
                    && (matches!(
                        c.layout,
                        ListLayoutType::SingleColumn | ListLayoutType::SingleRow
                    ) || if vertical { c.columns > 0 } else { c.rows > 0 })
                {
                    self.set_size(
                        object,
                        if vertical { part } else { before.width },
                        if vertical { before.height } else { part },
                    );
                }
                render(self, index % logical_count, object);
                if !self.is_virtual_list(list) {
                    return Err(VirtualListError::Removed);
                }
                rendered += 1;
                let size = self.object(object).ok_or(VirtualListError::Removed)?.size();
                if ![size.width, size.height]
                    .into_iter()
                    .all(|v| v.is_finite() && v >= 0.)
                {
                    return Err(VirtualListError::InvalidGeometry);
                }
                let measured = Size::new(size.width.ceil(), size.height.ceil());
                let old = self.virtual_states[&list].geometry.sizes[index];
                if index % n == 0 {
                    let change = if vertical {
                        measured.height - old.height
                    } else {
                        measured.width - old.width
                    };
                    delta += change;
                    if index == first && forward {
                        first_delta = change;
                    }
                }
                let s = self
                    .virtual_states
                    .get_mut(&list)
                    .ok_or(VirtualListError::Removed)?;
                s.geometry.sizes[index] = measured;
                s.measured[index] = true;
            }
            let s = self
                .virtual_states
                .get_mut(&list)
                .ok_or(VirtualListError::Removed)?;
            s.entries[index].version = version;
            let size = s.geometry.sizes[index];
            self.set_xy(
                object,
                if vertical { cross } else { main },
                if vertical { main } else { cross },
            );
            if index == first {
                end_pos += if vertical { size.height } else { size.width };
            }
            cross += if vertical {
                size.width + c.column_gap as f32
            } else {
                size.height + c.line_gap as f32
            };
            if index % n == n - 1 {
                cross = 0.;
                main += if vertical {
                    size.height + c.line_gap as f32
                } else {
                    size.width + c.column_gap as f32
                };
            }
            index += 1;
        }
        for i in old_first..last {
            if self.virtual_states[&list].entries[i].version != version {
                self.virtual_release_row(list, i);
            }
        }
        for i in first..index {
            if let Some(object) = self.virtual_states[&list].entries[i].object {
                self.virtual_place_child(list, object, i - first);
            }
        }
        if delta != 0. || first_delta != 0. {
            self.change_scroll_content_on_scrolling(
                list,
                if vertical {
                    Size::new(0., delta)
                } else {
                    Size::new(delta, 0.)
                },
                if vertical {
                    Point::new(0., first_delta)
                } else {
                    Point::new(first_delta, 0.)
                },
            );
        }
        let first_child = self
            .content_children(list)
            .and_then(|v| v.first().copied())
            .and_then(|id| self.object(id));
        let display = self.scroll_pane(list).unwrap().display_pos();
        let display = if vertical { display.y } else { display.x };
        let again = index > 0
            && display >= 0.
            && first_child.is_some_and(|o| {
                if vertical {
                    o.y() > display
                } else {
                    o.x() > display
                }
            });
        Ok((rendered, again))
    }
    fn virtual_scroll_pages(
        &mut self,
        list: GObjectId,
        force: bool,
        create: &mut VirtualItemFactory<'_>,
        provider: &mut VirtualItemProvider<'_>,
        render: &mut VirtualItemRenderer<'_>,
    ) -> Result<usize, VirtualListError> {
        let pos = self
            .scroll_pane(list)
            .ok_or(VirtualListError::Removed)?
            .scrolling_pos()
            .x;
        let s = &self.virtual_states[&list];
        let g = &s.geometry;
        let c = g.config;
        let n = g.line_items;
        let rows = g.page_rows;
        let (first, _) = g.index_on_position(pos, force);
        if first == g.first && !force {
            return Ok(0);
        }
        let old_first = g.first;
        let version = s.version.wrapping_add(1);
        let count = g.count;
        let logical_count = s.num_items;
        let stored = s.entries.len();
        let page_size = n * rows;
        let start = first / page_size * page_size;
        let end = start.saturating_add(page_size.saturating_mul(2)).min(count);
        let start_column = first % n;
        let default_url = s.default_url.clone();
        let s = self.virtual_states.get_mut(&list).unwrap();
        s.version = version;
        s.geometry.first = first;
        for i in start..end {
            let col = i % n;
            if (i - start < page_size && col >= start_column)
                || (i - start >= page_size && col <= start_column)
            {
                s.entries[i].version = version;
            }
        }
        let mut reuse = old_first;
        let mut insert = Some(0);
        let mut last_object = None;
        let mut rendered = 0;
        let part_w = (c.view.width - c.column_gap as f32 * (n - 1) as f32) / n as f32;
        let part_h = (c.view.height - c.line_gap as f32 * (rows - 1) as f32) / rows as f32;
        for i in start..end {
            if self.virtual_states[&list].entries[i].version != version {
                continue;
            }
            let row = self.virtual_states[&list].entries[i].clone();
            let missing = row.object.is_none();
            let object = if let Some(object) = row.object {
                insert = None;
                last_object = Some(object);
                object
            } else {
                let mut reused = None;
                while reuse < stored {
                    let e = &self.virtual_states[&list].entries[reuse];
                    if let Some(object) = e.object
                        && e.version != version
                    {
                        let selected = self.button_selected(object);
                        let url = e.url.clone();
                        let e = &mut self.virtual_states.get_mut(&list).unwrap().entries[reuse];
                        e.object = None;
                        if let Some(v) = selected {
                            e.selected = v;
                        }
                        reused = Some((object, url));
                        break;
                    }
                    reuse += 1;
                }
                let (object, url) = if let Some(pair) = reused {
                    pair
                } else {
                    let url = provider(i % logical_count).unwrap_or_else(|| default_url.clone());
                    let object = self.virtual_take_object(list, &url, create)?;
                    (object, url)
                };
                let s = self.virtual_states.get_mut(&list).unwrap();
                s.entries[i].object = Some(object);
                s.entries[i].url = url;
                let mut index = insert.unwrap_or_else(|| {
                    last_object
                        .and_then(|o| self.list_item_index(list, o))
                        .map_or(0, |i| i + 1)
                });
                if self
                    .list_item_index(list, object)
                    .is_some_and(|old| old < index)
                {
                    index -= 1;
                }
                self.virtual_place_child(list, object, index);
                insert = Some(index + 1);
                self.set_button_selected(object, row.selected);
                object
            };
            if missing || force {
                let mut size = self.object(object).ok_or(VirtualListError::Removed)?.size();
                if c.auto_resize {
                    if n == c.columns.max(0) as usize {
                        size.width = part_w;
                    }
                    if rows == c.rows.max(0) as usize {
                        size.height = part_h;
                    }
                    self.set_size(object, size.width, size.height);
                }
                render(self, i % logical_count, object);
                if !self.is_virtual_list(list) {
                    return Err(VirtualListError::Removed);
                }
                rendered += 1;
                let size = self.object(object).ok_or(VirtualListError::Removed)?.size();
                if ![size.width, size.height]
                    .into_iter()
                    .all(|v| v.is_finite() && v >= 0.)
                {
                    return Err(VirtualListError::InvalidGeometry);
                }
                let s = self
                    .virtual_states
                    .get_mut(&list)
                    .ok_or(VirtualListError::Removed)?;
                s.geometry.sizes[i] = Size::new(size.width.ceil(), size.height.ceil());
                s.measured[i] = true;
            }
        }
        let mut border = start as f32 / page_size as f32 * c.view.width;
        let (mut x, mut y, mut height) = (border, 0., 0_f32);
        for i in start..end {
            let e = &self.virtual_states[&list].entries[i];
            let size = self.virtual_states[&list].geometry.sizes[i];
            if e.version == version {
                self.set_xy(e.object.unwrap(), x, y);
            }
            height = height.max(size.height);
            if i % n == n - 1 {
                x = border;
                y += height + c.line_gap as f32;
                height = 0.;
                if i == start + page_size - 1 {
                    border += c.view.width;
                    x = border;
                    y = 0.;
                }
            } else {
                x += size.width + c.column_gap as f32;
            }
        }
        for i in reuse..stored {
            if self.virtual_states[&list].entries[i].version != version {
                self.virtual_release_row(list, i);
            }
        }
        Ok(rendered)
    }
    fn virtual_update_arch(&mut self, list: GObjectId) {
        let Some(ControlData::List(data)) = self.object(list).map(|o| o.control()) else {
            return;
        };
        if data.children_render_order != 2 || data.layout == ListLayoutType::Pagination {
            return;
        }
        let vertical = self.virtual_states[&list].geometry.config.vertical();
        let p = self.scroll_pane(list).unwrap();
        let middle = if vertical {
            p.pos().y + p.view_size().height / 2.
        } else {
            p.pos().x + p.view_size().width / 2.
        };
        let fold = data.fold_invisible_items;
        let mut min = f32::INFINITY;
        let mut apex = 0;
        for (i, id) in self.list_items(list).unwrap().into_iter().enumerate() {
            let o = &self.objects[id];
            if fold && !o.visible() {
                continue;
            }
            let distance = (middle
                - if vertical {
                    o.y() + o.height() / 2.
                } else {
                    o.x() + o.width() / 2.
                })
            .abs();
            if distance < min {
                min = distance;
                apex = i;
            }
        }
        if let ControlData::List(data) = &mut self.objects[list].control {
            data.apex_index = apex as i32;
        }
        self.touch(list, dirty::CHILDREN);
    }
}

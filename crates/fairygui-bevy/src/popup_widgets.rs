use super::templates::ResolvedTemplate;
use super::*;
use fairygui_core::geom::{Point, Rect as GuiRect};
use fairygui_core::widget::ComboPopup;

#[derive(Clone, Debug)]
pub(super) struct RetainedCombo {
    popup: ComboPopup,
    source: String,
    source_handle: Handle<FuiAsset>,
    items: Vec<String>,
    values: Vec<String>,
    icons: Vec<Option<String>>,
}

impl GuiRuntime {
    pub fn show_combo_dropdown(
        &mut self,
        commands: &mut Commands,
        combo: GObjectId,
        package: &FuiPackage,
    ) -> Option<GObjectId> {
        let object = self.store.object(combo)?;
        let anchor = GuiRect::new(object.x(), object.y(), object.width(), object.height());
        self.show_combo_dropdown_at(commands, combo, package, anchor)
    }

    pub(crate) fn show_combo_dropdown_at(
        &mut self,
        commands: &mut Commands,
        combo: GObjectId,
        package: &FuiPackage,
        anchor: GuiRect,
    ) -> Option<GObjectId> {
        self.root_container()?;
        let data = match self.store.object(combo)?.control() {
            ControlData::ComboBox(data) => data.clone(),
            _ => return None,
        };
        let source = data.dropdown.as_ref()?;
        let mut template = self.resolve_template(package, source)?;
        if template.package.id() == package.id() {
            template.handle = self.asset_handle_for(combo).unwrap_or(template.handle);
        }
        let existing = self.retained_combos.get(&combo).cloned();
        // Suppression is by retained dropdown identity, even after root checks
        // removed its open-combo mapping earlier in this pointer-down.
        if existing
            .as_ref()
            .is_some_and(|cached| self.just_closed_popups.contains(&cached.popup.dropdown))
        {
            return None;
        }
        let reuse = existing.as_ref().is_some_and(|cached| {
            cached.source == template.url
                && cached.source_handle == template.handle
                && self.store.contains(cached.popup.dropdown)
                && self.store.contains(cached.popup.list)
        });
        let mut cached = if reuse {
            existing.as_ref()?.clone()
        } else {
            let dropdown = self.create_template(&template)?;
            let list = self.store.child_by_name(dropdown, "list").filter(|list| {
                matches!(
                    self.store.object(*list).map(|list| list.control()),
                    Some(ControlData::List(_))
                )
            });
            let item_url = list.and_then(|list| match self.store.object(list)?.control() {
                ControlData::List(data) => data.default_item.clone(),
                _ => None,
            });
            let Some((list, item_url)) = list.zip(item_url) else {
                self.store.remove(dropdown);
                return None;
            };
            RetainedCombo {
                popup: ComboPopup {
                    dropdown,
                    list,
                    item_url: Some(item_url),
                },
                source: template.url.clone(),
                source_handle: template.handle.clone(),
                items: Vec::new(),
                values: Vec::new(),
                icons: Vec::new(),
            }
        };
        let list_owner = self
            .store
            .object(cached.popup.list)
            .and_then(|object| object.source_package_id())
            .and_then(|id| self.registry.get_by_id(id))
            .cloned()
            .unwrap_or_else(|| template.package.clone());
        let item_template = self.resolve_template(&list_owner, cached.popup.item_url.as_deref()?);
        let Some(mut item_template) = item_template else {
            if !reuse {
                self.store.remove(cached.popup.dropdown);
            }
            return None;
        };
        if item_template.package.id() == list_owner.id() {
            item_template.handle = self
                .asset_handle_for(cached.popup.list)
                .unwrap_or(item_template.handle);
        }
        cached.popup.item_url = Some(item_template.url.clone());
        if cached.items != data.items
            || cached.values != data.values
            || cached.icons != data.icons
            || self.store.list_item_count(cached.popup.list) != data.items.len()
        {
            if !self.replace_popup_items(
                cached.popup.list,
                &item_template,
                &data.items,
                &data.values,
                &data.icons,
            ) {
                if !reuse {
                    self.store.remove(cached.popup.dropdown);
                }
                return None;
            }
            cached.items = data.items.clone();
            cached.values = data.values.clone();
            cached.icons = data.icons.clone();
        }
        let popup = cached.popup.clone();
        let frame = self.store.object(popup.dropdown)?.size();
        let list_size = self.store.object(popup.list)?.size();
        let margin = frame.width - list_size.width;
        self.store
            .set_size(popup.dropdown, anchor.width, frame.height);
        self.store.set_size(
            popup.list,
            (anchor.width - margin).max(0.0),
            list_size.height,
        );
        self.size_popup_list(popup.dropdown, popup.list, data.visible_item_count, 0.0);
        self.store.set_list_selection(popup.list, -1);
        let size = self.store.object(popup.dropdown)?.size();
        let container = self.store.object(self.root_container()?)?.size();
        let mut x = anchor.x;
        if x + size.width > container.width {
            x += anchor.width - size.width;
        }
        let mut y = anchor.y + anchor.height;
        if data.popup_direction == 2
            || (data.popup_direction == 0 && anchor.y + size.height > container.height)
        {
            y = anchor.y - size.height - 1.0;
            if y < 0.0 {
                y = 0.0;
                x += anchor.width * 0.5;
            }
        }
        if !reuse && let Some(old) = existing {
            self.hide_popup(Some(old.popup.dropdown));
            self.despawn_subtree(commands, old.popup.dropdown);
        }
        self.retained_combos.insert(combo, cached);
        self.show_popup(popup.dropdown, Point::new(x, y));
        self.store
            .open_combo_popup(combo, popup.dropdown, popup.list, popup.item_url);
        Some(popup.dropdown)
    }

    pub fn close_combo_dropdown(&mut self, _commands: &mut Commands, combo: GObjectId) -> bool {
        self.store
            .combo_popup(combo)
            .is_some_and(|popup| self.hide_popup(Some(popup.dropdown)))
    }

    pub fn close_all_combo_dropdowns(&mut self, commands: &mut Commands) -> usize {
        let open = self.store.open_combo_ids();
        for combo in &open {
            self.close_combo_dropdown(commands, *combo);
        }
        open.len()
    }

    pub(super) fn replace_popup_items(
        &mut self,
        list: GObjectId,
        template: &ResolvedTemplate,
        captions: &[String],
        values: &[String],
        icons: &[Option<String>],
    ) -> bool {
        // Prepare the whole replacement before touching visible content. A bad
        // dependency or malformed resource must not leave a partial dropdown.
        let mut items = Vec::new();
        for _ in captions {
            let Some(item) = self.acquire_template(template) else {
                for item in items {
                    self.pool.release(&mut self.store, &template.url, item);
                }
                return false;
            };
            items.push(item);
        }
        let children = self.store.list_items(list).unwrap_or_default();
        for child in children {
            if let Some(url) = self.resource_url_for(child) {
                self.pool.release(&mut self.store, url, child);
            } else {
                self.store.remove(child);
            }
        }
        for (position, (item, caption)) in items.into_iter().zip(captions).enumerate() {
            self.store.set_visible(item, true);
            self.store
                .set_name(item, values.get(position).map_or("", String::as_str));
            self.store.set_object_text(item, caption);
            self.store.apply_icon_to(
                item,
                icons
                    .get(position)
                    .and_then(|icon| icon.as_deref())
                    .unwrap_or(""),
            );
            self.store.set_grayed(item, false);
            self.store.set_button_selected(item, false);
            self.store.set_controller_page_index(item, "checked", 0);
            self.store.apply_title_of(item);
            self.store.set_parent(item, Some(list));
            self.queue_subtree(item);
        }
        true
    }

    pub(super) fn size_popup_list(
        &mut self,
        popup: GObjectId,
        list: GObjectId,
        limit: i32,
        minimum: f32,
    ) {
        let Some((old_frame, old_list, children)) = self
            .store
            .object(popup)
            .zip(self.store.object(list))
            .map(|(popup, object)| {
                (
                    popup.size(),
                    object.size(),
                    self.store.list_items(list).unwrap_or_default(),
                )
            })
        else {
            return;
        };
        let (fold_invisible, auto_width, margin) =
            match self.store.object(list).map(|list| list.control()) {
                Some(ControlData::List(data)) => (
                    data.fold_invisible_items,
                    data.auto_resize_item
                        && data.layout == fairygui_core::widget::ListLayoutType::SingleColumn,
                    data.item_margin,
                ),
                _ => (false, false, Default::default()),
            };
        if auto_width {
            for &child in &children {
                if let Some(height) = self.store.object(child).map(|child| child.height()) {
                    self.store.set_size(
                        child,
                        (old_list.width - margin.left - margin.right).max(0.0),
                        height,
                    );
                }
            }
        }
        self.store.layout_list(list);
        let height = children
            .iter()
            .take(limit.max(0) as usize)
            .rev()
            .filter_map(|child| self.store.object(*child))
            .find(|child| !fold_invisible || child.visible())
            .map_or(margin.top + margin.bottom, |child| {
                child.y() + child.height() + margin.bottom
            });
        let height = height.max(minimum);
        self.store.set_size(list, old_list.width, height);
        self.store.set_size(
            popup,
            old_frame.width,
            height + (old_frame.height - old_list.height),
        );
    }
}

pub(crate) fn cleanup_retained_popups(mut runtime: NonSendMut<GuiRuntime>) {
    runtime.cleanup_retained_menu();
    let dead: Vec<_> = runtime
        .retained_combos
        .keys()
        .copied()
        .filter(|combo| !runtime.store.contains(*combo))
        .collect();
    for combo in dead {
        if let Some(cached) = runtime.retained_combos.remove(&combo) {
            runtime.hide_popup(Some(cached.popup.dropdown));
            runtime.store.remove(cached.popup.dropdown);
        }
    }
    let gui = &mut *runtime;
    gui.object_assets
        .retain(|object, _| gui.store.contains(*object));
}

#[cfg(test)]
#[path = "popup_widget_tests.rs"]
mod tests;

use super::templates::ResolvedTemplate;
use super::*;
use fairygui_core::{geom::Point, popup::MenuItem};

/// Local resource names or ui:// URLs in any registered package.
/// Separators require their own template.
#[derive(Debug, Clone, Copy)]
pub struct PopupMenuTemplates<'a> {
    pub content: &'a str,
    pub item: &'a str,
    pub separator: Option<&'a str>,
}

/// A delayed menu activation, published after closing, before `GuiSystems::Respond`.
/// `GuiEvent` observers still receive the original, immediate pointer click.
#[derive(Message, Debug, Clone)]
pub struct GuiMenuAction {
    pub menu: GObjectId,
    pub item: GObjectId,
    pub name: String,
    pub caption: String,
    pub action: Option<u64>,
    pub checked: bool,
    pub pointer: Option<Point>,
}

#[derive(Clone, Debug)]
pub(super) struct RetainedMenu {
    content: GObjectId,
    list: GObjectId,
    package: String,
    resource: String,
    content_handle: Handle<FuiAsset>,
    item_url: String,
    separator_url: Option<String>,
    // The initial configuration is separate from mutable state. Showing the
    // same configuration again preserves edits, checked state and row IDs.
    configured: Vec<MenuItem>,
    rows: Vec<MenuRow>,
}

#[derive(Clone, Debug)]
struct MenuRow {
    object: GObjectId,
    url: String,
    definition: MenuItem,
}

pub(super) struct PendingMenuClick {
    pub menu: GObjectId,
    item: GObjectId,
    remaining: f32,
    pointer: Option<Point>,
}

pub fn emit_menu_actions(
    mut runtime: NonSendMut<GuiRuntime>,
    mut messages: MessageWriter<GuiMenuAction>,
) {
    messages.write_batch(runtime.menu_actions.drain(..));
}

impl GuiRuntime {
    /// Caption-only compatibility API. Typed items support names and actions.
    #[allow(clippy::too_many_arguments)]
    pub fn show_popup_menu(
        &mut self,
        commands: &mut Commands,
        package: &FuiPackage,
        menu_item: &str,
        item_item: &str,
        captions: &[&str],
        at: Point,
    ) -> Option<GObjectId> {
        let items: Vec<_> = captions
            .iter()
            .enumerate()
            .map(|(index, caption)| MenuItem {
                name: index.to_string(),
                ..MenuItem::new(*caption)
            })
            .collect();
        self.show_popup_menu_items(
            commands,
            package,
            PopupMenuTemplates {
                content: menu_item,
                item: item_item,
                separator: None,
            },
            &items,
            at,
        )
    }

    /// Show retained menu data. Duplicate nonempty names, missing separator
    /// templates and non-button action templates are rejected. Unchanged input
    /// preserves dynamic edits; changed input replaces rows through the pool.
    pub fn show_popup_menu_items(
        &mut self,
        commands: &mut Commands,
        package: &FuiPackage,
        templates: PopupMenuTemplates<'_>,
        items: &[MenuItem],
        at: Point,
    ) -> Option<GObjectId> {
        self.root_container()?;
        let content_template = self.resolve_template(package, templates.content)?;
        let item_template = self.resolve_template(package, templates.item)?;
        let separator_template = match templates.separator {
            Some(reference) => Some(self.resolve_template(package, reference)?),
            None => None,
        };
        let item_url = item_template.url.clone();
        let separator_url = separator_template
            .as_ref()
            .map(|template| template.url.clone());
        if !valid_names(items) || (items.iter().any(|i| i.separator) && separator_url.is_none()) {
            return None;
        }
        let reuse = self.retained_menu.as_ref().is_some_and(|old| {
            self.store.contains(old.content)
                && self.store.contains(old.list)
                && old.package == package.id()
                && old.resource == content_template.url
                && old.content_handle == content_template.handle
                && old.item_url == item_url
                && old.separator_url == separator_url
        });
        let mut menu = if reuse {
            self.retained_menu.as_ref()?.clone()
        } else {
            let content = self.create_template(&content_template)?;
            let list = self.store.child_by_name(content, "list").filter(|list| {
                matches!(
                    self.store.object(*list).map(|o| o.control()),
                    Some(ControlData::List(_))
                )
            });
            let Some(list) = list else {
                self.store.remove(content);
                return None;
            };
            // The template may contain editor-preview rows, not application data.
            let children = self.store.list_items(list)?;
            for child in children {
                self.store.remove(child);
            }
            RetainedMenu {
                content,
                list,
                package: package.id().into(),
                resource: content_template.url,
                content_handle: content_template.handle,
                item_url,
                separator_url,
                configured: Vec::new(),
                rows: Vec::new(),
            }
        };
        if menu.configured != items || menu.rows.iter().any(|row| !self.store.contains(row.object))
        {
            // Prepare first so invalid row resources do not destroy a usable menu.
            let mut rows = Vec::new();
            for definition in items {
                let template = if definition.separator {
                    separator_template.as_ref()?
                } else {
                    &item_template
                };
                let Some(row) = self.create_menu_row(template, definition.clone()) else {
                    for row in rows {
                        self.release_menu_row(row);
                    }
                    if !reuse {
                        self.store.remove(menu.content);
                    }
                    return None;
                };
                rows.push(row);
            }
            self.pending_menu_click = None;
            for row in menu.rows.drain(..) {
                self.release_menu_row(row);
            }
            for row in &rows {
                self.store.set_parent(row.object, Some(menu.list));
                self.queue_subtree(row.object);
            }
            menu.rows = rows;
            menu.configured = items.to_vec();
        }
        if !reuse && let Some(old) = self.retained_menu.take() {
            self.hide_popup(Some(old.content));
            self.despawn_subtree(commands, old.content);
        }
        let content = menu.content;
        self.retained_menu = Some(menu);
        self.reopen_popup_menu(at)?;
        Some(content)
    }

    fn create_menu_row(
        &mut self,
        template: &ResolvedTemplate,
        mut definition: MenuItem,
    ) -> Option<MenuRow> {
        let object = self.acquire_template(template)?;
        if !definition.separator
            && !matches!(self.store.object(object)?.control(), ControlData::Button(_))
        {
            self.pool.release(&mut self.store, &template.url, object);
            return None;
        }
        definition.checkable |= definition.checked;
        self.store.set_button_selected(object, false);
        let row = MenuRow {
            object,
            url: template.url.clone(),
            definition,
        };
        self.apply_menu_row(&row);
        Some(row)
    }

    fn apply_menu_row(&mut self, row: &MenuRow) {
        let item = &row.definition;
        self.store.set_name(row.object, &item.name);
        self.store.set_visible(row.object, item.visible);
        if !item.separator {
            self.store.set_object_text(row.object, &item.caption);
            self.store
                .apply_icon_to(row.object, item.icon.as_deref().unwrap_or(""));
            self.store.set_grayed(row.object, !item.enabled);
            self.store.set_controller_page_index(
                row.object,
                "checked",
                if item.checked {
                    2
                } else if item.checkable {
                    1
                } else {
                    0
                },
            );
            self.store.apply_title_of(row.object);
        }
    }

    fn release_menu_row(&mut self, row: MenuRow) {
        if self
            .pending_menu_click
            .as_ref()
            .is_some_and(|click| click.item == row.object)
        {
            self.pending_menu_click = None;
        }
        self.pool.release(&mut self.store, &row.url, row.object);
    }

    pub fn is_popup_menu_open(&self) -> bool {
        self.popup_menu.is_open()
    }
    pub fn popup_menu_content(&self) -> Option<GObjectId> {
        self.popup_menu.content()
    }

    /// Dispose the retained menu and its views. Pooled rows removed earlier
    /// remain in the runtime's shared resource pool.
    pub fn dispose_popup_menu(&mut self, commands: &mut Commands) -> bool {
        let Some(menu) = self.retained_menu.take() else {
            return false;
        };
        self.hide_popup(Some(menu.content));
        self.popup_menu.set_closed();
        self.pending_menu_click = None;
        for row in &menu.rows {
            if !self.is_descendant_of_or_self(row.object, menu.content) {
                self.despawn_subtree(commands, row.object);
            }
        }
        self.despawn_subtree(commands, menu.content);
        true
    }

    pub(super) fn cleanup_retained_menu(&mut self) {
        if self.retained_menu.as_ref().is_some_and(|menu| {
            !self.store.contains(menu.content) || !self.store.contains(menu.list)
        }) {
            let menu = self.retained_menu.take().unwrap();
            self.hide_popup(Some(menu.content));
            self.popup_menu.set_closed();
            self.pending_menu_click = None;
            for row in menu.rows {
                self.store.remove(row.object);
            }
            self.store.remove(menu.content);
        }
    }

    /// Reopen the current menu without replacing its dynamically edited items.
    /// As in native resizeToFit, frame height is fitted on show, not on each edit.
    pub fn reopen_popup_menu(&mut self, at: Point) -> Option<GObjectId> {
        let menu = self.retained_menu.as_ref()?.clone();
        if !self.store.contains(menu.content) || !self.store.contains(menu.list) {
            return None;
        }
        self.pending_menu_click = None;
        self.size_popup_list(menu.content, menu.list, i32::MAX, 10.0);
        self.store.set_list_selection(menu.list, -1);
        for row in &menu.rows {
            self.store.set_button_selected(row.object, false);
        }
        if !self.show_popup(menu.content, at) {
            return None;
        }
        self.popup_menu
            .set_content(menu.content, Some(menu.item_url));
        Some(menu.content)
    }

    /// Read current application data and the retained row's object ID by name.
    pub fn popup_menu_item(&self, name: &str) -> Option<(&MenuItem, GObjectId)> {
        self.retained_menu
            .as_ref()?
            .rows
            .iter()
            .find(|row| row.definition.name == name && self.store.contains(row.object))
            .map(|row| (&row.definition, row.object))
    }

    /// Change caption, icon, action, visibility, enabled or checked state by name.
    /// The name and separator/template kind are immutable; changing either
    /// rejects the edit. Checked items enable the template's checked controller.
    pub fn update_popup_menu_item(&mut self, name: &str, edit: impl FnOnce(&mut MenuItem)) -> bool {
        let Some(menu) = self.retained_menu.as_ref() else {
            return false;
        };
        let Some(index) = menu.rows.iter().position(|row| row.definition.name == name) else {
            return false;
        };
        let mut row = menu.rows[index].clone();
        edit(&mut row.definition);
        row.definition.checkable |= row.definition.checked;
        if row.definition.name != name
            || row.definition.separator != menu.rows[index].definition.separator
        {
            return false;
        }
        let list = menu.list;
        self.apply_menu_row(&row);
        self.retained_menu.as_mut().unwrap().rows[index] = row;
        self.store.layout_list(list);
        true
    }

    /// Append a row using this menu's configured action/separator resource.
    pub fn add_popup_menu_item(
        &mut self,
        package: &FuiPackage,
        item: MenuItem,
    ) -> Option<GObjectId> {
        let menu = self.retained_menu.as_ref()?;
        if (!item.separator && item.name.is_empty())
            || menu.package != package.id()
            || (!item.name.is_empty() && menu.rows.iter().any(|r| r.definition.name == item.name))
        {
            return None;
        }
        let url = if item.separator {
            menu.separator_url.as_ref()?
        } else {
            &menu.item_url
        }
        .clone();
        let list = menu.list;
        let template = self.resolve_template(package, &url)?;
        let row = self.create_menu_row(&template, item)?;
        let object = row.object;
        self.store.set_parent(object, Some(list));
        self.queue_subtree(object);
        self.retained_menu.as_mut()?.rows.push(row);
        self.store.layout_list(list);
        Some(object)
    }

    pub fn remove_popup_menu_item(&mut self, name: &str) -> bool {
        let Some(menu) = self.retained_menu.as_mut() else {
            return false;
        };
        let Some(index) = menu.rows.iter().position(|row| row.definition.name == name) else {
            return false;
        };
        let row = menu.rows.remove(index);
        let list = menu.list;
        self.release_menu_row(row);
        self.store.set_list_selection(list, -1);
        self.store.layout_list(list);
        true
    }

    pub fn clear_popup_menu_items(&mut self) {
        let Some(menu) = self.retained_menu.as_mut() else {
            return;
        };
        let rows = std::mem::take(&mut menu.rows);
        let list = menu.list;
        for row in rows {
            self.release_menu_row(row);
        }
        self.store.set_list_selection(list, -1);
        self.store.layout_list(list);
    }

    /// The outer option indicates menu membership, the inner one a row index.
    pub fn popup_menu_hit(&self, object: GObjectId) -> Option<Option<usize>> {
        let menu = self.retained_menu.as_ref()?;
        if self.popup_menu.content() != Some(menu.content)
            || !self.is_descendant_of_or_self(object, menu.content)
        {
            return None;
        }
        Some(
            menu.rows
                .iter()
                .position(|row| self.is_descendant_of_or_self(object, row.object)),
        )
    }

    /// Closing detaches the complete retained subtree. Commands is kept for compatibility.
    pub fn close_popup_menu(&mut self, _commands: &mut Commands) -> bool {
        self.popup_menu
            .content()
            .is_some_and(|popup| self.hide_popup(Some(popup)))
    }

    pub(crate) fn activate_menu_item(&mut self, target: GObjectId, pointer: Option<Point>) {
        let Some(Some(index)) = self.popup_menu_hit(target) else {
            return;
        };
        let Some(menu) = self.retained_menu.as_ref() else {
            return;
        };
        let row = &menu.rows[index];
        if row.definition.separator
            || !matches!(
                self.store.object(row.object).map(|o| o.control()),
                Some(ControlData::Button(_))
            )
        {
            return;
        }
        self.store.set_list_selection(menu.list, index as i32);
        for (other_index, other) in menu.rows.iter().enumerate() {
            self.store
                .set_button_selected(other.object, other_index == index);
        }
        self.pending_menu_click = Some(PendingMenuClick {
            menu: menu.content,
            item: row.object,
            remaining: 0.1,
            pointer,
        });
    }

    pub(crate) fn advance_menu_click(&mut self, delta: f32) {
        let Some(mut click) = self.pending_menu_click.take() else {
            return;
        };
        if self.popup_menu.content() != Some(click.menu) {
            return;
        }
        click.remaining -= delta;
        if click.remaining > 0.0 {
            self.pending_menu_click = Some(click);
            return;
        }
        let Some(menu) = self
            .retained_menu
            .as_ref()
            .filter(|menu| menu.content == click.menu)
        else {
            return;
        };
        let Some(index) = menu.rows.iter().position(|row| row.object == click.item) else {
            return;
        };
        let mut row = menu.rows[index].clone();
        if !row.definition.is_actionable()
            || self.store.object(click.item).is_none_or(|item| {
                item.grayed()
                    || !item.visible()
                    || item.parent() != Some(menu.list)
                    || !matches!(item.control(), ControlData::Button(_))
            })
        {
            self.store.set_list_selection(menu.list, -1);
            self.store.set_button_selected(click.item, false);
            return;
        }
        let checked_index = self
            .store
            .object(click.item)
            .and_then(|item| item.control().component())
            .and_then(|data| data.controller("checked"))
            .map_or(0, |controller| controller.selected_index);
        row.definition.checked = checked_index == 1;
        row.definition.checkable = checked_index != 0;
        if checked_index != 0 {
            self.store.set_controller_page_index(
                click.item,
                "checked",
                if checked_index == 1 { 2 } else { 1 },
            );
        }
        self.retained_menu.as_mut().unwrap().rows[index] = row.clone();
        self.hide_popup(Some(click.menu));
        self.menu_actions.push(GuiMenuAction {
            menu: click.menu,
            item: click.item,
            name: row.definition.name,
            caption: row.definition.caption,
            action: row.definition.action,
            checked: row.definition.checked,
            pointer: click.pointer,
        });
    }
}

fn valid_names(items: &[MenuItem]) -> bool {
    let mut names = std::collections::HashSet::new();
    items.iter().all(|item| {
        (item.separator && item.name.is_empty())
            || (!item.name.is_empty() && names.insert(&item.name))
    })
}

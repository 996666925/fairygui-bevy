//! ScrollPane's reference to a controller, distinct from GList row selection.
use super::{GObjectId, ObjectStore, dirty};
use crate::scroll::{ScrollPane, ScrollType};

#[cfg(test)]
#[path = "scroll_pages_tests.rs"]
mod tests;

impl ObjectStore {
    /// Retains the controller's owner identity across reparenting, as native
    /// pageController does. Assigning a binding alone does not move either side.
    pub fn set_scroll_page_controller(
        &mut self,
        container: GObjectId,
        controller: Option<(GObjectId, usize)>,
    ) -> bool {
        if self.scroll_pane(container).is_none()
            || controller.is_some_and(|(owner, index)| {
                self.object(owner)
                    .and_then(|o| o.control().component())
                    .and_then(|c| c.controller_at(index))
                    .is_none()
            })
        {
            return false;
        }
        self.pending_scroll_page_initialization.remove(&container);
        let old = self.scroll_page_controllers.remove(&container);
        if let Some(binding) = controller {
            self.scroll_page_controllers.insert(container, binding);
        }
        old != controller
    }

    pub fn scroll_page_controller(&self, container: GObjectId) -> Option<(GObjectId, usize)> {
        let &(owner, index) = self.scroll_page_controllers.get(&container)?;
        self.object(owner)?
            .control()
            .component()?
            .controller_at(index)?;
        Some((owner, index))
    }

    pub fn scroll_current_page_x(&self, container: GObjectId) -> Option<i32> {
        self.scroll_pane(container).map(ScrollPane::current_page_x)
    }

    pub fn scroll_current_page_y(&self, container: GObjectId) -> Option<i32> {
        self.scroll_pane(container).map(ScrollPane::current_page_y)
    }

    pub fn set_scroll_current_page_x(
        &mut self,
        container: GObjectId,
        page: i32,
        animated: bool,
    ) -> bool {
        self.set_scroll_current_page(container, page, animated, true)
    }

    pub fn set_scroll_current_page_y(
        &mut self,
        container: GObjectId,
        page: i32,
        animated: bool,
    ) -> bool {
        self.set_scroll_current_page(container, page, animated, false)
    }

    fn set_scroll_current_page(
        &mut self,
        container: GObjectId,
        page: i32,
        animated: bool,
        horizontal: bool,
    ) -> bool {
        let changed = self
            .objects
            .get_mut(container)
            .and_then(|o| o.control.component_mut())
            .and_then(|c| c.scroll_pane.as_mut())
            .is_some_and(|p| {
                if horizontal {
                    p.set_current_page_x(page, animated)
                } else {
                    p.set_current_page_y(page, animated)
                }
            });
        if changed {
            self.touch(container, dirty::SCROLL);
            self.update_scroll_page_controller(container);
        }
        changed
    }

    pub(crate) fn bind_exported_scroll_page_controllers(&mut self, parent: GObjectId) {
        let children = self.objects[parent].children.clone();
        for child in children {
            let index = self.objects[child]
                .control
                .component()
                .and_then(|c| c.page_controller);
            if let Some(index) = index
                && self.set_scroll_page_controller(child, Some((parent, index)))
            {
                // List rows and final bar-adjusted viewports are supplied by
                // the adapter after package construction. Preserve home pages.
                self.pending_scroll_page_initialization.insert(child);
            }
        }
    }

    /// Complete exported bindings after rows, content ranges and viewports are
    /// ready. Each pending binding is applied once; runtime bindings are inert.
    pub fn initialize_scroll_page_controllers(&mut self, root: GObjectId) {
        let mut panes = vec![root];
        self.for_each_descendant(root, &mut |id| panes.push(id));
        for pane in panes {
            if !self.pending_scroll_page_initialization.remove(&pane) {
                continue;
            }
            let Some((owner, index)) = self.scroll_page_controller(pane) else {
                continue;
            };
            if self.object(pane).and_then(|o| o.parent()) != Some(owner) {
                continue;
            }
            let page =
                self.objects[owner].control.component().unwrap().controllers[index].selected_index;
            self.syncing_scroll_controllers.insert(pane);
            self.apply_scroll_controller_page(pane, page);
            self.syncing_scroll_controllers.remove(&pane);
        }
    }

    fn apply_scroll_controller_page(&mut self, container: GObjectId, page: i32) {
        let horizontal = self
            .scroll_pane(container)
            .is_some_and(|p| p.scroll_type == ScrollType::Horizontal);
        self.set_scroll_current_page(container, page, true, horizontal);
    }

    pub(super) fn sync_related_scroll_panes(&mut self, owner: GObjectId, name: &str) {
        let Some(component) = self.objects.get(owner).and_then(|o| o.control.component()) else {
            return;
        };
        let Some(index) = component.controller_index_of(name) else {
            return;
        };
        let page = component.controllers[index].selected_index;
        // GComponent.applyController visits direct children in child order.
        // Merely retaining a reference after reparenting does not subscribe a
        // pane under a different parent to the old owner's applyController.
        let panes: Vec<_> = self.objects[owner]
            .children
            .iter()
            .copied()
            .filter(|id| {
                !self.syncing_scroll_controllers.contains(id)
                    && !self.pending_scroll_page_initialization.contains(id)
                    && self.scroll_page_controllers.get(id) == Some(&(owner, index))
            })
            .collect();
        for pane in panes {
            self.apply_scroll_controller_page(pane, page);
        }
    }

    pub(super) fn update_scroll_page_controller(&mut self, container: GObjectId) {
        if self.syncing_scroll_controllers.contains(&container)
            || self.pending_scroll_page_initialization.contains(&container)
        {
            return;
        }
        let Some((owner, index)) = self.scroll_page_controller(container) else {
            return;
        };
        let Some(pane) = self.scroll_pane(container).filter(|p| p.page_mode) else {
            return;
        };
        let page = if pane.scroll_type == ScrollType::Horizontal {
            pane.current_page_x()
        } else {
            pane.current_page_y()
        };
        let controller = &self.objects[owner].control.component().unwrap().controllers[index];
        let name = controller.name.clone();
        if page < 0
            || page as usize >= controller.page_count()
            || self.changing_controllers.contains(&(owner, name.clone()))
        {
            return;
        }
        self.syncing_scroll_controllers.insert(container);
        self.set_controller_page_index(owner, &name, page);
        self.syncing_scroll_controllers.remove(&container);
    }
}

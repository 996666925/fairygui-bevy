use super::{GObjectId, ObjectStore, dirty};
use crate::{geom::Size, scroll::PullRelease};
#[cfg(test)]
#[path = "scroll_refresh_tests.rs"]
mod tests;

impl ObjectStore {
    pub fn scroll_header(&self, container: GObjectId) -> Option<GObjectId> {
        self.scroll_pane(container)?.header().filter(|&id| {
            self.object(id)
                .is_some_and(|o| o.parent() == Some(container))
        })
    }
    pub fn scroll_footer(&self, container: GObjectId) -> Option<GObjectId> {
        self.scroll_pane(container)?.footer().filter(|&id| {
            self.object(id)
                .is_some_and(|o| o.parent() == Some(container))
        })
    }

    pub fn is_scroll_refresh_overlay(&self, object: GObjectId) -> bool {
        self.object(object)
            .and_then(|o| o.parent())
            .and_then(|p| self.scroll_pane(p))
            .is_some_and(|p| p.header() == Some(object) || p.footer() == Some(object))
    }

    pub fn is_scroll_overlay(&self, object: GObjectId) -> bool {
        self.is_scroll_bar_overlay(object) || self.is_scroll_refresh_overlay(object)
    }

    pub(super) fn refresh_overlay_attached(&self, object: GObjectId) -> bool {
        let Some(o) = self.object(object) else {
            return false;
        };
        let Some(pane) = o.parent().and_then(|p| self.scroll_pane(p)) else {
            return true;
        };
        if pane.header() != Some(object) && pane.footer() != Some(object) {
            return true;
        }
        pane.refresh_bar_layout(pane.header() == Some(object), o.position(), o.size())
            .is_some_and(|g| g.visible)
    }

    /// Pane-owned refresh components never become list rows. Invisible parts
    /// remain owned for reuse/disposal but are disconnected from native stage.
    pub fn attach_scroll_refresh_bars(
        &mut self,
        container: GObjectId,
        create: &mut dyn FnMut(&mut Self, &str) -> Option<GObjectId>,
    ) -> usize {
        let Some(pane) = self.scroll_pane(container) else {
            return 0;
        };
        let resources = [pane.header_res.clone(), pane.footer_res.clone()];
        let mut count = 0;
        for (i, url) in resources.into_iter().enumerate() {
            let existing = if i == 0 {
                self.scroll_header(container)
            } else {
                self.scroll_footer(container)
            };
            if existing.is_some() {
                continue;
            }
            let Some(url) = url else {
                continue;
            };
            let Some(part) = create(self, &url) else {
                continue;
            };
            if self
                .object(part)
                .is_none_or(|o| o.control().component().is_none())
            {
                self.remove(part);
                continue;
            }
            let pane = self.objects[container]
                .control
                .component_mut()
                .unwrap()
                .scroll_pane
                .as_mut()
                .unwrap();
            if i == 0 {
                pane.set_refresh_part(true, Some(part));
            } else {
                pane.set_refresh_part(false, Some(part));
            }
            self.add_child_at(container, part, 0);
            count += 1;
        }
        self.layout_scroll_refresh_bars(container);
        count
    }

    pub fn layout_scroll_refresh_bars(&mut self, container: GObjectId) {
        let Some(pane) = self.scroll_pane(container) else {
            return;
        };
        let (header, footer, vertical, view) = (
            pane.header(),
            pane.footer(),
            pane.refresh_axis(),
            pane.view_size(),
        );
        let maxima = |id: Option<GObjectId>| {
            id.and_then(|id| self.object(id))
                .map(|o| Size::new(o.max_width, o.max_height))
        };
        let limits = (maxima(header), maxima(footer));
        self.objects[container]
            .control
            .component_mut()
            .unwrap()
            .scroll_pane
            .as_mut()
            .unwrap()
            .set_refresh_max_sizes(limits.0, limits.1);
        if let Some(vertical) = vertical {
            for (id, is_header) in [(header, true), (footer, false)] {
                let Some(id) = id else {
                    continue;
                };
                let Some(o) = self.object(id) else {
                    continue;
                };
                let mut size = o.size();
                // Preserve native handleSizeChanged, including its footer axis.
                if (is_header && !vertical) || (!is_header && vertical) {
                    size.height = view.height;
                } else {
                    size.width = view.width;
                }
                self.set_refresh_part_size(id, size);
            }
        }
        self.sync_scroll_refresh_bars(container);
    }

    fn set_refresh_part_size(&mut self, id: GObjectId, size: Size) {
        let Some(o) = self.object(id) else {
            return;
        };
        let limit = |value: f32, min: f32, max: f32| {
            let v = value.max(min);
            if max > 0. { v.min(max) } else { v }
        };
        self.set_size(
            id,
            limit(size.width, o.min_width, o.max_width),
            limit(size.height, o.min_height, o.max_height),
        );
    }

    pub fn sync_scroll_refresh_bars(&mut self, container: GObjectId) {
        let Some(pane) = self.scroll_pane(container) else {
            return;
        };
        let mut updates = Vec::new();
        for (id, header) in [(pane.header(), true), (pane.footer(), false)] {
            if let Some(id) = id
                && let Some(o) = self.object(id)
                && o.parent() == Some(container)
                && let Some(layout) = pane.refresh_bar_layout(header, o.position(), o.size())
            {
                updates.push((id, layout));
            }
        }
        if updates.is_empty() {
            return;
        }
        for (id, layout) in updates {
            if layout.visible {
                self.set_xy(id, layout.position.x, layout.position.y);
                self.set_refresh_part_size(id, layout.size);
            }
            self.touch(id, dirty::LOOK);
        }
        self.refresh_stage();
    }

    pub fn lock_scroll_header(&mut self, container: GObjectId, size: f32) -> bool {
        self.lock_scroll_refresh(container, size, true)
    }
    pub fn lock_scroll_footer(&mut self, container: GObjectId, size: f32) -> bool {
        self.lock_scroll_refresh(container, size, false)
    }
    fn lock_scroll_refresh(&mut self, container: GObjectId, size: f32, header: bool) -> bool {
        let changed = self
            .objects
            .get_mut(container)
            .and_then(|o| o.control.component_mut())
            .and_then(|c| c.scroll_pane.as_mut())
            .is_some_and(|p| {
                if header {
                    p.lock_header(size)
                } else {
                    p.lock_footer(size)
                }
            });
        if changed {
            self.touch(container, dirty::SCROLL);
            self.sync_scroll_refresh_bars(container);
        }
        changed
    }

    /// Target-only release notifications; consume before the next advance to
    /// assign locks with native release-event ordering.
    pub fn drain_pull_releases(&mut self) -> Vec<(GObjectId, PullRelease)> {
        let events = std::mem::take(&mut self.pull_releases);
        events
            .into_iter()
            .filter(|(id, _)| self.contains(*id))
            .collect()
    }
}

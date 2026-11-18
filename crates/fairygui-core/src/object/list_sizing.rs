//! Native GList.resizeToFit and getMaxItemWidth, in logical coordinates.
use super::{GObjectId, ObjectStore};
use crate::widget::{ControlData, ListLayoutType, VirtualLayout};

#[cfg(test)]
#[path = "list_sizing_tests.rs"]
mod tests;

impl ObjectStore {
    // Collapsed tree cells remain owned in this runtime, while native GTree
    // removes them from GList's children. Pane overlays are never list items.
    fn list_sizing_items(&self, list: GObjectId) -> Option<Vec<GObjectId>> {
        let items = self.list_items(list)?;
        if let ControlData::Tree(data) = self.object(list)?.control() {
            Some(
                data.visible_nodes()
                    .into_iter()
                    .filter_map(|index| items.get(index).copied())
                    .collect(),
            )
        } else {
            Some(items)
        }
    }

    /// Maximum unscaled width of materialized rows, including invisible rows.
    /// Virtual lists inspect active rows only; collapsed tree cells and pane
    /// overlays are excluded. Non-list objects return None.
    pub fn list_max_item_width(&self, list: GObjectId) -> Option<f32> {
        Some(
            self.list_sizing_items(list)?
                .into_iter()
                .filter_map(|row| self.object(row))
                .map(|row| row.width())
                .fold(0., f32::max),
        )
    }

    /// Fit the viewport to a leading logical item count. None uses the native
    /// default limit of 100000. SingleColumn/FlowHorizontal fit height; other
    /// layouts fit width. Ordinary lists use the last included row's trailing
    /// edge, skipping invisible rows only when folding is enabled, and apply
    /// min_size. Virtual lists use the template size and current line count,
    /// deliberately ignoring min_size and individual measured row sizes.
    /// Margins and nonfloating scrollbar gutters are added to the outer size.
    /// Returns whether the effective outer size changed. Invalid objects or a
    /// nonfinite/negative minimum are rejected without changing size.
    pub fn resize_list_to_fit(
        &mut self,
        list: GObjectId,
        item_count: Option<usize>,
        min_size: f32,
    ) -> bool {
        if !min_size.is_finite() || min_size < 0. || self.list_data(list).is_none() {
            return false;
        }
        self.layout_list(list);
        self.layout_scroll_bars(list);
        let data = self.list_data(list).unwrap();
        let vertical = matches!(
            data.layout,
            ListLayoutType::SingleColumn | ListLayoutType::FlowHorizontal
        );
        let count = item_count.unwrap_or(100000).min(self.list_num_items(list));
        let extent = if let Some(state) = self.virtual_states.get(&list) {
            let config = VirtualLayout::from_list(data, self.list_layout_view(list).unwrap());
            let lines = count.div_ceil(config.counts(state.geometry.item_size).0);
            let length = if vertical {
                state.geometry.item_size.height
            } else {
                state.geometry.item_size.width
            };
            lines as f32 * length + lines.saturating_sub(1) as f32 * config.gap()
        } else {
            self.list_sizing_items(list)
                .unwrap()
                .into_iter()
                .take(count)
                .rev()
                .filter_map(|row| self.object(row))
                .find(|row| !data.fold_invisible_items || row.visible())
                .map_or(min_size, |row| {
                    if vertical {
                        row.y() + row.height()
                    } else {
                        row.x() + row.width()
                    }
                    .max(min_size)
                })
        };
        let before = self.object(list).unwrap().size();
        if vertical {
            self.set_component_view_height(list, extent);
        } else {
            self.set_component_view_width(list, extent);
        }
        self.layout_list(list);
        self.layout_scroll_bars(list);
        self.object(list).unwrap().size() != before
    }
}

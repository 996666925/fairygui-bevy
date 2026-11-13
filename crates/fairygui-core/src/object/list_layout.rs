use super::{GObjectId, ObjectStore};
use crate::{
    geom::{Point, Size},
    widget::{ListData, ListLayoutType, VirtualLayout},
};

/// Live layout fields, independent of list content and selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListLayoutSettings {
    pub layout: ListLayoutType,
    pub line_count: i32,
    pub column_count: i32,
    pub line_gap: i32,
    pub column_gap: i32,
    /// 0 left, 1 center, 2 right.
    pub align: i32,
    /// 0 top, 1 middle, 2 bottom.
    pub vertical_align: i32,
    pub auto_resize_item: bool,
}
impl Default for ListLayoutSettings {
    fn default() -> Self {
        Self::from_list(&ListData::default())
    }
}
impl ListLayoutSettings {
    pub(crate) fn from_list(data: &ListData) -> Self {
        Self {
            layout: data.layout,
            line_count: data.line_count,
            column_count: data.column_count,
            line_gap: data.line_gap,
            column_gap: data.column_gap,
            align: data.align,
            vertical_align: data.vertical_align,
            auto_resize_item: data.auto_resize_item,
        }
    }
    fn assign(self, data: &mut ListData) {
        data.layout = self.layout;
        data.line_count = self.line_count;
        data.column_count = self.column_count;
        data.line_gap = self.line_gap;
        data.column_gap = self.column_gap;
        data.align = self.align;
        data.vertical_align = self.vertical_align;
        data.auto_resize_item = self.auto_resize_item;
    }
}
impl ObjectStore {
    pub fn list_layout_settings(&self, list: GObjectId) -> Option<ListLayoutSettings> {
        Some(ListLayoutSettings::from_list(self.list_data(list)?))
    }

    /// Apply a coherent layout. Invalid counts/alignment or virtual geometry
    /// leave the previous layout intact, including a loop's supported layouts.
    pub fn set_list_layout_settings(
        &mut self,
        list: GObjectId,
        settings: ListLayoutSettings,
    ) -> bool {
        if settings.line_count < 0
            || settings.column_count < 0
            || !(0..=2).contains(&settings.align)
            || !(0..=2).contains(&settings.vertical_align)
            || self
                .list_layout_settings(list)
                .is_none_or(|old| old == settings)
        {
            return false;
        }
        let Some(data) = self.list_data(list) else {
            return false;
        };
        if let Some(state) = self.virtual_states.get(&list) {
            if state.looping
                && matches!(
                    settings.layout,
                    ListLayoutType::FlowHorizontal | ListLayoutType::FlowVertical
                )
            {
                return false;
            }
            let mut config = VirtualLayout::from_list(data, state.geometry.config.view);
            config.layout = settings.layout;
            config.columns = settings.column_count;
            config.rows = settings.line_count;
            config.line_gap = settings.line_gap;
            config.column_gap = settings.column_gap;
            config.align = settings.align;
            config.vertical_align = settings.vertical_align;
            config.auto_resize = settings.auto_resize_item;
            if !config.valid(state.geometry.item_size) {
                return false;
            }
        }
        settings.assign(self.list_data_mut(list).unwrap());
        if self.is_virtual_list(list) {
            self.invalidate_virtual_list(list, true);
        } else {
            self.layout_list(list);
        }
        true
    }

    pub(super) fn list_layout_view(&self, list: GObjectId) -> Option<Size> {
        let object = self.object(list)?;
        let data = self.list_data(list)?;
        Some(
            data.component
                .scroll_pane
                .as_ref()
                .filter(|p| p.view_size().width > 0. && p.view_size().height > 0.)
                .map_or(
                    Size::new(
                        (object.width() - data.item_margin.horizontal()).max(0.),
                        (object.height() - data.item_margin.vertical()).max(0.),
                    ),
                    |p| p.view_size(),
                ),
        )
    }

    /// Native container offset; logical row positions do not include alignment.
    pub fn list_alignment(&self, list: GObjectId) -> Point {
        if let Some(state) = self.virtual_states.get(&list) {
            return state.alignment;
        }
        let Some(data) = self.list_data(list) else {
            return Point::ZERO;
        };
        let Some(view) = self.list_layout_view(list) else {
            return Point::ZERO;
        };
        let content = self.list_bounds.get(&list).copied().unwrap_or(Size::ZERO);
        VirtualLayout::from_list(data, view).alignment(content)
    }
}

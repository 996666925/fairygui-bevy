//! 列表。
//!
//! 列表是容器派生的控件，属性来自两块描述：块 5 描述布局与选择方式，
//! 块 8 描述数据项。注意它的溢出方式在自己的块 5 里又写了一次，
//! 与容器块 0 的那份是两个独立字段，因此这里会再解析一次滚动配置。

use crate::binary::{ByteBuffer, StringTable};
use crate::error::Result;
use crate::geom::{Margin, Point, Size};
use crate::widget::ComponentData;
#[path = "pagination.rs"]
mod pagination;
#[cfg(test)]
#[path = "pagination_tests.rs"]
mod pagination_tests;

/// 列表布局方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ListLayoutType {
    /// 单列纵向。
    #[default]
    SingleColumn,
    /// 单行横向。
    SingleRow,
    /// 横向流式：排满一行后换行。
    FlowHorizontal,
    /// 纵向流式：排满一列后换列。
    FlowVertical,
    /// 分页：按每页的行列数排布。
    Pagination,
}

impl ListLayoutType {
    /// 由描述中的字节编号构造。
    pub const fn from_code(code: i8) -> Self {
        match code {
            1 => Self::SingleRow,
            2 => Self::FlowHorizontal,
            3 => Self::FlowVertical,
            4 => Self::Pagination,
            _ => Self::SingleColumn,
        }
    }
}

/// 选择模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ListSelectionMode {
    /// 单选。
    #[default]
    Single,
    /// 多选。
    Multiple,
    /// 多选，单击即可切换该项，无需按 Ctrl。
    MultipleSingleClick,
    /// 不可选择。
    None,
}

impl ListSelectionMode {
    /// 由描述中的字节编号构造。
    pub const fn from_code(code: i8) -> Self {
        match code {
            1 => Self::Multiple,
            2 => Self::MultipleSingleClick,
            3 => Self::None,
            _ => Self::Single,
        }
    }

    /// 是否允许多选。
    pub const fn allows_multiple(self) -> bool {
        matches!(self, Self::Multiple | Self::MultipleSingleClick)
    }
}

/// 单个数据项的描述。
///
/// 这些值会在创建项对象时覆盖到对象上（标题、图标、控制器页面等）。
#[derive(Debug, Clone, Default)]
pub struct ListItemData {
    /// 项的资源地址。为空时使用列表的默认项。
    pub url: Option<String>,
    /// 标题。
    pub title: Option<String>,
    /// 选中时的标题。
    pub selected_title: Option<String>,
    /// 图标。
    pub icon: Option<String>,
    /// 选中时的图标。
    pub selected_icon: Option<String>,
    /// 对象名。
    pub name: Option<String>,
    /// 需要设置的控制器页面：`(控制器名, 页面名)`。
    pub controller_pages: Vec<(String, String)>,
    /// 需要覆盖的属性：`(目标路径, 属性编号, 值)`。
    pub overrides: Vec<(String, i32, String)>,
}

/// 列表数据。
#[derive(Debug, Clone)]
pub struct ListData {
    /// 容器部分的数据。
    pub component: ComponentData,
    /// 布局方式。
    pub layout: ListLayoutType,
    /// 选择模式。
    pub selection_mode: ListSelectionMode,
    /// 项在行内的水平对齐。
    pub align: i32,
    /// 项在列内的垂直对齐。
    pub vertical_align: i32,
    /// 行间距。
    pub line_gap: i32,
    /// 列间距。
    pub column_gap: i32,
    /// 每页行数（分页布局使用）。
    pub line_count: i32,
    /// 每页列数（分页布局使用）。
    pub column_count: i32,
    /// 项是否随列表尺寸自动拉伸。
    pub auto_resize_item: bool,
    /// 子对象的渲染顺序。
    pub children_render_order: i32,
    /// 渲染顺序的参考项。
    pub apex_index: i32,
    /// 项四周的边距。
    pub item_margin: Margin,
    /// 点击时是否把该项滚动到可视区。
    pub scroll_item_to_view_on_click: bool,
    /// 是否忽略不可见状的项。
    pub fold_invisible_items: bool,
    /// 默认项的资源地址。
    pub default_item: Option<String>,
    /// 数据项。
    pub items: Vec<ListItemData>,
    /// 当前选中的项序号，`-1` 表示未选中。
    pub selected_index: i32,
    /// Snapshot of selected materialized child indices, in child order.
    pub selected_indices: Vec<usize>,
    /// Shift-selection anchor. Clearing selection does not reset it.
    pub last_selected_index: Option<usize>,
    /// Parent controller index; -1 means no selection controller.
    pub selection_controller: i32,
}

impl Default for ListData {
    fn default() -> Self {
        Self {
            component: ComponentData::default(),
            layout: ListLayoutType::default(),
            selection_mode: ListSelectionMode::default(),
            align: 0,
            vertical_align: 0,
            line_gap: 0,
            column_gap: 0,
            line_count: 0,
            column_count: 0,
            auto_resize_item: false,
            children_render_order: 0,
            apex_index: 0,
            item_margin: Margin::default(),
            scroll_item_to_view_on_click: true,
            fold_invisible_items: false,
            default_item: None,
            items: Vec::new(),
            selected_index: -1,
            selected_indices: Vec::new(),
            last_selected_index: None,
            selection_controller: -1,
        }
    }
}

impl ListData {
    /// 数据项数量。
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// 按布局计算每个项的左上角坐标。
    ///
    /// `item_sizes` 是各项的原始尺寸，`view_size` 是视口尺寸（流式与分页布局
    /// 需要它来决定换行与分页）。
    pub fn layout_items(&self, item_sizes: &[Size], view_size: Size) -> Vec<Point> {
        let mut positions = Vec::with_capacity(item_sizes.len());
        let line_gap = self.line_gap as f32;
        let column_gap = self.column_gap as f32;
        let origin_x = self.item_margin.left;
        let origin_y = self.item_margin.top;

        match self.layout {
            ListLayoutType::SingleColumn => {
                let mut y = origin_y;
                for size in item_sizes {
                    positions.push(Point::new(origin_x, y));
                    y += size.height + line_gap;
                }
            }
            ListLayoutType::SingleRow => {
                let mut x = origin_x;
                for size in item_sizes {
                    positions.push(Point::new(x, origin_y));
                    x += size.width + column_gap;
                }
            }
            ListLayoutType::FlowHorizontal => {
                let (mut x, mut y) = (origin_x, origin_y);
                let mut line_height = 0.0f32;
                for size in item_sizes {
                    if x + size.width > view_size.width && x > origin_x {
                        x = origin_x;
                        y += line_height + line_gap;
                        line_height = 0.0;
                    }
                    positions.push(Point::new(x, y));
                    x += size.width + column_gap;
                    line_height = line_height.max(size.height);
                }
            }
            ListLayoutType::FlowVertical => {
                let (mut x, mut y) = (origin_x, origin_y);
                let mut column_width = 0.0f32;
                for size in item_sizes {
                    if y + size.height > view_size.height && y > origin_y {
                        y = origin_y;
                        x += column_width + column_gap;
                        column_width = 0.0;
                    }
                    positions.push(Point::new(x, y));
                    y += size.height + line_gap;
                    column_width = column_width.max(size.width);
                }
            }
            ListLayoutType::Pagination => {
                positions = self
                    .pagination_layout(item_sizes, item_sizes, view_size)
                    .positions;
            }
        }

        positions
    }

    /// 设置选中项。返回是否发生了变化。
    ///
    /// 选择模式为「不可选择」、或序号越界与为负时，都视为取消选中。
    pub fn set_selected(&mut self, index: i32) -> bool {
        self.set_selected_with_count(index, self.items.len())
    }

    /// Materialized rows can exist without serialized ListItemData.
    pub(crate) fn set_selected_with_count(&mut self, index: i32, count: usize) -> bool {
        let index = if self.selection_mode == ListSelectionMode::None
            || index < 0
            || index as usize >= count
        {
            -1
        } else {
            index
        };

        let indices: Vec<_> = usize::try_from(index).into_iter().collect();
        if index == self.selected_index && self.selected_indices == indices {
            return false;
        }
        self.selected_index = index;
        self.selected_indices = indices;
        if let Ok(index) = usize::try_from(index) {
            self.last_selected_index = Some(index);
        }
        true
    }

    /// 当前选中项的序号，`-1` 表示未选中。
    pub fn current_selection(&self) -> i32 {
        self.selected_index
    }

    /// 当前选中项的数据。
    pub fn selected_item(&self) -> Option<&ListItemData> {
        if self.selected_index < 0 {
            return None;
        }
        self.items.get(self.selected_index as usize)
    }

    /// 是否选中了某一项。
    pub fn is_selected(&self, index: usize) -> bool {
        self.selected_indices.contains(&index)
    }

    /// 计算当前可视区内的项范围。
    ///
    /// 虚拟化列表据此只创建可视项，而不是把成千上万项都建出来。
    /// 返回的是半开区间，可能为空（例如列表尚未布局或滚动到空白处）。
    ///
    /// `buffer` 是向两端额外扩展的项数。官方在边界外会多渲染一项，
    /// 避免滚动时边缘出现空白——这里把它做成显式参数，方便按需调整。
    pub fn visible_range(
        &self,
        positions: &[Point],
        item_sizes: &[Size],
        view_size: Size,
        scroll: Point,
        buffer: usize,
    ) -> std::ops::Range<usize> {
        let view = crate::geom::Rect::new(scroll.x, scroll.y, view_size.width, view_size.height);

        let mut first = usize::MAX;
        let mut last = 0usize;

        for (index, (position, size)) in positions.iter().zip(item_sizes).enumerate() {
            let item = crate::geom::Rect::new(position.x, position.y, size.width, size.height);
            if item.intersects(&view) {
                first = first.min(index);
                last = last.max(index);
            }
        }

        if first == usize::MAX {
            return 0..0;
        }

        first.saturating_sub(buffer)..(last + 1 + buffer).min(positions.len())
    }

    /// 把实际项索引映射成逻辑项索引。
    ///
    /// 循环列表内部会准备比逻辑项数多若干倍的项（官方用的是 6 倍），
    /// 因此取用数据时要按逻辑项数取模。
    pub fn logical_index(&self, raw_index: usize, looping: bool) -> usize {
        if looping && !self.items.is_empty() {
            raw_index % self.items.len()
        } else {
            raw_index
        }
    }

    /// 根据布局结果算出内容范围，用于设置滚动容器的内容尺寸。
    pub fn content_size(&self, positions: &[Point], item_sizes: &[Size]) -> Size {
        let mut width = 0.0f32;
        let mut height = 0.0f32;

        for (position, size) in positions.iter().zip(item_sizes.iter()) {
            width = width.max(position.x + size.width + self.item_margin.right);
            height = height.max(position.y + size.height + self.item_margin.bottom);
        }

        Size::new(width, height)
    }
}

/// 读取列表的布局与数据项。
///
/// `with_items` 为假时只读布局部分：树的项表结构与列表不同，需要由它自己解析。
pub(crate) fn read_list(
    data: &mut ListData,
    buffer: &mut ByteBuffer<'_>,
    table: &StringTable,
    begin_pos: usize,
    with_items: bool,
    scroll_defaults: crate::scroll::ScrollDefaults,
) -> Result<()> {
    // ---- 块 5：布局 ----
    if buffer.seek(begin_pos, 5)? {
        data.layout = ListLayoutType::from_code(buffer.read_i8()?);
        data.selection_mode = ListSelectionMode::from_code(buffer.read_i8()?);
        data.align = i32::from(buffer.read_i8()?);
        data.vertical_align = i32::from(buffer.read_i8()?);
        data.line_gap = i32::from(buffer.read_i16()?);
        data.column_gap = i32::from(buffer.read_i16()?);
        data.line_count = i32::from(buffer.read_i16()?);
        data.column_count = i32::from(buffer.read_i16()?);
        data.auto_resize_item = buffer.read_bool()?;
        data.children_render_order = i32::from(buffer.read_i8()?);
        data.apex_index = i32::from(buffer.read_i16()?);

        if buffer.read_bool()? {
            data.item_margin.top = buffer.read_i32()? as f32;
            data.item_margin.bottom = buffer.read_i32()? as f32;
            data.item_margin.left = buffer.read_i32()? as f32;
            data.item_margin.right = buffer.read_i32()? as f32;
        }

        // 列表自己也写了一份溢出方式。若为可滚动，配置同样位于块 7。
        let overflow = buffer.read_i8()?;
        data.component.overflow = crate::widget::OverflowType::from_code(overflow);
        if overflow == 2 {
            let saved = buffer.position();
            if buffer.seek(begin_pos, 7)? {
                let mut pane = crate::scroll::ScrollPane::default();
                pane.setup_with_defaults(buffer, table, scroll_defaults)?;
                data.component.scroll_pane = Some(pane);
            }
            buffer.set_position(saved);
        }

        if buffer.read_bool()? {
            // 裁剪软化，渲染层暂不支持。
            buffer.skip(8);
        }

        if buffer.version() >= 2 {
            data.scroll_item_to_view_on_click = buffer.read_bool()?;
            data.fold_invisible_items = buffer.read_bool()?;
        }
    }

    // ---- 块 8：数据项 ----
    if with_items && buffer.seek(begin_pos, 8)? {
        data.default_item = buffer.read_s(table)?;
        read_items(data, buffer, table)?;
    }

    if buffer.seek(begin_pos, 6)? {
        data.selection_controller = i32::from(buffer.read_i16()?);
    }

    Ok(())
}

/// 读取数据项表。
fn read_items(data: &mut ListData, buffer: &mut ByteBuffer<'_>, table: &StringTable) -> Result<()> {
    let count = buffer.read_i16()?.max(0) as usize;

    for _ in 0..count {
        let next_pos = i32::from(buffer.read_i16()?);
        let end = buffer.resolve_relative(next_pos)?;

        let mut item = ListItemData {
            url: buffer.read_s(table)?,
            ..Default::default()
        };

        // 没有指定资源地址时回落到默认项；仍然为空说明该项没有内容。
        if item.url.is_none() {
            item.url = data.default_item.clone();
        }

        if item.url.is_some() {
            setup_item(&mut item, buffer, table)?;
        }

        data.items.push(item);
        buffer.set_position(end);
    }

    Ok(())
}

/// 读取单个数据项的覆盖值。
pub(super) fn setup_item(
    item: &mut ListItemData,
    buffer: &mut ByteBuffer<'_>,
    table: &StringTable,
) -> Result<()> {
    item.title = buffer.read_s(table)?;
    item.selected_title = buffer.read_s(table)?;
    item.icon = buffer.read_s(table)?;
    item.selected_icon = buffer.read_s(table)?;
    item.name = buffer.read_s(table)?;

    // 控制器页面
    let controller_count = buffer.read_i16()?.max(0) as usize;
    for _ in 0..controller_count {
        let controller = buffer.read_s(table)?.unwrap_or_default();
        let page = buffer.read_s(table)?.unwrap_or_default();
        item.controller_pages.push((controller, page));
    }

    // 属性覆盖（格式 2 及以上）
    if buffer.version() >= 2 {
        let override_count = buffer.read_i16()?.max(0) as usize;
        for _ in 0..override_count {
            let target = buffer.read_s(table)?.unwrap_or_default();
            let property_id = i32::from(buffer.read_i16()?);
            let value = buffer.read_s(table)?.unwrap_or_default();
            item.overrides.push((target, property_id, value));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list_with(layout: ListLayoutType) -> ListData {
        ListData {
            layout,
            ..Default::default()
        }
    }

    #[test]
    fn layout_codes_match_description() {
        assert_eq!(ListLayoutType::from_code(0), ListLayoutType::SingleColumn);
        assert_eq!(ListLayoutType::from_code(1), ListLayoutType::SingleRow);
        assert_eq!(ListLayoutType::from_code(2), ListLayoutType::FlowHorizontal);
        assert_eq!(ListLayoutType::from_code(3), ListLayoutType::FlowVertical);
        assert_eq!(ListLayoutType::from_code(4), ListLayoutType::Pagination);
        assert_eq!(ListSelectionMode::from_code(3), ListSelectionMode::None);
    }

    #[test]
    fn instance_block_six_reads_parent_selection_controller_and_rejects_truncation() {
        for controller in [-1i16, 0, 3] {
            let mut bytes = vec![0u8; 20];
            bytes[0] = 9;
            bytes[1] = 1;
            bytes[14..16].copy_from_slice(&20u16.to_be_bytes());
            bytes.extend_from_slice(&controller.to_be_bytes());
            let mut data = ListData::default();
            let mut buffer = ByteBuffer::new(&bytes);
            buffer.set_position(3);
            read_list(
                &mut data,
                &mut buffer,
                &StringTable::new(),
                0,
                true,
                crate::scroll::ScrollDefaults::default(),
            )
            .unwrap();
            assert_eq!(data.selection_controller, i32::from(controller));
            bytes.pop();
            assert!(
                read_list(
                    &mut ListData::default(),
                    &mut ByteBuffer::new(&bytes),
                    &StringTable::new(),
                    0,
                    true,
                    crate::scroll::ScrollDefaults::default(),
                )
                .is_err()
            );
        }
    }

    #[test]
    fn single_column_stacks_vertically() {
        let mut list = list_with(ListLayoutType::SingleColumn);
        list.line_gap = 5;
        let sizes = [Size::new(100.0, 20.0), Size::new(100.0, 30.0)];
        let positions = list.layout_items(&sizes, Size::new(100.0, 100.0));

        assert_eq!(positions[0], Point::new(0.0, 0.0));
        assert_eq!(positions[1], Point::new(0.0, 25.0));
        assert_eq!(
            list.content_size(&positions, &sizes),
            Size::new(100.0, 55.0)
        );
    }

    #[test]
    fn single_row_places_horizontally() {
        let mut list = list_with(ListLayoutType::SingleRow);
        list.column_gap = 10;
        let sizes = [Size::new(20.0, 50.0), Size::new(30.0, 50.0)];
        let positions = list.layout_items(&sizes, Size::new(500.0, 50.0));

        assert_eq!(positions[0], Point::new(0.0, 0.0));
        assert_eq!(positions[1], Point::new(30.0, 0.0));
    }

    #[test]
    fn flow_horizontal_wraps_at_view_width() {
        let list = list_with(ListLayoutType::FlowHorizontal);
        let sizes = [
            Size::new(60.0, 20.0),
            Size::new(60.0, 20.0),
            Size::new(60.0, 20.0),
        ];
        let positions = list.layout_items(&sizes, Size::new(150.0, 100.0));

        assert_eq!(positions[0].y, 0.0);
        assert_eq!(positions[1].y, 0.0);
        assert_eq!(positions[2].y, 20.0, "第三个放不下应当换行");
    }

    #[test]
    fn flow_vertical_wraps_at_view_height() {
        let list = list_with(ListLayoutType::FlowVertical);
        let sizes = [
            Size::new(20.0, 60.0),
            Size::new(20.0, 60.0),
            Size::new(20.0, 60.0),
        ];
        let positions = list.layout_items(&sizes, Size::new(100.0, 150.0));

        assert_eq!(positions[0].x, 0.0);
        assert_eq!(positions[1].x, 0.0);
        assert_eq!(positions[2].x, 20.0, "第三个放不下应当换列");
    }

    #[test]
    fn pagination_places_pages_side_by_side() {
        let mut list = list_with(ListLayoutType::Pagination);
        list.line_count = 1;
        list.column_count = 2;

        let sizes = [Size::new(50.0, 50.0); 3];
        let positions = list.layout_items(&sizes, Size::new(100.0, 50.0));

        assert_eq!(positions[0], Point::new(0.0, 0.0));
        assert_eq!(positions[1], Point::new(50.0, 0.0));
        assert_eq!(
            positions[2],
            Point::new(100.0, 0.0),
            "超出每页项数后应当排到下一页"
        );
    }

    #[test]
    fn visible_range_covers_items_in_view() {
        let list = list_with(ListLayoutType::SingleColumn);
        let sizes = vec![Size::new(100.0, 20.0); 10];
        let positions = list.layout_items(&sizes, Size::new(100.0, 100.0));

        // 视口贴在顶部时应当从第 0 项开始。
        let range = list.visible_range(&positions, &sizes, Size::new(100.0, 50.0), Point::ZERO, 0);
        assert_eq!(range.start, 0);
        assert!(range.end >= 2, "至少应覆盖两项，实际 {range:?}");

        // 向下滚动后应当跳过前面的项。
        let range = list.visible_range(
            &positions,
            &sizes,
            Size::new(100.0, 50.0),
            Point::new(0.0, 100.0),
            0,
        );
        assert!(range.start > 0, "滚动后应当跳过前面的项，实际 {range:?}");
    }

    #[test]
    fn buffer_extends_the_range_on_both_sides() {
        let list = list_with(ListLayoutType::SingleColumn);
        let sizes = vec![Size::new(100.0, 20.0); 10];
        let positions = list.layout_items(&sizes, Size::new(100.0, 100.0));

        let plain = list.visible_range(
            &positions,
            &sizes,
            Size::new(100.0, 50.0),
            Point::new(0.0, 100.0),
            0,
        );
        let buffered = list.visible_range(
            &positions,
            &sizes,
            Size::new(100.0, 50.0),
            Point::new(0.0, 100.0),
            1,
        );

        assert_eq!(buffered.start, plain.start - 1, "应当向前多取一项");
        assert_eq!(buffered.end, plain.end + 1, "应当向后多取一项");
    }

    #[test]
    fn buffer_never_goes_out_of_bounds() {
        let list = list_with(ListLayoutType::SingleColumn);
        let sizes = vec![Size::new(100.0, 20.0); 3];
        let positions = list.layout_items(&sizes, Size::new(100.0, 100.0));

        let range = list.visible_range(&positions, &sizes, Size::new(100.0, 500.0), Point::ZERO, 5);
        assert_eq!(range.start, 0);
        assert_eq!(range.end, 3, "缓冲不应越过项总数");
    }

    #[test]
    fn visible_range_is_empty_when_scrolled_past_content() {
        let list = list_with(ListLayoutType::SingleColumn);
        let sizes = vec![Size::new(100.0, 20.0); 3];
        let positions = list.layout_items(&sizes, Size::new(100.0, 100.0));

        let range = list.visible_range(
            &positions,
            &sizes,
            Size::new(100.0, 50.0),
            Point::new(0.0, 10_000.0),
            0,
        );
        assert!(range.is_empty(), "滚过内容之后不应有可见项，实际 {range:?}");
    }

    #[test]
    fn visible_range_with_no_items_is_empty() {
        let list = list_with(ListLayoutType::SingleColumn);
        let range = list.visible_range(&[], &[], Size::new(100.0, 100.0), Point::ZERO, 0);
        assert!(range.is_empty());
    }

    #[test]
    fn looping_wraps_raw_index_into_logical_index() {
        let list = ListData {
            layout: ListLayoutType::SingleColumn,
            items: vec![Default::default(); 3],
            ..Default::default()
        };

        // 循环列表内部项数是逻辑项数的若干倍，取数据时要取模。
        assert_eq!(list.logical_index(0, true), 0);
        assert_eq!(list.logical_index(4, true), 1);
        assert_eq!(list.logical_index(6, true), 0);

        // 非循环时原样返回。
        assert_eq!(list.logical_index(4, false), 4);
    }

    #[test]
    fn logical_index_with_no_items_is_safe() {
        let list = list_with(ListLayoutType::SingleColumn);
        assert_eq!(list.logical_index(5, true), 5, "没有数据项时不应除以零");
    }

    #[test]
    fn item_margin_shifts_origin() {
        let mut list = list_with(ListLayoutType::SingleColumn);
        list.item_margin = Margin::new(5.0, 7.0, 0.0, 0.0);
        let positions = list.layout_items(&[Size::new(10.0, 10.0)], Size::new(100.0, 100.0));
        assert_eq!(positions[0], Point::new(5.0, 7.0));
    }
}

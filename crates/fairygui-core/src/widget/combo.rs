//! 下拉框。
//!
//! 实例数据来自父容器的块 6（项表、当前值、可见项数、弹层方向），
//! 扩展数据来自组件自身描述的块 6（下拉弹层所用的资源地址）。

use crate::binary::{ByteBuffer, StringTable};
use crate::color::Color;
use crate::error::Result;
use crate::geom::{Point, Size};
use crate::object::GObjectId;
use crate::widget::ComponentData;

/// 展开中的下拉弹层。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComboPopup {
    /// 弹层对象。
    pub dropdown: GObjectId,
    /// 弹层里的列表，选中项由它承载。
    pub list: GObjectId,
    /// 列表项模板地址，供适配器在数据变化时复用或重建项。
    pub item_url: Option<String>,
}

/// 计算弹层相对调用者的位置。
///
/// 默认向下展开；下方放不下时翻到上方。`downward` 显式指定方向时以它为准，
/// 但越界仍会被修正——否则弹层会有一半落在窗口之外。
pub fn popup_position(
    caller_pos: Point,
    caller_size: Size,
    popup_size: Size,
    container_size: Size,
    downward: Option<bool>,
) -> Point {
    let below = caller_pos.y + caller_size.height;
    let above = caller_pos.y - popup_size.height;

    let fits_below = below + popup_size.height <= container_size.height;
    let fits_above = above >= 0.0;

    let downward = downward.unwrap_or(fits_below || !fits_above);
    let mut y = if downward { below } else { above };

    // 两个方向都放不下时，只能贴住边界，至少保证整条弹层在容器内可见。
    y = y.clamp(0.0, (container_size.height - popup_size.height).max(0.0));

    // 横向与调用者左对齐，超出右边界时回退。
    let x = caller_pos
        .x
        .clamp(0.0, (container_size.width - popup_size.width).max(0.0));

    Point::new(x, y)
}

/// 下拉框数据。
#[derive(Debug, Clone)]
pub struct ComboBoxData {
    /// 容器部分的数据。
    pub component: ComponentData,
    /// 各选项的文本。
    pub items: Vec<String>,
    /// 各选项对应的值。
    pub values: Vec<String>,
    /// 各选项的图标。
    pub icons: Option<Vec<Option<String>>>,
    /// 当前显示文本。
    pub title: String,
    /// 当前图标。
    pub icon: Option<String>,
    /// 标题颜色。
    pub title_color: Option<Color>,
    /// 弹层中最多显示多少项，超出后滚动。
    pub visible_item_count: i32,
    /// 弹层展开方向。
    pub popup_direction: i32,
    /// 与选项联动的控制器编号。
    pub selection_controller: i32,
    /// 当前选中项的序号，`-1` 表示未选中。
    pub selected_index: i32,
    /// 下拉弹层的资源地址。
    pub dropdown: Option<String>,
    /// Native pointer state is independent of whether the dropdown is open.
    pub hovered: bool,
    pub pressed: bool,
}

impl Default for ComboBoxData {
    fn default() -> Self {
        Self {
            component: ComponentData::default(),
            items: Vec::new(),
            values: Vec::new(),
            icons: None,
            title: String::new(),
            icon: None,
            title_color: None,
            visible_item_count: 10,
            popup_direction: 0,
            selection_controller: -1,
            selected_index: -1,
            dropdown: None,
            hovered: false,
            pressed: false,
        }
    }
}

impl ComboBoxData {
    /// 选项数量。
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// 当前选中项对应的值。
    pub fn selected_value(&self) -> Option<&str> {
        if self.selected_index < 0 {
            return None;
        }
        self.values
            .get(self.selected_index as usize)
            .map(String::as_str)
    }

    /// 按文本查找选项序号。
    pub fn index_of(&self, text: &str) -> Option<usize> {
        self.items.iter().position(|item| item == text)
    }

    /// 按值查找选项序号。
    ///
    /// 界面上显示的是文本，但业务上更常用「值」来标识选项。
    pub fn index_of_value(&self, value: &str) -> Option<usize> {
        self.values.iter().position(|item| item == value)
    }

    /// 按值选中。值不存在时取消选择，与原版 value setter 一致。
    pub fn set_selected_value(&mut self, value: &str) -> bool {
        self.set_selected_index(self.index_of_value(value).map_or(-1, |index| index as i32))
    }

    /// 当前选中项的图标。
    pub fn selected_icon(&self) -> Option<&str> {
        if self.selected_index < 0 {
            return None;
        }
        self.icons
            .as_ref()?
            .get(self.selected_index as usize)
            .and_then(|icon| icon.as_deref())
    }

    /// 切换选中项。返回是否发生变化。
    pub fn set_selected_index(&mut self, index: i32) -> bool {
        if index == self.selected_index {
            return false;
        }
        self.selected_index = index;

        let at = usize::try_from(index).ok();
        self.title = at
            .and_then(|at| self.items.get(at))
            .cloned()
            .unwrap_or_default();
        if let Some(icons) = &self.icons {
            if at.is_some_and(|at| at < self.items.len()) {
                if let Some(icon) = at.and_then(|at| icons.get(at)) {
                    self.icon = icon.clone();
                }
            } else {
                self.icon = None;
            }
        }

        true
    }
}

/// 读取下拉框的实例数据（父容器中的块 6）。
pub(crate) fn read_combo(
    data: &mut ComboBoxData,
    buffer: &mut ByteBuffer<'_>,
    table: &StringTable,
    begin_pos: usize,
) -> Result<()> {
    if !buffer.seek(begin_pos, 6)? {
        return Ok(());
    }

    // Instance blocks start with the declared object type, before the item count.
    if crate::package::ObjectType::from_code(buffer.read_u8()?)
        != crate::package::ObjectType::ComboBox
    {
        return Ok(());
    }

    let count = buffer.read_i16()?.max(0) as usize;
    for _ in 0..count {
        let next_pos = i32::from(buffer.read_i16()?);
        let end = buffer.resolve_relative(next_pos)?;

        data.items.push(buffer.read_s(table)?.unwrap_or_default());
        data.values.push(buffer.read_s(table)?.unwrap_or_default());
        let icon = buffer.read_s(table)?;
        if icon.is_some() {
            let icons = data.icons.get_or_insert_with(Vec::new);
            icons.resize(data.items.len(), None);
            *icons.last_mut().unwrap() = icon;
        }

        buffer.set_position(end);
    }

    if let Some(title) = buffer.read_s(table)? {
        data.selected_index = data
            .items
            .iter()
            .position(|item| *item == title)
            .map_or(-1, |index| index as i32);
        data.title = title;
    } else if let Some(first) = data.items.first() {
        data.selected_index = 0;
        data.title = first.clone();
    } else {
        data.selected_index = -1;
    }

    data.icon = buffer.read_s(table)?;

    if buffer.read_bool()? {
        data.title_color = Some(read_color(buffer)?);
    }

    let visible_item_count = buffer.read_i32()?;
    if visible_item_count > 0 {
        data.visible_item_count = visible_item_count;
    }

    data.popup_direction = i32::from(buffer.read_i8()?);
    data.selection_controller = i32::from(buffer.read_i16()?);

    Ok(())
}

/// 读取下拉框的扩展数据（组件自身描述的块 6）。
pub(crate) fn read_combo_extension(
    data: &mut ComboBoxData,
    buffer: &mut ByteBuffer<'_>,
    table: &StringTable,
) -> Result<()> {
    if !buffer.seek(0, 6)? {
        return Ok(());
    }
    data.dropdown = buffer.read_s(table)?;
    Ok(())
}

fn read_color(buffer: &mut ByteBuffer<'_>) -> Result<Color> {
    let r = buffer.read_u8()?;
    let g = buffer.read_u8()?;
    let b = buffer.read_u8()?;
    let a = buffer.read_u8()?;
    Ok(Color::from_rgba8(r, g, b, a))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn combo_with(items: &[&str]) -> ComboBoxData {
        let mut data = ComboBoxData::default();
        for (index, item) in items.iter().enumerate() {
            data.items.push((*item).to_string());
            data.values.push(format!("v{index}"));
        }
        data
    }

    #[test]
    fn exported_icons_keep_null_array_and_sparse_trailing_length_distinct() {
        let mut table = StringTable::new();
        table.push(Some("icon".into()));
        for has_icon in [false, true] {
            let mut bytes = vec![0u8; 20];
            bytes[0] = 7;
            bytes[1] = 1;
            bytes[14..16].copy_from_slice(&20u16.to_be_bytes());
            bytes.push(13); // ObjectType.ComboBox
            bytes.extend_from_slice(&3i16.to_be_bytes());
            for index in 0..3 {
                bytes.extend_from_slice(&6i16.to_be_bytes());
                bytes.extend_from_slice(&65533u16.to_be_bytes()); // empty caption
                bytes.extend_from_slice(&65533u16.to_be_bytes()); // empty value
                bytes.extend_from_slice(
                    &(if has_icon && index == 1 { 0u16 } else { 65534 }).to_be_bytes(),
                );
            }
            bytes.extend_from_slice(&65534u16.to_be_bytes()); // default title
            bytes.extend_from_slice(&65534u16.to_be_bytes()); // no explicit icon
            bytes.push(0); // no title color
            bytes.extend_from_slice(&10i32.to_be_bytes());
            bytes.push(0); // automatic popup direction
            bytes.extend_from_slice(&(-1i16).to_be_bytes());
            let mut combo = ComboBoxData::default();
            read_combo(&mut combo, &mut ByteBuffer::new(&bytes), &table, 0).unwrap();
            if has_icon {
                assert_eq!(combo.icons, Some(vec![None, Some("icon".into())]));
            } else {
                assert!(combo.icons.is_none());
            }
        }
    }

    #[test]
    fn selecting_updates_title_and_value() {
        let mut combo = combo_with(&["a", "b", "c"]);
        combo.selected_index = -1;
        assert_eq!(combo.selected_value(), None);

        assert!(combo.set_selected_index(1));
        assert_eq!(combo.title, "b");
        assert_eq!(combo.selected_value(), Some("v1"));

        assert!(!combo.set_selected_index(1), "切到同一项不算变化");
    }

    #[test]
    fn out_of_range_preserves_requested_index_with_no_selected_value() {
        let mut combo = combo_with(&["a"]);
        combo.set_selected_index(0);
        assert!(combo.set_selected_index(9));
        assert_eq!(combo.selected_index, 9);
        assert_eq!(combo.selected_value(), None);
    }

    #[test]
    fn popup_opens_below_when_there_is_room() {
        let pos = popup_position(
            Point::new(100.0, 50.0),
            Size::new(200.0, 30.0),
            Size::new(200.0, 120.0),
            Size::new(800.0, 600.0),
            None,
        );
        assert_eq!(pos, Point::new(100.0, 80.0), "下方放得下就向下展开");
    }

    #[test]
    fn popup_flips_up_when_the_bottom_is_crowded() {
        let pos = popup_position(
            Point::new(100.0, 500.0),
            Size::new(200.0, 30.0),
            Size::new(200.0, 120.0),
            Size::new(800.0, 600.0),
            None,
        );
        assert_eq!(pos.y, 380.0, "下方放不下时翻到上方");
    }

    #[test]
    fn explicit_direction_overrides_the_auto_choice() {
        let pos = popup_position(
            Point::new(100.0, 50.0),
            Size::new(200.0, 30.0),
            Size::new(200.0, 120.0),
            Size::new(800.0, 600.0),
            Some(false),
        );
        assert_eq!(pos.y, 0.0, "显式要求向上时按上方定位，并夹进容器内");
    }

    #[test]
    fn popup_is_kept_inside_the_container() {
        // 上下都放不下：只能贴住边界，至少保证整条都在容器里。
        let pos = popup_position(
            Point::new(0.0, 580.0),
            Size::new(100.0, 30.0),
            Size::new(100.0, 700.0),
            Size::new(800.0, 600.0),
            None,
        );
        assert_eq!(pos.y, 0.0);

        // 调用者贴右边界时横向也要回退。
        let pos = popup_position(
            Point::new(750.0, 0.0),
            Size::new(100.0, 30.0),
            Size::new(200.0, 100.0),
            Size::new(800.0, 600.0),
            None,
        );
        assert_eq!(pos.x, 600.0, "右侧放不下时向左回退");
    }

    #[test]
    fn lookup_by_text() {
        let combo = combo_with(&["alpha", "beta"]);
        assert_eq!(combo.index_of("beta"), Some(1));
        assert_eq!(combo.index_of("missing"), None);
        assert_eq!(combo.item_count(), 2);
    }
}

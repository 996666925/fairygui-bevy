//! 控制器与解析时的全局环境。
//!
//! 控制器是一组「页面」，同一时刻只有一个生效。切换页面会驱动齿轮联动的属性、
//! 列表的分页、按钮的状态等，是界面「状态」的主要来源。
//!
//! 首屏页面可能由分支或全局变量决定，因此解析时需要一份环境信息。

use std::collections::HashMap;

use crate::binary::{ByteBuffer, StringTable};
use crate::error::Result;

/// 解析时的全局环境。
///
/// 对应官方运行时的 `UIPackage.branch` 与 `UIPackage.getVar`：它们不在包内，
/// 而由使用方在加载时提供。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackageEnv {
    /// 当前分支名，空串表示未启用。
    pub branch: String,
    /// 全局变量表。
    pub vars: HashMap<String, String>,
}

/// 控制器。
#[derive(Debug, Clone)]
pub struct Controller {
    /// 控制器名。
    pub name: String,
    /// 各页面的编号。
    pub page_ids: Vec<String>,
    /// 各页面的名称。
    pub page_names: Vec<String>,
    /// 当前页面序号，`-1` 表示未选中。
    pub selected_index: i32,
    /// 上一个页面序号，用于判断切换方向。
    pub previous_index: i32,
    /// 与单选按钮同组时的深度自动处理。
    pub auto_radio_group_depth: bool,
    /// 页面切换时要执行的动作。
    pub actions: Vec<crate::action::ControllerAction>,
}

impl Default for Controller {
    fn default() -> Self {
        Self {
            name: String::new(),
            page_ids: Vec::new(),
            page_names: Vec::new(),
            selected_index: -1,
            previous_index: -1,
            auto_radio_group_depth: false,
            actions: Vec::new(),
        }
    }
}

impl Controller {
    /// 页面数量。
    pub fn page_count(&self) -> usize {
        self.page_ids.len()
    }

    /// 当前页面的编号。
    pub fn selected_page_id(&self) -> Option<&str> {
        self.selected_page_id_at(self.selected_index)
    }

    /// 当前页面的名称。
    pub fn selected_page_name(&self) -> Option<&str> {
        self.selected_page_name_at(self.selected_index)
    }

    /// 上一个页面的编号。
    pub fn previous_page_id(&self) -> Option<&str> {
        self.selected_page_id_at(self.previous_index)
    }

    /// 指定序号的页面编号。
    pub fn selected_page_id_at(&self, index: i32) -> Option<&str> {
        if index < 0 {
            return None;
        }
        self.page_ids.get(index as usize).map(String::as_str)
    }

    /// 指定序号的页面名称。
    pub fn selected_page_name_at(&self, index: i32) -> Option<&str> {
        if index < 0 {
            return None;
        }
        self.page_names.get(index as usize).map(String::as_str)
    }

    /// 是否存在指定名称的页面。
    pub fn has_page(&self, name: &str) -> bool {
        self.page_names.iter().any(|page| page == name)
    }

    /// 按名称查找页面序号。
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.page_names.iter().position(|page| page == name)
    }

    /// 按导出页面 ID 查找页面序号。
    pub fn index_of_id(&self, id: &str) -> Option<usize> {
        self.page_ids.iter().position(|page| page == id)
    }

    /// 切换页面。返回是否真的发生了变化。
    pub fn set_selected_index(&mut self, index: i32) -> bool {
        let index = if index < 0 || index as usize >= self.page_ids.len() {
            -1
        } else {
            index
        };
        if index == self.selected_index {
            return false;
        }
        self.previous_index = self.selected_index;
        self.selected_index = index;
        true
    }

    /// 按名称切换页面。名称不存在时回到第一页，与原版 selectedPage 一致。
    pub fn set_selected_page_name(&mut self, name: &str) -> bool {
        self.set_selected_index(self.index_of(name).unwrap_or(0) as i32)
    }

    /// 按页面 ID 切换；ID 不存在时取消选择，与官方 selectedPageId 一致。
    pub fn set_selected_page_id(&mut self, id: &str) -> bool {
        self.set_selected_index(self.index_of_id(id).map_or(-1, |index| index as i32))
    }

    /// 读取控制器描述。
    ///
    /// `buffer` 的位置应当是控制器块的起点。
    pub(crate) fn setup(
        &mut self,
        buffer: &mut ByteBuffer<'_>,
        table: &StringTable,
        env: &PackageEnv,
    ) -> Result<()> {
        let begin_pos = buffer.position();
        let mut home_page_index = 0i32;

        // ---- 块 0：名称 ----
        if buffer.seek(begin_pos, 0)? {
            self.name = buffer.read_s(table)?.unwrap_or_default();
            if buffer.read_bool()? {
                self.auto_radio_group_depth = true;
            }
        }

        // ---- 块 1：页面表与首屏页面 ----
        if buffer.seek(begin_pos, 1)? {
            let count = buffer.read_i16()?.max(0) as usize;
            for _ in 0..count {
                self.page_ids
                    .push(buffer.read_s(table)?.unwrap_or_default());
                self.page_names
                    .push(buffer.read_s(table)?.unwrap_or_default());
            }

            if buffer.version() >= 2 {
                let home_page_type = buffer.read_i8()?;
                match home_page_type {
                    1 => home_page_index = i32::from(buffer.read_i16()?),
                    2 => {
                        // 由分支决定首屏页面。
                        home_page_index =
                            self.index_of(&env.branch).map_or(0, |index| index as i32);
                    }
                    3 => {
                        // 由全局变量决定首屏页面。
                        let key = buffer.read_s(table)?.unwrap_or_default();
                        let value = env.vars.get(&key).cloned().unwrap_or_default();
                        home_page_index = self.index_of(&value).map_or(0, |index| index as i32);
                    }
                    _ => {}
                }
            }
        }

        // ---- 块 2：页面动作 ----
        if buffer.seek(begin_pos, 2)? {
            let count = buffer.read_i16()?.max(0) as usize;
            for _ in 0..count {
                let next_pos = i32::from(buffer.read_i16()?);
                let end = buffer.resolve_relative(next_pos)?;

                let action_type = buffer.read_u8()?;
                if let Some(action) =
                    crate::action::ControllerAction::setup(buffer, table, action_type)?
                {
                    self.actions.push(action);
                }

                // 动作块的长度由外层给出，读不完的直接跳过。
                buffer.set_position(end);
            }
        }

        self.selected_index = if self.page_ids.is_empty() {
            -1
        } else {
            home_page_index
        };
        self.previous_index = -1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binary::StringTable;

    fn table_with(entries: &[&str]) -> StringTable {
        let mut table = StringTable::new();
        for entry in entries {
            table.push(Some((*entry).to_string()));
        }
        table
    }

    fn controller_with_pages(names: &[&str]) -> Controller {
        let mut controller = Controller::default();
        for (index, name) in names.iter().enumerate() {
            controller.page_ids.push(format!("id{index}"));
            controller.page_names.push((*name).to_string());
        }
        controller
    }

    #[test]
    fn switching_pages_reports_change_and_remembers_previous() {
        let mut controller = controller_with_pages(&["a", "b", "c"]);
        controller.selected_index = 0;

        assert!(!controller.set_selected_index(0), "切到同一页不算变化");
        assert!(controller.set_selected_index(2));
        assert_eq!(controller.selected_page_name(), Some("c"));
        assert_eq!(controller.previous_page_id(), Some("id0"));
    }

    #[test]
    fn out_of_range_index_selects_nothing() {
        let mut controller = controller_with_pages(&["a"]);
        controller.selected_index = 0;
        assert!(controller.set_selected_index(5));
        assert_eq!(controller.selected_index, -1);
        assert_eq!(controller.selected_page_name(), None);
    }

    #[test]
    fn switching_by_name_falls_back_to_the_first_page() {
        let mut controller = controller_with_pages(&["a", "b"]);
        controller.selected_index = 0;
        assert!(!controller.set_selected_page_name("missing"));
        assert_eq!(controller.selected_index, 0);
        assert!(controller.set_selected_page_name("b"));
        assert_eq!(controller.selected_page_id(), Some("id1"));
        assert!(controller.has_page("b"));
        assert_eq!(controller.index_of("b"), Some(1));
        assert!(controller.set_selected_page_name("missing"));
        assert_eq!(controller.selected_index, 0);
        assert_eq!(controller.previous_index, 1);
    }

    #[test]
    fn page_names_ids_and_previous_indices_match_native_selection_trace() {
        // Executed against FairyGUI-layabox Controller.selectedPage/selectedPageId.
        let mut controller = Controller {
            page_ids: ["idle-id", "active-id", "other-id"]
                .map(str::to_owned)
                .to_vec(),
            page_names: ["Idle", "Active", "Other"].map(str::to_owned).to_vec(),
            selected_index: 0,
            ..Default::default()
        };
        for (by_id, value, expected, previous) in [
            (false, "Active", 1, 0),
            (false, "missing", 0, 1),
            (false, "missing", 0, 1),
            (true, "other-id", 2, 0),
            (true, "missing", -1, 2),
            (true, "missing", -1, 2),
            (false, "missing", 0, -1),
            (false, "Other", 2, 0),
            (true, "idle-id", 0, 2),
        ] {
            let old = controller.selected_index;
            let changed = if by_id {
                controller.set_selected_page_id(value)
            } else {
                controller.set_selected_page_name(value)
            };
            assert_eq!(changed, old != expected);
            assert_eq!(
                (controller.selected_index, controller.previous_index),
                (expected, previous),
                "{by_id}/{value}"
            );
        }
    }

    #[test]
    fn page_id_selection_keeps_ids_separate_from_names_and_clears_unknown_ids() {
        let mut controller = controller_with_pages(&["a", "b"]);
        assert_eq!(controller.selected_index, -1);
        assert!(controller.set_selected_page_id("id1"));
        assert_eq!(controller.selected_page_name(), Some("b"));
        assert!(controller.set_selected_page_id("b"));
        assert_eq!(controller.selected_page_id(), None);
        assert_eq!(controller.previous_page_id(), Some("id1"));
        assert!(!controller.set_selected_page_id("missing"));
    }

    #[test]
    fn setup_reads_name_and_pages() {
        // 控制器块：索引表占 10 字节（段数 + 宽度标志 + 两个偏移），
        // 块 0 从 10 开始，占 3 字节，块 1 从 13 开始。
        let mut data = vec![2, 0];
        data.extend_from_slice(&[0x00, 0x00, 0x00, 0x0A]); // 块 0
        data.extend_from_slice(&[0x00, 0x00, 0x00, 0x0D]); // 块 1
        data.extend_from_slice(&[0x00, 0x0A]); // 块 0：name 索引 10
        data.push(0); // 无 autoRadioGroupDepth
        data.extend_from_slice(&[0x00, 0x01]); // 块 1：1 个页面
        data.extend_from_slice(&[0x00, 0x0B]); // 页面编号索引 11
        data.extend_from_slice(&[0x00, 0x0C]); // 页面名称索引 12
        data.push(0); // 首屏类型 0

        let table = table_with(&[
            "", "", "", "", "", "", "", "", "", "", "ctrl", "pageId", "pageName",
        ]);
        let mut buffer = ByteBuffer::new(&data);
        let mut controller = Controller::default();
        controller
            .setup(&mut buffer, &table, &PackageEnv::default())
            .expect("controller setup");

        assert_eq!(controller.name, "ctrl");
        assert_eq!(controller.page_ids, vec!["pageId".to_string()]);
        assert_eq!(controller.page_names, vec!["pageName".to_string()]);
        assert_eq!(controller.selected_index, 0);
        assert_eq!(controller.previous_page_id(), None);
        assert!(!controller.auto_radio_group_depth);
    }
}

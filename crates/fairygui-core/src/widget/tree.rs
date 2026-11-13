//! 树。
//!
//! 树沿用列表的布局与滚动部分；项覆盖值之前额外记录文件夹与层级信息。
//! 另外缩进与点击展开策略位于块 9。

use crate::binary::{ByteBuffer, StringTable};
use crate::error::Result;
use crate::widget::list::ListData;

/// 树节点。
#[derive(Debug, Clone, Default)]
pub struct TreeNodeData {
    /// 节点的资源地址。为空时使用默认项。
    pub url: Option<String>,
    /// 是否为可展开的文件夹。
    pub is_folder: bool,
    /// 节点层级，从 0 开始。
    pub level: u8,
}

/// 树数据。
#[derive(Debug, Clone, Default)]
pub struct TreeData {
    /// 列表部分的数据。
    pub list: ListData,
    /// 每层缩进的像素数。
    pub indent: i32,
    /// 点击展开策略。
    ///
    /// `0` 点击不触发展开（只能用展开箭头）；`1` 点击文件夹即切换展开。
    ///
    /// 官方的 `2`（原意是双击展开）在源码里**是被注释掉的**，因此实际不生效，
    /// 这里保留字段但同样不赋予行为。
    pub click_to_expand: i32,
    /// 节点。
    pub nodes: Vec<TreeNodeData>,
    /// 每个文件夹节点的展开状态，与 `nodes` 一一对应。
    ///
    /// 导出的节点初始全部展开，对应 GTree.readItems 的 node.expanded = true。
    ///
    /// 字段对 crate 内可见：结构体更新语法要求所有字段可访问。
    pub(crate) expanded: Vec<bool>,
}

impl TreeData {
    /// 节点数量。
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// 是否应当在该节点上显示展开箭头。
    pub fn is_folder(&self, index: usize) -> bool {
        self.nodes.get(index).is_some_and(|node| node.is_folder)
    }

    /// 某个节点相对根节点的缩进像素数。
    pub fn indent_of(&self, index: usize) -> f32 {
        self.nodes
            .get(index)
            .map_or(0.0, |node| node.level as f32 * self.indent as f32)
    }

    /// 节点是否处于展开状态。非文件夹节点恒为真。
    pub fn is_expanded(&self, index: usize) -> bool {
        if !self.is_folder(index) {
            return true;
        }
        self.expanded.get(index).copied().unwrap_or(true)
    }

    /// 展开或收起一个文件夹节点。返回状态是否变化。
    pub fn set_expanded(&mut self, index: usize, expanded: bool) -> bool {
        if !self.is_folder(index) {
            return false;
        }

        // 记录表按需增长，缺省视为展开。
        if self.expanded.len() < self.nodes.len() {
            self.expanded.resize(self.nodes.len(), true);
        }

        if self.expanded[index] == expanded {
            return false;
        }
        self.expanded[index] = expanded;
        true
    }

    /// 切换文件夹节点的展开状态。
    pub fn toggle_expanded(&mut self, index: usize) -> bool {
        let current = self.is_expanded(index);
        self.set_expanded(index, !current)
    }

    /// 展开某个节点的全部祖先，使它变为可见。
    ///
    /// 官方在选中节点时会这么做。返回是否有状态变化。
    pub fn expand_to(&mut self, index: usize) -> bool {
        // 先把层级取出来，避免随后的可变借用与它冲突。
        let target_level = match self.nodes.get(index) {
            Some(target) => target.level,
            None => return false,
        };
        if target_level == 0 {
            return false;
        }

        // 从目标往前找层级更浅的文件夹，逐个展开；层级递减到 0 即到根。
        let mut changed = false;
        let mut current_level = target_level;

        for candidate in (0..index).rev() {
            let (node_level, is_folder) = match self.nodes.get(candidate) {
                Some(node) => (node.level, node.is_folder),
                None => continue,
            };

            if node_level >= current_level {
                continue;
            }

            current_level = node_level;
            if is_folder && !self.is_expanded(candidate) {
                changed |= self.set_expanded(candidate, true);
            }
            if node_level == 0 {
                break;
            }
        }

        changed
    }

    /// 全部展开。
    pub fn expand_all(&mut self) {
        self.expanded = vec![true; self.nodes.len()];
    }

    /// 全部收起。
    ///
    /// 隐式根不在 nodes 中；所有导出的文件夹（含顶层）都会收起。
    pub fn collapse_all(&mut self) {
        self.expanded = self.nodes.iter().map(|node| !node.is_folder).collect();
    }

    /// 当前可见的节点序号。
    ///
    /// 收起的文件夹之后、层级更深的节点都不可见，直到遇到层级不深于它的节点。
    pub fn visible_nodes(&self) -> Vec<usize> {
        let mut visible = Vec::with_capacity(self.nodes.len());
        // 当前被收起、从而遮蔽子节点的层级。
        let mut hidden_from: Option<u8> = None;

        for (index, node) in self.nodes.iter().enumerate() {
            if let Some(level) = hidden_from {
                if node.level > level {
                    continue;
                }
                hidden_from = None;
            }

            visible.push(index);

            if node.is_folder && !self.is_expanded(index) {
                hidden_from = Some(node.level);
            }
        }

        visible
    }

    /// 填好展开状态，使其与节点数量一致。
    ///
    /// 解析完成后调用一次即可。
    pub(crate) fn reset_expansion(&mut self) {
        self.expanded = vec![true; self.nodes.len()];
    }
}

/// 读取树。
pub(crate) fn read_tree(
    data: &mut TreeData,
    buffer: &mut ByteBuffer<'_>,
    table: &StringTable,
    begin_pos: usize,
    scroll_defaults: crate::scroll::ScrollDefaults,
) -> Result<()> {
    // 布局部分与列表一致，但项表结构不同，因此交给下面的分支单独处理。
    super::list::read_list(
        &mut data.list,
        buffer,
        table,
        begin_pos,
        false,
        scroll_defaults,
    )?;

    // ---- 块 9：树自己的设置 ----
    if buffer.seek(begin_pos, 9)? {
        data.indent = buffer.read_i32()?;
        data.click_to_expand = i32::from(buffer.read_u8()?);
    }

    // ---- 块 8：节点表 ----
    if buffer.seek(begin_pos, 8)? {
        data.list.default_item = buffer.read_s(table)?;

        let count = buffer.read_i16()?.max(0) as usize;
        for _ in 0..count {
            let next_pos = i32::from(buffer.read_i16()?);
            let end = buffer.resolve_relative(next_pos)?;

            let url = buffer.read_s(table)?;
            let is_folder = buffer.read_bool()?;
            let level = buffer.read_u8()?;

            let url = url.or_else(|| data.list.default_item.clone());
            if url.is_none() {
                buffer.set_position(end);
                continue;
            }
            data.nodes.push(TreeNodeData {
                url: url.clone(),
                is_folder,
                level,
            });
            let mut item = super::list::ListItemData {
                url,
                ..Default::default()
            };
            super::list::setup_item(&mut item, buffer, table)?;
            if buffer.position() > end {
                return Err(crate::error::Error::invalid_data(
                    end,
                    "tree item exceeds its record",
                ));
            }
            data.list.items.push(item);

            buffer.set_position(end);
        }
    }

    // 列表侧的项数与树的节点数保持一致，便于复用列表的布局计算。
    data.reset_expansion();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree_with(levels: &[(bool, u8)]) -> TreeData {
        let mut tree = TreeData {
            indent: 15,
            ..Default::default()
        };
        for (is_folder, level) in levels {
            tree.nodes.push(TreeNodeData {
                url: Some("item".to_string()),
                is_folder: *is_folder,
                level: *level,
            });
        }
        tree
    }

    #[test]
    fn folders_and_indent_are_reported() {
        let tree = tree_with(&[(true, 0), (false, 1), (false, 2)]);

        assert_eq!(tree.node_count(), 3);
        assert!(tree.is_folder(0));
        assert!(!tree.is_folder(1));

        assert_eq!(tree.indent_of(0), 0.0);
        assert_eq!(tree.indent_of(1), 15.0);
        assert_eq!(tree.indent_of(2), 30.0);
    }

    #[test]
    fn out_of_range_index_is_safe() {
        let mut tree = tree_with(&[(true, 0)]);
        assert!(!tree.is_folder(9));
        assert_eq!(tree.indent_of(9), 0.0);
        assert!(tree.is_expanded(9), "非文件夹节点恒为展开");
        assert!(!tree.set_expanded(9, false));
    }

    #[test]
    fn collapsing_hides_descendants() {
        // 根文件夹下的两个子节点应当随折叠而隐藏。
        let mut tree = tree_with(&[(true, 0), (false, 1), (false, 1), (true, 0)]);
        assert_eq!(tree.visible_nodes(), vec![0, 1, 2, 3]);

        assert!(tree.set_expanded(0, false));
        assert_eq!(
            tree.visible_nodes(),
            vec![0, 3],
            "折叠后其下的层级更深节点应当隐藏，同级节点仍可见"
        );

        assert!(tree.set_expanded(0, true));
        assert_eq!(tree.visible_nodes().len(), 4);
    }

    #[test]
    fn toggling_flips_the_state() {
        let mut tree = tree_with(&[(true, 0), (false, 1)]);
        assert!(tree.is_expanded(0));

        assert!(tree.toggle_expanded(0));
        assert!(!tree.is_expanded(0));
        assert_eq!(tree.visible_nodes(), vec![0]);

        assert!(tree.toggle_expanded(0));
        assert_eq!(tree.visible_nodes().len(), 2);
    }

    #[test]
    fn toggling_a_leaf_does_nothing() {
        let mut tree = tree_with(&[(false, 0)]);
        assert!(!tree.toggle_expanded(0));
    }

    #[test]
    fn collapse_all_keeps_roots_visible() {
        // 顶层项不同于隐式根；收起后它本身保留，其全部后代隐藏。
        let mut tree = tree_with(&[(true, 0), (false, 1), (true, 2), (false, 3)]);
        tree.collapse_all();

        assert!(!tree.is_expanded(0), "顶层文件夹也要收起");
        assert!(!tree.is_expanded(2), "深层文件夹应当收起");

        let visible = tree.visible_nodes();
        assert!(visible.contains(&0));
        assert_eq!(visible, vec![0]);
        assert!(!visible.contains(&3), "被收起节点的子节点应当隐藏");
    }

    #[test]
    fn expand_to_opens_every_ancestor() {
        // 结构：root(0) → folder(1) → leaf(2)
        let mut tree = tree_with(&[(true, 0), (true, 1), (false, 2)]);
        tree.collapse_all();
        assert!(
            !tree.visible_nodes().contains(&2),
            "收起后深处的节点不应可见"
        );

        assert!(tree.expand_to(2));
        assert!(tree.is_expanded(1), "祖先文件夹应当被展开");
        assert!(tree.visible_nodes().contains(&2), "目标节点应当恢复可见");
    }

    #[test]
    fn expand_to_top_level_node_is_a_noop() {
        let mut tree = tree_with(&[(true, 0), (false, 1)]);
        assert!(!tree.expand_to(0), "顶层节点没有祖先可展开");
        assert!(!tree.expand_to(99), "越界应当安全返回");
    }

    #[test]
    fn expand_to_already_visible_node_reports_no_change() {
        let mut tree = tree_with(&[(true, 0), (false, 1)]);
        assert!(!tree.expand_to(1), "祖先本就展开时不应报告变化");
    }

    #[test]
    fn repeated_collapse_reports_no_change() {
        let mut tree = tree_with(&[(true, 0), (false, 1)]);
        assert!(tree.set_expanded(0, false));
        assert!(!tree.set_expanded(0, false), "重复收起不应再报告变化");
    }
}

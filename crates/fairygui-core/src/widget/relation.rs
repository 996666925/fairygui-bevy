//! 关联：对象与目标对象之间的位置与尺寸关系。
//!
//! 容器尺寸变化时，挂在子对象上的关联会把子对象的位置与尺寸相应调整，
//! 这是界面「自适应」的实现方式。描述格式很紧凑，有两点容易写错：
//!
//! - 每条定义的顺序是「类型字节 → 是否按比例（整字节布尔）」，不是反过来；
//! - 类型 `Size` 会被展开成 `Width` 与 `Height` 两条，顺序固定。

use crate::binary::{ByteBuffer, StringTable};
use crate::error::Result;
use crate::geom::Point;

/// 关联类型编号，取值与官方 `RelationType` 一致。
pub type RelationType = u8;

/// 宽度关联。
const WIDTH: RelationType = 14;
/// 高度关联。
const HEIGHT: RelationType = 15;
/// 同时关联宽高的简写。
const SIZE: RelationType = 24;

/// 一条关联定义。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationDef {
    /// 关联类型。
    pub relation_type: RelationType,
    /// 是否按比例计算。
    pub percent: bool,
    /// 作用轴：`0` 横轴，`1` 纵轴。
    pub axis: u8,
}

/// 针对某个目标的一组关联定义。
#[derive(Debug, Clone)]
pub struct RelationItem {
    /// 目标是否为父节点（描述里目标编号为 `-1`）。
    pub to_parent: bool,
    /// Component-level relations address the owner's children, rather than siblings.
    pub to_child: bool,
    /// 目标在容器内的位置。目标是父节点时为 `None`。
    ///
    /// 只在构建时解析；运行期通知使用 binding 中的稳定对象身份。
    pub target_index: Option<usize>,
    /// Resolved exactly once; a dead target never falls back to an index.
    pub(crate) binding: Option<RelationBinding>,
    /// 关联定义。
    pub defs: Vec<RelationDef>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RelationBinding {
    pub target: Option<crate::object::GObjectId>,
    pub track_position: bool,
    /// Position when subscribing; child extension relations retain this origin.
    pub initial_position: Point,
}

/// 一个对象上的全部关联。
#[derive(Debug, Clone, Default)]
pub struct Relations {
    /// 各目标对象的关联。
    pub items: Vec<RelationItem>,
}

/// 展开一条定义（`Size` 会展开成两条）。
fn push_def(defs: &mut Vec<RelationDef>, relation_type: RelationType, percent: bool) {
    if relation_type == SIZE {
        push_def(defs, WIDTH, percent);
        push_def(defs, HEIGHT, percent);
        return;
    }

    // 横轴：左侧与右侧系列，以及宽度；其余归纵轴。
    let axis = u8::from(
        !(relation_type <= 6 || relation_type == WIDTH || (16..=19).contains(&relation_type)),
    );

    defs.push(RelationDef {
        relation_type,
        percent,
        axis,
    });
}

impl Relations {
    /// 是否没有任何关联。
    pub fn is_empty(&self) -> bool {
        self.items.iter().all(|item| item.defs.is_empty())
    }

    /// 读取关联描述。
    ///
    /// `parent_to_child` 指出描述里的目标编号是相对自身（容器）还是相对父容器。
    pub(crate) fn setup(
        &mut self,
        buffer: &mut ByteBuffer<'_>,
        _table: &StringTable,
        parent_to_child: bool,
    ) -> Result<()> {
        let count = buffer.read_i8()?;
        for _ in 0..count {
            let target_index = buffer.read_i16()?;

            let item = if target_index == -1 {
                RelationItem {
                    to_parent: true,
                    to_child: false,
                    target_index: None,
                    binding: None,
                    defs: Vec::new(),
                }
            } else {
                RelationItem {
                    to_parent: false,
                    to_child: parent_to_child,
                    // 目标编号的含义取决于是谁的关联，这里只记录编号，
                    // 由调用方结合 `parent_to_child` 解释。
                    target_index: Some(target_index.max(0) as usize),
                    binding: None,
                    defs: Vec::new(),
                }
            };

            let mut item = item;
            let def_count = buffer.read_i8()?;
            for _ in 0..def_count {
                let relation_type = buffer.read_u8()?;
                let percent = buffer.read_bool()?;
                push_def(&mut item.defs, relation_type, percent);
            }

            self.items.push(item);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defs(relation_type: RelationType) -> Vec<RelationDef> {
        let mut out = Vec::new();
        push_def(&mut out, relation_type, false);
        out
    }

    #[test]
    fn size_expands_into_width_and_height_in_order() {
        let expanded = defs(SIZE);
        assert_eq!(expanded.len(), 2);
        assert_eq!(expanded[0].relation_type, WIDTH);
        assert_eq!(expanded[1].relation_type, HEIGHT);
        assert_eq!(expanded[0].axis, 0);
        assert_eq!(expanded[1].axis, 1);
    }

    #[test]
    fn axis_is_derived_from_type() {
        assert_eq!(defs(0)[0].axis, 0, "Left_Left 属于横轴");
        assert_eq!(defs(6)[0].axis, 0, "Right_Right 属于横轴");
        assert_eq!(defs(WIDTH)[0].axis, 0);
        assert_eq!(defs(7)[0].axis, 1, "Top_Top 属于纵轴");
        assert_eq!(defs(HEIGHT)[0].axis, 1);
    }
}

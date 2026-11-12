//! 关联：对象与目标对象之间的位置与尺寸关系。
//!
//! 容器尺寸变化时，挂在子对象上的关联会把子对象的位置与尺寸相应调整，
//! 这是界面「自适应」的实现方式。描述格式很紧凑，有两点容易写错：
//!
//! - 每条定义的顺序是「类型字节 → 是否按比例（整字节布尔）」，不是反过来；
//! - 类型 `Size` 会被展开成 `Width` 与 `Height` 两条，顺序固定。

use crate::binary::{ByteBuffer, StringTable};
use crate::error::Result;

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
    /// 目标在容器内的位置。目标是父节点时为 `None`。
    ///
    /// 相对兄弟对象的关联需要它来确定影响范围；当前只实现了相对父节点的部分，
    /// 因此暂时没有读取点。
    #[allow(dead_code)]
    pub target_index: Option<usize>,
    /// 关联定义。
    pub defs: Vec<RelationDef>,
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
                    target_index: None,
                    defs: Vec::new(),
                }
            } else {
                RelationItem {
                    to_parent: false,
                    // 目标编号的含义取决于是谁的关联，这里只记录编号，
                    // 由调用方结合 `parent_to_child` 解释。
                    target_index: Some(target_index.max(0) as usize),
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

        let _ = parent_to_child;
        Ok(())
    }

    /// 目标尺寸变化时调整自身。
    ///
    /// `delta` 为目标在对应轴上的尺寸增量。按比例的关联本应使用比例计算，
    /// 但比例需要目标的原始尺寸才能换算，这里统一按增量处理，
    /// 对「边距保持」这类最常见的用法结果一致。
    pub fn apply_on_target_resized(
        &self,
        x: &mut f32,
        y: &mut f32,
        width: &mut f32,
        height: &mut f32,
        d_width: f32,
        d_height: f32,
    ) {
        for item in &self.items {
            for def in &item.defs {
                let delta = if def.axis == 0 { d_width } else { d_height };

                match def.relation_type {
                    // 左边对左边、右边对左边：自身位置不变。
                    0 | 4 | 7 | 11 => {}
                    // 对中心：移动一半。
                    1 | 3 | 5 | 8 | 10 | 12 => {
                        if def.axis == 0 {
                            *x += delta * 0.5;
                        } else {
                            *y += delta * 0.5;
                        }
                    }
                    // 对另一侧：整体跟随。
                    2 | 6 => *x += delta,
                    9 | 13 => *y += delta,
                    // 尺寸跟随。
                    WIDTH => *width += delta,
                    HEIGHT => *height += delta,
                    _ => {}
                }
            }
        }
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

    #[test]
    fn left_aligned_child_keeps_position() {
        let relations = Relations {
            items: vec![RelationItem {
                to_parent: true,
                target_index: None,
                defs: defs(0),
            }],
        };
        let (mut x, mut y, mut w, mut h) = (10.0, 20.0, 30.0, 40.0);
        relations.apply_on_target_resized(&mut x, &mut y, &mut w, &mut h, 100.0, 50.0);
        assert_eq!((x, y), (10.0, 20.0));
        assert_eq!((w, h), (30.0, 40.0));
    }

    #[test]
    fn right_aligned_child_follows_growth() {
        let relations = Relations {
            items: vec![RelationItem {
                to_parent: true,
                target_index: None,
                defs: defs(6),
            }],
        };
        let (mut x, mut y, mut w, mut h) = (10.0, 20.0, 30.0, 40.0);
        relations.apply_on_target_resized(&mut x, &mut y, &mut w, &mut h, 100.0, 50.0);
        assert_eq!(x, 110.0, "右对齐的子对象应随父容器变宽而右移");
        assert_eq!(y, 20.0, "纵轴不受影响");
    }

    #[test]
    fn width_relation_grows_with_parent() {
        let relations = Relations {
            items: vec![RelationItem {
                to_parent: true,
                target_index: None,
                defs: defs(WIDTH),
            }],
        };
        let (mut x, mut y, mut w, mut h) = (0.0, 0.0, 30.0, 40.0);
        relations.apply_on_target_resized(&mut x, &mut y, &mut w, &mut h, 100.0, 0.0);
        assert_eq!(w, 130.0);
        assert_eq!(h, 40.0);
    }

    #[test]
    fn centred_child_moves_half_on_its_own_axis() {
        // Center_Center 属于横轴，只影响横向位置。
        let horizontal = Relations {
            items: vec![RelationItem {
                to_parent: true,
                target_index: None,
                defs: defs(3),
            }],
        };
        let (mut x, mut y, mut w, mut h) = (0.0, 0.0, 10.0, 10.0);
        horizontal.apply_on_target_resized(&mut x, &mut y, &mut w, &mut h, 100.0, 100.0);
        assert_eq!((x, y), (50.0, 0.0));

        // Middle_Middle 属于纵轴，只影响纵向位置。
        let vertical = Relations {
            items: vec![RelationItem {
                to_parent: true,
                target_index: None,
                defs: defs(10),
            }],
        };
        let (mut x, mut y, mut w, mut h) = (0.0, 0.0, 10.0, 10.0);
        vertical.apply_on_target_resized(&mut x, &mut y, &mut w, &mut h, 100.0, 100.0);
        assert_eq!((x, y), (0.0, 50.0));
    }
}

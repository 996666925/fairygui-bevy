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
    /// 相对兄弟对象的关联用它匹配发生尺寸变化的目标。
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

/// Geometry needed to apply one target resize to its related objects.
#[derive(Debug, Clone, Copy, Default)]
pub struct RelationTarget {
    pub old_size: Point,
    pub new_size: Point,
    pub position: Point,
    pub pivot: Point,
    pub pivot_as_anchor: bool,
    pub is_parent: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RelationMove {
    pub delta: Point,
    pub owner_pivot: Point,
    pub owner_pivot_as_anchor: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RelationResize {
    pub delta: Point,
    scale: Point,
    pub old_size: Point,
    pub new_size: Point,
    pub target_position: Point,
    pub target_pivot: Point,
    pub target_pivot_as_anchor: bool,
    pub target_is_parent: bool,
    pub owner_pivot: Point,
    pub owner_pivot_as_anchor: bool,
}

impl RelationResize {
    pub fn new(target: RelationTarget, owner_pivot: Point, owner_pivot_as_anchor: bool) -> Self {
        let ratio = |new: f32, old: f32| {
            if old == 0.0 { 0.0 } else { new / old }
        };
        Self {
            delta: Point::new(
                target.new_size.x - target.old_size.x,
                target.new_size.y - target.old_size.y,
            ),
            scale: Point::ZERO,
            old_size: target.old_size,
            new_size: target.new_size,
            target_position: target.position,
            target_pivot: target.pivot,
            target_pivot_as_anchor: target.pivot_as_anchor,
            target_is_parent: target.is_parent,
            owner_pivot,
            owner_pivot_as_anchor,
        }
        .with_scale(
            ratio(target.new_size.x, target.old_size.x),
            ratio(target.new_size.y, target.old_size.y),
        )
    }

    fn with_scale(mut self, x: f32, y: f32) -> Self {
        self.scale = Point::new(x, y);
        self
    }
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
                    defs: Vec::new(),
                }
            } else {
                RelationItem {
                    to_parent: false,
                    to_child: parent_to_child,
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

        Ok(())
    }

    /// 目标尺寸变化时调整自身。
    ///
    /// Compatibility helper for increment-based relation tests and callers.
    /// Runtime resize propagation supplies the full target geometry so percent
    /// relations can use the native target-size ratio.
    #[cfg(test)]
    pub fn apply_on_target_resized(
        &self,
        x: &mut f32,
        y: &mut f32,
        width: &mut f32,
        height: &mut f32,
        d_width: f32,
        d_height: f32,
    ) {
        self.apply_on_target_resized_with_context(
            x,
            y,
            width,
            height,
            RelationResize {
                delta: Point::new(d_width, d_height),
                scale: Point::new(1.0, 1.0),
                ..RelationResize::default()
            },
        );
    }

    pub fn apply_on_target_resized_with_context(
        &self,
        x: &mut f32,
        y: &mut f32,
        width: &mut f32,
        height: &mut f32,
        resize: RelationResize,
    ) {
        for item in &self.items {
            for def in &item.defs {
                let delta = if def.axis == 0 {
                    resize.delta.x
                } else {
                    resize.delta.y
                };

                if def.percent {
                    let scale = if def.axis == 0 {
                        resize.scale.x
                    } else {
                        resize.scale.y
                    };
                    let target_pos = if resize.target_is_parent {
                        0.0
                    } else if def.axis == 0 {
                        resize.target_position.x
                    } else {
                        resize.target_position.y
                    };
                    let owner_pivot = if resize.owner_pivot_as_anchor {
                        if def.axis == 0 {
                            resize.owner_pivot.x
                        } else {
                            resize.owner_pivot.y
                        }
                    } else {
                        0.0
                    };

                    match def.relation_type {
                        0..=6 if def.axis == 0 => {
                            let owner_size = *width;
                            let min = *x - owner_size * owner_pivot;
                            let next = match def.relation_type {
                                0..=2 => target_pos + (min - target_pos) * scale,
                                3 => {
                                    target_pos + (min + owner_size * 0.5 - target_pos) * scale
                                        - owner_size * 0.5
                                }
                                _ => {
                                    target_pos + (min + owner_size - target_pos) * scale
                                        - owner_size
                                }
                            };
                            *x = next + owner_size * owner_pivot;
                            continue;
                        }
                        7..=13 if def.axis == 1 => {
                            let owner_size = *height;
                            let min = *y - owner_size * owner_pivot;
                            let next = match def.relation_type {
                                7..=9 => target_pos + (min - target_pos) * scale,
                                10 => {
                                    target_pos + (min + owner_size * 0.5 - target_pos) * scale
                                        - owner_size * 0.5
                                }
                                _ => {
                                    target_pos + (min + owner_size - target_pos) * scale
                                        - owner_size
                                }
                            };
                            *y = next + owner_size * owner_pivot;
                            continue;
                        }
                        WIDTH => {
                            let old_width = *width;
                            *width = resize.new_size.x + (*width - resize.old_size.x) * scale;
                            if resize.target_is_parent && resize.owner_pivot_as_anchor {
                                *x += (*width - old_width) * resize.owner_pivot.x;
                            } else if !resize.target_is_parent && !resize.owner_pivot_as_anchor {
                                *x -= (*width - old_width) * resize.owner_pivot.x;
                            }
                            continue;
                        }
                        HEIGHT => {
                            let old_height = *height;
                            *height = resize.new_size.y + (*height - resize.old_size.y) * scale;
                            if resize.target_is_parent && resize.owner_pivot_as_anchor {
                                *y += (*height - old_height) * resize.owner_pivot.y;
                            } else if !resize.target_is_parent && !resize.owner_pivot_as_anchor {
                                *y -= (*height - old_height) * resize.owner_pivot.y;
                            }
                            continue;
                        }
                        16..=17 => {
                            let min = *x - *width * owner_pivot;
                            let shift = target_pos + (min - target_pos) * scale - min;
                            *width -= shift;
                            *x = min + shift + *width * owner_pivot;
                            continue;
                        }
                        18..=19 => {
                            let min = *x - *width * owner_pivot;
                            let edge = min + *width;
                            *width += target_pos + edge * scale - edge;
                            *x = min + *width * owner_pivot;
                            continue;
                        }
                        20..=21 => {
                            let min = *y - *height * owner_pivot;
                            let shift = target_pos + (min - target_pos) * scale - min;
                            *height -= shift;
                            *y = min + shift + *height * owner_pivot;
                            continue;
                        }
                        22..=23 => {
                            let min = *y - *height * owner_pivot;
                            let edge = min + *height;
                            *height += target_pos + edge * scale - edge;
                            *y = min + *height * owner_pivot;
                            continue;
                        }
                        _ => continue,
                    }
                }

                match def.relation_type {
                    0 | 4 => {
                        if !resize.target_is_parent && resize.target_pivot_as_anchor {
                            *x -= delta * resize.target_pivot.x;
                        }
                    }
                    7 | 11 => {
                        if !resize.target_is_parent && resize.target_pivot_as_anchor {
                            *y -= delta * resize.target_pivot.y;
                        }
                    }
                    // 对中心：移动一半。
                    1 | 3 | 5 | 8 | 10 | 12 => {
                        let pivot = if !resize.target_is_parent && resize.target_pivot_as_anchor {
                            if def.axis == 0 {
                                resize.target_pivot.x
                            } else {
                                resize.target_pivot.y
                            }
                        } else {
                            0.0
                        };
                        if def.axis == 0 {
                            *x += delta * (0.5 - pivot);
                        } else {
                            *y += delta * (0.5 - pivot);
                        }
                    }
                    // 对另一侧：整体跟随。
                    2 | 6 => {
                        let pivot = if !resize.target_is_parent && resize.target_pivot_as_anchor {
                            resize.target_pivot.x
                        } else {
                            0.0
                        };
                        *x += delta * (1.0 - pivot);
                    }
                    9 | 13 => {
                        let pivot = if !resize.target_is_parent && resize.target_pivot_as_anchor {
                            resize.target_pivot.y
                        } else {
                            0.0
                        };
                        *y += delta * (1.0 - pivot);
                    }
                    WIDTH => {
                        *width += delta;
                        if resize.target_is_parent && resize.owner_pivot_as_anchor {
                            *x += resize.owner_pivot.x * delta;
                        } else if !resize.target_is_parent && !resize.owner_pivot_as_anchor {
                            *x -= resize.owner_pivot.x * delta;
                        }
                    }
                    HEIGHT => {
                        *height += delta;
                        if resize.target_is_parent && resize.owner_pivot_as_anchor {
                            *y += resize.owner_pivot.y * delta;
                        } else if !resize.target_is_parent && !resize.owner_pivot_as_anchor {
                            *y -= resize.owner_pivot.y * delta;
                        }
                    }
                    16..=17 => {
                        let pivot = if !resize.target_is_parent && resize.target_pivot_as_anchor {
                            resize.target_pivot.x
                        } else {
                            0.0
                        };
                        let owner_pivot = if resize.owner_pivot_as_anchor {
                            resize.owner_pivot.x
                        } else {
                            0.0
                        };
                        let min = *x - *width * owner_pivot;
                        let factor = if def.relation_type == 16 {
                            -pivot
                        } else {
                            1.0 - pivot
                        };
                        let shift = delta * factor;
                        *width -= shift;
                        *x = min + shift + *width * owner_pivot;
                    }
                    18..=19 => {
                        let pivot = if !resize.target_is_parent && resize.target_pivot_as_anchor {
                            resize.target_pivot.x
                        } else {
                            0.0
                        };
                        let owner_pivot = if resize.owner_pivot_as_anchor {
                            resize.owner_pivot.x
                        } else {
                            0.0
                        };
                        let min = *x - *width * owner_pivot;
                        let factor = if def.relation_type == 18 {
                            -pivot
                        } else {
                            1.0 - pivot
                        };
                        *width += delta * factor;
                        *x = min + *width * owner_pivot;
                    }
                    20..=21 => {
                        let pivot = if !resize.target_is_parent && resize.target_pivot_as_anchor {
                            resize.target_pivot.y
                        } else {
                            0.0
                        };
                        let owner_pivot = if resize.owner_pivot_as_anchor {
                            resize.owner_pivot.y
                        } else {
                            0.0
                        };
                        let min = *y - *height * owner_pivot;
                        let factor = if def.relation_type == 20 {
                            -pivot
                        } else {
                            1.0 - pivot
                        };
                        let shift = delta * factor;
                        *height -= shift;
                        *y = min + shift + *height * owner_pivot;
                    }
                    22..=23 => {
                        let pivot = if !resize.target_is_parent && resize.target_pivot_as_anchor {
                            resize.target_pivot.y
                        } else {
                            0.0
                        };
                        let owner_pivot = if resize.owner_pivot_as_anchor {
                            resize.owner_pivot.y
                        } else {
                            0.0
                        };
                        let min = *y - *height * owner_pivot;
                        let factor = if def.relation_type == 22 {
                            -pivot
                        } else {
                            1.0 - pivot
                        };
                        *height += delta * factor;
                        *y = min + *height * owner_pivot;
                    }
                    _ => {}
                }
            }
        }
    }

    /// Move an object whose sibling relations refer to this target.
    /// Position relations follow the target on their axis; extension relations
    /// keep the opposite edge fixed while resizing around the moved edge.
    pub fn apply_on_target_moved(
        &self,
        x: &mut f32,
        y: &mut f32,
        width: &mut f32,
        height: &mut f32,
        movement: RelationMove,
    ) {
        let dx = movement.delta.x;
        let dy = movement.delta.y;
        let pivot_x = if movement.owner_pivot_as_anchor {
            movement.owner_pivot.x
        } else {
            0.0
        };
        let pivot_y = if movement.owner_pivot_as_anchor {
            movement.owner_pivot.y
        } else {
            0.0
        };
        for item in &self.items {
            if item.to_parent {
                continue;
            }
            for def in &item.defs {
                match def.relation_type {
                    0..=6 => *x += dx,
                    7..=13 => *y += dy,
                    16..=17 => {
                        let min = *x - *width * pivot_x;
                        *width -= dx;
                        *x = min + dx + *width * pivot_x;
                    }
                    18..=19 => {
                        let min = *x - *width * pivot_x;
                        *width += dx;
                        *x = min + *width * pivot_x;
                    }
                    20..=21 => {
                        let min = *y - *height * pivot_y;
                        *height -= dy;
                        *y = min + dy + *height * pivot_y;
                    }
                    22..=23 => {
                        let min = *y - *height * pivot_y;
                        *height += dy;
                        *y = min + *height * pivot_y;
                    }
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
                to_child: false,
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
                to_child: false,
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
                to_child: false,
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
                to_child: false,
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
                to_child: false,
                target_index: None,
                defs: defs(10),
            }],
        };
        let (mut x, mut y, mut w, mut h) = (0.0, 0.0, 10.0, 10.0);
        vertical.apply_on_target_resized(&mut x, &mut y, &mut w, &mut h, 100.0, 100.0);
        assert_eq!((x, y), (0.0, 50.0));
    }

    #[test]
    fn percent_position_relations_scale_edges_and_centers_from_parent_origin() {
        let resize = RelationResize::new(
            RelationTarget {
                old_size: Point::new(100.0, 100.0),
                new_size: Point::new(200.0, 200.0),
                is_parent: true,
                ..RelationTarget::default()
            },
            Point::ZERO,
            false,
        );
        for (kind, expected_x) in [(0, 20.0), (3, 30.0), (6, 40.0)] {
            let relations = Relations {
                items: vec![RelationItem {
                    to_parent: true,
                    to_child: false,
                    target_index: None,
                    defs: vec![RelationDef {
                        relation_type: kind,
                        percent: true,
                        axis: 0,
                    }],
                }],
            };
            let (mut x, mut y, mut width, mut height) = (10.0, 20.0, 20.0, 10.0);
            relations.apply_on_target_resized_with_context(
                &mut x,
                &mut y,
                &mut width,
                &mut height,
                resize,
            );
            assert_eq!(x, expected_x, "relation type {kind}");
            assert_eq!(y, 20.0);
            assert_eq!(width, 20.0);
        }
    }

    #[test]
    fn percent_size_and_extension_relations_follow_target_scale() {
        for (relation_type, axis, initial, expected) in [
            (WIDTH, 0, (10.0, 20.0, 40.0, 20.0), (10.0, 20.0, 80.0, 20.0)),
            (
                HEIGHT,
                1,
                (10.0, 20.0, 20.0, 40.0),
                (10.0, 20.0, 20.0, 80.0),
            ),
            (16, 0, (10.0, 20.0, 40.0, 20.0), (20.0, 20.0, 30.0, 20.0)),
            (18, 0, (10.0, 20.0, 40.0, 20.0), (10.0, 20.0, 90.0, 20.0)),
        ] {
            let relations = Relations {
                items: vec![RelationItem {
                    to_parent: true,
                    to_child: false,
                    target_index: None,
                    defs: vec![RelationDef {
                        relation_type,
                        percent: true,
                        axis,
                    }],
                }],
            };
            let (mut x, mut y, mut width, mut height) = initial;
            relations.apply_on_target_resized_with_context(
                &mut x,
                &mut y,
                &mut width,
                &mut height,
                RelationResize::new(
                    RelationTarget {
                        old_size: Point::new(100.0, 100.0),
                        new_size: Point::new(200.0, 200.0),
                        is_parent: true,
                        ..RelationTarget::default()
                    },
                    Point::ZERO,
                    false,
                ),
            );
            assert_eq!(
                (x, y, width, height),
                expected,
                "relation type {relation_type}"
            );
        }
    }

    #[test]
    fn percent_sibling_relation_scales_from_the_sibling_position() {
        let relations = Relations {
            items: vec![RelationItem {
                to_parent: false,
                to_child: false,
                target_index: Some(0),
                defs: vec![RelationDef {
                    relation_type: 0,
                    percent: true,
                    axis: 0,
                }],
            }],
        };
        let (mut x, mut y, mut width, mut height) = (60.0, 15.0, 20.0, 10.0);
        relations.apply_on_target_resized_with_context(
            &mut x,
            &mut y,
            &mut width,
            &mut height,
            RelationResize::new(
                RelationTarget {
                    old_size: Point::new(20.0, 20.0),
                    new_size: Point::new(40.0, 40.0),
                    position: Point::new(50.0, 30.0),
                    ..RelationTarget::default()
                },
                Point::ZERO,
                false,
            ),
        );
        assert_eq!(x, 70.0, "distance from sibling left edge doubles");
        assert_eq!(y, 15.0);
    }

    #[test]
    fn nonpercent_sibling_alignment_uses_target_pivot_as_anchor() {
        for (kind, expected_x) in [(0, 40.0), (1, 50.0), (2, 60.0)] {
            let relations = Relations {
                items: vec![RelationItem {
                    to_parent: false,
                    to_child: false,
                    target_index: Some(0),
                    defs: vec![RelationDef {
                        relation_type: kind,
                        percent: false,
                        axis: 0,
                    }],
                }],
            };
            let (mut x, mut y, mut width, mut height) = (50.0, 10.0, 10.0, 10.0);
            relations.apply_on_target_resized_with_context(
                &mut x,
                &mut y,
                &mut width,
                &mut height,
                RelationResize::new(
                    RelationTarget {
                        old_size: Point::new(20.0, 20.0),
                        new_size: Point::new(40.0, 40.0),
                        pivot: Point::new(0.5, 0.5),
                        pivot_as_anchor: true,
                        ..RelationTarget::default()
                    },
                    Point::ZERO,
                    false,
                ),
            );
            assert_eq!(x, expected_x, "relation type {kind}");
            assert_eq!(y, 10.0);
        }
    }

    #[test]
    fn extension_relations_use_owner_anchor_coordinates() {
        let relations = Relations {
            items: vec![RelationItem {
                to_parent: false,
                to_child: false,
                target_index: Some(0),
                defs: vec![RelationDef {
                    relation_type: 17,
                    percent: false,
                    axis: 0,
                }],
            }],
        };
        let (mut x, mut y, mut width, mut height) = (10.0, 20.0, 40.0, 10.0);
        relations.apply_on_target_resized_with_context(
            &mut x,
            &mut y,
            &mut width,
            &mut height,
            RelationResize::new(
                RelationTarget {
                    old_size: Point::new(20.0, 20.0),
                    new_size: Point::new(40.0, 40.0),
                    pivot: Point::new(0.5, 0.0),
                    pivot_as_anchor: true,
                    ..RelationTarget::default()
                },
                Point::new(0.5, 0.0),
                true,
            ),
        );
        assert_eq!(width, 30.0);
        assert_eq!(x, 15.0, "xMin follows the sibling's right edge");
        assert_eq!(y, 20.0);
    }
}

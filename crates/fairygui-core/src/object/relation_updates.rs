//! Apply each definition through native setters so clamping, pivots and
//! intermediate notifications use the same geometry as the original runtime.
use super::*;
use crate::widget::relation::{RelationBinding, RelationDef};

impl ObjectStore {
    fn relation_axis(&self, owner: GObjectId, horizontal: bool) -> (f32, f32, f32) {
        let o = &self.objects[owner];
        let raw = o.requested_size();
        let (position, size, raw, pivot) = if horizontal {
            (o.x, o.width, raw.width, o.pivot_x)
        } else {
            (o.y, o.height, raw.height, o.pivot_y)
        };
        (
            position,
            position - if o.pivot_as_anchor { size * pivot } else { 0. },
            raw,
        )
    }

    fn set_relation_min(&mut self, owner: GObjectId, horizontal: bool, min: f32) {
        let o = &self.objects[owner];
        let pivot_offset = if o.pivot_as_anchor {
            if horizontal {
                o.width * o.pivot_x
            } else {
                o.height * o.pivot_y
            }
        } else {
            0.
        };
        let (x, y) = if horizontal {
            (min + pivot_offset, o.y)
        } else {
            (o.x, min + pivot_offset)
        };
        self.set_xy(owner, x, y);
    }

    fn set_relation_size(
        &mut self,
        owner: GObjectId,
        horizontal: bool,
        extent: f32,
        ignore_pivot: bool,
    ) {
        let raw = self.objects[owner].requested_size();
        self.set_size_internal(
            owner,
            if horizontal { extent } else { raw.width },
            if horizontal { raw.height } else { extent },
            ignore_pivot,
            false,
        );
    }

    pub(super) fn apply_relation_move(
        &mut self,
        owner: GObjectId,
        target: GObjectId,
        def: &RelationDef,
        delta: Point,
    ) {
        let horizontal = def.axis == 0;
        let (position, min, raw) = self.relation_axis(owner, horizontal);
        let delta = if horizontal { delta.x } else { delta.y };
        match def.relation_type {
            0..=13 => {
                let o = &self.objects[owner];
                let (x, y) = if horizontal {
                    (position + delta, o.y)
                } else {
                    (o.x, position + delta)
                };
                self.set_xy(owner, x, y);
            }
            16..=23 => {
                let leading = matches!(def.relation_type, 16 | 17 | 20 | 21);
                self.set_relation_size(
                    owner,
                    horizontal,
                    raw + if leading { -delta } else { delta },
                    false,
                );
                // A component following its child's extension changes only its
                // size. Its own pivot/self relations still run in the setter.
                if self.objects[target].parent != Some(owner) {
                    self.set_relation_min(
                        owner,
                        horizontal,
                        min + if leading { delta } else { 0. },
                    );
                }
            }
            _ => {}
        }
    }

    pub(super) fn apply_relation_resize(
        &mut self,
        owner: GObjectId,
        target_id: GObjectId,
        binding: RelationBinding,
        def: &RelationDef,
        old_target_size: Size,
    ) {
        let horizontal = def.axis == 0;
        let target = &self.objects[target_id];
        let owner_object = &self.objects[owner];
        let to_parent = owner_object.parent == Some(target_id);
        let to_child = target.parent == Some(owner);
        let (position, min, raw) = self.relation_axis(owner, horizontal);
        let (
            target_position,
            target_pivot,
            target_size,
            old_size,
            init_size,
            source_size,
            init_position,
        ) = if horizontal {
            (
                target.x,
                target.pivot_x,
                target.width,
                old_target_size.width,
                target.init_width,
                owner_object.source_width,
                binding.initial_position.x,
            )
        } else {
            (
                target.y,
                target.pivot_y,
                target.height,
                old_target_size.height,
                target.init_height,
                owner_object.source_height,
                binding.initial_position.y,
            )
        };
        let pos = if to_parent { 0. } else { target_position };
        let pivot = if !to_parent && target.pivot_as_anchor {
            target_pivot
        } else {
            0.
        };
        let delta = target_size - old_size;
        let ratio = if old_size == 0. {
            0.
        } else {
            target_size / old_size
        };
        let kind = match def.relation_type {
            0..=6 => def.relation_type,
            7..=13 => def.relation_type - 7,
            14..=15 => 7,
            16..=19 => def.relation_type - 8,
            20..=23 => def.relation_type - 12,
            _ => return,
        };
        match kind {
            0..=6 => {
                if def.percent {
                    let edge = match kind {
                        0..=2 => 0.,
                        3 => raw * 0.5,
                        _ => raw,
                    };
                    self.set_relation_min(
                        owner,
                        horizontal,
                        pos + (min + edge - pos) * ratio - edge,
                    );
                } else {
                    let side = match kind {
                        0 | 4 => 0.,
                        1 | 3 | 5 => 0.5,
                        _ => 1.,
                    };
                    let o = &self.objects[owner];
                    let next = position + delta * (side - pivot);
                    let (x, y) = if horizontal { (next, o.y) } else { (o.x, next) };
                    self.set_xy(owner, x, y);
                }
            }
            7 => {
                let offset = if to_child {
                    source_size - init_size
                } else {
                    raw - old_size
                };
                let extent = target_size + if def.percent { offset * ratio } else { offset };
                let preserve_min = to_parent && owner_object.pivot_as_anchor;
                self.set_relation_size(owner, horizontal, extent, to_parent);
                if preserve_min {
                    self.set_relation_min(owner, horizontal, min);
                }
            }
            8..=11 => {
                if kind == 11 && to_child {
                    let extent = if def.percent {
                        pos + target_size * (1. - pivot)
                            + (source_size - init_position - init_size * (1. - pivot)) * ratio
                    } else {
                        source_size + pos - init_position + (target_size - init_size) * (1. - pivot)
                    };
                    self.set_relation_size(owner, horizontal, extent, false);
                } else {
                    let leading = kind <= 9;
                    let edge = if leading { min } else { min + raw };
                    let shift = if def.percent {
                        pos + (edge - pos) * ratio - edge
                    } else {
                        delta
                            * (if kind == 8 || kind == 10 {
                                -pivot
                            } else {
                                1. - pivot
                            })
                    };
                    self.set_relation_size(
                        owner,
                        horizontal,
                        raw + if leading { -shift } else { shift },
                        false,
                    );
                    self.set_relation_min(
                        owner,
                        horizontal,
                        min + if leading { shift } else { 0. },
                    );
                }
            }
            _ => unreachable!(),
        }
    }
}

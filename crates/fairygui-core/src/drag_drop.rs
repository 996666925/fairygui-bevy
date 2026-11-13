//! 拖拽。
//!
//! 一次拖拽由三部分组成：被拖拽的对象、跟手的图标，以及松手时命中的放置目标。
//! 核心层只维护这些状态与几何；命中检测交给上层，因为只有它知道指针位置与
//! 界面布局。

use crate::geom::Point;
use crate::geom::Rect;
use crate::object::{GObjectId, ObjectStore};

/// Native GObject movement session, distinct from a drag-and-drop proxy. Begins
/// at threshold crossing so the initial sensitivity displacement is discarded.
#[derive(Debug, Clone)]
pub struct ObjectDrag {
    owner: GObjectId,
    parent: GObjectId,
    pointer_start: Point,
    global_rect: Rect,
}
impl ObjectDrag {
    pub fn begin(store: &ObjectStore, owner: GObjectId, pointer: Point) -> Option<Self> {
        if !pointer.x.is_finite() || !pointer.y.is_finite() {
            return None;
        }
        let object = store.object(owner)?;
        let parent = object.parent()?;
        store.global_to_local(parent, pointer)?;
        Some(Self {
            owner,
            parent,
            pointer_start: pointer,
            global_rect: store
                .local_to_global_rect(owner, Rect::new(0., 0., object.width(), object.height()))?,
        })
    }
    pub fn parent(&self) -> GObjectId {
        self.parent
    }

    /// Bounds are stage-local, as in GObject.dragBounds. Oversized controls pin
    /// to the bounds origin. Live parent transforms are used before JS rounding.
    pub fn update(
        &self,
        store: &mut ObjectStore,
        pointer: Point,
        bounds: Option<Rect>,
        stage: Option<GObjectId>,
    ) -> bool {
        if !pointer.x.is_finite()
            || !pointer.y.is_finite()
            || store
                .object(self.owner)
                .is_none_or(|object| object.parent() != Some(self.parent))
        {
            return false;
        }
        let mut x = pointer.x - self.pointer_start.x + self.global_rect.x;
        let mut y = pointer.y - self.pointer_start.y + self.global_rect.y;
        if let Some(bounds) = bounds {
            let bounds = if let Some(stage) = stage {
                let Some(bounds) = store.local_to_global_rect(stage, bounds) else {
                    return false;
                };
                bounds
            } else {
                bounds
            };
            if ![bounds.x, bounds.y, bounds.width, bounds.height]
                .iter()
                .all(|v| v.is_finite())
            {
                return false;
            }
            if x < bounds.x {
                x = bounds.x;
            } else if x + self.global_rect.width > bounds.right() {
                x = (bounds.right() - self.global_rect.width).max(bounds.x);
            }
            if y < bounds.y {
                y = bounds.y;
            } else if y + self.global_rect.height > bounds.bottom() {
                y = (bounds.bottom() - self.global_rect.height).max(bounds.y);
            }
        }
        let Some(local) = store.global_to_local(self.parent, Point::new(x, y)) else {
            return false;
        };
        // Math.round(-0.5) is zero, unlike Rust's round-away-from-zero.
        store.set_xy(self.owner, (local.x + 0.5).floor(), (local.y + 0.5).floor());
        true
    }
}

/// 一次拖拽的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DragResult {
    /// 被拖拽的对象。
    pub source: GObjectId,
    /// 松手时命中的放置目标。没有命中时为 `None`。
    pub target: Option<GObjectId>,
}

/// 拖拽状态。
#[derive(Debug, Clone, Default)]
pub struct DragDrop {
    active: bool,
    source: Option<GObjectId>,
    icon: Option<GObjectId>,
    data: Option<String>,
    pointer: Point,
    target: Option<GObjectId>,
}

impl DragDrop {
    /// 空闲状态。
    pub fn new() -> Self {
        Self::default()
    }

    /// 是否正在拖拽。
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// 被拖拽的对象。
    pub fn source(&self) -> Option<GObjectId> {
        self.source
    }

    /// 跟手的图标。
    pub fn icon(&self) -> Option<GObjectId> {
        self.icon
    }

    /// 拖拽携带的数据。
    pub fn data(&self) -> Option<&str> {
        self.data.as_deref()
    }

    /// 当前指针位置。
    pub fn pointer(&self) -> Point {
        self.pointer
    }

    /// 当前命中的放置目标。
    pub fn target(&self) -> Option<GObjectId> {
        self.target
    }

    /// 开始拖拽。
    ///
    /// 若上一次拖拽还没结束，会先把它清理掉，避免状态叠加。
    pub fn start(
        &mut self,
        store: &mut ObjectStore,
        source: GObjectId,
        icon: Option<GObjectId>,
        data: Option<String>,
    ) {
        if self.active {
            self.clear(store);
        }

        self.active = true;
        self.source = Some(source);
        self.icon = icon;
        self.data = data;
        self.target = None;

        if let Some(icon) = icon {
            store.set_visible(icon, true);
        }
    }

    /// 更新指针位置，并把图标移到指针处。
    pub fn update(&mut self, store: &mut ObjectStore, pointer: Point) {
        if !self.active {
            return;
        }
        self.pointer = pointer;

        if let Some(icon) = self.icon {
            store.set_xy(icon, pointer.x, pointer.y);
        }
    }

    /// 记录当前命中的放置目标。未在拖拽中时忽略。
    pub fn set_target(&mut self, target: Option<GObjectId>) {
        if self.active {
            self.target = target;
        }
    }

    /// 结束拖拽并返回结果。未在拖拽中时返回 `None`。
    pub fn finish(&mut self, store: &mut ObjectStore) -> Option<DragResult> {
        if !self.active {
            return None;
        }

        let result = DragResult {
            source: self.source?,
            target: self.target,
        };
        self.clear(store);
        Some(result)
    }

    /// 取消拖拽，不产生结果。
    pub fn cancel(&mut self, store: &mut ObjectStore) {
        if self.active {
            self.clear(store);
        }
    }

    fn clear(&mut self, store: &mut ObjectStore) {
        if let Some(icon) = self.icon {
            store.set_visible(icon, false);
        }
        self.active = false;
        self.source = None;
        self.icon = None;
        self.data = None;
        self.target = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::ObjectType;

    #[test]
    fn object_drag_discards_sensitivity_motion_rounds_like_js_and_clamps_oversized_bounds() {
        let mut store = ObjectStore::new();
        let parent = store.create(ObjectType::Component);
        store.set_xy(parent, 50., 70.);
        let owner = store.create(ObjectType::Component);
        store.set_parent(owner, Some(parent));
        store.set_size(owner, 20., 30.);
        store.set_xy(owner, 10., 20.);
        let drag = ObjectDrag::begin(&store, owner, Point::new(120., 130.)).unwrap();
        assert!(drag.update(&mut store, Point::new(120., 130.), None, None));
        assert_eq!(
            store.object(owner).unwrap().position(),
            Point::new(10., 20.)
        );
        assert!(drag.update(&mut store, Point::new(109.5, 109.5), None, None));
        assert_eq!(store.object(owner).unwrap().position(), Point::new(0., 0.));
        let bounds = Rect::new(80., 100., 10., 10.);
        assert!(drag.update(&mut store, Point::new(1000., 1000.), Some(bounds), None));
        assert_eq!(
            store.object(owner).unwrap().position(),
            Point::new(30., 30.)
        );
        assert!(drag.update(&mut store, Point::new(-1000., -1000.), Some(bounds), None));
        assert_eq!(
            store.object(owner).unwrap().position(),
            Point::new(30., 30.)
        );
    }

    #[test]
    fn native_coordinate_conversion_preserves_anchor_skew_reflection_and_two_corner_rectangles() {
        let mut store = ObjectStore::new();
        let parent = store.create(ObjectType::Component);
        let owner = store.create(ObjectType::Component);
        store.set_parent(owner, Some(parent));
        store.set_xy(parent, 40., 70.);
        store.set_size(parent, 200., 100.);
        store.set_pivot(parent, 0.5, 0.5, true);
        store.set_scale(parent, -2., 1.5);
        store.set_rotation(parent, 35.);
        store.set_skew(parent, 8., -11.);
        store.set_xy(owner, 15., 20.);
        store.set_size(owner, 40., 30.);
        store.set_pivot(owner, 0.5, 0.5, true);
        let point = Point::new(9., -12.);
        let global = store.local_to_global(owner, point).unwrap();
        let local = store.global_to_local(owner, global).unwrap();
        assert!((local.x - point.x).abs() < 0.0001 && (local.y - point.y).abs() < 0.0001);
        let rect = store
            .local_to_global_rect(owner, Rect::new(0., 0., 40., 30.))
            .unwrap();
        assert!(
            rect.width < 0.,
            "native conversion retains reflected dimensions"
        );
        let origin = store.local_to_global(owner, Point::ZERO).unwrap();
        assert_eq!(rect.position(), origin);
        let drag = ObjectDrag::begin(&store, owner, global).unwrap();
        let desired = store.local_to_global(parent, Point::new(30., 45.)).unwrap();
        assert!(drag.update(
            &mut store,
            Point::new(
                global.x + desired.x - origin.x,
                global.y + desired.y - origin.y
            ),
            None,
            None
        ));
        assert_eq!(
            store.object(owner).unwrap().position(),
            Point::new(30., 45.)
        );
        store.set_scale(parent, 0., 1.);
        assert!(store.global_to_local(parent, global).is_none());
        assert!(!drag.update(&mut store, global, None, None));
    }

    fn setup() -> (ObjectStore, GObjectId, DragDrop) {
        let mut store = ObjectStore::new();
        let source = store.create(ObjectType::Component);
        (store, source, DragDrop::new())
    }

    #[test]
    fn start_activates_and_reveals_the_icon() {
        let (mut store, source, mut drag) = setup();
        let icon = store.create(ObjectType::Image);
        store.set_visible(icon, false);

        assert!(!drag.is_active());
        drag.start(&mut store, source, Some(icon), Some("payload".to_string()));

        assert!(drag.is_active());
        assert_eq!(drag.source(), Some(source));
        assert_eq!(drag.data(), Some("payload"));
        assert!(store.object(icon).unwrap().visible(), "图标应当显示出来");
    }

    #[test]
    fn update_moves_the_icon_to_the_pointer() {
        let (mut store, source, mut drag) = setup();
        let icon = store.create(ObjectType::Image);
        drag.start(&mut store, source, Some(icon), None);

        drag.update(&mut store, Point::new(120.0, 45.0));

        assert_eq!(drag.pointer(), Point::new(120.0, 45.0));
        assert_eq!(
            store.object(icon).unwrap().position(),
            Point::new(120.0, 45.0)
        );
    }

    #[test]
    fn finish_returns_source_and_target_then_cleans_up() {
        let (mut store, source, mut drag) = setup();
        let icon = store.create(ObjectType::Image);
        let target = store.create(ObjectType::Component);

        drag.start(&mut store, source, Some(icon), None);
        drag.update(&mut store, Point::new(10.0, 10.0));
        drag.set_target(Some(target));

        let result = drag.finish(&mut store).expect("应当返回结果");
        assert_eq!(result.source, source);
        assert_eq!(result.target, Some(target));

        assert!(!drag.is_active());
        assert!(!store.object(icon).unwrap().visible(), "结束后图标应当隐藏");
        assert!(drag.finish(&mut store).is_none(), "重复结束应当无结果");
    }

    #[test]
    fn cancel_produces_no_result() {
        let (mut store, source, mut drag) = setup();
        drag.start(&mut store, source, None, None);
        drag.cancel(&mut store);

        assert!(!drag.is_active());
        assert_eq!(drag.source(), None);
    }

    #[test]
    fn restarting_clears_the_previous_drag() {
        let (mut store, source, mut drag) = setup();
        let first_icon = store.create(ObjectType::Image);
        let second_icon = store.create(ObjectType::Image);

        drag.start(&mut store, source, Some(first_icon), None);
        drag.start(&mut store, source, Some(second_icon), None);

        assert!(
            !store.object(first_icon).unwrap().visible(),
            "上一次拖拽的图标应当被隐藏"
        );
        assert!(store.object(second_icon).unwrap().visible());
    }

    #[test]
    fn updates_outside_a_drag_are_ignored() {
        let (mut store, _source, mut drag) = setup();
        drag.update(&mut store, Point::new(50.0, 50.0));
        drag.set_target(None);

        assert_eq!(drag.pointer(), Point::ZERO);
        assert!(!drag.is_active());
    }
}

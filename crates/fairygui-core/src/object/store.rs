//! 对象竞技场与对象树。
//!
//! 对象之间既有父到子的引用，也有子到父的引用。用 `Rc<RefCell<..>>` 表达这种结构
//! 会同时招来循环引用（内存泄漏）和借用冲突（运行期 panic），因此在移植时改为
//! **竞技场 + 稳定句柄**：所有对象集中存放，父子关系只保存 [`GObjectId`]。
//!
//! 所有属性修改都必须经由本类型的方法，因为只有它能同时更新属性与脏标记。

// 少数 API 为后续的对象构造阶段预留（例如直接写入来源信息），当前尚无调用点。
#![allow(dead_code)]

use std::collections::HashSet;

use crate::geom::{Point, Size};
use crate::object::object::{BlendMode, GObject, GObjectId};
use crate::package::ObjectType;

#[path = "display.rs"]
mod display;
pub use display::DisplayLockToken;
#[path = "progress.rs"]
mod progress;

#[cfg(test)]
#[path = "animation_tests.rs"]
mod animation_tests;
#[cfg(test)]
#[path = "loader_component_tests.rs"]
mod loader_component_tests;
#[path = "masks.rs"]
mod masks;
#[path = "properties.rs"]
mod properties;

#[path = "groups.rs"]
mod groups;

#[path = "buttons.rs"]
mod buttons;
#[path = "list_items.rs"]
mod list_items;
#[path = "list_selection.rs"]
mod list_selection;
#[path = "virtual_list.rs"]
mod virtual_list;
pub use virtual_list::{
    VirtualItemFactory, VirtualItemProvider, VirtualItemRenderer, VirtualListError, VirtualListInfo,
};
#[path = "scroll_alignment.rs"]
mod scroll_alignment;
#[path = "scroll_bar_input.rs"]
mod scroll_bar_input;
#[path = "scroll_pages.rs"]
mod scroll_pages;
#[path = "scroll_refresh.rs"]
mod scroll_refresh;
#[path = "scroll_view.rs"]
mod scroll_view;
#[path = "scroll_wheel.rs"]
mod scroll_wheel;

#[path = "grayed.rs"]
mod grayed;

#[path = "gear_updates.rs"]
mod gear_updates;

#[path = "stage.rs"]
mod stage;
#[path = "transitions.rs"]
mod transitions;
pub(crate) use transitions::{NestedHandle, TransitionId};

/// 序列帧适配器依次执行的命令。补偿时间不受 Playing/timeScale 影响。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimationCommand {
    SetFrame(i32),
    Advance(f32),
    SetPlaySettings(AnimationPlaySettings),
    Rewind,
}

/// 序列帧分段播放。end/end_at 为 -1 时使用末帧；times <= 0 无限循环。
/// 到达 end 后，下一次满足该帧计时条件的更新才进入下一轮或停在 end_at 并通知完成。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationPlaySettings {
    pub start: i32,
    pub end: i32,
    pub times: i32,
    pub end_at: i32,
}

impl Default for AnimationPlaySettings {
    fn default() -> Self {
        Self {
            start: 0,
            end: -1,
            times: 0,
            end_at: -1,
        }
    }
}

impl AnimationPlaySettings {
    pub fn is_valid(&self) -> bool {
        self.start >= 0 && self.end >= -1 && self.end_at >= -1
    }
}

/// 属性变更位。
///
/// 属性 setter 置位，渲染层每帧消费后清除，从而只同步真正变化过的对象。
pub mod dirty {
    /// 位置变化。
    pub const XY: u32 = 0x0001;
    /// 尺寸变化。
    pub const SIZE: u32 = 0x0002;
    /// 外观变化：缩放、旋转、轴心、不透明度、可见性。
    pub const LOOK: u32 = 0x0004;
    /// 着色与灰度变化。
    pub const COLOR: u32 = 0x0008;
    /// 文本变化。
    pub const TEXT: u32 = 0x0010;
    /// 图像变化：纹理、图集切片、九宫格。
    pub const IMAGE: u32 = 0x0020;
    /// 子对象列表变化。
    pub const CHILDREN: u32 = 0x0040;
    /// 滚动位置变化。
    pub const SCROLL: u32 = 0x0080;
    /// 全部属性。
    pub const ALL: u32 = u32::MAX;
}

/// 正在进行的一次齿轮补间。
///
/// 控制器切页时，若齿轮配置了缓动，会记下一次过渡；此后每帧推进，
/// 逐步把属性从起点移到终点，而不是一步到位。
#[derive(Debug, Clone)]
pub struct GearTween {
    /// 被改动的对象。
    pub owner: GObjectId,
    /// 齿轮种类。
    pub kind: crate::widget::GearKind,
    /// 起点取值。
    pub start: crate::widget::GearValue,
    /// 终点取值。
    pub end: crate::widget::GearValue,
    /// 已过去的时间（秒）。
    pub elapsed: f32,
    /// 启动前等待的时间（秒），包含在 elapsed 内。
    pub delay: f32,
    /// 总时长（秒）。
    pub duration: f32,
    /// 缓动类型。
    pub ease_type: crate::transition::EaseType,
    /// 与本次补间绑定的显示锁。
    pub display_lock: Option<DisplayLockToken>,
}

/// 对象竞技场。
#[derive(Debug, Default)]
pub struct ObjectStore {
    objects: slotmap::SlotMap<GObjectId, GObject>,
    dirty_ids: HashSet<GObjectId>,
    syncing_list_controllers: HashSet<GObjectId>,
    scroll_page_controllers: std::collections::HashMap<GObjectId, (GObjectId, usize)>,
    syncing_scroll_controllers: HashSet<GObjectId>,
    pending_scroll_page_initialization: HashSet<GObjectId>,
    pull_releases: Vec<(GObjectId, crate::scroll::PullRelease)>,
    list_selection_batches: HashSet<GObjectId>,
    root: Option<GObjectId>,
    stage_roots: Vec<GObjectId>,
    staged: HashSet<GObjectId>,
    stage_order: Vec<GObjectId>,
    stage_events: std::collections::VecDeque<(GObjectId, bool)>,
    processing_stage: bool,
    stage_batch: usize,
    /// 正在进行的齿轮补间。
    gear_tweens: Vec<GearTween>,
    /// 独立补间，由使用方通过 [`Self::add_tween`] 加入。
    tweens: crate::transition::TweenManager,
    /// 各下拉框当前展开的弹层。
    combo_popups: std::collections::HashMap<GObjectId, crate::widget::ComboPopup>,
    /// 待播放的音效：(触发对象, 音效地址, 音量)。
    queued_sounds: Vec<(GObjectId, String, f32)>,
    queued_transition_hooks: Vec<crate::transition::TransitionHook>,
    queued_transition_callbacks: Vec<crate::transition::TransitionCallback>,
    animation_commands: std::collections::HashMap<GObjectId, Vec<AnimationCommand>>,
    completed_animations: Vec<GObjectId>,
    transition_driver: Option<transitions::TransitionDriver>,
    /// 记录每个动作曾启动的过渡，避免离页时停止其他来源启动的动画。
    active_transition_actions: HashSet<(GObjectId, String, usize)>,
    /// 控制器动作可以连锁，但不能重入尚未完成切页的控制器。
    changing_controllers: HashSet<(GObjectId, String)>,
    /// 构建期间的控制器动作也立即应用齿轮，不创建首屏属性补间。
    initializing_controllers: bool,
    button_feedback_time: f64,
    button_feedback: Vec<buttons::ButtonFeedback>,
    progress_bars: std::collections::HashMap<GObjectId, progress::ProgressBars>,
    virtual_states: std::collections::HashMap<GObjectId, virtual_list::VirtualState>,
}

impl ObjectStore {
    /// 空存储。
    pub fn new() -> Self {
        Self::default()
    }

    /// 已存在的对象数量。
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// 是否没有任何对象。
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    /// 句柄是否仍然有效。
    pub fn contains(&self, id: GObjectId) -> bool {
        self.objects.contains_key(id)
    }

    /// 只读访问对象。
    pub fn object(&self, id: GObjectId) -> Option<&GObject> {
        self.objects.get(id)
    }

    /// 可变访问对象。
    ///
    /// 仅限本 crate 内部使用：绕过本方法直接改属性不会置脏标记。
    pub(crate) fn object_mut(&mut self, id: GObjectId) -> Option<&mut GObject> {
        self.objects.get_mut(id)
    }

    /// 遍历全部对象。
    pub fn iter(&self) -> impl Iterator<Item = (GObjectId, &GObject)> {
        self.objects.iter()
    }

    // ---- 生命周期 ----

    /// 创建一个对象，初始状态下全部属性都标记为待同步。
    ///
    /// 控件数据按种类给出默认值——否则「按类型创建」出来的按钮、列表等
    /// 会连一份空的容器数据都没有，后续的容器相关行为全都无从谈起。
    /// 解析到真正的描述后，这些默认值会被覆盖。
    pub fn create(&mut self, kind: ObjectType) -> GObjectId {
        let mut object = GObject::new(kind);
        object.control = crate::widget::ControlData::for_kind(kind);

        let id = self.objects.insert(object);
        self.dirty_ids.insert(id);
        id
    }

    /// 删除对象及其整棵子树，返回被删除的全部句柄。
    pub fn remove(&mut self, id: GObjectId) -> Vec<GObjectId> {
        let mut removed = Vec::new();
        self.collect_subtree(id, &mut removed);
        if removed.is_empty() {
            return removed;
        }
        self.detach_from_parent(id);
        self.detach_from_stage(id);
        self.dispose_transitions_in(id);

        let removed_set: HashSet<GObjectId> = removed.iter().copied().collect();
        self.pull_releases
            .retain(|(owner, _)| !removed_set.contains(owner));
        self.virtual_states
            .retain(|owner, _| !removed_set.contains(owner));
        self.scroll_page_controllers
            .retain(|pane, (owner, _)| !removed_set.contains(pane) && !removed_set.contains(owner));
        self.pending_scroll_page_initialization
            .retain(|pane| !removed_set.contains(pane));
        self.button_feedback
            .retain(|feedback| !removed_set.contains(&feedback.button));

        // 从父节点摘除，并让父节点重新同步子列表。
        if let Some(parent) = self.objects.get(id).and_then(|object| object.parent) {
            if let Some(parent_object) = self.objects.get_mut(parent) {
                parent_object.children.retain(|child| *child != id);
                parent_object.dirty |= dirty::CHILDREN;
            }
            self.dirty_ids.insert(parent);
        }

        // 清理幸存对象里指向被删对象的引用。
        let mut detached_group_members = Vec::new();
        for (object_id, object) in self.objects.iter_mut() {
            if object
                .group
                .is_some_and(|group| removed_set.contains(&group))
            {
                object.group = None;
                detached_group_members.push(object_id);
            }
            if object
                .children
                .iter()
                .any(|child| removed_set.contains(child))
            {
                object.children.retain(|child| !removed_set.contains(child));
                object.dirty |= dirty::CHILDREN;
            }
            if let Some(parent) = object.parent
                && removed_set.contains(&parent)
            {
                object.parent = None;
            }
        }

        for &child in &removed {
            self.progress_bars.remove(&child);
            self.animation_commands.remove(&child);
            self.objects.remove(child);
            self.stage_roots.retain(|root| *root != child);
            self.staged.remove(&child);
            self.dirty_ids.remove(&child);
            if self.root == Some(child) {
                self.root = None;
            }
        }
        self.completed_animations
            .retain(|id| !removed_set.contains(id));
        self.stage_order.retain(|id| !removed_set.contains(id));
        for member in detached_group_members {
            self.touch_display(member);
        }

        self.active_transition_actions
            .retain(|(owner, _, _)| !removed_set.contains(owner));
        self.gear_tweens
            .retain(|tween| !removed_set.contains(&tween.owner));

        removed
    }

    fn collect_subtree(&self, id: GObjectId, out: &mut Vec<GObjectId>) {
        let Some(object) = self.objects.get(id) else {
            return;
        };
        out.push(id);
        for &child in &object.children {
            self.collect_subtree(child, out);
        }
        if let Some(state) = self.virtual_states.get(&id) {
            for &(_, child) in &state.pool {
                self.collect_subtree(child, out);
            }
        }
    }

    // ---- 树结构 ----

    /// 根对象。
    pub fn root(&self) -> Option<GObjectId> {
        self.root
    }

    /// 设置根对象。
    pub fn set_root(&mut self, root: Option<GObjectId>) {
        self.root = root;
    }

    /// 把 `child` 挂到 `parent` 下（追加到末尾）。
    pub fn set_parent(&mut self, child: GObjectId, parent: Option<GObjectId>) -> bool {
        if !self.objects.contains_key(child) {
            return false;
        }
        if let Some(parent) = parent
            && (parent == child
                || !self.objects.contains_key(parent)
                || self.is_ancestor(parent, child))
        {
            // 新父节点若是自己的后代，挂上去就会形成环，遍历时会无限递归。
            // 注意方向：要判断的是「新父节点是否以自己为祖先」。
            return false;
        }

        if self.objects.get(child).and_then(|object| object.parent) == parent {
            return true;
        }
        if parent.is_some() && self.stage_roots.contains(&child) {
            self.detach_from_stage(child);
        }
        self.detach_from_parent(child);

        if let Some(object) = self.objects.get_mut(child) {
            object.parent = parent;
        }

        if let Some(parent) = parent
            && let Some(parent_object) = self.objects.get_mut(parent)
        {
            parent_object.children.push(child);
            parent_object.dirty |= dirty::CHILDREN;
        }

        if let Some(parent) = parent {
            self.dirty_ids.insert(parent);
            self.initialize_list_item(parent, child);
            self.refresh_list_selection_snapshot(parent);
        }
        self.refresh_stage();
        true
    }

    /// 在指定位置插入子对象。
    pub fn add_child_at(&mut self, parent: GObjectId, child: GObjectId, index: usize) -> bool {
        if !self.set_parent(child, Some(parent)) {
            return false;
        }
        if let Some(parent_object) = self.objects.get_mut(parent) {
            let Some(current) = parent_object.children.iter().position(|id| *id == child) else {
                return false;
            };
            let index = index.min(parent_object.children.len().saturating_sub(1));
            if index != current {
                let moved = parent_object.children.remove(current);
                parent_object.children.insert(index, moved);
                parent_object.dirty |= dirty::CHILDREN;
            }
        }
        self.dirty_ids.insert(parent);
        self.refresh_list_selection_snapshot(parent);
        true
    }

    /// 把对象从父节点上摘下。
    pub fn detach_from_parent(&mut self, child: GObjectId) -> bool {
        let Some(parent) = self.objects.get(child).and_then(|object| object.parent) else {
            return false;
        };
        if let Some(parent_object) = self.objects.get_mut(parent) {
            if let Some(pane) = parent_object
                .control
                .component_mut()
                .and_then(|c| c.scroll_pane.as_mut())
            {
                if pane.vt_bar == Some(child) {
                    pane.reset_bar_visibility(true);
                    pane.vt_bar = None;
                }
                if pane.hz_bar == Some(child) {
                    pane.reset_bar_visibility(false);
                    pane.hz_bar = None;
                }
                if pane.header() == Some(child) {
                    pane.set_refresh_part(true, None);
                }
                if pane.footer() == Some(child) {
                    pane.set_refresh_part(false, None);
                }
            }
            parent_object
                .children
                .retain(|candidate| *candidate != child);
            parent_object.dirty |= dirty::CHILDREN;
        }
        if let Some(object) = self.objects.get_mut(child) {
            object.parent = None;
            object.group = None;
        }
        self.touch_display(child);
        self.dirty_ids.insert(parent);
        self.refresh_list_selection_snapshot(parent);
        self.layout_scroll_refresh_bars(parent);
        true
    }

    /// 按位置取子对象。
    pub fn child_at(&self, parent: GObjectId, index: usize) -> Option<GObjectId> {
        self.objects
            .get(parent)
            .and_then(|object| object.children.get(index).copied())
    }

    /// 在直接子对象中按编辑器导出的实例 ID 查找。
    pub fn child_by_id(&self, parent: GObjectId, instance_id: &str) -> Option<GObjectId> {
        self.objects
            .get(parent)?
            .children
            .iter()
            .copied()
            .find(|&child| {
                self.objects
                    .get(child)
                    .is_some_and(|object| object.instance_id() == Some(instance_id))
            })
    }

    /// 子对象在父节点中的位置。
    pub fn child_index(&self, child: GObjectId) -> Option<usize> {
        let parent = self.objects.get(child)?.parent?;
        let parent_object = self.objects.get(parent)?;
        parent_object
            .children
            .iter()
            .position(|candidate| *candidate == child)
    }

    /// 在直接子对象中按名称查找。
    pub fn child_by_name(&self, parent: GObjectId, name: &str) -> Option<GObjectId> {
        let parent_object = self.objects.get(parent)?;
        for &child in &parent_object.children {
            if let Some(object) = self.objects.get(child)
                && object.name == name
            {
                return Some(child);
            }
        }
        None
    }

    /// 在整棵子树中按名称查找（深度优先，先子后孙）。
    pub fn find_by_name(&self, root: GObjectId, name: &str) -> Option<GObjectId> {
        let object = self.objects.get(root)?;
        for &child in &object.children {
            if let Some(child_object) = self.objects.get(child)
                && child_object.name == name
            {
                return Some(child);
            }
        }
        for &child in &object.children {
            if let Some(found) = self.find_by_name(child, name) {
                return Some(found);
            }
        }
        None
    }

    /// `candidate` 是否为 `id` 的祖先。
    pub fn is_ancestor(&self, id: GObjectId, candidate: GObjectId) -> bool {
        let mut current = self.objects.get(id).and_then(|object| object.parent);
        while let Some(parent) = current {
            if parent == candidate {
                return true;
            }
            current = self.objects.get(parent).and_then(|object| object.parent);
        }
        false
    }

    /// 自内向外列出全部祖先，用于事件冒泡。
    pub fn ancestors(&self, id: GObjectId) -> Vec<GObjectId> {
        let mut result = Vec::new();
        let mut current = self.objects.get(id).and_then(|object| object.parent);
        while let Some(parent) = current {
            result.push(parent);
            current = self.objects.get(parent).and_then(|object| object.parent);
        }
        result
    }

    /// 自外向内遍历子树（不含自身）。
    ///
    /// 回调使用 trait 对象而非泛型：递归 + 泛型会让编译器不断实例化新类型。
    pub fn for_each_descendant(&self, id: GObjectId, visit: &mut dyn FnMut(GObjectId)) {
        let Some(object) = self.objects.get(id) else {
            return;
        };
        for &child in &object.children {
            visit(child);
            self.for_each_descendant(child, visit);
        }
    }

    // ---- 脏标记 ----

    /// 标记对象需要同步。
    pub fn mark(&mut self, id: GObjectId, flags: u32) {
        if let Some(object) = self.objects.get_mut(id) {
            object.dirty |= flags;
            self.dirty_ids.insert(id);
        }
    }

    /// 标记对象及其整棵子树需要同步。
    pub fn mark_subtree(&mut self, id: GObjectId, flags: u32) {
        self.mark(id, flags);
        let mut descendants = Vec::new();
        self.for_each_descendant(id, &mut |child| descendants.push(child));
        for child in descendants {
            self.mark(child, flags);
        }
    }

    /// 取走全部待同步对象及其变更位，并清除它们的脏标记。
    pub fn take_dirty(&mut self) -> Vec<(GObjectId, u32)> {
        let ids = std::mem::take(&mut self.dirty_ids);
        let mut result = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(object) = self.objects.get_mut(id)
                && object.dirty != 0
            {
                result.push((id, object.dirty));
                object.dirty = 0;
            }
        }
        result
    }

    // ---- 属性写入 ----

    fn touch(&mut self, id: GObjectId, flags: u32) {
        if let Some(object) = self.objects.get_mut(id) {
            object.dirty |= flags;
        }
        self.dirty_ids.insert(id);
    }

    /// 设置位置。
    pub fn set_xy(&mut self, id: GObjectId, x: f32, y: f32) {
        let delta = match self.objects.get_mut(id) {
            Some(object) if object.x != x || object.y != y => {
                let delta = Point::new(x - object.x, y - object.y);
                object.x = x;
                object.y = y;
                Some(delta)
            }
            _ => None,
        };
        if let Some(delta) = delta {
            self.touch(id, dirty::XY);
            for member in self.group_members(id) {
                if let Some(position) = self.position_of(member) {
                    self.set_xy(member, position.x + delta.x, position.y + delta.y);
                }
            }
            self.update_gear_state(id, crate::widget::GearKind::Xy);
        }
    }

    /// 设置尺寸。
    ///
    /// 容器尺寸变化会带动子对象的关联调整，与官方行为一致。
    pub fn set_size(&mut self, id: GObjectId, width: f32, height: f32) {
        let Some((old_width, old_height)) = self
            .objects
            .get(id)
            .map(|object| (object.width, object.height))
        else {
            return;
        };
        if old_width == width && old_height == height {
            return;
        }

        if let Some(object) = self.objects.get_mut(id) {
            object.width = width;
            object.height = height;
        }
        self.touch(id, dirty::SIZE);

        let (d_width, d_height) = (width - old_width, height - old_height);
        // 自己的尺寸变化会同时影响两批对象：挂在子对象上的「相对父节点」关联，
        // 以及挂在兄弟对象上的「以自己为目标」关联。
        self.apply_child_relations(id, d_width, d_height);
        self.apply_sibling_relations(id, d_width, d_height);
        self.refresh_progress_object(id);
        self.update_gear_state(id, crate::widget::GearKind::Size);
    }

    /// 容器尺寸变化后，让子对象的关联调整位置与尺寸。
    ///
    /// 只处理直接子对象；子对象若是容器，其自身尺寸变化会再次触发这一过程。
    fn apply_child_relations(&mut self, container: GObjectId, d_width: f32, d_height: f32) {
        let Some(children) = self
            .objects
            .get(container)
            .map(|object| object.children.clone())
        else {
            return;
        };

        for child in children {
            // 先把需要的数据取出来，避免后续改动时反复借用。
            let Some((x, y, width, height, owner_pivot, owner_pivot_as_anchor, mut relations)) =
                self.objects.get(child).map(|object| {
                    (
                        object.x,
                        object.y,
                        object.width,
                        object.height,
                        Point::new(object.pivot_x, object.pivot_y),
                        object.pivot_as_anchor,
                        object.relations.clone(),
                    )
                })
            else {
                continue;
            };

            // 容器尺寸变化只会影响「相对父节点」的那些关联。
            relations.items.retain(|item| item.to_parent);

            if relations.is_empty() {
                continue;
            }

            let (mut new_x, mut new_y) = (x, y);
            let (mut new_width, mut new_height) = (width, height);
            let target = self.objects.get(container).unwrap();
            relations.apply_on_target_resized_with_context(
                &mut new_x,
                &mut new_y,
                &mut new_width,
                &mut new_height,
                crate::widget::relation::RelationResize::new(
                    crate::widget::relation::RelationTarget {
                        old_size: Point::new(target.width - d_width, target.height - d_height),
                        new_size: Point::new(target.width, target.height),
                        pivot: Point::new(target.pivot_x, target.pivot_y),
                        pivot_as_anchor: target.pivot_as_anchor,
                        is_parent: true,
                        ..crate::widget::relation::RelationTarget::default()
                    },
                    owner_pivot,
                    owner_pivot_as_anchor,
                ),
            );

            if new_x != x || new_y != y {
                self.set_xy(child, new_x, new_y);
                self.update_gear_relations(
                    child,
                    crate::widget::GearKind::Xy,
                    new_x - x,
                    new_y - y,
                );
                self.update_transition_relations(container, child, new_x - x, new_y - y);
            }
            if new_width != width || new_height != height {
                self.set_size(child, new_width, new_height);
                self.update_gear_relations(
                    child,
                    crate::widget::GearKind::Size,
                    new_width - width,
                    new_height - height,
                );
            }
        }
    }

    /// 某个对象的尺寸变化后，让「以它为目标的兄弟关联」跟着调整。
    ///
    /// 关联的目标有两种：父节点，或容器内的某个兄弟对象。前者在容器尺寸变化时
    /// 处理（见 [`Self::apply_child_relations`]），后者在这里处理。描述里记的是
    /// 目标在容器内的编号，因此要先算出自己当前的位置。
    fn apply_sibling_relations(&mut self, changed: GObjectId, d_width: f32, d_height: f32) {
        let Some(parent) = self.objects.get(changed).and_then(|object| object.parent) else {
            return;
        };
        let Some(changed_index) = self.child_index(changed) else {
            return;
        };
        let Some(siblings) = self
            .objects
            .get(parent)
            .map(|object| object.children.clone())
        else {
            return;
        };

        for sibling in siblings {
            if sibling == changed {
                continue;
            }

            let Some((x, y, width, height, owner_pivot, owner_pivot_as_anchor, mut relations)) =
                self.objects.get(sibling).map(|object| {
                    (
                        object.x,
                        object.y,
                        object.width,
                        object.height,
                        Point::new(object.pivot_x, object.pivot_y),
                        object.pivot_as_anchor,
                        object.relations.clone(),
                    )
                })
            else {
                continue;
            };

            // 只保留「目标正是发生变化的那位兄弟」的关联。
            relations
                .items
                .retain(|item| !item.to_parent && item.target_index == Some(changed_index));
            if relations.is_empty() {
                continue;
            }

            let (mut new_x, mut new_y) = (x, y);
            let (mut new_width, mut new_height) = (width, height);
            let target = self.objects.get(changed).unwrap();
            relations.apply_on_target_resized_with_context(
                &mut new_x,
                &mut new_y,
                &mut new_width,
                &mut new_height,
                crate::widget::relation::RelationResize::new(
                    crate::widget::relation::RelationTarget {
                        old_size: Point::new(target.width - d_width, target.height - d_height),
                        new_size: Point::new(target.width, target.height),
                        position: Point::new(target.x, target.y),
                        pivot: Point::new(target.pivot_x, target.pivot_y),
                        pivot_as_anchor: target.pivot_as_anchor,
                        ..crate::widget::relation::RelationTarget::default()
                    },
                    owner_pivot,
                    owner_pivot_as_anchor,
                ),
            );

            if new_x != x || new_y != y {
                self.set_xy(sibling, new_x, new_y);
                self.update_gear_relations(
                    sibling,
                    crate::widget::GearKind::Xy,
                    new_x - x,
                    new_y - y,
                );
                self.update_transition_relations(parent, sibling, new_x - x, new_y - y);
            }
            if new_width != width || new_height != height {
                self.set_size(sibling, new_width, new_height);
                self.update_gear_relations(
                    sibling,
                    crate::widget::GearKind::Size,
                    new_width - width,
                    new_height - height,
                );
            }
        }
    }

    /// 设置缩放。
    pub fn set_scale(&mut self, id: GObjectId, scale_x: f32, scale_y: f32) {
        let changed = match self.objects.get_mut(id) {
            Some(object) if object.scale_x != scale_x || object.scale_y != scale_y => {
                object.scale_x = scale_x;
                object.scale_y = scale_y;
                true
            }
            _ => false,
        };
        if changed {
            self.touch(id, dirty::LOOK);
            self.update_gear_state(id, crate::widget::GearKind::Size);
        }
    }

    /// 设置斜切（角度）。
    pub fn set_skew(&mut self, id: GObjectId, skew_x: f32, skew_y: f32) {
        let changed = match self.objects.get_mut(id) {
            Some(object) if object.skew_x != skew_x || object.skew_y != skew_y => {
                object.skew_x = skew_x;
                object.skew_y = skew_y;
                true
            }
            _ => false,
        };
        if changed {
            self.touch(id, dirty::LOOK);
        }
    }

    /// 设置轴心，取值为尺寸比例。
    pub fn set_pivot(&mut self, id: GObjectId, pivot_x: f32, pivot_y: f32, as_anchor: bool) {
        let changed = match self.objects.get_mut(id) {
            Some(object)
                if object.pivot_x != pivot_x
                    || object.pivot_y != pivot_y
                    || object.pivot_as_anchor != as_anchor =>
            {
                object.pivot_x = pivot_x;
                object.pivot_y = pivot_y;
                object.pivot_as_anchor = as_anchor;
                true
            }
            _ => false,
        };
        if changed {
            self.touch(id, dirty::LOOK);
        }
    }

    /// 设置旋转角度（度）。
    pub fn set_rotation(&mut self, id: GObjectId, rotation: f32) {
        let changed = match self.objects.get_mut(id) {
            Some(object) if object.rotation != rotation => {
                object.rotation = rotation;
                true
            }
            _ => false,
        };
        if changed {
            self.touch(id, dirty::LOOK);
            self.update_gear_state(id, crate::widget::GearKind::Look);
        }
    }

    /// 设置不透明度。
    pub fn set_alpha(&mut self, id: GObjectId, alpha: f32) {
        let alpha = alpha.clamp(0.0, 1.0);
        if let Some(vertical) = self.scroll_bar_axis(id)
            && let Some(container) = self.scroll_container_of_bar(id)
            && let Some(pane) = self
                .objects
                .get_mut(container)
                .and_then(|o| o.control.component_mut())
                .and_then(|c| c.scroll_pane.as_mut())
        {
            pane.override_bar_alpha(vertical, alpha);
        }
        let changed = match self.objects.get_mut(id) {
            Some(object) if object.alpha != alpha => {
                object.alpha = alpha;
                true
            }
            _ => false,
        };
        if changed {
            // 透明度会沿着子树累积下去，因此整棵子树的渲染取值都变了。
            self.mark_subtree(id, dirty::LOOK);
            for member in self.group_members(id) {
                self.set_alpha(member, alpha);
            }
            self.update_gear_state(id, crate::widget::GearKind::Look);
        }
    }

    /// 对象的有效透明度：自身与全部祖先的透明度之积。
    ///
    /// 容器的透明度会作用到整个子树，因此渲染时不能只看自己那一个值。
    /// 祖先链断了（对象已被销毁）时按已累积到的部分返回。
    pub fn effective_alpha(&self, id: GObjectId) -> f32 {
        let mut alpha = 1.0f32;
        let mut current = Some(id);

        while let Some(object_id) = current {
            let Some(object) = self.objects.get(object_id) else {
                break;
            };
            alpha *= object.alpha;
            current = object.parent();
        }
        alpha
    }

    /// 设置可见性。
    pub fn set_visible(&mut self, id: GObjectId, visible: bool) {
        if let Some(vertical) = self.scroll_bar_axis(id)
            && let Some(container) = self.scroll_container_of_bar(id)
            && let Some(pane) = self
                .objects
                .get_mut(container)
                .and_then(|o| o.control.component_mut())
                .and_then(|c| c.scroll_pane.as_mut())
        {
            pane.override_bar_visible(vertical, visible);
        }
        let changed = match self.objects.get_mut(id) {
            Some(object) if object.visible != visible => {
                object.visible = visible;
                true
            }
            _ => false,
        };
        if changed {
            self.touch_display(id);
        }
    }

    /// 设置是否响应输入。
    pub fn set_touchable(&mut self, id: GObjectId, touchable: bool) {
        let changed = match self.objects.get_mut(id) {
            Some(object) if object.touchable != touchable => {
                object.touchable = touchable;
                true
            }
            _ => false,
        };
        if changed {
            self.touch(id, dirty::LOOK);
            self.update_gear_state(id, crate::widget::GearKind::Look);
        }
    }

    /// 设置置灰。组件通过 grayed 控制器或子对象赋值处理，按钮优先使用禁用页。
    /// 首次赋 false 也执行处理；后续相同赋值不再传播。置灰本身不修改 touchable。
    pub fn set_grayed(&mut self, id: GObjectId, grayed: bool) {
        let changed = match self.objects.get_mut(id) {
            Some(object) if !object.grayed_initialized || object.grayed != grayed => {
                object.grayed = grayed;
                object.grayed_initialized = true;
                true
            }
            _ => false,
        };
        if changed {
            self.touch(id, dirty::COLOR);
            self.handle_grayed_changed(id);
            self.update_gear_state(id, crate::widget::GearKind::Look);
        }
    }

    /// 设置混合模式。
    pub fn set_blend_mode(&mut self, id: GObjectId, blend_mode: BlendMode) {
        let changed = match self.objects.get_mut(id) {
            Some(object) if object.blend_mode != blend_mode || !object.blend_mode_set => {
                object.blend_mode = blend_mode;
                object.blend_mode_set = true;
                true
            }
            _ => false,
        };
        if changed {
            self.touch(id, dirty::LOOK);
        }
    }

    /// Reset the Sprite property so blending follows its parent again.
    pub fn reset_blend_mode(&mut self, id: GObjectId) {
        if let Some(object) = self.objects.get_mut(id)
            && object.blend_mode_set
        {
            object.blend_mode_set = false;
            object.blend_mode = BlendMode::Normal;
            self.touch(id, dirty::LOOK);
        }
    }

    /// 设置名称。
    pub fn set_name(&mut self, id: GObjectId, name: impl Into<String>) {
        if let Some(object) = self.objects.get_mut(id) {
            object.name = name.into();
        }
    }

    /// 设置编辑器数据。
    pub fn set_data(&mut self, id: GObjectId, data: Option<String>) {
        if let Some(object) = self.objects.get_mut(id) {
            object.data = data;
        }
    }

    /// 设置提示文本。
    pub fn set_tooltips(&mut self, id: GObjectId, tooltips: Option<String>) {
        if let Some(object) = self.objects.get_mut(id) {
            object.tooltips = tooltips;
        }
    }

    /// 设置所属的组。
    pub fn set_group(&mut self, id: GObjectId, group: Option<GObjectId>) {
        if let Some(group) = group {
            if !self
                .objects
                .get(group)
                .is_some_and(|object| object.kind == ObjectType::Group)
            {
                return;
            }
            let mut next = Some(group);
            let mut visited = HashSet::new();
            while let Some(candidate) = next {
                if candidate == id || !visited.insert(candidate) {
                    return;
                }
                next = self.objects.get(candidate).and_then(|object| object.group);
            }
        }
        if let Some(object) = self.objects.get_mut(id) {
            object.group = group;
        }
        self.touch_display(id);
    }

    /// 设置内容尺寸（容器未经拉伸时的尺寸）。
    pub fn set_source_size(&mut self, id: GObjectId, width: f32, height: f32) {
        if let Some(object) = self.objects.get_mut(id) {
            object.source_width = width;
            object.source_height = height;
        }
    }

    /// 设置编辑器里的原始尺寸。
    pub fn set_init_size(&mut self, id: GObjectId, width: f32, height: f32) {
        if let Some(object) = self.objects.get_mut(id) {
            object.init_width = width;
            object.init_height = height;
        }
    }

    /// 直接写入包条目编号与对象种类（创建阶段使用）。
    pub(crate) fn set_source(
        &mut self,
        id: GObjectId,
        kind: ObjectType,
        package_item_id: Option<String>,
    ) {
        if let Some(object) = self.objects.get_mut(id) {
            object.kind = kind;
            object.package_item_id = package_item_id;
        }
    }

    // ---- 列表项数据 ----

    /// 把某个列表项的数据套用到项对象上。
    ///
    /// 项对象通常是个容器，标题与图标落在它内部名为 `title` / `icon` 的子对象上；
    /// 若项本身就直接承载文本（例如简单列表），也会直接写入。
    /// 返回是否产生了改动。
    pub fn apply_list_item(&mut self, list: GObjectId, index: usize) -> bool {
        let Some(item_data) = self
            .objects
            .get(list)
            .and_then(|object| match object.control() {
                crate::widget::ControlData::List(data) => data.items.get(index),
                crate::widget::ControlData::Tree(data) => data.list.items.get(index),
                _ => None,
            })
            .cloned()
        else {
            return false;
        };

        let Some(item) = self.list_item_at(list, index) else {
            return false;
        };

        let mut changed = false;

        if let Some(title) = item_data.title.as_deref()
            && !title.is_empty()
        {
            // 先尝试项内部的 `title` 子对象，找不到就直接写在项本身上。
            let target = self.child_by_name(item, "title").unwrap_or(item);
            changed |= self.apply_text_to(target, title);
        }

        if let Some(icon) = item_data.icon.as_deref() {
            let target = self.child_by_name(item, "icon").unwrap_or(item);
            changed |= self.apply_icon_to(target, icon);
        }

        // 控制器页面：项可以指定把它内部的某个控制器切到某页——同一个项模板
        // 因状态不同而长相不同时，靠的就是这个。放在最后是因为切换会连带驱动
        // 齿轮改尺寸与位置，让标题与图标先落位更稳妥。
        for (controller, page) in &item_data.controller_pages {
            if self.set_controller_page(item, controller, page) {
                changed = true;
            }
        }

        changed
    }

    /// 把全部列表项的数据套用到各自的项对象上。
    ///
    /// 界面构建完成后调用一次；列表项数量变化后可再调。
    pub fn apply_all_list_items(&mut self) -> usize {
        let lists: Vec<GObjectId> = self
            .objects
            .iter()
            .filter(|(_, object)| {
                matches!(
                    object.control,
                    crate::widget::ControlData::List(_) | crate::widget::ControlData::Tree(_)
                )
            })
            .map(|(id, _)| id)
            .collect();

        let mut applied = 0;
        for list in lists {
            let count = self.list_item_count(list);
            for index in 0..count {
                if self.apply_list_item(list, index) {
                    applied += 1;
                }
            }
        }
        applied
    }

    // ---- 列表选中 ----

    /// 点击列表项时选中它。
    ///
    /// 从被点对象向上找到列表/树的直接子项，按无修饰键点击规则选择。
    /// 返回选中集合是否变化；非按钮项不参与选择。
    pub fn select_clicked_item(&mut self, item: GObjectId) -> bool {
        self.select_clicked_item_with_modifiers(item, false, false)
    }

    /// 点击树项时按其策略切换展开状态。
    ///
    /// 只有 `clickToExpand` 为 `1` 且被点的确实是文件夹时才生效；
    /// 其它取值下由使用方自行处理（官方对 `2` 也没有实现）。
    /// 返回展开状态是否变化。
    pub fn toggle_clicked_tree_node(&mut self, item: GObjectId) -> bool {
        let Some(parent) = self.objects.get(item).and_then(|object| object.parent) else {
            return false;
        };
        let Some(index) = self.list_item_index(parent, item) else {
            return false;
        };

        // 先把需要的两个值取出来，避免与随后的可变借用冲突。
        let (click_to_expand, is_folder) =
            match self.objects.get(parent).map(|object| &object.control) {
                Some(crate::widget::ControlData::Tree(data)) => {
                    (data.click_to_expand, data.is_folder(index))
                }
                _ => return false,
            };

        if click_to_expand != 1 || !is_folder {
            return false;
        }

        // 展开状态一变，哪些节点可见、各自排在第几行都会变，因此要重新布局。
        // 少了这一步，收起再展开后项的位置会停留在折叠时的样子。
        let changed = self.toggle_tree_node(parent, index);
        if changed {
            self.layout_list(parent);
        }
        changed
    }

    // ---- 下拉弹层 ----

    /// 记录下拉框的弹层已展开。同一个下拉框重复展开时以最后一次为准。
    ///
    /// `item_url` 是列表项模板地址，适配器据此在数据变化时复用项。
    pub fn open_combo_popup(
        &mut self,
        combo: GObjectId,
        dropdown: GObjectId,
        list: GObjectId,
        item_url: Option<String>,
    ) {
        self.combo_popups.insert(
            combo,
            crate::widget::ComboPopup {
                dropdown,
                list,
                item_url,
            },
        );
        self.set_controller_page(combo, "button", "down");
    }

    /// 关闭下拉框的弹层，返回被关掉的那个弹层对象。
    pub fn close_combo_popup(&mut self, combo: GObjectId) -> Option<GObjectId> {
        let popup = self.combo_popups.remove(&combo)?;
        let hovered = matches!(self.object(combo).map(|object| object.control()), Some(crate::widget::ControlData::ComboBox(data)) if data.hovered);
        self.set_controller_page(combo, "button", if hovered { "over" } else { "up" });
        Some(popup.dropdown)
    }

    /// 下拉框当前展开的弹层。
    pub fn combo_popup(&self, combo: GObjectId) -> Option<crate::widget::ComboPopup> {
        self.combo_popups.get(&combo).cloned()
    }

    /// 当前展开着弹层的下拉框。
    ///
    /// 适配器通过这些编号统一收起弹层并更新控制器状态。
    pub fn open_combo_ids(&self) -> Vec<GObjectId> {
        self.combo_popups.keys().copied().collect()
    }

    /// 是否有弹层展开着。
    pub fn has_combo_popup(&self) -> bool {
        !self.combo_popups.is_empty()
    }

    /// 关闭全部弹层，返回被关掉的那些。
    ///
    /// 核心状态工具；Bevy 使用 GuiRuntime 的共享弹出栈处理混合弹层。
    pub fn close_all_combo_popups(&mut self) -> Vec<GObjectId> {
        self.combo_popups
            .drain()
            .map(|(_, popup)| popup.dropdown)
            .collect()
    }

    /// 由弹层内的对象反查它属于哪个下拉框。
    pub fn combo_of_popup(&self, object: GObjectId) -> Option<GObjectId> {
        for (combo, popup) in &self.combo_popups {
            let mut current = Some(object);
            while let Some(id) = current {
                if id == popup.dropdown {
                    return Some(*combo);
                }
                current = self.objects.get(id).and_then(|item| item.parent());
            }
        }
        None
    }

    /// 按序号选中下拉框的项，并把标题与图标回填到控件上。
    pub fn select_combo_index(&mut self, combo: GObjectId, index: i32) -> bool {
        let changed = match self
            .objects
            .get_mut(combo)
            .map(|object| &mut object.control)
        {
            Some(crate::widget::ControlData::ComboBox(data)) => data.set_selected_index(index),
            _ => false,
        };

        if changed {
            self.touch(combo, dirty::TEXT | dirty::IMAGE);
            // 标题与图标要落到子对象上才会显示出来。
            self.apply_title_of(combo);
            self.update_gear_state(combo, crate::widget::GearKind::Text);
            if matches!(&self.objects[combo].control,crate::widget::ControlData::ComboBox(d) if !d.icons.is_empty())
            {
                self.update_gear_state(combo, crate::widget::GearKind::Icon);
            }
        }
        changed
    }

    /// 设置对象的文本，按控件类型分派到合适的位置。
    ///
    /// 官方用一个统一的 `text` 属性覆盖按钮标题、静态文本、标签与下拉框
    /// 标题，这里做同样的事，这样「填充列表项」之类的代码不必逐类型分支。
    pub fn set_object_text(&mut self, id: GObjectId, text: &str) -> bool {
        let changed = match self.objects.get_mut(id).map(|object| &mut object.control) {
            Some(crate::widget::ControlData::Button(data)) => {
                data.title = text.to_string();
                true
            }
            Some(crate::widget::ControlData::Label(data)) => {
                data.title = text.to_string();
                true
            }
            Some(crate::widget::ControlData::Text(data)) => {
                data.text = text.to_string();
                true
            }
            Some(crate::widget::ControlData::TextInput(data)) => {
                data.text_field.text =
                    crate::text::input::normalize_input_lines(text, !data.text_field.single_line);
                true
            }
            Some(crate::widget::ControlData::ComboBox(data)) => {
                data.title = text.to_string();
                true
            }
            _ => false,
        };

        if changed {
            self.touch(id, dirty::TEXT);
            // 按钮与标签的文本落在子对象上，其余控件的文本由渲染层直接取用。
            self.apply_title_parts(id, true, false);
            if matches!(
                self.objects[id].control,
                crate::widget::ControlData::Button(_)
                    | crate::widget::ControlData::Label(_)
                    | crate::widget::ControlData::ComboBox(_)
            ) {
                self.update_gear_state(id, crate::widget::GearKind::Text);
            }
        }
        changed
    }

    /// 把标题与图标套用到指定控件的子对象上。非按钮或标签时返回 `false`。
    ///
    /// 与构建期的整体套用是同一套逻辑，供运行期改过标题之后单独调用。
    pub fn apply_title_of(&mut self, id: GObjectId) -> bool {
        self.apply_title(id)
    }

    // ---- 列表布局 ----

    /// 对全部列表按各自的布局重新摆放项。
    ///
    /// 项的位置虽然由描述烘焙，但间距、对齐、分页与树的缩进都要在运行期算出来，
    /// 因此构建完成后统一重排一次。返回被移动的项数量。
    pub fn layout_all_lists(&mut self) -> usize {
        let lists: Vec<GObjectId> = self
            .objects
            .iter()
            .filter(|(_, object)| {
                matches!(
                    object.control,
                    crate::widget::ControlData::List(_) | crate::widget::ControlData::Tree(_)
                )
            })
            .map(|(id, _)| id)
            .collect();

        lists.iter().map(|id| self.layout_list(*id)).sum()
    }

    /// Configure whether invisible rows occupy layout space. Logical indices
    /// and selection remain unchanged. Also supported by a tree's backing list.
    pub fn set_list_fold_invisible_items(&mut self, list: GObjectId, fold: bool) -> bool {
        let data = match self.objects.get_mut(list).map(|object| &mut object.control) {
            Some(crate::widget::ControlData::List(data)) => data.as_mut(),
            Some(crate::widget::ControlData::Tree(data)) => &mut data.list,
            _ => return false,
        };
        if data.fold_invisible_items == fold {
            return false;
        }
        data.fold_invisible_items = fold;
        self.layout_list(list);
        true
    }

    /// 按布局摆放单个列表的项。
    pub fn layout_list(&mut self, list: GObjectId) -> usize {
        if self.is_virtual_list(list) {
            return 0;
        }
        let Some(children) = self.list_items(list) else {
            return 0;
        };
        let fold_invisible = match self.objects.get(list).map(|object| object.control()) {
            Some(crate::widget::ControlData::List(data)) => data.fold_invisible_items,
            Some(crate::widget::ControlData::Tree(data)) => data.list.fold_invisible_items,
            _ => return 0,
        };
        // Keep the original index for tree indentation and selection. Folding
        // affects layout only, never the identity or order of logical items.
        let (child_indices, children): (Vec<_>, Vec<_>) = children
            .into_iter()
            .enumerate()
            .filter(|(_, child)| {
                !fold_invisible || self.objects.get(*child).is_some_and(|o| o.visible())
            })
            .unzip();

        let Some(view_size) = self.objects.get(list).map(|object| object.size()) else {
            return 0;
        };

        // 各项尺寸可能不同（例如条目自带的宽高），逐项取用。
        let item_sizes: Vec<crate::geom::Size> = children
            .iter()
            .map(|child| {
                self.objects
                    .get(*child)
                    .map_or(crate::geom::Size::ZERO, |object| object.size())
            })
            .collect();

        let mut pagination = None;
        let positions = {
            let data = match self.objects.get(list).map(|object| object.control()) {
                Some(crate::widget::ControlData::List(data)) => Some(&**data),
                Some(crate::widget::ControlData::Tree(data)) => Some(&data.list),
                _ => None,
            };
            let Some(data) = data else {
                return 0;
            };
            if data.layout == crate::widget::ListLayoutType::Pagination {
                let view = data
                    .component
                    .scroll_pane
                    .as_ref()
                    .filter(|p| p.view_size().width > 0. && p.view_size().height > 0.)
                    .map_or(
                        Size::new(
                            view_size.width - data.item_margin.horizontal(),
                            view_size.height - data.item_margin.vertical(),
                        ),
                        |p| p.view_size(),
                    );
                let sources: Vec<_> = children
                    .iter()
                    .map(|id| {
                        let o = &self.objects[*id];
                        o.source_size()
                    })
                    .collect();
                let layout = data.pagination_layout(&item_sizes, &sources, view);
                let positions = layout.positions.clone();
                pagination = Some(layout);
                positions
            } else {
                data.layout_items(&item_sizes, view_size)
            }
        };

        if positions.len() != children.len() {
            return 0;
        }

        // 树要在布局结果之上再叠加层级缩进。缩进的计算交给树自己的方法，
        // 避免同一套规则散落两处。非树的列表这里为空。
        let indents: Vec<f32> = match self.objects.get(list).map(|object| &object.control) {
            Some(crate::widget::ControlData::Tree(data)) => child_indices
                .iter()
                .map(|index| data.indent_of(*index))
                .collect(),
            _ => Vec::new(),
        };

        let mut applied = 0;
        for (index, (child, position)) in children.iter().zip(&positions).enumerate() {
            let mut resized = false;
            if let Some(layout) = &pagination {
                let size = layout.sizes[index];
                resized = self.objects[*child].size() != size;
                self.set_size(*child, size.width, size.height);
            }
            let target_x = position.x + indents.get(index).copied().unwrap_or(0.0);
            let moved = self
                .objects
                .get(*child)
                .is_some_and(|object| object.x() != target_x || object.y() != position.y);
            if moved {
                self.set_xy(*child, target_x, position.y);
            }
            if moved || resized {
                applied += 1;
            }
        }

        // 内容尺寸决定滚动范围。
        let content = {
            let data = match self.objects.get(list).map(|object| &object.control) {
                Some(crate::widget::ControlData::List(data)) => Some(&**data),
                Some(crate::widget::ControlData::Tree(data)) => Some(&data.list),
                _ => None,
            };
            pagination
                .as_ref()
                .map(|p| p.content)
                .or_else(|| data.map(|data| data.content_size(&positions, &item_sizes)))
        };
        if let Some(content) = content {
            self.update_scroll_content_size(list, content);
        }

        applied
    }

    // ---- 分组布局 ----

    /// 对全部有排列方式的分组应用布局。
    ///
    /// 返回被重新排布的对象总数。界面构建完成后调用一次即可。
    pub fn layout_all_groups(&mut self) -> usize {
        let groups: Vec<GObjectId> = self
            .objects
            .iter()
            .filter(|(_, object)| matches!(object.control, crate::widget::ControlData::Group(_)))
            .map(|(id, _)| id)
            .collect();

        groups.iter().map(|id| self.layout_group(*id)).sum()
    }

    /// 对分组应用它的排列方式。
    ///
    /// 分组自己不绘制，它管理的是「父容器里归属它的那些对象」——
    /// 判断依据是子对象的 `group` 字段是否指向它。
    ///
    /// 排列以**第一个成员当前的位置**为起点，`excludeInvisibles` 为真时
    /// 跳过不可见的成员。返回被重新排布的对象数量。
    pub fn layout_group(&mut self, group: GObjectId) -> usize {
        let Some(parent) = self.objects.get(group).and_then(|object| object.parent) else {
            return 0;
        };

        let (layout, line_gap, column_gap, exclude_invisibles) =
            match self.objects.get(group).map(|object| &object.control) {
                Some(crate::widget::ControlData::Group(data)) => (
                    data.layout,
                    data.line_gap as f32,
                    data.column_gap as f32,
                    data.exclude_invisibles,
                ),
                _ => return 0,
            };

        // 排列方式为 0 表示不自动排布。
        if layout == 0 {
            return 0;
        }

        let Some(parent_object) = self.objects.get(parent) else {
            return 0;
        };

        let members: Vec<GObjectId> = parent_object
            .children
            .iter()
            .copied()
            .filter(|child| {
                let Some(object) = self.objects.get(*child) else {
                    return false;
                };
                if object.group != Some(group) {
                    return false;
                }
                !exclude_invisibles || object.display_visible()
            })
            .collect();

        if members.is_empty() {
            return 0;
        }

        // 以第一个成员的位置为起点。
        let Some(origin) = self.objects.get(members[0]).map(|o| (o.x(), o.y())) else {
            return 0;
        };

        let sizes: Vec<(f32, f32)> = members
            .iter()
            .map(|id| {
                self.objects
                    .get(*id)
                    .map_or((0.0, 0.0), |o| (o.width(), o.height()))
            })
            .collect();

        let mut placements = Vec::with_capacity(members.len());
        let (mut x, mut y) = origin;

        match layout {
            // 1：横向排列。
            1 => {
                for (width, _) in &sizes {
                    placements.push((x, origin.1));
                    x += width + column_gap;
                }
            }
            // 2：纵向排列。
            2 => {
                for (_, height) in &sizes {
                    placements.push((origin.0, y));
                    y += height + line_gap;
                }
            }
            // 3：网格排列。
            3 => {
                let parent_width = parent_object.width();
                let mut line_height = 0.0f32;

                for (width, height) in &sizes {
                    if x > origin.0 && x + width > origin.0 + parent_width {
                        x = origin.0;
                        y += line_height + line_gap;
                        line_height = 0.0;
                    }
                    placements.push((x, y));
                    x += width + column_gap;
                    line_height = line_height.max(*height);
                }
            }
            _ => return 0,
        }

        let mut applied = 0;
        for (id, (px, py)) in members.iter().zip(placements) {
            let changed = self
                .objects
                .get(*id)
                .is_some_and(|object| object.x() != px || object.y() != py);
            if changed {
                self.set_xy(*id, px, py);
                applied += 1;
            }
        }

        applied
    }

    // ---- 标题与图标 ----

    /// 把按钮与标签的标题、图标套用到各自的子对象上。
    ///
    /// 官方约定：容器里名为 `title` 的子对象承载文本、名为 `icon` 的承载图标。
    /// 描述里读到的标题只是一份数据，必须落到子对象上才会显示出来。
    /// 返回实际改动了多少个对象。
    pub fn apply_titles(&mut self) -> usize {
        let targets: Vec<GObjectId> = self
            .objects
            .iter()
            .filter(|(_, object)| {
                matches!(
                    object.control,
                    crate::widget::ControlData::Button(_)
                        | crate::widget::ControlData::Label(_)
                        | crate::widget::ControlData::ComboBox(_)
                )
            })
            .map(|(id, _)| id)
            .collect();

        let mut applied = 0;
        for id in targets {
            if self.apply_title(id) {
                applied += 1;
            }
        }
        applied
    }

    /// 套用单个按钮或标签的标题与图标。
    fn apply_title(&mut self, id: GObjectId) -> bool {
        self.apply_title_parts(id, true, true)
    }

    fn apply_title_parts(&mut self, id: GObjectId, apply_text: bool, apply_icon: bool) -> bool {
        let (title, icon) = match self.objects.get(id).map(|object| &object.control) {
            Some(crate::widget::ControlData::Button(data)) => {
                let title = if data.selected {
                    data.selected_title
                        .as_ref()
                        .filter(|s| !s.is_empty())
                        .unwrap_or(&data.title)
                } else {
                    &data.title
                };
                let icon = if data.selected {
                    data.selected_icon
                        .as_ref()
                        .filter(|s| !s.is_empty())
                        .or(data.icon.as_ref())
                } else {
                    data.icon.as_ref()
                };
                let icon = icon
                    .cloned()
                    .or_else(|| data.selected_icon.as_ref().map(|_| String::new()));
                (title.clone(), icon)
            }
            Some(crate::widget::ControlData::Label(data)) => {
                (data.title.clone(), data.icon.clone())
            }
            Some(crate::widget::ControlData::ComboBox(data)) => {
                // Cancellation clears both children, including a previously
                // selected item's icon when the next item has none.
                (
                    data.title.clone(),
                    Some(data.icon.clone().unwrap_or_default()),
                )
            }
            _ => return false,
        };

        let mut changed = false;

        if apply_text && let Some(target) = self.child_by_name(id, "title") {
            changed |= self.apply_text_to(target, &title);
        }

        if apply_icon
            && let Some(icon) = icon
            && let Some(target) = self.child_by_name(id, "icon")
        {
            changed |= self.apply_icon_to(target, &icon);
        }

        changed
    }

    /// 设置对象显示的文本，覆盖到各类带文本的控件上。
    pub fn apply_text_to(&mut self, object: GObjectId, text: &str) -> bool {
        if !matches!(
            self.object(object).map(|o| o.control()),
            Some(
                crate::widget::ControlData::Text(_)
                    | crate::widget::ControlData::TextInput(_)
                    | crate::widget::ControlData::Button(_)
                    | crate::widget::ControlData::Label(_)
                    | crate::widget::ControlData::ComboBox(_)
            )
        ) {
            return false;
        }
        let changed = self
            .gear_snapshot(object, crate::widget::GearKind::Text)
            .is_some_and(|value| value != crate::widget::GearValue::Text(text.to_owned()));
        if changed
            || matches!(
                self.objects[object].control,
                crate::widget::ControlData::Button(_)
                    | crate::widget::ControlData::Label(_)
                    | crate::widget::ControlData::ComboBox(_)
            )
        {
            self.set_object_text(object, text);
        }
        changed
    }

    /// 设置装载器或按钮的图标地址。
    pub fn apply_icon_to(&mut self, object: GObjectId, icon: &str) -> bool {
        let Some(target) = self.objects.get_mut(object) else {
            return false;
        };

        let (current, compound) = match &mut target.control {
            crate::widget::ControlData::Loader(data) => (&mut data.url, false),
            crate::widget::ControlData::Button(data) => (&mut data.icon, true),
            crate::widget::ControlData::Label(data) => (&mut data.icon, true),
            crate::widget::ControlData::ComboBox(data) => (&mut data.icon, true),
            _ => return false,
        };
        let changed = current.as_deref() != Some(icon);
        if !changed && !compound {
            return false;
        }
        *current = Some(icon.to_owned());

        if changed {
            self.touch(object, dirty::IMAGE);
        }
        if compound {
            self.apply_title_parts(object, false, true);
        }
        self.update_gear_state(object, crate::widget::GearKind::Icon);
        changed
    }

    // ---- 列表虚拟化 ----

    /// 回收列表里不在可视范围内的项对象。
    ///
    /// 配合 [`crate::widget::ListData::visible_range`] 使用：先算出可视区，
    /// 再把范围外的项归还到对象池。返回被回收的数量。
    ///
    /// 这里按「子对象的位置序号」判断，而不是按名称或标记——列表的项是按顺序
    /// 创建并挂到容器下的，位置即项序号。
    pub fn recycle_items_outside(
        &mut self,
        list: GObjectId,
        keep: std::ops::Range<usize>,
        pool: &mut crate::pool::ObjectPool,
        url: &str,
    ) -> usize {
        let Some(children) = self.content_children(list) else {
            return 0;
        };

        // 从尾部开始回收，避免边遍历边改动下标。
        let mut recycled = 0;
        for index in (0..children.len()).rev() {
            if keep.contains(&index) {
                continue;
            }

            let child = children[index];
            pool.release(self, url, child);
            recycled += 1;
        }

        recycled
    }

    /// 列表当前挂着的项对象数量。
    pub fn list_item_count(&self, list: GObjectId) -> usize {
        self.content_children(list).map_or(0, |items| items.len())
    }

    // ---- 树的展开与可见性 ----

    /// 按可见节点列表同步树里各项的可见性。
    ///
    /// 收起来的文件夹之后的项会被**隐藏而不是销毁**——再次展开时无需重建，
    /// 这也与官方的做法一致。返回被隐藏的项数量。
    pub fn apply_tree_visibility(&mut self, tree: GObjectId, visible: &[usize]) -> usize {
        let Some(children) = self.list_items(tree) else {
            return 0;
        };

        let mut hidden = 0;
        for (index, child) in children.iter().enumerate() {
            let should_show = visible.contains(&index);
            let is_shown = self
                .objects
                .get(*child)
                .is_some_and(|object| object.display_visible());

            if should_show == is_shown {
                continue;
            }

            self.set_visible(*child, should_show);
            if !should_show {
                hidden += 1;
            }
        }

        hidden
    }

    /// 切换树节点的展开状态，并同步各项可见性。返回状态是否变化。
    pub fn toggle_tree_node(&mut self, tree: GObjectId, index: usize) -> bool {
        let changed = match self.objects.get_mut(tree).map(|object| &mut object.control) {
            Some(crate::widget::ControlData::Tree(data)) => data.toggle_expanded(index),
            _ => return false,
        };

        if changed {
            let visible = match self.objects.get(tree).map(|object| &object.control) {
                Some(crate::widget::ControlData::Tree(data)) => data.visible_nodes(),
                _ => return changed,
            };
            self.apply_tree_visibility(tree, &visible);
        }

        changed
    }

    /// 展开到指定节点，使它变为可见。
    ///
    /// 官方在选中节点时会这么做：把它的全部祖先逐一展开。
    pub fn expand_tree_to(&mut self, tree: GObjectId, index: usize) -> bool {
        let changed = match self.objects.get_mut(tree).map(|object| &mut object.control) {
            Some(crate::widget::ControlData::Tree(data)) => data.expand_to(index),
            _ => return false,
        };

        if changed {
            self.refresh_tree_visibility(tree);
        }
        changed
    }

    /// 展开树里的全部文件夹。
    pub fn expand_tree(&mut self, tree: GObjectId) {
        if let Some(crate::widget::ControlData::Tree(data)) =
            self.objects.get_mut(tree).map(|object| &mut object.control)
        {
            data.expand_all();
        }
        self.refresh_tree_visibility(tree);
    }

    /// 收起树里的全部文件夹（根节点保持展开，否则整棵树会空掉）。
    pub fn collapse_tree(&mut self, tree: GObjectId) {
        if let Some(crate::widget::ControlData::Tree(data)) =
            self.objects.get_mut(tree).map(|object| &mut object.control)
        {
            data.collapse_all();
        }
        self.refresh_tree_visibility(tree);
    }

    fn refresh_tree_visibility(&mut self, tree: GObjectId) {
        let visible = match self.objects.get(tree).map(|object| &object.control) {
            Some(crate::widget::ControlData::Tree(data)) => data.visible_nodes(),
            _ => return,
        };
        self.apply_tree_visibility(tree, &visible);
    }

    /// 把一个对象的整棵子树归还到对象池，返回归还的数量。
    ///
    /// 与 [`Self::recycle_items_outside`] 的区别是这里回收的是**整棵树**，
    /// 适用于行/卡片这类自带子对象的项。
    pub fn recycle_subtree(
        &mut self,
        id: GObjectId,
        pool: &mut crate::pool::ObjectPool,
        url: &str,
    ) -> usize {
        if !self.contains(id) {
            return 0;
        }

        let mut subtree = vec![id];
        self.for_each_descendant(id, &mut |child| subtree.push(child));

        // 先从父节点上摘下整棵子树，再逐个归还——否则释放子节点时父节点
        // 还被引用着，回收顺序会乱。
        self.detach_from_parent(id);

        for object in &subtree {
            pool.release(self, url, *object);
        }

        subtree.len()
    }

    // ---- 颜色滤镜 ----

    /// 设置颜色滤镜。传 `None` 表示清除。
    /// 排队一条待播放的音效。
    ///
    /// 核心层不碰音频设备：过渡与控件把要放的声音记下来，使用方每帧取走。
    /// 音量沿用描述里的取值（0~1），使用方可以再乘上自己的总音量。
    pub fn queue_sound(&mut self, owner: GObjectId, sound: impl Into<String>, volume: f32) {
        self.queued_sounds.push((owner, sound.into(), volume));
    }

    /// 取走并清空待播放的音效队列。
    pub fn take_queued_sounds(&mut self) -> Vec<(GObjectId, String, f32)> {
        std::mem::take(&mut self.queued_sounds)
    }

    /// 当前待播放的音效条数。
    pub fn queued_sound_count(&self) -> usize {
        self.queued_sounds.len()
    }

    pub fn set_color_filter(
        &mut self,
        id: GObjectId,
        filter: Option<crate::color_matrix::ColorFilterParams>,
    ) {
        if let Some(object) = self.objects.get_mut(id) {
            object.color_filter = filter;
        }
        self.touch(id, dirty::COLOR);
    }

    // ---- 通用补间 ----

    /// 加入一个补间。
    pub fn add_tween(&mut self, tween: crate::transition::Tween) {
        self.tweens.add(tween);
    }

    /// 每帧推进全部补间，并回收已结束的。
    pub fn advance_tweens(&mut self, dt: f32) {
        // 补间会改动对象属性，先把调度器取出来，推进完再放回。
        let mut tweens = std::mem::take(&mut self.tweens);
        tweens.advance(self, dt);
        self.tweens = tweens;
    }

    /// 当前活跃的补间数量。
    pub fn tween_count(&self) -> usize {
        self.tweens.len()
    }

    /// 终止作用在某个对象上的补间。
    pub fn kill_tweens_for(&mut self, owner: GObjectId) {
        let mut tweens = std::mem::take(&mut self.tweens);
        tweens.kill_for(self, owner);
        self.tweens = tweens;
    }

    // ---- 控件交互 ----

    /// 设置按钮的显示状态。
    ///
    /// 按钮状态由名为 `button` 的控制器承载，切换状态即切换该控制器的页面。
    /// 该控制器不存在时返回 `false`（例如按钮只有一个状态页）。
    pub fn set_button_state(
        &mut self,
        button: GObjectId,
        state: crate::widget::ButtonState,
    ) -> bool {
        if let Some(crate::widget::ControlData::Button(data)) = self
            .objects
            .get_mut(button)
            .map(|object| &mut object.control)
        {
            data.hovered = matches!(
                state,
                crate::widget::ButtonState::Over | crate::widget::ButtonState::SelectedOver
            );
            data.pressed = matches!(
                state,
                crate::widget::ButtonState::Down | crate::widget::ButtonState::SelectedDown
            );
        }
        if matches!(
            state,
            crate::widget::ButtonState::Down | crate::widget::ButtonState::SelectedDown
        ) {
            return self.apply_button_visual_state(button, crate::widget::ButtonState::Down);
        }
        self.refresh_button_state(button)
    }

    // ---- 取值控件 ----

    /// 设置进度条的值，并刷新它的显示。返回是否发生了变化。
    ///
    /// 值会被夹在最小与最大之间。进度由名为 `bar` 的子对象表示，
    /// 标题由名为 `title` 的子对象表示。
    pub fn set_progress(&mut self, object: GObjectId, value: i32) -> bool {
        let Some((min, max, old)) = self.progress_state(object) else {
            return false;
        };

        let value = value.clamp(min, max);
        if value == old {
            return false;
        }

        if let Some(crate::widget::ControlData::ProgressBar(data)) = self
            .objects
            .get_mut(object)
            .map(|target| &mut target.control)
        {
            data.value = value;
        }

        self.refresh_progress(object, value, min, max);
        true
    }

    /// 读取进度条的值。
    pub fn progress_value(&self, object: GObjectId) -> Option<i32> {
        match self.objects.get(object).map(|target| &target.control) {
            Some(crate::widget::ControlData::ProgressBar(data)) => Some(data.value),
            _ => None,
        }
    }

    /// 设置滑动条的值，并移动它的滑块。返回是否发生了变化。
    pub fn set_slider_value(&mut self, object: GObjectId, value: i32) -> bool {
        let Some((min, max, old)) = self.slider_state(object) else {
            return false;
        };

        let value = value.clamp(min, max);
        if value == old {
            return false;
        }

        if let Some(crate::widget::ControlData::Slider(data)) = self
            .objects
            .get_mut(object)
            .map(|target| &mut target.control)
        {
            data.value = value;
        }

        self.refresh_slider(object, value, min, max);
        true
    }

    /// 读取滑动条的值。
    pub fn slider_value(&self, object: GObjectId) -> Option<i32> {
        match self.objects.get(object).map(|target| &target.control) {
            Some(crate::widget::ControlData::Slider(data)) => Some(data.value),
            _ => None,
        }
    }

    /// 按坐标反推滑动条的值，用于拖动滑块。
    ///
    /// 坐标为相对滑动条容器的位置。横向滑动条取横坐标、纵向取纵坐标，
    /// 越界会被夹在取值范围内。
    pub fn slider_value_at(&self, object: GObjectId, position: crate::geom::Point) -> Option<i32> {
        let (min, max, _) = self.slider_state(object)?;
        let (width, height) = self
            .objects
            .get(object)
            .map(|target| (target.width(), target.height()))?;

        let ratio = if width >= height {
            if width <= 0.0 {
                0.0
            } else {
                position.x / width
            }
        } else if height <= 0.0 {
            0.0
        } else {
            position.y / height
        };

        let ratio = ratio.clamp(0.0, 1.0);
        Some(min + ((max - min) as f32 * ratio).round() as i32)
    }

    /// 按控件内的像素坐标设置滑动条的取值。
    ///
    /// 这是「点哪儿跳到哪儿」的语义：坐标先折成取值，再按取值挪动滑块。
    /// 坐标应当是相对控件左上角的像素值。返回取值是否变化。
    pub fn set_slider_value_at(&mut self, object: GObjectId, position: crate::geom::Point) -> bool {
        let Some(value) = self.slider_value_at(object, position) else {
            return false;
        };
        self.set_slider_value(object, value)
    }

    fn progress_state(&self, object: GObjectId) -> Option<(i32, i32, i32)> {
        match self.objects.get(object).map(|target| &target.control) {
            Some(crate::widget::ControlData::ProgressBar(data)) => {
                Some((data.min, data.max, data.value))
            }
            _ => None,
        }
    }

    fn slider_state(&self, object: GObjectId) -> Option<(i32, i32, i32)> {
        match self.objects.get(object).map(|target| &target.control) {
            Some(crate::widget::ControlData::Slider(data)) => {
                Some((data.min, data.max, data.value))
            }
            _ => None,
        }
    }

    /// 按当前值刷新进度条的显示。
    ///
    /// 基准尺寸取编辑器里的原始尺寸，这样反复刷新不会因累计缩放而失真。
    fn refresh_progress(&mut self, object: GObjectId, value: i32, min: i32, max: i32) {
        let span = f64::from(max) - f64::from(min);
        let percent = if span == 0.0 {
            0.0
        } else {
            ((f64::from(value) - f64::from(min)) / span).clamp(0.0, 1.0)
        };

        self.refresh_progress_bars(object, percent);

        let Some(title) = self.child_by_name(object, "title") else {
            return;
        };
        let title_type = match self.objects.get(object).map(|target| &target.control) {
            Some(crate::widget::ControlData::ProgressBar(data)) => data.title_type,
            _ => return,
        };

        let text = match title_type {
            0 => format!("{}%", (percent * 100.0).floor() as i32),
            1 => format!("{value}/{max}"),
            2 => format!("{value}"),
            3 => format!("{max}"),
            _ => return,
        };
        self.apply_text(title, &text);
    }

    /// 按当前值移动滑动条的滑块。
    fn refresh_slider(&mut self, object: GObjectId, value: i32, min: i32, max: i32) {
        let span = max - min;
        let percent = if span == 0 {
            0.0
        } else {
            (value - min) as f32 / span as f32
        };

        let Some(bar) = self.child_by_name(object, "bar") else {
            return;
        };

        let Some((container_width, container_height)) = self
            .objects
            .get(object)
            .map(|target| (target.width(), target.height()))
        else {
            return;
        };
        let Some(child) = self.objects.get(bar) else {
            return;
        };
        let (bar_width, bar_height) = (child.width(), child.height());

        // 横向滑动条移动滑块，纵向则改纵坐标。
        if container_width >= container_height {
            self.set_xy(
                bar,
                ((container_width - bar_width) * percent).round(),
                child.y(),
            );
        } else {
            self.set_xy(
                bar,
                child.x(),
                ((container_height - bar_height) * percent).round(),
            );
        }
    }

    /// 设置对象显示的文本，覆盖到各类带文本的控件上。
    fn apply_text(&mut self, object: GObjectId, text: &str) {
        let Some(target) = self.objects.get_mut(object) else {
            return;
        };
        match &mut target.control {
            crate::widget::ControlData::Text(data) => data.text = text.to_string(),
            crate::widget::ControlData::TextInput(data) => data.text_field.text = text.to_string(),
            crate::widget::ControlData::Button(data) => data.title = text.to_string(),
            crate::widget::ControlData::Label(data) => data.title = text.to_string(),
            _ => return,
        }
        self.mark(object, dirty::TEXT);
    }

    // ---- 滚动 ----

    /// 取容器的滚动容器。不可滚动时返回 `None`。
    pub fn scroll_pane(&self, id: GObjectId) -> Option<&crate::scroll::ScrollPane> {
        self.objects
            .get(id)
            .and_then(|object| object.control.component())
            .and_then(|data| data.scroll_pane.as_ref())
    }

    /// Configure native frame-rate/mobile velocity thresholds and deceleration.
    /// Assignment is inert; an active release keeps its already planned tween.
    pub fn set_scroll_physics(
        &mut self,
        id: GObjectId,
        physics: crate::scroll::ScrollPhysics,
    ) -> bool {
        self.objects
            .get_mut(id)
            .and_then(|o| o.control.component_mut())
            .and_then(|data| data.scroll_pane.as_mut())
            .is_some_and(|pane| pane.set_physics(physics))
    }

    /// 设置滚动条的显示方式。
    ///
    /// 改成 [`crate::scroll::ScrollBarDisplayType::Hidden`] 只影响后续的创建；
    /// 已经建出来的滚动条需要由使用方自行移除。
    pub fn set_scroll_bar_display(
        &mut self,
        container: GObjectId,
        display: crate::scroll::ScrollBarDisplayType,
    ) -> bool {
        let changed = self
            .objects
            .get_mut(container)
            .and_then(|object| object.control.component_mut())
            .and_then(|data| data.scroll_pane.as_mut())
            .is_some_and(|pane| {
                if pane.bar_display == display {
                    false
                } else {
                    pane.bar_display = display;
                    pane.refresh_bar_visibility();
                    true
                }
            });

        if changed {
            self.touch(container, dirty::SCROLL);
        }
        changed
    }

    /// 设置容器的滚动位置。
    ///
    /// `allow_bounce` 为真时允许暂时越界（拖拽与惯性期间使用）。
    pub fn set_scroll_pos(&mut self, id: GObjectId, x: f32, y: f32, allow_bounce: bool) -> bool {
        let changed = self
            .objects
            .get_mut(id)
            .and_then(|object| object.control.component_mut())
            .and_then(|data| data.scroll_pane.as_mut())
            .is_some_and(|pane| pane.set_pos(x, y, allow_bounce));

        if changed {
            self.touch(id, dirty::SCROLL);
            self.update_scroll_page_controller(id);
            self.sync_scroll_refresh_bars(id);
        }
        changed
    }

    /// 推进全部滚动容器的惯性，返回位置发生变化的容器。
    ///
    /// 惯性与回弹都需要跨帧累积，因此每帧调用一次。
    pub fn advance_scrolls(&mut self, dt: f32) -> Vec<GObjectId> {
        let mut moved = Vec::new();
        let mut position_changed = Vec::new();

        for (id, object) in self.objects.iter_mut() {
            let Some(data) = object.control.component_mut() else {
                continue;
            };
            let Some(pane) = data.scroll_pane.as_mut() else {
                continue;
            };
            if pane.advance(dt) {
                object.dirty |= dirty::SCROLL;
                moved.push(id);
                position_changed.push(id);
            }

            // 按需显示的淡出也要跨帧推进。
            if pane.advance_bar_fade(dt) {
                object.dirty |= dirty::SCROLL;
                if !moved.contains(&id) {
                    moved.push(id);
                }
            }
        }

        for id in &moved {
            self.dirty_ids.insert(*id);
        }
        for id in position_changed {
            self.update_scroll_page_controller(id);
            self.sync_scroll_refresh_bars(id);
        }
        moved
    }

    /// 按配置实例化滚动条并加入容器，返回创建出的数量。
    ///
    /// 创建动作由使用方提供：滚动条的地址是 `ui://` 形式，解析它需要运行时
    /// 注册表，而注册表不由本层持有。创建出来的滚动条会作为容器的最后几个
    /// 子对象加入，因此绘制在内容之上。
    pub fn attach_scroll_bars(
        &mut self,
        container: GObjectId,
        create: &mut dyn FnMut(&mut Self, &str) -> Option<GObjectId>,
    ) -> usize {
        let Some((vertical_res, horizontal_res, axes, hidden)) = self
            .objects
            .get(container)
            .and_then(|object| object.control.component())
            .and_then(|data| data.scroll_pane.as_ref())
            .map(|pane| {
                (
                    pane.vertical_bar_res.clone(),
                    pane.horizontal_bar_res.clone(),
                    (
                        pane.scroll_type.allows_vertical(),
                        pane.scroll_type.allows_horizontal(),
                    ),
                    pane.bar_display == crate::scroll::ScrollBarDisplayType::Hidden,
                )
            })
        else {
            return 0;
        };

        // 配置为不显示时根本不创建，与官方一致。
        if hidden {
            return 0;
        }

        let mut created = Vec::new();

        let existing = self
            .scroll_pane(container)
            .map(|pane| (pane.vt_bar, pane.hz_bar))
            .unwrap();
        if axes.0
            && !existing
                .0
                .is_some_and(|bar| self.is_scroll_bar_overlay(bar))
            && let Some(res) = vertical_res.as_deref()
            && let Some(bar) = create(self, res)
        {
            let pane = self
                .objects
                .get_mut(container)
                .unwrap()
                .control
                .component_mut()
                .unwrap()
                .scroll_pane
                .as_mut()
                .unwrap();
            pane.vt_bar = Some(bar);
            pane.reset_bar_visibility(true);
            self.set_parent(bar, Some(container));
            created.push((bar, true));
        }

        if axes.1
            && !existing
                .1
                .is_some_and(|bar| self.is_scroll_bar_overlay(bar))
            && let Some(res) = horizontal_res.as_deref()
            && let Some(bar) = create(self, res)
        {
            let pane = self
                .objects
                .get_mut(container)
                .unwrap()
                .control
                .component_mut()
                .unwrap()
                .scroll_pane
                .as_mut()
                .unwrap();
            pane.hz_bar = Some(bar);
            pane.reset_bar_visibility(false);
            self.set_parent(bar, Some(container));
            created.push((bar, false));
        }

        self.update_scroll_bar_visible(container);
        created.len()
    }

    /// 按容器尺寸摆放滚动条，并把算出的视口尺寸写回。
    ///
    /// 对应官方的 `ScrollPane.setSize`：滚动条各自贴合容器边缘，视口尺寸则要
    /// 扣除滚动条（浮起时不扣）与容器边距。容器尺寸变化后应当重新调用。
    pub fn layout_scroll_bars(&mut self, container: GObjectId) -> bool {
        let Some((owner_size, margin, mut pane)) = self.objects.get(container).and_then(|object| {
            object
                .control
                .component()
                .map(|data| (object.size(), data.margin, data.scroll_pane.clone()))
        }) else {
            return false;
        };
        let Some(pane) = pane.as_mut() else {
            return false;
        };

        let vbar_size = pane
            .vt_bar
            .and_then(|id| self.objects.get(id).map(|object| object.size()));
        let hbar_size = pane
            .hz_bar
            .and_then(|id| self.objects.get(id).map(|object| object.size()));
        let bar_margin = pane.scroll_bar_margin;

        // 横向滚动条贴底；有纵向滚动条时要避开它。
        if let (Some(hz), Some(size)) = (pane.hz_bar, hbar_size) {
            let width = match vbar_size {
                Some(vt) => owner_size.width - vt.width - bar_margin.left - bar_margin.right,
                None => owner_size.width - bar_margin.left - bar_margin.right,
            };
            let x = match vbar_size {
                Some(vt) if pane.display_on_left => bar_margin.left + vt.width,
                _ => bar_margin.left,
            };
            self.set_size(hz, width, size.height);
            self.set_xy(hz, x, owner_size.height - size.height);
        }

        // 纵向滚动条贴右（除非要求放在左侧）；有横向滚动条时高度要避开它。
        if let (Some(vt), Some(size)) = (pane.vt_bar, vbar_size) {
            let height = match hbar_size {
                Some(hz) => owner_size.height - hz.height - bar_margin.top - bar_margin.bottom,
                None => owner_size.height - bar_margin.top - bar_margin.bottom,
            };
            let x = if pane.display_on_left {
                bar_margin.left
            } else {
                owner_size.width - size.width
            };
            self.set_xy(vt, x, bar_margin.top);
            self.set_size(vt, size.width, height);
        }

        // 视口尺寸。
        let mut view = owner_size;
        if let Some(size) = hbar_size
            && !pane.floating
        {
            view.height -= size.height;
        }
        if let Some(size) = vbar_size
            && !pane.floating
        {
            view.width -= size.width;
        }
        view.width = (view.width - margin.left - margin.right).max(1.0);
        view.height = (view.height - margin.top - margin.bottom).max(1.0);

        let changed = pane.view_size() != view;
        pane.set_view_size(view);
        if changed {
            pane.clamp_pos();
        }

        if let Some(data) = self
            .objects
            .get_mut(container)
            .and_then(|object| object.control.component_mut())
        {
            data.scroll_pane = Some(pane.clone());
        }

        self.layout_scroll_refresh_bars(container);

        self.update_scroll_bar_visible(container);
        if changed {
            self.update_scroll_page_controller(container);
        }
        changed
    }

    /// 把当前滚动比例同步到滚动条的滑块。
    ///
    /// 返回摆放过滑块的滚动条数量。滑块的位置与尺寸都由轨道与比例算出，
    /// 每帧或滚动位置变化后调用即可。
    pub fn sync_scroll_bars(&mut self, container: GObjectId) -> usize {
        self.sync_scroll_refresh_bars(container);
        let Some(pane) = self
            .objects
            .get(container)
            .and_then(|object| object.control.component())
            .and_then(|data| data.scroll_pane.as_ref())
            .cloned()
        else {
            return 0;
        };

        let mut applied = 0;
        if let Some(vt) = pane.vt_bar
            && self.sync_one_bar(vt, true, pane.display_perc_y(), self.bar_ratio(&pane, true))
        {
            applied += 1;
        }
        if let Some(hz) = pane.hz_bar
            && self.sync_one_bar(
                hz,
                false,
                pane.display_perc_x(),
                self.bar_ratio(&pane, false),
            )
        {
            applied += 1;
        }

        // 整条滚动条的可见性跟着内容与视口走，与滑块是同一次同步的两部分。
        self.update_scroll_bar_visible(container);

        applied
    }

    /// 滑块占轨道的比例（视口 / 内容）。
    fn bar_ratio(&self, pane: &crate::scroll::ScrollPane, vertical: bool) -> f32 {
        let (view, content) = if vertical {
            (pane.view_size().height, pane.content_size().height)
        } else {
            (pane.view_size().width, pane.content_size().width)
        };
        crate::scroll::ScrollBarData::grip_ratio(view, content)
    }

    /// 摆放单个滚动条的滑块。返回是否找到了滑块。
    fn sync_one_bar(&mut self, bar: GObjectId, vertical: bool, perc: f32, ratio: f32) -> bool {
        let fixed_grip_size = matches!(
            self.objects.get(bar).map(|object| object.control()),
            Some(crate::widget::ControlData::ScrollBar(data)) if data.fixed_grip_size
        );

        let Some((bar_id, grip_id)) = self.scroll_bar_parts(bar) else {
            return false;
        };

        let Some((bar_pos, bar_size, grip_pos, grip_size)) = self
            .objects
            .get(bar_id)
            .zip(self.objects.get(grip_id))
            .map(|(bar, grip)| (bar.position(), bar.size(), grip.position(), grip.size()))
        else {
            return false;
        };

        let layout = crate::scroll::bar::apply_grip_layout(
            crate::scroll::bar::GripState {
                vertical,
                fixed_grip_size,
                ratio,
                perc,
            },
            crate::scroll::bar::GripGeometry {
                bar: crate::geom::Rect::new(bar_pos.x, bar_pos.y, bar_size.width, bar_size.height),
                grip: crate::geom::Rect::new(
                    grip_pos.x,
                    grip_pos.y,
                    grip_size.width,
                    grip_size.height,
                ),
            },
        );

        self.set_xy(grip_id, layout.pos.x, layout.pos.y);
        self.set_size(grip_id, layout.size.width, layout.size.height);
        self.set_visible(grip_id, layout.visible);
        true
    }

    /// 取滚动条内的轨道与滑块。两者缺一不可，缺失时返回 `None`。
    fn scroll_bar_parts(&self, bar: GObjectId) -> Option<(GObjectId, GObjectId)> {
        let children = self.objects.get(bar)?.children.clone();

        let find = |name: &str| {
            children.iter().copied().find(|child| {
                self.objects
                    .get(*child)
                    .is_some_and(|object| object.name() == name)
            })
        };

        Some((
            find(crate::scroll::bar::BAR_NAME)?,
            find(crate::scroll::bar::GRIP_NAME)?,
        ))
    }

    /// 按步长滚动容器，供滚动条的箭头与轨道点击使用。
    pub fn scroll_container_by(
        &mut self,
        container: GObjectId,
        horizontal: bool,
        direction: f32,
    ) -> bool {
        self.prepare_scroll_snapping(container);
        let changed = self
            .objects
            .get_mut(container)
            .and_then(|object| object.control.component_mut())
            .and_then(|data| data.scroll_pane.as_mut())
            .is_some_and(|pane| pane.scroll_by(horizontal, direction));

        if changed {
            self.touch(container, dirty::SCROLL);
            self.update_scroll_page_controller(container);
            self.sync_scroll_refresh_bars(container);
        }
        changed
    }

    /// 按比例设置滚动位置，供滑块的拖动使用。
    pub fn set_scroll_perc(&mut self, container: GObjectId, vertical: bool, perc: f32) -> bool {
        self.prepare_scroll_snapping(container);
        let changed = self
            .objects
            .get_mut(container)
            .and_then(|object| object.control.component_mut())
            .and_then(|data| data.scroll_pane.as_mut())
            .is_some_and(|pane| {
                if vertical {
                    pane.set_perc_y(perc)
                } else {
                    pane.set_perc_x(perc)
                }
            });

        if changed {
            self.touch(container, dirty::SCROLL);
            self.update_scroll_page_controller(container);
        }
        changed
    }

    /// 取滚动条所属的滚动容器。
    ///
    /// 滚动条是容器的子对象，因此向上找一层即可。
    pub fn scroll_container_of_bar(&self, bar: GObjectId) -> Option<GObjectId> {
        let parent = self.objects.get(bar)?.parent()?;
        self.objects
            .get(parent)
            .filter(|object| {
                object
                    .control
                    .component()
                    .is_some_and(|data| data.scroll_pane.is_some())
            })
            .map(|_| parent)
    }

    /// 取某个部件所属的滚动条。滚动条的部件是它的直接子对象。
    pub fn scroll_bar_of_part(&self, part: GObjectId) -> Option<GObjectId> {
        let bar = self.objects.get(part)?.parent()?;
        matches!(
            self.objects.get(bar).map(|object| object.control()),
            Some(crate::widget::ControlData::ScrollBar(_))
        )
        .then_some(bar)
    }

    /// 滚动条的最小尺寸：两端箭头尺寸之和。
    ///
    /// 视口小于这个值时整条滚动条都会隐藏——它连两个箭头都摆不下。箭头缺失
    /// 时按 0 计，此时只要视口为正就不算小。
    pub fn scroll_bar_min_size(&self, bar: GObjectId) -> f32 {
        let Some(vertical) = self.scroll_bar_axis(bar) else {
            return 0.0;
        };
        let Some(children) = self.objects.get(bar).map(|object| object.children.clone()) else {
            return 0.0;
        };

        let mut total = 0.0;
        for child in children {
            let Some(object) = self.objects.get(child) else {
                continue;
            };
            let name = object.name();
            if name == crate::scroll::ARROW1_NAME || name == crate::scroll::ARROW2_NAME {
                let size = object.size();
                total += if vertical { size.height } else { size.width };
            }
        }
        total
    }

    /// 更新滚动条的整体可见性，返回状态发生变化的滚动条数量。
    ///
    /// 启用 display_in_demand 时内容不超出会隐藏整条滚动条；视口放不下箭头时
    /// 也隐藏。每个轴独立应用按需淡出。滑块自身的可见性另由 [`Self::sync_scroll_bars`]
    /// 按比例决定。
    pub fn update_scroll_bar_visible(&mut self, container: GObjectId) -> usize {
        let Some(pane) = self.scroll_pane(container).cloned() else {
            return 0;
        };

        let mut targets = Vec::new();
        if let Some(vt) = pane.vt_bar {
            let scrollable =
                !pane.display_in_demand || pane.content_size().height > pane.view_size().height;
            let roomy = pane.view_size().height > self.scroll_bar_min_size(vt);
            targets.push((vt, true, scrollable && roomy));
        }
        if let Some(hz) = pane.hz_bar {
            let scrollable =
                !pane.display_in_demand || pane.content_size().width > pane.view_size().width;
            let roomy = pane.view_size().width > self.scroll_bar_min_size(hz);
            targets.push((hz, false, scrollable && roomy));
        }
        if let Some(pane) = self
            .objects
            .get_mut(container)
            .and_then(|o| o.control.component_mut())
            .and_then(|c| c.scroll_pane.as_mut())
        {
            for &(_, vertical, available) in &targets {
                pane.set_bar_available(vertical, available);
            }
        }
        let pane = self.scroll_pane(container).unwrap().clone();

        let mut applied = 0;
        for (bar, vertical, _) in targets {
            let visible = pane.bar_visible_for(vertical);
            let alpha = pane.bar_alpha_for(vertical);
            let current = self.objects.get(bar).is_some_and(|object| object.visible());
            if current != visible {
                self.set_visible(bar, visible);
                applied += 1;
            }

            // 只改滚动条自身的透明度就够了：它会沿子树累积到各个部件上，
            // 挨个下发反而会让子部件把自己的透明度乘两次。
            if self
                .objects
                .get(bar)
                .is_some_and(|object| object.alpha() != alpha)
            {
                self.set_alpha(bar, alpha);
            }
        }
        applied
    }

    /// 滑块的可拖动行程：轨道长度减去滑块长度。
    ///
    /// 拖动时用它把光标的位移折算成滚动比例，于是不必知道滚动条在屏幕上的
    /// 绝对位置——界面可能被整体平移或缩放。行程为零时返回 `None`。
    pub fn scroll_grip_track(&self, bar: GObjectId) -> Option<f32> {
        let vertical = self.scroll_bar_axis(bar)?;
        let (bar_id, grip_id) = self.scroll_bar_parts(bar)?;

        let bar_size = self.objects.get(bar_id)?.size();
        let grip_size = self.objects.get(grip_id)?.size();

        let track = if vertical {
            bar_size.height - grip_size.height
        } else {
            bar_size.width - grip_size.width
        };

        (track > 0.0).then_some(track)
    }

    /// 判断某个滚动条是纵向还是横向。返回 `None` 表示它不是已知的滚动条。
    pub fn scroll_bar_axis(&self, bar: GObjectId) -> Option<bool> {
        let container = self.scroll_container_of_bar(bar)?;
        let pane = self.scroll_pane(container)?;

        if pane.vt_bar == Some(bar) {
            Some(true)
        } else if pane.hz_bar == Some(bar) {
            Some(false)
        } else {
            None
        }
    }

    /// 开始拖动滚动容器里的内容。
    pub fn begin_scroll_drag(&mut self, container: GObjectId) -> bool {
        let changed = match self
            .objects
            .get_mut(container)
            .and_then(|object| object.control.component_mut())
            .and_then(|data| data.scroll_pane.as_mut())
        {
            Some(pane) => {
                pane.begin_drag();
                true
            }
            None => false,
        };
        if changed {
            self.touch(container, dirty::SCROLL);
            self.sync_scroll_bars(container);
        }
        changed
    }

    /// 拖动滚动容器里的内容。
    ///
    /// `dt` 是这次位移所花的时间，核心层据此折算速度——低帧率下松手后
    /// 的惯性才不会忽大忽小。返回位置是否变化。
    pub fn drag_scroll_by(&mut self, container: GObjectId, dx: f32, dy: f32, dt: f32) -> bool {
        self.drag_scroll_by_scaled(container, dx, dy, dt, Point::new(1., 1.))
    }

    pub fn drag_scroll_by_scaled(
        &mut self,
        container: GObjectId,
        dx: f32,
        dy: f32,
        dt: f32,
        scale: Point,
    ) -> bool {
        let Some(pane) = self
            .objects
            .get_mut(container)
            .and_then(|object| object.control.component_mut())
            .and_then(|data| data.scroll_pane.as_mut())
        else {
            return false;
        };

        let before = pane.display_pos();
        pane.drag_with_scale(dx, dy, dt, scale);
        let changed = pane.display_pos() != before;

        if changed {
            self.touch(container, dirty::SCROLL);
            self.update_scroll_page_controller(container);
            self.sync_scroll_refresh_bars(container);
        }
        changed
    }

    /// 结束拖动。位置不再跟随，但速度保留给惯性继续滑行。
    pub fn end_scroll_drag(&mut self, container: GObjectId) -> bool {
        self.end_scroll_drag_after(container, 0.)
    }

    pub fn end_scroll_drag_after(&mut self, container: GObjectId, idle: f32) -> bool {
        self.prepare_scroll_snapping(container);
        let changed = match self
            .objects
            .get_mut(container)
            .and_then(|object| object.control.component_mut())
            .and_then(|data| data.scroll_pane.as_mut())
        {
            Some(pane) => {
                if let Some(event) = pane.defer_drag_release(idle) {
                    self.pull_releases.push((container, event));
                } else {
                    pane.complete_drag_release();
                }
                true
            }
            None => false,
        };
        if changed {
            self.touch(container, dirty::SCROLL);
            self.sync_scroll_bars(container);
        }
        changed
    }

    /// 内容尺寸变化时更新滚动容器的可滚动范围。
    pub fn update_scroll_content_size(&mut self, id: GObjectId, size: crate::geom::Size) {
        let changed = self
            .objects
            .get_mut(id)
            .and_then(|object| object.control.component_mut())
            .and_then(|data| data.scroll_pane.as_mut())
            .is_some_and(|pane| {
                pane.set_content_size(size);
                true
            });

        if changed {
            self.touch(id, dirty::SCROLL);
            self.update_scroll_page_controller(id);
            self.layout_scroll_refresh_bars(id);
        }
    }

    pub fn change_scroll_content_on_scrolling(
        &mut self,
        id: GObjectId,
        delta: Size,
        offset: Point,
    ) -> bool {
        let changed = self
            .objects
            .get_mut(id)
            .and_then(|o| o.control.component_mut())
            .and_then(|c| c.scroll_pane.as_mut())
            .is_some_and(|p| p.change_content_size_on_scrolling(delta, offset));
        if changed {
            self.touch(id, dirty::SCROLL);
            self.sync_scroll_bars(id);
            self.update_scroll_page_controller(id);
        }
        changed
    }

    // ---- 过渡动画 ----

    pub(crate) fn queue_transition_hook(&mut self, hook: crate::transition::TransitionHook) {
        self.queued_transition_hooks.push(hook);
    }

    /// 取走本帧经过的过渡标签通知；结束标签不改变播放位置。
    pub fn take_transition_hooks(&mut self) -> Vec<crate::transition::TransitionHook> {
        std::mem::take(&mut self.queued_transition_hooks)
    }

    // ---- 控制器与齿轮联动 ----

    /// 应用全部控制器的齿轮与动作。构建完成后执行一次，也可手动重新应用。
    pub fn apply_all_controllers(&mut self, component: GObjectId) {
        let names: Vec<_> = self
            .objects
            .get(component)
            .and_then(|object| object.control.component())
            .map(|data| {
                data.controllers
                    .iter()
                    .map(|controller| controller.name.clone())
                    .collect()
            })
            .unwrap_or_default();
        for name in names {
            self.apply_gears(component, Some(&name));
            self.sync_related_buttons(component, &name);
            self.sync_related_lists(component, &name);
            self.sync_related_scroll_panes(component, &name);
            self.run_controller_actions(component, &name);
        }
    }

    pub(crate) fn initialize_controllers(&mut self, component: GObjectId) {
        let previous = self.initializing_controllers;
        self.initializing_controllers = true;
        self.apply_all_controllers(component);
        self.initializing_controllers = previous;
    }

    /// 按导出页面 ID 切换，ID 不存在时取消选择。找不到控制器时不做任何事。
    pub fn set_controller_page_id(
        &mut self,
        component: GObjectId,
        controller_name: &str,
        page_id: &str,
    ) -> bool {
        let Some(controller) = self
            .objects
            .get(component)
            .and_then(|object| object.control.component())
            .and_then(|data| data.controller(controller_name))
        else {
            return false;
        };
        let index = controller
            .index_of_id(page_id)
            .map_or(-1, |index| index as i32);
        self.set_controller_page_index(component, controller_name, index)
    }

    /// 切换容器上某个控制器的页面，并驱动直接子对象中绑定该控制器的齿轮。
    ///
    /// 返回是否真的发生了切换。
    pub fn set_controller_page(
        &mut self,
        component: GObjectId,
        controller_name: &str,
        page_name: &str,
    ) -> bool {
        let key = (component, controller_name.to_string());
        if self.changing_controllers.contains(&key) {
            return false;
        }
        let switched = match self
            .objects
            .get_mut(component)
            .and_then(|object| object.control.component_mut())
            .and_then(|data| data.controller_mut(controller_name))
        {
            Some(controller) => controller.set_selected_page_name(page_name),
            None => return false,
        };

        if switched {
            self.changing_controllers.insert(key.clone());
            self.apply_gears(component, Some(controller_name));
            self.sync_related_buttons(component, controller_name);
            self.sync_related_lists(component, controller_name);
            self.sync_related_scroll_panes(component, controller_name);
            self.run_controller_actions(component, controller_name);
            self.changing_controllers.remove(&key);
        }
        switched
    }

    /// 按序号切换控制器页面。
    pub fn set_controller_page_index(
        &mut self,
        component: GObjectId,
        controller_name: &str,
        page_index: i32,
    ) -> bool {
        let key = (component, controller_name.to_string());
        if self.changing_controllers.contains(&key) {
            return false;
        }
        let switched = match self
            .objects
            .get_mut(component)
            .and_then(|object| object.control.component_mut())
            .and_then(|data| data.controller_mut(controller_name))
        {
            Some(controller) => controller.set_selected_index(page_index),
            None => return false,
        };

        if switched {
            self.changing_controllers.insert(key.clone());
            self.apply_gears(component, Some(controller_name));
            self.sync_related_buttons(component, controller_name);
            self.sync_related_lists(component, controller_name);
            self.sync_related_scroll_panes(component, controller_name);
            self.run_controller_actions(component, controller_name);
            self.changing_controllers.remove(&key);
        }
        switched
    }

    /// 执行某个控制器在切页时要跑的动作。
    ///
    /// 动作分两种：播放本容器上的过渡，以及把另一个对象的控制器切到指定页面。
    /// 后者会再次进入切页流程，因此连锁动作天然被支持。
    fn run_controller_actions(&mut self, component: GObjectId, controller_name: &str) {
        let Some(controller) = self
            .objects
            .get(component)
            .and_then(|object| object.control.component())
            .and_then(|data| data.controller(controller_name))
        else {
            return;
        };
        let previous_page = controller.previous_page_id().map(str::to_string);
        let current_page = controller.selected_page_id().map(str::to_string);
        let current_index = controller.selected_index;
        let current_name = controller.selected_page_name().map(str::to_string);
        let actions = controller.actions.clone();

        for (index, action) in actions.into_iter().enumerate() {
            let triggered = action.matches_pages(previous_page.as_deref(), current_page.as_deref());

            match action {
                crate::action::ControllerAction::PlayTransition {
                    transition_name,
                    play_times,
                    delay,
                    stop_on_exit,
                    ..
                } => {
                    let key = (component, controller_name.to_string(), index);
                    let active = self.active_transition_actions.contains(&key);
                    if triggered {
                        let playing = self
                            .transition_state(component, &transition_name)
                            .is_some_and(|(playing, _)| playing);
                        let found = if active && playing {
                            self.change_transition_play_times(
                                component,
                                &transition_name,
                                play_times,
                            )
                        } else {
                            self.play_transition_delayed(
                                component,
                                &transition_name,
                                play_times,
                                delay,
                            )
                        };
                        if found {
                            self.active_transition_actions.insert(key);
                        }
                    } else if stop_on_exit && self.active_transition_actions.remove(&key) {
                        self.stop_transition(component, &transition_name);
                    }
                }
                crate::action::ControllerAction::ChangePage {
                    object_id,
                    controller_name: target_controller_name,
                    target_page,
                    ..
                } => {
                    if !triggered || target_controller_name.is_empty() {
                        continue;
                    }
                    let target = if object_id.is_empty() {
                        Some(component)
                    } else {
                        self.child_by_id(component, &object_id)
                    };
                    let Some(target) = target else { continue };
                    if target == component && target_controller_name == controller_name {
                        continue;
                    }
                    let Some(target_controller) = self
                        .objects
                        .get(target)
                        .and_then(|object| object.control.component())
                        .and_then(|data| data.controller(&target_controller_name))
                    else {
                        continue;
                    };
                    let page_index = match target_page.as_str() {
                        "~1" => {
                            // A shorter target controller keeps its selected page.
                            if current_index >= target_controller.page_count() as i32 {
                                continue;
                            }
                            current_index
                        }
                        "~2" => current_name
                            .as_deref()
                            .and_then(|name| target_controller.index_of(name))
                            .map_or(0, |index| index as i32),
                        _ => target_controller
                            .index_of_id(&target_page)
                            .map_or(-1, |index| index as i32),
                    };
                    self.set_controller_page_index(target, &target_controller_name, page_index);
                }
            }
        }
    }

    /// 按页面 ID 更新直接子对象的齿轮；初始化时立即应用所有控制器。
    pub(crate) fn apply_gears(&mut self, component: GObjectId, changed: Option<&str>) {
        // 先把各控制器的当前页面取出来，避免随后反复借用。
        let pages: Vec<Option<String>> = match self.objects.get(component) {
            Some(object) => object
                .control
                .component()
                .map(|data| {
                    data.controllers
                        .iter()
                        .map(|controller| controller.selected_page_id().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
            None => return,
        };

        // Gear controller indices refer to the immediate parent, not to every
        // ancestor. A button state must not overwrite nested components' gears.
        let targets = self
            .objects
            .get(component)
            .map(|o| o.children.clone())
            .unwrap_or_default();
        let changed_index = changed.and_then(|name| {
            self.objects
                .get(component)?
                .control
                .component()?
                .controllers
                .iter()
                .position(|c| c.name == name)
        });

        for id in targets {
            let Some(object) = self.objects.get(id) else {
                continue;
            };
            if object.gears.is_empty() {
                continue;
            }
            // 齿轮要按引用读取、按可变借用套用，先复制一份出来。
            let mut gears = object.gears.to_vec();
            // Official handleControllerChanged applies display before numeric gears.
            gears.sort_by_key(|gear| gear.kind as u8);

            self.stage_batch += 1;
            for gear in &gears {
                if gear.controller_index < 0
                    || gear.controller_index as usize >= pages.len()
                    || gear
                        .controller_owner
                        .is_some_and(|owner| owner != component)
                    || changed_index.is_some_and(|index| index != gear.controller_index as usize)
                {
                    continue;
                }
                if let Some(original) = self.objects[id]
                    .gears
                    .iter_mut()
                    .find(|g| g.kind == gear.kind)
                {
                    original.controller_owner.get_or_insert(component);
                }
                let page = pages
                    .get(gear.controller_index.max(0) as usize)
                    .cloned()
                    .flatten();

                // 配了缓动的齿轮不会立即设值，而是交回一个补间请求。
                let mut gear = gear.clone();
                if changed.is_none() || self.initializing_controllers {
                    gear.tween = None;
                }
                if let Some(tween) = gear.apply(self, id, page.as_deref()) {
                    self.gear_tweens.push(tween);
                }
            }
            self.stage_batch -= 1;
            self.refresh_stage();
        }
    }

    pub(crate) fn take_gear_tween(
        &mut self,
        owner: GObjectId,
        kind: crate::widget::GearKind,
    ) -> Option<GearTween> {
        let index = self
            .gear_tweens
            .iter()
            .position(|tween| tween.owner == owner && tween.kind == kind)?;
        Some(self.gear_tweens.swap_remove(index))
    }

    /// 每帧推进齿轮补间。
    ///
    /// 补间结束后会做最后一次设值，确保落到精确的终点。
    pub fn advance_gear_tweens(&mut self, dt: f32) {
        if self.gear_tweens.is_empty() || !dt.is_finite() || dt < 0.0 {
            return;
        }

        let mut finished = Vec::new();
        for index in 0..self.gear_tweens.len() {
            let tween = self.gear_tweens[index].clone();
            self.gear_tweens[index].elapsed += dt;
            let elapsed = self.gear_tweens[index].elapsed - tween.delay;
            if elapsed < 0.0 {
                continue;
            }
            let value = if elapsed >= tween.duration {
                finished.push(index);
                tween.end
            } else {
                let progress =
                    crate::transition::evaluate(tween.ease_type, elapsed, tween.duration);
                tween.start.lerp(&tween.end, progress)
            };
            value.apply(self, tween.owner, tween.kind);
            if elapsed >= tween.duration
                && let Some(token) = tween.display_lock
            {
                self.release_display_lock(tween.owner, token);
            }
        }

        // 从后往前移除，避免下标错位。
        for index in finished.into_iter().rev() {
            self.gear_tweens.remove(index);
        }
    }

    /// 当前是否还有齿轮补间在进行。
    pub fn has_gear_tweens(&self) -> bool {
        !self.gear_tweens.is_empty()
    }

    /// 读取对象的位置。
    pub fn position_of(&self, id: GObjectId) -> Option<Point> {
        self.objects.get(id).map(|object| object.position())
    }

    /// 读取对象的尺寸。
    pub fn size_of(&self, id: GObjectId) -> Option<Size> {
        self.objects.get(id).map(|object| object.size())
    }
}

#[cfg(test)]
#[path = "controller_actions_tests.rs"]
mod controller_actions_tests;

#[cfg(test)]
#[path = "change_page_tests.rs"]
mod change_page_tests;

#[cfg(test)]
#[path = "gear_tween_tests.rs"]
mod gear_tween_tests;

#[cfg(test)]
#[path = "gear_state_tests.rs"]
mod gear_state_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn store_with_tree() -> (ObjectStore, GObjectId, GObjectId, GObjectId) {
        let mut store = ObjectStore::new();
        let root = store.create(ObjectType::Component);
        let child = store.create(ObjectType::Image);
        let grandchild = store.create(ObjectType::Text);
        store.set_parent(child, Some(root));
        store.set_parent(grandchild, Some(child));
        store.set_root(Some(root));
        (store, root, child, grandchild)
    }

    #[test]
    fn create_returns_distinct_handles() {
        let mut store = ObjectStore::new();
        let a = store.create(ObjectType::Image);
        let b = store.create(ObjectType::Image);
        assert_ne!(a, b);
        assert_eq!(store.len(), 2);
        assert!(store.contains(a));
    }

    #[test]
    fn handles_are_not_reused_after_removal() {
        let mut store = ObjectStore::new();
        let a = store.create(ObjectType::Image);
        store.remove(a);
        let b = store.create(ObjectType::Image);
        assert_ne!(a, b, "句柄带版本号，回收后不应被复用");
        assert!(!store.contains(a));
    }

    #[test]
    fn proportional_parent_relations_scale_positions_and_sizes_from_original_ratio() {
        let mut store = ObjectStore::new();
        let parent = store.create(ObjectType::Component);
        let child = store.create(ObjectType::Component);
        store.set_parent(child, Some(parent));
        store.set_size(parent, 100.0, 100.0);
        store.set_size(child, 20.0, 10.0);
        store.set_xy(child, 10.0, 20.0);
        store.objects.get_mut(child).unwrap().relations.items = vec![
            crate::widget::relation::RelationItem {
                to_parent: true,
                target_index: None,
                defs: vec![crate::widget::relation::RelationDef {
                    relation_type: 3,
                    percent: true,
                    axis: 0,
                }],
            },
            crate::widget::relation::RelationItem {
                to_parent: true,
                target_index: None,
                defs: vec![crate::widget::relation::RelationDef {
                    relation_type: 14,
                    percent: true,
                    axis: 0,
                }],
            },
        ];

        store.set_size(parent, 200.0, 200.0);

        let child = store.object(child).unwrap();
        assert_eq!(child.x(), 30.0, "the child center keeps its parent ratio");
        assert_eq!(child.width(), 40.0, "relative width scales with the parent");
        assert_eq!(child.y(), 20.0, "unrelated axes stay unchanged");
        assert_eq!(child.height(), 10.0);
    }

    #[test]
    fn proportional_sibling_relations_scale_from_the_sibling_origin() {
        let mut store = ObjectStore::new();
        let parent = store.create(ObjectType::Component);
        let target = store.create(ObjectType::Component);
        let owner = store.create(ObjectType::Component);
        store.set_parent(target, Some(parent));
        store.set_parent(owner, Some(parent));
        store.set_xy(target, 50.0, 30.0);
        store.set_size(target, 20.0, 20.0);
        store.set_xy(owner, 60.0, 15.0);
        store.set_size(owner, 10.0, 10.0);
        store.objects.get_mut(owner).unwrap().relations.items = vec![
            crate::widget::relation::RelationItem {
                to_parent: false,
                target_index: Some(0),
                defs: vec![crate::widget::relation::RelationDef {
                    relation_type: 0,
                    percent: true,
                    axis: 0,
                }],
            },
            crate::widget::relation::RelationItem {
                to_parent: false,
                target_index: Some(0),
                defs: vec![crate::widget::relation::RelationDef {
                    relation_type: 14,
                    percent: true,
                    axis: 0,
                }],
            },
        ];

        store.set_size(target, 40.0, 40.0);

        let owner = store.object(owner).unwrap();
        assert_eq!(owner.x(), 70.0);
        assert_eq!(owner.width(), 20.0);
        assert_eq!(owner.y(), 15.0);
        assert_eq!(owner.height(), 10.0);
    }

    #[test]
    fn proportional_width_relations_keep_native_owner_pivot_semantics() {
        let mut store = ObjectStore::new();
        let parent = store.create(ObjectType::Component);
        let child = store.create(ObjectType::Component);
        store.set_parent(child, Some(parent));
        store.set_size(parent, 100.0, 100.0);
        store.set_size(child, 20.0, 10.0);
        store.set_xy(child, 10.0, 20.0);
        store.set_pivot(child, 0.5, 0.0, true);
        store.objects.get_mut(child).unwrap().relations.items =
            vec![crate::widget::relation::RelationItem {
                to_parent: true,
                target_index: None,
                defs: vec![crate::widget::relation::RelationDef {
                    relation_type: 14,
                    percent: true,
                    axis: 0,
                }],
            }];

        store.set_size(parent, 200.0, 100.0);

        let child = store.object(child).unwrap();
        assert_eq!(child.width(), 40.0);
        assert_eq!(child.x(), 20.0, "parent relation preserves xMin");
    }

    #[test]
    fn parent_and_children_stay_consistent() {
        let (store, root, child, grandchild) = store_with_tree();

        assert_eq!(store.ancestors(grandchild), vec![child, root]);
        assert!(store.is_ancestor(grandchild, root));
        assert!(!store.is_ancestor(root, grandchild));
        assert_eq!(store.child_at(root, 0), Some(child));
        assert_eq!(store.child_index(grandchild), Some(0));
    }

    #[test]
    fn moving_a_node_detaches_it_from_previous_parent() {
        let (mut store, root, child, grandchild) = store_with_tree();
        store.set_parent(grandchild, Some(root));

        assert_eq!(store.object(child).unwrap().children().len(), 0);
        assert_eq!(store.object(root).unwrap().children(), &[child, grandchild]);
    }

    #[test]
    fn cycles_are_rejected() {
        let (mut store, root, child, _) = store_with_tree();
        assert!(!store.set_parent(root, Some(child)), "不能把祖先挂到后代下");
        assert!(!store.set_parent(root, Some(root)), "不能把自己挂到自己下");
    }

    #[test]
    fn removal_takes_the_whole_subtree() {
        let (mut store, root, child, grandchild) = store_with_tree();
        let removed = store.remove(child);

        assert_eq!(removed.len(), 2);
        assert!(removed.contains(&child) && removed.contains(&grandchild));
        assert!(!store.contains(child) && !store.contains(grandchild));
        assert!(store.object(root).unwrap().children().is_empty());
    }

    #[test]
    fn find_by_name_searches_depth_first() {
        let (mut store, root, child, grandchild) = store_with_tree();
        store.set_name(child, "child");
        store.set_name(grandchild, "grandchild");

        assert_eq!(store.find_by_name(root, "child"), Some(child));
        assert_eq!(store.find_by_name(root, "grandchild"), Some(grandchild));
        assert_eq!(
            store.child_by_name(root, "grandchild"),
            None,
            "只查直接子对象"
        );
        assert_eq!(store.find_by_name(root, "missing"), None);
    }

    #[test]
    fn dirty_flags_accumulate_and_are_drained_once() {
        let mut store = ObjectStore::new();
        let id = store.create(ObjectType::Image);

        // 创建时已经全部标记过，先清空
        let initial = store.take_dirty();
        assert_eq!(initial.len(), 1);
        assert_eq!(initial[0].1, dirty::ALL);

        store.set_xy(id, 10.0, 20.0);
        store.set_alpha(id, 0.5);
        // 相同值不应再次标记
        store.set_xy(id, 10.0, 20.0);

        let drained = store.take_dirty();
        assert_eq!(drained.len(), 1, "同一对象的多处修改应合并为一条");
        assert_eq!(drained[0].0, id);
        assert_eq!(drained[0].1, dirty::XY | dirty::LOOK);

        assert!(store.take_dirty().is_empty(), "取走后应无残留");
    }

    #[test]
    fn subtree_marking_covers_descendants() {
        let (mut store, root, _child, grandchild) = store_with_tree();
        store.take_dirty();

        store.mark_subtree(root, dirty::COLOR);
        let drained = store.take_dirty();
        assert_eq!(drained.len(), 3);
        assert!(drained.iter().any(|(id, _)| *id == grandchild));
    }

    #[test]
    fn recycling_keeps_only_the_visible_items() {
        let mut store = ObjectStore::new();
        let mut pool = crate::pool::ObjectPool::new();
        let list = store.create(ObjectType::Component);

        let items: Vec<_> = (0..5)
            .map(|_| {
                let item = store.create(ObjectType::Component);
                store.set_parent(item, Some(list));
                item
            })
            .collect();

        let recycled = store.recycle_items_outside(list, 1..3, &mut pool, "item");

        assert_eq!(recycled, 3);
        assert_eq!(store.list_item_count(list), 2);
        assert_eq!(pool.free_count("item"), 3);

        // 保留的正是中间两项，其余已经摘离。
        assert!(store.object(items[1]).unwrap().parent().is_some());
        assert!(store.object(items[2]).unwrap().parent().is_some());
        assert_eq!(store.object(items[0]).unwrap().parent(), None);
        assert!(!store.object(items[0]).unwrap().visible(), "回收时应当隐藏");
    }

    #[test]
    fn recycling_takes_the_whole_subtree() {
        let mut store = ObjectStore::new();
        let mut pool = crate::pool::ObjectPool::new();

        let list = store.create(ObjectType::Component);
        let item = store.create(ObjectType::Component);
        let label = store.create(ObjectType::Text);
        store.set_parent(item, Some(list));
        store.set_parent(label, Some(item));

        // 行式项自带子对象，应当整棵回收。
        assert_eq!(store.recycle_subtree(item, &mut pool, "row"), 2);
        assert_eq!(pool.free_count("row"), 2);
        assert!(store.object(list).unwrap().children().is_empty());
        assert_eq!(store.object(label).unwrap().parent(), None);

        // 对象并没有被销毁，只是进了池子。
        assert!(store.contains(item));
        assert!(store.contains(label));
    }

    #[test]
    fn recycling_a_dead_handle_is_a_noop() {
        let mut store = ObjectStore::new();
        let mut pool = crate::pool::ObjectPool::new();

        let id = store.create(ObjectType::Component);
        store.remove(id);
        assert_eq!(store.recycle_subtree(id, &mut pool, "row"), 0);
    }

    #[test]
    fn recycling_everything_leaves_an_empty_list() {
        let mut store = ObjectStore::new();
        let mut pool = crate::pool::ObjectPool::new();
        let list = store.create(ObjectType::Component);

        for _ in 0..3 {
            let item = store.create(ObjectType::Component);
            store.set_parent(item, Some(list));
        }

        // 空范围表示一项都不保留。
        assert_eq!(
            store.recycle_items_outside(list, 0..0, &mut pool, "item"),
            3
        );
        assert_eq!(store.list_item_count(list), 0);
    }

    #[test]
    fn button_titles_land_on_child_objects() {
        let mut store = ObjectStore::new();
        let button = store.create(ObjectType::Button);

        let title = store.create(ObjectType::Text);
        store.set_parent(title, Some(button));
        store.set_name(title, "title");

        // 模拟从描述里读到的标题。
        if let Some(crate::widget::ControlData::Button(data)) =
            store.object_mut(button).map(|object| &mut object.control)
        {
            data.title = "开始游戏".to_string();
        }

        assert_eq!(store.apply_titles(), 1, "应当套用一次");
        match store.object(title).map(|object| object.control()) {
            Some(crate::widget::ControlData::Text(data)) => {
                assert_eq!(data.text, "开始游戏");
            }
            other => panic!("类型不符：{other:?}"),
        }

        // 再跑一次不应报告变化。
        assert_eq!(store.apply_titles(), 0, "同一标题不应反复写入");
    }

    #[test]
    fn labels_without_children_are_skipped() {
        let mut store = ObjectStore::new();
        let label = store.create(ObjectType::Label);
        if let Some(crate::widget::ControlData::Label(data)) =
            store.object_mut(label).map(|object| &mut object.control)
        {
            data.title = "标题".to_string();
        }

        // 没有名为 title 的子对象时不应报错，也不该报告改动。
        assert_eq!(store.apply_titles(), 0);
    }

    #[test]
    fn icons_are_applied_to_loader_children() {
        let mut store = ObjectStore::new();
        let button = store.create(ObjectType::Button);

        let icon = store.create(ObjectType::Loader);
        store.set_parent(icon, Some(button));
        store.set_name(icon, "icon");

        if let Some(crate::widget::ControlData::Button(data)) =
            store.object_mut(button).map(|object| &mut object.control)
        {
            data.icon = Some("ui://pkg/star".to_string());
        }

        assert_eq!(store.apply_titles(), 1);
        match store.object(icon).map(|object| object.control()) {
            Some(crate::widget::ControlData::Loader(data)) => {
                assert_eq!(data.url.as_deref(), Some("ui://pkg/star"));
            }
            other => panic!("类型不符：{other:?}"),
        }
    }

    #[test]
    fn horizontal_group_arranges_its_members() {
        let mut store = ObjectStore::new();
        let parent = store.create(ObjectType::Component);
        store.set_size(parent, 500.0, 300.0);

        let group = store.create(ObjectType::Group);
        store.set_parent(group, Some(parent));

        let members: Vec<_> = (0..3)
            .map(|_| {
                let item = store.create(ObjectType::Component);
                store.set_parent(item, Some(parent));
                store.set_group(item, Some(group));
                store.set_size(item, 40.0, 20.0);
                item
            })
            .collect();

        // layout = 1 表示横向排列，列间距 10。
        if let Some(crate::widget::ControlData::Group(data)) =
            store.object_mut(group).map(|object| &mut object.control)
        {
            data.layout = 1;
            data.column_gap = 10;
        }

        store.set_xy(members[0], 100.0, 50.0);
        assert!(store.layout_group(group) > 0);

        // 第一个保持原位，随后依次右移一个「宽 + 间距」。
        assert_eq!(
            store.object(members[0]).unwrap().position(),
            crate::geom::Point::new(100.0, 50.0)
        );
        assert_eq!(
            store.object(members[1]).unwrap().position(),
            crate::geom::Point::new(150.0, 50.0)
        );
        assert_eq!(
            store.object(members[2]).unwrap().position(),
            crate::geom::Point::new(200.0, 50.0)
        );
    }

    #[test]
    fn group_without_layout_is_left_alone() {
        let mut store = ObjectStore::new();
        let parent = store.create(ObjectType::Component);
        let group = store.create(ObjectType::Group);
        store.set_parent(group, Some(parent));

        let item = store.create(ObjectType::Component);
        store.set_parent(item, Some(parent));
        store.set_group(item, Some(group));
        store.set_xy(item, 33.0, 44.0);

        // layout = 0 表示不自动排布。
        assert_eq!(store.layout_group(group), 0);
        assert_eq!(
            store.object(item).unwrap().position(),
            crate::geom::Point::new(33.0, 44.0)
        );
    }

    #[test]
    fn group_skips_invisible_members_when_asked() {
        let mut store = ObjectStore::new();
        let parent = store.create(ObjectType::Component);
        store.set_size(parent, 500.0, 300.0);

        let group = store.create(ObjectType::Group);
        store.set_parent(group, Some(parent));

        let first = store.create(ObjectType::Component);
        let hidden = store.create(ObjectType::Component);
        let last = store.create(ObjectType::Component);
        for item in [first, hidden, last] {
            store.set_parent(item, Some(parent));
            store.set_group(item, Some(group));
            store.set_size(item, 40.0, 20.0);
        }
        store.set_visible(hidden, false);

        if let Some(crate::widget::ControlData::Group(data)) =
            store.object_mut(group).map(|object| &mut object.control)
        {
            data.layout = 1;
            data.column_gap = 10;
            data.exclude_invisibles = true;
        }

        store.set_xy(first, 0.0, 0.0);
        store.layout_group(group);

        // 不可见的成员被跳过，最后一项紧跟在第一项之后。
        assert_eq!(
            store.object(last).unwrap().position(),
            crate::geom::Point::new(50.0, 0.0)
        );
    }

    #[test]
    fn clicking_an_item_selects_it_in_the_list() {
        let mut store = ObjectStore::new();
        let list = store.create(ObjectType::List);

        // 单选模式下需要有数据项才谈得上选中。
        if let Some(crate::widget::ControlData::List(data)) =
            store.object_mut(list).map(|object| &mut object.control)
        {
            data.items = vec![Default::default(); 3];
        }

        let items: Vec<_> = (0..3)
            .map(|_| {
                let item = store.create(ObjectType::Button);
                if let crate::widget::ControlData::Button(data) = &mut store.objects[item].control {
                    data.mode = crate::widget::ButtonMode::Check;
                }
                store.set_parent(item, Some(list));
                item
            })
            .collect();

        assert!(store.select_clicked_item(items[1]));
        assert_eq!(store.list_selection(list), Some(1));

        assert!(!store.select_clicked_item(items[1]), "重复点击不应报告变化");

        assert!(store.select_clicked_item(items[2]));
        assert_eq!(store.list_selection(list), Some(2));
    }

    #[test]
    fn clicking_outside_a_list_does_nothing() {
        let mut store = ObjectStore::new();
        let parent = store.create(ObjectType::Component);
        let child = store.create(ObjectType::Component);
        store.set_parent(child, Some(parent));

        assert!(
            !store.select_clicked_item(child),
            "父对象不是列表时不应生效"
        );
    }

    #[test]
    fn effective_alpha_multiplies_ancestors() {
        let mut store = ObjectStore::new();
        let root = store.create(ObjectType::Component);
        let child = store.create(ObjectType::Component);
        let grandchild = store.create(ObjectType::Component);

        store.set_parent(child, Some(root));
        store.set_parent(grandchild, Some(child));

        assert_eq!(store.effective_alpha(grandchild), 1.0, "默认全不透明");

        store.set_alpha(root, 0.5);
        assert_eq!(store.effective_alpha(child), 0.5);
        assert_eq!(
            store.effective_alpha(grandchild),
            0.5,
            "容器的透明度要一路传到孙辈"
        );

        // 中间层再乘一次。
        store.set_alpha(child, 0.5);
        assert_eq!(store.effective_alpha(grandchild), 0.25);

        // 自身也参与。
        store.set_alpha(grandchild, 0.5);
        assert_eq!(store.effective_alpha(grandchild), 0.125);
    }

    #[test]
    fn setting_alpha_marks_the_whole_subtree() {
        let mut store = ObjectStore::new();
        let root = store.create(ObjectType::Component);
        let child = store.create(ObjectType::Component);
        store.set_parent(child, Some(root));

        store.take_dirty();
        store.set_alpha(root, 0.5);

        let dirty = store.take_dirty();
        let ids: Vec<GObjectId> = dirty.iter().map(|(id, _)| *id).collect();
        assert!(ids.contains(&root), "容器自身要更新");
        assert!(ids.contains(&child), "子对象也要重算透明度");
    }

    #[test]
    fn slider_value_follows_the_cursor_position() {
        let mut store = ObjectStore::new();
        let slider = store.create(ObjectType::Slider);
        store.set_size(slider, 200.0, 20.0);

        if let Some(crate::widget::ControlData::Slider(data)) =
            store.object_mut(slider).map(|object| &mut object.control)
        {
            data.min = 0;
            data.max = 100;
            data.value = 0;
        }

        // 点中间 → 一半。
        assert!(store.set_slider_value_at(slider, crate::geom::Point::new(100.0, 10.0)));
        assert_eq!(store.slider_value(slider), Some(50));

        // 点最右 → 满值。
        store.set_slider_value_at(slider, crate::geom::Point::new(200.0, 10.0));
        assert_eq!(store.slider_value(slider), Some(100));

        // 超出右边界也夹在满值。
        store.set_slider_value_at(slider, crate::geom::Point::new(999.0, 10.0));
        assert_eq!(store.slider_value(slider), Some(100));

        // 回到最左。
        store.set_slider_value_at(slider, crate::geom::Point::new(0.0, 10.0));
        assert_eq!(store.slider_value(slider), Some(0));
    }

    #[test]
    fn list_layout_moves_items_into_place() {
        let mut store = ObjectStore::new();
        let list = store.create(ObjectType::List);
        store.set_size(list, 200.0, 300.0);

        // 三项都摆在原点——描述里烘焙的位置未必符合布局。
        let items: Vec<_> = (0..3)
            .map(|_| {
                let item = store.create(ObjectType::Button);
                if let crate::widget::ControlData::Button(data) = &mut store.objects[item].control {
                    data.mode = crate::widget::ButtonMode::Check;
                }
                store.set_parent(item, Some(list));
                store.set_size(item, 100.0, 20.0);
                item
            })
            .collect();

        assert!(store.layout_list(list) > 0, "应当重新摆放了项");
        assert_eq!(store.list_item_count(list), 3);
        assert!(store.set_list_selection(list, 2));
        assert_eq!(
            store.list_selection(list),
            Some(2),
            "dynamic rows need no serialized descriptors"
        );
        store.set_list_selection(list, 3);
        assert_eq!(store.list_selection(list), Some(-1));

        // 单列布局纵向依次排列。
        assert_eq!(store.object(items[0]).unwrap().y(), 0.0);
        assert_eq!(store.object(items[1]).unwrap().y(), 20.0);
        assert_eq!(store.object(items[2]).unwrap().y(), 40.0);
    }

    #[test]
    fn list_layout_is_idempotent() {
        let mut store = ObjectStore::new();
        let list = store.create(ObjectType::List);
        store.set_size(list, 200.0, 300.0);

        for _ in 0..2 {
            let item = store.create(ObjectType::Component);
            store.set_parent(item, Some(list));
            store.set_size(item, 100.0, 20.0);
        }

        store.layout_list(list);
        // 已经摆好之后再次布局不应再报告移动。
        assert_eq!(store.layout_list(list), 0, "重复布局应当是幂等的");
    }

    #[test]
    fn folding_invisible_rows_preserves_tree_indices_and_resets_scroll_extent_when_empty() {
        let mut store = ObjectStore::new();
        let tree = store.create(ObjectType::Tree);
        store.set_size(tree, 100.0, 20.0);
        if let crate::widget::ControlData::Tree(data) = &mut store.object_mut(tree).unwrap().control
        {
            data.indent = 17;
            data.nodes = (0..3)
                .map(|level| crate::widget::TreeNodeData {
                    level,
                    ..Default::default()
                })
                .collect();
            data.list.line_gap = 3;
        }
        let mut pane = crate::scroll::ScrollPane::default();
        pane.set_view_size(Size::new(100.0, 20.0));
        store
            .object_mut(tree)
            .unwrap()
            .control
            .component_mut()
            .unwrap()
            .scroll_pane = Some(pane);
        let rows: Vec<_> = (0..3)
            .map(|_| {
                let row = store.create(ObjectType::Component);
                store.set_size(row, 100.0, 30.0);
                store.set_parent(row, Some(tree));
                row
            })
            .collect();
        store.set_visible(rows[1], false);
        store.layout_list(tree);
        assert_eq!(store.object(rows[2]).unwrap().y(), 66.0);
        assert!(store.set_list_fold_invisible_items(tree, true));
        assert_eq!(store.object(rows[2]).unwrap().y(), 33.0);
        assert_eq!(
            store.object(rows[2]).unwrap().x(),
            34.0,
            "indent uses logical index"
        );
        assert_eq!(store.child_at(tree, 2), Some(rows[2]));
        assert!(store.scroll_pane(tree).unwrap().max_scroll_pos().y > 0.0);
        for row in rows {
            store.set_visible(row, false);
        }
        store.layout_list(tree);
        assert_eq!(store.scroll_pane(tree).unwrap().max_scroll_pos().y, 0.0);
    }

    #[test]
    fn tree_layout_applies_indentation() {
        let mut store = ObjectStore::new();
        let tree = store.create(ObjectType::Tree);
        store.set_size(tree, 300.0, 300.0);

        // 两个节点：根层级与子层级。
        if let Some(crate::widget::ControlData::Tree(data)) =
            store.object_mut(tree).map(|object| &mut object.control)
        {
            data.indent = 15;
            data.nodes = vec![
                crate::widget::TreeNodeData {
                    is_folder: true,
                    level: 0,
                    ..Default::default()
                },
                crate::widget::TreeNodeData {
                    is_folder: false,
                    level: 1,
                    ..Default::default()
                },
            ];
            data.list.items = vec![Default::default(); 2];
        }

        let items: Vec<_> = (0..2)
            .map(|_| {
                let item = store.create(ObjectType::Component);
                store.set_parent(item, Some(tree));
                store.set_size(item, 100.0, 20.0);
                item
            })
            .collect();

        store.layout_list(tree);

        // 第二个节点比第一个多一级缩进。
        let first = store.object(items[0]).unwrap().x();
        let second = store.object(items[1]).unwrap().x();
        assert_eq!(first, 0.0);
        assert_eq!(second, 15.0, "子层级应当多缩进一级");
    }

    #[test]
    fn alpha_is_clamped() {
        let mut store = ObjectStore::new();
        let id = store.create(ObjectType::Image);
        store.set_alpha(id, 2.0);
        assert_eq!(store.object(id).unwrap().alpha(), 1.0);
        store.set_alpha(id, -1.0);
        assert_eq!(store.object(id).unwrap().alpha(), 0.0);
    }
}

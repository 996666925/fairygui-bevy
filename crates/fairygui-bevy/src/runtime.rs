//! 运行期：对象存储、实体映射与实体树生成。
//!
//! 核心对象树是**唯一权威状态**，ECS 实体只是它的视图。因此
//! [`GuiRuntime`] 只在主线程访问，不要求 `Send`。

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::text::ComputedTextBlock;

use fairygui_core::object::{GObjectId, ObjectStore, dirty};
use fairygui_core::package::FuiPackage;
use fairygui_core::widget::{ControlData, OverflowType};

use crate::asset::FuiAsset;
use crate::backend;
use crate::ecs::GuiRootView;

#[path = "drag_drop.rs"]
pub(crate) mod ecs_drag_drop;
#[cfg(test)]
#[path = "ecs_root_tests.rs"]
mod ecs_root_tests;
#[path = "global_wait.rs"]
pub(crate) mod global_wait;
#[path = "object_drag.rs"]
pub(crate) mod object_drag;
#[path = "text_measure.rs"]
pub(crate) mod text_measure;
#[path = "tree_content.rs"]
mod tree_content;
#[path = "windows.rs"]
pub(crate) mod windows;

/// 实体所镜像的核心对象。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct GObjectRef(pub GObjectId);

/// Current selected row indices projected onto a FairyGUI list entity.
///
/// This is a read-only ECS view: change selection through [`GuiObjectCommand`]
/// so controller and virtual-list behavior stay synchronized.
#[derive(Component, Debug, Default, Clone, PartialEq, Eq)]
pub struct GuiListSelection(pub Vec<usize>);

/// Current value projected from a FairyGUI progress bar.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GuiProgressValue(pub i32);

/// Current value projected from a FairyGUI slider.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GuiSliderValue(pub i32);

/// Current selected state projected from a FairyGUI button.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GuiButtonSelected(pub bool);

/// Global FairyGUI defaults captured by the next package construction pass.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
pub struct GuiConfig {
    /// Global scroll-pane defaults used while loading `.fui` components.
    pub scroll: fairygui_core::scroll::ScrollDefaults,
}

/// ECS-owned FairyGUI root. Add this component to any entity to create the
/// named component from an already requested `.fui` asset when it is ready.
/// Removing the component removes the generated UI tree and its Bevy views.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct FairyGuiRoot {
    pub asset: Handle<FuiAsset>,
    pub item_name: String,
    /// Detach the root from the stage while keeping its model and views alive.
    pub active: bool,
    /// Requested logical position. Applied before the first view and when this
    /// value changes; None leaves native/current geometry unchanged.
    pub position: Option<Vec2>,
    /// Requested unscaled size in logical pixels. None keeps exported/current
    /// size. Changes update the retained root and its relations without rebuilding.
    pub size: Option<Vec2>,
    /// Keep the root sized to the input window's logical dimensions (primary
    /// window by default). Takes precedence over `size` while a window exists.
    pub follow_window_size: bool,
}

impl FairyGuiRoot {
    pub fn new(asset: Handle<FuiAsset>, item_name: impl Into<String>) -> Self {
        Self {
            asset,
            item_name: item_name.into(),
            active: true,
            position: None,
            size: None,
            follow_window_size: false,
        }
    }

    pub fn with_position(mut self, position: Vec2) -> Self {
        self.position = Some(position);
        self
    }

    pub fn with_size(mut self, size: Vec2) -> Self {
        self.size = Some(size);
        self
    }

    /// Size to the window before the first view and follow subsequent resizes.
    /// Uses [`crate::GuiInputSettings::window`], or the primary window by default.
    /// Position is independent; use `with_position(Vec2::ZERO)` to fill the window.
    pub fn with_window_size(mut self) -> Self {
        self.follow_window_size = true;
        self
    }
}

/// Runtime result attached to the entity that owns [`FairyGuiRoot`].
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuiRootInstance {
    /// Stable `GuiRuntime` root slot for APIs that still require its handle.
    pub handle: usize,
    /// Authoritative root object in the FairyGUI object store.
    pub object: GObjectId,
}

/// Common model mutations addressed by the Bevy entity that owns a FairyGUI
/// object (`GObjectRef`) or root (`GuiRootInstance`).
#[derive(Debug, Clone)]
pub enum GuiObjectAction {
    /// Animate from the current value, replacing only this property tween.
    Tween {
        target: crate::GuiTweenTarget,
        duration: f32,
        ease: i32,
    },
    StopTween(crate::GuiTweenProperty),
    SetPosition(Vec2),
    SetSize(Vec2),
    SetVisible(bool),
    SetTouchable(bool),
    SetAlpha(f32),
    SetScale(Vec2),
    SetPivot {
        pivot: Vec2,
        as_anchor: bool,
    },
    SetRotation(f32),
    SetText(String),
    /// Resolve the current text's auto size synchronously, including relations.
    /// Subsequent text/font changes keep this field measured before projection.
    EnsureTextSize,
    /// Set the native Loader/button/label/combo icon URL; empty clears it.
    SetIcon(String),
    SetColor(fairygui_core::color::Color),
    SetGrayed(bool),
    SetButtonSelected(bool),
    /// Let application observers own selection instead of native click toggles.
    SetButtonChangeStateOnClick(bool),
    SetAnimationPlaying(bool),
    SetAnimationFrame(i32),
    SetAnimationTimeScale(f32),
    /// Configure a range and seek to its start, preserving Playing and speed.
    SetAnimationPlaySettings(crate::GuiAnimationPlaySettings),
    /// Native DeltaTime catch-up in seconds, independent of Playing and speed.
    AdvanceAnimation(f32),
    /// Rewind without restarting an ended finite playback.
    RewindAnimation,
    SetSliderValue(i32),
    SetControllerPage {
        name: String,
        index: i32,
    },
    SetProgress(i32),
    SetScrollPosition {
        position: Vec2,
        allow_bounce: bool,
    },
    /// Native programmatic scroll with page/item alignment and optional animation.
    ScrollTo {
        position: Vec2,
        animated: bool,
    },
    SetScrollPercentage {
        percentage: Vec2,
        animated: bool,
    },
    /// Reveal a rectangle in the scroll pane's content coordinates.
    ScrollToView {
        rect: fairygui_core::geom::Rect,
        animated: bool,
        set_first: bool,
    },
    SetListSelection(i32),
    SelectListIndex {
        index: usize,
        scroll_to_view: bool,
    },
    SetTreeExpanded {
        index: usize,
        expanded: bool,
    },
    ToggleTreeNode(usize),
    ExpandTree,
    CollapseTree,
    /// Expand ancestors and add this logical node to the native selection.
    SelectTreeNode {
        index: usize,
        scroll_to_view: bool,
    },
    PlayTransition {
        name: String,
        times: i32,
    },
    PlayTransitionWithOptions {
        name: String,
        options: fairygui_core::transition::TransitionPlayOptions,
    },
    SetTransitionPaused {
        name: String,
        paused: bool,
    },
    SetTransitionTimeScale {
        name: String,
        scale: f32,
    },
    PlayTransitionReverse {
        name: String,
        delay: f32,
    },
    StopTransition {
        name: String,
    },
}

/// Entity-addressed FairyGUI mutation. Send this from application ECS systems
/// instead of borrowing the main-thread `GuiRuntime` for common property edits.
/// Commands for a root waiting on its asset are retained until it is created.
/// Removing or replacing that root cancels its retained commands.
#[derive(Message, Debug, Clone)]
pub struct GuiObjectCommand {
    pub target: Entity,
    pub action: GuiObjectAction,
}

impl GuiObjectCommand {
    pub fn new(target: Entity, action: GuiObjectAction) -> Self {
        Self { target, action }
    }
}

#[derive(Debug)]
struct EcsRootBinding {
    asset: Handle<FuiAsset>,
    item_name: String,
    handle: Option<usize>,
    active: bool,
    failed: bool,
    position: Option<Vec2>,
    size: Option<Vec2>,
    follow_window_size: bool,
}

#[derive(Component, Debug)]
pub(crate) struct ScrollBarOverlay;

/// 命中区域由某个子对象决定的容器。
///
/// 它自己不参与命中——编辑器把命中判定指给了另一个子对象，那就由那个子对象
/// 去接，容器再抢一次只会让本该落空的点击被吃掉。
#[derive(Component, Debug, Clone, Copy)]
pub struct HitAreaFromChild;

/// 像素级命中数据。
///
/// 只有带该组件的节点才会做逐像素判定；其余节点按矩形命中处理。
#[derive(Component, Debug, Clone)]
pub struct PixelHitTest(pub std::sync::Arc<fairygui_core::package::PixelHitTestData>);

/// 创建时的着色快照，保留用于兼容。渲染以核心模型当前的颜色重新计算。
#[derive(Component, Debug, Clone, Copy)]
pub struct BaseTint(pub bevy::color::Color);

#[path = "appearance.rs"]
pub(crate) mod appearance;
#[cfg(test)]
#[path = "bitmap_text_tests.rs"]
mod bitmap_text_tests;
#[cfg(test)]
#[path = "list_bar_tests.rs"]
mod list_bar_tests;
#[path = "scroll_extent.rs"]
mod scroll_extent;
#[cfg(test)]
#[path = "scroll_pages_tests.rs"]
mod scroll_pages_tests;
pub(crate) use scroll_extent::sync_scroll_extents;
#[path = "list_content.rs"]
mod list_content;
#[path = "virtual_lists.rs"]
mod virtual_lists;
pub(crate) use virtual_lists::sync_virtual_lists;
#[path = "scroll_overflow.rs"]
mod scroll_overflow;
pub(crate) use scroll_overflow::project_scroll_overflow;
#[cfg(test)]
#[path = "pull_refresh_tests.rs"]
mod pull_refresh_tests;
#[cfg(test)]
#[path = "scroll_motion_tests.rs"]
mod scroll_motion_tests;
#[cfg(test)]
#[path = "scroll_view_tests.rs"]
mod scroll_view_tests;
pub(crate) use appearance::sync_appearance;
pub(crate) use appearance::tint;

#[path = "text_fonts.rs"]
mod text_fonts;
pub(crate) use text_fonts::sync_text_fonts;
#[path = "text_input.rs"]
mod text_input;
pub(crate) use text_input::InputEditPolicy;
#[path = "input_prompt.rs"]
mod input_prompt;
pub use input_prompt::InputPromptOwner;
pub(crate) use input_prompt::{sync_input_prompts, update_prompt_visibility};
pub(crate) use text_input::{apply_input_edits, sync_input_cursors, sync_input_policy};
#[path = "input_password.rs"]
mod input_password;
pub(crate) use input_password::{
    InputPasswordView, prepare_password_layout, publish_password_layout, sync_password_views,
};
#[path = "bitmap_rich_text.rs"]
mod bitmap_rich_text;
pub(crate) use bitmap_rich_text::{BitmapRichText, sync_bitmap_rich_text};

#[path = "movie_sources.rs"]
mod movie_sources;
pub(crate) use movie_sources::{MovieSource, sync_movie_sources};
#[path = "stage.rs"]
mod stage;
pub(crate) use stage::sync_stage_hierarchy;
#[path = "rich_text.rs"]
mod rich_text;
#[cfg(test)]
pub(crate) use rich_text::InlineMovieClipPlayer;
pub(crate) use rich_text::LinkHit;
pub(crate) use rich_text::{
    InlineImage, RichTextBlock, advance_inline_movie_clips, finalize_inline_images,
    sync_inline_image_hierarchy, sync_rich_text,
};
pub use rich_text::{InlineImageOwner, InlineImageParts};

#[path = "button_popups.rs"]
mod button_popups;
#[path = "loader_components.rs"]
mod loader_components;
#[path = "popup_widgets.rs"]
mod popup_widgets;
#[path = "templates.rs"]
mod templates;
pub(crate) use loader_components::sync_loader_components;
#[path = "loader_status.rs"]
mod loader_status;
pub(crate) use loader_status::sync_loader_states;
pub(crate) use loader_status::sync_loader_status;
pub use loader_status::{GuiLoaderFailure, GuiLoaderState};
pub(crate) use popup_widgets::cleanup_retained_popups;
#[path = "popup_menu.rs"]
mod popup_menu;
pub use popup_menu::emit_menu_actions;
pub use popup_menu::{GuiMenuAction, PopupMenuTemplates};

#[path = "content.rs"]
pub(crate) mod content;

#[cfg(test)]
#[path = "resource_tests.rs"]
mod resource_tests;

/// FairyGUI object event delivered to the matching Bevy UI entity observer.
///
/// The event is triggered once for its original object and then once per logical
/// ancestor with `event.bubbling` set. Attach an observer directly to an entity:
/// `commands.entity(entity).observe(on_gui_event)`.
#[derive(EntityEvent, Debug, Clone)]
pub struct GuiEvent {
    pub entity: Entity,
    /// Original control view, independent of the current bubbling recipient.
    /// None for native notifications whose original object has no view.
    pub target: Option<Entity>,
    /// Original target, current logical bubbling recipient and pointer payload.
    pub event: fairygui_core::object::UiEvent,
}

/// One rich-text link activation, published before `GuiSystems::Respond`.
/// The href is preserved verbatim (including `event:`). Applications decide how
/// to handle it; the runtime never opens a browser. Core events also bubble.
#[derive(Message, Debug, Clone)]
pub struct GuiLink {
    /// Bevy control entity that originally activated the link.
    pub entity: Entity,
    pub href: String,
    pub event: fairygui_core::object::UiEvent,
}

/// Native input notification. FocusIn/FocusOut correspond to FOCUS/BLUR;
/// Input/Change/Enter retain their native timing and do not bubble.
#[derive(EntityEvent, Message, Debug, Clone)]
pub struct GuiTextInput {
    /// Input view that emitted this notification, including for compound labels.
    pub entity: Entity,
    pub event: fairygui_core::object::UiEvent,
    /// The real committed value, including for password fields; preedit is excluded.
    pub value: String,
}

/// One completed click on a list row, published after selection. Virtual rows
/// report their logical item index, independent of the current physical child.
/// Native CLICK_ITEM is emitted on the list even when selection did not change.
#[derive(EntityEvent, Message, Debug, Clone)]
pub struct GuiListItemClick {
    /// List/tree view receiving this target-only event.
    pub entity: Entity,
    pub event: fairygui_core::object::UiEvent,
    /// Clicked row view. Use this directly even when selection is disabled or
    /// the click toggled the row out of a multiple selection.
    pub item: Entity,
    pub index: usize,
    pub shift: bool,
    pub ctrl: bool,
}

#[derive(Debug, Clone)]
struct PendingListItemClick {
    event: fairygui_core::object::UiEvent,
    item: GObjectId,
    index: usize,
    shift: bool,
    ctrl: bool,
    views: Option<(Entity, Entity)>,
}

/// Native target-only pull release, published before application Respond.
/// Assign header/footer locks there before the release tween is finalized.
#[derive(Message, Debug, Clone)]
pub struct GuiPullRelease {
    pub entity: Entity,
    pub event: fairygui_core::object::UiEvent,
}

pub fn emit_pull_release_messages(
    mut runtime: NonSendMut<GuiRuntime>,
    mut messages: MessageWriter<GuiPullRelease>,
) {
    for (target, direction) in runtime.store_mut().drain_pull_releases() {
        let Some(entity) = runtime.entity_of(target) else {
            continue;
        };
        let kind = match direction {
            fairygui_core::scroll::PullRelease::Down => {
                fairygui_core::object::EventKind::PullDownRelease
            }
            fairygui_core::scroll::PullRelease::Up => {
                fairygui_core::object::EventKind::PullUpRelease
            }
        };
        let event = fairygui_core::object::UiEvent::new(kind, target);
        runtime.events.push(event.clone());
        messages.write(GuiPullRelease { entity, event });
    }
}

pub fn emit_list_item_messages(
    mut commands: Commands,
    mut runtime: NonSendMut<GuiRuntime>,
    mut messages: MessageWriter<GuiListItemClick>,
    entities: Query<(Entity, &GObjectRef)>,
) {
    let clicks = std::mem::take(&mut runtime.list_clicks);
    let resolve = |object| {
        runtime.entity_of(object).or_else(|| {
            entities
                .iter()
                .find_map(|(entity, reference)| (reference.0 == object).then_some(entity))
        })
    };
    for pending in clicks {
        let Some((entity, item)) = pending
            .views
            .or_else(|| resolve(pending.event.target).zip(resolve(pending.item)))
        else {
            continue;
        };
        let event = GuiListItemClick {
            entity,
            item,
            event: pending.event,
            index: pending.index,
            shift: pending.shift,
            ctrl: pending.ctrl,
        };
        messages.write(event.clone());
        commands.trigger(event);
    }
}

pub fn emit_link_messages(
    mut runtime: NonSendMut<GuiRuntime>,
    mut messages: MessageWriter<GuiLink>,
) {
    let links = std::mem::take(&mut runtime.links);
    for event in links {
        if let Some(entity) = runtime.entity_of(event.target)
            && let fairygui_core::object::EventData::Text(href) = &event.data
        {
            messages.write(GuiLink {
                entity,
                href: href.clone(),
                event,
            });
        }
    }
}

/// Transition label notification, published with the owner view after Sync.
#[derive(Message, Debug, Clone)]
pub struct GuiTransitionHook {
    pub entity: Entity,
    pub hook: fairygui_core::transition::TransitionHook,
}

/// 通过 set_transition_hook 绑定的用户编号；原有标签消息仍独立发布。
#[derive(Message, Debug, Clone)]
pub struct GuiTransitionCallback {
    pub entity: Entity,
    pub callback: fairygui_core::transition::TransitionCallback,
}

/// One naturally completed playback, after all repetitions and nested tasks.
/// Published after Sync, including playbacks started and finished in that Sync.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct GuiTransitionComplete {
    pub entity: Entity,
    pub transition: String,
    pub play_generation: u64,
}

pub fn emit_transition_completions(
    mut runtime: NonSendMut<GuiRuntime>,
    mut completed: MessageWriter<GuiTransitionComplete>,
) {
    for completion in runtime.store_mut().take_transition_completions() {
        if let Some(entity) = runtime.entity_of(completion.owner) {
            completed.write(GuiTransitionComplete {
                entity,
                transition: completion.transition,
                play_generation: completion.play_generation,
            });
        }
    }
}

/// Finite movie playback completion on the control view, after Sync refreshes
/// its final frame and state. Available to per-entity observers and readers.
#[derive(Message, EntityEvent, Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuiAnimationComplete {
    pub entity: Entity,
}

pub fn emit_animation_completions(
    mut runtime: NonSendMut<GuiRuntime>,
    mut completed: MessageWriter<GuiAnimationComplete>,
    players: Query<&crate::movie::MovieClipPlayer>,
    mut commands: Commands,
) {
    for object in runtime.store_mut().take_animation_completions() {
        if let Some(entity) = runtime.entity_of(object)
            && players
                .get(entity)
                .is_ok_and(crate::movie::MovieClipPlayer::is_ended)
        {
            let event = GuiAnimationComplete { entity };
            completed.write(event);
            commands.trigger(event);
        }
    }
}

pub fn emit_transition_hooks(
    mut runtime: NonSendMut<GuiRuntime>,
    mut hooks: MessageWriter<GuiTransitionHook>,
) {
    for hook in runtime.store_mut().take_transition_hooks() {
        if let Some(entity) = runtime.entity_of(hook.owner) {
            hooks.write(GuiTransitionHook { entity, hook });
        }
    }
}

pub fn emit_transition_callbacks(
    mut runtime: NonSendMut<GuiRuntime>,
    mut callbacks: MessageWriter<GuiTransitionCallback>,
) {
    for callback in runtime.store_mut().take_transition_callbacks() {
        if let Some(entity) = runtime.entity_of(callback.hook.owner) {
            callbacks.write(GuiTransitionCallback { entity, callback });
        }
    }
}

/// 界面运行期。
#[derive(Default)]
pub struct GuiRuntime {
    pub(crate) compositing: bool,
    store: ObjectStore,
    entities: HashMap<GObjectId, Entity>,
    roots: Vec<GuiRoot>,
    ecs_roots: HashMap<Entity, EcsRootBinding>,
    object_command_cursor: bevy::ecs::message::MessageCursor<GuiObjectCommand>,
    pending_object_commands: Vec<(ObjectCommandSource, GuiObjectCommand)>,
    ecs_windows: HashMap<Entity, windows::Binding>,
    global_wait: global_wait::GlobalWait,
    /// 已派发但尚未被取走的事件。
    events: fairygui_core::object::EventQueue,
    links: Vec<fairygui_core::object::UiEvent>,
    input_events: Vec<GuiTextInput>,
    list_clicks: Vec<PendingListItemClick>,
    pub(crate) focused_input: Option<GObjectId>,
    pub(crate) input_focus_request: Option<Option<GObjectId>>,
    pub(crate) input_submit: Option<(GObjectId, bool)>,
    /// Bevy entity events waiting for `GuiSystems::Publish`.
    gui_events: Vec<fairygui_core::object::UiEvent>,
    /// 进行中的分帧构建。
    pending_builds: Vec<PendingBuild>,
    /// 多语言翻译表。文本在转成实体时按它改写。
    translations: fairygui_core::translation::TranslationTable,
    /// 对象池。列表虚拟化等场景用它复用对象，避免反复创建与销毁。
    pool: fairygui_core::pool::ObjectPool,
    /// 根节点。界面与窗口都是它的子对象，因此共享同一套层级顺序。
    root: Option<fairygui_core::root::Root>,
    /// 已创建的窗口，按显示顺序排列。
    windows: Vec<fairygui_core::window::Window>,
    /// 运行时新加入对象树、但还没有实体的对象。
    pending_spawn: Vec<GObjectId>,
    /// 当前的弹出菜单。数据部分跨次保留，切换的只是承载对象。
    popup_menu: fairygui_core::popup::PopupMenu,
    popups: Vec<GObjectId>,
    retained_combos: HashMap<GObjectId, popup_widgets::RetainedCombo>,
    retained_menu: Option<popup_menu::RetainedMenu>,
    pending_menu_click: Option<popup_menu::PendingMenuClick>,
    menu_actions: Vec<GuiMenuAction>,
    loader_components: HashMap<GObjectId, loader_components::LoadedComponent>,
    failed_loader_components: std::collections::HashSet<(GObjectId, String)>,
    loader_status: HashMap<GObjectId, loader_status::LoaderStatus>,
    loader_error_template: Option<String>,
    loader_error_pool: HashMap<String, std::collections::VecDeque<GObjectId>>,
    loader_retries: std::collections::HashSet<GObjectId>,
    package_handles: HashMap<String, Handle<FuiAsset>>,
    object_assets: HashMap<GObjectId, Handle<FuiAsset>>,
    just_closed_popups: Vec<GObjectId>,
    /// 拖拽状态：被拖对象、跟手图标与当前命中的放置目标。
    drag_drop: fairygui_core::drag_drop::DragDrop,
    pub(crate) object_drag_bindings: HashMap<GObjectId, object_drag::Binding>,
    pub(crate) active_object_drags: HashMap<GObjectId, object_drag::Active>,
    pub(crate) drag_source_bindings: std::collections::HashSet<GObjectId>,
    ecs_drag: Option<ecs_drag_drop::Session>,
    drop_targets: HashMap<GObjectId, Entity>,
    drop_events: Vec<ecs_drag_drop::GuiDrop>,
    pub(crate) drag_touch_id: Option<u64>,
    /// 已加载的包。跨包引用（`ui://别的包/条目`）靠它解析。
    registry: fairygui_core::package::PackageRegistry,
    /// 界面文本使用的字体。
    ///
    /// 由 `sync_gui_font` 在构建 ECS 根之前从 [`GuiFont`] 同步进来：`create_root`
    /// 这类不在系统里跑的路径拿不到资源，只能从这里取。
    font: Option<Handle<Font>>,
    pub(crate) text_measurer: std::rc::Rc<std::cell::RefCell<text_measure::TextMeasurer>>,
    /// 登记为虚拟化的列表。
    virtual_lists: Vec<GObjectId>,
    /// 滚动过、等待使用方处理的虚拟列表。
    scrolled_virtual_lists: Vec<GObjectId>,
    virtual_renderers: HashMap<GObjectId, virtual_lists::VirtualCallbacks>,
}

/// 一次进行中的分帧构建。
struct PendingBuild {
    handle: Handle<FuiAsset>,
    builder: fairygui_core::async_build::AsyncBuilder,
    ctx: fairygui_core::package::LoadContext,
}

struct GuiRoot {
    handle: Handle<FuiAsset>,
    object: GObjectId,
    spawned: bool,
    attached: bool,
}

impl std::fmt::Debug for GuiRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuiRuntime")
            .field("objects", &self.store.len())
            .field("entities", &self.entities.len())
            .field("roots", &self.roots.len())
            .finish()
    }
}

impl GuiRuntime {
    /// Execute native fireClick internal behavior. No GuiEvent/Click is emitted,
    /// and linked popups are unaffected. Check/Radio changes emit StateChanged
    /// immediately, without a pointer payload. Common feedback is enabled by true.
    pub fn fire_button_click(&mut self, button: GObjectId, down_effect: bool) -> bool {
        let Some(changed) = self.store.fire_button_click(button, down_effect) else {
            return false;
        };
        if changed {
            self.dispatch(fairygui_core::object::EventKind::StateChanged, button);
        }
        true
    }

    /// Create the declared data items of list controls from their default item
    /// template.  FairyGUI stores list data separately from the visual child
    /// objects; the renderer needs both before it can apply titles and layout.
    fn materialize_list_items(&mut self, package: &FuiPackage, root: GObjectId) {
        let mut lists = vec![root];
        self.store
            .for_each_descendant(root, &mut |id| lists.push(id));

        for list in lists {
            if self.store.is_virtual_list(list) {
                continue;
            }
            let (default_item, item_urls, existing) = {
                let Some(object) = self.store.object(list) else {
                    continue;
                };
                let data = match object.control() {
                    ControlData::List(data) => data,
                    ControlData::Tree(data) => &data.list,
                    _ => continue,
                };
                (
                    data.default_item.clone(),
                    data.items
                        .iter()
                        .map(|item| item.url.clone().or_else(|| data.default_item.clone()))
                        .collect::<Vec<_>>(),
                    self.store.list_item_count(list),
                )
            };

            if item_urls.len() <= existing {
                continue;
            }

            for url in item_urls.into_iter().skip(existing) {
                let Some(url) = url.or_else(|| default_item.clone()) else {
                    continue;
                };
                let owner = self
                    .store
                    .object(list)
                    .and_then(|object| object.source_package_id())
                    .filter(|id| *id != package.id())
                    .and_then(|id| self.registry.get_by_id(id))
                    .cloned()
                    .unwrap_or_else(|| std::sync::Arc::new(package.clone()));
                let Some(template) = self.resolve_template(&owner, &url) else {
                    tracing::warn!(list = ?list, %url, "list item template cannot be resolved");
                    continue;
                };
                let Some(item) = self.create_template(&template) else {
                    tracing::warn!(list = ?list, %url, "list item template cannot be created");
                    continue;
                };
                self.store.set_parent(item, Some(list));
            }
        }
    }

    /// Finish model layout for both roots and dynamically queued subtrees.
    fn prepare_subtree(&mut self, package: &FuiPackage, object: GObjectId) {
        self.materialize_list_items(package, object);
        self.store.initialize_list_selections(object);
        self.layout_prepared_subtree(object);
        attach_scroll_bars(&mut self.store, &self.registry, package, object);
        // Bar resources may add nested lists; finish layout before applying a
        // page controller's home page against the final content/view sizes.
        layout_scroll_bars(&mut self.store, object);
        self.layout_prepared_subtree(object);
        self.store.initialize_scroll_page_controllers(object);
    }

    fn layout_prepared_subtree(&mut self, object: GObjectId) {
        let mut objects = vec![object];
        self.store
            .for_each_descendant(object, &mut |id| objects.push(id));
        for &id in &objects {
            self.store.apply_title_of(id);
        }
        for &id in &objects {
            self.store.layout_group(id);
        }
        for &id in &objects {
            if self.store.is_virtual_list(id) {
                continue;
            }
            for index in 0..self.store.list_item_count(id) {
                self.store.apply_list_item(id, index);
            }
        }
        for id in objects {
            if matches!(
                self.store.object(id).map(|o| o.control()),
                Some(ControlData::Tree(_))
            ) {
                for index in 0..self.store.list_item_count(id) {
                    self.store.apply_tree_item(id, index);
                }
                self.store.initialize_tree_cells(id);
            }
            self.store.layout_list(id);
            self.store.layout_component_scroll_content(id);
        }
    }

    /// 由包创建一个顶层界面，返回界面编号。
    pub fn create_root(
        &mut self,
        handle: Handle<FuiAsset>,
        package: &FuiPackage,
        item_name: &str,
    ) -> Option<usize> {
        let index = package.item_index_by_name(item_name)?;
        self.register_package(handle.clone(), package);
        let object = match fairygui_core::widget::create_from_package_with_registry(
            &mut self.store,
            package,
            index,
            &self.registry.load_context(package),
            &self.registry,
        ) {
            Ok(object) => object,
            Err(error) => {
                tracing::error!(item_name, %error, "Failed to build FairyGUI root");
                return None;
            }
        };

        self.remember_asset_tree(object, &handle);
        self.roots.push(GuiRoot {
            handle,
            object,
            spawned: false,
            attached: true,
        });
        Some(self.roots.len() - 1)
    }

    /// 顶层界面的根对象。
    pub fn root_object(&self, handle: usize) -> Option<GObjectId> {
        self.roots
            .get(handle)
            .map(|root| root.object)
            .filter(|id| self.store.contains(*id))
    }

    /// Resolve the root currently owned by an ECS entity with [`FairyGuiRoot`].
    pub fn ecs_root_object(&self, owner: Entity) -> Option<GObjectId> {
        let handle = self.ecs_roots.get(&owner)?.handle?;
        self.root_object(handle)
    }

    /// Resolve the manual root slot for an ECS-owned UI root.
    pub fn ecs_root_handle(&self, owner: Entity) -> Option<usize> {
        let handle = self.ecs_roots.get(&owner)?.handle?;
        self.root_object(handle).map(|_| handle)
    }

    pub(crate) fn ecs_root_pending(&self, owner: Entity) -> bool {
        self.ecs_roots
            .get(&owner)
            .is_some_and(|binding| binding.handle.is_none() && !binding.failed)
    }

    /// 撤下/重新挂载顶层界面，保留对象、实体及资源编号。
    pub fn set_root_attached(&mut self, handle: usize, attached: bool) -> bool {
        let Some(root) = self.roots.get_mut(handle) else {
            return false;
        };
        if !self.store.contains(root.object) {
            return false;
        }
        if root.attached == attached
            && (!root.spawned || self.store.is_on_stage(root.object) == attached)
        {
            return true;
        }
        root.attached = attached;
        if attached {
            if root.spawned {
                self.store.attach_to_stage(root.object);
            }
        } else {
            self.store.detach_from_parent(root.object);
            self.store.detach_from_stage(root.object);
        }
        true
    }

    /// 界面数量。
    pub fn root_count(&self) -> usize {
        self.roots.len()
    }

    /// 设置多语言翻译表。
    ///
    /// 表在**创建界面时**生效：编辑器里写成 `!key!` 的文本会被替换成译文。
    /// 对已经创建好的界面，需要重建才会应用新表。
    pub fn set_translations(&mut self, table: fairygui_core::translation::TranslationTable) {
        self.translations = table;
    }

    /// 翻译表。
    pub fn translations(&self) -> &fairygui_core::translation::TranslationTable {
        &self.translations
    }

    /// 按翻译表改写文本。
    pub fn translate(&self, text: &str) -> String {
        self.translations.translate(text)
    }

    /// 对象池。
    pub fn pool(&self) -> &fairygui_core::pool::ObjectPool {
        &self.pool
    }

    /// 可变的对象池。列表虚拟化等场景用它回收与取用对象。
    pub fn pool_mut(&mut self) -> &mut fairygui_core::pool::ObjectPool {
        &mut self.pool
    }

    /// 切换树节点的展开状态，并同步各项可见性。
    ///
    /// 收起来的节点是**隐藏而不是销毁**，因此再次展开不需要重建。
    /// 返回状态是否发生变化。
    /// 展开或收起树里的一个节点。
    ///
    /// 展开状态一变，哪些节点可见、各自排在第几行都会变，因此这里顺带重新
    /// 布局——否则项的位置会停留在折叠时的样子。
    pub fn toggle_tree_node(&mut self, tree: GObjectId, index: usize) -> bool {
        let changed = self.store.toggle_tree_node(tree, index);
        if changed {
            self.store.layout_list(tree);
        }
        changed
    }

    /// 展开树里的全部文件夹。
    pub fn expand_tree(&mut self, tree: GObjectId) {
        self.store.expand_tree(tree);
        self.store.layout_list(tree);
    }

    /// 展开到指定节点，使它变为可见。沿途的祖先都会被展开。
    pub fn expand_tree_to(&mut self, tree: GObjectId, index: usize) -> bool {
        let changed = self.store.expand_tree_to(tree, index);
        if changed {
            self.store.layout_list(tree);
        }
        changed
    }

    /// 计算虚拟列表当前应当存在的项范围。
    ///
    /// 滚动位置取自列表自己的滚动容器，`buffer` 是两端额外多取的项数。
    /// 使用方据此创建缺少的项、回收范围外的项——控件本身不会自动驱动。
    pub fn virtual_list_range(
        &self,
        list: GObjectId,
        item_sizes: &[fairygui_core::geom::Size],
        buffer: usize,
    ) -> Option<std::ops::Range<usize>> {
        let object = self.store.object(list)?;
        let data = match object.control() {
            fairygui_core::widget::ControlData::List(data) => data,
            _ => return None,
        };

        let view_size = object.size();
        let scroll = object
            .control()
            .component()
            .and_then(|component| component.scroll_pane.as_ref())
            .map(|pane| pane.display_pos())
            .unwrap_or_default();

        let positions = data.layout_items(item_sizes, view_size);
        Some(data.visible_range(&positions, item_sizes, view_size, scroll, buffer))
    }

    /// 回收虚拟列表里落到范围外的项对象。
    ///
    /// 配合 [`Self::virtual_list_range`] 使用。返回被回收的数量。
    pub fn recycle_virtual_items(
        &mut self,
        list: GObjectId,
        keep: std::ops::Range<usize>,
        url: &str,
    ) -> usize {
        let pool = &mut self.pool;
        self.store.recycle_items_outside(list, keep, pool, url)
    }

    /// 收起树里的全部导出文件夹，保留顶层项。
    pub fn collapse_tree(&mut self, tree: GObjectId) {
        self.store.collapse_tree(tree);
    }

    /// 树当前可见的节点序号。
    pub fn visible_tree_nodes(&self, tree: GObjectId) -> Option<Vec<usize>> {
        match self.store.object(tree).map(|object| object.control()) {
            Some(fairygui_core::widget::ControlData::Tree(data)) => Some(data.visible_nodes()),
            _ => None,
        }
    }

    /// 分帧创建界面。
    ///
    /// 与 [`Self::create_root`] 的区别是不在调用当帧建完整棵树，而是每帧
    /// 构建若干条目，避免启动瞬间卡顿。`budget` 是每帧最多构建的条目数。
    ///
    /// 返回一个编号，用于查询这次构建的进度。
    pub fn create_roots_async(
        &mut self,
        handle: Handle<FuiAsset>,
        package: &fairygui_core::package::FuiPackage,
        names: &[&str],
        budget: usize,
    ) -> Result<usize, fairygui_core::Error> {
        let builder = fairygui_core::async_build::AsyncBuilder::from_names(package, names, budget)?;
        self.register_package(handle.clone(), package);

        self.pending_builds.push(PendingBuild {
            handle,
            builder,
            ctx: self.registry.load_context(package),
        });
        Ok(self.pending_builds.len() - 1)
    }

    /// 某次分帧构建剩余待建的条目数。
    pub fn pending_build_remaining(&self, index: usize) -> Option<usize> {
        self.pending_builds
            .get(index)
            .map(|build| build.builder.remaining())
    }

    /// 是否还有分帧构建未完成。
    pub fn has_pending_builds(&self) -> bool {
        !self.pending_builds.is_empty()
    }

    /// 推进所有分帧构建。已完成的会转成正式界面。
    fn advance_builds(&mut self, assets: &Assets<FuiAsset>) {
        if self.pending_builds.is_empty() {
            return;
        }

        let mut finished = Vec::new();

        for index in 0..self.pending_builds.len() {
            let Some(asset) = assets.get(&self.pending_builds[index].handle) else {
                // 资产还没加载好，下一帧再试。
                continue;
            };

            let build = &mut self.pending_builds[index];
            let done = build.builder.advance_with_registry(
                &mut self.store,
                &asset.package,
                &build.ctx,
                &self.registry,
            );

            if done {
                let roots = build.builder.roots().to_vec();
                finished.push((index, roots));
            }
        }

        // 从后往前收尾，避免下标错位。
        for (index, roots) in finished.into_iter().rev() {
            let build = self.pending_builds.remove(index);
            for object in roots {
                self.remember_asset_tree(object, &build.handle);
                self.roots.push(GuiRoot {
                    handle: build.handle.clone(),
                    object,
                    spawned: false,
                    attached: true,
                });
            }
        }
    }

    /// 只读访问对象存储。
    pub fn store(&self) -> &ObjectStore {
        &self.store
    }

    /// 可变访问对象存储。
    pub fn store_mut(&mut self) -> &mut ObjectStore {
        &mut self.store
    }

    /// 对象对应的实体。
    pub fn entity_of(&self, object: GObjectId) -> Option<Entity> {
        self.entities.get(&object).copied()
    }

    /// 销毁一个界面：对象、实体与映射一并回收。
    pub fn remove_root(&mut self, commands: &mut Commands, handle: usize) {
        let Some(root) = self.roots.get(handle) else {
            return;
        };
        let object = root.object;

        let mut removed = vec![object];
        self.store
            .for_each_descendant(object, &mut |id| removed.push(id));
        let _ = self.store.remove(object);
        self.preserve_surviving_views(commands);

        // Bevy despawns Children recursively. Remove leaves first so no queued
        // command addresses an entity already removed by its parent.
        for id in removed.into_iter().rev() {
            if let Some(entity) = self.entities.remove(&id) {
                commands.entity(entity).despawn();
            }
        }
    }

    // ---- 根节点与窗口 ----

    /// 把对象排进「待生成实体」的队列。
    ///
    /// 运行期新加入对象树的窗口需要自己的实体，但它们不是构建阶段产生的，
    /// 因此走这个队列由专门系统补生成。
    fn queue_spawn(&mut self, object: GObjectId) {
        if !self.pending_spawn.contains(&object) {
            self.pending_spawn.push(object);
        }
    }

    /// 对象是否在等待生成实体。
    pub fn is_pending_spawn(&self, object: GObjectId) -> bool {
        self.pending_spawn.contains(&object)
    }

    /// 把一棵子树里还没有实体的对象排进生成队列。
    ///
    /// 动态增删子对象后调用：新加进来的那支要补上实体。已经生成过的会被
    /// 跳过，因此重复调用是安全的。入队顺序是父先于子，轮到子对象时父
    /// 实体必然已经就绪。
    ///
    /// 容器被标记 `CHILDREN` 脏时运行期会自动调用，因此手工调用一般只
    /// 用在「先改对象树、再让运行期补实体」的场景。
    pub fn queue_subtree(&mut self, root: GObjectId) {
        if self.store.object(root).is_none() {
            return;
        }

        if !self.entities.contains_key(&root) && !self.pending_spawn.contains(&root) {
            self.pending_spawn.push(root);
        }

        let mut descendants = Vec::new();
        self.store
            .for_each_descendant(root, &mut |id| descendants.push(id));

        for id in descendants {
            if !self.entities.contains_key(&id) && !self.pending_spawn.contains(&id) {
                self.pending_spawn.push(id);
            }
        }
    }

    /// 确保根节点存在，返回根容器对象。
    ///
    /// 根节点是一片专门的容器，普通界面与窗口都挂在它下面，因此层与层之间的
    /// 顺序、模态状态才能统一维护。第一次需要时自动建立。
    pub fn ensure_root(&mut self, size: fairygui_core::geom::Size) -> GObjectId {
        if let Some(root) = &self.root {
            return root.owner();
        }

        let owner = self
            .store
            .create(fairygui_core::package::ObjectType::Component);
        self.store.set_size(owner, size.width, size.height);
        self.store.attach_to_stage(owner);
        self.root = Some(fairygui_core::root::Root::new(owner));
        self.queue_spawn(owner);
        owner
    }

    /// 首个界面的资产句柄。
    ///
    /// 仅作为未声明来源的辅助对象的后备；有来源的对象使用 asset_handle_for。
    pub fn primary_handle(&self) -> Option<Handle<FuiAsset>> {
        self.roots.first().map(|root| root.handle.clone())
    }

    fn remember_asset_tree(&mut self, object: GObjectId, handle: &Handle<FuiAsset>) {
        let package_id = self
            .store
            .object(object)
            .and_then(|o| o.source_package_id())
            .map(str::to_owned);
        let mut pending = vec![(object, handle.clone(), package_id)];
        while let Some((object, inherited, inherited_package)) = pending.pop() {
            let Some(gobject) = self.store.object(object) else {
                continue;
            };
            let declared_package = gobject
                .source_package_id()
                .map(str::to_owned)
                .or(inherited_package.clone());
            let source = self
                .object_assets
                .entry(object)
                .or_insert_with(|| {
                    if declared_package != inherited_package {
                        declared_package
                            .as_deref()
                            .and_then(|id| self.package_handles.get(id))
                            .cloned()
                            .unwrap_or(inherited)
                    } else {
                        inherited
                    }
                })
                .clone();
            pending.extend(
                gobject
                    .children()
                    .iter()
                    .map(|child| (*child, source.clone(), declared_package.clone())),
            );
        }
    }

    /// Returns the asset owning an object, including dynamically opened demo roots.
    pub fn asset_handle_for(&self, object: GObjectId) -> Option<Handle<FuiAsset>> {
        let mut current = Some(object);
        while let Some(id) = current {
            if let Some(handle) = self.object_assets.get(&id) {
                return Some(handle.clone());
            }
            if let Some(root) = self.roots.iter().find(|root| root.object == id) {
                return Some(root.handle.clone());
            }
            current = self.store.object(id).and_then(|item| item.parent());
        }
        self.primary_handle()
    }

    /// 根容器对象。界面与窗口都挂在它下面。
    pub fn root_container(&self) -> Option<GObjectId> {
        self.root.as_ref().map(|root| root.owner())
    }

    /// 是否处于模态状态。为真时下层不应响应输入。
    pub fn is_modal(&self) -> bool {
        self.root.as_ref().is_some_and(|root| root.is_modal())
    }

    /// 窗口数量。
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// 取窗口。
    pub fn window(&self, index: usize) -> Option<&fairygui_core::window::Window> {
        self.windows.get(index)
    }

    /// 可变地取窗口。
    pub fn window_mut(&mut self, index: usize) -> Option<&mut fairygui_core::window::Window> {
        self.windows.get_mut(index)
    }

    /// 从包里创建并显示一个窗口，返回窗口编号。
    ///
    /// 窗口对象会成为根节点弹窗层的子对象；`modal` 为真时它会挡住下层输入，
    /// 直到被关闭。窗框与关闭按钮按编辑器约定名识别。
    pub fn show_window(
        &mut self,
        package: &FuiPackage,
        item_name: &str,
        modal: bool,
        root_size: fairygui_core::geom::Size,
    ) -> Option<usize> {
        self.ensure_root(root_size);
        self.construct_window(package, item_name, modal, true, true)
    }

    fn construct_window(
        &mut self,
        package: &FuiPackage,
        item_name: &str,
        modal: bool,
        visible: bool,
        center: bool,
    ) -> Option<usize> {
        let root_object = self.root.as_ref()?.owner();
        let index = package.item_index_by_name(item_name)?;
        let owner = fairygui_core::widget::create_from_package_with_registry(
            &mut self.store,
            package,
            index,
            &self.registry.load_context(package),
            &self.registry,
        )
        .ok()?;
        if let Some(handle) = self.package_handles.get(package.id()).cloned() {
            self.remember_asset_tree(owner, &handle);
            // Standalone ECS windows can be the first GUI in the app. The
            // synthetic stage needs an asset fallback before it can spawn.
            self.object_assets.entry(root_object).or_insert(handle);
        }
        let mut window = fairygui_core::window::Window::new(owner);
        window.attach_parts(&mut self.store);
        window.set_modal(self.root.as_mut()?, modal);
        if center {
            window.center_on(&mut self.store, self.root.as_ref()?);
        }
        if visible {
            window.show(&mut self.store, self.root.as_mut()?, modal);
        }
        self.windows.push(window);
        self.queue_subtree(owner);
        self.queue_spawn(root_object);
        if visible {
            self.dispatch(fairygui_core::object::EventKind::WindowShown, owner);
        }
        Some(self.windows.len() - 1)
    }

    /// 隐藏窗口，保留对象以便再次显示。
    pub fn hide_window(&mut self, index: usize) -> bool {
        let Some(window) = self.windows.get(index).cloned() else {
            return false;
        };
        if !window.is_showing() {
            return true;
        }
        let Some(root) = self.root.as_mut() else {
            return false;
        };

        let mut window = window;
        let owner = window.owner();
        window.close_modal_wait(&mut self.store, 0);
        window.hide(&mut self.store, root);
        self.windows[index] = window;

        self.dispatch(fairygui_core::object::EventKind::WindowHide, owner);
        true
    }

    /// 重新显示保留对象的窗口，恢复原先的模态设置。
    pub fn show_existing_window(&mut self, index: usize) -> bool {
        let Some(mut window) = self.windows.get(index).cloned() else {
            return false;
        };
        if !self.store.contains(window.owner()) {
            return false;
        }
        if window.is_showing() {
            return true;
        }
        let Some(root) = self.root.as_mut() else {
            return false;
        };
        let owner = window.owner();
        let modal = window.is_modal();
        window.show(&mut self.store, root, modal);
        self.windows[index] = window;
        self.queue_subtree(owner);
        self.dispatch(fairygui_core::object::EventKind::WindowShown, owner);
        true
    }

    /// 关闭窗口：从根节点移除、销毁对象，并回收它的实体。
    pub fn close_window(&mut self, commands: &mut Commands, index: usize) -> bool {
        let Some(window) = self.windows.get(index).cloned() else {
            return false;
        };
        let owner = window.owner();
        let mut removed = vec![owner];
        self.store
            .for_each_descendant(owner, &mut |id| removed.push(id));
        let retained_wait = window
            .modal_wait_pane()
            .filter(|&pane| !removed.contains(&pane));
        if let Some(pane) = retained_wait {
            removed.push(pane);
            self.store
                .for_each_descendant(pane, &mut |id| removed.push(id));
            self.store.remove(pane);
        }

        // 事件要在销毁之前派发：派发会沿对象树冒泡，对象没了就无处可派。
        self.dispatch(fairygui_core::object::EventKind::WindowHide, owner);

        if let Some(root) = self.root.as_mut() {
            root.remove(&mut self.store, owner);
        }
        self.windows.remove(index);

        // 整棵子树的实体一并回收。
        if self.store.contains(owner) {
            self.store.remove(owner);
        }
        self.preserve_surviving_views(commands);
        for object in removed.into_iter().rev() {
            if let Some(entity) = self.entities.remove(&object) {
                commands.entity(entity).despawn();
            }
            self.pending_spawn.retain(|pending| *pending != object);
        }
        true
    }

    /// 对象是否就是 `ancestor` 或它的后代。
    fn is_descendant_of_or_self(&self, object: GObjectId, ancestor: GObjectId) -> bool {
        let mut current = Some(object);
        while let Some(id) = current {
            if id == ancestor {
                return true;
            }
            current = self.store.object(id).and_then(|item| item.parent());
        }
        false
    }

    /// 取包含某个对象的最上层窗口编号。
    ///
    /// 从最上方的窗口往下找，因此窗口重叠时命中的是视觉上压在上面的那个。
    pub fn window_containing(&self, object: GObjectId) -> Option<usize> {
        for index in (0..self.windows.len()).rev() {
            let owner = self.windows[index].owner();
            if self.is_descendant_of_or_self(object, owner) {
                return Some(index);
            }
        }
        None
    }

    /// 最上层模态窗口及其上方弹层中的对象可以响应输入。
    /// 没有模态窗口时恒为真，存在时阻挡其下方的层。
    pub fn is_in_top_modal(&self, object: GObjectId) -> bool {
        if let Some(pane) = self.global_wait_pane() {
            return self.is_descendant_of_or_self(object, pane);
        }
        let Some(modal) = self.root.as_ref().and_then(|root| root.top_modal()) else {
            return true;
        };
        self.is_descendant_of_or_self(object, modal)
            || self.root.as_ref().is_some_and(|root| {
                root.popups()
                    .skip_while(|popup| *popup != modal)
                    .skip(1)
                    .any(|popup| self.is_descendant_of_or_self(object, popup))
            })
    }

    /// 指定窗口的内容组件。
    ///
    /// 窗口创建时会按编辑器约定名自动识别（`frame` / `content` / `closeButton`），
    /// 这里是识别不出来或想改指别处时的手动入口。
    pub fn set_window_content(&mut self, index: usize, content: GObjectId) -> bool {
        let Some(mut window) = self.windows.get(index).cloned() else {
            return false;
        };

        window.set_content(&mut self.store, content);
        self.windows[index] = window;

        // 内容刚成为窗口的子对象，实体要补生成。
        self.queue_spawn(content);
        true
    }

    /// 指定窗口的窗框组件。窗框同时也是默认的拖拽区域。
    pub fn set_window_frame(&mut self, index: usize, frame: GObjectId) -> bool {
        let Some(mut window) = self.windows.get(index).cloned() else {
            return false;
        };

        window.set_frame(&mut self.store, frame);
        self.windows[index] = window;

        self.queue_spawn(frame);
        true
    }

    /// 指定窗口的拖拽区域，通常是窗框里的标题栏。
    pub fn set_window_drag_area(&mut self, index: usize, area: GObjectId) -> bool {
        let Some(window) = self.windows.get_mut(index) else {
            return false;
        };
        window.set_drag_area(area);
        true
    }

    /// 指定窗口的关闭按钮。点它会关掉这个窗口。
    pub fn set_window_close_button(&mut self, index: usize, button: GObjectId) -> bool {
        let Some(window) = self.windows.get_mut(index) else {
            return false;
        };
        window.set_close_button(button);
        true
    }

    /// 设置窗口能否拖动。禁止之后，按住标题栏不再移动窗口。
    pub fn set_window_draggable(&mut self, index: usize, draggable: bool) -> bool {
        let Some(window) = self.windows.get_mut(index) else {
            return false;
        };
        window.set_draggable(draggable);
        true
    }

    /// 重新显示一个已隐藏的窗口。
    pub fn reopen_window(&mut self, index: usize, modal: bool) -> bool {
        let Some(mut window) = self.windows.get(index).cloned() else {
            return false;
        };
        let Some(root) = self.root.as_mut() else {
            return false;
        };

        let owner = window.owner();
        window.show(&mut self.store, root, modal);
        self.windows[index] = window;

        self.dispatch(fairygui_core::object::EventKind::WindowShown, owner);
        true
    }

    /// 调整根容器尺寸。窗口居中之类的计算以它为准，屏幕尺寸变化后应调用。
    pub fn set_root_size(&mut self, size: fairygui_core::geom::Size) -> bool {
        let Some(root) = self.root.as_ref().map(|root| root.owner()) else {
            return false;
        };
        self.store.set_size(root, size.width, size.height);
        true
    }

    /// 为窗口设置模态等待面板。
    ///
    /// 面板通常是从包里创建的组件或图片，长什么样由使用方决定；没有设置
    /// 面板时等待只记录状态，不遮挡任何东西。
    pub fn set_window_modal_wait_pane(&mut self, index: usize, pane: GObjectId) -> bool {
        let Some(window) = self.windows.get_mut(index) else {
            return false;
        };
        window.set_modal_wait_pane(pane);
        true
    }

    /// 让窗口进入模态等待：在内容区上盖一块面板，挡住窗口内的操作。
    ///
    /// `cmd` 是调用方自定的命令编号，结束时据此判断「这次等待是不是我要等的
    /// 那一个」。传 0 表示不限定。
    ///
    /// 这里之所以要包一层，是因为窗口与对象存储同属运行期：直接取
    /// [`Self::window_mut`] 再取 [`Self::store_mut`] 会同时借出两次可变引用。
    pub fn show_window_modal_wait(&mut self, index: usize, cmd: u32) -> bool {
        let Some(mut window) = self.windows.get(index).cloned() else {
            return false;
        };

        window.show_modal_wait(&mut self.store, cmd);
        self.windows[index] = window;

        // 面板刚被挂成窗口的子对象，实体需要补生成。
        if let Some(pane) = self.windows[index].modal_wait_pane() {
            self.queue_spawn(pane);
        }
        true
    }

    /// 结束窗口的模态等待。
    ///
    /// 传入了非零命令编号且与发起时不一致时不做任何事并返回 `false`，
    /// 因此多个并存的请求不会互相把对方的等待关掉。
    pub fn close_window_modal_wait(&mut self, index: usize, cmd: u32) -> bool {
        let Some(mut window) = self.windows.get(index).cloned() else {
            return false;
        };

        let closed = window.close_modal_wait(&mut self.store, cmd);
        self.windows[index] = window;
        closed
    }

    /// 窗口是否正在模态等待。
    pub fn is_window_modal_waiting(&self, index: usize) -> bool {
        self.windows
            .get(index)
            .is_some_and(|window| window.is_modal_waiting())
    }

    /// 登记一个需要虚拟化的列表。
    ///
    /// 登记后**滚动位置一变就会被记下**，使用方每帧取一次变化列表即可，
    /// 不必自己轮询滚动位置。取到之后按需重算可视范围、补齐缺少的项、
    /// 回收范围外的项——这三步都需要使用方提供项尺寸与数据，因此交给它做。
    pub fn register_virtual_list(&mut self, list: GObjectId) {
        if !self.virtual_lists.contains(&list) {
            self.virtual_lists.push(list);
        }
        // 刚登记时也通知一次，否则使用方不知道从哪儿开始补。
        self.note_scrolled(list);
    }

    /// 取消虚拟化登记。
    pub fn unregister_virtual_list(&mut self, list: GObjectId) {
        self.virtual_lists.retain(|item| *item != list);
        self.scrolled_virtual_lists.retain(|item| *item != list);
    }

    /// 是否已登记为虚拟列表。
    pub fn is_virtual_list(&self, list: GObjectId) -> bool {
        self.virtual_lists.contains(&list) || self.store.is_virtual_list(list)
    }

    /// 记下某个虚拟列表滚动过。未登记的列表会被忽略。
    pub fn note_scrolled(&mut self, list: GObjectId) {
        if self.is_virtual_list(list) && !self.scrolled_virtual_lists.contains(&list) {
            self.scrolled_virtual_lists.push(list);
        }
    }

    /// 取走滚动过的虚拟列表。取走后清空，同一次滚动不会重复通知。
    pub fn take_scrolled_virtual_lists(&mut self) -> Vec<GObjectId> {
        std::mem::take(&mut self.scrolled_virtual_lists)
    }

    /// 播放容器上的某个过渡。`times` 为播放次数，0 表示无限循环。
    pub fn play_transition(&mut self, container: GObjectId, name: &str, times: i32) -> bool {
        self.store.play_transition(container, name, times)
    }

    /// 按名称播放过渡，并指定启动延迟（秒）。
    pub fn play_transition_delayed(
        &mut self,
        container: GObjectId,
        name: &str,
        times: i32,
        delay: f32,
    ) -> bool {
        self.store
            .play_transition_delayed(container, name, times, delay)
    }

    /// 停止容器上的某个过渡。
    pub fn stop_transition(&mut self, container: GObjectId, name: &str) -> bool {
        self.store.stop_transition(container, name)
    }

    /// 把容器上的某个过渡定位到指定标签所在的时刻。
    ///
    /// 定位**不等于播放**——定到某处后会停在那里，要显示还得另行播放。
    /// 这正是「先摆好姿势再演」的用法。
    pub fn seek_transition(&mut self, container: GObjectId, name: &str, label: &str) -> bool {
        self.store.seek_transition(container, name, label)
    }

    /// 容器上某个过渡的播放状态：`(是否在播放, 当前时间)`。
    pub fn transition_state(&self, container: GObjectId, name: &str) -> Option<(bool, f32)> {
        self.store.transition_state(container, name)
    }

    /// 容器上所有过渡的名称。使用方据此知道有哪些可以播放。
    pub fn transition_names(&self, container: GObjectId) -> Vec<String> {
        self.store.transition_names(container)
    }

    /// 登记一个已加载的包，返回**尚未登记的依赖包名**。
    ///
    /// 包与包之间靠包名寻址，依赖与主包同目录、文件名就是包名——这是编辑器
    /// 导出的约定，`FuiAsset::dependency_path` 会把它拼成资产路径。拿返回的
    /// 名字加载完依赖后，对依赖再调一次本方法，如此往复直到返回空列表，
    /// 整棵依赖就齐了。
    ///
    /// 库不直接碰资产服务器：加载策略（先加载哪个、要不要并行、失败了怎么办）
    /// 归使用方决定。
    pub fn register_package(
        &mut self,
        handle: Handle<FuiAsset>,
        package: &FuiPackage,
    ) -> Vec<String> {
        self.package_handles
            .entry(package.id().into())
            .or_insert_with(|| handle.clone());
        if self.registry.get_by_name(package.name()).is_none() {
            self.registry.add(
                std::sync::Arc::new(package.clone()),
                handle.id().to_string(),
            );
        }

        package
            .dependencies()
            .iter()
            .filter(|dependency| self.registry.get_by_name(&dependency.name).is_none())
            .map(|dependency| dependency.name.clone())
            .collect()
    }

    /// 设置界面文本使用的字体。
    ///
    /// ECS 界面不必手工调用——`sync_gui_font` 会在创建根前从 [`GuiFont`] 同步。
    /// 但在**系统之外**建界面时（比如 `Startup` 里、或与同步系统没有先后
    /// 约束的同帧调用），最好先设一次，否则那批界面会拿到空字体。
    pub fn set_font(&mut self, font: Option<Handle<Font>>) {
        self.font = font;
    }

    /// 界面文本当前使用的字体。
    pub fn font(&self) -> Option<&Handle<Font>> {
        self.font.as_ref()
    }

    /// 已登记的包。
    pub fn registry(&self) -> &fairygui_core::package::PackageRegistry {
        &self.registry
    }

    /// Apply global scroll defaults to roots created after this call.
    pub fn set_scroll_defaults(&mut self, defaults: fairygui_core::scroll::ScrollDefaults) {
        self.registry.set_scroll_defaults(defaults);
    }

    /// 把音效地址解析成包内记录的文件名。
    ///
    /// 地址通常是 `ui://` 形式，经注册表查出条目再取它记录的文件名；
    /// 不是地址形式时按包内文件名直接使用。
    ///
    /// `ui://` 地址解析不出来就是真的找不到，返回 `None` 而不是把它当成
    /// 文件名——那样只会让资产加载器在运行时静默失败。
    pub fn resolve_sound_file(&self, sound: &str) -> Option<String> {
        if sound.starts_with("ui://") {
            let (package, index) = self.registry.resolve_url(sound)?;
            return package.items().get(index)?.file.clone();
        }

        Some(sound.to_string())
    }

    /// 按 `ui://` 地址解析出所属的包与条目下标。
    ///
    /// 与包内的解析不同，这里能跨包——只要那个包已经登记过。
    pub fn resolve_url(&self, url: &str) -> Option<(&std::sync::Arc<FuiPackage>, usize)> {
        self.registry.resolve_url(url)
    }

    /// 开始一次拖拽。
    ///
    /// `icon` 是跟手图标对象，由使用方从包里创建；它会被挂到根节点的顶层，
    /// 随指针移动。「什么能拖」属于业务逻辑，因此**开始与否由使用方决定**；
    /// 松手与命中判定则由输入层负责。已有拖拽时忽略新请求。
    pub fn start_drag(
        &mut self,
        source: GObjectId,
        icon: Option<GObjectId>,
        data: Option<String>,
        pointer: fairygui_core::geom::Point,
    ) {
        if self.is_dragging() {
            return;
        }
        self.drag_touch_id = None;
        self.drag_drop.start(&mut self.store, source, icon, data);

        if let Some(icon) = icon {
            self.store.set_touchable(icon, false);
            if let Some(root) = self.root.as_mut() {
                self.store.set_parent(icon, Some(root.owner()));
            }
            self.queue_spawn(icon);
        }

        self.drag_drop.update(&mut self.store, pointer);
    }

    /// Start drag-and-drop owned by an already captured finger. Pass the
    /// `UiEvent::touch_id` from a touch event; other fingers do not move or finish it.
    pub fn start_touch_drag(
        &mut self,
        source: GObjectId,
        icon: Option<GObjectId>,
        data: Option<String>,
        pointer: fairygui_core::Point,
        touch_id: u64,
    ) {
        if self.is_dragging() {
            return;
        }
        self.start_drag(source, icon, data, pointer);
        self.drag_touch_id = Some(touch_id);
    }

    /// 是否正在拖拽。
    pub fn is_dragging(&self) -> bool {
        self.drag_drop.is_active()
    }

    /// 拖拽携带的数据。放置方据此判断这次拖拽是不是它想要的。
    pub fn drag_data(&self) -> Option<&str> {
        self.drag_drop.data()
    }

    /// 更新拖拽的指针位置与命中的放置目标。
    pub fn update_drag(&mut self, pointer: fairygui_core::geom::Point, target: Option<GObjectId>) {
        self.drag_drop.update(&mut self.store, pointer);
        self.drag_drop.set_target(target);
    }

    /// 结束拖拽，返回结果，并向有效放置目标派发带完整数据的 Drop 事件。
    ///
    /// 跟手图标用完即销毁——它是临时对象，留着只会积攒。
    pub fn finish_drag(
        &mut self,
        commands: &mut Commands,
    ) -> Option<fairygui_core::drag_drop::DragResult> {
        let data = self.drag_drop.data().map(str::to_owned);
        let pointer = self.drag_drop.pointer();
        let touch_id = self.drag_touch_id;
        self.drag_touch_id = None;
        let icon = self.drag_drop.icon();
        let result = self.drag_drop.finish(&mut self.store);
        let session = self.ecs_drag.take();

        if let Some(icon) = icon {
            self.despawn_subtree(commands, icon);
        }
        if let Some(result) = result
            && let Some(target) = result.target.and_then(|target| {
                if session.is_some() {
                    self.ecs_drop_target(target).map(|(target, _)| target)
                } else {
                    Some(target)
                }
            })
            && self.store.object(result.source).is_some()
            && crate::input::eligible(self, target)
            && session.as_ref().is_none_or(|session| {
                self.entity_of(result.source) == Some(session.source)
                    && crate::input::eligible(self, result.source)
            })
        {
            if let Some(session) = session
                && let Some(&entity) = self.drop_targets.get(&target)
            {
                self.drop_events.push(ecs_drag_drop::GuiDrop {
                    entity,
                    source: session.source,
                    data: data.clone(),
                    position: Vec2::new(pointer.x, pointer.y),
                    touch_id,
                });
            }
            let mut event =
                fairygui_core::object::UiEvent::new(fairygui_core::object::EventKind::Drop, target)
                    .with_pointer(pointer)
                    .with_data(fairygui_core::object::EventData::Drop {
                        source: result.source,
                        data,
                    });
            event.touch_id = touch_id;
            self.dispatch_event(event);
        }
        result
    }

    /// Cancel the active drag, reclaiming its temporary icon without a Drop event.
    pub fn cancel_drag(&mut self, commands: &mut Commands) {
        self.ecs_drag = None;
        self.drag_touch_id = None;
        let icon = self.drag_drop.icon();
        self.drag_drop.cancel(&mut self.store);
        if let Some(icon) = icon {
            self.despawn_subtree(commands, icon);
        }
    }

    /// 销毁一棵子树：对象、实体与待生成队列一并清理。
    fn despawn_subtree(&mut self, commands: &mut Commands, root: GObjectId) {
        let mut removed = vec![root];
        self.store
            .for_each_descendant(root, &mut |id| removed.push(id));
        self.store.remove(root);
        self.preserve_surviving_views(commands);

        for object in removed.into_iter().rev() {
            self.object_assets.remove(&object);
            if let Some(entity) = self.entities.remove(&object) {
                commands.entity(entity).despawn();
            }
            self.pending_spawn.retain(|pending| *pending != object);
        }
    }

    /// 把窗口提到最上方。
    pub fn bring_window_to_front(&mut self, index: usize) -> bool {
        let Some(window) = self.windows.get(index).cloned() else {
            return false;
        };
        let Some(root) = self.root.as_mut() else {
            return false;
        };

        window.bring_to_front(&mut self.store, root)
    }

    /// 派发一个界面事件，并沿对象树逐级冒泡。
    ///
    /// 队列里会为目标本身以及它的每个祖先各留一条记录，
    /// 使用方据此实现「在自己或某个父节点上处理事件」。
    pub fn dispatch(&mut self, kind: fairygui_core::object::EventKind, target: GObjectId) {
        self.dispatch_event(fairygui_core::object::UiEvent::new(kind, target));
    }

    /// Dispatch an event with its pointer/data payload preserved through bubbling.
    pub fn dispatch_event(&mut self, base: fairygui_core::object::UiEvent) {
        self.dispatch_event_until(base, None);
    }

    pub(crate) fn dispatch_event_until(
        &mut self,
        mut base: fairygui_core::object::UiEvent,
        stop: Option<GObjectId>,
    ) {
        let target = base.target;
        base.current_target = target;
        base.bubbling = false;
        if base.kind == fairygui_core::object::EventKind::Link {
            self.links.push(base.clone());
        }
        self.events.push(base.clone());
        self.gui_events.push(base.clone());

        let mut current = target;
        if Some(current) == stop {
            return;
        }
        while let Some(parent) = self
            .store
            .object(current)
            .and_then(|object| object.parent())
        {
            let mut event = base.clone();
            event.current_target = parent;
            event.bubbling = true;
            self.gui_events.push(event.clone());
            self.events.push(event);
            current = parent;
            if Some(current) == stop {
                break;
            }
        }
    }

    pub(crate) fn take_gui_events(&mut self) -> Vec<fairygui_core::object::UiEvent> {
        std::mem::take(&mut self.gui_events)
    }

    /// 取走待处理的事件。
    pub fn drain_events(&mut self) -> impl Iterator<Item = fairygui_core::object::UiEvent> + '_ {
        self.events.drain()
    }

    /// 待处理的事件数量。
    pub fn pending_event_count(&self) -> usize {
        self.events.len()
    }

    /// Request input focus (or None to release GUI input focus) during Respond.
    /// Compound controls forward to their input title. Hidden/off-stage/modal-
    /// blocked fields and unsupported rich/bitmap editors cannot acquire focus.
    pub fn request_input_focus(&mut self, input: Option<GObjectId>) -> bool {
        let input = if let Some(input) = input {
            let Some(input) = self.store.input_field_of(input) else {
                return false;
            };
            Some(input)
        } else {
            None
        };
        self.input_focus_request = Some(input);
        true
    }

    pub fn focused_input(&self) -> Option<GObjectId> {
        self.focused_input
    }

    /// Call while handling an Enter message in Respond to keep the session open.
    pub fn prevent_input_submit(&mut self, input: GObjectId) -> bool {
        if self.input_submit.is_some_and(|(owner, _)| owner == input) {
            self.input_submit = None;
            true
        } else {
            false
        }
    }

    pub(crate) fn emit_input_event(
        &mut self,
        kind: fairygui_core::object::EventKind,
        target: GObjectId,
        value: &str,
    ) {
        let event = fairygui_core::object::UiEvent::new(kind, target);
        self.events.push(event.clone());
        if let Some(entity) = self.entity_of(target) {
            self.input_events.push(GuiTextInput {
                entity,
                event,
                value: value.to_owned(),
            });
        }
    }

    pub(crate) fn emit_list_item_click(
        &mut self,
        target: GObjectId,
        pointer: fairygui_core::Point,
        shift: bool,
        ctrl: bool,
        touch_id: Option<u64>,
    ) {
        let Some((list, item, index)) = self.store.list_item_of(target) else {
            return;
        };
        let mut event =
            fairygui_core::object::UiEvent::new(fairygui_core::object::EventKind::ClickItem, list)
                .with_pointer(pointer)
                .with_data(fairygui_core::object::EventData::Item(index.to_string()));
        event.touch_id = touch_id;
        self.events.push(event.clone());
        self.gui_events.push(event.clone());
        self.list_clicks.push(PendingListItemClick {
            event,
            item,
            index,
            shift,
            ctrl,
            views: self.entity_of(list).zip(self.entity_of(item)),
        });
    }
}

pub(crate) fn emit_text_input_messages(
    mut commands: Commands,
    mut runtime: NonSendMut<GuiRuntime>,
    mut messages: MessageWriter<GuiTextInput>,
) {
    for event in std::mem::take(&mut runtime.input_events) {
        if event.event.kind == fairygui_core::object::EventKind::Enter
            && let Some((owner, ready)) = &mut runtime.input_submit
            && *owner == event.event.target
        {
            *ready = true;
        }
        commands.trigger(event.clone());
        messages.write(event);
    }
}

/// 把运行时已经路由好的点击转成 Bevy 消息。
pub fn emit_gui_events(
    mut commands: Commands,
    mut runtime: NonSendMut<GuiRuntime>,
    entities: Query<(Entity, &GObjectRef)>,
) {
    for event in runtime.take_gui_events() {
        let Some(entity) = runtime.entity_of(event.current_target).or_else(|| {
            entities
                .iter()
                .find_map(|(entity, object)| (object.0 == event.current_target).then_some(entity))
        }) else {
            continue;
        };
        let target = runtime.entity_of(event.target).or_else(|| {
            entities
                .iter()
                .find_map(|(entity, object)| (object.0 == event.target).then_some(entity))
        });
        commands.trigger(GuiEvent {
            entity,
            target,
            event,
        });
    }
}

#[derive(Clone)]
pub(crate) enum ObjectCommandSource {
    Root(FairyGuiRoot),
    Window(windows::FairyGuiWindow),
}
impl ObjectCommandSource {
    pub(crate) fn same_template(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Root(a), Self::Root(b)) => a.asset == b.asset && a.item_name == b.item_name,
            (Self::Window(a), Self::Window(b)) => a.asset == b.asset && a.item_name == b.item_name,
            _ => false,
        }
    }
}

pub(crate) fn discard_removed_object_commands(
    mut removed: RemovedComponents<FairyGuiRoot>,
    mut removed_windows: RemovedComponents<windows::FairyGuiWindow>,
    mut runtime: NonSendMut<GuiRuntime>,
) {
    for target in removed.read().chain(removed_windows.read()) {
        runtime
            .pending_object_commands
            .retain(|(_, command)| command.target != target);
    }
}

/// Both Sync passes share their cursor and pending requests. Commands sent in
/// PostLayout apply this frame without replaying commands from Respond.
pub(crate) fn apply_object_commands(
    messages: Res<Messages<GuiObjectCommand>>,
    objects: Query<&GObjectRef>,
    roots: Query<&GuiRootInstance>,
    sources: Query<&FairyGuiRoot>,
    window_sources: Query<&windows::FairyGuiWindow>,
    mut runtime: NonSendMut<GuiRuntime>,
) {
    let incoming = runtime
        .object_command_cursor
        .read(&messages)
        .cloned()
        .collect::<Vec<_>>();
    let waiting = std::mem::take(&mut runtime.pending_object_commands);
    for (expected, command) in waiting
        .into_iter()
        .map(|(source, command)| (Some(source), command))
        .chain(incoming.into_iter().map(|command| (None, command)))
    {
        let source = sources
            .get(command.target)
            .ok()
            .cloned()
            .map(ObjectCommandSource::Root)
            .or_else(|| {
                window_sources
                    .get(command.target)
                    .ok()
                    .cloned()
                    .map(ObjectCommandSource::Window)
            });
        if let Some(expected) = expected
            && source
                .as_ref()
                .is_none_or(|current| !current.same_template(&expected))
        {
            continue;
        }
        let object = objects
            .get(command.target)
            .map(|reference| reference.0)
            .or_else(|_| roots.get(command.target).map(|root| root.object))
            .ok()
            .or_else(|| runtime.ecs_window_object(command.target));
        let Some(object) = object else {
            if let Some(source) = source
                && !runtime
                    .ecs_roots
                    .get(&command.target)
                    .is_some_and(|binding| binding.failed)
                && !runtime
                    .ecs_windows
                    .get(&command.target)
                    .is_some_and(|binding| binding.failed)
            {
                runtime.pending_object_commands.push((source, command));
            }
            continue;
        };
        match &command.action {
            GuiObjectAction::SetAnimationPlaying(playing) => {
                runtime.store_mut().set_animation_playing(object, *playing);
            }
            GuiObjectAction::SetAnimationFrame(frame) => {
                runtime.store_mut().set_animation_frame(object, *frame);
            }
            GuiObjectAction::SetAnimationTimeScale(scale) => {
                runtime.store_mut().set_animation_time_scale(object, *scale);
            }
            GuiObjectAction::SetAnimationPlaySettings(settings) => {
                runtime
                    .store_mut()
                    .set_animation_play_settings(object, *settings);
            }
            GuiObjectAction::AdvanceAnimation(seconds) => {
                runtime.store_mut().advance_animation(object, *seconds);
            }
            GuiObjectAction::RewindAnimation => {
                runtime.store_mut().rewind_animation(object);
            }
            GuiObjectAction::Tween {
                target,
                duration,
                ease,
            } => {
                target.apply(runtime.store_mut(), object, *duration, *ease);
            }
            GuiObjectAction::StopTween(property) => {
                runtime.store_mut().kill_tween_property(object, *property)
            }
            GuiObjectAction::SetPosition(position) if position.is_finite() => {
                runtime.store_mut().set_xy(object, position.x, position.y);
            }
            GuiObjectAction::SetSize(size) if size.is_finite() && size.min_element() >= 0.0 => {
                runtime.store_mut().set_size(object, size.x, size.y);
            }
            GuiObjectAction::SetVisible(visible) => {
                runtime.store_mut().set_visible(object, *visible);
            }
            GuiObjectAction::SetTouchable(touchable) => {
                runtime.store_mut().set_touchable(object, *touchable);
            }
            GuiObjectAction::SetAlpha(alpha) if alpha.is_finite() => {
                runtime.store_mut().set_alpha(object, *alpha);
            }
            GuiObjectAction::SetScale(scale) if scale.is_finite() => {
                runtime.store_mut().set_scale(object, scale.x, scale.y);
            }
            GuiObjectAction::SetPivot { pivot, as_anchor } if pivot.is_finite() => {
                runtime
                    .store_mut()
                    .set_pivot(object, pivot.x, pivot.y, *as_anchor);
            }
            GuiObjectAction::SetRotation(rotation) if rotation.is_finite() => {
                runtime.store_mut().set_rotation(object, *rotation);
            }
            GuiObjectAction::SetText(text) => {
                runtime.store_mut().set_object_text(object, text);
            }
            GuiObjectAction::EnsureTextSize => {
                let measurer = runtime.text_measurer.clone();
                measurer.borrow_mut().ensure(&mut runtime.store, object);
            }
            GuiObjectAction::SetIcon(icon) => {
                runtime.store_mut().apply_icon_to(object, icon);
            }
            GuiObjectAction::SetColor(color) => {
                runtime.store_mut().set_control_color(object, *color, None);
            }
            GuiObjectAction::SetGrayed(grayed) => {
                runtime.store_mut().set_grayed(object, *grayed);
            }
            GuiObjectAction::SetButtonSelected(selected) => {
                runtime.store_mut().set_button_selected(object, *selected);
            }
            GuiObjectAction::SetButtonChangeStateOnClick(enabled) => {
                runtime
                    .store_mut()
                    .set_button_change_state_on_click(object, *enabled);
            }
            GuiObjectAction::SetSliderValue(value) => {
                runtime.store_mut().set_slider_value(object, *value);
            }
            GuiObjectAction::SetControllerPage { name, index } => {
                runtime
                    .store_mut()
                    .set_controller_page_index(object, name, *index);
            }
            GuiObjectAction::SetProgress(value) => {
                runtime.store_mut().set_progress(object, *value);
            }
            GuiObjectAction::SetListSelection(index) => {
                runtime.store_mut().set_list_selection(object, *index);
            }
            GuiObjectAction::SetTreeExpanded { index, expanded } => {
                runtime
                    .store_mut()
                    .set_tree_node_expanded(object, *index, *expanded);
            }
            GuiObjectAction::ToggleTreeNode(index) => {
                runtime.toggle_tree_node(object, *index);
            }
            GuiObjectAction::ExpandTree => {
                runtime.expand_tree(object);
            }
            GuiObjectAction::CollapseTree => {
                runtime.collapse_tree(object);
            }
            GuiObjectAction::SelectTreeNode {
                index,
                scroll_to_view,
            } => {
                let valid = matches!(runtime.store().object(object).map(|o|o.control()),Some(ControlData::Tree(data)) if *index<data.node_count());
                if valid {
                    runtime.expand_tree_to(object, *index);
                    runtime.store_mut().add_list_selection_with_scroll(
                        object,
                        *index,
                        *scroll_to_view,
                    );
                }
            }
            GuiObjectAction::SetScrollPosition {
                position,
                allow_bounce,
            } if position.is_finite() => {
                runtime
                    .store_mut()
                    .set_scroll_pos(object, position.x, position.y, *allow_bounce);
            }
            GuiObjectAction::SelectListIndex {
                index,
                scroll_to_view,
            } => {
                runtime
                    .store_mut()
                    .add_list_selection_with_scroll(object, *index, *scroll_to_view);
            }
            GuiObjectAction::ScrollTo { position, animated } if position.is_finite() => {
                runtime.store_mut().layout_scroll_bars(object);
                runtime
                    .store_mut()
                    .set_scroll_pos_animated(object, position.x, position.y, *animated);
            }
            GuiObjectAction::SetScrollPercentage {
                percentage,
                animated,
            } if percentage.is_finite() => {
                runtime.store_mut().layout_scroll_bars(object);
                if let Some(pane) = runtime.store().scroll_pane(object) {
                    let max = pane.max_scroll_pos();
                    let position =
                        percentage.clamp(Vec2::ZERO, Vec2::ONE) * Vec2::new(max.x, max.y);
                    runtime
                        .store_mut()
                        .set_scroll_pos_animated(object, position.x, position.y, *animated);
                }
            }
            GuiObjectAction::ScrollToView {
                rect,
                animated,
                set_first,
            } => {
                runtime
                    .store_mut()
                    .scroll_rect_to_view(object, *rect, *animated, *set_first);
            }
            GuiObjectAction::PlayTransition { name, times } => {
                runtime.store_mut().play_transition(object, name, *times);
            }
            GuiObjectAction::PlayTransitionWithOptions { name, options } => {
                runtime
                    .store_mut()
                    .play_transition_with_options(object, name, *options);
            }
            GuiObjectAction::SetTransitionPaused { name, paused } => {
                runtime
                    .store_mut()
                    .set_transition_paused(object, name, *paused);
            }
            GuiObjectAction::SetTransitionTimeScale { name, scale } if scale.is_finite() => {
                runtime
                    .store_mut()
                    .set_transition_time_scale(object, name, *scale);
            }
            GuiObjectAction::PlayTransitionReverse { name, delay } if delay.is_finite() => {
                runtime
                    .store_mut()
                    .play_transition_reverse(object, name, *delay);
            }
            GuiObjectAction::StopTransition { name } => {
                runtime.store_mut().stop_transition(object, name);
            }
            GuiObjectAction::SetPosition(_)
            | GuiObjectAction::SetSize(_)
            | GuiObjectAction::SetAlpha(_)
            | GuiObjectAction::SetScale(_)
            | GuiObjectAction::SetPivot { .. }
            | GuiObjectAction::SetRotation(_)
            | GuiObjectAction::SetScrollPosition { .. }
            | GuiObjectAction::ScrollTo { .. }
            | GuiObjectAction::SetScrollPercentage { .. }
            | GuiObjectAction::SetTransitionTimeScale { .. }
            | GuiObjectAction::PlayTransitionReverse { .. } => {}
        }
    }
}

/// Project native list selection onto ECS components after application commands
/// have been applied. Input-driven selection is visible in the same frame too.
pub fn sync_list_selection_components(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    lists: Query<(Entity, &GObjectRef, &GuiListSelection)>,
) {
    for (entity, object, current) in &lists {
        let Some(selected) = runtime.store().list_selections(object.0) else {
            continue;
        };
        if current.0 != selected {
            commands.entity(entity).insert(GuiListSelection(selected));
        }
    }
}

/// Project current control values onto their ECS entities after model changes.
#[allow(clippy::type_complexity)]
pub fn sync_control_state_components(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    controls: Query<
        (
            Entity,
            &GObjectRef,
            Option<&GuiProgressValue>,
            Option<&GuiSliderValue>,
            Option<&GuiButtonSelected>,
        ),
        Or<(
            With<GuiProgressValue>,
            With<GuiSliderValue>,
            With<GuiButtonSelected>,
        )>,
    >,
) {
    for (entity, object, progress, slider, button) in &controls {
        if let (Some(current), Some(value)) = (progress, runtime.store().progress_value(object.0))
            && current.0 != value
        {
            commands.entity(entity).insert(GuiProgressValue(value));
        }
        if let (Some(current), Some(value)) = (slider, runtime.store().slider_value(object.0))
            && current.0 != value
        {
            commands.entity(entity).insert(GuiSliderValue(value));
        }
        if let (Some(current), Some(value)) = (button, runtime.store().button_selected(object.0))
            && current.0 != value
        {
            commands.entity(entity).insert(GuiButtonSelected(value));
        }
    }
}

/// 为新建的界面生成实体树。
pub fn spawn_pending_roots(
    mut commands: Commands,
    mut runtime: NonSendMut<GuiRuntime>,
    assets: Res<Assets<FuiAsset>>,
    asset_server: Res<AssetServer>,
    image_assets: Res<Assets<Image>>,
) {
    for index in 0..runtime.roots.len() {
        if runtime.roots[index].spawned {
            continue;
        }

        let handle = runtime.roots[index].handle.clone();
        let Some(asset) = assets.get(&handle) else {
            // 资产尚未加载完成，下一帧再试。
            continue;
        };
        let object = runtime.roots[index].object;
        if !runtime.store.contains(object) {
            runtime.roots[index].spawned = true;
            continue;
        }
        // A pending root can already have been adopted by a retained popup or
        // another model parent. Wait for that parent's view rather than spawning
        // it as an unrelated UI root and briefly detaching its stage subtree.
        let parent_object = runtime
            .store
            .object(object)
            .and_then(|object| object.parent());
        let parent = parent_object.and_then(|parent| runtime.entity_of(parent));
        if parent_object.is_some() && parent.is_none() {
            continue;
        }

        // 分别借用两个字段：对象存储只读，实体映射需要写入。
        let runtime = &mut *runtime;

        runtime.prepare_subtree(&asset.package, object);

        runtime.remember_asset_tree(object, &handle);
        let store = &runtime.store;
        let entities = &mut runtime.entities;
        let translations = &runtime.translations;
        let font = runtime.font.clone();
        spawn_object(
            &mut commands,
            store,
            entities,
            asset,
            &assets,
            &runtime.object_assets,
            &asset_server,
            &image_assets,
            translations,
            font.as_ref(),
            object,
            parent,
        );

        // 界面构建完成后，启动标记为自动播放的过渡。
        if runtime.roots[index].attached {
            runtime.store.attach_to_stage(object);
        }
        runtime.roots[index].spawned = true;
    }
}

/// Create, attach and dispose roots owned by ECS components.
///
/// Unavailable assets remain pending without creating partial model objects.
/// Component removal/despawn reclaims the root and all generated views.
#[allow(clippy::too_many_arguments)]
pub fn sync_ecs_roots(
    mut commands: Commands,
    changed: Query<(Entity, &FairyGuiRoot), Changed<FairyGuiRoot>>,
    roots: Query<&FairyGuiRoot>,
    entities: Query<()>,
    mut removed: RemovedComponents<FairyGuiRoot>,
    assets: Res<Assets<FuiAsset>>,
    windows: Query<(Entity, &Window, Has<bevy::window::PrimaryWindow>)>,
    settings: Option<Res<crate::GuiInputSettings>>,
    mut runtime: NonSendMut<GuiRuntime>,
) {
    for owner in removed.read() {
        if let Some(binding) = runtime.ecs_roots.remove(&owner)
            && let Some(handle) = binding.handle
        {
            runtime.remove_root(&mut commands, handle);
        }
        if entities.get(owner).is_ok() {
            commands
                .entity(owner)
                .remove::<(GuiRootInstance, GuiRootView)>();
        }
    }

    for (owner, source) in &changed {
        let unchanged = runtime.ecs_roots.get(&owner).is_some_and(|binding| {
            binding.asset == source.asset && binding.item_name == source.item_name
        });
        if unchanged {
            let handle = runtime.ecs_roots[&owner].handle;
            if let Some(handle) = handle {
                if let Some(object) = runtime.root_object(handle) {
                    let binding = &runtime.ecs_roots[&owner];
                    let position = (binding.position != source.position)
                        .then_some(source.position)
                        .flatten();
                    let size = (binding.size != source.size
                        || binding.follow_window_size != source.follow_window_size)
                        .then_some(source.size)
                        .flatten();
                    apply_root_geometry(&mut runtime, object, position, size);
                    let binding = runtime.ecs_roots.get_mut(&owner).unwrap();
                    binding.position = source.position;
                    binding.size = source.size;
                    binding.follow_window_size = source.follow_window_size;
                    let active = runtime.ecs_roots[&owner].active;
                    if active != source.active {
                        runtime.set_root_attached(handle, source.active);
                        runtime.ecs_roots.get_mut(&owner).unwrap().active = source.active;
                    }
                    continue;
                }

                runtime.ecs_roots.get_mut(&owner).unwrap().handle = None;
                commands
                    .entity(owner)
                    .remove::<(GuiRootInstance, GuiRootView)>();
            }
            runtime.ecs_roots.get_mut(&owner).unwrap().active = source.active;
            continue;
        }

        if let Some(binding) = runtime.ecs_roots.remove(&owner)
            && let Some(handle) = binding.handle
        {
            runtime.remove_root(&mut commands, handle);
        }
        commands
            .entity(owner)
            .remove::<(GuiRootInstance, GuiRootView)>();
        runtime.ecs_roots.insert(
            owner,
            EcsRootBinding {
                asset: source.asset.clone(),
                item_name: source.item_name.clone(),
                handle: None,
                active: source.active,
                failed: false,
                position: source.position,
                size: source.size,
                follow_window_size: source.follow_window_size,
            },
        );
    }

    let stale: Vec<_> = runtime
        .ecs_roots
        .iter()
        .filter_map(|(&owner, binding)| {
            binding
                .handle
                .filter(|&handle| runtime.root_object(handle).is_none())
                .map(|_| owner)
        })
        .collect();
    for owner in stale {
        runtime.ecs_roots.get_mut(&owner).unwrap().handle = None;
        commands
            .entity(owner)
            .remove::<(GuiRootInstance, GuiRootView)>();
    }

    let pending: Vec<_> = runtime
        .ecs_roots
        .iter()
        .filter_map(|(&owner, binding)| {
            (binding.handle.is_none() && !binding.failed).then_some(owner)
        })
        .collect();
    for owner in pending {
        let Ok(source) = roots.get(owner) else {
            continue;
        };
        if runtime.ecs_roots[&owner].failed {
            continue;
        }
        let Some(asset) = assets.get(&source.asset) else {
            continue;
        };
        let Some(handle) =
            runtime.create_root(source.asset.clone(), &asset.package, &source.item_name)
        else {
            tracing::warn!(
                owner = ?owner,
                item = %source.item_name,
                "ECS FairyGUI root could not be created"
            );
            runtime.ecs_roots.get_mut(&owner).unwrap().failed = true;
            continue;
        };
        runtime.set_root_attached(handle, source.active);
        let object = runtime.root_object(handle).unwrap();
        apply_root_geometry(&mut runtime, object, source.position, source.size);
        let binding = runtime.ecs_roots.get_mut(&owner).unwrap();
        binding.handle = Some(handle);
        binding.active = source.active;
        binding.position = source.position;
        binding.size = source.size;
        binding.follow_window_size = source.follow_window_size;
        commands
            .entity(owner)
            .insert(GuiRootInstance { handle, object });
    }

    // Resolve the current window even when the root component has not changed.
    // Apply before materialization so initial layout and child relations see the
    // window size on the first frame, including after asynchronous asset loading.
    if let Some((_, window, _)) = windows.iter().find(|(entity, _, primary)| {
        settings
            .as_ref()
            .and_then(|settings| settings.window)
            .map_or(*primary, |chosen| chosen == *entity)
    }) {
        let size = Vec2::new(window.width(), window.height());
        let following: Vec<_> = runtime
            .ecs_roots
            .iter()
            .filter_map(|(owner, binding)| {
                roots
                    .get(*owner)
                    .ok()
                    .filter(|source| source.follow_window_size)
                    .and(binding.handle)
                    .and_then(|handle| runtime.root_object(handle))
            })
            .collect();
        for object in following {
            apply_root_geometry(&mut runtime, object, None, Some(size));
        }
    }
}

fn apply_root_geometry(
    runtime: &mut GuiRuntime,
    object: GObjectId,
    position: Option<Vec2>,
    size: Option<Vec2>,
) {
    if let Some(size) = size.filter(|size| size.is_finite() && size.min_element() >= 0.) {
        runtime.store.set_size(object, size.x, size.y);
    }
    if let Some(position) = position.filter(|position| position.is_finite()) {
        runtime.store.set_xy(object, position.x, position.y);
    }
}

/// 为这棵对象树里的可滚动容器实例化滚动条，返回创建出的数量。
///
/// 滚动条是包内条目，按配置里的地址创建后作为容器的子对象加入。指向别的包
/// 或者根本没配置滚动条的容器会被跳过——滚动本身依然可用，只是没有滚动条。
fn attach_scroll_bars(
    store: &mut ObjectStore,
    registry: &fairygui_core::package::PackageRegistry,
    package: &FuiPackage,
    root: GObjectId,
) -> usize {
    // 先收集容器：创建过程会往树里加对象，边遍历边改树会漏掉或重复。
    let mut containers = vec![root];
    store.for_each_descendant(root, &mut |id| containers.push(id));

    let mut total = 0;
    for container in containers {
        let scrollable = store
            .object(container)
            .and_then(|object| object.control().component())
            .is_some_and(|data| data.scroll_pane.is_some());
        if !scrollable {
            continue;
        }

        let mut create = |store: &mut ObjectStore, url: &str| {
            // 先试本包：资源引用绝大多数是同包内的，这条路不必查表。
            if let Some(index) = package.resolve_local_url(url) {
                return fairygui_core::widget::create_from_package_with_registry(
                    store,
                    package,
                    index,
                    &registry.load_context(package),
                    registry,
                )
                .ok();
            }

            // 再试注册表：滚动条放在别的包里也能创建出来。
            let (owner, index) = registry.resolve_url(url)?;
            fairygui_core::widget::create_from_package_with_registry(
                store,
                owner,
                index,
                &registry.load_context(owner),
                registry,
            )
            .ok()
        };
        total += store.attach_scroll_bars(container, &mut create);
        total += store.attach_scroll_refresh_bars(container, &mut create);
    }

    total
}

/// 按容器尺寸摆放这棵对象树里的全部滚动条。
fn layout_scroll_bars(store: &mut ObjectStore, root: GObjectId) {
    let mut containers = vec![root];
    store.for_each_descendant(root, &mut |id| containers.push(id));

    for container in containers {
        if store.layout_scroll_bars(container) {
            // 视口变了，滚动条的整体可见性与滑块都要跟着更新。
            store.sync_scroll_bars(container);
        }
    }
}

/// 为运行时新加入对象树的对象生成实体。
///
/// 窗口是在运行期创建的，实体树需要跟上。队列按加入顺序排列、且父对象总是
/// 界面文本使用的字体。
///
/// 引擎自带的默认字体只有拉丁字形，中日韩文本会渲染成空白。注册本资源后，
/// 由引擎渲染的文本（非位图字体的那些）都会改用它。
///
/// ```no_run
/// # use bevy::prelude::*;
/// # use fairygui_bevy::{FairyGuiPlugin, GuiFont};
/// # let mut app = App::new();
/// app.add_plugins((DefaultPlugins, FairyGuiPlugin));
/// app.insert_resource(GuiFont(Some(
///     app.world().resource::<AssetServer>().load("fonts/NotoSansSC.otf"),
/// )));
/// ```
#[derive(Resource, Debug, Clone, Default)]
pub struct GuiFont(pub Option<Handle<Font>>);

/// 先于子对象入队，因此轮到某个对象时它的父实体一定已经就绪。
pub fn spawn_pending_objects(
    mut commands: Commands,
    mut runtime: NonSendMut<GuiRuntime>,
    assets: Res<Assets<FuiAsset>>,
    asset_server: Res<AssetServer>,
    image_assets: Res<Assets<Image>>,
) {
    if runtime.pending_spawn.is_empty() {
        return;
    }

    let pending = std::mem::take(&mut runtime.pending_spawn);
    // 同一批入队的对象互相可见，用来判断「父是不是也在等着生成」。
    let mut queued: std::collections::HashSet<GObjectId> = pending.iter().copied().collect();
    let runtime = &mut *runtime;
    // 本轮轮不到的，留到下一帧再试。
    let mut deferred = Vec::new();

    for object in pending {
        queued.remove(&object);

        // 对象已经从 store 里消失（被移除或销毁），不必生成。
        if runtime.store.object(object).is_none() {
            continue;
        }

        // 父实体没就绪时不能生成子对象：子实体要挂到父实体下，
        // 父没就绪只会挂错层级。父也在这批队列里就留到下一帧——那时
        // 父已经生成；父不在队列里又没实体，说明这一支已经脱离了实体树。
        let parent_object = runtime.store.object(object).and_then(|item| item.parent());
        if let Some(parent) = parent_object
            && !runtime.entities.contains_key(&parent)
        {
            if queued.contains(&parent) {
                deferred.push(object);
            }
            continue;
        }

        if runtime.entities.contains_key(&object) {
            continue;
        }

        let parent = parent_object.and_then(|parent| runtime.entities.get(&parent).copied());
        let Some(handle) = runtime.asset_handle_for(object) else {
            deferred.push(object);
            continue;
        };
        let Some(asset) = assets.get(&handle) else {
            deferred.push(object);
            continue;
        };

        runtime.prepare_subtree(&asset.package, object);
        runtime.remember_asset_tree(object, &handle);
        spawn_object(
            &mut commands,
            &runtime.store,
            &mut runtime.entities,
            asset,
            &assets,
            &runtime.object_assets,
            &asset_server,
            &image_assets,
            &runtime.translations,
            runtime.font.as_ref(),
            object,
            parent,
        );
    }

    runtime.pending_spawn = deferred;
}

/// 播放核心层排队的音效。
///
/// 过渡与控件只把「谁要放、放什么、多大声」记进队列，播放由这里接手：
/// 地址解析成包内记录的文件名，再拼成资产路径交给 Bevy 的音频系统。
/// 播放实体是一次性的，放完自动回收。
///
/// 声音文件与界面在同一个包目录下——包内记录的文件名带包名前缀但不带目录，
/// 这是编辑器导出的约定，与图集走的是同一套解析。
pub fn play_queued_sounds(
    mut commands: Commands,
    mut runtime: NonSendMut<GuiRuntime>,
    assets: Res<Assets<FuiAsset>>,
    asset_server: Res<AssetServer>,
) {
    let queued = runtime.store_mut().take_queued_sounds();
    if queued.is_empty() {
        return;
    }

    for (owner, sound, volume) in queued {
        let Some(file) = runtime.resolve_sound_file(&sound) else {
            tracing::warn!(?owner, %sound, "找不到音效资源，已跳过");
            continue;
        };
        let asset = if sound.starts_with("ui://") {
            assets
                .iter()
                .find_map(|(_, asset)| asset.package.resolve_local_url(&sound).map(|_| asset))
        } else {
            runtime
                .asset_handle_for(owner)
                .and_then(|handle| assets.get(&handle))
        };
        let Some(asset) = asset else {
            continue;
        };

        commands.spawn((
            bevy::audio::AudioPlayer::new(asset_server.load(asset.resolve_asset_path(&file))),
            bevy::audio::PlaybackSettings::DESPAWN
                .with_volume(bevy::audio::Volume::Linear(volume.clamp(0.0, 1.0))),
        ));
    }
}

/// 递归生成一个对象及其子对象的实体。
// 生成实体树需要的上下文都要一路传下去；打包成结构体反而让递归调用更啰嗦。
#[allow(clippy::too_many_arguments)]
fn spawn_object(
    commands: &mut Commands,
    store: &ObjectStore,
    entities: &mut HashMap<GObjectId, Entity>,
    asset: &FuiAsset,
    assets: &Assets<FuiAsset>,
    object_assets: &HashMap<GObjectId, Handle<FuiAsset>>,
    asset_server: &AssetServer,
    image_assets: &Assets<Image>,
    translations: &fairygui_core::translation::TranslationTable,
    font: Option<&Handle<Font>>,
    object: GObjectId,
    parent: Option<Entity>,
) -> Option<Entity> {
    let gobject = store.object(object)?;
    // A layout parent can own a different package (e.g. GRoot/MainMenu with a
    // Basics dropdown). Resolve at every recursion boundary before view creation.
    let asset = if let Some(source) = object_assets.get(&object) {
        assets.get(source)?
    } else {
        asset
    };
    // A ready view can be adopted by a root whose asset/view is still pending.
    // Reuse it instead of replacing the mapping and leaking its owned visuals.
    if let Some(entity) = entities.get(&object).copied() {
        if let Some(parent) = parent {
            commands.entity(entity).insert(ChildOf(parent));
        } else {
            commands.entity(entity).remove::<ChildOf>();
        }
        return Some(entity);
    }

    // 3D 装载器用于骨骼动画（Spine / DragonBones）。Rust 生态暂无成熟对应库，
    // 因此明确告警而不是静默失败。
    if gobject.kind() == fairygui_core::package::ObjectType::Loader3D {
        tracing::warn!(?object, "3D 装载器（骨骼动画）暂不支持，将按普通容器处理");
    }

    let mut node = backend::node_for(gobject);

    if let Some(data) = gobject.control().component() {
        node.overflow = backend::overflow_for(data.overflow);
    }

    // Fill masks crop the drawn image via UiMaterial; layout/input stay full-size.

    // Basic shapes use node styles; polygons receive material surfaces in Sync.
    let graph = match gobject.control() {
        ControlData::Graph(data) if data.graph_kind().is_basic_shape() => {
            Some(backend::graph_style(data))
        }
        _ => None,
    };

    // 形状与描边都是节点自身的属性：圆角决定是直角、圆角还是椭圆，
    // 边框宽度决定描边画不画得出来（宽度为零就没有线）。
    if let Some(style) = &graph {
        if style.stroke_width > 0.0 {
            node.border = UiRect::all(Val::Px(style.stroke_width));
        }
        node.border_radius = style.radius;
    }

    let name = if gobject.name().is_empty() {
        gobject.kind().as_str().to_string()
    } else {
        gobject.name().to_string()
    };

    let mut entity = commands.spawn((
        node,
        backend::transform_for(gobject),
        GObjectRef(object),
        Name::new(name),
        // Bevy picking 在 PreUpdate 更新悬停状态，指针捕获由输入层维护。
        bevy::picking::hover::Hovered::default(),
        // 先收集所有 GUI 候选，输入层再检查像素遮罩、模态与可点击状态。
        bevy::picking::Pickable {
            should_block_lower: false,
            is_hoverable: true,
        },
        bevy::ui::RelativeCursorPosition::default(),
        if store.display_visible(object) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        },
    ));
    let entity_id = entity.id();

    if let Some(selection) = store.list_selections(object) {
        entity.insert(GuiListSelection(selection));
    }
    match gobject.control() {
        ControlData::ProgressBar(_) => {
            entity.insert(GuiProgressValue(
                store.progress_value(object).unwrap_or_default(),
            ));
        }
        ControlData::Slider(_) => {
            entity.insert(GuiSliderValue(
                store.slider_value(object).unwrap_or_default(),
            ));
        }
        ControlData::Button(_) => {
            entity.insert(GuiButtonSelected(
                store.button_selected(object).unwrap_or_default(),
            ));
        }
        _ => {}
    }

    // 图形控件的外观：填充走背景色，描边走边框，形状由圆角决定。
    if let Some(style) = &graph {
        entity.insert(BackgroundColor(style.fill));
        entity.insert(BorderColor::all(style.stroke));
    }

    // 图片：直接以子区域方式引用图集纹理。
    if matches!(gobject.control(), ControlData::Image(_))
        && let Some(item_id) = gobject.package_item_id()
        && let Some(index) = asset.package.item_index_by_id(item_id)
        && let Some(sprite) = asset.package.sprite_for(index)
        && let Some(atlas) = asset.package.item(sprite.atlas_item)
        && let Some(file) = atlas.file.as_deref()
    {
        // Native grid layout/tile semantics are carried alongside the image descriptor.
        let scale9_grid = asset.package.item(index).and_then(|item| item.scale9_grid);
        let scale_by_tile = asset
            .package
            .item(index)
            .is_some_and(|item| item.scale_by_tile);

        let path = asset.resolve_asset_path(file);
        let mut image_node =
            backend::image_for(sprite, asset_server.load(path), scale9_grid, scale_by_tile);
        if let Some(grid) = asset
            .package
            .item(index)
            .and_then(|item| crate::image_fill::ImageGrid::from_package(sprite, item))
        {
            entity.insert(grid);
        }

        // 翻转标志按位组合：最低位是水平翻转，次低位是垂直翻转。
        if let Some(data) = gobject.control().as_image() {
            image_node.flip_x = data.flip & 1 != 0;
            image_node.flip_y = data.flip & 2 != 0;
        }

        // 同步阶段的图片材质在采样后执行颜色矩阵；此处只提供初始外观。
        let tint = tint(gobject, None);

        // 有效不透明度乘进着色里：它沿祖先累积，容器变淡会带着子对象一起淡。
        let alpha = store.effective_alpha(object);
        image_node.color = backend::apply_alpha(tint, alpha);
        entity.insert((image_node, BaseTint(tint)));

        // 该图片若带像素级命中数据，一并挂上供输入层使用。
        if let Some(data) = asset
            .package
            .item(index)
            .and_then(|item| item.pixel_hit_test_data.clone())
        {
            entity.insert((
                PixelHitTest(std::sync::Arc::new(data)),
                bevy::ui::RelativeCursorPosition::default(),
            ));
        }
    }

    // 命中区域的两种细分。默认的矩形命中由节点范围天然提供，不用额外处理。
    if let Some(data) = gobject.control().component()
        && let Some(area) = &data.hit_test
    {
        match area {
            fairygui_core::widget::HitTestArea::MaskChild { .. } => {
                entity.insert(bevy::ui::RelativeCursorPosition::default());
            }
            // 命中区域取自另一个子对象：容器自己让开。
            fairygui_core::widget::HitTestArea::Child { .. } => {
                entity.insert(HitAreaFromChild);
            }
            // 命中区域取自某个图片条目的像素数据：把数据挂到容器上，
            // 输入层会按它逐像素判定。
            fairygui_core::widget::HitTestArea::Pixel { item_id, .. } => {
                let pixel = asset
                    .package
                    .item_index_by_id(item_id.as_str())
                    .and_then(|index| asset.package.item(index))
                    .and_then(|item| item.pixel_hit_test_data.clone());

                if let Some(pixel) = pixel {
                    entity.insert((
                        PixelHitTest(std::sync::Arc::new(pixel)),
                        bevy::ui::RelativeCursorPosition::default(),
                    ));
                } else {
                    tracing::warn!(
                        ?object,
                        item_id,
                        "命中区域引用的像素数据不存在，将按矩形命中"
                    );
                }
            }
        }
    }

    // 滚动条的部件要读光标位置：轨道点击靠它判断该往哪个方向滚。
    if store.scroll_bar_of_part(object).is_some() {
        entity.insert(bevy::ui::RelativeCursorPosition::default());
    }

    if store.is_scroll_overlay(object) {
        entity.insert((
            ScrollBarOverlay,
            bevy::ui::IgnoreScroll(bevy::math::BVec2::TRUE),
        ));
    }

    // 序列帧：解析帧数据并挂上播放状态，此后只更新纹理子区域。
    if let ControlData::MovieClip(data) = gobject.control()
        && let Some(item_id) = gobject.package_item_id()
        && let Some(index) = asset.package.item_index_by_id(item_id)
        && let Some(player) =
            movie_sources::player_for(asset, asset_server, index, data.frame, data.playing)
    {
        let mut image = ImageNode {
            image_mode: bevy::ui::widget::NodeImageMode::Stretch,
            ..default()
        };
        player.apply_frame(&mut image);
        let size = asset
            .package
            .item(index)
            .map_or(Vec2::ZERO, |item| Vec2::new(item.width, item.height));
        content::spawn_visual(&mut entity, object, image, size, None, None);
        entity.insert(player);
    }

    // 装载器：按地址取资源。支持包内资源与直接的文件路径两种形式。
    if let ControlData::Loader(data) = gobject.control()
        && let Some(url) = data.url.as_deref()
    {
        let source_asset = if url.starts_with("ui://") {
            if asset.package.resolve_local_url(url).is_some() {
                Some(asset)
            } else {
                assets
                    .iter()
                    .find_map(|(_, asset)| asset.package.resolve_local_url(url).map(|_| asset))
            }
        } else {
            Some(asset)
        };
        if let Some(asset) = source_asset {
            if asset
                .package
                .resolve_local_url(url)
                .and_then(|index| asset.package.item(index))
                .is_some_and(|item| {
                    item.item_type == fairygui_core::package::PackageItemType::Component
                })
            {
                entity.insert(movie_sources::MovieSource {
                    url: url.to_string(),
                    visual: None,
                });
            } else if let Some(index) = asset.package.resolve_local_url(url)
                && let Some(player) =
                    movie_sources::player_for(asset, asset_server, index, data.frame, data.playing)
            {
                let mut image = ImageNode {
                    image_mode: bevy::ui::widget::NodeImageMode::Stretch,
                    ..default()
                };
                player.apply_frame(&mut image);
                let size = asset
                    .package
                    .item(index)
                    .map_or(Vec2::ZERO, |item| Vec2::new(item.width, item.height));
                let visual = content::spawn_visual(&mut entity, object, image, size, None, None);
                entity.insert((
                    player,
                    movie_sources::MovieSource {
                        url: url.to_string(),
                        visual: Some(visual),
                    },
                ));
            } else {
                match resolve_loader_url(asset, url) {
                    Some(LoadedSource {
                        image,
                        sprite,
                        grid,
                        scale_by_tile,
                    }) => {
                        let mut image_node = ImageNode {
                            image: asset_server.load(image),
                            image_mode: bevy::ui::widget::NodeImageMode::Stretch,
                            ..default()
                        };

                        // 包内图片要按图集切片取子区域。
                        if let Some(sprite) = &sprite {
                            image_node = backend::image_for(
                                sprite,
                                image_node.image,
                                grid.map(|g| g.grid),
                                scale_by_tile,
                            );
                        }

                        let (size, trim) = content::sprite_geometry(sprite.as_ref());
                        let visual = content::spawn_visual(
                            &mut entity,
                            object,
                            image_node,
                            size,
                            trim,
                            grid,
                        );
                        entity.insert(movie_sources::MovieSource {
                            url: url.to_string(),
                            visual: Some(visual),
                        });
                    }
                    None => {
                        tracing::debug!(url, "装载器地址无法解析，暂时留空");
                    }
                }
            }
        } else {
            entity.insert(movie_sources::MovieSource {
                url: String::new(),
                visual: None,
            });
        }
    }

    // 只有可滚动容器才需要滚动位置，其余容器不挂这个组件。
    if gobject
        .control()
        .component()
        .is_some_and(|data| data.overflow == OverflowType::Scroll)
    {
        entity.insert(ScrollPosition::default());
        if let Some(pane) = gobject
            .control()
            .component()
            .and_then(|c| c.scroll_pane.as_ref())
        {
            entity.with_children(|parent| {
                parent.spawn((
                    scroll_extent::ScrollExtent(object),
                    scroll_extent::node(gobject, pane),
                    bevy::picking::Pickable::IGNORE,
                    Visibility::Hidden,
                    Name::new("FairyGUI scroll extent"),
                ));
            });
        }
    }

    entities.insert(object, entity_id);

    if let Some(parent) = parent {
        commands.entity(parent).add_child(entity_id);
    }
    if let Some(data) = gobject.control().as_text_field() {
        let text = translations.translate(&data.text);
        let images = text_fonts::spawn_text(
            commands,
            entity_id,
            object,
            gobject,
            asset,
            assets,
            asset_server,
            image_assets,
            font,
            &text,
        );
        for image in images {
            if let Some(parent) = parent {
                commands.entity(image).insert(ChildOf(parent));
            } else {
                commands.entity(image).remove::<ChildOf>();
            }
        }
    }

    // 子对象的顺序即绘制顺序，保持与描述一致。
    let children: Vec<GObjectId> = gobject.children().to_vec();
    for (index, child) in children.into_iter().enumerate() {
        if let Some(child_entity) = spawn_object(
            commands,
            store,
            entities,
            asset,
            assets,
            object_assets,
            asset_server,
            image_assets,
            translations,
            font,
            child,
            Some(entity_id),
        ) {
            // Bevy normally uses insertion order for siblings.  FairyGUI can
            // reorder children at runtime (windows, popups and list items),
            // so keep an explicit local z value on every direct child.  This
            // also makes newly spawned children deterministic when the parent
            // already contains dynamic content.
            commands
                .entity(child_entity)
                .insert(ZIndex(child_paint_order(store, object, child, index)));
        }
    }

    Some(entity_id)
}

/// GList paint order is independent of logical row/index order. Refresh parts
/// stay behind content; pane bars stay above it, including with Arch ordering.
fn child_paint_order(store: &ObjectStore, parent: GObjectId, child: GObjectId, raw: usize) -> i32 {
    let Some(ControlData::List(data)) = store.object(parent).map(|o| o.control()) else {
        return raw as i32;
    };
    if store.is_scroll_refresh_overlay(child) {
        return raw as i32 - store.object(parent).unwrap().children().len() as i32;
    }
    if data.children_render_order == 0 {
        return raw as i32;
    }
    let rows = store.list_items(parent).unwrap_or_default();
    let Some(index) = rows.iter().position(|&id| id == child) else {
        return (rows.len() + raw) as i32;
    };
    let apex = (data.apex_index.max(0) as usize).min(rows.len().saturating_sub(1));
    if data.children_render_order == 1 {
        (rows.len() - 1 - index) as i32
    } else if index >= apex {
        (rows.len() - 1 - index + apex) as i32
    } else {
        index as i32
    }
}

/// 按对象当前引用的资源取出它的图集切片。
///
/// 有两条查找路径：
/// - **图片对象**记的是条目编号，按编号找；
/// - **装载器与按钮**的图标是地址（`url` / `icon`），齿轮与过渡改的正是它，
///   因此要按名字找。
///
/// 按对象记录的源包过滤资源编号；显式 ui:// 地址只接受它声明的所属包。
fn resolve_current_sprite(
    assets: &Assets<FuiAsset>,
    gobject: &fairygui_core::GObject,
) -> Option<fairygui_core::package::AtlasSprite> {
    if let Some(item_id) = gobject.package_item_id()
        && let Some(sprite) = assets.iter().find_map(|(_, asset)| {
            if gobject
                .source_package_id()
                .is_some_and(|id| id != asset.package.id())
            {
                return None;
            }
            let index = asset.package.item_index_by_id(item_id)?;
            asset.package.sprite_for(index).cloned()
        })
    {
        return Some(sprite);
    }

    let url = match gobject.control() {
        fairygui_core::widget::ControlData::Loader(data) => data.url.as_deref(),
        fairygui_core::widget::ControlData::Button(data) => data.icon.as_deref(),
        fairygui_core::widget::ControlData::Label(data) => data.icon.as_deref(),
        _ => None,
    }?;

    assets.iter().find_map(|(_, asset)| {
        let index = if url.starts_with("ui://") {
            asset.package.resolve_local_url(url)?
        } else {
            if gobject
                .source_package_id()
                .is_some_and(|id| id != asset.package.id())
            {
                return None;
            }
            asset.package.item_index_by_name(url.rsplit('/').next()?)?
        };
        asset.package.sprite_for(index).cloned()
    })
}

/// 装载器解析出来的资源。
struct LoadedSource {
    /// 图集或独立图片的文件路径。
    image: String,
    /// 若是包内图片，这里给出它在图集中的切片。
    sprite: Option<fairygui_core::package::AtlasSprite>,
    grid: Option<crate::image_fill::ImageGrid>,
    scale_by_tile: bool,
}

/// 解析装载器的资源地址。
///
/// 支持两种形式：
/// - 包内资源 `ui://<包名>/<条目名>`：图片条目取其所在图集与切片，其余取其文件；
/// - 其它地址按「相对包所在目录」的路径处理。
fn resolve_loader_url(asset: &FuiAsset, url: &str) -> Option<LoadedSource> {
    if url.is_empty() {
        return None;
    }
    let is_package_url = url.starts_with("ui://");

    if !is_package_url {
        return Some(LoadedSource {
            image: asset.resolve_asset_path(url),
            sprite: None,
            grid: None,
            scale_by_tile: false,
        });
    }

    let index = asset.package.resolve_local_url(url)?;
    let item = asset.package.item(index)?;
    if item.item_type != fairygui_core::package::PackageItemType::Image {
        return None;
    }

    // 图片条目：用它的图集文件与切片。
    if let Some(sprite) = asset.package.sprite_for(index)
        && let Some(file) = asset
            .package
            .item(sprite.atlas_item)
            .and_then(|atlas| atlas.file.clone())
    {
        return Some(LoadedSource {
            image: asset.resolve_asset_path(&file),
            sprite: Some(sprite.clone()),
            grid: crate::image_fill::ImageGrid::from_package(sprite, item),
            scale_by_tile: item.scale_by_tile,
        });
    }

    // 其它条目（图集、序列帧等）：直接用条目记录的文件。
    item.file.as_ref().map(|file| LoadedSource {
        image: asset.resolve_asset_path(file),
        sprite: None,
        grid: None,
        scale_by_tile: item.scale_by_tile,
    })
}

/// 位图字体文本的字形子实体。
///
/// 文本变化时整批替换：字形的数量与位置都会随内容变，逐个比对不划算。
#[derive(Component, Debug, Default)]
pub struct BitmapGlyphs(pub Vec<Entity>);

/// Plain bitmap field underline entities, owned separately from sampled glyphs.
#[derive(Component, Debug, Default)]
pub struct BitmapDecorations(pub Vec<Entity>);

/// Logical pixel measurements from native bitmap text layout, independent of
/// Bevy's physical node size. Empty/missing-only text keeps bitmap ownership.
#[derive(Component, Debug, Clone, Copy)]
pub struct BitmapTextMetrics {
    pub text_size: fairygui_core::Size,
    pub object_size: fairygui_core::Size,
}

fn bitmap_layout(
    font: &fairygui_core::package::BitmapFontData,
    data: &fairygui_core::widget::TextFieldData,
    text: &str,
    size: fairygui_core::Size,
) -> (
    Vec<crate::text::PlacedGlyph>,
    BitmapTextMetrics,
    Vec<fairygui_core::geom::Rect>,
) {
    let layout = fairygui_core::bitmap_text::layout_bitmap_text(font, data, text, size);
    let glyphs = layout
        .glyphs
        .into_iter()
        .map(|g| crate::text::PlacedGlyph {
            tint: font.tint,
            x: g.x,
            y: g.y,
            width: g.width,
            height: g.height,
            texture: font.glyph(g.char_code).and_then(|g| g.texture.clone()),
        })
        .collect();
    (
        glyphs,
        BitmapTextMetrics {
            text_size: layout.text_size,
            object_size: layout.object_size,
        },
        layout.underlines,
    )
}

fn spawn_text_decoration(
    commands: &mut Commands,
    parent: Entity,
    owner: GObjectId,
    rect: fairygui_core::geom::Rect,
    color: Option<fairygui_core::color::Color>,
) -> Entity {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(rect.x),
                top: Val::Px(rect.y),
                width: Val::Px(rect.width),
                height: Val::Px(rect.height),
                ..default()
            },
            BackgroundColor::default(),
            appearance::TextDecoration,
            appearance::TextTintSource { owner, color },
            bevy::picking::Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id()
}

/// 为一个字形创建子节点。字形没有可渲染的来源时返回 `None`。
fn spawn_glyph(
    commands: &mut Commands,
    parent: Entity,
    owner: GObjectId,
    glyph: &crate::text::PlacedGlyph,
    asset: &FuiAsset,
    asset_server: &AssetServer,
) -> Option<Entity> {
    use fairygui_core::package::GlyphTexture;

    // 字形的纹理有两种来源：字体自带图集里的一个矩形，或另一个图片条目的
    // 图集切片。两者最终都归结为「哪个图集文件 + 图集内的矩形」。
    let mut position = Vec2::new(glyph.x, glyph.y);
    let mut size = Vec2::new(glyph.width, glyph.height);
    let (file, rect) = match glyph.texture.as_ref()? {
        GlyphTexture::FontAtlas { atlas_item, rect } => {
            let file = asset
                .package
                .item(*atlas_item)
                .and_then(|entry| entry.file.clone())?;
            (file, *rect)
        }
        GlyphTexture::Item { item } => {
            let sprite = asset.package.sprite_for(*item)?;
            if sprite.original_size.x <= 0.0 || sprite.original_size.y <= 0.0 {
                return None;
            }
            let scale = size / Vec2::new(sprite.original_size.x, sprite.original_size.y);
            position += Vec2::new(sprite.offset.x, sprite.offset.y) * scale;
            size = Vec2::new(sprite.rect.width, sprite.rect.height) * scale;
            let file = asset
                .package
                .item(sprite.atlas_item)
                .and_then(|entry| entry.file.clone())?;
            (file, sprite.rect)
        }
    };

    let path = asset.resolve_asset_path(&file);
    let child = commands
        .spawn((
            bevy::picking::Pickable::IGNORE,
            appearance::TextTintSource {
                owner,
                color: (!glyph.tint).then_some(fairygui_core::color::Color::WHITE),
            },
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(position.x),
                top: Val::Px(position.y),
                width: Val::Px(size.x),
                height: Val::Px(size.y),
                ..default()
            },
            ImageNode {
                image: asset_server.load(path),
                image_mode: bevy::ui::widget::NodeImageMode::Stretch,
                rect: Some(Rect::new(
                    rect.x,
                    rect.y,
                    rect.x + rect.width,
                    rect.y + rect.height,
                )),
                ..default()
            },
        ))
        .id();

    commands.entity(parent).add_child(child);
    Some(child)
}

/// 每帧推进滚动容器的惯性。
///
/// 惯性与回弹都要跨帧累积，因此状态保存在核心层，这里只负责驱动。
pub fn advance_scrolls(mut runtime: NonSendMut<GuiRuntime>, time: Res<Time>) {
    runtime.store_mut().advance_scrolls(time.delta_secs());
}

/// 每帧推进过渡动画。
pub fn advance_transitions(mut runtime: NonSendMut<GuiRuntime>, time: Res<Time>) {
    runtime.store_mut().advance_transitions(time.delta_secs());
}

/// 每帧推进齿轮补间。
pub fn advance_gear_tweens(mut runtime: NonSendMut<GuiRuntime>, time: Res<Time>) {
    runtime.store_mut().advance_gear_tweens(time.delta_secs());
}

/// 每帧推进通用补间。
pub fn advance_tweens(mut runtime: NonSendMut<GuiRuntime>, time: Res<Time>) {
    runtime.store_mut().advance_tweens(time.delta_secs());
}

/// Last model/display values used to synchronize an input's editor buffer.
/// Maintained by the runtime; application text edits belong in the core model.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct InputTextState {
    source: String,
    display: String,
}
type RichTextLayout<'a> = (
    &'a ComputedTextBlock,
    &'a bevy::text::TextLayoutInfo,
    &'a Node,
    &'a UiTransform,
    &'a ComputedNode,
);
type RichTextLayoutFilter = (With<RichTextBlock>, Without<InlineImage>);

/// Position atlas quads inside their original-canvas text inline boxes.
pub(crate) fn position_inline_images(
    rich_text: Query<RichTextLayout<'_>, RichTextLayoutFilter>,
    mut images: Query<
        (&InlineImageOwner, &InlineImage, &mut Node, &mut UiTransform),
        Without<RichTextBlock>,
    >,
) {
    for (owner, image, mut node, mut transform) in &mut images {
        let Ok((block, layout, text_node, text_transform, computed)) = rich_text.get(owner.0)
        else {
            node.display = Display::None;
            continue;
        };
        let bounds = block
            .entities()
            .iter()
            .any(|entity| entity.entity == image.inline_box)
            .then(|| {
                layout.inline_boxes.iter().find(|(entity, kind, _)| {
                    *entity == image.inline_box && *kind == bevy::text::InlineBoxKind::InFlow
                })
            })
            .flatten()
            .map(|(_, _, bounds)| bounds);
        let Some(bounds) = bounds else {
            node.display = Display::None;
            continue;
        };
        let scale = layout.scale_factor.max(f32::EPSILON);
        let base = Vec2::new(px(text_node.left), px(text_node.top));
        let local = (computed.content_inset().min_inset + bounds.min) / scale + image.offset;
        let left = Val::Px(base.x + local.x);
        let top = Val::Px(base.y + local.y);
        let next_transform = rich_text::image_transform(
            text_transform,
            computed.size() * computed.inverse_scale_factor(),
            local,
            image.size,
        );
        if *transform != next_transform {
            *transform = next_transform;
        }
        if node.left != left || node.top != top || node.display != Display::Flex {
            node.left = left;
            node.top = top;
            node.display = Display::Flex;
        }
    }
}

fn px(value: Val) -> f32 {
    match value {
        Val::Px(value) => value,
        _ => 0.0,
    }
}

/// Synchronize actual EditableText values and core edits without resetting the
/// editor on font changes. Explicit model changes take precedence over old text.
#[allow(clippy::type_complexity)]
pub fn sync_text_inputs(
    mut commands: Commands,
    mut runtime: NonSendMut<GuiRuntime>,
    mut query: Query<(
        Entity,
        &GObjectRef,
        &mut bevy::text::EditableText,
        Option<&mut Text>,
        Option<&mut InputTextState>,
    )>,
) {
    for (entity, entity_ref, mut editor, text, state) in &mut query {
        let object = entity_ref.0;
        let Some(current) = runtime.store().object(object).and_then(|item| {
            if let ControlData::TextInput(data) = item.control() {
                Some((
                    data.text_field.text.clone(),
                    !data.text_field.single_line,
                    data.password,
                    data.text_field.ubb_enabled,
                ))
            } else {
                None
            }
        }) else {
            continue;
        };
        let (current, multiline, password, ubb) = current;
        let translated = fairygui_core::text::input::normalize_input_lines(
            &runtime.translate(&current),
            multiline,
        );
        let value = editor.value().to_string();
        // Explicit model/translation edits win over a concurrent editor change.
        // Font/layout updates alone must not reset selection or IME composition.
        let model_changed = state
            .as_ref()
            .is_some_and(|state| state.source != current || state.display != translated);
        let next = if model_changed {
            if state.as_ref().is_some_and(|state| state.source != current)
                && runtime.focused_input == Some(object)
            {
                runtime.emit_input_event(
                    fairygui_core::object::EventKind::Change,
                    object,
                    &translated,
                );
            }
            if value != translated {
                editor.editor.set_text(&translated);
            }
            InputTextState {
                source: current,
                display: translated,
            }
        } else if state.as_ref().is_some_and(|state| state.display == value) {
            InputTextState {
                source: current,
                display: translated,
            }
        } else {
            if current != value {
                runtime.store_mut().set_object_text(object, &value);
            }
            InputTextState {
                source: value.clone(),
                display: value,
            }
        };
        let display = if password {
            fairygui_core::text::input::password_text(&next.display, ubb)
        } else {
            next.display.clone()
        };
        if let Some(mut text) = text
            && text.0 != display
        {
            text.0 = display;
        }
        if let Some(mut state) = state {
            if *state != next {
                *state = next;
            }
        } else {
            commands.entity(entity).insert(next);
        }
    }
}

/// 把脏对象的变化同步到已有实体。
///
/// 只有被属性 setter 标记过的对象才会被处理，因此界面静止时几乎没有开销。
// 这里一次性列出同步所需的全部组件；拆成类型别名反而更难对照。
#[allow(clippy::type_complexity)]
pub(crate) fn sync_dirty(
    mut commands: Commands,
    mut runtime: NonSendMut<GuiRuntime>,
    assets: Res<Assets<FuiAsset>>,
    asset_server: Res<AssetServer>,
    decorations: Query<&BitmapDecorations>,
    font_bindings: Query<&text_fonts::TextFontBinding>,
    mut query: Query<(
        &GObjectRef,
        &mut Node,
        &mut UiTransform,
        &mut Visibility,
        Option<&mut ScrollPosition>,
        Option<&mut ImageNode>,
        Option<&BaseTint>,
        Option<&mut Text>,
        Option<&mut TextFont>,
        Option<&mut bevy::text::LineHeight>,
        Option<&mut BitmapGlyphs>,
        Option<&RichTextBlock>,
    )>,
) {
    let dirty_objects = runtime.store_mut().take_dirty();
    if dirty_objects.is_empty() {
        return;
    }

    // 滚动位置变化的容器，循环结束后再统一同步滚动条。
    let mut scroll_bars = Vec::new();
    // 子对象发生增删的对象，循环结束后再补实体。
    let mut children_changed = Vec::new();
    // 滚动过的容器也要通知虚拟列表——把通知挂在滚动事件上，使用方就不必轮询。
    let mut scrolled = Vec::new();
    // 文本变化过的位图字体文本。字形是子实体，重排要改实体树，因此也要
    // 等到循环结束——此刻还借着对象存储。
    let mut glyph_updates = Vec::new();

    for (object, flags) in dirty_objects {
        let Some(entity) = runtime.entity_of(object) else {
            continue;
        };
        let Ok((
            _,
            mut node,
            mut transform,
            mut visibility,
            scroll_position,
            mut image_node,
            _base_tint,
            mut text,
            mut text_font,
            mut line_height,
            bitmap_glyphs,
            rich_text,
        )) = query.get_mut(entity)
        else {
            continue;
        };
        // 透明度沿祖先累积，渲染时取的是有效值而不是自身那一个——
        // 否则容器设了透明度，里面的子对象丝毫不变。
        let Some(gobject) = runtime.store().object(object) else {
            continue;
        };

        if flags & dirty::XY != 0 {
            node.left = Val::Px(gobject.x());
            node.top = Val::Px(gobject.y());
        }

        if flags & dirty::SIZE != 0 {
            node.width = Val::Px(gobject.width());
            node.height = Val::Px(gobject.height());
        }

        // 位图字体的文本没有 Text 组件——它的内容是字形子实体，要整批重排。
        if flags & (dirty::TEXT | dirty::SIZE) != 0
            && bitmap_glyphs.is_some()
            && !gobject
                .control()
                .as_text_field()
                .is_some_and(|d| d.ubb_enabled || d.html_enabled)
        {
            glyph_updates.push(object);
        }

        // 文本内容变化：重新取一次并改写。齿轮、过渡、控制器动作都可能改它。
        if flags & dirty::TEXT != 0
            && rich_text.is_none()
            && let Some(text) = text.as_mut()
        {
            let content = match gobject.control() {
                ControlData::Text(data) => runtime.translate(&data.text),
                ControlData::TextInput(data) => {
                    text_fonts::display_text(gobject, &runtime.translate(&data.text_field.text))
                }
                _ => String::new(),
            };

            if text.0 != content {
                text.0 = content;
            }
        }
        if flags & dirty::TEXT != 0
            && rich_text.is_none()
            && let Some(data) = gobject.control().as_text_field()
        {
            if text.is_some() {
                commands
                    .entity(entity)
                    .insert(backend::text_layout_for_object(gobject, data));
                if data.underline {
                    commands.entity(entity).insert(Underline);
                } else {
                    commands.entity(entity).remove::<Underline>();
                }
            }
            if let Some(font) = text_font.as_mut() {
                let mut next = backend::text_font_for(gobject, data, runtime.font.as_ref());
                if let Ok(binding) = font_bindings.get(entity) {
                    next.font = binding.raster().clone();
                }
                if **font != next {
                    **font = next;
                }
            }
            if let Some(height) = line_height.as_mut() {
                let next = backend::text_line_height_for(data, None);
                if **height != next {
                    **height = next;
                }
            }
        }

        // 图标或纹理变化：按新的条目重取图集切片。
        //
        // 这里只更新子区域，不换纹理句柄——同一切换通常发生在同一张图集内。
        // 若切换到了另一张图集，需要重建实体才会生效。
        if flags & dirty::IMAGE != 0
            && let Some(image_node) = image_node.as_mut()
            && let Some(sprite) = resolve_current_sprite(&assets, gobject)
        {
            image_node.rect = Some(Rect::new(
                sprite.rect.x,
                sprite.rect.y,
                sprite.rect.x + sprite.rect.width,
                sprite.rect.y + sprite.rect.height,
            ));
        }

        if flags & dirty::LOOK != 0 {
            *transform = backend::transform_for(gobject);
            *visibility = if runtime.store().display_visible(object) {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }

        if flags & dirty::SCROLL != 0 {
            if let Some(mut scroll) = scroll_position
                && let Some(pane) = gobject
                    .control()
                    .component()
                    .and_then(|data| data.scroll_pane.as_ref())
            {
                scroll.0 = Vec2::new(pane.display_pos().x, pane.display_pos().y);
            }

            // 滑块的位置与尺寸都要跟着滚动比例走。此刻还借着 gobject，
            // 先记下来，等循环结束再统一处理。
            if gobject
                .control()
                .component()
                .is_some_and(|data| data.scroll_pane.is_some())
            {
                scroll_bars.push(object);
                scrolled.push(object);
            }
        }

        // 子对象增删：新加进来的那支要补实体。不该出现的那些由移除路径
        // 回收，这里只管把缺的补上。
        if flags & dirty::CHILDREN != 0 {
            children_changed.push(object);
        }
    }

    for container in scroll_bars {
        runtime.store_mut().sync_scroll_bars(container);
    }

    // 只对登记过的列表发出通知；未登记的会被 note_scrolled 忽略。
    for container in scrolled {
        runtime.note_scrolled(container);
    }

    for object in children_changed {
        runtime.queue_subtree(object);

        // A CHILDREN dirty mark also covers reordering (for example
        // Root::bring_to_front).  Refresh z values for entities that already
        // exist; queued entities will receive their value during spawning.
        if let Some(parent_object) = runtime.store().object(object) {
            for (index, child) in parent_object.children().iter().copied().enumerate() {
                if let Some(child_entity) = runtime.entity_of(child) {
                    commands
                        .entity(child_entity)
                        .insert(ZIndex(child_paint_order(
                            runtime.store(),
                            object,
                            child,
                            index,
                        )));
                }
            }
        }
    }

    // 位图字体的字形重排：旧的整批销毁，再按当前文本重新排一遍。
    // 字形没有 Text 组件，不这样做的话改了文本只会看到原样不动的旧字形。
    for object in glyph_updates {
        let Some(handle) = runtime.asset_handle_for(object) else {
            continue;
        };
        let Some(asset) = assets.get(&handle) else {
            continue;
        };
        let Some(entity) = runtime.entity_of(object) else {
            continue;
        };
        let Ok((.., glyphs, _)) = query.get_mut(entity) else {
            continue;
        };
        let Some(mut glyphs) = glyphs else {
            continue;
        };

        for glyph in glyphs.0.drain(..) {
            commands.entity(glyph).despawn();
        }

        let Some(gobject) = runtime.store().object(object) else {
            continue;
        };
        let Some(data) = gobject.control().as_text_field() else {
            continue;
        };
        let content = text_fonts::display_text(gobject, &runtime.translate(&data.text));
        let Some((font_asset, font)) = data
            .font
            .as_deref()
            .and_then(|name| text_fonts::resolve_bitmap_font(asset, &assets, name))
        else {
            continue;
        };

        if let Ok(parts) = decorations.get(entity) {
            for &part in &parts.0 {
                commands.entity(part).despawn();
            }
        }
        let (placed, metrics, underlines) = bitmap_layout(&font, data, &content, gobject.size());
        let parts = underlines
            .into_iter()
            .map(|rect| spawn_text_decoration(&mut commands, entity, object, rect, None))
            .collect();
        commands.entity(entity).insert(BitmapDecorations(parts));
        commands.entity(entity).insert(metrics);
        for glyph in &placed {
            if let Some(child) = spawn_glyph(
                &mut commands,
                entity,
                object,
                glyph,
                font_asset,
                &asset_server,
            ) {
                glyphs.0.push(child);
            }
        }
    }
}

/// 每帧推进分帧构建。已完成的构建会转成正式界面并在下一帧生成实体。
pub fn advance_async_builds(mut runtime: NonSendMut<GuiRuntime>, assets: Res<Assets<FuiAsset>>) {
    runtime.advance_builds(&assets);
}

pub fn sync_ui_config(config: Res<GuiConfig>, mut runtime: NonSendMut<GuiRuntime>) {
    if config.is_changed() {
        runtime.set_scroll_defaults(config.scroll);
    }
}

/// Capture the global font before roots, dynamic views and text are generated.
pub fn sync_gui_font(font: Res<GuiFont>, mut runtime: NonSendMut<GuiRuntime>) {
    if runtime.font != font.0 {
        runtime.set_font(font.0.clone());
    }
}

/// Refresh ordinary component panes after child geometry and ownership changes.
pub(crate) fn sync_component_scroll_content(mut runtime: NonSendMut<GuiRuntime>) {
    let panes: Vec<_> = runtime
        .store()
        .iter()
        .filter_map(|(id, object)| {
            (object
                .control()
                .component()
                .is_some_and(|component| component.scroll_pane.is_some())
                && !matches!(
                    object.control(),
                    ControlData::List(_) | ControlData::Tree(_)
                ))
            .then_some(id)
        })
        .collect();
    for pane in panes {
        runtime.store_mut().layout_component_scroll_content(pane);
    }
}

/// 把引擎算出的文本尺寸回填到核心层，驱动自动尺寸策略。
/// Bevy 布局与实体生成之间存在一帧延迟，依赖文本自动尺寸的布局会晚一帧到位。
pub fn sync_text_sizes(
    mut runtime: NonSendMut<GuiRuntime>,
    query: Query<(&GObjectRef, &ComputedNode, Option<&BitmapTextMetrics>)>,
) {
    for (object_ref, node, bitmap) in &query {
        if runtime.text_measurer.borrow().contains(object_ref.0) {
            continue;
        }
        let Some(object) = runtime.store().object(object_ref.0) else {
            continue;
        };

        // 只有明确要求自动尺寸的文本才回填，其余尺寸由描述决定。
        let auto_size = match object.control() {
            ControlData::Text(data) => data.auto_size,
            ControlData::TextInput(data) => data.text_field.auto_size,
            _ => continue,
        };
        if auto_size == 0 || auto_size == 3 || auto_size == 4 {
            continue;
        }
        if let Some(bitmap) = bitmap {
            let size = bitmap.object_size;
            if object.size() != size {
                runtime
                    .store_mut()
                    .set_size(object_ref.0, size.width, size.height);
            }
            continue;
        }

        // Layout reports physical pixels; the object model stores logical pixels.
        // Feeding physical size back makes auto-sized text grow every frame at HiDPI.
        let size = node.size() * node.inverse_scale_factor();
        if size.x <= 0.0 || size.y <= 0.0 {
            continue;
        }

        // 留一点容差，避免因亚像素差异反复写回。
        let changed =
            (size.x - object.width()).abs() > 0.5 || (size.y - object.height()).abs() > 0.5;
        if changed {
            let id = object_ref.0;
            runtime.store_mut().set_size(id, size.x, size.y);
        }
    }
}

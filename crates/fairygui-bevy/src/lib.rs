//! FairyGUI 运行时的 Bevy 集成层。
//!
//! 面向应用的界面生命周期、事件响应与控件修改都通过 Bevy ECS 完成：用
//! [`FairyGuiRoot`] 拥有界面，用 [`GuiQuery`] 查找控件实体，用 [`GuiEvent`]
//! observer 响应交互，直接修改可变属性组件（如 [`GuiText`]、[`GuiTextFormat`]）；
//! 播放、滚动等使用 [`GuiMut`]。
//!
//! [`fairygui_core`] 在内部执行 FairyGUI 包语义，运行时把结果同步到对应的 ECS
//! 实体与 Bevy UI 组件。应用代码通常不需要直接访问 [`GuiRuntime`] 或
//! `ObjectStore`；它们保留给尚未封装为 ECS 操作的高级能力。
//!
//! # 分层约定
//!
//! 核心层不依赖 Bevy。渲染表示由 [`backend`]、[`text`] 与 [`movie`] 负责，
//! [`runtime`] 管理模型与实体之间的同步，[`input`] 管理指针路由与捕获。
//! 应用 observer 与消息处理系统放入 [`GuiSystems::Respond`]，修改的组件会在本帧
//! [`GuiSystems::Sync`] 应用并同步到 Bevy UI 组件。
//!
//! # 渲染方案
//!
//! 每个对象对应一个绝对定位的 [`bevy::ui::Node`]，按需附加图片、文本与二维变换。
//! 图集切片不生成子纹理，直接用节点的子区域引用截取。
//!
//! # 用法
//!
//! ```no_run
//! use bevy::prelude::*;
//! use fairygui_bevy::FairyGuiPlugin;
//!
//! App::new()
//!     .add_plugins((DefaultPlugins, FairyGuiPlugin))
//!     .run();
//! ```
//!
//! 创建界面的方式是先加载 `.fui` 资产，再把它交给运行期：
//!
//! ```no_run
//! # use bevy::prelude::*;
//! # use fairygui_bevy::FairyGuiRoot;
//! fn spawn_ui(mut commands: Commands, asset_server: Res<AssetServer>) {
//!     commands.spawn(FairyGuiRoot::new(
//!         asset_server.load("ui/MainMenu.fui"),
//!         "Main",
//!     ));
//! }
//! ```
//!
//! The plugin waits for the asset, creates the root once and removes it when
//! the component is removed. `GuiRootInstance` exposes a stable entity-owned
//! root reference when advanced package operations need it.

pub mod animation;
pub mod asset;
pub use animation::{GuiAnimationPlaySettings, GuiAnimationState};
pub mod backend;
pub mod combo;
pub use combo::GuiComboBoxState;
mod component_properties;
mod compositor;
pub use component_properties::GuiOpaque;
pub mod ecs;
mod effect_properties;
pub use effect_properties::{GuiBlendMode, GuiColorFilter};
pub mod extension;
mod graph;
mod graph_properties;
pub use graph_properties::GuiGraph;
mod image_fill;
pub mod input;
mod input_properties;
pub use input_properties::GuiTextInputSettings;
mod button_properties;
pub use button_properties::GuiButtonSettings;
pub mod list;
pub mod movie;
pub mod popup;
pub mod progress;
mod properties;
mod scroll_properties;
pub use scroll_properties::GuiScrollSettings;
mod visual_properties;
pub use visual_properties::{GuiColor, GuiImageFill, GuiImageFlip, GuiLoaderSettings};
pub mod pull;
pub use progress::{GuiProgressState, GuiProgressTweenComplete};
pub mod runtime;
pub use runtime::root_mount::GuiRootMountState;
pub use runtime::{GuiMut, GuiObjectMut, GuiWorldExt};
pub mod slider;
pub use slider::GuiSliderState;
pub mod text;
mod text_format;
pub use text_format::GuiTextFormat;
mod text_stroke;
pub mod tree;
pub mod tween;
pub use tween::{GuiTweenComplete, GuiTweenProperty, GuiTweenTarget};

pub use asset::{FuiAsset, FuiAssetLoader, FuiLoadError};
pub use ecs::{
    GuiControllerState, GuiControllers, GuiDisplay, GuiIcon, GuiObjectInfo, GuiPullPart, GuiQuery,
    GuiRenderOrder, GuiRootView, GuiScrollState, GuiSortingOrder, GuiSpace, GuiText, GuiTransform,
    GuiTransitionState, GuiTransitions,
};
pub use extension::{FairyGuiAppExt, GuiExtension, GuiExtensionError, GuiExtensionState};
pub use fairygui_bevy_macros::GuiExtension;
pub use input::{
    GuiInputCommand, GuiInputSettings, GuiInputState, GuiPointerCapture, GuiPointerId,
};
pub use list::{
    GuiListContent, GuiListContentError, GuiListContentState, GuiListItem, GuiListRow,
    GuiListState, GuiVirtualList, GuiVirtualListState,
};
pub use popup::{GuiPopupCommand, GuiPopupDirection, GuiPopupMenu, GuiPopupState};
pub use pull::GuiPullCommand;
pub use runtime::ecs_drag_drop::{
    GuiDragCommand, GuiDragSource, GuiDragState, GuiDrop, GuiDropTarget,
};
pub use runtime::global_wait::{
    GuiGlobalWaitCommand, GuiGlobalWaitError, GuiGlobalWaitSettings, GuiGlobalWaitState,
};
pub use runtime::object_drag::{GuiDragBounds, GuiDraggable, GuiObjectDragState};
pub use runtime::tooltip::{
    GuiTooltipCommand, GuiTooltipError, GuiTooltipSettings, GuiTooltipState,
};
pub use runtime::windows::{FairyGuiWindow, GuiWindowAction, GuiWindowCommand, GuiWindowState};
pub use runtime::{
    FairyGuiRoot, GObjectRef, GuiAnimationComplete, GuiButtonSelected, GuiConfig, GuiEvent,
    GuiFont, GuiLink, GuiListItemClick, GuiListSelection, GuiLoaderFailure, GuiLoaderState,
    GuiMenuAction, GuiProgressValue, GuiPullRelease, GuiRootInstance, GuiRuntime, GuiSliderValue,
    GuiTextInput, GuiTransitionCallback, GuiTransitionComplete, GuiTransitionHook,
    PopupMenuTemplates,
};
pub use tree::{
    GuiTreeContent, GuiTreeContentError, GuiTreeContentState, GuiTreeItem, GuiTreeNode,
    GuiTreeNodeState, GuiTreeState,
};

pub use fairygui_core as core;
pub use fairygui_core::widget::{ButtonMode, ChildrenRenderOrder, FillMethod, FlipType, GraphType};
pub use fairygui_core::{color_matrix::ColorFilterParams, object::BlendMode};

use bevy::prelude::*;

/// Ordered frame phases. Put application UI handlers in `Respond` so mutations
/// are mirrored into Bevy entities in the same frame.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GuiSystems {
    Input,
    Publish,
    Respond,
    Advance,
    Sync,
    /// Inside Sync, after root views and logical snapshots exist, before list
    /// and tree data are prepared. Attach control components and observers here
    /// to configure newly loaded roots before their first rendered frame.
    Initialize,
    /// Inside Sync, after native list layout and row spawning, before projection.
    /// Read fresh logical snapshots and edit components or call direct methods here.
    /// This precedes Bevy's PostUpdate UI layout and text measurement.
    PostLayout,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SyncPhase {
    Materialize,
    Prepare,
    Project,
}

/// Common application imports; the existing crate-root API remains supported.
pub mod prelude {
    pub use crate::{
        BlendMode, ButtonMode, ChildrenRenderOrder, ColorFilterParams, FairyGuiPlugin,
        FairyGuiRoot, FairyGuiWindow, FillMethod, FlipType, FuiAsset, GObjectRef, GraphType,
        GuiAnimationComplete, GuiAnimationPlaySettings, GuiAnimationState, GuiBlendMode,
        GuiButtonSelected, GuiButtonSettings, GuiColor, GuiColorFilter, GuiComboBoxState,
        GuiConfig, GuiControllerState, GuiControllers, GuiDisplay, GuiDragBounds, GuiDragCommand,
        GuiDragSource, GuiDragState, GuiDraggable, GuiDrop, GuiDropTarget, GuiEvent, GuiFont,
        GuiGlobalWaitCommand, GuiGlobalWaitError, GuiGlobalWaitSettings, GuiGlobalWaitState,
        GuiGraph, GuiIcon, GuiImageFill, GuiImageFlip, GuiInputCommand, GuiInputSettings,
        GuiInputState, GuiLink, GuiListContent, GuiListContentError, GuiListContentState,
        GuiListItem, GuiListItemClick, GuiListRow, GuiListSelection, GuiListState,
        GuiLoaderFailure, GuiLoaderSettings, GuiLoaderState, GuiMenuAction, GuiMut,
        GuiObjectDragState, GuiObjectInfo, GuiObjectMut, GuiOpaque, GuiPointerCapture,
        GuiPointerId, GuiPopupCommand, GuiPopupDirection, GuiPopupMenu, GuiPopupState,
        GuiProgressState, GuiProgressTweenComplete, GuiProgressValue, GuiPullCommand, GuiPullPart,
        GuiPullRelease, GuiQuery, GuiRenderOrder, GuiRootInstance, GuiRootMountState, GuiRootView,
        GuiRuntime, GuiScrollSettings, GuiScrollState, GuiSliderState, GuiSliderValue,
        GuiSortingOrder, GuiSpace, GuiSystems, GuiText, GuiTextFormat, GuiTextInput,
        GuiTextInputSettings, GuiTooltipCommand, GuiTooltipError, GuiTooltipSettings,
        GuiTooltipState, GuiTransform, GuiTransitionCallback, GuiTransitionComplete,
        GuiTransitionHook, GuiTransitionState, GuiTransitions, GuiTreeContent, GuiTreeContentError,
        GuiTreeContentState, GuiTreeItem, GuiTreeNode, GuiTreeNodeState, GuiTreeState,
        GuiTweenComplete, GuiTweenProperty, GuiTweenTarget, GuiVirtualList, GuiVirtualListState,
        GuiWindowAction, GuiWindowCommand, GuiWindowState, GuiWorldExt, PopupMenuTemplates,
    };
    pub use crate::{FairyGuiAppExt, GuiExtension, GuiExtensionError, GuiExtensionState};
}

/// FairyGUI 集成插件。
///
/// 注册 `.fui` 资产类型与加载器，把界面运行期接入主线程，
/// 并把指针事件桥接成界面事件。
#[derive(Debug, Default)]
pub struct FairyGuiPlugin;

impl Plugin for FairyGuiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            input::apply_scroll_drag_commands
                .after(GuiSystems::Respond)
                .before(GuiSystems::Advance),
        );
        // Text source snapshots also run in renderer-free apps. Preserve an
        // existing ImagePlugin store; init_asset would replace it if repeated.
        if !app.world().contains_resource::<Assets<Image>>() {
            app.init_asset::<Image>();
        }
        if !app.world().contains_resource::<Assets<Font>>() {
            app.init_asset::<Font>();
        }
        graph::install(app);
        image_fill::install(app);
        text_stroke::install(app);
        app.configure_sets(
            Update,
            (
                GuiSystems::Input,
                GuiSystems::Publish,
                GuiSystems::Respond,
                GuiSystems::Advance,
                GuiSystems::Sync,
            )
                .chain(),
        )
        .configure_sets(
            Update,
            (
                SyncPhase::Materialize,
                GuiSystems::Initialize,
                SyncPhase::Prepare,
                GuiSystems::PostLayout,
                SyncPhase::Project,
            )
                .chain()
                .in_set(GuiSystems::Sync),
        )
        .add_message::<GuiLink>()
        .add_message::<GuiMenuAction>()
        .add_message::<GuiTransitionHook>()
        .add_message::<GuiTransitionCallback>()
        .add_message::<GuiTransitionComplete>()
        .add_message::<GuiAnimationComplete>()
        .add_message::<GuiTextInput>()
        .add_message::<GuiListItemClick>()
        .add_message::<GuiPullRelease>()
        .add_message::<GuiPullCommand>()
        .add_message::<GuiPopupCommand>()
        .add_message::<GuiWindowCommand>()
        .add_message::<GuiGlobalWaitCommand>()
        .add_message::<GuiTooltipCommand>()
        .add_message::<GuiDragCommand>()
        .init_resource::<GuiDragState>()
        .init_resource::<GuiGlobalWaitSettings>()
        .init_resource::<GuiGlobalWaitState>()
        .init_resource::<GuiTooltipSettings>()
        .init_resource::<GuiTooltipState>()
        .init_resource::<GuiPopupState>()
        .init_asset::<FuiAsset>()
        .init_asset_loader::<FuiAssetLoader>()
        .init_resource::<GuiConfig>()
        .init_non_send::<GuiRuntime>()
        .add_systems(
            Update,
            // Consume Respond edits before any clock advances. In particular,
            // an ECS pause must stop this tick instead of one tick later.
            (properties::sync, pull::apply_pull_commands)
                .chain()
                .after(GuiSystems::Respond)
                .before(GuiSystems::Advance),
        )
        .add_systems(
            Update,
            (
                runtime::sync_ui_config,
                runtime::sync_gui_font,
                runtime::text_measure::sync_assets,
                runtime::advance_async_builds,
                runtime::advance_scrolls,
                runtime::advance_transitions,
                runtime::advance_gear_tweens,
                runtime::advance_tweens,
                runtime::sync_movie_sources,
                movie::advance_movie_clips,
                runtime::advance_inline_movie_clips,
                movie::discard_unused_animation_commands,
            )
                .chain()
                .in_set(GuiSystems::Advance),
        )
        .add_systems(
            Update,
            (
                runtime::sync_list_selection_components,
                properties::sync,
                ecs::sync_gui_scroll_state,
                (ecs::sync_gui_transitions, animation::sync_state).chain(),
                popup::sync_popup_state,
                runtime::windows::sync_window_state,
                (
                    runtime::global_wait::sync_global_wait_state,
                    runtime::tooltip::sync_tooltip_state,
                )
                    .chain(),
                runtime::ecs_drag_drop::sync_drag_state,
                runtime::object_drag::sync_object_drag_state,
                tree::sync_tree_state,
                runtime::sync_loader_states,
                (
                    runtime::emit_gui_events,
                    runtime::emit_link_messages,
                    runtime::emit_menu_actions,
                    runtime::emit_text_input_messages,
                    runtime::emit_list_item_messages,
                    runtime::emit_pull_release_messages,
                    runtime::ecs_drag_drop::emit_drops,
                ),
            )
                .chain()
                .in_set(GuiSystems::Publish),
        )
        .add_systems(
            Update,
            (
                runtime::sync_loader_components,
                runtime::sync_loader_status,
                runtime::cleanup_retained_popups,
                runtime::spawn_pending_roots,
                ecs::sync_gui_objects,
                properties::sync,
                ecs::sync_gui_scroll_state,
                extension::construct,
            )
                .chain()
                .in_set(SyncPhase::Materialize),
        )
        .add_systems(
            Update,
            (
                input::apply_scroll_drag_commands,
                properties::sync,
                runtime::text_measure::sync_sizes,
                (
                    list::sync_list_content,
                    tree::sync_tree_content,
                    list::sync_virtual_list_bindings,
                    runtime::sync_virtual_lists,
                    runtime::sync_component_scroll_content,
                )
                    .chain(),
                runtime::spawn_pending_objects,
                (
                    runtime::sync_movie_sources,
                    movie::sync_movie_controls,
                    movie::discard_unused_animation_commands,
                    animation::sync_state,
                )
                    .chain(),
                ecs::sync_gui_objects,
                properties::sync,
                ecs::sync_gui_scroll_state,
                (tree::sync_tree_state, list::sync_list_state).chain(),
                (
                    list::sync_virtual_list_state,
                    runtime::sync_loader_states,
                    input::sync_input_state,
                    extension::construct,
                )
                    .chain(),
            )
                .chain()
                .in_set(SyncPhase::Prepare),
        )
        .add_systems(
            Update,
            (
                input::apply_scroll_drag_commands,
                properties::sync,
                (
                    runtime::sync_movie_sources,
                    movie::sync_movie_controls,
                    movie::discard_unused_animation_commands,
                )
                    .chain(),
                (
                    runtime::text_measure::sync_sizes,
                    runtime::sync_virtual_lists,
                    runtime::tooltip::sync_tooltips,
                    runtime::spawn_pending_objects,
                    ecs::sync_gui_objects,
                    properties::sync,
                    extension::construct,
                    properties::sync,
                )
                    .chain(),
                runtime::sync_text_inputs,
                runtime::text_measure::sync_sizes,
                (
                    runtime::content::sync_content_layout,
                    runtime::tooltip::reconcile_tooltips,
                )
                    .chain(),
                (
                    runtime::sync_component_scroll_content,
                    runtime::sync_dirty,
                    runtime::sync_scroll_extents,
                )
                    .chain(),
                (
                    runtime::sync_text_fonts,
                    runtime::sync_input_policy,
                    runtime::sync_input_prompts,
                    runtime::sync_password_views,
                )
                    .chain(),
                runtime::sync_bitmap_rich_text,
                graph::sync_graphs,
                (runtime::sync_rich_text, runtime::text_measure::sync_padding).chain(),
                runtime::sync_appearance,
                image_fill::sync_image_fills,
                (
                    runtime::sync_stage_hierarchy,
                    input::editing::apply_input_commands,
                    input::editing::reconcile_focus,
                )
                    .chain(),
                runtime::sync_inline_image_hierarchy,
                movie::sync_movie_visibility,
                runtime::sync_text_sizes,
                backend::align_text,
                runtime::play_queued_sounds,
            )
                .chain()
                .in_set(SyncPhase::Project),
        );
        app.add_systems(
            Update,
            (
                runtime::root_mount::prepare_mounts,
                runtime::sync_ecs_roots,
                runtime::windows::sync_ecs_windows,
                runtime::root_mount::sync_mounts,
                input::apply_scroll_drag_commands,
                properties::sync,
                runtime::windows::apply_window_commands,
                popup::apply_popup_commands,
                runtime::ecs_drag_drop::apply_drag_commands,
                runtime::global_wait::sync_global_wait,
                runtime::tooltip::sync_tooltips,
            )
                .chain()
                .in_set(SyncPhase::Materialize)
                .before(runtime::sync_loader_components),
        );
        app.add_systems(
            Update,
            (
                runtime::sync_list_selection_components,
                properties::sync,
                ecs::sync_gui_objects,
                properties::sync,
                ecs::sync_gui_scroll_state,
                ecs::sync_gui_transitions,
                runtime::emit_transition_hooks,
                runtime::emit_transition_callbacks,
                (
                    runtime::emit_transition_completions,
                    tween::emit_completions,
                    progress::emit_completions,
                    animation::sync_state,
                    runtime::emit_animation_completions,
                )
                    .chain(),
                popup::sync_popup_state,
                runtime::windows::sync_window_state,
                (
                    runtime::global_wait::sync_global_wait_state,
                    runtime::tooltip::sync_tooltip_state,
                )
                    .chain(),
                runtime::ecs_drag_drop::prepare_drag_targets,
                runtime::ecs_drag_drop::sync_drag_state,
                runtime::object_drag::prepare_object_drags,
                runtime::object_drag::sync_object_drag_state,
                (tree::sync_tree_state, list::sync_list_state).chain(),
                (
                    list::sync_virtual_list_state,
                    runtime::sync_loader_states,
                    input::sync_input_state,
                )
                    .chain(),
            )
                .chain()
                .in_set(SyncPhase::Project)
                .after(runtime::play_queued_sounds),
        );

        app.add_systems(
            PostUpdate,
            backend::align_text
                .after(bevy::ui::widget::text_system)
                .after(bevy::ui::widget::update_editable_text_layout)
                .after(runtime::publish_password_layout)
                .before(runtime::position_inline_images)
                .before(bevy::ui::update::update_clipping_system),
        );
        app.add_systems(
            PostUpdate,
            runtime::project_scroll_overflow
                .after(bevy::ui::UiSystems::Layout)
                .before(bevy::ui::widget::text_system)
                .before(bevy::ui::widget::sync_editable_text_viewports)
                .before(bevy::ui::update::update_clipping_system),
        );
        app.add_systems(
            PostUpdate,
            (
                runtime::position_inline_images,
                runtime::finalize_inline_images,
            )
                .chain()
                .after(bevy::ui::widget::text_system)
                .before(bevy::ui::update::update_clipping_system),
        );
        app.add_systems(
            PostUpdate,
            runtime::apply_input_edits
                .after(bevy::ui::widget::update_editable_text_styles)
                .after(bevy::ui::widget::sync_editable_text_viewports)
                .before(bevy::text::EditableTextSystems),
        );
        app.add_systems(
            PostUpdate,
            runtime::update_prompt_visibility
                .after(runtime::apply_input_edits)
                .before(bevy::text::EditableTextSystems)
                .before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate)
                .before(bevy::ui::update::update_clipping_system),
        );
        app.add_systems(
            PostUpdate,
            runtime::sync_input_cursors
                .after(runtime::apply_input_edits)
                .before(bevy::text::EditableTextSystems),
        );
        app.add_systems(
            PostUpdate,
            runtime::prepare_password_layout
                .after(runtime::apply_input_edits)
                .before(bevy::text::EditableTextSystems),
        );
        app.add_systems(
            PostUpdate,
            runtime::publish_password_layout.after(bevy::ui::widget::update_editable_text_layout),
        );

        // 默认不指定字体：引擎自带的那套只有拉丁字形，中日韩会渲染成空白。
        // 使用方按需插入 `GuiFont` 换成含中文的字体。
        app.init_resource::<runtime::GuiFont>();

        input::register(app);
        compositor::install(app);
    }
}

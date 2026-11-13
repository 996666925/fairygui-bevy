//! FairyGUI 运行时的 Bevy 集成层。
//!
//! 面向应用的界面生命周期、事件响应与控件修改都通过 Bevy ECS 完成：用
//! [`FairyGuiRoot`] 拥有界面，用 [`GuiEvent`] observer 响应交互，再通过
//! [`GuiObjectCommand`] 修改控件。
//!
//! [`fairygui_core`] 在内部执行 FairyGUI 包语义，运行时把结果同步到对应的 ECS
//! 实体与 Bevy UI 组件。应用代码通常不需要直接访问 [`GuiRuntime`] 或
//! `ObjectStore`；它们保留给尚未封装为 ECS 操作的高级能力。
//!
//! # 分层约定
//!
//! 核心层不依赖 Bevy。渲染表示由 [`backend`]、[`text`] 与 [`movie`] 负责，
//! [`runtime`] 管理模型与实体之间的同步，[`input`] 管理指针路由与捕获。
//! 应用 observer 与消息处理系统放入 [`GuiSystems::Respond`]，发送的命令会在本帧
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
//! use fairygui_bevy::{FairyGuiPlugin, GuiRuntime};
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

pub mod asset;
pub mod backend;
mod compositor;
mod graph;
mod image_fill;
pub mod input;
pub mod movie;
pub mod runtime;
pub mod text;
mod text_stroke;

pub use asset::{FuiAsset, FuiAssetLoader, FuiLoadError};
pub use input::{GuiInputSettings, GuiInputState};
pub use runtime::{
    FairyGuiRoot, GObjectRef, GuiAnimationComplete, GuiButtonSelected, GuiConfig, GuiEvent,
    GuiFont, GuiLink, GuiListItemClick, GuiListSelection, GuiLoaderFailure, GuiLoaderState,
    GuiMenuAction, GuiObjectAction, GuiObjectCommand, GuiProgressValue, GuiPullRelease,
    GuiRootInstance, GuiRuntime, GuiSliderValue, GuiTextInput, GuiTransitionCallback,
    GuiTransitionHook, PopupMenuTemplates,
};

pub use fairygui_core as core;

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
}

/// Common application imports; the existing crate-root API remains supported.
pub mod prelude {
    pub use crate::{
        FairyGuiPlugin, FairyGuiRoot, FuiAsset, GObjectRef, GuiAnimationComplete,
        GuiButtonSelected, GuiConfig, GuiEvent, GuiFont, GuiInputSettings, GuiInputState, GuiLink,
        GuiListItemClick, GuiListSelection, GuiLoaderFailure, GuiLoaderState, GuiMenuAction,
        GuiObjectAction, GuiObjectCommand, GuiProgressValue, GuiPullRelease, GuiRootInstance,
        GuiRuntime, GuiSliderValue, GuiSystems, GuiTextInput, GuiTransitionCallback,
        GuiTransitionHook, PopupMenuTemplates,
    };
}

/// FairyGUI 集成插件。
///
/// 注册 `.fui` 资产类型与加载器，把界面运行期接入主线程，
/// 并把指针事件桥接成界面事件。
#[derive(Debug, Default)]
pub struct FairyGuiPlugin;

impl Plugin for FairyGuiPlugin {
    fn build(&self, app: &mut App) {
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
        .add_message::<GuiLink>()
        .add_message::<GuiMenuAction>()
        .add_message::<GuiTransitionHook>()
        .add_message::<GuiTransitionCallback>()
        .add_message::<GuiAnimationComplete>()
        .add_message::<GuiTextInput>()
        .add_message::<GuiListItemClick>()
        .add_message::<GuiPullRelease>()
        .add_message::<GuiObjectCommand>()
        .init_asset::<FuiAsset>()
        .init_asset_loader::<FuiAssetLoader>()
        .init_resource::<GuiConfig>()
        .init_non_send::<GuiRuntime>()
        .add_systems(
            Update,
            (
                runtime::sync_ui_config,
                runtime::advance_async_builds,
                runtime::advance_scrolls,
                runtime::advance_transitions,
                runtime::emit_transition_hooks,
                runtime::emit_transition_callbacks,
                runtime::advance_gear_tweens,
                runtime::advance_tweens,
                runtime::sync_movie_sources,
                movie::advance_movie_clips,
                runtime::advance_inline_movie_clips,
                movie::discard_unused_animation_commands,
                runtime::emit_animation_completions,
            )
                .chain()
                .in_set(GuiSystems::Advance),
        )
        .add_systems(
            Update,
            (
                runtime::emit_gui_events,
                runtime::emit_link_messages,
                runtime::emit_menu_actions,
                runtime::emit_text_input_messages,
                runtime::emit_list_item_messages,
                runtime::emit_pull_release_messages,
            )
                .in_set(GuiSystems::Publish),
        )
        .add_systems(
            Update,
            (
                runtime::sync_loader_components,
                runtime::sync_loader_status,
                (
                    runtime::cleanup_retained_popups,
                    runtime::sync_virtual_lists,
                )
                    .chain(),
                runtime::spawn_pending_roots,
                runtime::spawn_pending_objects,
                runtime::sync_text_inputs,
                runtime::content::sync_content_layout,
                (runtime::sync_dirty, runtime::sync_scroll_extents).chain(),
                (
                    runtime::sync_text_fonts,
                    runtime::sync_input_policy,
                    runtime::sync_input_prompts,
                    runtime::sync_password_views,
                )
                    .chain(),
                runtime::sync_bitmap_rich_text,
                graph::sync_graphs,
                runtime::sync_rich_text,
                runtime::sync_appearance,
                image_fill::sync_image_fills,
                (
                    runtime::sync_stage_hierarchy,
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
                .in_set(GuiSystems::Sync),
        );
        app.add_systems(
            Update,
            (runtime::sync_ecs_roots, runtime::apply_object_commands)
                .chain()
                .in_set(GuiSystems::Sync)
                .before(runtime::sync_loader_components),
        );
        app.add_systems(
            Update,
            (
                runtime::sync_list_selection_components,
                runtime::sync_control_state_components,
            )
                .chain()
                .in_set(GuiSystems::Sync)
                .after(runtime::play_queued_sounds),
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

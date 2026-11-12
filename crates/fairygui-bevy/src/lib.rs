//! FairyGUI 运行时的 Bevy 集成层。
//!
//! 本 crate 把 [`fairygui_core`] 的对象树镜像为 Bevy 的 ECS 实体树，并提供资产加载。
//! 核心对象树是**唯一权威状态**，ECS 实体只是它的视图。
//!
//! # 分层约定
//!
//! 核心层不依赖 Bevy。渲染表示由 [`backend`]、[`text`] 与 [`movie`] 负责，
//! [`runtime`] 管理视图同步，[`input`] 管理指针路由与捕获。
//! 应用事件处理系统放入 [`GuiSystems::Respond`]，在本帧视图同步之前修改对象树。
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
//! The plugin waits for the asset, creates the root once and removes its view
//! when the component is removed. `GuiRootInstance` exposes the created model
//! object and manual root handle when application setup needs them.

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
    FairyGuiRoot, GObjectRef, GuiAnimationComplete, GuiEvent, GuiFont, GuiLink, GuiListItemClick,
    GuiLoaderFailure, GuiLoaderState, GuiMenuAction, GuiPullRelease, GuiRootInstance, GuiRuntime,
    GuiTextInput, GuiTransitionCallback, GuiTransitionHook, PopupMenuTemplates,
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
        FairyGuiPlugin, FairyGuiRoot, FuiAsset, GObjectRef, GuiAnimationComplete, GuiEvent,
        GuiFont, GuiInputSettings, GuiInputState, GuiLink, GuiListItemClick, GuiLoaderFailure,
        GuiLoaderState, GuiMenuAction, GuiPullRelease, GuiRootInstance, GuiRuntime, GuiSystems,
        GuiTextInput, GuiTransitionCallback, GuiTransitionHook, PopupMenuTemplates,
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
        .init_asset::<FuiAsset>()
        .init_asset_loader::<FuiAssetLoader>()
        .init_non_send::<GuiRuntime>()
        .add_systems(
            Update,
            (
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
                (runtime::sync_ecs_roots, runtime::spawn_pending_roots).chain(),
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

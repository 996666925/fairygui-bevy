//! `fairygui-bevy` 的可运行示例。
//!
//! 与官方 Laya 示例一样从主菜单出发：点击菜单里的条目切换到对应的演示界面，
//! 界面右下角的关闭按钮退回菜单。
//!
//! 资源需要自行准备：把 FairyGUI 编辑器导出的 `.fui` 与其图集放进
//! `crates/fairygui-demo/assets/ui/`，详见该目录的 `README.md`。

use bevy::prelude::*;

use fairygui_bevy::prelude::*;
use fairygui_core::object::GObjectId;

mod bag_demo;
mod catalog;
mod demos;
mod drag_drop_demo;
mod list_effect_demo;
mod loop_list_demo;
mod modal_wait_demo;
mod pull_refresh_demo;
mod scroll_pane_demo;
mod smoke;
mod tree_demo;
mod window_demo;

/// 主菜单所在的包与界面名。
const MENU_PACKAGE: &str = "ui/MainMenu.fui";
const MENU_ITEM: &str = "Main";
/// 关闭按钮的条目名，与官方示例一致。
const CLOSE_ITEM: &str = "CloseButton";

/// Position manually created roots in the window for focused smoke fixtures.
fn center_root(runtime: &mut GuiRuntime, index: usize, viewport: (f32, f32)) {
    let Some(root) = runtime.root_object(index) else {
        return;
    };
    let Some((width, height)) = runtime
        .store()
        .object(root)
        .map(|object| (object.width(), object.height()))
    else {
        return;
    };
    runtime.store_mut().set_xy(
        root,
        (viewport.0 - width) / 2.0,
        (viewport.1 - height) / 2.0,
    );
}

fn center_root_command(
    gui: &GuiQuery,
    commands: &mut MessageWriter<GuiObjectCommand>,
    owner: Entity,
    viewport: (f32, f32),
) {
    let Some(transform) = gui.transform(owner) else {
        return;
    };
    let Vec2 {
        x: width,
        y: height,
    } = transform.size();
    let position = Vec2::new((viewport.0 - width) / 2.0, (viewport.1 - height) / 2.0);
    commands.write(GuiObjectCommand::new(
        owner,
        GuiObjectAction::SetPosition(position),
    ));

    info!(
        "界面 {width}×{height}，视口 {}×{}，摆到 ({}, {})",
        viewport.0, viewport.1, position.x, position.y
    );
}

/// 示例的运行状态。
#[derive(Resource, Default)]
struct Demo {
    /// 主菜单的资产句柄。
    menu_asset: Option<Handle<FuiAsset>>,
    /// ECS 实体拥有的主菜单根。
    menu_owner: Option<Entity>,
    /// ECS 实体拥有的关闭按钮根。
    close_owner: Option<Entity>,
    /// 主菜单的根编号。
    menu: Option<usize>,
    /// 右下角的关闭按钮。
    close_button: Option<GObjectId>,
    /// 请求加载中的演示：包名与该包的资产。
    loading: Option<(String, Handle<FuiAsset>)>,
    /// 已打开的演示：根拥有实体与该包的资产。
    open: Option<(Entity, Handle<FuiAsset>, String)>,
}

#[derive(Component)]
pub(crate) struct DemoAction(pub u64);

#[derive(Component)]
struct MenuEntry(&'static str);

#[derive(Component)]
struct DemoObserverBound;

fn main() -> AppExit {
    let mut app = App::new();
    // Some Windows Vulkan drivers fail to present this UI. Keep WGPU_BACKEND
    // overrides available while using the native backend by default on Windows.
    let mut render_settings = bevy::render::settings::WgpuSettings::default();
    if cfg!(target_os = "windows") && std::env::var_os("WGPU_BACKEND").is_none() {
        render_settings.backends = Some(bevy::render::settings::Backends::DX12);
    }

    app.add_plugins((
        DefaultPlugins
            .set(bevy::render::RenderPlugin {
                render_creation: render_settings.into(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "fairygui-bevy demo".to_string(),
                    resolution: (1280u32, 720u32).into(),
                    ..default()
                }),
                ..default()
            }),
        FairyGuiPlugin,
    ))
    .init_resource::<Demo>()
    .init_resource::<demos::DemoController>()
    .init_resource::<pull_refresh_demo::PullDemo>()
    .init_resource::<modal_wait_demo::GlobalDemo>()
    .add_systems(
        Update,
        pull_refresh_demo::update
            .in_set(GuiSystems::Respond)
            .after(advance_demo_behaviors),
    )
    .add_systems(Startup, setup_camera)
    .add_systems(Update, modal_wait_demo::advance.in_set(GuiSystems::Respond))
    .add_systems(
        Update,
        loop_list_demo::update.in_set(GuiSystems::PostLayout),
    )
    .add_systems(
        Update,
        list_effect_demo::play.in_set(GuiSystems::PostLayout),
    )
    .add_systems(
        Update,
        (
            boot_menu,
            open_pending_demo,
            demos::handle_links,
            demos::handle_menu_actions,
            advance_demo_behaviors,
            demos::advance_transition_demo,
            keep_close_button_on_top,
        )
            .chain()
            .in_set(GuiSystems::Respond),
    );
    app.add_systems(
        Update,
        (
            demos::initialize_demo_roots,
            bind_demo_observers,
            window_demo::bind,
            modal_wait_demo::bind,
            drag_drop_demo::bind,
            tree_demo::bind,
            bag_demo::bind,
            scroll_pane_demo::bind,
            list_effect_demo::bind,
        )
            .chain()
            .after(GuiSystems::Sync),
    );

    // Use the bundled OFL font so CJK demo text renders consistently across hosts.
    let font = app
        .world()
        .resource::<AssetServer>()
        .load::<Font>("fonts/NotoSansSC-VF.ttf");
    app.insert_resource(GuiFont(Some(font)));

    if std::env::args().any(|arg| arg == "--smoke-list-effect") {
        smoke::install_list_effect(&mut app);
    } else if std::env::args().any(|arg| arg == "--smoke-nested-scroll") {
        smoke::install_nested_scroll(&mut app);
    } else if std::env::args().any(|arg| arg == "--smoke-bag") {
        smoke::install_bag(&mut app);
    } else if std::env::args().any(|arg| arg == "--smoke-tree") {
        smoke::install_tree(&mut app);
    } else if std::env::args().any(|arg| arg == "--smoke-drag-drop") {
        smoke::install_drag_drop(&mut app);
    } else if std::env::args().any(|arg| arg == "--smoke-window-wait") {
        smoke::install_window_wait(&mut app);
    } else if std::env::args().any(|arg| arg == "--smoke-windows") {
        smoke::install_windows(&mut app);
    } else if std::env::args().any(|arg| arg == "--smoke-masks") {
        smoke::install_masks(&mut app);
    } else if std::env::args().any(|arg| arg == "--smoke-compositor") {
        smoke::install_compositor(&mut app);
    } else if std::env::args().any(|arg| arg == "--smoke-loop-list") {
        smoke::install_loop_list(&mut app);
    } else if std::env::args().any(|arg| arg == "--smoke-virtual-list") {
        smoke::install_virtual_list(&mut app);
    } else if std::env::args().any(|arg| arg == "--smoke-pull-refresh") {
        smoke::install_pull_refresh(&mut app);
    } else if std::env::args().any(|arg| arg == "--smoke-scroll-pages") {
        smoke::install_scroll_pages(&mut app);
    } else if std::env::args().any(|arg| arg == "--smoke-list-bars") {
        smoke::install_list_bars(&mut app);
    } else if std::env::args().any(|arg| arg == "--smoke-scroll-view") {
        smoke::install_scroll_view(&mut app);
    } else if std::env::args().any(|arg| arg == "--smoke-transition") {
        smoke::install_transition(&mut app);
    } else if std::env::args().any(|arg| arg == "--smoke-test") {
        smoke::install(&mut app);
    }

    app.run()
}

fn keep_close_button_on_top(
    mut commands: Commands,
    gui: GuiQuery,
    demo: Res<Demo>,
    layers: Query<&GlobalZIndex>,
) {
    if let Some(entity) = demo.close_owner.and_then(|owner| gui.view(owner))
        && layers.get(entity).is_err()
    {
        commands.entity(entity).insert(GlobalZIndex(1000));
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

/// 加载主菜单。资产是异步的，因此每帧检查一次。
fn boot_menu(
    mut commands: Commands,
    mut gui_commands: MessageWriter<GuiObjectCommand>,
    asset_server: Res<AssetServer>,
    windows: Query<&Window>,
    gui: GuiQuery,
    roots: Query<&GuiRootInstance>,
    mut demo: ResMut<Demo>,
) {
    if demo.menu.is_some() {
        return;
    }

    if demo.menu_owner.is_none() {
        let handle = asset_server.load::<FuiAsset>(MENU_PACKAGE);
        demo.menu_owner = Some(
            commands
                .spawn(FairyGuiRoot::new(handle.clone(), MENU_ITEM))
                .id(),
        );
        let close_owner = commands
            .spawn(FairyGuiRoot::new(handle.clone(), CLOSE_ITEM))
            .id();
        demo.close_owner = Some(close_owner);
        gui_commands.write(GuiObjectCommand::new(
            close_owner,
            GuiObjectAction::SetVisible(false),
        ));
        demo.menu_asset = Some(handle);
        return;
    }

    let Some(owner) = demo.menu_owner else {
        return;
    };
    let Ok(menu) = roots.get(owner) else {
        return;
    };
    let Some(close_owner) = demo.close_owner else {
        return;
    };
    let Ok(close) = roots.get(close_owner) else {
        return;
    };
    let Some(transform) = gui.transform(close_owner) else {
        return;
    };
    // 关闭按钮与主界面平级挂在根节点下，对应官方示例里挂在 GRoot 上的做法。
    // 它自带尺寸，因此单独由 ECS 实体拥有，并摆到右下角。
    let (viewport_width, viewport_height) = windows
        .iter()
        .next()
        .map(|window| (window.width(), window.height()))
        .unwrap_or((1280.0, 720.0));
    gui_commands.write(GuiObjectCommand::new(
        close_owner,
        GuiObjectAction::SetPosition(Vec2::new(
            viewport_width - transform.size().x - 10.0,
            viewport_height - transform.size().y - 10.0,
        )),
    ));

    // 界面摆到窗口中央。按实际窗口尺寸算，不用假设的分辨率。
    if let Some(window) = windows.iter().next() {
        center_root_command(
            &gui,
            &mut gui_commands,
            owner,
            (window.width(), window.height()),
        );
    }

    demo.close_button = Some(close.object);
    info!("主菜单已就绪，点击条目进入演示");
    demo.menu = Some(menu.handle);
}

/// 演示包的资产到达后创建界面。
#[allow(clippy::too_many_arguments)]
fn open_pending_demo(
    mut commands: Commands,
    mut gui_commands: MessageWriter<GuiObjectCommand>,
    assets: Res<Assets<FuiAsset>>,
    windows: Query<&Window>,
    gui: GuiQuery,
    roots: Query<&GuiRootInstance>,
    mut demo: ResMut<Demo>,
    mut controller: ResMut<demos::DemoController>,
) {
    if let Some((owner, handle, name)) = demo.open.clone() {
        if controller.root.is_some() {
            return;
        }
        let Ok(instance) = roots.get(owner) else {
            return;
        };
        let root = instance.object;
        if assets.get(&handle).is_none() {
            return;
        }
        if let Some(window) = windows.iter().next() {
            center_root_command(
                &gui,
                &mut gui_commands,
                owner,
                (window.width(), window.height()),
            );
        }
        if name == "VirtualList"
            && let Some(list) = gui.child_by_name(owner, "mailList")
        {
            commands.entity(list).insert(demos::mail_list_config());
        } else if name == "LoopList" {
            loop_list_demo::initialize(
                &mut commands,
                &gui,
                owner,
                &assets.get(&handle).unwrap().package,
            );
        }
        if name == "Basics"
            && let Some(back) = gui.find_by_name(owner, "btn_Back")
        {
            gui_commands.write(GuiObjectCommand::new(
                back,
                GuiObjectAction::SetVisible(false),
            ));
        }
        controller.package = Some(name.clone());
        controller.root = Some(root);
        controller.root_owner = Some(owner);
        controller.elapsed = 0.0;
        info!("已打开演示：{name}");
        return;
    }

    let Some((name, handle)) = demo.loading.clone() else {
        return;
    };
    let Some(asset) = assets.get(&handle) else {
        return;
    };
    if asset.package.item_index_by_name("Main").is_none() {
        error!("{name} 包里没有 Main 界面");
        demo.loading = None;
        return;
    }
    let owner = commands
        .spawn(FairyGuiRoot::new(handle.clone(), "Main"))
        .id();

    // 菜单让位：演示界面盖在上面。
    if let Some(target) = demo.menu_owner {
        gui_commands.write(GuiObjectCommand::new(
            target,
            GuiObjectAction::SetVisible(false),
        ));
    }
    if let Some(target) = demo.close_owner {
        gui_commands.write(GuiObjectCommand::new(
            target,
            GuiObjectAction::SetVisible(true),
        ));
    }

    demo.open = Some((owner, handle, name));
    demo.loading = None;
}

fn bind_demo_observers(
    mut commands: Commands,
    gui: GuiQuery,
    demo: Res<Demo>,
    controller: Res<demos::DemoController>,
    bound: Query<(), With<DemoObserverBound>>,
) {
    if let Some(owner) = demo.close_owner
        && let Some(entity) = gui.view(owner)
        && bound.get(entity).is_err()
    {
        commands
            .entity(entity)
            .insert(DemoObserverBound)
            .observe(close_demo);
    }
    if let Some(root) = demo.menu_owner {
        for (name, package) in catalog::ENTRIES {
            if let Some(entity) = gui.find_by_name(root, name)
                && bound.get(entity).is_err()
            {
                commands
                    .entity(entity)
                    .insert((DemoObserverBound, MenuEntry(package)))
                    .observe(open_menu_entry);
            }
        }
    }
    if let (Some(package), Some(root)) = (controller.package.as_deref(), controller.root_owner) {
        for (entity, action) in demos::demo_action_bindings(&gui, package, root) {
            if bound.get(entity).is_err() {
                let mut target = commands.entity(entity);
                target.insert((DemoObserverBound, DemoAction(action)));
                match package {
                    "Basics" => {
                        target.observe(navigate_basics_action);
                    }
                    "Transition" => {
                        target.observe(play_transition_action);
                    }
                    "VirtualList" => {
                        target.observe(virtual_list_action);
                    }
                    _ => {}
                }
            }
        }
    }
    if let Some(page) = controller.basics_page_owner {
        if let Some(entity) = gui.find_by_name(page, "n25")
            && bound.get(entity).is_err()
        {
            commands
                .entity(entity)
                .insert(DemoObserverBound)
                .observe(basics_text_get_input);
        }
        if let Some(entity) = gui.find_by_name(page, "n0")
            && bound.get(entity).is_err()
        {
            commands
                .entity(entity)
                .insert(DemoObserverBound)
                .observe(demos::show_basics_menu);
        }
    }
}

fn close_demo(
    event: On<GuiEvent>,
    mut commands: Commands,
    mut gui_commands: MessageWriter<GuiObjectCommand>,
    mut demo: ResMut<Demo>,
    mut controller: ResMut<demos::DemoController>,
) {
    if event.event().event.kind != fairygui_core::object::EventKind::Click {
        return;
    }
    commands.write_message(GuiPopupCommand::HideAll);
    demos::dispose_dynamic_roots(&mut commands, &mut controller);
    // 销毁演示界面，收起关闭按钮，把菜单放回来。
    if let Some((owner, _, _)) = demo.open.take() {
        commands.entity(owner).despawn();
    }
    controller.package = None;
    controller.root = None;
    controller.root_owner = None;
    controller.elapsed = 0.0;
    if let Some(target) = demo.close_owner {
        gui_commands.write(GuiObjectCommand::new(
            target,
            GuiObjectAction::SetVisible(false),
        ));
    }
    if let Some(target) = demo.menu_owner {
        gui_commands.write(GuiObjectCommand::new(
            target,
            GuiObjectAction::SetVisible(true),
        ));
    }
    info!("已退回主菜单");
}

fn open_menu_entry(
    event: On<GuiEvent>,
    entries: Query<&MenuEntry>,
    asset_server: Res<AssetServer>,
    mut demo: ResMut<Demo>,
) {
    if event.event().event.kind != fairygui_core::object::EventKind::Click {
        return;
    }
    let Ok(entry) = entries.get(event.entity) else {
        return;
    };
    let package = entry.0;
    if demo.open.is_some() || demo.loading.is_some() {
        info!("请先关闭当前演示");
        return;
    }
    if !catalog::AVAILABLE.contains(&package) {
        info!("{package} 的资源未随仓库提供，跳过");
        return;
    }
    let path = format!("ui/{package}.fui");
    let handle = asset_server.load::<FuiAsset>(path.clone());
    info!("正在加载 {path}");
    demo.loading = Some((package.to_string(), handle));
}

fn navigate_basics_action(
    event: On<GuiEvent>,
    actions: Query<&DemoAction>,
    mut commands: Commands,
    gui: GuiQuery,
    mut gui_commands: MessageWriter<GuiObjectCommand>,
    demo: Res<Demo>,
    mut controller: ResMut<demos::DemoController>,
) {
    if event.event().event.kind != fairygui_core::object::EventKind::Click {
        return;
    }
    let Ok(action) = actions.get(event.entity) else {
        return;
    };
    let Some((_, handle, _)) = &demo.open else {
        return;
    };
    demos::navigate_basics(
        &mut commands,
        &gui,
        &mut controller,
        handle.clone(),
        action.0,
        &mut gui_commands,
    );
}

fn play_transition_action(
    event: On<GuiEvent>,
    actions: Query<&DemoAction>,
    mut commands: Commands,
    demo: Res<Demo>,
    mut controller: ResMut<demos::DemoController>,
) {
    let Ok(action) = actions.get(event.entity) else {
        return;
    };
    if event.event().event.kind != fairygui_core::object::EventKind::Click {
        return;
    }
    let Some((_, handle, _)) = &demo.open else {
        return;
    };
    demos::navigate_transition(&mut commands, &mut controller, handle.clone(), action.0);
}

fn virtual_list_action(
    event: On<GuiEvent>,
    actions: Query<&DemoAction>,
    mut gui_commands: MessageWriter<GuiObjectCommand>,
    gui: GuiQuery,
    controller: Res<demos::DemoController>,
) {
    if event.event().event.kind != fairygui_core::object::EventKind::Click {
        return;
    }
    let Ok(action) = actions.get(event.entity) else {
        return;
    };
    demos::handle_virtual_list_action(&gui, &controller, action.0, &mut gui_commands);
}

fn basics_text_get_input(
    event: On<GuiEvent>,
    gui: GuiQuery,
    controller: Res<demos::DemoController>,
    mut gui_commands: MessageWriter<GuiObjectCommand>,
) {
    if event.event().event.kind != fairygui_core::object::EventKind::Click {
        return;
    }
    demos::copy_basics_input(&gui, &controller, &mut gui_commands);
}

fn advance_demo_behaviors(
    time: Res<Time>,
    gui: GuiQuery,
    mut controller: ResMut<demos::DemoController>,
    mut gui_commands: MessageWriter<GuiObjectCommand>,
) {
    demos::tick(&gui, &mut controller, time.delta_secs(), &mut gui_commands);
}

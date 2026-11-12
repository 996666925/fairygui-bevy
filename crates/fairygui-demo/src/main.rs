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

mod catalog;
mod demos;
mod loop_list_demo;
mod pull_refresh_demo;
mod smoke;

/// 主菜单所在的包与界面名。
const MENU_PACKAGE: &str = "ui/MainMenu.fui";
const MENU_ITEM: &str = "Main";
/// 关闭按钮的条目名，与官方示例一致。
const CLOSE_ITEM: &str = "CloseButton";

/// 把界面摆到窗口中央。
///
/// 界面的原点在左上角，不理会的话它会贴着左上角铺开；界面尺寸与窗口不一致时
/// 看起来就是整体偏在一边。窗口尺寸要**实测**——硬编码会在窗口被系统缩放、
/// 或分辨率与假设不符时算错。
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

    let position = ((viewport.0 - width) / 2.0, (viewport.1 - height) / 2.0);
    runtime.store_mut().set_xy(root, position.0, position.1);

    info!(
        "界面 {width}×{height}，视口 {}×{}，摆到 ({}, {})",
        viewport.0, viewport.1, position.0, position.1
    );
}

/// 示例的运行状态。
#[derive(Resource, Default)]
struct Demo {
    /// 主菜单的资产，请求出去后一直留着。
    menu_asset: Option<Handle<FuiAsset>>,
    /// 主菜单的根编号。
    menu: Option<usize>,
    /// 右下角的关闭按钮。
    close_button: Option<GObjectId>,
    /// 请求加载中的演示：包名与该包的资产。
    loading: Option<(String, Handle<FuiAsset>)>,
    /// 已打开的演示：根编号与该包的资产。
    open: Option<(usize, Handle<FuiAsset>, String)>,
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
    .add_systems(
        Update,
        pull_refresh_demo::update
            .in_set(GuiSystems::Respond)
            .after(advance_demo_behaviors),
    )
    .add_systems(Startup, setup_camera)
    .add_systems(
        Update,
        loop_list_demo::update
            .after(GuiSystems::Advance)
            .before(GuiSystems::Sync),
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
    app.add_systems(Update, bind_demo_observers.after(GuiSystems::Sync));

    // Use the bundled OFL font so CJK demo text renders consistently across hosts.
    let font = app
        .world()
        .resource::<AssetServer>()
        .load::<Font>("fonts/NotoSansSC-VF.ttf");
    app.insert_resource(GuiFont(Some(font)));

    if std::env::args().any(|arg| arg == "--smoke-masks") {
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
    runtime: NonSend<GuiRuntime>,
    demo: Res<Demo>,
    layers: Query<&GlobalZIndex>,
) {
    if let Some(entity) = demo.close_button.and_then(|id| runtime.entity_of(id))
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
    asset_server: Res<AssetServer>,
    assets: Res<Assets<FuiAsset>>,
    font: Res<GuiFont>,
    windows: Query<&Window>,
    mut runtime: NonSendMut<GuiRuntime>,
    mut demo: ResMut<Demo>,
) {
    if demo.menu.is_some() {
        return;
    }

    // 先设一次字体：本系统与同步字体的系统之间没有先后约束，同帧谁先跑不定。
    // 建界面只发生在第一帧，错过这一帧就再也没机会补上了。
    runtime.set_font(font.0.clone());

    // 先把菜单的资产请求出去；到达之前不做别的。
    let handle = demo
        .menu_asset
        .get_or_insert_with(|| asset_server.load::<FuiAsset>(MENU_PACKAGE))
        .clone();

    let Some(asset) = assets.get(&handle) else {
        return;
    };

    let Some(index) = runtime.create_root(handle.clone(), &asset.package, MENU_ITEM) else {
        error!("主菜单界面 {MENU_ITEM} 不存在，请检查 {MENU_PACKAGE}");
        return;
    };

    // 关闭按钮与主界面平级挂在根节点下，对应官方示例里挂在 GRoot 上的做法。
    // 它自带尺寸，因此单独建成一个根，摆到右下角。
    if let Some(close_index) = runtime.create_root(handle.clone(), &asset.package, CLOSE_ITEM)
        && let Some(close) = runtime.root_object(close_index)
    {
        let (width, height) = runtime
            .store()
            .object(close)
            .map(|object| (object.width(), object.height()))
            .unwrap_or_default();

        let (viewport_width, viewport_height) = windows
            .iter()
            .next()
            .map(|window| (window.width(), window.height()))
            .unwrap_or((1280.0, 720.0));
        let store = runtime.store_mut();
        store.set_xy(
            close,
            viewport_width - width - 10.0,
            viewport_height - height - 10.0,
        );
        store.set_visible(close, false);
        demo.close_button = Some(close);
    }

    // 界面摆到窗口中央。按实际窗口尺寸算，不用假设的分辨率。
    if let Some(window) = windows.iter().next() {
        center_root(&mut runtime, index, (window.width(), window.height()));
    }

    info!("主菜单已就绪，点击条目进入演示");
    demo.menu = Some(index);
}

/// 演示包的资产到达后创建界面。
fn open_pending_demo(
    assets: Res<Assets<FuiAsset>>,
    font: Res<GuiFont>,
    windows: Query<&Window>,
    mut runtime: NonSendMut<GuiRuntime>,
    mut demo: ResMut<Demo>,
    mut controller: ResMut<demos::DemoController>,
) {
    let Some((name, handle)) = demo.loading.clone() else {
        return;
    };
    runtime.set_font(font.0.clone());
    let Some(asset) = assets.get(&handle) else {
        return;
    };

    let Some(index) = runtime.create_root(handle.clone(), &asset.package, "Main") else {
        error!("{name} 包里的 Main 界面创建失败");
        demo.loading = None;
        return;
    };

    // 菜单让位：演示界面盖在上面。
    if let Some(menu) = demo.menu
        && let Some(root) = runtime.root_object(menu)
    {
        runtime.store_mut().set_visible(root, false);
    }
    if let Some(close) = demo.close_button {
        runtime.store_mut().set_visible(close, true);
    }

    if let Some(window) = windows.iter().next() {
        center_root(&mut runtime, index, (window.width(), window.height()));
    }

    if let Some(root) = runtime.root_object(index) {
        demos::initialize_demo(&mut runtime, &name, root);
    }
    controller.package = Some(name.clone());
    controller.root = runtime.root_object(index);
    controller.elapsed = 0.0;
    info!("已打开演示：{name}");
    demo.open = Some((index, handle, name));
    demo.loading = None;
}

fn bind_demo_observers(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    demo: Res<Demo>,
    controller: Res<demos::DemoController>,
    bound: Query<(), With<DemoObserverBound>>,
) {
    if let Some(close) = demo.close_button
        && let Some(entity) = runtime.entity_of(close)
        && bound.get(entity).is_err()
    {
        commands
            .entity(entity)
            .insert(DemoObserverBound)
            .observe(close_demo);
    }
    if let Some(menu) = demo.menu
        && let Some(root) = runtime.root_object(menu)
    {
        for (name, package) in catalog::ENTRIES {
            if let Some(object) = runtime.store().find_by_name(root, name)
                && let Some(entity) = runtime.entity_of(object)
                && bound.get(entity).is_err()
            {
                commands
                    .entity(entity)
                    .insert((DemoObserverBound, MenuEntry(package)))
                    .observe(open_menu_entry);
            }
        }
    }
    if let (Some(package), Some(root)) = (controller.package.as_deref(), controller.root) {
        for (object, action) in demos::demo_action_bindings(&runtime, package, root) {
            if let Some(entity) = runtime.entity_of(object)
                && bound.get(entity).is_err()
            {
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
    if let Some(page) = controller
        .basics_page
        .and_then(|index| runtime.root_object(index))
    {
        if let Some(object) = runtime.store().find_by_name(page, "n25")
            && let Some(entity) = runtime.entity_of(object)
            && bound.get(entity).is_err()
        {
            commands
                .entity(entity)
                .insert(DemoObserverBound)
                .observe(basics_text_get_input);
        }
        if let Some(object) = runtime.store().find_by_name(page, "n0")
            && let Some(entity) = runtime.entity_of(object)
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
    mut runtime: NonSendMut<GuiRuntime>,
    mut demo: ResMut<Demo>,
    mut controller: ResMut<demos::DemoController>,
) {
    if event.event().event.kind != fairygui_core::object::EventKind::Click {
        return;
    }
    runtime.hide_popup(None);
    if let Some(index) = controller.transition_sample.take() {
        runtime.remove_root(&mut commands, index);
    }
    if let Some(index) = controller.basics_page.take() {
        runtime.remove_root(&mut commands, index);
    }
    // 销毁演示界面，收起关闭按钮，把菜单放回来。
    if let Some((index, _, _)) = demo.open.take() {
        runtime.remove_root(&mut commands, index);
    }
    controller.package = None;
    controller.root = None;
    controller.elapsed = 0.0;
    if let Some(close) = demo.close_button {
        runtime.store_mut().set_visible(close, false);
    }
    if let Some(menu) = demo.menu
        && let Some(root) = runtime.root_object(menu)
    {
        runtime.store_mut().set_visible(root, true);
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
    if demo.open.is_some() {
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
    assets: Res<Assets<FuiAsset>>,
    mut runtime: NonSendMut<GuiRuntime>,
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
    let Some(asset) = assets.get(handle) else {
        return;
    };
    demos::navigate_basics(
        &mut commands,
        &mut runtime,
        &mut controller,
        handle.clone(),
        &asset.package,
        action.0,
    );
}

fn play_transition_action(
    event: On<GuiEvent>,
    actions: Query<&DemoAction>,
    assets: Res<Assets<FuiAsset>>,
    mut runtime: NonSendMut<GuiRuntime>,
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
    let Some(asset) = assets.get(handle) else {
        return;
    };
    demos::navigate_transition(
        &mut runtime,
        &mut controller,
        handle.clone(),
        &asset.package,
        action.0,
    );
}

fn virtual_list_action(
    event: On<GuiEvent>,
    actions: Query<&DemoAction>,
    mut runtime: NonSendMut<GuiRuntime>,
    controller: Res<demos::DemoController>,
) {
    if event.event().event.kind != fairygui_core::object::EventKind::Click {
        return;
    }
    let Ok(action) = actions.get(event.entity) else {
        return;
    };
    demos::handle_virtual_list_action(&mut runtime, &controller, action.0);
}

fn basics_text_get_input(
    event: On<GuiEvent>,
    mut runtime: NonSendMut<GuiRuntime>,
    controller: Res<demos::DemoController>,
) {
    if event.event().event.kind != fairygui_core::object::EventKind::Click {
        return;
    }
    demos::copy_basics_input(&mut runtime, &controller);
}

fn advance_demo_behaviors(
    time: Res<Time>,
    mut runtime: NonSendMut<GuiRuntime>,
    mut controller: ResMut<demos::DemoController>,
) {
    demos::tick(&mut runtime, &mut controller, time.delta_secs());
}

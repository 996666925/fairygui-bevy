//! 界面运行期的测试。
//!
//! 这里只验证「由包创建对象树」这一段，不涉及窗口与渲染，
//! 因此可以在无显示环境下运行。夹具与核心层共用。

use bevy::prelude::*;

use fairygui_bevy::{FuiAsset, GuiRuntime};
use fairygui_core::package::FuiPackage;

const MAIN_MENU: &[u8] = include_bytes!("../../fairygui-core/tests/fixtures/MainMenu.fui");
const BAG: &[u8] = include_bytes!("../../fairygui-core/tests/fixtures/Bag.fui");

fn transition_fixture(item: &str) -> (App, fairygui_core::object::GObjectId, Entity) {
    use fairygui_bevy::{FairyGuiPlugin, FairyGuiRoot, GuiRootInstance};
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let package = FuiPackage::parse(
        "Transition",
        include_bytes!("../../fairygui-core/tests/fixtures/Transition.fui"),
    )
    .unwrap();
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package,
            res_key: "ui/Transition".into(),
        });
    let owner = app.world_mut().spawn(FairyGuiRoot::new(asset, item)).id();
    app.update();
    let root = app.world().get::<GuiRootInstance>(owner).unwrap().object;
    let view = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(root)
        .unwrap();
    (app, root, view)
}

#[test]
fn edited_transition_hook_listeners_publish_once_without_consuming_label_notifications() {
    use fairygui_bevy::{GuiTransitionCallback, GuiTransitionHook};
    let (mut app, root, entity) = transition_fixture("PowerUp");
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut()
            .set_transition_hook(root, "t0", "play_num_now", Some(42))
            .unwrap();
        gui.store_mut().play_transition(root, "t0", 1);
        gui.store_mut().advance_transitions(0.8);
    }
    app.update();
    let callbacks: Vec<_> = app
        .world_mut()
        .resource_mut::<Messages<GuiTransitionCallback>>()
        .drain()
        .collect();
    assert_eq!(callbacks.len(), 1);
    assert_eq!(callbacks[0].entity, entity);
    assert_eq!(callbacks[0].callback.listener, 42);
    assert!(
        callbacks[0].callback.hook.at_end,
        "first match is an end label before a later start label"
    );
    assert!(
        app.world_mut()
            .resource_mut::<Messages<GuiTransitionHook>>()
            .drain()
            .any(|hook| hook.entity == entity
                && hook.hook.label == "play_num_now"
                && !hook.hook.at_end)
    );
    app.update();
    assert_eq!(
        app.world_mut()
            .resource_mut::<Messages<GuiTransitionCallback>>()
            .drain()
            .count(),
        0
    );
}

#[test]
fn bundled_fairygui_wav_and_mp3_sounds_decode() {
    use bevy::audio::Decodable;
    for bytes in [
        include_bytes!("../../fairygui-demo/assets/ui/Basics_gojg7u.wav").as_slice(),
        include_bytes!("../../fairygui-demo/assets/ui/Transition_lvxrs.mp3").as_slice(),
    ] {
        let source = bevy::audio::AudioSource {
            bytes: bytes.into(),
        };
        assert!(source.decoder().next().is_some());
    }
}

#[test]
fn root_packages_register_automatically_and_sound_uses_its_own_asset_directory() {
    let menu = FuiPackage::parse("MainMenu", MAIN_MENU).unwrap();
    let basics = FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui"),
    )
    .unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<FuiAsset>()
        .init_asset::<bevy::audio::AudioSource>();
    let menu_handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: menu.clone(),
            res_key: "menu/MainMenu".into(),
        });
    let basics_handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: basics.clone(),
            res_key: "different/Basics".into(),
        });
    let mut gui = GuiRuntime::default();
    gui.create_root(menu_handle, &menu, "Main").unwrap();
    let index = gui
        .create_root(basics_handle, &basics, "Demo_Controller")
        .unwrap();
    let root = gui.root_object(index).unwrap();
    let button = gui.store().child_by_name(root, "switchBtn").unwrap();
    assert!(gui.store_mut().click_button(button));
    assert_eq!(
        gui.resolve_sound_file("ui://9leh0eyfgojg7u").as_deref(),
        Some("Basics_gojg7u.wav")
    );
    app.insert_non_send(gui)
        .add_systems(Update, fairygui_bevy::runtime::play_queued_sounds);
    app.update();
    let player = app
        .world_mut()
        .query::<&bevy::audio::AudioPlayer>()
        .single(app.world())
        .unwrap();
    let path = app
        .world()
        .resource::<AssetServer>()
        .get_path(player.0.id())
        .unwrap();
    assert_eq!(path.path().to_string_lossy(), "different/Basics_gojg7u.wav");
}

#[test]
fn transition_labels_are_published_as_bevy_messages_once() {
    use fairygui_bevy::GuiTransitionHook;
    let (mut app, root, entity) = transition_fixture("PowerUp");
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut().play_transition(root, "t0", 1);
        gui.store_mut().advance_transitions(10.0);
    }
    app.update();
    let hooks: Vec<_> = app
        .world_mut()
        .resource_mut::<Messages<GuiTransitionHook>>()
        .drain()
        .collect();
    assert!(hooks.iter().any(|hook| hook.entity == entity
        && hook.hook.owner == root
        && hook.hook.label == "end"
        && hook.hook.at_end));
    app.update();
    assert_eq!(
        app.world_mut()
            .resource_mut::<Messages<GuiTransitionHook>>()
            .drain()
            .count(),
        0
    );
    assert!(
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .take_transition_hooks()
            .is_empty()
    );
}

#[test]
fn menu_title_is_centered_without_moving_its_hit_rectangle() {
    let package = FuiPackage::parse("MainMenu", MAIN_MENU).unwrap();
    let mut gui = GuiRuntime::default();
    let root = gui
        .create_root(Handle::default(), &package, "Main")
        .unwrap();
    let button = gui
        .store()
        .find_by_name(gui.root_object(root).unwrap(), "n1")
        .unwrap();
    let title = gui.store().find_by_name(button, "title").unwrap();
    let data = gui
        .store()
        .object(title)
        .unwrap()
        .control()
        .as_text_field()
        .unwrap();
    assert_eq!((data.align, data.vertical_align), (1, 1));
    let text_layout = fairygui_bevy::backend::text_layout_for(data);
    gui.store_mut().set_size(title, 240.0, 50.0);
    let node = fairygui_bevy::backend::node_for(gui.store().object(title).unwrap());
    let original = node.clone();
    let mut app = App::new();
    app.insert_non_send(gui)
        .add_systems(Update, fairygui_bevy::backend::align_text);
    let entity = app
        .world_mut()
        .spawn((
            fairygui_bevy::GObjectRef(title),
            node,
            text_layout,
            bevy::text::TextLayoutInfo {
                size: Vec2::new(120.0, 24.0) * 1.75,
                scale_factor: 1.75,
                ..default()
            },
        ))
        .id();
    for scale in [1.0, 1.75, 2.5] {
        {
            let mut info = app
                .world_mut()
                .get_mut::<bevy::text::TextLayoutInfo>(entity)
                .unwrap();
            info.scale_factor = scale;
            info.size = Vec2::new(120.0, 24.0) * scale;
        }
        {
            let mut computed = app.world_mut().get_mut::<ComputedNode>(entity).unwrap();
            computed.size = Vec2::new(240., 50.) * scale;
            computed.inverse_scale_factor = scale.recip();
        }
        app.update();
        let node = app.world().get::<Node>(entity).unwrap();
        assert_eq!(node.padding.left, Val::Px(60.0));
        assert_eq!(node.padding.top, Val::Px(13.0));
        let computed = app.world().get::<ComputedNode>(entity).unwrap();
        assert!(
            computed
                .padding
                .min_inset
                .abs_diff_eq(Vec2::new(60., 13.) * scale, 0.0001)
        );
        assert_eq!(
            (node.left, node.top, node.width, node.height),
            (original.left, original.top, original.width, original.height)
        );
    }
    // A new label measurement and a resized object must recenter, not accumulate.
    {
        let mut info = app
            .world_mut()
            .get_mut::<bevy::text::TextLayoutInfo>(entity)
            .unwrap();
        info.scale_factor = 2.5;
        info.size = Vec2::new(160.0, 24.0) * 2.5;
    }
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_size(title, 300.0, 60.0);
    app.update();
    let node = app.world().get::<Node>(entity).unwrap();
    assert_eq!(node.padding.left, Val::Px(70.0));
    assert_eq!(node.padding.top, Val::Px(18.0));
}

#[test]
fn every_basics_demo_component_builds() {
    let package = FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui"),
    )
    .unwrap();
    for item in package
        .items()
        .iter()
        .filter(|item| item.name.starts_with("Demo_"))
    {
        let mut runtime = GuiRuntime::default();
        assert!(
            runtime
                .create_root(Handle::default(), &package, &item.name)
                .is_some(),
            "{} must build",
            item.name
        );
    }
}

#[test]
fn basics_text_demo_keeps_exported_text_layout_modes() {
    let package = FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-demo/assets/ui/Basics.fui"),
    )
    .unwrap();
    let mut runtime = GuiRuntime::default();
    let root = runtime
        .create_root(Handle::default(), &package, "Demo_Text")
        .unwrap();
    let root = runtime.root_object(root).unwrap();

    let field = |name| {
        let id = runtime.store().find_by_name(root, name).unwrap();
        runtime
            .store()
            .object(id)
            .unwrap()
            .control()
            .as_text_field()
            .unwrap()
    };

    let wrapped = field("n5");
    assert_eq!(wrapped.auto_size, 0);
    assert!(!wrapped.single_line);
    assert_eq!(wrapped.leading, 12);
    assert!(wrapped.ubb_enabled);

    let auto_width = field("n0");
    assert_eq!(auto_width.auto_size, 1);
    assert!(!auto_width.single_line);

    let styled = field("n3");
    assert!(styled.bold && styled.italic && styled.underline);

    let input = field("n22");
    assert!(input.single_line);
}

#[test]
fn auto_sized_text_does_not_grow_each_frame_on_hidpi() {
    let package = FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui"),
    )
    .unwrap();
    let mut runtime = GuiRuntime::default();
    let root = runtime
        .create_root(Handle::default(), &package, "Main")
        .unwrap();
    let text = runtime
        .store()
        .find_by_name(runtime.root_object(root).unwrap(), "n24")
        .unwrap();
    let mut app = App::new();
    app.insert_non_send(runtime)
        .add_systems(Update, fairygui_bevy::runtime::sync_text_sizes);
    let entity = app
        .world_mut()
        .spawn((
            fairygui_bevy::GObjectRef(text),
            ComputedNode {
                size: Vec2::new(350.0, 70.0),
                inverse_scale_factor: 1.0 / 1.75,
                ..default()
            },
        ))
        .id();
    for _ in 0..60 {
        app.update();
        let gui = app.world().non_send::<GuiRuntime>();
        let object = gui.store().object(text).unwrap();
        let logical = Vec2::new(object.width(), object.height());
        assert!((logical - Vec2::new(200.0, 40.0)).length() < 0.01);
        app.world_mut()
            .get_mut::<ComputedNode>(entity)
            .unwrap()
            .size = logical * 1.75;
    }
}

#[test]
fn editable_text_is_mirrored_back_to_the_object_model() {
    let package = FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui"),
    )
    .unwrap();
    let mut runtime = GuiRuntime::default();
    let root = runtime
        .create_root(Handle::default(), &package, "Demo_Text")
        .unwrap();
    let input = runtime
        .store()
        .find_by_name(runtime.root_object(root).unwrap(), "n22")
        .unwrap();
    let mut app = App::new();
    app.insert_non_send(runtime)
        .add_systems(Update, fairygui_bevy::runtime::sync_text_inputs);
    app.world_mut().spawn((
        fairygui_bevy::GObjectRef(input),
        Text::new("typed value"),
        bevy::text::EditableText::new("typed value"),
    ));

    app.update();

    let runtime = app.world().non_send::<GuiRuntime>();
    let text = runtime
        .store()
        .object(input)
        .unwrap()
        .control()
        .as_text_field()
        .unwrap();
    assert_eq!(text.text, "typed value");
}

const DEMO_PACKAGES: &[(&str, &[u8])] = &[
    (
        "Basics",
        include_bytes!("../../fairygui-demo/assets/ui/Basics.fui"),
    ),
    (
        "Transition",
        include_bytes!("../../fairygui-demo/assets/ui/Transition.fui"),
    ),
    (
        "VirtualList",
        include_bytes!("../../fairygui-demo/assets/ui/VirtualList.fui"),
    ),
    (
        "LoopList",
        include_bytes!("../../fairygui-demo/assets/ui/LoopList.fui"),
    ),
    (
        "HitTest",
        include_bytes!("../../fairygui-demo/assets/ui/HitTest.fui"),
    ),
    (
        "PullToRefresh",
        include_bytes!("../../fairygui-demo/assets/ui/PullToRefresh.fui"),
    ),
    (
        "ModalWaiting",
        include_bytes!("../../fairygui-demo/assets/ui/ModalWaiting.fui"),
    ),
    (
        "Joystick",
        include_bytes!("../../fairygui-demo/assets/ui/Joystick.fui"),
    ),
    (
        "Bag",
        include_bytes!("../../fairygui-demo/assets/ui/Bag.fui"),
    ),
    (
        "Chat",
        include_bytes!("../../fairygui-demo/assets/ui/Chat.fui"),
    ),
    (
        "ListEffect",
        include_bytes!("../../fairygui-demo/assets/ui/ListEffect.fui"),
    ),
    (
        "ScrollPane",
        include_bytes!("../../fairygui-demo/assets/ui/ScrollPane.fui"),
    ),
    (
        "TreeView",
        include_bytes!("../../fairygui-demo/assets/ui/TreeView.fui"),
    ),
    (
        "Guide",
        include_bytes!("../../fairygui-demo/assets/ui/Guide.fui"),
    ),
    (
        "Cooldown",
        include_bytes!("../../fairygui-demo/assets/ui/Cooldown.fui"),
    ),
];

#[test]
fn all_laya_demo_packages_build_renderable_roots() {
    for (name, bytes) in DEMO_PACKAGES {
        let package = FuiPackage::parse(*name, bytes).expect("demo package should parse");
        let mut runtime = GuiRuntime::default();
        let index = runtime
            .create_root(Handle::<FuiAsset>::default(), &package, "Main")
            .unwrap_or_else(|| panic!("{name} should contain a Main component"));
        let root = runtime.root_object(index).expect("demo root object");
        assert!(
            runtime
                .store()
                .object(root)
                .is_some_and(|object| !object.children().is_empty()),
            "{name} Main should contain renderable children"
        );
    }
}

#[test]
fn creates_object_tree_from_package() {
    let package = FuiPackage::parse("MainMenu", MAIN_MENU).expect("parse package");
    let mut runtime = GuiRuntime::default();

    let handle = Handle::<FuiAsset>::default();
    let index = runtime
        .create_root(handle, &package, "Main")
        .expect("root should be created");

    assert_eq!(index, 0);
    assert_eq!(runtime.root_count(), 1);

    let root = runtime.root_object(index).expect("root object");
    let object = runtime.store().object(root).expect("object");

    assert_eq!((object.width(), object.height()), (1136.0, 640.0));
    assert!(!object.children().is_empty());
    assert!(runtime.store().len() > 1, "整棵对象树都应当被创建出来");
}

#[test]
fn gui_events_are_queued_for_the_target_and_each_bubbling_ancestor() {
    let mut runtime = GuiRuntime::default();
    let parent = runtime
        .store_mut()
        .create(fairygui_core::package::ObjectType::Component);
    let child = runtime
        .store_mut()
        .create(fairygui_core::package::ObjectType::Button);
    assert!(runtime.store_mut().add_child_at(parent, child, 0));
    runtime.dispatch(fairygui_core::object::EventKind::Click, child);
    let events: Vec<_> = runtime.drain_events().collect();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].target, child);
    assert_eq!(events[0].current_target, child);
    assert!(!events[0].bubbling);
    assert_eq!(events[1].target, child);
    assert_eq!(events[1].current_target, parent);
    assert!(events[1].bubbling);
    assert_eq!(runtime.pending_event_count(), 0);
}

/// 建一个带根节点的运行期，用于窗口相关的测试。
///
/// 这里借用包里的普通组件充当窗口：它没有窗框与关闭按钮，因此少了对应的
/// 行为，但窗口本身——入栈、居中、模态、显隐——都能验证。
fn runtime_with_root(package: &FuiPackage) -> GuiRuntime {
    let mut runtime = GuiRuntime::default();
    let handle = Handle::<FuiAsset>::default();
    runtime
        .create_root(handle, package, "Main")
        .expect("root should be created");
    runtime
}

const ROOT_SIZE: fairygui_core::geom::Size = fairygui_core::geom::Size::new(1280.0, 720.0);

#[test]
fn window_stack_and_modality() {
    let package = FuiPackage::parse("MainMenu", MAIN_MENU).expect("parse package");
    let mut runtime = runtime_with_root(&package);

    let first = runtime
        .show_window(&package, "Main", false, ROOT_SIZE)
        .expect("应当能创建窗口");
    assert!(!runtime.is_modal(), "普通窗口不构成模态");

    let second = runtime
        .show_window(&package, "Main", true, ROOT_SIZE)
        .expect("应当能创建第二个窗口");
    assert_eq!(runtime.window_count(), 2);
    assert!(runtime.is_modal(), "有模态窗口时应当处于模态状态");

    // 窗口都挂在根容器下，因此与界面共享同一套层级顺序。
    let root_container = runtime.root_container().expect("根容器");
    for index in [first, second] {
        let owner = runtime.window(index).expect("窗口").owner();
        assert_eq!(
            runtime.store().object(owner).expect("window").parent(),
            Some(root_container)
        );
    }

    // 模态窗口之内的对象照常，之外的完全不响应。
    let modal_owner = runtime.window(second).expect("窗口").owner();
    let outside = runtime.root_object(0).expect("界面根对象");
    assert!(runtime.is_in_top_modal(modal_owner));
    assert!(
        !runtime.is_in_top_modal(outside),
        "模态窗口之外的界面不应响应输入"
    );

    // 关掉模态窗口即解除阻挡。
    let mut app = App::new();
    let world = app.world_mut();
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let mut commands = Commands::new(&mut queue, world);
    assert!(runtime.close_window(&mut commands, second));
    queue.apply(world);

    assert_eq!(runtime.window_count(), 1);
    assert!(!runtime.is_modal(), "模态窗口关闭后应当解除阻挡");
}

#[test]
fn window_events_are_dispatched() {
    use fairygui_core::object::EventKind;

    let package = FuiPackage::parse("MainMenu", MAIN_MENU).expect("parse package");
    let mut runtime = runtime_with_root(&package);

    // 创建界面的过程可能已经留下事件，先清空。
    let _ = runtime.drain_events().count();

    let index = runtime
        .show_window(&package, "Main", false, ROOT_SIZE)
        .expect("应当能创建窗口");
    assert!(
        runtime
            .drain_events()
            .any(|event| event.kind == EventKind::WindowShown),
        "显示窗口应当派发事件"
    );

    assert!(runtime.hide_window(index));
    assert!(
        runtime
            .drain_events()
            .any(|event| event.kind == EventKind::WindowHide),
        "隐藏窗口应当派发事件"
    );

    assert!(runtime.reopen_window(index, false));
    assert!(
        runtime
            .drain_events()
            .any(|event| event.kind == EventKind::WindowShown),
        "重新显示也应当派发事件"
    );

    // 关闭同样派发隐藏事件——与官方一致，两种情况共用一个事件。
    let mut app = App::new();
    let world = app.world_mut();
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let mut commands = Commands::new(&mut queue, world);
    assert!(runtime.close_window(&mut commands, index));
    queue.apply(world);

    assert!(
        runtime
            .drain_events()
            .any(|event| event.kind == EventKind::WindowHide),
        "关闭窗口应当派发事件"
    );
}

#[test]
fn window_can_be_hidden_and_reopened() {
    let package = FuiPackage::parse("MainMenu", MAIN_MENU).expect("parse package");
    let mut runtime = runtime_with_root(&package);

    let index = runtime
        .show_window(&package, "Main", false, ROOT_SIZE)
        .expect("应当能创建窗口");
    assert!(runtime.window(index).expect("窗口").is_showing());

    assert!(runtime.hide_window(index));
    assert!(!runtime.window(index).expect("窗口").is_showing());

    // 隐藏只是从根节点摘下来，对象还在，可以再次显示。
    assert!(runtime.reopen_window(index, false));
    assert!(runtime.window(index).expect("窗口").is_showing());
    assert_eq!(runtime.window_count(), 1, "重新显示不应新建窗口");
}

/// 从参考库里找一个**声明了依赖**的示例包。
///
/// 六个夹具包的依赖都是空的，拿它们测不出任何东西——只有参考库里那些相互
/// 引用的界面才带依赖。返回 `(包名, 依赖名列表)`。
fn package_with_dependencies() -> Option<(String, Vec<String>)> {
    let dir = "../../.refs/FairyGUI-layabox/demo/assets/resources/ui";
    let entries = std::fs::read_dir(dir).ok()?;

    // 排序保证每次跑挑到的是同一个包，结果稳定。
    let mut paths: Vec<std::path::PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "fui"))
        .collect();
    paths.sort();

    for path in paths {
        let Some(name) = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
        else {
            continue;
        };
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(package) = FuiPackage::parse(name.clone(), &bytes) else {
            continue;
        };

        let dependencies: Vec<String> = package
            .dependencies()
            .iter()
            .map(|dependency| dependency.name.clone())
            .collect();
        if !dependencies.is_empty() {
            return Some((name, dependencies));
        }
    }
    None
}

/// 跨包地址：只登记一个包时解析不了，把另一个包登记进来就能解析。
///
/// 这是注册表最实际的收益——滚动条一类的资源引用可以放在别的包里。
#[test]
fn cross_package_urls_resolve_after_registration() {
    const BASICS: &[u8] = include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui");

    let main = FuiPackage::parse("MainMenu", MAIN_MENU).expect("parse main");
    let other = FuiPackage::parse("Basics", BASICS).expect("parse basics");

    let item = other.items().first().expect("Basics 应当有条目");
    let url = format!("ui://{}/{}", other.name(), item.name);

    let mut runtime = GuiRuntime::default();
    runtime.register_package(Handle::default(), &main);

    // 只登记了主包，指向另一个包的地址解析不出来。
    assert!(
        runtime.resolve_url(&url).is_none(),
        "包还没登记时不该能解析出跨包地址"
    );

    // 登记之后就能解析，而且解析出的正是那个包。
    runtime.register_package(Handle::default(), &other);
    let resolved = runtime.resolve_url(&url).expect("登记之后应当能解析");
    assert_eq!(
        resolved.0.name(),
        other.name(),
        "解析出应当是地址所指的那个包"
    );

    // 地址里的条目不存在时仍然解析失败。
    assert!(
        runtime
            .resolve_url(&format!("ui://{}/绝不存在", other.name()))
            .is_none()
    );
}

/// 音效地址解析成包内记录的文件名。
///
/// 包内记录的图集与声音文件名带包名前缀但不带目录，目录与包文件本身相同
/// ——这里解析出的是文件名那一半，资产路径由 `FuiAsset` 补上目录。
#[test]
fn sound_urls_resolve_to_package_file_names() {
    use fairygui_core::package::PackageItemType;

    const BASICS: &[u8] = include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui");
    let package = FuiPackage::parse("Basics", BASICS).expect("parse package");

    let Some(sound) = package
        .items()
        .iter()
        .find(|item| item.item_type == PackageItemType::Sound)
    else {
        println!("Basics 里没有声音条目，跳过");
        return;
    };

    let mut runtime = GuiRuntime::default();
    runtime.register_package(Handle::default(), &package);

    let url = format!("ui://{}/{}", package.name(), sound.name);
    assert_eq!(
        runtime.resolve_sound_file(&url),
        sound.file,
        "地址应当解析出条目记录的文件名"
    );

    // 不是 ui:// 形式时按文件名直接使用。
    assert_eq!(
        runtime.resolve_sound_file("plain.wav"),
        Some("plain.wav".to_string())
    );

    // 指向不存在的条目时解析失败，调用方会据此跳过。
    assert_eq!(
        runtime.resolve_sound_file(&format!("ui://{}/绝不存在", package.name())),
        None
    );
}

/// 动态加进来的子对象会补上实体。
///
/// 这是对象池复用与列表虚拟化的形状：项对象先在存储里建好、挂到容器下，
/// 再让运行期补出实体。
#[test]
fn adding_a_child_queues_its_entity() {
    use fairygui_core::object::dirty;
    use fairygui_core::package::ObjectType;

    let package = FuiPackage::parse("MainMenu", MAIN_MENU).expect("parse package");
    let mut runtime = runtime_with_root(&package);
    let root = runtime.root_object(0).expect("root object");

    let child = runtime.store_mut().create(ObjectType::Component);
    runtime.store_mut().set_parent(child, Some(root));

    // 核心层要把容器标脏，运行期才会去补实体。
    let touched = runtime.store_mut().take_dirty();
    assert!(
        touched
            .iter()
            .any(|(id, flags)| *id == root && flags & dirty::CHILDREN != 0),
        "挂上子对象应当把容器的 CHILDREN 标脏"
    );

    assert!(
        !runtime.is_pending_spawn(child),
        "还没有人通知生成实体时不该在队列里"
    );

    runtime.queue_subtree(child);
    assert!(runtime.is_pending_spawn(child), "排进队列后应当等待生成");
}

/// 排队是把整棵树带上，不只是那一个对象。
#[test]
fn queuing_a_subtree_includes_descendants() {
    use fairygui_core::package::ObjectType;

    let package = FuiPackage::parse("MainMenu", MAIN_MENU).expect("parse package");
    let mut runtime = runtime_with_root(&package);
    let root = runtime.root_object(0).expect("root object");

    let parent = runtime.store_mut().create(ObjectType::Component);
    runtime.store_mut().set_parent(parent, Some(root));
    let child = runtime.store_mut().create(ObjectType::Component);
    runtime.store_mut().set_parent(child, Some(parent));

    runtime.queue_subtree(parent);

    assert!(runtime.is_pending_spawn(parent));
    assert!(
        runtime.is_pending_spawn(child),
        "子对象应当跟着父对象一起入队"
    );
}

/// 重复排队不会把同一个对象排进去两次。
#[test]
fn queuing_is_idempotent() {
    use fairygui_core::package::ObjectType;

    let package = FuiPackage::parse("MainMenu", MAIN_MENU).expect("parse package");
    let mut runtime = runtime_with_root(&package);
    let root = runtime.root_object(0).expect("root object");

    let child = runtime.store_mut().create(ObjectType::Component);
    runtime.store_mut().set_parent(child, Some(root));

    runtime.queue_subtree(child);
    runtime.queue_subtree(child);

    // 对象已经在队列里，再次排队应当被跳过——队列里只该有一份。
    assert!(runtime.is_pending_spawn(child));
    assert_eq!(
        runtime.store().object(child).map(|object| object.parent()),
        Some(Some(root)),
        "排队不应改动对象树"
    );
}

#[test]
fn registering_a_package_reports_its_missing_dependencies() {
    let Some((name, declared)) = package_with_dependencies() else {
        println!("参考库里没有带依赖的包，跳过");
        return;
    };
    println!("{name} 声明的依赖：{declared:?}");

    let dir = "../../.refs/FairyGUI-layabox/demo/assets/resources/ui";
    let bytes = std::fs::read(format!("{dir}/{name}.fui")).expect("read package");
    let package = FuiPackage::parse(name.clone(), &bytes).expect("parse package");

    let mut runtime = GuiRuntime::default();

    // 一个依赖都还没登记，因此声明的依赖全都算缺失。
    let missing = runtime.register_package(Handle::default(), &package);
    assert_eq!(
        missing, declared,
        "首次登记时，声明的依赖应当原样报告为缺失"
    );

    // 把第一个依赖登记进去，它就不该再出现在缺失列表里。
    let first = declared.first().expect("至少有一个依赖").clone();
    let Ok(dep_bytes) = std::fs::read(format!("{dir}/{first}.fui")) else {
        println!("依赖 {first} 的文件不在，跳过后续断言");
        return;
    };
    let Ok(dep_package) = FuiPackage::parse(first.clone(), &dep_bytes) else {
        println!("依赖 {first} 解析失败，跳过后续断言");
        return;
    };

    runtime.register_package(Handle::default(), &dep_package);
    let missing = runtime.register_package(Handle::default(), &package);
    assert!(
        !missing.contains(&first),
        "已登记的依赖不该再报告为缺失（{first}）"
    );
    assert_eq!(
        missing.len(),
        declared.len() - 1,
        "缺失列表应当只少掉那一个已登记的依赖"
    );
}

#[test]
fn virtual_lists_are_notified_when_scrolled() {
    let package = FuiPackage::parse("MainMenu", MAIN_MENU).expect("parse package");
    let mut runtime = runtime_with_root(&package);
    let root = runtime.root_object(0).expect("界面根对象");

    // 没登记的列表不会被通知——滚动事件是全局的，只有登记过的才关心。
    runtime.note_scrolled(root);
    assert!(runtime.take_scrolled_virtual_lists().is_empty());

    // 登记时立刻通知一次，否则使用方不知道从哪儿开始补项。
    runtime.register_virtual_list(root);
    assert!(runtime.is_virtual_list(root));
    assert_eq!(runtime.take_scrolled_virtual_lists(), vec![root]);

    // 取走即清空，同一次滚动不会反复通知。
    assert!(runtime.take_scrolled_virtual_lists().is_empty());

    // 之后每次滚动重新通知。
    runtime.note_scrolled(root);
    runtime.note_scrolled(root);
    assert_eq!(
        runtime.take_scrolled_virtual_lists(),
        vec![root],
        "同一次滚动里通知多次也只算一次"
    );

    // 取消登记后彻底不再打扰。
    runtime.unregister_virtual_list(root);
    assert!(!runtime.is_virtual_list(root));
    runtime.note_scrolled(root);
    assert!(runtime.take_scrolled_virtual_lists().is_empty());
}

#[test]
fn window_parts_can_be_specified() {
    let package = FuiPackage::parse("MainMenu", MAIN_MENU).expect("parse package");
    let mut runtime = runtime_with_root(&package);

    let index = runtime
        .show_window(&package, "Main", false, ROOT_SIZE)
        .expect("应当能创建窗口");
    let owner = runtime.window(index).expect("窗口").owner();

    // 手动指定内容与窗框：两者都会成为窗口的子对象。
    let content = runtime
        .store()
        .object(owner)
        .and_then(|window| window.children().first().copied())
        .expect("窗口应当有子对象");

    assert!(runtime.set_window_content(index, content));
    assert_eq!(
        runtime.window(index).expect("窗口").content(),
        Some(content)
    );
    assert_eq!(
        runtime
            .store()
            .object(content)
            .and_then(|item| item.parent()),
        Some(owner),
        "内容应当挂到窗口下"
    );

    assert!(runtime.set_window_drag_area(index, content));
    assert_eq!(
        runtime.window(index).expect("窗口").drag_area(),
        Some(content)
    );

    assert!(runtime.set_window_close_button(index, content));
    assert_eq!(
        runtime.window(index).expect("窗口").close_button(),
        Some(content)
    );

    // 禁止拖动后窗口自己知道这件事。
    assert!(runtime.set_window_draggable(index, false));
    assert!(!runtime.window(index).expect("窗口").is_draggable());
}

#[test]
fn window_modal_wait_roundtrip() {
    let package = FuiPackage::parse("MainMenu", MAIN_MENU).expect("parse package");
    let mut runtime = runtime_with_root(&package);

    let index = runtime
        .show_window(&package, "Main", true, ROOT_SIZE)
        .expect("应当能创建窗口");

    assert!(!runtime.is_window_modal_waiting(index));

    assert!(runtime.show_window_modal_wait(index, 1));
    assert!(runtime.is_window_modal_waiting(index));

    // 命令编号不匹配时不会误关别人的等待。
    assert!(!runtime.close_window_modal_wait(index, 2));
    assert!(runtime.is_window_modal_waiting(index));

    assert!(runtime.close_window_modal_wait(index, 1));
    assert!(!runtime.is_window_modal_waiting(index));
}

#[test]
fn window_containing_distinguishes_windows_from_content() {
    let package = FuiPackage::parse("MainMenu", MAIN_MENU).expect("parse package");
    let mut runtime = runtime_with_root(&package);

    let index = runtime
        .show_window(&package, "Main", false, ROOT_SIZE)
        .expect("应当能创建窗口");

    // 窗口里的对象能反查到所属窗口。
    let owner = runtime.window(index).expect("窗口").owner();
    let child = runtime
        .store()
        .object(owner)
        .and_then(|object| object.children().first().copied());
    if let Some(child) = child {
        assert_eq!(runtime.window_containing(child), Some(index));
    }

    // 普通界面里的对象不属于任何窗口。
    let outside = runtime.root_object(0).expect("界面根对象");
    assert_eq!(runtime.window_containing(outside), None);
}

#[test]
fn removing_root_reclaims_objects() {
    let package = FuiPackage::parse("Bag", BAG).expect("parse package");
    let mut runtime = GuiRuntime::default();

    let handle = Handle::<FuiAsset>::default();
    let index = runtime
        .create_root(handle, &package, "BagWin")
        .expect("root should be created");
    assert!(runtime.store().len() > 1);

    // 无实体时需要 Commands，这里用最简的应用取出一个。
    let mut app = App::new();
    let world = app.world_mut();
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let mut commands = Commands::new(&mut queue, world);
    runtime.remove_root(&mut commands, index);
    queue.apply(world);

    assert_eq!(runtime.store().len(), 0, "销毁界面应当回收全部对象");
    assert_eq!(
        runtime.root_count(),
        1,
        "界面记录本身保留，由调用方决定何时清理"
    );
}

#[test]
fn translations_apply_to_text_lookups() {
    use fairygui_core::translation::TranslationTable;

    let mut runtime = GuiRuntime::default();

    // 空表是直通的，不该把占位符吃掉。
    assert_eq!(runtime.translate("!title!"), "!title!");

    let mut table = TranslationTable::new();
    table.set("title", "开始游戏");
    runtime.set_translations(table);

    assert_eq!(runtime.translate("!title!"), "开始游戏");
    assert_eq!(
        runtime.translate("缺翻译 !missing!"),
        "缺翻译 !missing!",
        "缺翻译时应当保留占位符，便于定位"
    );
    assert_eq!(runtime.translations().get("title"), Some("开始游戏"));
}

#[test]
fn missing_item_yields_no_root() {
    let package = FuiPackage::parse("MainMenu", MAIN_MENU).expect("parse package");
    let mut runtime = GuiRuntime::default();

    let handle = Handle::<FuiAsset>::default();
    assert!(runtime.create_root(handle, &package, "NotExist").is_none());
    assert!(runtime.store().is_empty());
}

//! Laya 示例的业务控制层。
//!
//! Demo actions observe FairyGUI events on their corresponding Bevy entities,
//! keeping application behavior out of the shared input layer.

use bevy::prelude::*;
use fairygui_bevy::{
    FairyGuiRoot, GuiMenuAction, GuiObjectAction, GuiObjectCommand, GuiQuery, GuiRootInstance,
    GuiRuntime, GuiTransitionHook,
};
use fairygui_core::object::GObjectId;

/// Match the official Basics text example: the link replaces its own text.
pub fn handle_links(
    mut links: MessageReader<fairygui_bevy::GuiLink>,
    controller: Res<DemoController>,
    runtime: NonSend<GuiRuntime>,
    mut commands: MessageWriter<GuiObjectCommand>,
) {
    for link in links.read() {
        let Some(root) = controller
            .basics_page
            .and_then(|handle| runtime.root_object(handle))
        else {
            continue;
        };
        if controller.package.as_deref() == Some("Basics")
            && runtime.store().child_by_name(root, "n12") == Some(link.event.target)
            && let Some(target) = runtime.entity_of(link.event.target)
        {
            commands.write(GuiObjectCommand::new(
                target,
                GuiObjectAction::SetText(format!(
                    "[img]ui://9leh0eyft9fj5f[/img][color=#FF0000]你点击了链接[/color]：{}",
                    link.href
                )),
            ));
        }
    }
}

/// Demo 内部点击动作编号从菜单动作编号之后开始，避免混用。
pub const DEMO_ACTION_BASE: u64 = 10_000;

/// 当前打开 Demo 的运行状态。
#[derive(Resource, Default)]
pub struct DemoController {
    pub package: Option<String>,
    pub root: Option<GObjectId>,
    pub(super) root_owner: Option<Entity>,
    pub elapsed: f32,
    pub basics_page: Option<usize>,
    pub transition_sample: Option<usize>,
    pub(super) basics_page_owner: Option<Entity>,
    transition_sample_owner: Option<Entity>,
    pub last_menu_action: Option<GuiMenuAction>,
    numeric_elapsed: Option<f32>,
    transition_paused: bool,
    transition_time_scale: f32,
    transition_start_pending: bool,
}

/// Identifies dynamically loaded demo roots that need one-time setup.
#[derive(Component, Debug, Clone, Copy)]
pub enum DemoRootKind {
    BasicsPage,
    TransitionSample { index: usize },
}

#[derive(Component, Debug)]
pub(super) struct DemoRootInitialized;

pub fn dispose_dynamic_roots(commands: &mut Commands, controller: &mut DemoController) {
    if let Some(owner) = controller.basics_page_owner.take() {
        commands.entity(owner).despawn();
    }
    if let Some(owner) = controller.transition_sample_owner.take() {
        commands.entity(owner).despawn();
    }
    controller.basics_page = None;
    controller.transition_sample = None;
    controller.numeric_elapsed = None;
    controller.transition_start_pending = false;
}

/// Resolve roots created by FairyGuiRoot and issue their one-time setup through
/// entity-addressed commands. This keeps temporary pages in ECS ownership too.
pub fn initialize_demo_roots(
    mut commands: Commands,
    mut gui_commands: MessageWriter<GuiObjectCommand>,
    roots: Query<(Entity, &GuiRootInstance, &DemoRootKind), Without<DemoRootInitialized>>,
    gui: GuiQuery,
    mut controller: ResMut<DemoController>,
) {
    let Some(root) = controller.root_owner else {
        return;
    };
    let Some(owner) = gui.transform(root) else {
        return;
    };
    for (entity, instance, kind) in &roots {
        match *kind {
            DemoRootKind::BasicsPage => {
                let container = gui.find_by_name(root, "container").unwrap_or(root);
                let y =
                    owner.position().y + gui.transform(container).map_or(70.0, |o| o.position().y);
                gui_commands.write(GuiObjectCommand::new(
                    entity,
                    GuiObjectAction::SetPosition(Vec2::new(owner.position().x, y)),
                ));
                controller.basics_page = Some(instance.handle);
                controller.basics_page_owner = Some(entity);
            }
            DemoRootKind::TransitionSample { index } => {
                let Some(sample) = gui.transform(entity) else {
                    continue;
                };
                let position = match index {
                    3 => Vec2::new(
                        owner.position().x + owner.size().x - sample.size().x - 20.0,
                        owner.position().y + 100.0,
                    ),
                    4 => Vec2::new(
                        owner.position().x + 20.0,
                        owner.position().y + owner.size().y - sample.size().y - 100.0,
                    ),
                    _ => owner.position(),
                };
                gui_commands.write(GuiObjectCommand::new(
                    entity,
                    GuiObjectAction::SetPosition(position),
                ));
                gui_commands.write(GuiObjectCommand::new(
                    entity,
                    GuiObjectAction::PlayTransition {
                        name: "t0".into(),
                        times: if index == 3 { 3 } else { 1 },
                    },
                ));
                if index == 4 {
                    for (name, value) in [("value", "10000"), ("add_value", "+2000")] {
                        if let Some(target) = gui.find_by_name(entity, name) {
                            gui_commands.write(GuiObjectCommand::new(
                                target,
                                GuiObjectAction::SetText(value.into()),
                            ));
                        }
                    }
                }
                if let Some(target) = gui.child_by_name(root, "g0") {
                    gui_commands.write(GuiObjectCommand::new(
                        target,
                        GuiObjectAction::SetVisible(false),
                    ));
                }
                controller.transition_sample = Some(instance.handle);
                controller.transition_sample_owner = Some(entity);
                controller.transition_paused = false;
                controller.transition_time_scale = 1.0;
                controller.transition_start_pending = true;
                controller.numeric_elapsed = None;
            }
        }
        commands.entity(entity).insert(DemoRootInitialized);
    }
}

pub const BASICS_BACK: u64 = DEMO_ACTION_BASE + 100;
pub const BASICS_ENTRY: u64 = DEMO_ACTION_BASE + 200;
pub const BASICS_MENU_ACTION: u64 = DEMO_ACTION_BASE + 700;
pub const BASICS_PAGES: &[&str] = &[
    "Graph",
    "Image",
    "MovieClip",
    "Text",
    "Loader",
    "List",
    "Grid",
    "Controller",
    "Relation",
    "Clip&Scroll",
    "Component",
    "Label",
    "Button",
    "ComboBox",
    "Slider",
    "ProgressBar",
    "Popup",
    "Window",
    "Depth",
    "Drag&Drop",
];

/// Initialize behavior that does not depend on Bevy entity observers.
pub fn initialize_demo(runtime: &mut GuiRuntime, package: &str, root: GObjectId) {
    if package == "LoopList" {
        crate::loop_list_demo::initialize(runtime, root);
    }
    if package == "VirtualList"
        && let Some(list) = runtime.store().child_by_name(root, "mailList")
    {
        runtime
            .enable_virtual_list(list, |store, index, row| {
                store.set_controller_page_index(row, "IsRead", i32::from(index % 2 == 0));
                store.set_controller_page_index(row, "c1", i32::from(index % 3 == 0));
                if let Some(time) = store.child_by_name(row, "timeText") {
                    store.set_object_text(time, "5 Nov 2015 16:24:33");
                }
                store.set_object_text(row, &format!("{index} Mail title here"));
            })
            .expect("VirtualList default item");
        runtime
            .set_virtual_item_count(list, 1000)
            .expect("VirtualList renderer");
    }
}

pub fn demo_action_bindings(gui: &GuiQuery, package: &str, root: Entity) -> Vec<(Entity, u64)> {
    let mut bindings = Vec::new();
    if package == "Basics" {
        if let Some(back) = gui.find_by_name(root, "btn_Back") {
            bindings.push((back, BASICS_BACK));
        }
        for (index, page) in BASICS_PAGES.iter().enumerate() {
            if let Some(button) = gui.find_by_name(root, &format!("btn_{page}")) {
                bindings.push((button, BASICS_ENTRY + index as u64));
            }
        }
        return bindings;
    }
    let names: &[&str] = match package {
        "VirtualList" => &["n6", "n7", "n8"],
        "Transition" => &["btn0", "btn1", "btn2", "btn3", "btn4", "btn5"],
        _ => &[],
    };
    bindings.extend(names.iter().enumerate().filter_map(|(index, name)| {
        gui.find_by_name(root, name)
            .map(|object| (object, DEMO_ACTION_BASE + index as u64))
    }));
    bindings
}

/// 官方过渡示例的六个播放入口。展示对象作为独立根，结束后回收。
pub fn navigate_transition(
    commands: &mut Commands,
    controller: &mut DemoController,
    handle: Handle<fairygui_bevy::FuiAsset>,
    listener: u64,
) -> bool {
    if controller.package.as_deref() != Some("Transition") {
        return false;
    }
    let Some(index) = listener.checked_sub(DEMO_ACTION_BASE).filter(|i| *i < 6) else {
        return false;
    };
    if controller.transition_sample_owner.is_some() {
        return true;
    }
    let samples = [
        "BOSS",
        "BOSS_SKILL",
        "TRAP",
        "GoodHit",
        "PowerUp",
        "PathDemo",
    ];
    let owner = commands
        .spawn((
            FairyGuiRoot::new(handle, samples[index as usize]),
            DemoRootKind::TransitionSample {
                index: index as usize,
            },
        ))
        .id();
    controller.transition_sample_owner = Some(owner);
    true
}

/// 处理 PowerUp 的数字钩子，并在有限过渡结束后恢复按钮组。
pub fn advance_transition_demo(
    mut commands: Commands,
    runtime: NonSend<GuiRuntime>,
    mut controller: ResMut<DemoController>,
    time: Res<Time>,
    mut hooks: MessageReader<GuiTransitionHook>,
    mut gui_commands: MessageWriter<GuiObjectCommand>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    let sample = controller
        .transition_sample
        .and_then(|handle| runtime.root_object(handle));
    for GuiTransitionHook(hook) in hooks.read() {
        if Some(hook.owner) == sample && hook.label == "play_num_now" && !hook.at_end {
            controller.numeric_elapsed = Some(0.0);
        }
    }
    let Some(object) = sample else { return };
    if controller.transition_start_pending {
        controller.transition_start_pending = false;
        return;
    }
    if keys.just_pressed(KeyCode::Space) {
        controller.transition_paused = !controller.transition_paused;
        if let Some(target) = runtime.entity_of(object) {
            gui_commands.write(GuiObjectCommand::new(
                target,
                GuiObjectAction::SetTransitionPaused {
                    name: "t0".into(),
                    paused: controller.transition_paused,
                },
            ));
        }
    }
    if keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::ArrowDown) {
        controller.transition_time_scale = if keys.just_pressed(KeyCode::ArrowUp) {
            controller.transition_time_scale * 2.0
        } else {
            controller.transition_time_scale * 0.5
        };
        controller.transition_time_scale = controller.transition_time_scale.clamp(0.25, 4.0);
        if let Some(target) = runtime.entity_of(object) {
            gui_commands.write(GuiObjectCommand::new(
                target,
                GuiObjectAction::SetTransitionTimeScale {
                    name: "t0".into(),
                    scale: controller.transition_time_scale,
                },
            ));
        }
    }
    if keys.just_pressed(KeyCode::KeyR)
        && let Some(target) = runtime.entity_of(object)
    {
        gui_commands.write(GuiObjectCommand::new(
            target,
            GuiObjectAction::PlayTransitionReverse {
                name: "t0".into(),
                delay: 0.0,
            },
        ));
    }
    if let Some(elapsed) = &mut controller.numeric_elapsed {
        *elapsed = (*elapsed + time.delta_secs()).min(0.3);
        let value = (10000.0 + 2000.0 * *elapsed / 0.3).floor() as i32;
        if let Some(id) = runtime.store().child_by_name(object, "value")
            && let Some(target) = runtime.entity_of(id)
        {
            gui_commands.write(GuiObjectCommand::new(
                target,
                GuiObjectAction::SetText(value.to_string()),
            ));
        }
    }
    if runtime
        .store()
        .transition_state(object, "t0")
        .is_some_and(|(playing, _)| !playing)
    {
        controller.transition_sample = None;
        if let Some(owner) = controller.transition_sample_owner.take() {
            commands.entity(owner).despawn();
        }
        controller.numeric_elapsed = None;
        if let Some(group) = controller
            .root
            .and_then(|id| runtime.store().child_by_name(id, "g0"))
            .and_then(|id| runtime.entity_of(id))
        {
            gui_commands.write(GuiObjectCommand::new(
                group,
                GuiObjectAction::SetVisible(true),
            ));
        }
    }
}

/// The Basics package contains a menu and separate Demo_* components.
pub fn navigate_basics(
    commands: &mut Commands,
    runtime: &mut GuiRuntime,
    controller: &mut DemoController,
    handle: Handle<fairygui_bevy::FuiAsset>,
    listener: u64,
    gui_commands: &mut MessageWriter<GuiObjectCommand>,
) -> bool {
    if controller.package.as_deref() != Some("Basics") {
        return false;
    }
    let Some(root) = controller.root else {
        return false;
    };
    let page = listener
        .checked_sub(BASICS_ENTRY)
        .and_then(|i| BASICS_PAGES.get(i as usize));
    if listener != BASICS_BACK && page.is_none() {
        return false;
    }
    runtime.hide_popup(None);
    controller.last_menu_action = None;
    if let Some(owner) = controller.basics_page_owner.take() {
        commands.entity(owner).despawn();
    }
    controller.basics_page = None;
    if let Some(target) = runtime.entity_of(root) {
        gui_commands.write(GuiObjectCommand::new(
            target,
            GuiObjectAction::SetControllerPage {
                name: "c1".into(),
                index: i32::from(page.is_some()),
            },
        ));
    }
    if let Some(page) = page {
        let owner = commands
            .spawn((
                FairyGuiRoot::new(handle, format!("Demo_{page}")),
                DemoRootKind::BasicsPage,
            ))
            .id();
        controller.basics_page_owner = Some(owner);
    }
    for name in BASICS_PAGES {
        if let Some(button) = runtime.store().find_by_name(root, &format!("btn_{name}"))
            && let Some(target) = runtime.entity_of(button)
        {
            gui_commands.write(GuiObjectCommand::new(
                target,
                GuiObjectAction::SetVisible(page.is_none()),
            ));
        }
    }
    if let Some(back) = runtime.store().find_by_name(root, "btn_Back")
        && let Some(target) = runtime.entity_of(back)
    {
        gui_commands.write(GuiObjectCommand::new(
            target,
            GuiObjectAction::SetVisible(page.is_some()),
        ));
    }
    true
}

/// The official Popup page's menu entry, using application actions rather than
/// interpreting the runtime's raw event queue.
pub fn show_basics_menu(
    event: On<fairygui_bevy::GuiEvent>,
    mut commands: Commands,
    controller: Res<DemoController>,
    mut runtime: NonSendMut<GuiRuntime>,
    assets: Res<Assets<fairygui_bevy::FuiAsset>>,
    windows: Query<&Window>,
) {
    if event.event().event.kind != fairygui_core::object::EventKind::Click {
        return;
    }
    if event.event().event.kind != fairygui_core::object::EventKind::Click {
        return;
    }
    use fairygui_core::{
        geom::{Point, Size},
        popup::MenuItem,
    };
    {
        let Some(page) = controller
            .basics_page
            .and_then(|handle| runtime.root_object(handle))
        else {
            return;
        };
        if controller.package.as_deref() != Some("Basics") {
            return;
        }
        let Some(handle) = runtime.asset_handle_for(page) else {
            return;
        };
        let Some(asset) = assets.get(&handle) else {
            return;
        };
        let Some(button) = runtime.store().object(event.event().event.current_target) else {
            return;
        };
        let mut at = Point::new(button.x(), button.y() + button.height());
        let mut parent = button.parent();
        while let Some(object) = parent.and_then(|id| runtime.store().object(id)) {
            at.x += object.x();
            at.y += object.y();
            parent = object.parent();
        }
        if let Some(window) = windows.iter().next() {
            runtime.ensure_root(Size::new(window.width(), window.height()));
        }
        let items = [
            MenuItem::action("first", "Item 1", BASICS_MENU_ACTION),
            MenuItem {
                checkable: true,
                ..MenuItem::action("toggle", "Item 2", BASICS_MENU_ACTION + 1)
            },
            MenuItem {
                enabled: false,
                ..MenuItem::action("disabled", "Item 3 (disabled)", BASICS_MENU_ACTION + 2)
            },
            MenuItem::action("last", "Item 4", BASICS_MENU_ACTION + 3),
        ];
        runtime.show_popup_menu_items(
            &mut commands,
            &asset.package,
            fairygui_bevy::PopupMenuTemplates {
                content: "PopupMenu",
                item: "PopupMenuItem",
                separator: None,
            },
            &items,
            at,
        );
    }
}

pub fn handle_menu_actions(
    mut actions: MessageReader<GuiMenuAction>,
    mut controller: ResMut<DemoController>,
    runtime: NonSend<GuiRuntime>,
    mut commands: MessageWriter<GuiObjectCommand>,
) {
    for action in actions.read() {
        if controller.package.as_deref() != Some("Basics")
            || !action
                .action
                .is_some_and(|id| (BASICS_MENU_ACTION..BASICS_MENU_ACTION + 4).contains(&id))
        {
            continue;
        }
        let Some(page) = controller
            .basics_page
            .and_then(|handle| runtime.root_object(handle))
        else {
            continue;
        };
        if let Some(button) = runtime.store().child_by_name(page, "n0")
            && let Some(target) = runtime.entity_of(button)
        {
            commands.write(GuiObjectCommand::new(
                target,
                GuiObjectAction::SetText(format!(
                    "{}{}",
                    action.caption,
                    if action.checked { " ✓" } else { "" }
                )),
            ));
        }
        controller.last_menu_action = Some(action.clone());
    }
}

/// 推进 Laya CooldownDemo 的两个循环进度条。
pub fn tick(
    runtime: &GuiRuntime,
    controller: &mut DemoController,
    delta: f32,
    commands: &mut MessageWriter<GuiObjectCommand>,
) {
    controller.elapsed += delta;
    let Some(root) = controller.root else { return };
    if controller.package.as_deref() != Some("Cooldown") {
        return;
    }

    let Some(first) = runtime.store().find_by_name(root, "b0") else {
        return;
    };
    let Some(second) = runtime.store().find_by_name(root, "b1") else {
        return;
    };
    let first_value = ((controller.elapsed / 5.0) % 1.0 * 100.0) as i32;
    let second_value = (10.0 - (controller.elapsed / 10.0) % 10.0) as i32;
    for (object, value) in [(first, first_value), (second, second_value)] {
        if let Some(target) = runtime.entity_of(object) {
            commands.write(GuiObjectCommand::new(
                target,
                GuiObjectAction::SetProgress(value),
            ));
        }
    }
}

pub fn copy_basics_input(
    runtime: &GuiRuntime,
    controller: &DemoController,
    commands: &mut MessageWriter<GuiObjectCommand>,
) {
    let Some(page) = controller
        .basics_page
        .and_then(|handle| runtime.root_object(handle))
    else {
        return;
    };
    let Some(input) = runtime.store().find_by_name(page, "n22") else {
        return;
    };
    let value = runtime
        .store()
        .object(input)
        .and_then(|object| object.control().as_text_field())
        .map(|data| data.text.clone())
        .unwrap_or_default();
    if let Some(output) = runtime.store().find_by_name(page, "n24")
        && let Some(target) = runtime.entity_of(output)
    {
        commands.write(GuiObjectCommand::new(
            target,
            GuiObjectAction::SetText(value),
        ));
    }
}

pub fn handle_virtual_list_action(
    runtime: &GuiRuntime,
    controller: &DemoController,
    listener: u64,
    commands: &mut MessageWriter<GuiObjectCommand>,
) {
    let Some(root) = controller.root else {
        return;
    };
    if controller.package.as_deref() != Some("VirtualList") {
        return;
    }
    match listener - DEMO_ACTION_BASE {
        0 => {
            if let Some(list) = runtime.store().find_by_name(root, "mailList")
                && let Some(target) = runtime.entity_of(list)
            {
                commands.write(GuiObjectCommand::new(
                    target,
                    GuiObjectAction::SelectListIndex {
                        index: 500,
                        scroll_to_view: true,
                    },
                ));
            }
        }
        1 | 2 => {
            let Some(list) = runtime.store().find_by_name(root, "mailList") else {
                return;
            };
            let max = runtime
                .store()
                .scroll_pane(list)
                .map(|pane| pane.max_scroll_pos())
                .unwrap_or_default();
            let y = if listener - DEMO_ACTION_BASE == 1 {
                0.0
            } else {
                max.y
            };
            if let Some(target) = runtime.entity_of(list) {
                commands.write(GuiObjectCommand::new(
                    target,
                    GuiObjectAction::SetScrollPosition {
                        position: Vec2::new(max.x, y),
                        allow_bounce: false,
                    },
                ));
            }
        }
        _ => {}
    }
}

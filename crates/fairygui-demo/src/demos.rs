//! Laya 示例的业务控制层。
//!
//! Demo actions observe FairyGUI events on their corresponding Bevy entities,
//! keeping application behavior out of the shared input layer.

use bevy::prelude::*;
use fairygui_bevy::{
    GuiMenuAction, GuiObjectAction, GuiObjectCommand, GuiRuntime, GuiTransitionHook,
};
use fairygui_core::object::GObjectId;

/// Match the official Basics text example: the link replaces its own text.
pub fn handle_links(
    mut links: MessageReader<fairygui_bevy::GuiLink>,
    controller: Res<DemoController>,
    mut runtime: NonSendMut<GuiRuntime>,
) {
    for link in links.read() {
        let Some(root) = controller
            .basics_page
            .and_then(|index| runtime.root_object(index))
        else {
            continue;
        };
        if controller.package.as_deref() == Some("Basics")
            && runtime.store().child_by_name(root, "n12") == Some(link.event.target)
        {
            runtime.store_mut().set_object_text(
                link.event.target,
                &format!(
                    "[img]ui://9leh0eyft9fj5f[/img][color=#FF0000]你点击了链接[/color]：{}",
                    link.href
                ),
            );
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
    pub elapsed: f32,
    pub basics_page: Option<usize>,
    pub transition_sample: Option<usize>,
    pub last_menu_action: Option<GuiMenuAction>,
    numeric_elapsed: Option<f32>,
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
    if package == "Basics"
        && let Some(back) = runtime.store().find_by_name(root, "btn_Back")
    {
        runtime.store_mut().set_visible(back, false);
    }
}

pub fn demo_action_bindings(
    runtime: &GuiRuntime,
    package: &str,
    root: GObjectId,
) -> Vec<(GObjectId, u64)> {
    let mut bindings = Vec::new();
    if package == "Basics" {
        if let Some(back) = runtime.store().find_by_name(root, "btn_Back") {
            bindings.push((back, BASICS_BACK));
        }
        for (index, page) in BASICS_PAGES.iter().enumerate() {
            if let Some(button) = runtime.store().find_by_name(root, &format!("btn_{page}")) {
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
        runtime
            .store()
            .find_by_name(root, name)
            .map(|object| (object, DEMO_ACTION_BASE + index as u64))
    }));
    bindings
}

/// 官方过渡示例的六个播放入口。展示对象作为独立根，结束后回收。
pub fn navigate_transition(
    runtime: &mut GuiRuntime,
    controller: &mut DemoController,
    handle: Handle<fairygui_bevy::FuiAsset>,
    package: &fairygui_core::package::FuiPackage,
    listener: u64,
) -> bool {
    if controller.package.as_deref() != Some("Transition") {
        return false;
    }
    let Some(index) = listener.checked_sub(DEMO_ACTION_BASE).filter(|i| *i < 6) else {
        return false;
    };
    if controller.transition_sample.is_some() {
        return true;
    }
    let Some(root) = controller.root else {
        return true;
    };
    let samples = [
        "BOSS",
        "BOSS_SKILL",
        "TRAP",
        "GoodHit",
        "PowerUp",
        "PathDemo",
    ];
    let Some(sample) = runtime.create_root(handle, package, samples[index as usize]) else {
        return true;
    };
    let object = runtime.root_object(sample).unwrap();
    let owner = runtime.store().object(root).unwrap();
    let origin = Vec2::new(owner.x(), owner.y());
    let size = Vec2::new(owner.width(), owner.height());
    let sample_size = runtime.store().object(object).unwrap().size();
    let position = match index {
        3 => origin + Vec2::new(size.x - sample_size.width - 20.0, 100.0),
        4 => origin + Vec2::new(20.0, size.y - sample_size.height - 100.0),
        _ => origin,
    };
    runtime.store_mut().set_xy(object, position.x, position.y);
    if index == 4 {
        for (name, value) in [("value", "10000"), ("add_value", "+2000")] {
            if let Some(id) = runtime.store().find_by_name(object, name) {
                runtime.store_mut().set_object_text(id, value);
            }
        }
    }
    if let Some(group) = runtime.store().child_by_name(root, "g0") {
        runtime.store_mut().set_visible(group, false);
    }
    runtime
        .store_mut()
        .play_transition(object, "t0", if index == 3 { 3 } else { 1 });
    controller.transition_sample = Some(sample);
    controller.numeric_elapsed = None;
    true
}

/// 处理 PowerUp 的数字钩子，并在有限过渡结束后恢复按钮组。
pub fn advance_transition_demo(
    mut commands: Commands,
    mut runtime: NonSendMut<GuiRuntime>,
    mut controller: ResMut<DemoController>,
    time: Res<Time>,
    mut hooks: MessageReader<GuiTransitionHook>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    let sample = controller
        .transition_sample
        .and_then(|index| runtime.root_object(index));
    for GuiTransitionHook(hook) in hooks.read() {
        if Some(hook.owner) == sample && hook.label == "play_num_now" && !hook.at_end {
            controller.numeric_elapsed = Some(0.0);
        }
    }
    let Some(object) = sample else { return };
    if keys.just_pressed(KeyCode::Space) {
        let paused = runtime
            .store()
            .transition_paused(object, "t0")
            .unwrap_or(false);
        runtime
            .store_mut()
            .set_transition_paused(object, "t0", !paused);
    }
    if keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::ArrowDown) {
        let rate = runtime
            .store()
            .transition_time_scale(object, "t0")
            .unwrap_or(1.0);
        let rate = if keys.just_pressed(KeyCode::ArrowUp) {
            rate * 2.0
        } else {
            rate * 0.5
        };
        runtime
            .store_mut()
            .set_transition_time_scale(object, "t0", rate.clamp(0.25, 4.0));
    }
    if keys.just_pressed(KeyCode::KeyR) {
        runtime
            .store_mut()
            .play_transition_reverse(object, "t0", 0.0);
    }
    if let Some(elapsed) = &mut controller.numeric_elapsed {
        *elapsed = (*elapsed + time.delta_secs()).min(0.3);
        let value = (10000.0 + 2000.0 * *elapsed / 0.3).floor() as i32;
        if let Some(id) = runtime.store().child_by_name(object, "value") {
            runtime.store_mut().set_object_text(id, &value.to_string());
        }
    }
    if runtime
        .store()
        .transition_state(object, "t0")
        .is_some_and(|(playing, _)| !playing)
    {
        let index = controller.transition_sample.take().unwrap();
        runtime.remove_root(&mut commands, index);
        controller.numeric_elapsed = None;
        if let Some(group) = controller
            .root
            .and_then(|id| runtime.store().child_by_name(id, "g0"))
        {
            runtime.store_mut().set_visible(group, true);
        }
    }
}

/// The Basics package contains a menu and separate Demo_* components.
pub fn navigate_basics(
    commands: &mut Commands,
    runtime: &mut GuiRuntime,
    controller: &mut DemoController,
    handle: Handle<fairygui_bevy::FuiAsset>,
    package: &fairygui_core::package::FuiPackage,
    listener: u64,
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
    if let Some(index) = controller.basics_page.take() {
        runtime.remove_root(commands, index);
    }
    runtime
        .store_mut()
        .set_controller_page_index(root, "c1", i32::from(page.is_some()));
    if let Some(page) = page {
        let Some(index) = runtime.create_root(handle, package, &format!("Demo_{page}")) else {
            return true;
        };
        let object = runtime.root_object(index).unwrap();
        let container = runtime
            .store()
            .find_by_name(root, "container")
            .unwrap_or(root);
        // The container's x position is animated by the page controller. Use
        // the settled content origin for this separately-owned demo root.
        let owner = runtime.store().object(root).unwrap();
        let at = Vec2::new(
            owner.x(),
            owner.y() + runtime.store().object(container).map_or(70.0, |o| o.y()),
        );
        runtime.store_mut().set_xy(object, at.x, at.y);
        controller.basics_page = Some(index);
    }
    for name in BASICS_PAGES {
        if let Some(button) = runtime.store().find_by_name(root, &format!("btn_{name}")) {
            runtime.store_mut().set_visible(button, page.is_none());
        }
    }
    if let Some(back) = runtime.store().find_by_name(root, "btn_Back") {
        runtime.store_mut().set_visible(back, page.is_some());
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
            .and_then(|index| runtime.root_object(index))
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
    mut runtime: NonSendMut<GuiRuntime>,
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
            .and_then(|index| runtime.root_object(index))
        else {
            continue;
        };
        if let Some(button) = runtime.store().child_by_name(page, "n0") {
            runtime.store_mut().set_object_text(
                button,
                &format!(
                    "{}{}",
                    action.caption,
                    if action.checked { " ✓" } else { "" }
                ),
            );
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

pub fn copy_basics_input(runtime: &mut GuiRuntime, controller: &DemoController) {
    let Some(page) = controller
        .basics_page
        .and_then(|index| runtime.root_object(index))
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
    if let Some(output) = runtime.store().find_by_name(page, "n24") {
        runtime.store_mut().set_object_text(output, &value);
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

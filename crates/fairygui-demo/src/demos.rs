//! Laya 示例的业务控制层。
//!
//! Demo actions observe FairyGUI events on their corresponding Bevy entities,
//! keeping application behavior out of the shared input layer.

use bevy::prelude::*;
use fairygui_bevy::{
    FairyGuiRoot, GuiMenuAction, GuiMut, GuiObjectMut, GuiPopupCommand, GuiPopupMenu, GuiQuery,
    GuiRootInstance, GuiTransitionComplete, GuiTransitionHook,
};
use fairygui_core::object::GObjectId;

/// Match the official Basics text example: the link replaces its own text.
pub fn handle_links(
    mut links: MessageReader<fairygui_bevy::GuiLink>,
    controller: Res<DemoController>,
    gui: GuiQuery,
    mut objects: GuiMut,
) {
    for link in links.read() {
        let Some(root) = controller.basics_page_owner else {
            continue;
        };
        if controller.package.as_deref() == Some("Basics")
            && gui.child_by_name(root, "n12") == Some(link.entity)
        {
            objects.edit(link.entity, |object: &mut GuiObjectMut| {
                object.set_text(format!(
                    "[img]ui://9leh0eyft9fj5f[/img][color=#FF0000]你点击了链接[/color]：{}",
                    link.href
                ))
            });
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
    mut objects: GuiMut,
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
                objects.edit(entity, |object: &mut GuiObjectMut| {
                    object.set_position(Vec2::new(owner.position().x, y))
                });
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
                objects.edit(entity, |object: &mut GuiObjectMut| {
                    object.set_position(position)
                });
                objects.edit(entity, |object: &mut GuiObjectMut| {
                    object.play_transition("t0", if index == 3 { 3 } else { 1 })
                });
                if index == 4 {
                    for (name, value) in [("value", "10000"), ("add_value", "+2000")] {
                        if let Some(target) = gui.find_by_name(entity, name) {
                            objects
                                .edit(target, |object: &mut GuiObjectMut| object.set_text(value));
                        }
                    }
                }
                if let Some(target) = gui.child_by_name(root, "g0") {
                    objects.edit(target, |object: &mut GuiObjectMut| {
                        object.set_visible(false)
                    });
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

/// Synchronous row measurements and values configured by an ECS component.
pub fn mail_list_config() -> fairygui_bevy::GuiVirtualList {
    fairygui_bevy::GuiVirtualList::new(1000, |index, row| {
        row.set_controller_page("IsRead", i32::from(index % 2 == 0));
        row.set_controller_page("c1", i32::from(index % 3 == 0));
        if let Some(mut time) = row.child_by_path("timeText") {
            time.set_text("5 Nov 2015 16:24:33");
        }
        row.set_text(&format!("{index} Mail title here"));
    })
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
    gui: GuiQuery,
    mut controller: ResMut<DemoController>,
    time: Res<Time>,
    notifications: (
        MessageReader<GuiTransitionHook>,
        MessageReader<GuiTransitionComplete>,
    ),
    mut objects: GuiMut,
    keys: Res<ButtonInput<KeyCode>>,
) {
    let (mut hooks, mut completions) = notifications;
    let sample = controller.transition_sample_owner;
    let view = sample.and_then(|owner| gui.view(owner));
    for notification in hooks.read() {
        let hook = &notification.hook;
        if Some(notification.entity) == view && hook.label == "play_num_now" && !hook.at_end {
            controller.numeric_elapsed = Some(0.0);
        }
    }
    let completed = completions.read().any(|completion| {
        Some(completion.entity) == view
            && completion.transition == "t0"
            && gui
                .transition(completion.entity, "t0")
                .is_some_and(|state| {
                    !state.is_playing() && state.play_generation() == completion.play_generation
                })
    });
    let Some(target) = sample.filter(|_| view.is_some()) else {
        return;
    };
    let stopped = !controller.transition_start_pending
        && gui
            .transition(target, "t0")
            .is_some_and(|state| state.play_generation() > 0 && !state.is_playing());
    if completed || stopped {
        controller.transition_sample = None;
        controller.transition_sample_owner = None;
        commands.entity(target).despawn();
        controller.numeric_elapsed = None;
        if let Some(group) = controller
            .root_owner
            .and_then(|root| gui.child_by_name(root, "g0"))
        {
            objects.edit(group, |object: &mut GuiObjectMut| object.set_visible(true));
        }
        return;
    }
    if controller.transition_start_pending {
        controller.transition_start_pending = false;
        return;
    }
    if keys.just_pressed(KeyCode::Space) {
        controller.transition_paused = !controller.transition_paused;
        objects.edit(target, |object: &mut GuiObjectMut| {
            object.set_transition_paused("t0", controller.transition_paused)
        });
    }
    if keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::ArrowDown) {
        controller.transition_time_scale = if keys.just_pressed(KeyCode::ArrowUp) {
            controller.transition_time_scale * 2.0
        } else {
            controller.transition_time_scale * 0.5
        };
        controller.transition_time_scale = controller.transition_time_scale.clamp(0.25, 4.0);
        objects.edit(target, |object: &mut GuiObjectMut| {
            object.set_transition_time_scale("t0", controller.transition_time_scale)
        });
    }
    if keys.just_pressed(KeyCode::KeyR) {
        objects.edit(target, |object: &mut GuiObjectMut| {
            object.play_transition_reverse("t0", 0.0)
        });
    }
    if let Some(elapsed) = &mut controller.numeric_elapsed {
        *elapsed = (*elapsed + time.delta_secs()).min(0.3);
        let value = (10000.0 + 2000.0 * *elapsed / 0.3).floor() as i32;
        if let Some(target) = gui.child_by_name(target, "value") {
            objects.edit(target, |object: &mut GuiObjectMut| {
                object.set_text(value.to_string())
            });
        }
    }
}

/// The Basics package contains a menu and separate Demo_* components.
pub fn navigate_basics(
    commands: &mut Commands,
    gui: &GuiQuery,
    controller: &mut DemoController,
    handle: Handle<fairygui_bevy::FuiAsset>,
    listener: u64,
    objects: &mut GuiMut,
) -> bool {
    if controller.package.as_deref() != Some("Basics") {
        return false;
    }
    let Some(root) = controller.root_owner else {
        return false;
    };
    let page = listener
        .checked_sub(BASICS_ENTRY)
        .and_then(|i| BASICS_PAGES.get(i as usize));
    if listener != BASICS_BACK && page.is_none() {
        return false;
    }
    commands.write_message(GuiPopupCommand::HideAll);
    controller.last_menu_action = None;
    if let Some(owner) = controller.basics_page_owner.take() {
        commands.entity(owner).despawn();
    }
    controller.basics_page = None;
    objects.edit(root, |object: &mut GuiObjectMut| {
        object.set_controller_page("c1", i32::from(page.is_some()))
    });
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
        if let Some(target) = gui.find_by_name(root, &format!("btn_{name}")) {
            objects.edit(target, |object: &mut GuiObjectMut| {
                object.set_visible(page.is_none())
            });
        }
    }
    if let Some(target) = gui.find_by_name(root, "btn_Back") {
        objects.edit(target, |object: &mut GuiObjectMut| {
            object.set_visible(page.is_some())
        });
    }
    true
}

/// The official Popup page's menu entry, using application actions rather than
/// interpreting the runtime's raw event queue.
pub fn show_basics_menu(
    event: On<fairygui_bevy::GuiEvent>,
    mut commands: Commands,
    controller: Res<DemoController>,
    gui: GuiQuery,
) {
    use fairygui_core::popup::MenuItem;
    if event.event().event.kind != fairygui_core::object::EventKind::Click
        || controller.package.as_deref() != Some("Basics")
    {
        return;
    }
    let Some(page) = controller.basics_page_owner else {
        return;
    };
    let Some(button) = gui.transform(event.entity) else {
        return;
    };
    let mut position = button.position() + Vec2::new(0., button.size().y);
    let mut parent = gui.get(event.entity).and_then(|object| object.parent());
    while let Some(entity) = parent {
        if let Some(transform) = gui.transform(entity) {
            position += transform.position();
        }
        parent = gui.get(entity).and_then(|object| object.parent());
    }
    let items = vec![
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
    commands.write_message(GuiPopupCommand::ShowMenu {
        source: page,
        menu: GuiPopupMenu::new("PopupMenu", "PopupMenuItem", items),
        position,
    });
}

pub fn handle_menu_actions(
    mut actions: MessageReader<GuiMenuAction>,
    mut controller: ResMut<DemoController>,
    gui: GuiQuery,
    mut objects: GuiMut,
) {
    for action in actions.read() {
        if controller.package.as_deref() != Some("Basics")
            || !action
                .action
                .is_some_and(|id| (BASICS_MENU_ACTION..BASICS_MENU_ACTION + 4).contains(&id))
        {
            continue;
        }
        let Some(page) = controller.basics_page_owner else {
            continue;
        };
        if let Some(target) = gui.child_by_name(page, "n0") {
            objects.edit(target, |object: &mut GuiObjectMut| {
                object.set_text(format!(
                    "{}{}",
                    action.caption,
                    if action.checked { " ✓" } else { "" }
                ))
            });
        }
        controller.last_menu_action = Some(action.clone());
    }
}

/// 推进 Laya CooldownDemo 的两个循环进度条。
pub fn tick(gui: &GuiQuery, controller: &mut DemoController, delta: f32, objects: &mut GuiMut) {
    controller.elapsed += delta;
    let Some(root) = controller.root_owner else {
        return;
    };
    if controller.package.as_deref() != Some("Cooldown") {
        return;
    }

    let Some(first) = gui.find_by_name(root, "b0") else {
        return;
    };
    let Some(second) = gui.find_by_name(root, "b1") else {
        return;
    };
    let first_value = f64::from((controller.elapsed / 5.0) % 1.0 * 100.0);
    let second_value = f64::from(10.0 - (controller.elapsed / 10.0) % 10.0);
    for (target, value) in [(first, first_value), (second, second_value)] {
        objects.edit(target, |object: &mut GuiObjectMut| {
            object.set_progress(value)
        });
    }
}

pub fn copy_basics_input(gui: &GuiQuery, controller: &DemoController, objects: &mut GuiMut) {
    let Some(page) = controller.basics_page_owner else {
        return;
    };
    let Some(input) = gui.find_by_name(page, "n22") else {
        return;
    };
    let value = gui.text(input).unwrap_or_default();
    if let Some(target) = gui.find_by_name(page, "n24") {
        objects.edit(target, |object: &mut GuiObjectMut| {
            object.set_text(value.to_owned())
        });
    }
}

pub fn handle_virtual_list_action(
    gui: &GuiQuery,
    controller: &DemoController,
    listener: u64,
    objects: &mut GuiMut,
) {
    let Some(root) = controller.root_owner else {
        return;
    };
    if controller.package.as_deref() != Some("VirtualList") {
        return;
    }
    let Some(target) = gui.find_by_name(root, "mailList") else {
        return;
    };
    match listener.checked_sub(DEMO_ACTION_BASE) {
        Some(0) => {
            objects.edit(target, |object: &mut GuiObjectMut| {
                object.select_list_index(500, true)
            });
        }
        Some(index @ (1 | 2)) => {
            objects.edit(target, |object: &mut GuiObjectMut| {
                object.set_scroll_percentage(
                    Vec2::new(0.0, if index == 1 { 0.0 } else { 1.0 }),
                    false,
                )
            });
        }
        _ => {}
    }
}

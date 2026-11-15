//! ModalWaiting's retained window and simulated asynchronous request use ECS.
use crate::ui::modal_waiting::{self as ui, GlobalModalWaiting, TestWin, WindowModalWaiting};
use bevy::prelude::*;
use fairygui_bevy::prelude::*;

#[derive(Component)]
pub(crate) struct PageBound;
#[derive(Component)]
pub(crate) struct DemoWindow;
#[derive(Component)]
pub(crate) struct WindowBound;
#[derive(Component)]
struct Show(Entity);
#[derive(Component)]
struct Start(Entity);
#[derive(Component)]
pub(crate) struct WaitJob(Timer);
#[derive(Resource, Default)]
pub(crate) struct GlobalDemo {
    page: Option<Entity>,
    timer: Option<Timer>,
}

pub fn bind(
    mut commands: Commands,
    pages: Query<(Entity, &FairyGuiRoot), Without<PageBound>>,
    windows: Query<Entity, (With<DemoWindow>, Without<WindowBound>)>,
    gui: GuiQuery,
    mut global: ResMut<GlobalDemo>,
) {
    for (page, source) in &pages {
        if source.item_name != ui::Main::ITEM_NAME {
            continue;
        }
        let Ok(binding) = ui::Main::bind(&gui, page) else {
            continue;
        };
        let mut window = FairyGuiWindow::new(source.asset.clone(), TestWin::ITEM_NAME);
        window.visible = false;
        window.modal_wait = Some(WindowModalWaiting::ITEM_NAME.into());
        let owner = commands.spawn((window, DemoWindow, ChildOf(page))).id();
        commands
            .entity(binding.n0.entity())
            .insert(Show(owner))
            .observe(show);
        commands.entity(page).insert(PageBound);
        let mut settings =
            GuiGlobalWaitSettings::new(source.asset.clone(), GlobalModalWaiting::ITEM_NAME);
        settings.visible = true;
        commands.insert_resource(settings);
        global.page = Some(page);
        global.timer = Some(Timer::from_seconds(3., TimerMode::Once));
    }
    for owner in &windows {
        let Ok(binding) = TestWin::bind(&gui, owner) else {
            continue;
        };
        commands
            .entity(binding.n1.entity())
            .insert(Start(owner))
            .observe(start);
        commands.entity(owner).insert(WindowBound);
    }
}
fn show(
    event: On<GuiClick>,
    buttons: Query<&Show>,
    mut windows: Query<&mut FairyGuiWindow>,
    mut gui: GuiMut,
) {
    if let Ok(button) = buttons.get(event.entity) {
        if let Ok(mut window) = windows.get_mut(button.0) {
            window.visible = true;
        }
        if let Some(mut window) = gui.get_mut(button.0) {
            window.bring_window_to_front();
        }
    }
}
fn start(
    event: On<GuiClick>,
    buttons: Query<&Start>,
    mut commands: Commands,
    mut windows: Query<&mut FairyGuiWindow>,
) {
    if let Ok(button) = buttons.get(event.entity) {
        if let Ok(mut window) = windows.get_mut(button.0) {
            window.modal_wait_request = Some(1);
        }
        commands
            .entity(button.0)
            .insert(WaitJob(Timer::from_seconds(3., TimerMode::Once)));
    }
}
pub fn advance(
    mut commands: Commands,
    mut jobs: Query<(Entity, &mut WaitJob)>,
    mut windows: Query<&mut FairyGuiWindow>,
    time: Res<Time>,
    mut global: ResMut<GlobalDemo>,
    mut settings: Option<ResMut<GuiGlobalWaitSettings>>,
    existing: Query<()>,
) {
    if global.page.is_some_and(|page| !existing.contains(page)) {
        if let Some(settings) = settings.as_deref_mut() {
            settings.visible = false;
        }
        global.page = None;
        global.timer = None;
    } else if global
        .timer
        .as_mut()
        .is_some_and(|timer| timer.tick(time.delta()).just_finished())
    {
        if let Some(settings) = settings.as_deref_mut() {
            settings.visible = false;
        }
        global.timer = None;
    }
    for (owner, mut job) in &mut jobs {
        if job.0.tick(time.delta()).just_finished() {
            if let Ok(mut window) = windows.get_mut(owner) {
                window.close_modal_wait(1);
            }
            commands.entity(owner).remove::<WaitJob>();
        }
    }
}

//! ModalWaiting's retained window and simulated asynchronous request use ECS.
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

pub fn bind(
    mut commands: Commands,
    pages: Query<(Entity, &FairyGuiRoot), Without<PageBound>>,
    windows: Query<Entity, (With<DemoWindow>, Without<WindowBound>)>,
    assets: Res<Assets<FuiAsset>>,
    gui: GuiQuery,
) {
    for (page, source) in &pages {
        if source.item_name != "Main"
            || assets
                .get(&source.asset)
                .is_none_or(|asset| asset.package.name() != "ModalWaiting")
        {
            continue;
        }
        let Some(button) = gui.child_by_name(page, "n0") else {
            continue;
        };
        let mut window = FairyGuiWindow::new(source.asset.clone(), "TestWin");
        window.visible = false;
        window.modal_wait = Some("WindowModalWaiting".into());
        let owner = commands.spawn((window, DemoWindow, ChildOf(page))).id();
        commands.entity(button).insert(Show(owner)).observe(show);
        commands.entity(page).insert(PageBound);
    }
    for owner in &windows {
        let Some(button) = gui.child_by_name(owner, "n1") else {
            continue;
        };
        commands.entity(button).insert(Start(owner)).observe(start);
        commands.entity(owner).insert(WindowBound);
    }
}
fn show(event: On<GuiEvent>, buttons: Query<&Show>, mut commands: Commands) {
    if event.event().event.kind == fairygui_bevy::core::object::EventKind::Click
        && let Ok(button) = buttons.get(event.entity)
    {
        commands.write_message(GuiWindowCommand::new(button.0, GuiWindowAction::Show));
        commands.write_message(GuiWindowCommand::new(
            button.0,
            GuiWindowAction::BringToFront,
        ));
    }
}
fn start(event: On<GuiEvent>, buttons: Query<&Start>, mut commands: Commands) {
    if event.event().event.kind == fairygui_bevy::core::object::EventKind::Click
        && let Ok(button) = buttons.get(event.entity)
    {
        commands.write_message(GuiWindowCommand::new(
            button.0,
            GuiWindowAction::ShowModalWait { request: 1 },
        ));
        commands
            .entity(button.0)
            .insert(WaitJob(Timer::from_seconds(3., TimerMode::Once)));
    }
}
pub fn advance(mut commands: Commands, mut jobs: Query<(Entity, &mut WaitJob)>, time: Res<Time>) {
    for (owner, mut job) in &mut jobs {
        if job.0.tick(time.delta()).just_finished() {
            commands.write_message(GuiWindowCommand::new(
                owner,
                GuiWindowAction::CloseModalWait { request: 1 },
            ));
            commands.entity(owner).remove::<WaitJob>();
        }
    }
}

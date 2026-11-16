//! An ECS-owned root stays invisible until all exported objects are constructed.
use super::*;

#[derive(Resource, Default)]
struct State {
    owner: Option<Entity>,
    progress: usize,
    frame: u32,
    ready: Option<u32>,
    view: Option<Entity>,
    initialized: usize,
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<State>()
        .add_systems(PreUpdate, drive)
        .add_systems(Update, initialize.in_set(GuiSystems::Initialize));
}

fn initialize(
    roots: Query<(Entity, &GuiRootView), Added<GuiRootView>>,
    gui: GuiQuery,
    mut state: ResMut<State>,
) {
    for (owner, view) in &roots {
        if state.owner != Some(owner) {
            continue;
        }
        assert_eq!(
            gui.transform(view.0).unwrap().position(),
            Vec2::new(32., 24.)
        );
        assert!(gui.child_by_name(owner, "n1").is_some());
        state.initialized += 1;
    }
}

#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    demo: Res<Demo>,
    gui: GuiQuery,
    progress: Query<&GuiRootBuildProgress>,
    mounts: Query<&GuiRootMountState>,
    mut roots: Query<&mut FairyGuiRoot>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    assert!(state.frame < 1200, "asynchronous root smoke did not finish");
    let Some(owner) = state.owner else {
        let Some(menu) = demo.menu_owner.filter(|&owner| gui.view(owner).is_some()) else {
            return;
        };
        let mut display = *gui.display(menu).unwrap();
        display.visible = false;
        commands.entity(menu).insert(display);
        state.owner = Some(
            commands
                .spawn(
                    FairyGuiRoot::new(demo.menu_asset.as_ref().unwrap().clone(), "Main")
                        .with_build_budget(1)
                        .with_position(Vec2::new(64., 48.)),
                )
                .id(),
        );
        return;
    };
    let Some(ready) = state.ready else {
        let Ok(progress) = progress.get(owner) else {
            return;
        };
        assert_eq!(progress.constructed, state.progress + 1);
        state.progress = progress.constructed;
        if progress.constructed < progress.total {
            assert!(gui.view(owner).is_none());
            assert_eq!(mounts.get(owner).unwrap(), &GuiRootMountState::Building);
            if progress.constructed == 3 {
                roots.get_mut(owner).unwrap().position = Some(Vec2::new(32., 24.));
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk("target/fairygui-demo-async-root-building.png"));
            }
            return;
        }
        state.ready = Some(state.frame);
        state.view = gui.view(owner);
        assert!(state.view.is_some());
        assert_eq!(state.initialized, 1);
        return;
    };
    if state.frame < ready + 40 {
        assert_eq!(gui.view(owner), state.view);
        assert_eq!(state.initialized, 1);
        assert_eq!(progress.get(owner).unwrap().constructed, state.progress);
    }
    if state.frame == ready + 1 || state.frame == ready + 25 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(if state.frame == ready + 1 {
                "target/fairygui-demo-async-root-ready.png"
            } else {
                "target/fairygui-demo-async-root-retained.png"
            }));
    }
    if state.frame == ready + 10 {
        roots.get_mut(owner).unwrap().build_budget = Some(64);
    }
    if state.frame == ready + 40 {
        commands.entity(owner).despawn();
    }
    if state.frame == ready + 45 {
        assert!(gui.view(owner).is_none());
        info!(
            "demo smoke PASSED: per-object ECS root construction, atomic visibility, initialization and retention"
        );
        exit.write(AppExit::Success);
    }
}

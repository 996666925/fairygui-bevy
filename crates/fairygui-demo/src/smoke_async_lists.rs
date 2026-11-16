//! Compare synchronous and budgeted nested-list materialization in a window.
use super::*;

#[derive(Resource, Default)]
struct State {
    frame: u32,
    baseline: Option<Entity>,
    owner: Option<Entity>,
    asset: Option<Handle<FuiAsset>>,
    ready: Option<u32>,
    progress: usize,
    initialized: usize,
    captured_pending: bool,
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<State>()
        .add_systems(PreUpdate, drive)
        .add_systems(Update, initialize.in_set(GuiSystems::Initialize));
}

fn initialize(
    owners: Query<(Entity, &GuiRootInstance), Added<GuiRootInstance>>,
    runtime: NonSend<GuiRuntime>,
    mut state: ResMut<State>,
) {
    for (owner, root) in &owners {
        if state.baseline != Some(owner) && state.owner != Some(owner) {
            continue;
        }
        let store = runtime.store();
        let outer = store.child_by_name(root.object, "list1").unwrap();
        assert_eq!(store.list_item_count(outer), 4);
        for row in store.list_items(outer).unwrap() {
            let inner = store.child_by_name(row, "list").unwrap();
            assert_eq!(store.list_item_count(inner), 2);
            for item in store.list_items(inner).unwrap() {
                assert!(runtime.entity_of(item).is_some());
                assert!(!store.is_constructing(item));
            }
        }
        state.initialized += 1;
    }
}

#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    demo: Res<Demo>,
    gui: GuiQuery,
    progress: Query<&GuiRootBuildProgress>,
    mut assets: ResMut<Assets<FuiAsset>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    assert!(
        state.frame < 1800,
        "asynchronous nested-list smoke timed out"
    );
    if state.baseline.is_none() {
        let Some(menu) = demo.menu_owner.filter(|&owner| gui.view(owner).is_some()) else {
            return;
        };
        let mut display = *gui.display(menu).unwrap();
        display.visible = false;
        commands.entity(menu).insert(display);
        let asset = assets.add(FuiAsset {
            package: fairygui_bevy::core::FuiPackage::parse(
                "Basics",
                include_bytes!("../../fairygui-core/tests/fixtures/AsyncLists.fui"),
            )
            .unwrap(),
            res_key: "ui/Basics".into(),
        });
        state.baseline = Some(
            commands
                .spawn(
                    FairyGuiRoot::new(asset.clone(), "Demo_Grid")
                        .with_position(Vec2::new(32., 24.)),
                )
                .id(),
        );
        state.asset = Some(asset);
        state.frame = 0;
        return;
    }
    if state.frame == 100 {
        assert_eq!(state.initialized, 1);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(
                "target/fairygui-demo-async-lists-baseline.png",
            ));
    }
    if state.frame == 110 {
        commands.entity(state.baseline.unwrap()).despawn();
        state.owner = Some(
            commands
                .spawn(
                    FairyGuiRoot::new(state.asset.as_ref().unwrap().clone(), "Demo_Grid")
                        .with_position(Vec2::new(32., 24.))
                        .with_build_budget(2),
                )
                .id(),
        );
    }
    let Some(owner) = state.owner else {
        return;
    };
    let Some(ready) = state.ready else {
        let Ok(progress) = progress.get(owner) else {
            return;
        };
        assert!(
            progress.constructed > state.progress && progress.constructed - state.progress <= 2
        );
        state.progress = progress.constructed;
        if progress.constructed < progress.total {
            assert!(gui.view(owner).is_none());
            if !state.captured_pending {
                state.captured_pending = true;
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(
                        "target/fairygui-demo-async-lists-building.png",
                    ));
            }
        } else {
            assert!(gui.view(owner).is_some());
            assert_eq!(state.initialized, 2);
            state.ready = Some(state.frame);
        }
        return;
    };
    if state.frame == ready + 100 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk("target/fairygui-demo-async-lists-ready.png"));
    }
    if state.frame == ready + 110 {
        commands.entity(owner).despawn();
    }
    if state.frame == ready + 115 {
        assert!(gui.view(owner).is_none());
        assert!(state.captured_pending);
        info!(
            "demo smoke PASSED: nested lists, prepared row reuse, incremental construction and complete first initialization"
        );
        exit.write(AppExit::Success);
    }
}

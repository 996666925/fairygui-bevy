use super::*;
use crate::GuiRootMountState;

fn object_ids(app: &App) -> Vec<GObjectId> {
    app.world()
        .non_send::<GuiRuntime>()
        .store()
        .iter()
        .map(|(id, _)| id)
        .collect()
}

#[test]
fn root_waits_for_assets_builds_with_an_object_budget_and_publishes_one_complete_view() {
    #[derive(Resource, Default)]
    struct Initialized(usize);
    fn initialize(
        roots: Query<&GuiRootView, Added<GuiRootView>>,
        gui: GuiQuery,
        mut count: ResMut<Initialized>,
    ) {
        for view in &roots {
            assert_eq!(
                gui.transform(view.0).unwrap().position(),
                Vec2::new(31., 47.)
            );
            assert_eq!(gui.transform(view.0).unwrap().size(), Vec2::new(700., 500.));
            assert!(gui.child_by_name(view.0, "n1").is_some());
            count.0 += 1;
        }
    }
    let (mut app, asset) = app();
    app.init_resource::<Initialized>()
        .add_systems(Update, initialize.in_set(crate::GuiSystems::Initialize));
    let window = app
        .world_mut()
        .spawn((
            Window {
                resolution: (800, 600).into(),
                ..default()
            },
            bevy::window::PrimaryWindow,
        ))
        .id();
    let owner = app
        .world_mut()
        .spawn(
            FairyGuiRoot::new(asset.clone(), "Main")
                .with_build_budget(1)
                .with_window_size(),
        )
        .id();
    app.update();
    assert_eq!(
        app.world().get::<GuiRootMountState>(owner),
        Some(&GuiRootMountState::WaitingForAsset)
    );
    assert!(object_ids(&app).is_empty());
    assert!(app.world().get::<GuiRootBuildProgress>(owner).is_none());
    load(&mut app, &asset);
    app.update();
    let first = *app.world().get::<GuiRootBuildProgress>(owner).unwrap();
    assert_eq!(first.constructed, 1);
    assert!(first.total > 10);
    assert_eq!(
        app.world().get::<GuiRootMountState>(owner),
        Some(&GuiRootMountState::Building)
    );
    app.world_mut()
        .get_mut::<FairyGuiRoot>(owner)
        .unwrap()
        .position = Some(Vec2::new(31., 47.));
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .resolution
        .set(700., 500.);
    for expected in 2..=first.total {
        app.update();
        let progress = app.world().get::<GuiRootBuildProgress>(owner).unwrap();
        assert_eq!(progress.constructed, expected);
        assert_eq!(progress.total, first.total);
        if expected < first.total {
            assert!(app.world().get::<GuiRootInstance>(owner).is_none());
            assert!(
                app.world_mut()
                    .query::<&GObjectRef>()
                    .iter(app.world())
                    .next()
                    .is_none()
            );
            let runtime = app.world().non_send::<GuiRuntime>();
            assert!(
                runtime
                    .store()
                    .iter()
                    .all(|(id, _)| !runtime.store().is_on_stage(id))
            );
        }
    }
    assert_eq!(app.world().resource::<Initialized>().0, 1);
    let root = *app.world().get::<GuiRootInstance>(owner).unwrap();
    let view = *app.world().get::<GuiRootView>(owner).unwrap();
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(app.world().resource::<Initialized>().0, 1);
    assert_eq!(app.world().get::<GuiRootInstance>(owner), Some(&root));
    assert_eq!(app.world().get::<GuiRootView>(owner), Some(&view));
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .is_on_stage(root.object)
    );
}

#[test]
fn cancelling_or_replacing_pending_roots_reclaims_every_prepared_object() {
    for operation in 0..4 {
        let (mut app, asset) = app();
        load(&mut app, &asset);
        let owner = app
            .world_mut()
            .spawn(FairyGuiRoot::new(asset.clone(), "Main").with_build_budget(3))
            .id();
        app.update();
        let previous = object_ids(&app);
        assert_eq!(previous.len(), 3);
        match operation {
            0 => {
                app.world_mut().entity_mut(owner).remove::<FairyGuiRoot>();
            }
            1 => {
                app.world_mut().entity_mut(owner).despawn();
            }
            2 => {
                app.world_mut()
                    .get_mut::<FairyGuiRoot>(owner)
                    .unwrap()
                    .item_name = "missing".into();
            }
            _ => {
                app.world_mut().entity_mut(owner).remove::<FairyGuiRoot>();
                app.world_mut()
                    .entity_mut(owner)
                    .insert(FairyGuiRoot::new(asset, "Main").with_build_budget(1));
            }
        }
        app.update();
        let runtime = app.world().non_send::<GuiRuntime>();
        assert!(previous.iter().all(|&id| !runtime.store().contains(id)));
        assert!(
            runtime
                .store()
                .iter()
                .all(|(id, _)| runtime.entity_of(id).is_none())
        );
        assert_eq!(runtime.store().len(), usize::from(operation == 3));
        if operation == 2 {
            assert_eq!(
                app.world().get::<GuiRootMountState>(owner),
                Some(&GuiRootMountState::CreationFailed)
            );
        }
        if operation == 0 {
            assert!(app.world().get::<GuiRootBuildProgress>(owner).is_none());
        }
    }
}

#[test]
fn changing_the_budget_keeps_prepared_objects_and_inactive_roots_remain_off_stage() {
    let (mut app, asset) = app();
    load(&mut app, &asset);
    let mut source = FairyGuiRoot::new(asset, "Main").with_build_budget(0);
    source.active = false;
    let owner = app.world_mut().spawn(source).id();
    app.update();
    let previous = object_ids(&app);
    assert_eq!(previous.len(), 1);
    app.world_mut()
        .get_mut::<FairyGuiRoot>(owner)
        .unwrap()
        .build_budget = Some(4);
    app.update();
    assert_eq!(
        app.world()
            .get::<GuiRootBuildProgress>(owner)
            .unwrap()
            .constructed,
        5
    );
    assert!(
        previous
            .iter()
            .all(|&id| app.world().non_send::<GuiRuntime>().store().contains(id))
    );
    app.world_mut()
        .get_mut::<FairyGuiRoot>(owner)
        .unwrap()
        .build_budget = None;
    app.update();
    let root = *app.world().get::<GuiRootInstance>(owner).unwrap();
    let runtime = app.world().non_send::<GuiRuntime>();
    assert!(!runtime.store().is_on_stage(root.object));
    assert!(previous.iter().all(|&id| runtime.store().contains(id)));
    app.world_mut()
        .get_mut::<FairyGuiRoot>(owner)
        .unwrap()
        .active = true;
    app.update();
    assert_eq!(app.world().get::<GuiRootInstance>(owner), Some(&root));
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .is_on_stage(root.object)
    );
}

#[test]
fn a_completed_child_waits_for_its_incrementally_built_parent_without_becoming_a_separate_window() {
    let (mut app, asset) = app();
    load(&mut app, &asset);
    let parent = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), "Main").with_build_budget(1))
        .id();
    let child = app
        .world_mut()
        .spawn(
            FairyGuiRoot::new(asset, "Main")
                .with_build_budget(usize::MAX)
                .with_parent(parent),
        )
        .id();
    app.update();
    let object = app.world().get::<GuiRootInstance>(child).unwrap().object;
    assert_eq!(
        app.world().get::<GuiRootMountState>(child),
        Some(&GuiRootMountState::WaitingForParent)
    );
    assert!(app.world().get::<GuiRootView>(child).is_none());
    assert!(
        !app.world()
            .non_send::<GuiRuntime>()
            .store()
            .is_on_stage(object)
    );
    let remaining = app
        .world()
        .get::<GuiRootBuildProgress>(parent)
        .unwrap()
        .total;
    for _ in 0..remaining {
        app.update();
    }
    let parent_object = app.world().get::<GuiRootInstance>(parent).unwrap().object;
    let runtime = app.world().non_send::<GuiRuntime>();
    assert_eq!(
        runtime.store().object(object).unwrap().parent(),
        Some(parent_object)
    );
    assert!(runtime.store().is_on_stage(object));
    let child_view = app.world().get::<GuiRootView>(child).unwrap().0;
    assert_eq!(
        app.world().get::<ChildOf>(child_view).unwrap().parent(),
        app.world().get::<GuiRootView>(parent).unwrap().0
    );
}

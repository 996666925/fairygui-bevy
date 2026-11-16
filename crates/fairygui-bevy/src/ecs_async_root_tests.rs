use super::*;
use crate::GuiRootMountState;
use bevy::ecs::system::RunSystemOnce;

fn package_app(bytes: &[u8], resource: &str) -> (App, Handle<FuiAsset>) {
    let (mut app, handle) = app();
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(
            handle.id(),
            FuiAsset {
                package: FuiPackage::parse(resource, bytes).unwrap(),
                res_key: format!("ui/{resource}"),
            },
        )
        .unwrap();
    (app, handle)
}

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

#[test]
fn list_and_tree_rows_are_built_under_the_root_budget_and_ready_during_initialize() {
    #[derive(Resource, Default)]
    struct Seen(usize);
    fn initialize(
        roots: Query<&GuiRootView, Added<GuiRootView>>,
        references: Query<&GObjectRef>,
        mut seen: ResMut<Seen>,
        runtime: NonSend<GuiRuntime>,
    ) {
        for view in &roots {
            let root = references.get(view.0).unwrap().0;
            let mut objects = vec![root];
            runtime
                .store()
                .for_each_descendant(root, &mut |id| objects.push(id));
            for id in objects {
                let Some(data) = runtime.store().object(id).and_then(|o| match o.control() {
                    ControlData::List(data) => Some(data.as_ref()),
                    ControlData::Tree(data) => Some(&data.list),
                    _ => None,
                }) else {
                    continue;
                };
                if !data.items.is_empty() {
                    assert_eq!(runtime.store().list_item_count(id), data.items.len());
                    for row in runtime.store().list_items(id).unwrap() {
                        assert!(runtime.entity_of(row).is_some());
                        assert!(!runtime.store().is_constructing(row));
                    }
                    seen.0 += 1;
                }
            }
        }
    }
    for (bytes, package_name, item) in [
        (
            include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui").as_slice(),
            "Basics",
            "Demo_List",
        ),
        (
            include_bytes!("../../fairygui-core/tests/fixtures/AsyncLists.fui").as_slice(),
            "Basics",
            "Demo_Grid",
        ),
        (
            include_bytes!("../../fairygui-demo/assets/ui/TreeView.fui").as_slice(),
            "TreeView",
            "Main",
        ),
    ] {
        let (mut app, asset) = package_app(bytes, package_name);
        app.init_resource::<Seen>()
            .add_systems(Update, initialize.in_set(crate::GuiSystems::Initialize));
        let owner = app
            .world_mut()
            .spawn(FairyGuiRoot::new(asset, item).with_build_budget(1))
            .id();
        let mut completed = false;
        let mut before = Vec::new();
        for _ in 0..2500 {
            app.update();
            let progress = app.world().get::<GuiRootBuildProgress>(owner).unwrap();
            assert!(progress.total > 10);
            let runtime = app.world().non_send::<GuiRuntime>();
            if progress.constructed < progress.total {
                assert_eq!(
                    runtime.store().len(),
                    progress.constructed,
                    "{package_name}/{item}"
                );
                assert!(app.world().get::<GuiRootInstance>(owner).is_none());
                assert!(
                    runtime
                        .store()
                        .iter()
                        .all(|(id, _)| runtime.entity_of(id).is_none())
                );
                before = object_ids(&app);
            } else {
                assert!(
                    before.iter().all(|&id| runtime.store().contains(id)),
                    "prepared objects must be reused"
                );
                assert!(app.world().resource::<Seen>().0 > 0);
                assert!(
                    runtime
                        .store()
                        .iter()
                        .all(|(id, _)| !runtime.store().is_constructing(id))
                );
                completed = true;
                break;
            }
        }
        assert!(completed);
        app.world_mut().entity_mut(owner).despawn();
        app.update();
        assert!(object_ids(&app).is_empty());
    }
}

#[test]
fn unpublished_loader_content_and_error_signs_cannot_allocate_outside_the_build_budget() {
    let (mut app, asset) = package_app(
        include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui"),
        "Basics",
    );
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "Demo_List").with_build_budget(1))
        .id();
    let loader = (0..1000)
        .find_map(|_| {
            app.update();
            let runtime = app.world().non_send::<GuiRuntime>();
            runtime.store().iter().find_map(|(id, object)| {
                (runtime.store().is_constructing(id)
                    && object.kind() == fairygui_core::package::ObjectType::Loader)
                    .then_some(id)
            })
        })
        .unwrap();
    let before = object_ids(&app);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .apply_icon_to(loader, "ui://Basics/Component12");
    app.world_mut()
        .run_system_once(sync_loader_components)
        .unwrap();
    assert_eq!(object_ids(&app), before);
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .loader_component(loader)
            .is_none()
    );
    {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        runtime
            .store_mut()
            .apply_icon_to(loader, "ui://Basics/missing");
        runtime.set_loader_error_sign(Some("ui://Basics/Component12"));
    }
    app.world_mut().run_system_once(sync_loader_status).unwrap();
    assert_eq!(object_ids(&app), before);
    assert!(
        !app.world()
            .non_send::<GuiRuntime>()
            .loader_status
            .contains_key(&loader)
    );
    app.world_mut().entity_mut(owner).despawn();
    app.update();
    assert!(object_ids(&app).is_empty());
}

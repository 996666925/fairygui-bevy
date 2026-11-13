use super::*;
use crate::prelude::*;
use bevy::ecs::system::SystemState;

fn fixture() -> (App, Handle<FuiAsset>, Handle<FuiAsset>) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let ready = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: FuiPackage::parse(
                "Basics",
                include_bytes!("../../fairygui-demo/assets/ui/Basics.fui"),
            )
            .unwrap(),
            res_key: "ui/Basics".into(),
        });
    let pending = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
    (app, ready, pending)
}
fn load_parent(app: &mut App, handle: &Handle<FuiAsset>) {
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(
            handle.id(),
            FuiAsset {
                package: FuiPackage::parse(
                    "MainMenu",
                    include_bytes!("../../fairygui-demo/assets/ui/MainMenu.fui"),
                )
                .unwrap(),
                res_key: "ui/MainMenu".into(),
            },
        )
        .unwrap();
}
fn view(app: &App, owner: Entity) -> Entity {
    app.world().get::<GuiRootView>(owner).unwrap().0
}
fn object(app: &App, owner: Entity) -> GObjectId {
    app.world().get::<GuiRootInstance>(owner).unwrap().object
}
fn mount(app: &App, owner: Entity) -> GuiRootMountState {
    *app.world().get::<GuiRootMountState>(owner).unwrap()
}
fn child(app: &mut App, owner: Entity, name: &str) -> Entity {
    SystemState::<GuiQuery>::new(app.world_mut())
        .get(app.world())
        .unwrap()
        .child_by_name(owner, name)
        .unwrap()
}

#[test]
fn pending_parent_keeps_a_ready_child_off_stage_then_mounts_before_its_first_initialize() {
    #[derive(Component)]
    struct Nested;
    #[derive(Resource, Default)]
    struct Initial(bool);
    #[allow(clippy::type_complexity)]
    fn inspect(
        roots: Query<(Entity, &FairyGuiRoot), (With<Nested>, Added<GuiRootView>)>,
        gui: GuiQuery,
        mut initial: ResMut<Initial>,
    ) {
        for (owner, source) in &roots {
            let parent = gui.view(source.parent.unwrap()).unwrap();
            assert_eq!(gui.get(owner).unwrap().parent(), Some(parent));
            assert!(gui.display(owner).unwrap().on_stage());
            assert_eq!(
                gui.transform(owner).unwrap().position(),
                Vec2::new(17., 23.)
            );
            assert_eq!(gui.transform(owner).unwrap().size(), Vec2::new(200., 30.));
            assert_eq!(gui.slider(owner).unwrap().value(), 0.25);
            assert_eq!(
                gui.root_mount(owner),
                Some(&GuiRootMountState::Mounted {
                    parent: source.parent
                })
            );
            initial.0 = true;
        }
    }
    let (mut app, ready, pending) = fixture();
    app.init_resource::<Initial>()
        .add_systems(Update, inspect.in_set(GuiSystems::Initialize));
    let parent = app
        .world_mut()
        .spawn(FairyGuiRoot::new(pending.clone(), "Main").with_position(Vec2::new(50., 40.)))
        .id();
    let owner = app
        .world_mut()
        .spawn((
            FairyGuiRoot::new(ready, "Slider_HZ")
                .with_parent(parent)
                .with_position(Vec2::new(17., 23.))
                .with_size(Vec2::new(200., 30.)),
            Nested,
        ))
        .id();
    app.world_mut()
        .entity_mut(owner)
        .insert(crate::GuiSliderState::new(0., 1., 0.25));
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(mount(&app, owner), GuiRootMountState::WaitingForParent);
    assert!(app.world().get::<GuiRootView>(owner).is_none());
    assert!(
        !app.world()
            .non_send::<GuiRuntime>()
            .store()
            .is_on_stage(object(&app, owner))
    );
    load_parent(&mut app, &pending);
    app.update();
    assert!(app.world().resource::<Initial>().0);
    let nested = view(&app, owner);
    assert_eq!(
        app.world().get::<ChildOf>(nested).unwrap().parent(),
        view(&app, parent)
    );
    let mut gui = SystemState::<GuiQuery>::new(app.world_mut());
    assert_eq!(
        gui.get(app.world())
            .unwrap()
            .local_to_global(owner, Vec2::ZERO),
        Some(Vec2::new(67., 63.))
    );
    let tick = app
        .world()
        .entity(owner)
        .get_ref::<GuiRootMountState>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(view(&app, owner), nested);
    assert_eq!(
        app.world()
            .entity(owner)
            .get_ref::<GuiRootMountState>()
            .unwrap()
            .last_changed(),
        tick
    );
}

#[test]
fn owners_and_control_views_support_retained_moves_detach_restore_and_top_level_promotion() {
    let (mut app, ready, _) = fixture();
    let a = app
        .world_mut()
        .spawn(FairyGuiRoot::new(ready.clone(), "Demo_Button"))
        .id();
    let b = app
        .world_mut()
        .spawn(FairyGuiRoot::new(ready.clone(), "Demo_Button").with_position(Vec2::new(500., 0.)))
        .id();
    app.update();
    let container = view(&app, b);
    let owner = app
        .world_mut()
        .spawn(
            FairyGuiRoot::new(ready, "Slider_HZ")
                .with_parent(a)
                .with_position(Vec2::new(10., 20.)),
        )
        .id();
    app.update();
    let nested = view(&app, owner);
    let native = object(&app, owner);
    app.world_mut()
        .get_mut::<FairyGuiRoot>(owner)
        .unwrap()
        .parent = Some(container);
    app.update();
    assert_eq!(object(&app, owner), native);
    assert_eq!(view(&app, owner), nested);
    assert_eq!(
        app.world().get::<ChildOf>(nested).unwrap().parent(),
        container
    );
    assert_eq!(
        mount(&app, owner),
        GuiRootMountState::Mounted {
            parent: Some(container)
        }
    );
    app.world_mut()
        .get_mut::<FairyGuiRoot>(owner)
        .unwrap()
        .active = false;
    app.update();
    assert_eq!(mount(&app, owner), GuiRootMountState::Detached);
    assert!(app.world().get::<ChildOf>(nested).is_none());
    assert_eq!(
        app.world().get::<Visibility>(nested),
        Some(&Visibility::Hidden)
    );
    app.world_mut()
        .get_mut::<FairyGuiRoot>(owner)
        .unwrap()
        .active = true;
    app.update();
    assert_eq!(
        app.world().get::<ChildOf>(nested).unwrap().parent(),
        container
    );
    app.world_mut().get_mut::<FairyGuiRoot>(b).unwrap().active = false;
    app.update();
    assert_eq!(view(&app, owner), nested);
    assert!(
        !app.world()
            .non_send::<GuiRuntime>()
            .store()
            .is_on_stage(native)
    );
    app.world_mut().get_mut::<FairyGuiRoot>(b).unwrap().active = true;
    app.update();
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .is_on_stage(native)
    );
    app.world_mut()
        .get_mut::<FairyGuiRoot>(owner)
        .unwrap()
        .parent = None;
    app.update();
    assert_eq!(view(&app, owner), nested);
    assert!(app.world().get::<ChildOf>(nested).is_none());
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .is_on_stage(native)
    );
}

#[test]
fn declared_cycles_and_leaf_parents_stay_off_stage_and_recover_when_fixed() {
    let (mut app, ready, _) = fixture();
    let a = app
        .world_mut()
        .spawn(FairyGuiRoot::new(ready.clone(), "Demo_Button"))
        .id();
    let b = app
        .world_mut()
        .spawn(FairyGuiRoot::new(ready.clone(), "Slider_HZ").with_parent(a))
        .id();
    app.world_mut().get_mut::<FairyGuiRoot>(a).unwrap().parent = Some(b);
    app.update();
    for owner in [a, b] {
        assert_eq!(mount(&app, owner), GuiRootMountState::ParentCycle);
        assert!(app.world().get::<GuiRootView>(owner).is_none());
        assert!(
            !app.world()
                .non_send::<GuiRuntime>()
                .store()
                .is_on_stage(object(&app, owner))
        );
    }
    app.world_mut().get_mut::<FairyGuiRoot>(a).unwrap().parent = None;
    app.update();
    assert_eq!(
        app.world().get::<ChildOf>(view(&app, b)).unwrap().parent(),
        view(&app, a)
    );
    let leaf = child(&mut app, a, "n8");
    app.world_mut().get_mut::<FairyGuiRoot>(b).unwrap().parent = Some(leaf);
    app.update();
    assert_eq!(mount(&app, b), GuiRootMountState::InvalidParent);
    assert!(
        !app.world()
            .non_send::<GuiRuntime>()
            .store()
            .is_on_stage(object(&app, b))
    );
    // A component in the root's own subtree is a cycle too.
    let grip = child(&mut app, b, "grip");
    app.world_mut().get_mut::<FairyGuiRoot>(b).unwrap().parent = Some(grip);
    app.update();
    assert_eq!(mount(&app, b), GuiRootMountState::ParentCycle);
    app.world_mut().get_mut::<FairyGuiRoot>(b).unwrap().parent = Some(a);
    app.update();
    assert_eq!(
        mount(&app, b),
        GuiRootMountState::Mounted { parent: Some(a) }
    );
}

#[test]
fn destroying_a_parent_reclaims_nested_views_and_owner_targets_rebuild_after_replacement() {
    let (mut app, ready, _) = fixture();
    let parent = app
        .world_mut()
        .spawn(FairyGuiRoot::new(ready.clone(), "Demo_Button"))
        .id();
    let nested = app
        .world_mut()
        .spawn(FairyGuiRoot::new(ready.clone(), "Slider_HZ").with_parent(parent))
        .id();
    app.update();
    let old = view(&app, nested);
    let old_object = object(&app, nested);
    app.world_mut()
        .get_mut::<FairyGuiRoot>(parent)
        .unwrap()
        .item_name = "Demo_Controller".into();
    app.update();
    assert!(app.world().get_entity(old).is_err());
    assert!(
        !app.world()
            .non_send::<GuiRuntime>()
            .store()
            .contains(old_object)
    );
    assert_ne!(view(&app, nested), old);
    assert_eq!(
        app.world()
            .get::<ChildOf>(view(&app, nested))
            .unwrap()
            .parent(),
        view(&app, parent)
    );
    let replacement_view = view(&app, nested);
    app.world_mut().despawn(parent);
    app.update();
    assert!(app.world().get_entity(replacement_view).is_err());
    assert_eq!(mount(&app, nested), GuiRootMountState::WaitingForParent);
    assert!(app.world().get::<GuiRootView>(nested).is_none());
    let retained_count = app.world().non_send::<GuiRuntime>().store().len();
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(
        app.world().non_send::<GuiRuntime>().store().len(),
        retained_count
    );
    app.world_mut().despawn(nested);
    app.update();
    assert_eq!(app.world().non_send::<GuiRuntime>().store().len(), 0);
}

#[test]
fn moving_a_child_before_its_parent_is_destroyed_preserves_the_child_and_its_application_components()
 {
    #[derive(Component)]
    struct Keep;
    let (mut app, ready, _) = fixture();
    let parent = app
        .world_mut()
        .spawn(FairyGuiRoot::new(ready.clone(), "Demo_Button"))
        .id();
    let next = app
        .world_mut()
        .spawn(FairyGuiRoot::new(ready.clone(), "Demo_Controller"))
        .id();
    let nested = app
        .world_mut()
        .spawn(FairyGuiRoot::new(ready, "Slider_HZ").with_parent(parent))
        .id();
    app.update();
    let retained = view(&app, nested);
    app.world_mut().entity_mut(retained).insert(Keep);
    app.world_mut()
        .get_mut::<FairyGuiRoot>(nested)
        .unwrap()
        .parent = Some(next);
    app.world_mut().despawn(parent);
    app.update();
    assert_eq!(view(&app, nested), retained);
    assert!(app.world().get::<Keep>(retained).is_some());
    assert_eq!(
        app.world().get::<ChildOf>(retained).unwrap().parent(),
        view(&app, next)
    );
    app.world_mut().entity_mut(nested).remove::<FairyGuiRoot>();
    app.update();
    assert!(app.world().get_entity(retained).is_err());
    assert!(app.world().get::<GuiRootMountState>(nested).is_none());
}

#[test]
fn newly_created_owner_chains_mount_regardless_of_creation_order() {
    let (mut app, ready, _) = fixture();
    let leaf = app
        .world_mut()
        .spawn(FairyGuiRoot::new(ready.clone(), "Slider_HZ"))
        .id();
    let middle = app
        .world_mut()
        .spawn(FairyGuiRoot::new(ready.clone(), "Component1"))
        .id();
    let top = app
        .world_mut()
        .spawn(FairyGuiRoot::new(ready, "Demo_Button"))
        .id();
    app.world_mut()
        .get_mut::<FairyGuiRoot>(leaf)
        .unwrap()
        .parent = Some(middle);
    app.world_mut()
        .get_mut::<FairyGuiRoot>(middle)
        .unwrap()
        .parent = Some(top);
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(
        app.world()
            .get::<ChildOf>(view(&app, leaf))
            .unwrap()
            .parent(),
        view(&app, middle)
    );
    assert_eq!(
        app.world()
            .get::<ChildOf>(view(&app, middle))
            .unwrap()
            .parent(),
        view(&app, top)
    );
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .is_on_stage(object(&app, leaf))
    );
}

#[test]
fn window_owner_mounts_inherit_hidden_state_and_destruction() {
    let (mut app, ready, _) = fixture();
    let window = app
        .world_mut()
        .spawn(FairyGuiWindow::new(ready.clone(), "WindowA"))
        .id();
    let nested = app
        .world_mut()
        .spawn(FairyGuiRoot::new(ready, "Slider_HZ").with_parent(window))
        .id();
    app.update();
    let window_view = app.world().get::<GuiWindowState>(window).unwrap().view();
    let nested_view = view(&app, nested);
    assert_eq!(
        app.world().get::<ChildOf>(nested_view).unwrap().parent(),
        window_view
    );
    app.world_mut()
        .write_message(GuiWindowCommand::new(window, GuiWindowAction::Hide));
    app.update();
    assert_eq!(view(&app, nested), nested_view);
    assert!(
        !app.world()
            .non_send::<GuiRuntime>()
            .store()
            .is_on_stage(object(&app, nested))
    );
    app.world_mut().despawn(window);
    app.update();
    assert!(app.world().get_entity(nested_view).is_err());
    assert!(app.world().get::<GuiRootView>(nested).is_none());
}

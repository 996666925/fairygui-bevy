use super::*;
use crate::FairyGuiPlugin;
use crate::ecs::{GuiObjectInfo, GuiQuery};
use bevy::ecs::system::SystemState;

fn app() -> (App, Handle<FuiAsset>) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let asset = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
    (app, asset)
}

fn load(app: &mut App, asset: &Handle<FuiAsset>) {
    let package = FuiPackage::parse(
        "MainMenu",
        include_bytes!("../../fairygui-demo/assets/ui/MainMenu.fui"),
    )
    .unwrap();
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(
            asset.id(),
            FuiAsset {
                package,
                res_key: "ui/MainMenu".into(),
            },
        )
        .unwrap();
}

fn send(app: &mut App, target: Entity, action: GuiObjectAction) {
    app.world_mut()
        .write_message(GuiObjectCommand::new(target, action));
}

fn assert_position(app: &App, owner: Entity, expected: Vec2) -> GObjectId {
    let root = app.world().get::<GuiRootInstance>(owner).unwrap().object;
    let runtime = app.world().non_send::<GuiRuntime>();
    let object = runtime.store().object(root).unwrap();
    assert_eq!((object.x(), object.y()), (expected.x, expected.y));
    let view = runtime.entity_of(root).unwrap();
    let node = app.world().get::<Node>(view).unwrap();
    assert_eq!(node.left, Val::Px(expected.x));
    assert_eq!(node.top, Val::Px(expected.y));
    root
}

#[test]
fn startup_root_commands_apply_before_the_first_view_is_spawned() {
    #[derive(Resource)]
    struct Source(Handle<FuiAsset>);
    #[derive(Component)]
    struct Menu;
    fn spawn(mut commands: Commands, source: Res<Source>) {
        let owner = commands
            .spawn((FairyGuiRoot::new(source.0.clone(), "Main"), Menu))
            .id();
        commands.write_message(GuiObjectCommand::new(
            owner,
            GuiObjectAction::SetPosition(Vec2::new(37., 42.)),
        ));
        commands.write_message(GuiObjectCommand::new(
            owner,
            GuiObjectAction::SetVisible(false),
        ));
    }

    let (mut app, asset) = app();
    load(&mut app, &asset);
    app.insert_resource(Source(asset))
        .add_systems(Startup, spawn);
    app.update();
    let owner = app
        .world_mut()
        .query_filtered::<Entity, With<Menu>>()
        .single(app.world())
        .unwrap();
    let root = assert_position(&app, owner, Vec2::new(37., 42.));
    let view = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(root)
        .unwrap();
    assert_eq!(
        app.world().get::<Visibility>(view),
        Some(&Visibility::Hidden)
    );
}

#[test]
fn delayed_root_commands_outlive_message_retention_preserve_order_and_apply_once() {
    let (mut app, asset) = app();
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), "Main"))
        .id();
    send(
        &mut app,
        owner,
        GuiObjectAction::SetPosition(Vec2::new(10., 20.)),
    );
    send(&mut app, owner, GuiObjectAction::SetVisible(false));
    app.update();
    send(
        &mut app,
        owner,
        GuiObjectAction::SetPosition(Vec2::new(40., 50.)),
    );
    for _ in 0..4 {
        app.update();
    }
    assert!(app.world().get::<GuiRootInstance>(owner).is_none());
    load(&mut app, &asset);
    send(
        &mut app,
        owner,
        GuiObjectAction::SetPosition(Vec2::new(60., 70.)),
    );
    app.update();
    let root = assert_position(&app, owner, Vec2::new(60., 70.));
    let view = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(root)
        .unwrap();
    assert_eq!(
        app.world().get::<Visibility>(view),
        Some(&Visibility::Hidden)
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_xy(root, 90., 100.);
    app.update();
    assert_position(&app, owner, Vec2::new(90., 100.));
}

#[test]
fn replacing_or_removing_a_pending_root_cancels_old_commands() {
    for replace in [0, 1, 2] {
        let (mut app, asset) = app();
        let owner = app
            .world_mut()
            .spawn(FairyGuiRoot::new(asset.clone(), "Main"))
            .id();
        send(
            &mut app,
            owner,
            GuiObjectAction::SetPosition(Vec2::new(37., 42.)),
        );
        app.update();
        let loaded_asset = if replace == 1 {
            app.world_mut()
                .get_mut::<FairyGuiRoot>(owner)
                .unwrap()
                .item_name = "CloseButton".into();
            asset
        } else if replace == 2 {
            let replacement = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
            app.world_mut()
                .get_mut::<FairyGuiRoot>(owner)
                .unwrap()
                .asset = replacement.clone();
            replacement
        } else {
            // Even removal and reinsertion between updates starts a new lifetime.
            app.world_mut().entity_mut(owner).remove::<FairyGuiRoot>();
            app.world_mut()
                .entity_mut(owner)
                .insert(FairyGuiRoot::new(asset.clone(), "Main"));
            asset
        };
        load(&mut app, &loaded_asset);
        app.update();
        assert_position(&app, owner, Vec2::ZERO);
    }
}

#[test]
fn ready_root_replacement_applies_new_commands_to_its_new_model() {
    let (mut app, asset) = app();
    load(&mut app, &asset);
    let owner = app.world_mut().spawn(FairyGuiRoot::new(asset, "Main")).id();
    app.update();
    let old = assert_position(&app, owner, Vec2::ZERO);
    app.world_mut()
        .get_mut::<FairyGuiRoot>(owner)
        .unwrap()
        .item_name = "CloseButton".into();
    send(
        &mut app,
        owner,
        GuiObjectAction::SetPosition(Vec2::new(37., 42.)),
    );
    app.update();
    let new = assert_position(&app, owner, Vec2::new(37., 42.));
    assert_ne!(new, old);
    assert!(!app.world().non_send::<GuiRuntime>().store().contains(old));
}

#[test]
fn pending_commands_are_discarded_when_the_owner_is_despawned() {
    let (mut app, asset) = app();
    let old = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), "Main"))
        .id();
    send(
        &mut app,
        old,
        GuiObjectAction::SetPosition(Vec2::new(37., 42.)),
    );
    app.update();
    app.world_mut().despawn(old);
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), "Main"))
        .id();
    load(&mut app, &asset);
    app.update();
    assert_ne!(owner, old);
    assert_position(&app, owner, Vec2::ZERO);
}

#[test]
fn ecs_queries_follow_root_loading_replacement_removal_and_retained_views() {
    let (mut app, asset) = app();
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), "Main"))
        .id();
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    app.update();
    assert_eq!(query.get(app.world()).unwrap().view(owner), None);
    load(&mut app, &asset);
    app.update();
    let view = {
        let gui = query.get(app.world()).unwrap();
        let view = gui.view(owner).unwrap();
        assert_eq!(gui.view(view), Some(view));
        let button = gui.child_by_name(owner, "n1").unwrap();
        assert_eq!(gui.get(button).unwrap().name(), "n1");
        assert_eq!(
            gui.get(button).unwrap().kind(),
            fairygui_core::package::ObjectType::Button
        );
        assert_eq!(gui.get(button).unwrap().parent(), Some(view));
        assert!(gui.child_by_path(owner, "n1/title").is_some());
        assert!(gui.child_by_path(owner, "n1/missing").is_none());
        assert_eq!(gui.child_by_path(owner, ""), Some(view));
        view
    };
    let unchanged = app
        .world()
        .entity(view)
        .get_ref::<GuiObjectInfo>()
        .unwrap()
        .last_changed();
    app.world_mut()
        .get_mut::<FairyGuiRoot>(owner)
        .unwrap()
        .active = false;
    app.update();
    assert_eq!(query.get(app.world()).unwrap().view(owner), Some(view));
    assert_eq!(
        app.world()
            .entity(view)
            .get_ref::<GuiObjectInfo>()
            .unwrap()
            .last_changed(),
        unchanged
    );

    app.world_mut()
        .get_mut::<FairyGuiRoot>(owner)
        .unwrap()
        .item_name = "CloseButton".into();
    app.update();
    let replacement = query.get(app.world()).unwrap().view(owner).unwrap();
    assert_ne!(replacement, view);
    assert!(query.get(app.world()).unwrap().get(view).is_none());
    app.world_mut().entity_mut(owner).remove::<FairyGuiRoot>();
    app.update();
    assert!(app.world().get::<GuiRootView>(owner).is_none());
    let gui = query.get(app.world()).unwrap();
    assert!(gui.view(owner).is_none());
    assert!(gui.get(replacement).is_none());
}

#[test]
fn ecs_queries_preserve_logical_order_and_track_dynamic_names_and_parenting() {
    use fairygui_core::package::ObjectType;
    let (mut app, asset) = app();
    load(&mut app, &asset);
    let owner = app.world_mut().spawn(FairyGuiRoot::new(asset, "Main")).id();
    app.update();
    let root = app.world().get::<GuiRootInstance>(owner).unwrap().object;
    let (branch, nested, direct) = {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        let branch = runtime.store_mut().create(ObjectType::Component);
        runtime.store_mut().set_name(branch, "branch");
        runtime.store_mut().set_parent(branch, Some(root));
        let nested = runtime.store_mut().create(ObjectType::Graph);
        runtime.store_mut().set_name(nested, "needle");
        runtime.store_mut().set_parent(nested, Some(branch));
        let direct = runtime.store_mut().create(ObjectType::Graph);
        runtime.store_mut().set_name(direct, "needle");
        runtime.store_mut().set_parent(direct, Some(root));
        runtime.queue_subtree(branch);
        runtime.queue_subtree(direct);
        (branch, nested, direct)
    };
    app.update();
    let (view, branch_view, nested_view, direct_view) = {
        let runtime = app.world().non_send::<GuiRuntime>();
        (
            runtime.entity_of(root).unwrap(),
            runtime.entity_of(branch).unwrap(),
            runtime.entity_of(nested).unwrap(),
            runtime.entity_of(direct).unwrap(),
        )
    };
    let decorative = app
        .world_mut()
        .spawn((Name::new("needle"), ChildOf(view)))
        .id();
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    {
        let gui = query.get(app.world()).unwrap();
        assert_eq!(gui.find_by_name(owner, "needle"), Some(direct_view));
        assert_eq!(gui.child_by_path(owner, "branch/needle"), Some(nested_view));
        assert!(!gui.get(view).unwrap().children().contains(&decorative));
        assert!(gui.get(decorative).is_none());
    }

    {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        runtime.store_mut().set_name(direct, "renamed");
        runtime.store_mut().add_child_at(root, nested, 0);
    }
    app.update();
    {
        let gui = query.get(app.world()).unwrap();
        assert_eq!(gui.find_by_name(owner, "needle"), Some(nested_view));
        assert_eq!(gui.child_by_name(owner, "renamed"), Some(direct_view));
        assert_eq!(gui.get(nested_view).unwrap().parent(), Some(view));
        assert_eq!(
            gui.get(view).unwrap().children().first(),
            Some(&nested_view)
        );
        assert!(gui.child_by_path(owner, "branch/needle").is_none());
    }
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .detach_from_parent(branch);
    app.update();
    let gui = query.get(app.world()).unwrap();
    assert!(gui.child_by_name(owner, "branch").is_none());
    assert_eq!(gui.get(branch_view).unwrap().parent(), None);
    assert_eq!(gui.view(branch_view), Some(branch_view));
}

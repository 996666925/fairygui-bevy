use crate::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};
use fairygui_core::package::ObjectType;

struct Scene {
    owner: Entity,
    root: Entity,
    group: Entity,
    alternate: Entity,
    image: Entity,
    movie: Entity,
}

fn fixture() -> (App, Scene, Handle<FuiAsset>) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: fairygui_core::FuiPackage::parse(
                "ModalWaiting",
                include_bytes!("../../fairygui-demo/assets/ui/ModalWaiting.fui"),
            )
            .unwrap(),
            res_key: "ui/ModalWaiting".into(),
        });
    let owner = app
        .world_mut()
        .spawn(
            FairyGuiRoot::new(asset.clone(), "WindowModalWaiting").with_size(Vec2::new(400., 300.)),
        )
        .id();
    app.update();
    let alternate = {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        let parent = runtime.ecs_root_object(owner).unwrap();
        let store = runtime.store_mut();
        let group = store.create(ObjectType::Group);
        store.set_name(group, "alternate_group");
        store.set_parent(group, Some(parent));
        group
    };
    app.update();
    // Native child insertion queues its retained view during projection.
    app.update();
    let alternate = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(alternate)
        .unwrap();
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    let gui = query.get(app.world()).unwrap();
    let scene = Scene {
        owner,
        alternate,
        root: gui.view(owner).unwrap(),
        group: gui.child_by_name(owner, "n4").unwrap(),
        image: gui.child_by_name(owner, "n1").unwrap(),
        movie: gui.child_by_name(owner, "n3").unwrap(),
    };
    (app, scene, asset)
}

#[derive(Resource)]
struct Switch {
    member: Entity,
    old_group: Entity,
    new_group: Entity,
    done: bool,
    verified: bool,
}
fn switch_groups(mut switch: ResMut<Switch>, mut objects: GuiMut) {
    if switch.done {
        return;
    }
    assert!(objects.set_group(switch.member, Some(switch.new_group)));
    assert!(!objects.set_group(switch.member, Some(switch.new_group)));
    objects
        .get_mut(switch.old_group)
        .unwrap()
        .set_visible(false);
    objects
        .get_mut(switch.new_group)
        .unwrap()
        .set_visible(false);
    switch.done = true;
}

fn verify_switch_before_projection(mut switch: ResMut<Switch>, gui: GuiQuery) {
    if !switch.done || switch.verified {
        return;
    }
    assert_eq!(gui.group(switch.member), Some(switch.new_group));
    assert_eq!(
        gui.group_members(switch.new_group)
            .unwrap()
            .collect::<Vec<_>>(),
        [switch.member]
    );
    assert!(!gui.display(switch.member).unwrap().display_visible());
    switch.verified = true;
}

#[test]
fn entity_group_edits_publish_membership_visibility_and_geometry_without_reparenting() {
    let (mut app, s, _) = fixture();
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    let gui = query.get(app.world()).unwrap();
    assert_eq!(gui.group(s.image), Some(s.group));
    assert_eq!(gui.get(s.movie).unwrap().group(), Some(s.group));
    assert_eq!(
        gui.group_members(s.group).unwrap().collect::<Vec<_>>(),
        [s.image, s.movie]
    );
    assert!(gui.group_members(s.alternate).unwrap().next().is_none());
    assert!(gui.group_members(s.image).is_none());
    let initial_position = gui.transform(s.image).unwrap().position();
    let initial_size = gui.transform(s.image).unwrap().size();
    app.insert_resource(Switch {
        member: s.image,
        old_group: s.group,
        new_group: s.alternate,
        done: false,
        verified: false,
    })
    .add_systems(Update, switch_groups.in_set(GuiSystems::Respond))
    .add_systems(
        Update,
        verify_switch_before_projection.in_set(GuiSystems::PostLayout),
    );
    app.update();
    let gui = query.get(app.world()).unwrap();
    assert!(app.world().resource::<Switch>().verified);
    assert_eq!(gui.group(s.image), Some(s.alternate));
    assert_eq!(
        gui.group_members(s.group).unwrap().collect::<Vec<_>>(),
        [s.movie]
    );
    assert_eq!(
        gui.group_members(s.alternate).unwrap().collect::<Vec<_>>(),
        [s.image]
    );
    assert_eq!(
        gui.transform(s.alternate).unwrap().position(),
        initial_position
    );
    assert_eq!(gui.transform(s.alternate).unwrap().size(), initial_size);
    assert!(!gui.display(s.image).unwrap().display_visible());
    assert!(!gui.display(s.movie).unwrap().display_visible());
    assert!(gui.display(s.image).unwrap().visible);
    for member in [s.image, s.movie] {
        assert_eq!(gui.get(member).unwrap().parent(), Some(s.root));
        assert_eq!(app.world().get::<ChildOf>(member).unwrap().parent(), s.root);
    }
    assert!(gui.get(s.alternate).unwrap().children().is_empty());
    assert!(app.world_mut().gui_set_group(s.image, None));
    assert!(!app.world_mut().gui_set_group(s.image, None));
    app.update();
    let gui = query.get(app.world()).unwrap();
    assert_eq!(gui.group(s.image), None);
    assert!(gui.display(s.image).unwrap().display_visible());
    assert!(gui.group_members(s.alternate).unwrap().next().is_none());
    assert_eq!(gui.transform(s.alternate).unwrap().size(), Vec2::ZERO);
    // A detached member no longer follows the old group's movement.
    app.world_mut()
        .gui_mut(s.alternate)
        .unwrap()
        .set_position(Vec2::new(20., 30.));
    app.update();
    let gui = query.get(app.world()).unwrap();
    assert_eq!(gui.transform(s.image).unwrap().position(), initial_position);
}

#[test]
fn group_edits_reject_invalid_targets_and_cycles_and_query_direct_siblings_in_order() {
    let (mut app, s, asset) = fixture();
    let pending = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
    let pending = app
        .world_mut()
        .spawn(FairyGuiRoot::new(pending, "WindowModalWaiting"))
        .id();
    let foreign = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "WindowModalWaiting"))
        .id();
    let ordinary = app.world_mut().spawn_empty().id();
    let stale = app.world_mut().spawn_empty().id();
    app.world_mut().despawn(stale);
    app.update();
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    let gui = query.get(app.world()).unwrap();
    let foreign_group = gui.child_by_name(foreign, "n4").unwrap();
    for target in [
        ordinary,
        pending,
        stale,
        s.image,
        s.root,
        s.owner,
        foreign_group,
    ] {
        assert!(
            !app.world_mut().gui_set_group(s.movie, Some(target)),
            "{target:?}"
        );
    }
    for member in [ordinary, pending, stale] {
        assert!(!app.world_mut().gui_set_group(member, Some(s.group)));
        assert!(!app.world_mut().gui_set_group(member, None));
    }
    assert!(!app.world_mut().gui_set_group(s.group, Some(s.group)));
    assert!(app.world_mut().gui_set_group(s.group, Some(s.alternate)));
    assert!(!app.world_mut().gui_set_group(s.alternate, Some(s.group)));
    app.world_mut().gui_mut(s.movie).unwrap().set_visible(false);
    app.world_mut().gui_mut(s.movie).unwrap().set_child_index(0);
    app.update();
    let gui = query.get(app.world()).unwrap();
    assert_eq!(
        gui.group_members(s.alternate).unwrap().collect::<Vec<_>>(),
        [s.group]
    );
    assert_eq!(
        gui.group_members(s.group).unwrap().collect::<Vec<_>>(),
        [s.movie, s.image]
    );
    assert_eq!(gui.group(s.group), Some(s.alternate));
    assert_eq!(gui.group(s.movie), Some(s.group));
    assert!(gui.group_members(stale).is_none());
    assert!(gui.group_members(s.owner).is_none());
    app.world_mut().despawn(s.owner);
    app.update();
    let gui = query.get(app.world()).unwrap();
    assert_eq!(gui.group(s.movie), None);
    assert!(gui.group_members(s.group).is_none());
    assert!(!app.world_mut().gui_set_group(s.movie, Some(s.alternate)));
}

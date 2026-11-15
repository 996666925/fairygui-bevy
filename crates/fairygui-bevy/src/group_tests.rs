use super::*;
use crate::prelude::*;
use bevy::ecs::system::SystemState;

#[test]
fn exported_centered_group_is_correct_on_first_frame_and_after_ecs_member_edits() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        crate::FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: FuiPackage::parse(
                "ModalWaiting",
                include_bytes!("../../fairygui-demo/assets/ui/ModalWaiting.fui"),
            )
            .unwrap(),
            res_key: "ui/ModalWaiting".into(),
        });
    let owner = app
        .world_mut()
        .spawn(
            FairyGuiRoot::new(asset, "WindowModalWaiting")
                .with_position(Vec2::new(31., 17.))
                .with_size(Vec2::new(400., 300.)),
        )
        .id();
    app.update();
    let mut queries = SystemState::<GuiQuery>::new(app.world_mut());
    let gui = queries.get(app.world()).unwrap();
    let group = gui.child_by_name(owner, "n4").unwrap();
    let image = gui.child_by_name(owner, "n1").unwrap();
    let movie = gui.child_by_name(owner, "n3").unwrap();
    assert_eq!(
        gui.transform(group).unwrap().position(),
        Vec2::new(155., 110.)
    );
    assert_eq!(
        gui.transform(image).unwrap().position(),
        Vec2::new(155., 110.)
    );
    assert_eq!(
        gui.transform(movie).unwrap().position(),
        Vec2::new(177., 136.)
    );
    let mut transform = *gui.transform(image).unwrap();
    transform.size.x = 130.;
    app.world_mut().entity_mut(image).insert(transform);
    app.update();
    let gui = queries.get(app.world()).unwrap();
    assert_eq!(gui.transform(group).unwrap().size(), Vec2::new(130., 80.));
    assert_eq!(
        gui.transform(group).unwrap().position(),
        Vec2::new(135., 110.)
    );
    assert_eq!(
        gui.transform(image).unwrap().position(),
        Vec2::new(135., 110.)
    );
    assert_eq!(
        gui.transform(movie).unwrap().position(),
        Vec2::new(157., 136.)
    );
    for entity in [group, image, movie] {
        let transform = gui.transform(entity).unwrap();
        let node = app.world().get::<Node>(entity).unwrap();
        assert_eq!(
            (node.left, node.top),
            (Val::Px(transform.position.x), Val::Px(transform.position.y))
        );
    }
    let stable: Vec<_> = [group, image, movie]
        .into_iter()
        .map(|entity| *gui.transform(entity).unwrap())
        .collect();
    app.update();
    let gui = queries.get(app.world()).unwrap();
    assert_eq!(
        [group, image, movie]
            .into_iter()
            .map(|entity| *gui.transform(entity).unwrap())
            .collect::<Vec<_>>(),
        stable
    );
    // Parent resizing must keep the already enlarged group centered without rebuilding.
    let mut root = *gui.transform(owner).unwrap();
    root.size = Vec2::new(600., 500.);
    app.world_mut().entity_mut(owner).insert(root);
    app.update();
    let gui = queries.get(app.world()).unwrap();
    assert_eq!(
        gui.transform(group).unwrap().position(),
        Vec2::new(235., 210.)
    );
    assert_eq!(
        gui.transform(image).unwrap().position(),
        Vec2::new(235., 210.)
    );
    assert_eq!(
        gui.transform(movie).unwrap().position(),
        Vec2::new(257., 236.)
    );
}

#[test]
fn grouped_exported_size_relations_keep_member_edges_after_ecs_parent_resizing() {
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
            package: FuiPackage::parse(
                "Basics",
                include_bytes!("../../fairygui-demo/assets/ui/Basics.fui"),
            )
            .unwrap(),
            res_key: "ui/Basics".into(),
        });
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "Button1").with_size(Vec2::new(160., 80.)))
        .id();
    app.update();
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    let gui = query.get(app.world()).unwrap();
    let view = gui.view(owner).unwrap();
    let members: Vec<_> = ["n1", "n2", "n3", "title"]
        .into_iter()
        .map(|name| gui.child_by_name(owner, name).unwrap())
        .collect();
    let parent = app.world().get::<GObjectRef>(view).unwrap().0;
    let ids: Vec<_> = members
        .iter()
        .map(|&e| app.world().get::<GObjectRef>(e).unwrap().0)
        .collect();
    let group = {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        let store = runtime.store_mut();
        let group = store.create(fairygui_core::package::ObjectType::Group);
        store.set_parent(group, Some(parent));
        for &member in &ids {
            store.set_group(member, Some(group));
        }
        group
    };
    // Native child insertion queues the new group's view during projection.
    app.update();
    app.update();
    let group_entity = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(group)
        .unwrap();
    for (pivot, anchor) in [(0., false), (0.5, false), (1., false), (0.5, true)] {
        for &entity in &members {
            app.world_mut()
                .gui_mut(entity)
                .unwrap()
                .set_pivot(Vec2::splat(pivot), anchor);
            // Restore the same left/top edge before each independent cycle.
            app.world_mut()
                .gui_mut(entity)
                .unwrap()
                .set_position(if anchor {
                    Vec2::new(160., 80.) * pivot
                } else {
                    Vec2::ZERO
                });
        }
        app.update();
        for size in [Vec2::new(260., 120.), Vec2::new(160., 80.)] {
            app.world_mut().get_mut::<GuiTransform>(view).unwrap().size = size;
            app.update();
            let gui = query.get(app.world()).unwrap();
            assert_eq!(gui.transform(group_entity).unwrap().position(), Vec2::ZERO);
            assert_eq!(gui.transform(group_entity).unwrap().size(), size);
            for &entity in &members {
                let transform = gui.transform(entity).unwrap();
                assert_eq!(transform.size(), size);
                let position = if anchor { size * pivot } else { Vec2::ZERO };
                assert_eq!(
                    transform.position(),
                    position,
                    "pivot={pivot}, anchor={anchor}"
                );
                let node = app.world().get::<Node>(entity).unwrap();
                assert_eq!(
                    (node.left, node.top),
                    (Val::Px(position.x), Val::Px(position.y))
                );
                assert_eq!(
                    app.world().get::<UiTransform>(entity).unwrap().translation,
                    Val2::px(-position.x, -position.y)
                );
            }
        }
    }
}

#[test]
fn group_alignment_survives_an_ecs_edit_to_the_other_position_axis() {
    use crate::{core::package::ObjectType, properties::sync_property};
    for (resize_group, edit_alignment) in [(false, false), (true, false), (true, true)] {
        for (horizontal, relation, factor) in [
            (true, RelationType::LeftLeft, 0.),
            (true, RelationType::CenterCenter, 0.5),
            (true, RelationType::RightRight, 1.),
            (false, RelationType::TopTop, 0.),
            (false, RelationType::MiddleMiddle, 0.5),
            (false, RelationType::BottomBottom, 1.),
        ] {
            let mut world = World::new();
            world.init_non_send::<GuiRuntime>();
            let (parent, group, member) = {
                let mut runtime = world.non_send_mut::<GuiRuntime>();
                let store = runtime.store_mut();
                let parent = store.create(ObjectType::Component);
                let group = store.create(ObjectType::Group);
                let member = store.create(ObjectType::Image);
                store.set_size(parent, 400., 300.);
                for id in [group, member] {
                    store.set_parent(id, Some(parent));
                    store.set_size(id, 100., 40.);
                    store.set_xy(id, 150., 130.);
                }
                store.set_group(member, Some(group));
                store.set_group_layout_settings(
                    group,
                    fairygui_core::object::GroupLayoutSettings {
                        layout: if horizontal {
                            GroupLayoutType::Horizontal
                        } else {
                            GroupLayoutType::Vertical
                        },
                        auto_size_disabled: true,
                        ..default()
                    },
                );
                store.flush_group_layouts();
                store.add_relation(group, parent, relation, false);
                (parent, group, member)
            };
            // Parent is visited first. Its resize updates the native group position
            // before the group's pending application edit is consumed.
            let parent_entity = world.spawn(GObjectRef(parent)).id();
            let group_entity = world.spawn(GObjectRef(group)).id();
            let member_entity = world.spawn(GObjectRef(member)).id();
            sync_property::<GuiTransform>(&mut world);
            let resized = if resize_group {
                group_entity
            } else {
                parent_entity
            };
            let delta = if resize_group { 40. } else { 200. };
            if edit_alignment {
                // The application position equals the position produced by the
                // parent's resize, but must still win after our own resize.
                let mut parent = world.get_mut::<GuiTransform>(parent_entity).unwrap();
                if horizontal {
                    parent.size.x += 200.;
                } else {
                    parent.size.y += 200.;
                }
                let mut group = world.get_mut::<GuiTransform>(group_entity).unwrap();
                if horizontal {
                    group.size.x += delta;
                    group.position.x += 200. * factor;
                } else {
                    group.size.y += delta;
                    group.position.y += 200. * factor;
                }
            } else if horizontal {
                world.get_mut::<GuiTransform>(resized).unwrap().size.x += delta;
                world
                    .get_mut::<GuiTransform>(group_entity)
                    .unwrap()
                    .position
                    .y += 17.;
            } else {
                world.get_mut::<GuiTransform>(resized).unwrap().size.y += delta;
                world
                    .get_mut::<GuiTransform>(group_entity)
                    .unwrap()
                    .position
                    .x += 17.;
            }
            let aligned_delta = if edit_alignment {
                200. * factor
            } else if resize_group {
                -delta * factor
            } else {
                delta * factor
            };
            let offset = if edit_alignment { 0. } else { 17. };
            let expected = Vec2::new(150., 130.)
                + if horizontal {
                    Vec2::new(aligned_delta, offset)
                } else {
                    Vec2::new(offset, aligned_delta)
                };
            for _ in 0..3 {
                sync_property::<GuiTransform>(&mut world);
                for entity in [group_entity, member_entity] {
                    assert_eq!(
                        world.get::<GuiTransform>(entity).unwrap().position,
                        expected,
                        "relation={relation:?}, resize_group={resize_group}, edit_alignment={edit_alignment}"
                    );
                }
            }
        }
    }
}

use crate::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};

fn app() -> (App, Handle<FuiAsset>) {
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
                "Basics",
                include_bytes!("../../fairygui-demo/assets/ui/Basics.fui"),
            )
            .unwrap(),
            res_key: "ui/Basics".into(),
        });
    (app, asset)
}

#[derive(Resource)]
struct Pair {
    owner: Entity,
    target: Entity,
    step: u8,
}

fn edit(mut pair: ResMut<Pair>, mut objects: GuiMut, gui: GuiQuery) {
    let Pair {
        owner,
        target,
        step,
    } = *pair;
    match step {
        0 => {
            assert!(objects.add_relation(owner, target, RelationType::LeftLeft, false));
            assert!(objects.add_relation(owner, target, RelationType::TopTop, false));
            assert!(objects.add_relation(owner, target, RelationType::Size, false));
            assert!(!objects.add_relation(
                owner,
                gui.view(target).unwrap(),
                RelationType::Size,
                true
            ));
            assert!(objects.has_relation(gui.view(owner).unwrap(), target, RelationType::Size));
            objects
                .get_mut(target)
                .unwrap()
                .set_position(Vec2::new(50., 50.));
            objects
                .get_mut(target)
                .unwrap()
                .set_size(Vec2::new(200., 120.));
        }
        1 => {
            assert!(objects.remove_relation(owner, target, RelationType::Width));
            assert!(!objects.has_relation(owner, target, RelationType::Size));
            assert!(objects.has_relation(owner, target, RelationType::Height));
            objects
                .get_mut(target)
                .unwrap()
                .set_size(Vec2::new(250., 150.));
        }
        2 => {
            assert!(objects.clear_relations_to(owner, target));
            objects
                .get_mut(target)
                .unwrap()
                .set_position(Vec2::new(100., 100.));
            objects
                .get_mut(target)
                .unwrap()
                .set_size(Vec2::new(300., 180.));
        }
        _ => {}
    }
    pair.step += 1;
}

fn check(app: &mut App, owner: Entity, position: Vec2, size: Vec2) {
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    let gui = query.get(app.world()).unwrap();
    let view = gui.view(owner).unwrap();
    let transform = gui.transform(view).unwrap();
    assert_eq!(transform.position(), position);
    assert_eq!(transform.size(), size);
    let node = app.world().get::<Node>(view).unwrap();
    assert_eq!(
        (node.left, node.top, node.width, node.height),
        (
            Val::Px(position.x),
            Val::Px(position.y),
            Val::Px(size.x),
            Val::Px(size.y)
        )
    );
    let title = gui.child_by_name(owner, "title").unwrap();
    assert_eq!(gui.transform(title).unwrap().size(), size);
}

#[test]
fn entity_relation_edits_publish_in_the_same_frame_and_propagate_to_exported_children() {
    let (mut app, asset) = app();
    let target = app
        .world_mut()
        .spawn(
            FairyGuiRoot::new(asset.clone(), "Button1")
                .with_position(Vec2::new(30., 40.))
                .with_size(Vec2::new(100., 60.)),
        )
        .id();
    let owner = app
        .world_mut()
        .spawn(
            FairyGuiRoot::new(asset, "Button1")
                .with_position(Vec2::new(240., 160.))
                .with_size(Vec2::new(160., 80.)),
        )
        .id();
    app.update();
    app.insert_resource(Pair {
        owner,
        target,
        step: 0,
    })
    .add_systems(Update, edit.in_set(GuiSystems::Respond));
    app.update();
    check(
        &mut app,
        owner,
        Vec2::new(260., 170.),
        Vec2::new(260., 140.),
    );
    app.update();
    check(
        &mut app,
        owner,
        Vec2::new(260., 170.),
        Vec2::new(260., 170.),
    );
    app.update();
    check(
        &mut app,
        owner,
        Vec2::new(260., 170.),
        Vec2::new(260., 170.),
    );
}

#[test]
fn world_relations_reject_unready_aliases_and_stale_entities_without_latching_future_targets() {
    let (mut app, asset) = app();
    let missing = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
    let pending = app
        .world_mut()
        .spawn(FairyGuiRoot::new(missing, "Button1"))
        .id();
    let target = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), "Button1"))
        .id();
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "Button1"))
        .id();
    let ordinary = app.world_mut().spawn_empty().id();
    app.update();
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    let gui = query.get(app.world()).unwrap();
    let owner_view = gui.view(owner).unwrap();
    let target_view = gui.view(target).unwrap();
    for invalid in [ordinary, pending, owner_view] {
        assert!(
            !app.world_mut()
                .gui_add_relation(owner, invalid, RelationType::Size, false)
        );
    }
    assert!(
        app.world_mut()
            .gui_add_relation(owner, target_view, RelationType::Size, false)
    );
    assert!(
        app.world_mut()
            .gui_has_relation(owner_view, target, RelationType::Size)
    );
    assert!(
        app.world_mut()
            .gui_remove_relation(owner_view, target, RelationType::Height)
    );
    assert!(
        !app.world_mut()
            .gui_has_relation(owner, target, RelationType::Size)
    );
    assert!(app.world_mut().gui_clear_relations_to(owner_view, target));
    assert!(
        app.world_mut()
            .gui_add_relation(owner, target, RelationType::Width, false)
    );
    app.world_mut().despawn(target);
    app.update();
    assert!(!app.world().entities().contains(target_view));
    assert!(
        !app.world_mut()
            .gui_add_relation(owner, target_view, RelationType::Size, false)
    );
    assert!(app.world_mut().gui_mut(owner).unwrap().clear_relations());
    assert!(!app.world_mut().gui_mut(owner).unwrap().clear_relations());
}

#[test]
fn window_owners_and_views_resolve_the_same_relation_target() {
    let (mut app, asset) = app();
    let mut window = FairyGuiWindow::new(asset.clone(), "WindowA");
    window.center_on_create = false;
    let target = app.world_mut().spawn(window).id();
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "Button1"))
        .id();
    app.update();
    let target_view = app.world().get::<GuiWindowState>(target).unwrap().view();
    assert!(
        app.world_mut()
            .gui_add_relation(owner, target, RelationType::Size, false)
    );
    assert!(
        !app.world_mut()
            .gui_add_relation(owner, target_view, RelationType::Width, true)
    );
    assert!(
        app.world_mut()
            .gui_has_relation(owner, target_view, RelationType::Size)
    );
    assert!(app.world_mut().gui_clear_relations_to(owner, target_view));
}

#[test]
fn child_extension_relations_restore_design_edges_after_manual_resize_and_size_limits() {
    let (mut app, asset) = app();
    let owner = app
        .world_mut()
        .spawn(
            FairyGuiRoot::new(asset, "Button1")
                .with_position(Vec2::new(400., 200.))
                .with_size(Vec2::new(200., 120.)),
        )
        .id();
    app.update();
    let mut queries = SystemState::<GuiQuery>::new(app.world_mut());
    let gui = queries.get(app.world()).unwrap();
    let title = gui.child_by_name(owner, "title").unwrap();
    let view = gui.view(owner).unwrap();
    app.world_mut().gui_mut(title).unwrap().clear_relations();
    app.world_mut()
        .gui_mut(owner)
        .unwrap()
        .set_pivot(Vec2::splat(0.5), false);
    assert!(
        app.world_mut()
            .gui_add_relation(owner, title, RelationType::RightExtRight, false)
    );
    app.world_mut().gui_mut(owner).unwrap().set_width(400.);
    app.world_mut().gui_mut(title).unwrap().set_width(260.);
    app.update();
    let gui = queries.get(app.world()).unwrap();
    assert_eq!(gui.transform(owner).unwrap().size(), Vec2::new(260., 120.));
    assert_eq!(
        gui.transform(owner).unwrap().position(),
        Vec2::new(370., 200.)
    );
    app.world_mut().entity_mut(view).insert(GuiSizeConstraints {
        min: Vec2::new(70., 50.),
        max: Vec2::new(240., 190.),
    });
    app.update();
    for (request, effective) in [(280., 240.), (30., 70.), (180., 180.)] {
        app.world_mut().gui_mut(title).unwrap().set_width(request);
        app.update();
        let gui = queries.get(app.world()).unwrap();
        assert_eq!(gui.transform(owner).unwrap().size().x, effective);
        assert_eq!(
            gui.transform(owner).unwrap().position().x,
            500. - effective * 0.5
        );
        let node = app.world().get::<Node>(view).unwrap();
        assert_eq!(
            (node.left, node.width),
            (Val::Px(500. - effective * 0.5), Val::Px(effective))
        );
    }
}

#[test]
fn exported_chat_height_relation_uses_design_size_after_runtime_row_height_overrides() {
    let (mut app, _) = app();
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: fairygui_core::FuiPackage::parse(
                "Chat",
                include_bytes!("../../fairygui-demo/assets/ui/Chat.fui"),
            )
            .unwrap(),
            res_key: "ui/Chat".into(),
        });
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "chatLeft"))
        .id();
    app.update();
    let mut queries = SystemState::<GuiQuery>::new(app.world_mut());
    let gui = queries.get(app.world()).unwrap();
    let bubble = gui.child_by_name(owner, "n0").unwrap();
    let view = gui.view(owner).unwrap();
    assert!(
        app.world_mut()
            .gui_has_relation(owner, bubble, RelationType::Height)
    );
    app.world_mut().gui_mut(owner).unwrap().set_height(240.);
    app.world_mut().gui_mut(bubble).unwrap().set_height(121.);
    app.update();
    let gui = queries.get(app.world()).unwrap();
    assert_eq!(gui.transform(owner).unwrap().size().y, 160.);
    assert_eq!(app.world().get::<Node>(view).unwrap().height, Val::Px(160.));
    app.world_mut().gui_mut(owner).unwrap().set_height(300.);
    app.world_mut().gui_mut(bubble).unwrap().set_height(61.);
    app.update();
    let gui = queries.get(app.world()).unwrap();
    assert_eq!(gui.transform(owner).unwrap().size().y, 100.);
    assert_eq!(app.world().get::<Node>(view).unwrap().height, Val::Px(100.));
}

#[test]
fn direct_edit_services_reject_replaced_removed_or_despawned_owner_aliases_before_sync() {
    for window in [false, true] {
        for change in 0..5 {
            let (mut app, asset) = app();
            let owner = if window {
                app.world_mut()
                    .spawn(FairyGuiWindow::new(asset.clone(), "Button1"))
                    .id()
            } else {
                app.world_mut()
                    .spawn(FairyGuiRoot::new(asset.clone(), "Button1"))
                    .id()
            };
            app.update();
            let old = if window {
                app.world()
                    .non_send::<GuiRuntime>()
                    .ecs_window_object(owner)
                    .unwrap()
            } else {
                app.world()
                    .non_send::<GuiRuntime>()
                    .ecs_root_object(owner)
                    .unwrap()
            };
            let view = app.world().non_send::<GuiRuntime>().entity_of(old).unwrap();
            let original = app
                .world()
                .non_send::<GuiRuntime>()
                .store()
                .object_text(old)
                .unwrap()
                .to_owned();
            match change {
                0 => {
                    if window {
                        app.world_mut()
                            .get_mut::<FairyGuiWindow>(owner)
                            .unwrap()
                            .item_name = "Button2".into();
                    } else {
                        app.world_mut()
                            .get_mut::<FairyGuiRoot>(owner)
                            .unwrap()
                            .item_name = "Button2".into();
                    }
                }
                1 => {
                    if window {
                        app.world_mut().entity_mut(owner).remove::<FairyGuiWindow>();
                    } else {
                        app.world_mut().entity_mut(owner).remove::<FairyGuiRoot>();
                    }
                }
                2 => {
                    app.world_mut().despawn(owner);
                }
                3 => {
                    let pending = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
                    if window {
                        app.world_mut()
                            .get_mut::<FairyGuiWindow>(owner)
                            .unwrap()
                            .asset = pending;
                    } else {
                        app.world_mut()
                            .get_mut::<FairyGuiRoot>(owner)
                            .unwrap()
                            .asset = pending;
                    }
                }
                _ => {
                    if window {
                        let source = app.world().get::<FairyGuiWindow>(owner).unwrap().clone();
                        app.world_mut()
                            .entity_mut(owner)
                            .remove::<FairyGuiWindow>()
                            .insert(source);
                    } else {
                        let source = app.world().get::<FairyGuiRoot>(owner).unwrap().clone();
                        app.world_mut()
                            .entity_mut(owner)
                            .remove::<FairyGuiRoot>()
                            .insert(source);
                    }
                }
            }
            assert!(
                app.world_mut().gui_mut(owner).is_none(),
                "window={window}, change={change}"
            );
            assert!(
                !app.world_mut()
                    .gui_add_relation(view, owner, RelationType::Size, false)
            );
            let mut service = SystemState::<GuiMut>::new(app.world_mut());
            let mut edit = service.get_mut(app.world_mut()).unwrap();
            assert!(edit.get_mut(owner).is_none());
            assert!(!edit.edit(owner, |object| object.set_text("wrong old template")));
            assert!(!edit.add_relation(view, owner, RelationType::Size, false));
            assert_eq!(
                app.world()
                    .non_send::<GuiRuntime>()
                    .store()
                    .object_text(old),
                Some(original.as_str())
            );
            // A still-materialized old view remains an explicit, stable handle
            // until scheduled lifecycle cleanup; only the owner alias is invalid.
            assert!(app.world_mut().gui_mut(view).is_some());
            app.update();
            assert!(!app.world().non_send::<GuiRuntime>().store().contains(old));
            assert!(app.world_mut().gui_mut(view).is_none());
            if change == 0 || change == 4 {
                assert!(app.world_mut().gui_mut(owner).is_some());
            } else {
                assert!(app.world_mut().gui_mut(owner).is_none());
            }
        }
    }
}

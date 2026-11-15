use crate::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};

fn fixture() -> (App, Entity, Vec<Entity>) {
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
            package: crate::core::FuiPackage::parse(
                "Basics",
                include_bytes!("../../fairygui-demo/assets/ui/Basics.fui"),
            )
            .unwrap(),
            res_key: "ui/Basics".into(),
        });
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "Demo_Button"))
        .id();
    app.update();
    let children = SystemState::<GuiQuery>::new(app.world_mut())
        .get(app.world())
        .unwrap()
        .get(owner)
        .unwrap()
        .children()
        .to_vec();
    (app, owner, children)
}

fn assert_order(app: &mut App, owner: Entity, expected: &[Entity]) {
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    let gui = query.get(app.world()).unwrap();
    assert_eq!(gui.get(owner).unwrap().children(), expected);
    for (index, &child) in expected.iter().enumerate() {
        assert_eq!(gui.child_index(child), Some(index));
        assert_eq!(
            app.world().get::<ZIndex>(child),
            Some(&ZIndex(index as i32))
        );
        let native = app.world().non_send::<GuiRuntime>().store();
        let object = app.world().get::<GObjectRef>(child).unwrap().0;
        assert_eq!(native.child_index(object), Some(index));
        assert_eq!(
            gui.sorting_order(child),
            Some(native.object(object).unwrap().sorting_order())
        );
    }
}

fn mounted_siblings() -> (App, Entity, Entity, Entity) {
    let (mut app, parent, _) = fixture();
    let asset = app
        .world()
        .get::<FairyGuiRoot>(parent)
        .unwrap()
        .asset
        .clone();
    let first = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), "Button1").with_parent(parent))
        .id();
    let second = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "Button1").with_parent(parent))
        .id();
    app.update();
    (app, parent, first, second)
}

fn swap(app: &mut App, first: Entity, second: Entity, system_param: bool) -> bool {
    if system_param {
        SystemState::<GuiMut>::new(app.world_mut())
            .get_mut(app.world_mut())
            .unwrap()
            .swap_children(first, second)
    } else {
        app.world_mut().gui_swap_children(first, second)
    }
}

#[test]
fn swapping_rejects_stale_owners_and_missing_view_references_before_sync() {
    for system_param in [false, true] {
        for change in 0..6 {
            let (mut app, _, first, second) = mounted_siblings();
            let first_view = app.world().get::<GuiRootView>(first).unwrap().0;
            let second_view = app.world().get::<GuiRootView>(second).unwrap().0;
            let first_index = app.world_mut().gui_mut(first).unwrap().child_index();
            let second_index = app.world_mut().gui_mut(second).unwrap().child_index();
            let target = if change < 4 { second } else { second_view };
            match change {
                0 => {
                    app.world_mut()
                        .get_mut::<FairyGuiRoot>(second)
                        .unwrap()
                        .item_name = "Button2".into()
                }
                1 => {
                    app.world_mut().entity_mut(second).remove::<FairyGuiRoot>();
                }
                2 => {
                    app.world_mut().despawn(second);
                }
                3 => {
                    let pending = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
                    app.world_mut()
                        .get_mut::<FairyGuiRoot>(second)
                        .unwrap()
                        .asset = pending;
                }
                4 => {
                    app.world_mut()
                        .entity_mut(second_view)
                        .remove::<GObjectRef>();
                }
                _ => {
                    app.world_mut().despawn(second_view);
                }
            }
            assert!(
                !swap(&mut app, first, target, system_param),
                "change={change}"
            );
            assert!(
                !swap(&mut app, target, first, system_param),
                "change={change}"
            );
            assert_eq!(
                app.world_mut().gui_mut(first).unwrap().child_index(),
                first_index
            );
            if change < 4 {
                // Explicit old views retain their identity until lifecycle cleanup.
                assert_eq!(
                    app.world_mut().gui_mut(second_view).unwrap().child_index(),
                    second_index
                );
                assert!(swap(&mut app, first_view, second_view, system_param));
            }
        }
    }
}

#[test]
fn entity_swaps_publish_retained_order_and_accept_owner_view_combinations() {
    let (mut app, parent, first, second) = mounted_siblings();
    let first_view = app.world().get::<GuiRootView>(first).unwrap().0;
    let second_view = app.world().get::<GuiRootView>(second).unwrap().0;
    let mut expected = SystemState::<GuiQuery>::new(app.world_mut())
        .get(app.world())
        .unwrap()
        .get(parent)
        .unwrap()
        .children()
        .to_vec();
    for system_param in [false, true] {
        for (a, b) in [
            (first, second),
            (first_view, second),
            (first, second_view),
            (first_view, second_view),
        ] {
            assert!(swap(&mut app, a, b, system_param));
            let index_a = expected.iter().position(|&e| e == first_view).unwrap();
            let index_b = expected.iter().position(|&e| e == second_view).unwrap();
            expected.swap(index_a, index_b);
            // The model changes synchronously, while ECS snapshots publish on Sync.
            assert_eq!(
                app.world_mut().gui_mut(first).unwrap().child_index(),
                Some(index_b)
            );
            app.update();
            assert_order(&mut app, parent, &expected);
            assert_eq!(app.world().get::<GuiRootView>(first).unwrap().0, first_view);
            assert_eq!(
                app.world().get::<GuiRootView>(second).unwrap().0,
                second_view
            );
        }
        assert!(!swap(&mut app, first, first_view, system_param));
        assert!(!swap(&mut app, first, parent, system_param));
        assert!(!swap(&mut app, Entity::PLACEHOLDER, first, system_param));
        let pending = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
        let unready = app
            .world_mut()
            .spawn(FairyGuiRoot::new(pending, "Button1").with_parent(parent))
            .id();
        assert!(!swap(&mut app, first, unready, system_param));
        assert_order(&mut app, parent, &expected);
    }
    // Native sorted children retain their slots even for an accepted entity pair.
    app.world_mut().entity_mut(first).insert(GuiSortingOrder(7));
    app.world_mut()
        .entity_mut(second)
        .insert(GuiSortingOrder(9));
    app.update();
    expected.retain(|&view| view != first_view && view != second_view);
    expected.extend([first_view, second_view]);
    assert_order(&mut app, parent, &expected);
    for system_param in [false, true] {
        assert!(!swap(&mut app, first, second, system_param));
    }
    assert_order(&mut app, parent, &expected);
}

#[test]
fn sorting_components_and_direct_indices_keep_logical_order_and_paint_order_in_sync() {
    let (mut app, owner, mut expected) = fixture();
    let a = expected[0];
    let b = expected[1];
    app.world_mut().get_mut::<GuiSortingOrder>(a).unwrap().0 = 5;
    app.update();
    expected.remove(0);
    expected.push(a);
    assert_order(&mut app, owner, &expected);
    assert_eq!(
        app.world_mut().gui_mut(a).unwrap().set_child_index(0),
        Some(expected.len() - 1)
    );
    app.world_mut()
        .gui_mut(b)
        .unwrap()
        .set_child_index(usize::MAX);
    app.update();
    expected.remove(0);
    expected.insert(expected.len() - 1, b);
    assert_order(&mut app, owner, &expected);
    app.world_mut().gui_mut(b).unwrap().set_sorting_order(5);
    app.update();
    expected.remove(expected.len() - 2);
    expected.push(b);
    assert_order(&mut app, owner, &expected);
    app.world_mut().get_mut::<GuiSortingOrder>(a).unwrap().0 = -20;
    app.update();
    assert_eq!(
        app.world().get::<GuiSortingOrder>(a),
        Some(&GuiSortingOrder(0))
    );
    assert_order(&mut app, owner, &expected);
    let tick = app
        .world()
        .entity(a)
        .get_ref::<GuiSortingOrder>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .entity(a)
            .get_ref::<GuiSortingOrder>()
            .unwrap()
            .last_changed(),
        tick
    );
    assert!(app.world_mut().gui_swap_children(a, expected[0]));
    let last_ordinary = expected.len() - 2;
    expected.swap(0, last_ordinary);
    app.update();
    assert_order(&mut app, owner, &expected);
    assert!(!app.world_mut().gui_swap_children(a, owner));
}

#[test]
fn direct_native_sorting_edits_are_published_without_stale_components_replaying() {
    let (mut app, owner, mut expected) = fixture();
    let first = expected.remove(0);
    let object = app.world().get::<GObjectRef>(first).unwrap().0;
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_sorting_order(object, 10);
    app.update();
    expected.push(first);
    assert_order(&mut app, owner, &expected);
    assert_eq!(
        app.world().get::<GuiSortingOrder>(first),
        Some(&GuiSortingOrder(10))
    );
    app.update();
    assert_order(&mut app, owner, &expected);
    app.world_mut().gui_mut(first).unwrap().set_sorting_order(0);
    app.world_mut()
        .gui_mut(first)
        .unwrap()
        .set_child_index_before(1);
    app.update();
    expected.pop();
    expected.insert(1, first);
    assert_order(&mut app, owner, &expected);
}

#[test]
fn root_sorting_properties_wait_for_assets_and_remain_effective_across_parent_moves() {
    let (mut app, parent, _) = fixture();
    let asset = app
        .world()
        .get::<FairyGuiRoot>(parent)
        .unwrap()
        .asset
        .clone();
    let pending = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
    let child = app
        .world_mut()
        .spawn((
            FairyGuiRoot::new(pending.clone(), "Button1").with_parent(parent),
            GuiSortingOrder(9),
        ))
        .id();
    let other = app
        .world_mut()
        .spawn((
            FairyGuiRoot::new(asset, "Button1").with_parent(parent),
            GuiSortingOrder(2),
        ))
        .id();
    for _ in 0..3 {
        app.update();
    }
    assert!(app.world().get::<GuiRootView>(child).is_none());
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(
            pending.id(),
            FuiAsset {
                package: crate::core::FuiPackage::parse(
                    "Basics",
                    include_bytes!("../../fairygui-demo/assets/ui/Basics.fui"),
                )
                .unwrap(),
                res_key: "ui/DelayedBasics".into(),
            },
        )
        .unwrap();
    app.update();
    let child_view = app.world().get::<GuiRootView>(child).unwrap().0;
    let other_view = app.world().get::<GuiRootView>(other).unwrap().0;
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    let gui = query.get(app.world()).unwrap();
    let children = gui.get(parent).unwrap().children();
    assert_eq!(&children[children.len() - 2..], &[other_view, child_view]);
    assert_eq!(gui.sorting_order(child), Some(9));
    app.world_mut()
        .get_mut::<GuiSortingOrder>(child_view)
        .unwrap()
        .0 = 1;
    app.update();
    assert_eq!(
        app.world().get::<GuiSortingOrder>(child),
        Some(&GuiSortingOrder(1))
    );
    app.world_mut()
        .get_mut::<FairyGuiRoot>(child)
        .unwrap()
        .parent = Some(other_view);
    app.update();
    assert_eq!(
        app.world().get::<ChildOf>(child_view).unwrap().parent(),
        other_view
    );
    app.world_mut()
        .get_mut::<FairyGuiRoot>(child)
        .unwrap()
        .parent = Some(parent);
    app.update();
    assert_eq!(app.world().get::<GuiRootView>(child).unwrap().0, child_view);
    assert_eq!(
        app.world().get::<GuiSortingOrder>(child_view),
        Some(&GuiSortingOrder(1))
    );
}

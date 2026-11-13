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
    assert!(app.world_mut().gui_mut(a).unwrap().swap_with(expected[0]));
    let last_ordinary = expected.len() - 2;
    expected.swap(0, last_ordinary);
    app.update();
    assert_order(&mut app, owner, &expected);
    assert!(!app.world_mut().gui_mut(a).unwrap().swap_with(owner));
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

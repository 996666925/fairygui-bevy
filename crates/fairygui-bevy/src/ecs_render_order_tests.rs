use crate::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};

fn fixture(item: &str, loaded: bool) -> (App, Entity, Handle<FuiAsset>) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let asset = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), item))
        .id();
    if loaded {
        load(&mut app, &asset);
        app.update();
    }
    (app, owner, asset)
}
fn load(app: &mut App, asset: &Handle<FuiAsset>) {
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(
            asset.id(),
            FuiAsset {
                package: crate::core::FuiPackage::parse(
                    "Basics",
                    include_bytes!("../../fairygui-demo/assets/ui/Basics.fui"),
                )
                .unwrap(),
                res_key: "ui/Basics".into(),
            },
        )
        .unwrap();
}
fn paint(app: &App, children: &[Entity]) -> Vec<Entity> {
    let mut children = children.to_vec();
    children.sort_by_key(|&e| app.world().get::<ZIndex>(e).unwrap().0);
    children
}

#[test]
fn paint_modes_change_z_indices_without_changing_logical_children_or_selection() {
    let (mut app, owner, _) = fixture("Demo_Button", true);
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    let children = app
        .world()
        .get::<GuiObjectInfo>(view)
        .unwrap()
        .children()
        .to_vec();
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    let tab = query
        .get(app.world())
        .unwrap()
        .child_by_name(owner, "n23")
        .unwrap();
    assert!(app.world().get::<GuiButtonSelected>(tab).unwrap().0);
    for (order, apex) in [
        (ChildrenRenderOrder::Descent, 0),
        (ChildrenRenderOrder::Arch, 2),
        (ChildrenRenderOrder::Arch, -5),
        (ChildrenRenderOrder::Arch, i32::MAX),
        (ChildrenRenderOrder::Ascent, 0),
    ] {
        *app.world_mut().get_mut::<GuiRenderOrder>(view).unwrap() = GuiRenderOrder {
            order,
            apex_index: apex,
        };
        app.update();
        let mut expected = children.clone();
        match order {
            ChildrenRenderOrder::Ascent => {}
            ChildrenRenderOrder::Descent => expected.reverse(),
            ChildrenRenderOrder::Arch => {
                let apex = (apex.max(0) as usize).min(expected.len());
                expected[apex..].reverse();
            }
        }
        assert_eq!(paint(&app, &children), expected);
        assert_eq!(
            app.world().get::<GuiObjectInfo>(view).unwrap().children(),
            children
        );
        assert_eq!(
            query.get(app.world()).unwrap().render_order(owner),
            Some(&GuiRenderOrder {
                order,
                apex_index: apex
            })
        );
        assert!(app.world().get::<GuiButtonSelected>(tab).unwrap().0);
    }
}

#[test]
fn render_properties_wait_for_assets_and_apply_to_the_first_view() {
    #[derive(Resource, Default)]
    struct Seen(bool);
    fn inspect(
        roots: Query<&GuiRootView, Added<GuiRootView>>,
        gui: GuiQuery,
        z: Query<&ZIndex>,
        mut seen: ResMut<Seen>,
    ) {
        for view in &roots {
            let children = gui.get(view.0).unwrap().children();
            assert_eq!(
                gui.render_order(view.0).unwrap().order,
                ChildrenRenderOrder::Descent
            );
            assert!(z.get(children[0]).unwrap().0 > z.get(children[1]).unwrap().0);
            seen.0 = true;
        }
    }
    let (mut app, owner, asset) = fixture("Demo_Button", false);
    app.init_resource::<Seen>()
        .add_systems(Update, inspect.in_set(GuiSystems::Initialize));
    app.world_mut().entity_mut(owner).insert(GuiRenderOrder {
        order: ChildrenRenderOrder::Descent,
        apex_index: 2,
    });
    for _ in 0..4 {
        app.update();
    }
    assert!(!app.world().resource::<Seen>().0);
    load(&mut app, &asset);
    app.update();
    assert!(app.world().resource::<Seen>().0);
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    let tick = app
        .world()
        .entity(view)
        .get_ref::<GuiRenderOrder>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .entity(view)
            .get_ref::<GuiRenderOrder>()
            .unwrap()
            .last_changed(),
        tick
    );
    assert_eq!(
        app.world().get::<GuiRenderOrder>(owner),
        app.world().get::<GuiRenderOrder>(view)
    );
}

#[test]
fn untouched_mode_and_apex_fields_follow_native_edits_instead_of_replaying_stale_values() {
    let (mut app, owner, _) = fixture("Demo_Button", true);
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    app.world_mut()
        .gui_mut(view)
        .unwrap()
        .set_children_render_order(ChildrenRenderOrder::Descent);
    app.world_mut()
        .get_mut::<GuiRenderOrder>(view)
        .unwrap()
        .apex_index = 42;
    app.update();
    assert_eq!(
        app.world().get::<GuiRenderOrder>(view),
        Some(&GuiRenderOrder {
            order: ChildrenRenderOrder::Descent,
            apex_index: 42
        })
    );
    app.world_mut().gui_mut(view).unwrap().set_apex_index(2);
    app.world_mut()
        .get_mut::<GuiRenderOrder>(view)
        .unwrap()
        .order = ChildrenRenderOrder::Arch;
    app.update();
    assert_eq!(
        app.world().get::<GuiRenderOrder>(view),
        Some(&GuiRenderOrder {
            order: ChildrenRenderOrder::Arch,
            apex_index: 2
        })
    );
    let children = app
        .world()
        .get::<GuiObjectInfo>(view)
        .unwrap()
        .children()
        .to_vec();
    assert_eq!(paint(&app, &children).last(), Some(&children[2]));
    app.world_mut().despawn(owner);
    app.update();
    assert!(app.world().get_entity(view).is_err());
}

#[test]
fn virtual_arch_apex_feedback_follows_scroll_and_preserves_item_identity() {
    let (mut app, owner, asset) = fixture("Main", false);
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(
            asset.id(),
            FuiAsset {
                package: crate::core::FuiPackage::parse(
                    "LoopList",
                    include_bytes!("../../fairygui-demo/assets/ui/LoopList.fui"),
                )
                .unwrap(),
                res_key: "ui/LoopList".into(),
            },
        )
        .unwrap();
    app.update();
    let list = SystemState::<GuiQuery>::new(app.world_mut())
        .get(app.world())
        .unwrap()
        .child_by_name(owner, "list")
        .unwrap();
    app.world_mut().entity_mut(list).insert((
        GuiVirtualList::new(5, |_, _| {}).looping(),
        GuiRenderOrder {
            order: ChildrenRenderOrder::Arch,
            apex_index: 99,
        },
    ));
    app.update();
    let object = app.world().get::<GObjectRef>(list).unwrap().0;
    for x in [0., 100., 280., 390.] {
        app.world_mut().gui_mut(list).unwrap().set_apex_index(99);
        app.world_mut()
            .gui_mut(list)
            .unwrap()
            .scroll_to(Vec2::new(x, 0.), false);
        app.update();
        let store = app.world().non_send::<GuiRuntime>().store();
        assert!(store.virtual_list_info(object).unwrap().looping);
        let apex = store.apex_index(object).unwrap();
        let rows = store.list_items(object).unwrap();
        assert!((0..rows.len() as i32).contains(&apex));
        assert_eq!(
            app.world().get::<GuiRenderOrder>(list),
            Some(&GuiRenderOrder {
                order: ChildrenRenderOrder::Arch,
                apex_index: apex
            })
        );
        let expected_top = app
            .world()
            .non_send::<GuiRuntime>()
            .entity_of(rows[apex as usize])
            .unwrap();
        let views: Vec<_> = rows
            .iter()
            .map(|&id| app.world().non_send::<GuiRuntime>().entity_of(id).unwrap())
            .collect();
        assert_eq!(paint(&app, &views).last(), Some(&expected_top));
        for &row in &rows {
            assert!(store.virtual_item_index(object, row).unwrap() < 5);
        }
    }
}

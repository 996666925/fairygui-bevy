use super::*;
use crate::prelude::*;
use bevy::ecs::system::SystemState;

fn fixture(package: &str, bytes: &[u8], child: &str) -> (App, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: crate::core::FuiPackage::parse(package, bytes).unwrap(),
            res_key: format!("ui/{package}"),
        });
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(handle, "Main"))
        .id();
    app.update();
    let mut queries = SystemState::<GuiQuery>::new(app.world_mut());
    let list = queries
        .get(app.world())
        .unwrap()
        .child_by_name(owner, child)
        .unwrap();
    (app, owner, list)
}
fn chat() -> (App, Entity, Entity) {
    fixture(
        "Chat",
        include_bytes!("../../fairygui-demo/assets/ui/Chat.fui"),
        "list",
    )
}
fn config(count: usize) -> GuiVirtualList {
    GuiVirtualList::new(count, |index, row| {
        row.set_size(Vec2::new(900., 100.));
        if let Some(mut text) = row.child_by_path("msg") {
            text.set_text(&format!("Message {index}"));
        }
    })
    .with_item_provider(|index| match index % 4 {
        0 => Some("chatLeft".into()),
        1 => Some("ui://Chat/chatRight".into()),
        2 => None,
        _ => Some(String::new()),
    })
}
fn rows(app: &App, list: Entity) -> Vec<Entity> {
    app.world()
        .get::<GuiListState>(list)
        .unwrap()
        .items()
        .to_vec()
}
fn assert_row(app: &App, row: Entity, item: &str, package: &str) {
    let id = app.world().get::<GObjectRef>(row).unwrap().0;
    let runtime = app.world().non_send::<GuiRuntime>();
    let object = runtime.store().object(id).unwrap();
    assert_eq!(object.package_item_id(), Some(item));
    assert_eq!(object.source_package_id(), Some(package));
    assert_eq!(runtime.entity_of(id), Some(row));
}
fn verify_chat_rows(app: &mut App, list: Entity, left: impl Fn(usize) -> bool) {
    let mut queries = SystemState::<GuiQuery>::new(app.world_mut());
    let gui = queries.get(app.world()).unwrap();
    let state = gui.list(list).unwrap();
    assert!(!state.items().is_empty());
    for &row in state.items() {
        let binding = app.world().get::<GuiListRow>(row).unwrap();
        let index = binding.index();
        assert_row(
            app,
            row,
            if left(index) { "kfcb1h" } else { "kfcb1j" },
            "y768eypa",
        );
        let text = gui.child_by_name(row, "msg").unwrap();
        assert_eq!(gui.text(text), Some(format!("Message {index}").as_str()));
    }
}
fn select(app: &mut App, list: Entity, index: usize) {
    app.world_mut().write_message(GuiObjectCommand::new(
        list,
        GuiObjectAction::SelectListIndex {
            index,
            scroll_to_view: true,
        },
    ));
    app.update();
}

#[test]
fn ecs_provider_uses_native_templates_aliases_defaults_recycling_and_common_button_rules() {
    let (mut app, owner, list) = chat();
    app.world_mut().entity_mut(list).insert(config(80));
    app.update();
    verify_chat_rows(&mut app, list, |index| index % 4 == 0);
    let first = rows(&app, list);
    assert!(first.len() < 10);
    let membership: Vec<_> = first
        .iter()
        .map(|&row| (row, app.world().get::<GuiListRow>(row).unwrap().index()))
        .collect();
    select(&mut app, list, 1);
    assert!(
        app.world()
            .get::<crate::GuiListSelection>(list)
            .unwrap()
            .0
            .is_empty(),
        "native Common buttons do not acquire selection"
    );
    app.world_mut()
        .get_mut::<GuiVirtualList>(list)
        .unwrap()
        .set_item_provider(|index| {
            Some(
                if index % 4 == 0 {
                    "ui://Chat/chatLeft"
                } else {
                    "ui://y768eypakfcb1j"
                }
                .into(),
            )
        });
    app.update();
    verify_chat_rows(&mut app, list, |index| index % 4 == 0);
    assert_eq!(
        rows(&app, list),
        first,
        "aliases normalize before pool/template comparison"
    );
    app.world_mut()
        .get_mut::<GuiVirtualList>(list)
        .unwrap()
        .set_item_provider(|index| {
            Some(
                if index % 2 == 0 {
                    "chatRight"
                } else {
                    "chatLeft"
                }
                .into(),
            )
        });
    app.update();
    verify_chat_rows(&mut app, list, |index| index % 2 != 0);
    assert!(
        app.world()
            .get::<crate::GuiListSelection>(list)
            .unwrap()
            .0
            .is_empty()
    );
    assert!(membership.iter().any(|&(row, index)| {
        index == 0
            && app
                .world()
                .get::<GuiListRow>(row)
                .is_none_or(|current| current.index() != 0)
    }));
    let mut all = first;
    all.extend(rows(&app, list));
    app.world_mut().write_message(GuiObjectCommand::new(
        list,
        GuiObjectAction::ScrollTo {
            position: Vec2::new(0., 6000.),
            animated: false,
        },
    ));
    app.update();
    verify_chat_rows(&mut app, list, |index| index % 2 != 0);
    assert!(
        rows(&app, list)
            .iter()
            .all(|&row| app.world().get::<GuiListRow>(row).unwrap().index() >= 55)
    );
    assert!(
        rows(&app, list).iter().any(|row| all.contains(row)),
        "same-template views must recycle"
    );
    all.extend(rows(&app, list));
    app.world_mut()
        .get_mut::<GuiVirtualList>(list)
        .unwrap()
        .clear_item_provider();
    app.update();
    verify_chat_rows(&mut app, list, |_| false);
    app.world_mut().write_message(GuiObjectCommand::new(
        list,
        GuiObjectAction::ScrollTo {
            position: Vec2::ZERO,
            animated: false,
        },
    ));
    app.update();
    verify_chat_rows(&mut app, list, |_| false);
    let state_tick = app
        .world()
        .entity(list)
        .get_ref::<GuiVirtualListState>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .entity(list)
            .get_ref::<GuiVirtualListState>()
            .unwrap()
            .last_changed(),
        state_tick
    );
    all.extend(rows(&app, list));
    app.world_mut().entity_mut(owner).despawn();
    app.update();
    assert!(all.iter().all(|&row| app.world().get_entity(row).is_err()));
}

#[test]
fn ecs_provider_preserves_selectable_row_indices_across_template_changes_and_recycling() {
    let (mut app, _, list) = fixture(
        "VirtualList",
        include_bytes!("../../fairygui-demo/assets/ui/VirtualList.fui"),
        "mailList",
    );
    let foreign = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: crate::core::FuiPackage::parse(
                "ListEffect",
                include_bytes!("../../fairygui-demo/assets/ui/ListEffect.fui"),
            )
            .unwrap(),
            res_key: "ui/ListEffect".into(),
        });
    let mut root = FairyGuiRoot::new(foreign, "Main");
    root.active = false;
    app.world_mut().spawn(root);
    app.world_mut().entity_mut(list).insert(
        GuiVirtualList::new(80, |index, row| {
            row.set_text(&format!("Mail {index}"));
            row.set_size(Vec2::new(400., 60.));
        })
        .with_item_provider(|index| (index % 2 == 1).then(|| "ui://ListEffect/mailItem".into())),
    );
    app.update();
    select(&mut app, list, 1);
    assert_eq!(
        app.world().get::<crate::GuiListSelection>(list).unwrap().0,
        [1]
    );
    app.world_mut()
        .get_mut::<GuiVirtualList>(list)
        .unwrap()
        .set_item_provider(|index| (index % 2 == 0).then(|| "ui://ListEffect/mailItem".into()));
    app.update();
    assert_eq!(
        app.world().get::<crate::GuiListSelection>(list).unwrap().0,
        [1]
    );
    let selected = rows(&app, list)
        .into_iter()
        .find(|&row| app.world().get::<GuiListRow>(row).unwrap().index() == 1)
        .unwrap();
    assert_row(&app, selected, "c8s24", "qkteqwfp");
    assert!(app.world().get::<GuiButtonSelected>(selected).unwrap().0);
    app.world_mut().write_message(GuiObjectCommand::new(
        list,
        GuiObjectAction::ScrollTo {
            position: Vec2::new(0., 3000.),
            animated: false,
        },
    ));
    app.update();
    assert_eq!(
        app.world().get::<crate::GuiListSelection>(list).unwrap().0,
        [1]
    );
    app.world_mut().write_message(GuiObjectCommand::new(
        list,
        GuiObjectAction::ScrollTo {
            position: Vec2::ZERO,
            animated: false,
        },
    ));
    app.update();
    assert_eq!(
        app.world().get::<crate::GuiListSelection>(list).unwrap().0,
        [1]
    );
}

#[test]
fn ecs_provider_retries_late_packages_and_clears_current_resource_errors() {
    let (mut app, _, list) = chat();
    app.world_mut()
        .entity_mut(list)
        .insert(config(50).with_item_provider(|_| Some("ui://VirtualList/mailItem".into())));
    app.update();
    assert!(matches!(
        app.world()
            .get::<GuiVirtualListState>(list)
            .unwrap()
            .error(),
        Some(VirtualListError::MissingResource(_))
    ));
    assert!(rows(&app, list).is_empty());
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: crate::core::FuiPackage::parse(
                "VirtualList",
                include_bytes!("../../fairygui-demo/assets/ui/VirtualList.fui"),
            )
            .unwrap(),
            res_key: "foreign/VirtualList".into(),
        });
    let mut source = FairyGuiRoot::new(asset, "Main");
    source.active = false;
    app.world_mut().spawn(source);
    app.update();
    assert!(
        app.world()
            .get::<GuiVirtualListState>(list)
            .unwrap()
            .error()
            .is_none()
    );
    assert!(!rows(&app, list).is_empty());
    for row in rows(&app, list) {
        assert_row(&app, row, "c8s24", "qkteqwfp");
    }
    // A provider can succeed initially and only fail when scrolling reaches a
    // new template. The live refresh error must update the unchanged binding.
    app.world_mut().write_message(GuiObjectCommand::new(
        list,
        GuiObjectAction::ScrollTo {
            position: Vec2::ZERO,
            animated: false,
        },
    ));
    app.world_mut()
        .get_mut::<GuiVirtualList>(list)
        .unwrap()
        .set_item_provider(|index| (index >= 10).then(|| "ui://missing/row".into()));
    app.update();
    assert!(
        app.world()
            .get::<GuiVirtualListState>(list)
            .unwrap()
            .error()
            .is_none()
    );
    app.world_mut().write_message(GuiObjectCommand::new(
        list,
        GuiObjectAction::ScrollTo {
            position: Vec2::new(0., 3000.),
            animated: false,
        },
    ));
    app.update();
    assert!(matches!(
        app.world()
            .get::<GuiVirtualListState>(list)
            .unwrap()
            .error(),
        Some(VirtualListError::MissingResource(_))
    ));
    app.world_mut().write_message(GuiObjectCommand::new(
        list,
        GuiObjectAction::ScrollTo {
            position: Vec2::ZERO,
            animated: false,
        },
    ));
    app.update();
    assert!(
        app.world()
            .get::<GuiVirtualListState>(list)
            .unwrap()
            .error()
            .is_none()
    );
    // A changed provider can fail immediately too, then clear on correction.
    app.world_mut()
        .get_mut::<GuiVirtualList>(list)
        .unwrap()
        .set_item_provider(|_| Some("ui://missing/row".into()));
    app.update();
    assert!(matches!(
        app.world()
            .get::<GuiVirtualListState>(list)
            .unwrap()
            .error(),
        Some(VirtualListError::MissingResource(_))
    ));
    app.world_mut()
        .get_mut::<GuiVirtualList>(list)
        .unwrap()
        .clear_item_provider();
    app.update();
    assert!(
        app.world()
            .get::<GuiVirtualListState>(list)
            .unwrap()
            .error()
            .is_none()
    );
    verify_chat_rows(&mut app, list, |_| false);
}

#[test]
fn ecs_provider_receives_logical_loop_indices_and_drops_with_its_component() {
    let (mut app, owner, list) = fixture(
        "LoopList",
        include_bytes!("../../fairygui-demo/assets/ui/LoopList.fui"),
        "list",
    );
    let token = Arc::new(());
    let weak = Arc::downgrade(&token);
    app.world_mut().entity_mut(list).insert(
        GuiVirtualList::new(5, |index, row| {
            assert!(index < 5);
            row.set_text(&index.to_string());
        })
        .looping()
        .with_item_provider(move |index| {
            let _keep_alive = &token;
            assert!(
                index < 5,
                "provider must never see sixfold internal record indices"
            );
            None
        }),
    );
    app.update();
    for percentage in [0., 1., 0.4, 0.] {
        app.world_mut().write_message(GuiObjectCommand::new(
            list,
            GuiObjectAction::SetScrollPercentage {
                percentage: Vec2::splat(percentage),
                animated: false,
            },
        ));
        app.update();
        let info = app
            .world()
            .get::<GuiVirtualListState>(list)
            .unwrap()
            .info()
            .unwrap();
        assert!(info.looping);
        assert_eq!(info.real_item_count, 30);
        assert!(!rows(&app, list).is_empty());
    }
    assert!(weak.upgrade().is_some());
    app.world_mut().entity_mut(list).remove::<GuiVirtualList>();
    app.update();
    assert!(weak.upgrade().is_none());
    assert!(rows(&app, list).is_empty());
    app.world_mut().entity_mut(owner).despawn();
    app.update();
}

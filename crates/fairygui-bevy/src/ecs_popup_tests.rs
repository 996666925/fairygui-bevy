use super::*;
use crate::GuiWorldExt as _;
use crate::{FairyGuiPlugin, GuiQuery, GuiRootView, GuiSystems};
use bevy::ecs::system::SystemState;
use fairygui_core::package::FuiPackage;

fn fixture() -> (App, Handle<FuiAsset>) {
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

fn load(app: &mut App, asset: &Handle<FuiAsset>, basics: bool) {
    let (name, bytes) = if basics {
        (
            "Basics",
            include_bytes!("../../fairygui-core/tests/fixtures/Basics.fui").as_slice(),
        )
    } else {
        (
            "MainMenu",
            include_bytes!("../../fairygui-core/tests/fixtures/MainMenu.fui").as_slice(),
        )
    };
    let package = FuiPackage::parse(name, bytes).unwrap();
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(
            asset.id(),
            FuiAsset {
                package,
                res_key: format!("ui/{name}"),
            },
        )
        .unwrap();
}

fn with_popup(app: &mut App, edit: impl FnOnce(&mut GuiPopup)) {
    let mut state = SystemState::<GuiPopup>::new(app.world_mut());
    edit(&mut state.get_mut(app.world_mut()).unwrap());
    state.apply(app.world_mut());
}
fn menu() -> GuiPopupMenu {
    GuiPopupMenu::new(
        "PopupMenu",
        "PopupMenuItem",
        vec![MenuItem {
            checkable: true,
            ..MenuItem::action("toggle", "Toggle", 73)
        }],
    )
}
fn show_menu(app: &mut App, source: Entity) {
    with_popup(app, |popup| {
        assert!(popup.show_menu(source, menu(), Vec2::new(30., 40.)));
    });
}

#[test]
fn anchored_popup_uses_live_anchor_geometry_directions_and_retained_owner_views() {
    let (mut app, asset) = fixture();
    load(&mut app, &asset, false);
    let anchor = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), "CloseButton"))
        .id();
    let mut source = FairyGuiRoot::new(asset, "CloseButton");
    source.active = false;
    let popup = app.world_mut().spawn(source).id();
    app.update();
    if let Some(mut object) = app.world_mut().gui_mut(anchor) {
        object.set_size(Vec2::new(40., 20.))
    }
    if let Some(mut object) = app.world_mut().gui_mut(popup) {
        object.set_size(Vec2::new(150., 70.))
    }
    let view = app.world().get::<GuiRootView>(popup).unwrap().0;
    // Native GRoot.showPopup branches: overflow shifts by anchor width minus
    // popup width; Up adds -1; an above-screen popup shifts x by half the anchor.
    for (direction, position, expected) in [
        (
            GuiPopupDirection::Down,
            Vec2::new(1200., 660.),
            Vec2::new(1090., 680.),
        ),
        (
            GuiPopupDirection::Up,
            Vec2::new(1200., 660.),
            Vec2::new(1090., 589.),
        ),
        (
            GuiPopupDirection::Auto,
            Vec2::new(1200., 700.),
            Vec2::new(1090., 629.),
        ),
        (
            GuiPopupDirection::Up,
            Vec2::new(20., 10.),
            Vec2::new(40., 0.),
        ),
        (
            GuiPopupDirection::Auto,
            Vec2::new(20., 10.),
            Vec2::new(20., 30.),
        ),
    ] {
        if let Some(mut object) = app.world_mut().gui_mut(anchor) {
            object.set_position(position)
        }
        with_popup(&mut app, |popups| {
            assert!(popups.show_anchored(popup, anchor, direction));
        });
        app.update();
        let mut query = SystemState::<GuiQuery>::new(app.world_mut());
        let gui = query.get(app.world()).unwrap();
        let actual = gui.transform(popup).unwrap().position();
        assert_eq!(actual, expected);
        assert_eq!(gui.view(popup), Some(view));
        assert!(
            app.world()
                .resource::<GuiPopupState>()
                .is_open(gui.view(popup).unwrap())
        );
    }
}

#[test]
fn ecs_popup_roots_wait_for_assets_retain_views_and_close_the_stack_above_a_target() {
    let (mut app, asset) = fixture();
    let first = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), "CloseButton"))
        .id();
    with_popup(&mut app, |popups| {
        assert!(popups.show(first, Vec2::new(21., 32.)));
    });
    for _ in 0..5 {
        app.update();
    }
    assert!(app.world().resource::<GuiPopupState>().stack().is_empty());
    load(&mut app, &asset, false);
    app.update();
    let first_view = app.world().get::<GuiRootView>(first).unwrap().0;
    assert_eq!(
        app.world().resource::<GuiPopupState>().stack(),
        &[first_view]
    );
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    assert_eq!(
        query
            .get(app.world())
            .unwrap()
            .transform(first)
            .unwrap()
            .position(),
        Vec2::new(21., 32.)
    );
    let second = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "CloseButton"))
        .id();
    with_popup(&mut app, |popups| {
        assert!(popups.show(second, Vec2::new(50., 60.)));
    });
    app.update();
    let second_view = app.world().get::<GuiRootView>(second).unwrap().0;
    assert_eq!(
        app.world().resource::<GuiPopupState>().stack(),
        &[first_view, second_view]
    );
    with_popup(&mut app, |popups| {
        assert!(popups.hide(first));
    });
    app.update();
    assert!(app.world().resource::<GuiPopupState>().stack().is_empty());
    assert!(query.get(app.world()).unwrap().view(first).is_some());
    assert!(
        !query
            .get(app.world())
            .unwrap()
            .display(first)
            .unwrap()
            .on_stage()
    );
    assert!(
        !query
            .get(app.world())
            .unwrap()
            .display(second)
            .unwrap()
            .on_stage()
    );
    with_popup(&mut app, |popups| {
        assert!(popups.show(first_view, Vec2::new(70., 80.)));
    });
    app.update();
    assert_eq!(
        app.world().resource::<GuiPopupState>().stack(),
        &[first_view]
    );
    app.world_mut().despawn(first);
    app.update();
    assert!(app.world().resource::<GuiPopupState>().stack().is_empty());
    assert!(query.get(app.world()).unwrap().get(first_view).is_none());
}

#[test]
fn ecs_popup_pending_requests_cancel_on_hide_replacement_removal_and_disposal() {
    for cancel in 0..7 {
        let (mut app, asset) = fixture();
        let owner = app
            .world_mut()
            .spawn(FairyGuiRoot::new(asset.clone(), "Demo_Popup"))
            .id();
        show_menu(&mut app, owner);
        app.update();
        match cancel {
            0 => with_popup(&mut app, |popups| popups.hide_all()),
            1 => with_popup(&mut app, |popups| {
                assert!(popups.hide(owner));
            }),
            2 => with_popup(&mut app, |popups| popups.dispose_menu()),
            3 => {
                app.world_mut()
                    .get_mut::<FairyGuiRoot>(owner)
                    .unwrap()
                    .item_name = "Demo_Text".into()
            }
            4 => {
                app.world_mut().entity_mut(owner).remove::<FairyGuiRoot>();
                app.world_mut()
                    .entity_mut(owner)
                    .insert(FairyGuiRoot::new(asset.clone(), "Demo_Popup"));
            }
            5 => {
                app.world_mut().despawn(owner);
            }
            _ => {
                app.world_mut()
                    .get_mut::<FairyGuiRoot>(owner)
                    .unwrap()
                    .asset = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
            }
        }
        // Cancellation applies while assets are still unavailable.
        app.update();
        load(&mut app, &asset, true);
        app.update();
        app.update();
        assert!(
            app.world().resource::<GuiPopupState>().menu().is_none(),
            "cancel scenario {cancel}"
        );
    }
}

#[test]
fn ecs_popup_menu_resolves_the_source_package_and_reuses_checked_rows_until_disposal() {
    let (mut app, asset) = fixture();
    let unrelated = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
    load(&mut app, &unrelated, false);
    app.world_mut().spawn(FairyGuiRoot::new(unrelated, "Main"));
    let source = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), "Demo_Popup"))
        .id();
    show_menu(&mut app, source);
    for _ in 0..4 {
        app.update();
    }
    load(&mut app, &asset, true);
    app.update();
    let content = app.world().resource::<GuiPopupState>().menu().unwrap();
    with_popup(&mut app, |popups| {
        assert!(popups.is_menu_open());
        assert!(!popups.show_menu(
            source,
            GuiPopupMenu::new("Missing", "PopupMenuItem", vec![]),
            Vec2::ZERO
        ));
        assert!(!popups.show_menu(source, menu(), Vec2::splat(f32::INFINITY)));
        assert!(popups.is_menu_open());
    });
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    assert!(query.get(app.world()).unwrap().get(content).is_some());
    let row = {
        let runtime = app.world().non_send::<GuiRuntime>();
        assert_eq!(
            runtime.asset_handle_for(app.world().get::<GObjectRef>(content).unwrap().0),
            Some(asset)
        );
        runtime.popup_menu_item("toggle").unwrap().1
    };
    let row_view = app.world().non_send::<GuiRuntime>().entity_of(row).unwrap();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .activate_menu_item(row, Some(Point::new(42., 84.)));
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .advance_menu_click(0.1);
    #[derive(Resource, Default)]
    struct Received(bool);
    fn receive(
        mut messages: MessageReader<crate::GuiMenuAction>,
        state: Res<GuiPopupState>,
        mut received: ResMut<Received>,
        mut popups: GuiPopup,
    ) {
        for action in messages.read() {
            assert!(state.menu().is_none());
            assert!(state.stack().is_empty());
            assert!(action.checked);
            assert_eq!(action.action, Some(73));
            received.0 = true;
            assert!(popups.reopen_menu(Vec2::new(90., 100.)));
        }
    }
    app.init_resource::<Received>()
        .add_systems(Update, receive.in_set(GuiSystems::Respond));
    app.update();
    assert!(app.world().resource::<Received>().0);
    assert_eq!(
        app.world().resource::<GuiPopupState>().menu(),
        Some(content)
    );
    assert_eq!(
        app.world().non_send::<GuiRuntime>().entity_of(row),
        Some(row_view)
    );
    assert_eq!(
        query
            .get(app.world())
            .unwrap()
            .transform(content)
            .unwrap()
            .position(),
        Vec2::new(90., 100.)
    );
    with_popup(&mut app, |popups| popups.hide_all());
    app.update();
    show_menu(&mut app, source);
    app.update();
    assert_eq!(
        app.world().resource::<GuiPopupState>().menu(),
        Some(content)
    );
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .popup_menu_item("toggle")
            .unwrap()
            .0
            .checked
    );
    let unchanged = app
        .world()
        .get_resource_ref::<GuiPopupState>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .get_resource_ref::<GuiPopupState>()
            .unwrap()
            .last_changed(),
        unchanged
    );
    with_popup(&mut app, |popups| {
        assert!(!popups.reopen_menu(Vec2::splat(f32::NAN)));
    });
    app.update();
    assert_eq!(
        app.world().resource::<GuiPopupState>().menu(),
        Some(content)
    );
    with_popup(&mut app, |popups| popups.dispose_menu());
    app.update();
    assert!(app.world().resource::<GuiPopupState>().menu().is_none());
    assert!(query.get(app.world()).unwrap().get(content).is_none());
    assert!(query.get(app.world()).unwrap().get(row_view).is_none());
}

#[test]
fn ready_popups_change_the_native_stack_immediately_and_reject_invalid_arguments() {
    let (mut app, asset) = fixture();
    load(&mut app, &asset, false);
    let first = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), "CloseButton"))
        .id();
    let second = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "CloseButton"))
        .id();
    app.update();
    let first_view = app.world().get::<GuiRootView>(first).unwrap().0;
    let second_view = app.world().get::<GuiRootView>(second).unwrap().0;
    let first_id = app.world().get::<GObjectRef>(first_view).unwrap().0;
    with_popup(&mut app, |popups| {
        assert!(popups.show(first, Vec2::ZERO));
    });
    app.update();
    let stage = app
        .world()
        .non_send::<GuiRuntime>()
        .root_container()
        .unwrap();
    let stage_view = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(stage)
        .unwrap();
    let unrelated = app.world_mut().spawn_empty().id();
    with_popup(&mut app, |popups| {
        assert!(popups.show(first, Vec2::new(15., 25.)));
        assert!(popups.is_open(first) && popups.is_open(first_view));
        assert!(popups.show(second_view, Vec2::new(35., 45.)));
        assert!(popups.is_open(second));
        assert!(!popups.show(first, Vec2::splat(f32::NAN)));
        assert!(!popups.show(unrelated, Vec2::ZERO));
        assert!(!popups.show(stage_view, Vec2::ZERO));
        assert!(!popups.show_anchored(second, unrelated, GuiPopupDirection::Up));
        assert!(popups.is_open(first) && popups.is_open(second));
        assert!(popups.hide(first));
        assert!(!popups.is_open(first) && !popups.is_open(second));
        assert!(!popups.hide(first));
    });
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .popup_stack()
            .is_empty()
    );
    assert!(
        !app.world()
            .non_send::<GuiRuntime>()
            .store()
            .is_on_stage(first_id)
    );
    app.update();
    assert!(app.world().resource::<GuiPopupState>().stack().is_empty());
    assert!(app.world().get_entity(first_view).is_ok());
}

#[test]
fn pending_shows_keep_only_latest_intent_and_new_requests_survive_owner_reinsertion() {
    let (mut app, asset) = fixture();
    let source = FairyGuiRoot::new(asset.clone(), "CloseButton");
    let owner = app.world_mut().spawn(source.clone()).id();
    with_popup(&mut app, |popups| {
        assert!(popups.show(owner, Vec2::splat(10.)));
        assert!(popups.is_pending(owner));
        assert!(popups.show(owner, Vec2::splat(20.)));
    });
    assert_eq!(app.world().non_send::<GuiRuntime>().pending_popups.len(), 1);
    app.update();
    app.world_mut()
        .entity_mut(owner)
        .remove::<FairyGuiRoot>()
        .insert(source);
    with_popup(&mut app, |popups| {
        assert!(popups.show(owner, Vec2::splat(30.)));
    });
    app.update();
    with_popup(&mut app, |popups| assert!(popups.is_pending(owner)));
    load(&mut app, &asset, false);
    app.update();
    with_popup(&mut app, |popups| {
        assert!(!popups.is_pending(owner));
        assert!(popups.is_open(owner));
    });
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    assert_eq!(
        app.world()
            .get::<crate::GuiTransform>(view)
            .unwrap()
            .position(),
        Vec2::splat(30.)
    );
}

#[test]
fn a_show_after_template_replacement_waits_for_the_new_native_object() {
    let (mut app, asset) = fixture();
    load(&mut app, &asset, false);
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "CloseButton"))
        .id();
    app.update();
    let old_view = app.world().get::<GuiRootView>(owner).unwrap().0;
    app.world_mut()
        .get_mut::<FairyGuiRoot>(owner)
        .unwrap()
        .item_name = "Main".into();
    with_popup(&mut app, |popups| {
        assert!(popups.show(owner, Vec2::new(12., 34.)));
        assert!(popups.is_pending(owner));
        assert!(!popups.is_open(old_view));
    });
    app.update();
    let new_view = app.world().get::<GuiRootView>(owner).unwrap().0;
    assert_ne!(new_view, old_view);
    assert!(app.world().get_entity(old_view).is_err());
    assert!(app.world().resource::<GuiPopupState>().is_open(new_view));
    assert_eq!(
        app.world()
            .get::<crate::GuiTransform>(new_view)
            .unwrap()
            .position(),
        Vec2::new(12., 34.)
    );
}

#[test]
fn a_new_window_menu_request_survives_reinsertion_before_asset_loading() {
    let (mut app, asset) = fixture();
    let source = FairyGuiWindow::new(asset.clone(), "WindowA");
    let owner = app.world_mut().spawn(source.clone()).id();
    show_menu(&mut app, owner);
    app.update();
    app.world_mut()
        .entity_mut(owner)
        .remove::<FairyGuiWindow>()
        .insert(source);
    with_popup(&mut app, |popups| {
        assert!(popups.show_menu(owner, menu(), Vec2::new(90., 100.)));
    });
    app.update();
    load(&mut app, &asset, true);
    app.update();
    let menu = app.world().resource::<GuiPopupState>().menu().unwrap();
    assert_eq!(
        app.world()
            .get::<crate::GuiTransform>(menu)
            .unwrap()
            .position(),
        Vec2::new(90., 100.)
    );
    with_popup(&mut app, |popups| {
        assert!(popups.is_menu_open());
        assert!(!popups.is_pending(owner));
    });
}

#[test]
fn post_layout_menu_calls_materialize_and_project_in_the_same_frame() {
    #[derive(Resource)]
    struct Source(Entity);
    fn open(source: Res<Source>, mut popup: GuiPopup, mut once: Local<bool>) {
        if !*once {
            assert!(popup.show_menu(source.0, menu(), Vec2::new(50., 60.)));
            assert!(popup.is_menu_open());
            *once = true;
        }
    }
    let (mut app, asset) = fixture();
    load(&mut app, &asset, true);
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "Demo_Popup"))
        .id();
    app.insert_resource(Source(owner))
        .add_systems(Update, open.in_set(GuiSystems::PostLayout));
    app.update();
    let menu = app.world().resource::<GuiPopupState>().menu().unwrap();
    let node = app.world().get::<Node>(menu).unwrap();
    assert_eq!((node.left, node.top), (Val::Px(50.), Val::Px(60.)));
    let id = app.world().get::<GObjectRef>(menu).unwrap().0;
    with_popup(&mut app, |popups| {
        popups.dispose_menu();
        assert!(!popups.is_menu_open());
    });
    assert!(!app.world().non_send::<GuiRuntime>().store().contains(id));
    assert!(app.world().get_entity(menu).is_err());
}

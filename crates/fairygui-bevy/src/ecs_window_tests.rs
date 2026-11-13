use super::*;
use crate::{FairyGuiPlugin, GuiObjectAction, GuiObjectCommand, GuiQuery};
use bevy::ecs::system::SystemState;

fn app(loaded: bool) -> (App, Handle<FuiAsset>) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let asset = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
    if loaded {
        load(&mut app, &asset);
    }
    (app, asset)
}
fn load(app: &mut App, asset: &Handle<FuiAsset>) {
    let package = FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-demo/assets/ui/Basics.fui"),
    )
    .unwrap();
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(
            asset.id(),
            FuiAsset {
                package,
                res_key: "ui/Basics".into(),
            },
        )
        .unwrap();
}
fn send(app: &mut App, owner: Entity, action: GuiWindowAction) {
    app.world_mut()
        .write_message(GuiWindowCommand::new(owner, action));
}
fn object(app: &App, owner: Entity) -> GObjectId {
    app.world()
        .non_send::<GuiRuntime>()
        .ecs_window_object(owner)
        .unwrap()
}

#[test]
fn ecs_windows_wait_for_assets_keep_hidden_views_and_dispose_on_owner_removal() {
    let (mut app, asset) = app(false);
    let owner = app
        .world_mut()
        .spawn(FairyGuiWindow::new(asset.clone(), "WindowA"))
        .id();
    send(&mut app, owner, GuiWindowAction::Hide);
    app.world_mut().write_message(GuiObjectCommand::new(
        owner,
        GuiObjectAction::SetPosition(Vec2::new(23., 37.)),
    ));
    app.update();
    assert!(app.world().get::<GuiWindowState>(owner).is_none());
    assert_eq!(app.world().non_send::<GuiRuntime>().window_count(), 0);
    load(&mut app, &asset);
    app.update();
    app.update();
    let state = app.world().get::<GuiWindowState>(owner).unwrap().clone();
    assert!(!state.is_showing());
    assert!(!app.world().get::<FairyGuiWindow>(owner).unwrap().visible);
    assert!(state.frame().is_some() && state.close_button().is_some());
    let model = object(&app, owner);
    let runtime = app.world().non_send::<GuiRuntime>();
    assert_eq!(
        runtime.store().object(model).unwrap().position(),
        fairygui_core::Point::new(23., 37.)
    );
    assert!(!runtime.store().is_on_stage(model));
    let mut gui = SystemState::<GuiQuery>::new(app.world_mut());
    assert_eq!(
        gui.get(app.world()).unwrap().view(owner),
        Some(state.view())
    );
    assert_eq!(gui.get(app.world()).unwrap().window(owner), Some(&state));
    assert!(
        gui.get(app.world())
            .unwrap()
            .find_by_name(owner, "closeButton")
            .is_some()
    );
    send(&mut app, state.view(), GuiWindowAction::Show);
    app.update();
    assert_eq!(
        app.world().get::<GuiWindowState>(owner).unwrap().view(),
        state.view()
    );
    assert!(
        app.world()
            .get::<GuiWindowState>(owner)
            .unwrap()
            .is_showing()
    );
    assert!(app.world().get::<FairyGuiWindow>(owner).unwrap().visible);
    send(&mut app, owner, GuiWindowAction::Center);
    app.update();
    let runtime = app.world().non_send::<GuiRuntime>();
    let window = runtime.store().object(model).unwrap();
    assert_eq!(window.x(), (1280. - window.width()) / 2.);
    assert_eq!(window.y(), (720. - window.height()) / 2.);
    let frame = state.frame().unwrap();
    app.world_mut().entity_mut(owner).remove::<FairyGuiWindow>();
    app.update();
    assert!(app.world().get::<GuiWindowState>(owner).is_none());
    assert!(app.world().get::<GuiRootView>(owner).is_none());
    assert!(app.world().get_entity(state.view()).is_err());
    assert!(app.world().get_entity(frame).is_err());
    assert_eq!(app.world().non_send::<GuiRuntime>().window_count(), 0);
    assert!(!app.world().non_send::<GuiRuntime>().store().contains(model));
}

#[test]
fn ecs_window_modal_changes_keep_order_and_commands_survive_index_compaction() {
    let (mut app, asset) = app(true);
    let first = app
        .world_mut()
        .spawn(FairyGuiWindow::new(asset.clone(), "WindowA"))
        .id();
    let mut config = FairyGuiWindow::new(asset.clone(), "WindowB");
    config.modal = true;
    let second = app.world_mut().spawn(config).id();
    app.update();
    app.update();
    let first_object = object(&app, first);
    let second_object = object(&app, second);
    let runtime = app.world().non_send::<GuiRuntime>();
    assert!(runtime.is_in_top_modal(second_object));
    assert!(!runtime.is_in_top_modal(first_object));
    let order = runtime.root.as_ref().unwrap().popups().collect::<Vec<_>>();
    app.world_mut()
        .get_mut::<FairyGuiWindow>(second)
        .unwrap()
        .modal = false;
    app.update();
    let runtime = app.world().non_send::<GuiRuntime>();
    assert!(!runtime.is_modal());
    assert_eq!(
        runtime.root.as_ref().unwrap().popups().collect::<Vec<_>>(),
        order
    );
    // Remove the first array slot, then target the second by its stable entity.
    app.world_mut().despawn(first);
    send(&mut app, second, GuiWindowAction::Hide);
    app.update();
    assert_eq!(app.world().non_send::<GuiRuntime>().window_count(), 1);
    assert!(
        !app.world()
            .get::<GuiWindowState>(second)
            .unwrap()
            .is_showing()
    );
    send(&mut app, second, GuiWindowAction::Show);
    app.world_mut()
        .get_mut::<FairyGuiWindow>(second)
        .unwrap()
        .draggable = false;
    app.update();
    let state = app.world().get::<GuiWindowState>(second).unwrap();
    assert!(state.is_showing());
    assert!(!state.is_draggable());
    assert_eq!(object(&app, second), second_object);
}

#[test]
fn ecs_window_pending_requests_are_cancelled_on_replacement_removal_and_despawn() {
    for replacement in 0..3 {
        let (mut app, asset) = app(false);
        let mut source = FairyGuiWindow::new(asset.clone(), "WindowA");
        source.visible = false;
        let owner = app.world_mut().spawn(source.clone()).id();
        send(&mut app, owner, GuiWindowAction::Show);
        app.world_mut().write_message(GuiObjectCommand::new(
            owner,
            GuiObjectAction::SetPosition(Vec2::new(17., 29.)),
        ));
        app.update();
        match replacement {
            0 => {
                app.world_mut()
                    .get_mut::<FairyGuiWindow>(owner)
                    .unwrap()
                    .item_name = "WindowB".into();
            }
            1 => {
                app.world_mut()
                    .entity_mut(owner)
                    .remove::<FairyGuiWindow>()
                    .insert(source);
            }
            _ => {
                app.world_mut().despawn(owner);
            }
        }
        load(&mut app, &asset);
        app.update();
        app.update();
        if replacement == 2 {
            assert_eq!(app.world().non_send::<GuiRuntime>().window_count(), 0);
            continue;
        }
        let state = app.world().get::<GuiWindowState>(owner).unwrap();
        assert!(!state.is_showing());
        let runtime = app.world().non_send::<GuiRuntime>();
        let model = runtime.store().object(object(&app, owner)).unwrap();
        assert_ne!(model.position(), fairygui_core::Point::new(17., 29.));
    }
}

#[test]
fn ecs_window_native_hide_updates_source_and_unchanged_snapshots_keep_ticks() {
    let (mut app, asset) = app(true);
    let owner = app
        .world_mut()
        .spawn(FairyGuiWindow::new(asset, "WindowA"))
        .id();
    app.update();
    app.update();
    let state = app.world().get::<GuiWindowState>(owner).unwrap().clone();
    let tick = app
        .world()
        .entity(owner)
        .get_ref::<GuiWindowState>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .entity(owner)
            .get_ref::<GuiWindowState>()
            .unwrap()
            .last_changed(),
        tick
    );
    let model = object(&app, owner);
    let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
    let index = runtime.window_index(model).unwrap();
    runtime.hide_window(index);
    app.update();
    assert!(!app.world().get::<FairyGuiWindow>(owner).unwrap().visible);
    assert!(
        !app.world()
            .get::<GuiWindowState>(owner)
            .unwrap()
            .is_showing()
    );
    assert_eq!(
        app.world().get::<GuiWindowState>(owner).unwrap().view(),
        state.view()
    );
    app.world_mut()
        .get_mut::<FairyGuiWindow>(owner)
        .unwrap()
        .visible = true;
    app.update();
    assert!(
        app.world()
            .get::<GuiWindowState>(owner)
            .unwrap()
            .is_showing()
    );
    app.world_mut()
        .get_mut::<FairyGuiWindow>(owner)
        .unwrap()
        .item_name = "WindowB".into();
    app.update();
    app.update();
    assert!(!app.world().non_send::<GuiRuntime>().store().contains(model));
    assert!(app.world().get_entity(state.view()).is_err());
    assert_ne!(
        app.world().get::<GuiWindowState>(owner).unwrap().view(),
        state.view()
    );
}

#[test]
fn ecs_modal_wait_matches_request_tokens_retains_detached_pane_and_disposes_it_with_window() {
    let (mut app, asset) = app(true);
    let mut config = FairyGuiWindow::new(asset, "WindowA");
    config.modal_wait = Some("PopupMenu".into());
    let owner = app.world_mut().spawn(config).id();
    app.update();
    app.update();
    send(
        &mut app,
        owner,
        GuiWindowAction::ShowModalWait { request: 7 },
    );
    app.update();
    let state = app.world().get::<GuiWindowState>(owner).unwrap();
    assert!(state.is_modal_waiting());
    assert_eq!(state.modal_wait_request(), 7);
    assert!(state.modal_wait_error().is_none());
    let pane_view = state.modal_wait_pane().unwrap();
    let pane = app.world().get::<GObjectRef>(pane_view).unwrap().0;
    let model = object(&app, owner);
    let runtime = app.world().non_send::<GuiRuntime>();
    let frame = runtime.windows[runtime.window_index(model).unwrap()]
        .frame()
        .unwrap();
    let area = runtime.windows[runtime.window_index(model).unwrap()]
        .content_area()
        .unwrap();
    let area_object = runtime.store().object(area).unwrap();
    let origin = runtime.store().object(frame).unwrap().position();
    let overlay = runtime.store().object(pane).unwrap();
    assert_eq!(overlay.size(), area_object.size());
    assert_eq!(overlay.x(), origin.x + area_object.x());
    assert_eq!(overlay.y(), origin.y + area_object.y());
    assert_eq!(
        runtime.store().object(model).unwrap().children().last(),
        Some(&pane)
    );
    send(
        &mut app,
        owner,
        GuiWindowAction::CloseModalWait { request: 9 },
    );
    app.update();
    assert!(
        app.world()
            .get::<GuiWindowState>(owner)
            .unwrap()
            .is_modal_waiting()
    );
    send(
        &mut app,
        owner,
        GuiWindowAction::CloseModalWait { request: 7 },
    );
    app.update();
    assert!(
        !app.world()
            .get::<GuiWindowState>(owner)
            .unwrap()
            .is_modal_waiting()
    );
    assert_eq!(
        app.world()
            .get::<GuiWindowState>(owner)
            .unwrap()
            .modal_wait_request(),
        0
    );
    assert!(
        !app.world()
            .get::<crate::GuiDisplay>(pane_view)
            .unwrap()
            .on_stage()
    );
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .object(pane)
            .unwrap()
            .parent()
            .is_none()
    );
    send(
        &mut app,
        owner,
        GuiWindowAction::ShowModalWait { request: 11 },
    );
    app.update();
    assert_eq!(
        app.world()
            .get::<GuiWindowState>(owner)
            .unwrap()
            .modal_wait_pane(),
        Some(pane_view)
    );
    send(&mut app, owner, GuiWindowAction::Hide);
    app.update();
    assert!(
        !app.world()
            .get::<GuiWindowState>(owner)
            .unwrap()
            .is_modal_waiting()
    );
    send(&mut app, owner, GuiWindowAction::Show);
    app.update();
    assert!(
        !app.world()
            .get::<GuiWindowState>(owner)
            .unwrap()
            .is_modal_waiting()
    );
    assert!(
        !app.world()
            .get::<crate::GuiDisplay>(pane_view)
            .unwrap()
            .on_stage()
    );
    app.world_mut().despawn(owner);
    app.update();
    assert!(app.world().get_entity(pane_view).is_err());
    assert!(!app.world().non_send::<GuiRuntime>().store().contains(pane));
}

#[test]
fn ecs_modal_wait_template_replacement_failure_and_cross_package_retry_keep_ownership() {
    let (mut app, asset) = app(true);
    let mut source = FairyGuiWindow::new(asset, "WindowA");
    source.modal_wait = Some("MissingPane".into());
    let owner = app.world_mut().spawn(source).id();
    send(
        &mut app,
        owner,
        GuiWindowAction::ShowModalWait { request: 1 },
    );
    app.update();
    app.update();
    assert_eq!(
        app.world()
            .get::<GuiWindowState>(owner)
            .unwrap()
            .modal_wait_error(),
        Some("MissingPane")
    );
    assert!(
        app.world()
            .get::<GuiWindowState>(owner)
            .unwrap()
            .modal_wait_pane()
            .is_none()
    );
    let package = FuiPackage::parse(
        "ModalWaiting",
        include_bytes!("../../fairygui-demo/assets/ui/ModalWaiting.fui"),
    )
    .unwrap();
    let foreign = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: package.clone(),
            res_key: "ui/ModalWaiting".into(),
        });
    app.world_mut()
        .get_mut::<FairyGuiWindow>(owner)
        .unwrap()
        .modal_wait = Some("ui://ModalWaiting/WindowModalWaiting".into());
    send(
        &mut app,
        owner,
        GuiWindowAction::ShowModalWait { request: 2 },
    );
    app.update();
    assert!(
        app.world()
            .get::<GuiWindowState>(owner)
            .unwrap()
            .modal_wait_error()
            .is_some()
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .register_package(foreign, &package);
    send(
        &mut app,
        owner,
        GuiWindowAction::ShowModalWait { request: 2 },
    );
    app.update();
    app.update();
    let state = app.world().get::<GuiWindowState>(owner).unwrap();
    assert!(state.modal_wait_error().is_none());
    let pane = state.modal_wait_pane().unwrap();
    let pane_object = app.world().get::<GObjectRef>(pane).unwrap().0;
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .object(pane_object)
            .unwrap()
            .source_package_id(),
        Some(package.id())
    );
    send(
        &mut app,
        owner,
        GuiWindowAction::CloseModalWait { request: 0 },
    );
    app.update();
    app.world_mut()
        .get_mut::<FairyGuiWindow>(owner)
        .unwrap()
        .modal_wait = None;
    app.update();
    assert!(app.world().get_entity(pane).is_err());
    assert!(
        app.world()
            .get::<GuiWindowState>(owner)
            .unwrap()
            .modal_wait_pane()
            .is_none()
    );
    assert!(
        app.world()
            .get::<GuiWindowState>(owner)
            .unwrap()
            .modal_wait_error()
            .is_none()
    );
}

#[test]
fn ecs_window_menu_sources_wait_for_assets_and_cancel_replaced_or_removed_requests() {
    use crate::{GuiPopupCommand, GuiPopupMenu, GuiPopupState};
    let (mut app, asset) = app(false);
    let owner = app
        .world_mut()
        .spawn(FairyGuiWindow::new(asset.clone(), "WindowA"))
        .id();
    let menu = |source| GuiPopupCommand::ShowMenu {
        source,
        position: Vec2::new(40., 50.),
        menu: GuiPopupMenu::new(
            "PopupMenu",
            "PopupMenuItem",
            vec![fairygui_core::popup::MenuItem::action("save", "Save", 1)],
        ),
    };
    app.world_mut().write_message(menu(owner));
    app.update();
    assert!(app.world().resource::<GuiPopupState>().menu().is_none());
    load(&mut app, &asset);
    app.update();
    app.update();
    let view = app.world().get::<GuiWindowState>(owner).unwrap().view();
    assert!(app.world().resource::<GuiPopupState>().menu().is_some());
    let mut gui = SystemState::<GuiQuery>::new(app.world_mut());
    assert!(
        gui.get(app.world())
            .unwrap()
            .window(view)
            .unwrap()
            .is_showing()
    );
    app.world_mut().write_message(GuiPopupCommand::DisposeMenu);
    app.update();
    // Showing a window through a plain popup must not duplicate its root entry.
    app.world_mut().write_message(GuiPopupCommand::Show {
        popup: owner,
        position: Vec2::ZERO,
    });
    app.update();
    assert!(app.world().resource::<GuiPopupState>().stack().is_empty());
    assert!(
        app.world()
            .get::<GuiWindowState>(owner)
            .unwrap()
            .is_showing()
    );
    for replacement in 0..3 {
        let (mut app, asset) = self::app(false);
        let source = FairyGuiWindow::new(asset.clone(), "WindowA");
        let owner = app.world_mut().spawn(source.clone()).id();
        app.world_mut().write_message(menu(owner));
        app.update();
        match replacement {
            0 => {
                app.world_mut()
                    .get_mut::<FairyGuiWindow>(owner)
                    .unwrap()
                    .item_name = "WindowB".into();
            }
            1 => {
                app.world_mut()
                    .entity_mut(owner)
                    .remove::<FairyGuiWindow>()
                    .insert(source);
            }
            _ => {
                app.world_mut().despawn(owner);
            }
        }
        load(&mut app, &asset);
        app.update();
        app.update();
        assert!(app.world().resource::<GuiPopupState>().menu().is_none());
    }
}

use super::*;
use crate::{FairyGuiPlugin, FairyGuiWindow, GuiWindowAction, GuiWindowCommand};
use bevy::window::PrimaryWindow;

fn fixture(loaded: bool) -> (App, Handle<FuiAsset>) {
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
    app.insert_resource(GuiGlobalWaitSettings::new(
        asset.clone(),
        "GlobalModalWaiting",
    ));
    (app, asset)
}
fn load(app: &mut App, handle: &Handle<FuiAsset>) {
    let package = FuiPackage::parse(
        "ModalWaiting",
        include_bytes!("../../fairygui-demo/assets/ui/ModalWaiting.fui"),
    )
    .unwrap();
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(
            handle.id(),
            FuiAsset {
                package,
                res_key: "ui/ModalWaiting".into(),
            },
        )
        .unwrap();
}
fn show(app: &mut App) {
    app.world_mut().write_message(GuiGlobalWaitCommand::Show {
        message: Some("Loading".into()),
    });
}

#[test]
fn global_wait_waits_for_assets_resizes_reuses_hidden_views_and_disposes_the_retained_tree() {
    let (mut app, asset) = fixture(false);
    show(&mut app);
    app.update();
    let state = app.world().resource::<GuiGlobalWaitState>();
    assert!(state.is_requested() && !state.is_waiting());
    assert!(state.view().is_none() && state.error().is_none());
    load(&mut app, &asset);
    app.update();
    app.update();
    let state = app.world().resource::<GuiGlobalWaitState>();
    assert!(state.is_waiting());
    let view = state.view().unwrap();
    let model = app.world().get::<GObjectRef>(view).unwrap().0;
    let runtime = app.world().non_send::<GuiRuntime>();
    assert_eq!(
        runtime.store().object(model).unwrap().size(),
        fairygui_core::Size::new(1280., 720.)
    );
    assert_eq!(app.world().get::<GlobalZIndex>(view).unwrap().0, i32::MAX);
    let tick = app
        .world()
        .get_resource_ref::<GuiGlobalWaitState>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .get_resource_ref::<GuiGlobalWaitState>()
            .unwrap()
            .last_changed(),
        tick
    );
    let mut host = Window::default();
    host.resolution.set(1100., 650.);
    app.world_mut().spawn((host, PrimaryWindow));
    app.update();
    assert_eq!(
        app.world().get::<crate::GuiTransform>(view).unwrap().size(),
        Vec2::new(1100., 650.)
    );
    app.world_mut().write_message(GuiGlobalWaitCommand::Hide);
    app.update();
    let state = app.world().resource::<GuiGlobalWaitState>();
    assert!(!state.is_requested() && !state.is_waiting());
    assert_eq!(state.view(), Some(view));
    assert!(
        !app.world()
            .get::<crate::GuiDisplay>(view)
            .unwrap()
            .on_stage()
    );
    show(&mut app);
    app.update();
    assert_eq!(
        app.world().resource::<GuiGlobalWaitState>().view(),
        Some(view)
    );
    app.world_mut().write_message(GuiGlobalWaitCommand::Hide);
    app.update();
    app.world_mut().write_message(GuiGlobalWaitCommand::Dispose);
    app.update();
    assert!(app.world().get_entity(view).is_err());
    assert!(!app.world().non_send::<GuiRuntime>().store().contains(model));
    assert!(
        app.world()
            .resource::<GuiGlobalWaitState>()
            .view()
            .is_none()
    );
}

#[test]
fn pending_global_wait_cancels_without_an_invisible_barrier_and_errors_retry_after_configuration() {
    let (mut app, asset) = fixture(false);
    show(&mut app);
    app.update();
    app.world_mut().write_message(GuiGlobalWaitCommand::Hide);
    app.update();
    load(&mut app, &asset);
    app.update();
    assert!(
        app.world()
            .resource::<GuiGlobalWaitState>()
            .view()
            .is_none()
    );
    app.world_mut()
        .insert_resource(GuiGlobalWaitSettings::default());
    show(&mut app);
    app.update();
    let state = app.world().resource::<GuiGlobalWaitState>();
    assert_eq!(state.error(), Some(&GuiGlobalWaitError::MissingTemplate));
    assert!(!state.is_waiting());
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .global_wait_pane()
            .is_none()
    );
    app.insert_resource(GuiGlobalWaitSettings::new(asset.clone(), "MissingPane"));
    app.update();
    assert_eq!(
        app.world().resource::<GuiGlobalWaitState>().error(),
        Some(&GuiGlobalWaitError::MissingResource("MissingPane".into()))
    );
    app.insert_resource(GuiGlobalWaitSettings::new(asset, "GlobalModalWaiting"));
    app.update();
    app.update();
    let view = app.world().resource::<GuiGlobalWaitState>().view().unwrap();
    assert!(app.world().resource::<GuiGlobalWaitState>().is_waiting());
    app.world_mut()
        .resource_mut::<GuiGlobalWaitSettings>()
        .item_name = "WindowModalWaiting".into();
    app.update();
    app.update();
    assert!(app.world().get_entity(view).is_err());
    assert_ne!(
        app.world().resource::<GuiGlobalWaitState>().view(),
        Some(view)
    );
    app.world_mut().write_message(GuiGlobalWaitCommand::Dispose);
    app.update();
    assert!(!app.world().resource::<GuiGlobalWaitState>().is_requested());
    assert!(
        app.world()
            .resource::<GuiGlobalWaitState>()
            .view()
            .is_none()
    );
}

#[test]
fn global_wait_stays_above_new_windows_and_popups_and_gates_their_input_until_closed() {
    let (mut app, _) = fixture(true);
    let package = FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-demo/assets/ui/Basics.fui"),
    )
    .unwrap();
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package,
            res_key: "ui/Basics".into(),
        });
    show(&mut app);
    app.update();
    app.update();
    let view = app.world().resource::<GuiGlobalWaitState>().view().unwrap();
    let pane = app.world().get::<GObjectRef>(view).unwrap().0;
    let mut config = FairyGuiWindow::new(asset, "WindowA");
    config.modal = true;
    let owner = app.world_mut().spawn(config).id();
    app.update();
    app.update();
    let window = app
        .world()
        .non_send::<GuiRuntime>()
        .ecs_window_object(owner)
        .unwrap();
    let runtime = app.world().non_send::<GuiRuntime>();
    assert!(runtime.is_in_top_modal(pane));
    assert!(!runtime.is_in_top_modal(window));
    assert!(crate::input::eligible(runtime, pane));
    assert!(!crate::input::eligible(runtime, window));
    assert_eq!(
        runtime
            .store()
            .object(runtime.root_container().unwrap())
            .unwrap()
            .children()
            .last(),
        Some(&pane)
    );
    app.world_mut()
        .write_message(GuiWindowCommand::new(owner, GuiWindowAction::BringToFront));
    app.world_mut()
        .write_message(crate::GuiPopupCommand::ShowMenu {
            source: owner,
            position: Vec2::new(40., 50.),
            menu: crate::GuiPopupMenu::new(
                "PopupMenu",
                "PopupMenuItem",
                vec![fairygui_core::popup::MenuItem::action(
                    "action", "Action", 1,
                )],
            ),
        });
    app.update();
    let menu = app
        .world()
        .resource::<crate::GuiPopupState>()
        .menu()
        .unwrap();
    let menu_model = app.world().get::<GObjectRef>(menu).unwrap().0;
    let runtime = app.world().non_send::<GuiRuntime>();
    assert_eq!(
        runtime
            .store()
            .object(runtime.root_container().unwrap())
            .unwrap()
            .children()
            .last(),
        Some(&pane)
    );
    assert!(!crate::input::eligible(runtime, menu_model));
    app.world_mut().write_message(GuiGlobalWaitCommand::Hide);
    app.update();
    let runtime = app.world().non_send::<GuiRuntime>();
    assert!(crate::input::eligible(runtime, window));
    assert!(crate::input::eligible(runtime, menu_model));
    assert!(!crate::input::eligible(runtime, pane));
}

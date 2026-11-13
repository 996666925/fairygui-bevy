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

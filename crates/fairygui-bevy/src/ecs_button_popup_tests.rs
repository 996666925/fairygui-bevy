use super::*;
use crate::prelude::*;
use bevy::{
    picking::hover::Hovered,
    ui::{ComputedStackIndex, RelativeCursorPosition},
    window::PrimaryWindow,
};

fn fixture() -> (App, Handle<FuiAsset>, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let mut window = Window {
        focused: true,
        ..default()
    };
    window.set_cursor_position(Some(Vec2::new(20., 20.)));
    app.world_mut().spawn((window, PrimaryWindow));
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
    let button = app
        .world_mut()
        .spawn((FairyGuiRoot::new(asset.clone(), "Button1"), GuiOpaque(true)))
        .id();
    app.update();
    (app, asset, button)
}
fn view(app: &App, owner: Entity) -> Entity {
    app.world().get::<GuiRootView>(owner).unwrap().0
}
fn id(app: &App, owner: Entity) -> GObjectId {
    app.world().get::<GObjectRef>(view(app, owner)).unwrap().0
}
fn linked(app: &App, button: Entity) -> Option<GObjectId> {
    let ControlData::Button(data) = app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .object(id(app, button))
        .unwrap()
        .control()
    else {
        panic!()
    };
    data.linked_popup
}
fn press(app: &mut App, button: Entity) {
    let button = view(app, button);
    app.world_mut().entity_mut(button).insert((
        Hovered(true),
        ComputedStackIndex(100),
        RelativeCursorPosition {
            cursor_over: true,
            normalized: Some(Vec2::ZERO),
        },
    ));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    // MinimalPlugins has no InputPlugin to reset frame transition flags.
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
}

#[test]
fn entity_button_popup_waits_rebinds_templates_and_clears_on_removal() {
    let (mut app, asset, button) = fixture();
    let pending = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
    let mut source = FairyGuiRoot::new(pending.clone(), "Component12");
    source.active = false;
    let popup = app.world_mut().spawn(source).id();
    app.world_mut()
        .entity_mut(button)
        .insert(GuiButtonPopup(Some(popup)));
    app.update();
    assert_eq!(linked(&app, button), None);
    let loaded = {
        let assets = app.world().resource::<Assets<FuiAsset>>();
        let asset = assets.get(&asset).unwrap();
        FuiAsset {
            package: asset.package.clone(),
            res_key: asset.res_key.clone(),
        }
    };
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(pending.id(), loaded)
        .unwrap();
    app.update();
    let original = id(&app, popup);
    assert_eq!(linked(&app, button), Some(original));
    app.world_mut()
        .get_mut::<FairyGuiRoot>(popup)
        .unwrap()
        .item_name = "Button2".into();
    crate::properties::sync(app.world_mut());
    assert_eq!(linked(&app, button), None);
    app.update();
    assert_ne!(id(&app, popup), original);
    assert_eq!(linked(&app, button), Some(id(&app, popup)));
    app.world_mut()
        .entity_mut(button)
        .remove::<GuiButtonPopup>();
    crate::properties::sync(app.world_mut());
    assert_eq!(linked(&app, button), None);
}

#[test]
fn button_popup_view_override_removal_and_invalid_targets_are_deterministic() {
    let (mut app, asset, button) = fixture();
    let mut source = FairyGuiRoot::new(asset, "Component12");
    source.active = false;
    let popup = app.world_mut().spawn(source).id();
    app.update();
    let button_view = view(&app, button);
    app.world_mut()
        .entity_mut(button)
        .insert(GuiButtonPopup(Some(popup)));
    app.world_mut()
        .entity_mut(button_view)
        .insert(GuiButtonPopup(None));
    app.update();
    assert_eq!(linked(&app, button), None);
    app.world_mut()
        .entity_mut(button_view)
        .remove::<GuiButtonPopup>();
    app.update();
    assert_eq!(linked(&app, button), Some(id(&app, popup)));
    for target in [button, button_view, Entity::PLACEHOLDER] {
        app.world_mut().get_mut::<GuiButtonPopup>(button).unwrap().0 = Some(target);
        app.update();
        assert_eq!(linked(&app, button), None);
    }
    let popup_view = view(&app, popup);
    app.world_mut().get_mut::<GuiButtonPopup>(button).unwrap().0 = Some(popup_view);
    app.update();
    assert_eq!(linked(&app, button), Some(id(&app, popup)));
    app.world_mut().despawn(popup);
    app.update();
    assert_eq!(linked(&app, button), None);
}

#[test]
fn entity_linked_popups_toggle_on_input_and_keep_their_views() {
    for window in [false, true] {
        let (mut app, asset, button) = fixture();
        let popup = if window {
            let mut source = FairyGuiWindow::new(asset, "WindowA");
            source.visible = false;
            app.world_mut().spawn(source).id()
        } else {
            let mut source = FairyGuiRoot::new(asset, "Component12");
            source.active = false;
            app.world_mut().spawn(source).id()
        };
        app.update();
        let popup_view = view(&app, popup);
        // Must apply before the current pointer down, without a prior Sync.
        app.world_mut()
            .entity_mut(button)
            .insert(GuiButtonPopup(Some(popup)));
        press(&mut app, button);
        if window {
            assert!(
                app.world()
                    .get::<GuiWindowState>(popup)
                    .unwrap()
                    .is_showing()
            );
        } else {
            assert_eq!(
                app.world().resource::<GuiPopupState>().stack(),
                &[popup_view]
            );
        }
        press(&mut app, button);
        if window {
            assert!(
                !app.world()
                    .get::<GuiWindowState>(popup)
                    .unwrap()
                    .is_showing()
            );
        } else {
            assert!(app.world().resource::<GuiPopupState>().stack().is_empty());
        }
        assert_eq!(view(&app, popup), popup_view);
        press(&mut app, button);
        assert_eq!(view(&app, popup), popup_view);
        if window {
            assert!(
                app.world()
                    .get::<GuiWindowState>(popup)
                    .unwrap()
                    .is_showing()
            );
        } else {
            assert_eq!(
                app.world().resource::<GuiPopupState>().stack(),
                &[popup_view]
            );
        }
    }
}

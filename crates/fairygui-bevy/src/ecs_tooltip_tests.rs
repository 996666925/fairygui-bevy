use super::*;
use crate::{FairyGuiPlugin, GuiObjectInfo, GuiTransform};
use bevy::picking::hover::Hovered;
use bevy::time::TimeUpdateStrategy;
use bevy::ui::{ComputedStackIndex, RelativeCursorPosition};
use std::time::Duration;

fn fixture(loaded: bool) -> (App, Handle<FuiAsset>, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>()
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        20,
    )));
    let handle = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
    if loaded {
        load(&mut app, &handle);
    }
    app.insert_resource(GuiTooltipSettings::new(handle.clone(), "Component3"));
    let mut window = Window {
        focused: true,
        ..default()
    };
    window.resolution.set(800., 600.);
    window.set_cursor_position(Some(Vec2::new(50., 60.)));
    let window = app.world_mut().spawn((window, PrimaryWindow)).id();
    (app, handle, window)
}
fn load(app: &mut App, handle: &Handle<FuiAsset>) {
    let package = FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-demo/assets/ui/Basics.fui"),
    )
    .unwrap();
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(
            handle.id(),
            FuiAsset {
                package,
                res_key: "ui/Basics".into(),
            },
        )
        .unwrap();
}
fn show(app: &mut App, position: Option<Vec2>) {
    app.world_mut().write_message(GuiTooltipCommand::Show {
        text: "Tooltip text".into(),
        position,
    });
}
fn view(app: &App) -> Entity {
    app.world().resource::<GuiTooltipState>().view().unwrap()
}
fn at(app: &App) -> Vec2 {
    app.world()
        .get::<GuiTransform>(view(app))
        .unwrap()
        .position()
}
fn frames(app: &mut App, count: usize) {
    for _ in 0..count {
        app.update();
    }
}
fn hover(app: &mut App, target: Option<Entity>) {
    let entities: Vec<_> = app
        .world_mut()
        .query_filtered::<Entity, With<GObjectRef>>()
        .iter(app.world())
        .collect();
    for entity in entities {
        app.world_mut()
            .entity_mut(entity)
            .insert(Hovered(Some(entity) == target));
    }
    if let Some(target) = target {
        app.world_mut().entity_mut(target).insert((
            ComputedStackIndex(100),
            RelativeCursorPosition {
                cursor_over: true,
                normalized: Some(Vec2::ZERO),
            },
        ));
    }
}
fn control(app: &mut App, handle: Handle<FuiAsset>) -> (Entity, Entity) {
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(handle, "Demo_Button"))
        .id();
    app.update();
    let runtime = app.world().non_send::<GuiRuntime>();
    let root = app.world().get::<GuiRootInstance>(owner).unwrap().object;
    let button = runtime.store().child_by_name(root, "n3").unwrap();
    (owner, runtime.entity_of(button).unwrap())
}
fn set_tip(app: &mut App, target: Entity, text: Option<&str>) {
    app.world_mut().write_message(GuiObjectCommand::new(
        target,
        GuiObjectAction::SetTooltips(text.map(str::to_owned)),
    ));
    app.update();
}

#[test]
fn tooltip_waits_for_assets_positions_on_first_view_reuses_and_disposes() {
    let (mut app, asset, _) = fixture(false);
    show(&mut app, None);
    frames(&mut app, 4);
    let state = app.world().resource::<GuiTooltipState>();
    assert!(state.is_requested() && !state.is_showing() && state.error().is_none());
    load(&mut app, &asset);
    app.update();
    let pane = view(&app);
    assert!(app.world().resource::<GuiTooltipState>().is_showing());
    assert_eq!(at(&app), Vec2::new(60., 80.));
    let node = app.world().get::<Node>(pane).unwrap();
    assert_eq!((node.left, node.top), (Val::Px(60.), Val::Px(80.)));
    assert_eq!(
        app.world().get::<crate::GuiText>(pane).unwrap().value(),
        "Tooltip text"
    );
    assert_eq!(
        app.world().get::<GlobalZIndex>(pane),
        Some(&GlobalZIndex(i32::MAX - 1))
    );
    let id = app.world().get::<GObjectRef>(pane).unwrap().0;
    assert!(!crate::input::eligible(
        app.world().non_send::<GuiRuntime>(),
        id
    ));
    let tick = app
        .world()
        .get_resource_ref::<GuiTooltipState>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .get_resource_ref::<GuiTooltipState>()
            .unwrap()
            .last_changed(),
        tick
    );
    app.world_mut().write_message(GuiTooltipCommand::Hide);
    app.update();
    assert!(!app.world().resource::<GuiTooltipState>().is_showing());
    assert_eq!(view(&app), pane);
    assert!(app.world().get::<GlobalZIndex>(pane).is_none());
    assert!(
        !app.world()
            .get::<crate::GuiDisplay>(pane)
            .unwrap()
            .on_stage()
    );
    show(&mut app, Some(Vec2::new(100., 120.)));
    app.update();
    assert_eq!(view(&app), pane);
    assert_eq!(at(&app), Vec2::new(100., 120.));
    app.world_mut().write_message(GuiTooltipCommand::Dispose);
    app.update();
    assert!(app.world().get_entity(pane).is_err());
    assert!(!app.world().non_send::<GuiRuntime>().store().contains(id));
}

#[test]
fn hover_delay_reads_current_text_and_cursor_then_hides_on_leave_click_and_visibility() {
    let (mut app, asset, window) = fixture(true);
    let (owner, button) = control(&mut app, asset);
    set_tip(&mut app, button, Some("First"));
    assert_eq!(
        app.world().get::<GuiObjectInfo>(button).unwrap().tooltips(),
        Some("First")
    );
    hover(&mut app, Some(button));
    frames(&mut app, 5); // 0, 20, 40, 60, 80 ms since hover start.
    assert!(!app.world().resource::<GuiTooltipState>().is_showing());
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(Vec2::new(200., 250.)));
    set_tip(&mut app, button, Some("Changed before delay"));
    assert!(app.world().resource::<GuiTooltipState>().is_showing());
    assert_eq!(
        app.world().resource::<GuiTooltipState>().source(),
        Some(button)
    );
    assert_eq!(
        app.world()
            .get::<crate::GuiText>(view(&app))
            .unwrap()
            .value(),
        "Changed before delay"
    );
    assert_eq!(at(&app), Vec2::new(210., 270.));
    let pane = view(&app);
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(Vec2::new(220., 250.)));
    app.update();
    assert_eq!(at(&app), Vec2::new(210., 270.)); // Tooltip doesn't track the pointer.
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .reset_all();
    assert!(!app.world().resource::<GuiTooltipState>().is_showing());
    frames(&mut app, 7);
    assert!(!app.world().resource::<GuiTooltipState>().is_showing());
    hover(&mut app, None);
    app.update();
    hover(&mut app, Some(button));
    frames(&mut app, 6);
    assert_eq!(view(&app), pane);
    assert!(app.world().resource::<GuiTooltipState>().is_showing());
    app.world_mut().write_message(GuiObjectCommand::new(
        owner,
        GuiObjectAction::SetVisible(false),
    ));
    app.update();
    assert!(!app.world().resource::<GuiTooltipState>().is_showing());
}

#[test]
fn leaving_cancels_delay_but_mouse_down_during_delay_does_not() {
    let (mut app, asset, _) = fixture(true);
    let (_, button) = control(&mut app, asset);
    set_tip(&mut app, button, Some("Tip"));
    hover(&mut app, Some(button));
    frames(&mut app, 3);
    hover(&mut app, None);
    frames(&mut app, 7);
    assert!(app.world().resource::<GuiTooltipState>().view().is_none());
    hover(&mut app, Some(button));
    frames(&mut app, 2);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Right);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .reset_all();
    frames(&mut app, 3);
    assert!(app.world().resource::<GuiTooltipState>().is_showing());
    set_tip(&mut app, button, None);
    assert!(!app.world().resource::<GuiTooltipState>().is_showing());
    assert!(
        app.world()
            .get::<GuiObjectInfo>(button)
            .unwrap()
            .tooltips()
            .is_none()
    );
}

#[test]
fn tooltip_uses_native_edge_placement_and_root_coordinate_conversion() {
    assert_eq!(
        tooltip_position(
            Vec2::new(790., 590.),
            Vec2::new(100., 40.),
            Vec2::new(800., 600.)
        ),
        Vec2::new(588., 549.)
    );
    assert_eq!(
        tooltip_position(
            Vec2::new(5., 5.),
            Vec2::new(900., 700.),
            Vec2::new(800., 600.)
        ),
        Vec2::splat(10.)
    );
    let (mut app, _, _) = fixture(true);
    show(&mut app, Some(Vec2::new(790., 590.)));
    app.update();
    assert_eq!(at(&app), Vec2::new(92., 527.)); // Component3 is 348x62.
    let stage = app
        .world()
        .non_send::<GuiRuntime>()
        .root_container()
        .unwrap();
    {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        runtime.store_mut().set_xy(stage, 50., 60.);
        runtime.store_mut().set_scale(stage, 2., 2.);
    }
    show(&mut app, Some(Vec2::new(250., 260.)));
    app.update();
    assert_eq!(at(&app), Vec2::splat(100.));
}

#[test]
fn tooltip_errors_retry_configuration_and_pending_hide_prevents_late_show() {
    let (mut app, asset, _) = fixture(false);
    show(&mut app, None);
    app.update();
    app.world_mut().write_message(GuiTooltipCommand::Hide);
    app.update();
    load(&mut app, &asset);
    app.update();
    assert!(app.world().resource::<GuiTooltipState>().view().is_none());
    app.insert_resource(GuiTooltipSettings::default());
    show(&mut app, None);
    app.update();
    assert_eq!(
        app.world().resource::<GuiTooltipState>().error(),
        Some(&GuiTooltipError::MissingTemplate)
    );
    app.insert_resource(GuiTooltipSettings::new(asset.clone(), "Missing"));
    app.update();
    assert_eq!(
        app.world().resource::<GuiTooltipState>().error(),
        Some(&GuiTooltipError::MissingResource("Missing".into()))
    );
    app.insert_resource(GuiTooltipSettings::new(asset, "Component3"));
    app.update();
    let old = view(&app);
    assert!(app.world().resource::<GuiTooltipState>().is_showing());
    app.world_mut()
        .resource_mut::<GuiTooltipSettings>()
        .item_name = "WindowFrame".into();
    app.update();
    assert!(app.world().get_entity(old).is_err());
    assert_ne!(view(&app), old);
}

#[test]
fn custom_child_tooltip_is_popup_independent_passes_input_and_dies_with_original_page() {
    let (mut app, asset, _) = fixture(true);
    let (owner, button) = control(&mut app, asset);
    app.world_mut().entity_mut(button).insert(GlobalZIndex(7));
    app.world_mut().write_message(GuiTooltipCommand::ShowView {
        view: button,
        position: Some(Vec2::new(100., 100.)),
    });
    app.update();
    let runtime = app.world().non_send::<GuiRuntime>();
    let pane = app.world().get::<GObjectRef>(button).unwrap().0;
    assert_eq!(
        runtime.store().object(pane).unwrap().parent(),
        runtime.root_container()
    );
    assert!(runtime.popup_stack().is_empty());
    let mut subtree = vec![pane];
    runtime
        .store()
        .for_each_descendant(pane, &mut |id| subtree.push(id));
    for id in subtree {
        assert!(!crate::input::eligible(runtime, id));
    }
    app.world_mut()
        .write_message(crate::GuiPopupCommand::HideAll);
    app.update();
    assert!(app.world().resource::<GuiTooltipState>().is_showing());
    app.world_mut().write_message(GuiTooltipCommand::Hide);
    app.update();
    assert_eq!(
        app.world().get::<GlobalZIndex>(button),
        Some(&GlobalZIndex(7))
    );
    assert!(app.world().get_entity(button).is_ok());
    app.world_mut().despawn(owner);
    app.update();
    assert!(app.world().get_entity(button).is_err());
    assert!(!app.world().non_send::<GuiRuntime>().store().contains(pane));
}

#[test]
fn custom_root_request_waits_for_assets_and_cancels_on_replacement() {
    let (mut app, asset, _) = fixture(false);
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), "Component3"))
        .id();
    app.world_mut().write_message(GuiTooltipCommand::ShowView {
        view: owner,
        position: None,
    });
    frames(&mut app, 4);
    assert!(app.world().resource::<GuiTooltipState>().is_requested());
    load(&mut app, &asset);
    app.update();
    let pane = app.world().get::<GuiRootView>(owner).unwrap().0;
    assert_eq!(view(&app), pane);
    assert!(app.world().resource::<GuiTooltipState>().is_showing());
    app.world_mut().write_message(GuiTooltipCommand::Hide);
    app.update();
    assert!(
        !app.world()
            .get::<crate::GuiDisplay>(pane)
            .unwrap()
            .on_stage()
    );
    app.world_mut().write_message(GuiTooltipCommand::ShowView {
        view: owner,
        position: None,
    });
    app.update();
    app.world_mut().despawn(owner);
    app.update();
    assert!(!app.world().resource::<GuiTooltipState>().is_showing());
    assert!(app.world().get_entity(pane).is_err());

    let pending = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(pending.clone(), "Component3"))
        .id();
    app.world_mut().write_message(GuiTooltipCommand::ShowView {
        view: owner,
        position: None,
    });
    app.update();
    assert!(app.world().resource::<GuiTooltipState>().is_requested());
    app.world_mut()
        .get_mut::<FairyGuiRoot>(owner)
        .unwrap()
        .item_name = "WindowFrame".into();
    app.update();
    assert!(!app.world().resource::<GuiTooltipState>().is_requested());
    load(&mut app, &pending);
    app.update();
    assert!(!app.world().resource::<GuiTooltipState>().is_showing());
}

#[test]
fn ancestor_tooltip_keeps_delay_across_children_and_cancels_on_blur_and_disposal() {
    let (mut app, asset, window) = fixture(true);
    let (owner, button) = control(&mut app, asset);
    let second = {
        let runtime = app.world().non_send::<GuiRuntime>();
        let root = app.world().get::<GuiRootInstance>(owner).unwrap().object;
        runtime
            .entity_of(runtime.store().child_by_name(root, "n6").unwrap())
            .unwrap()
    };
    set_tip(&mut app, owner, Some("Page help"));
    hover(&mut app, Some(button));
    frames(&mut app, 3);
    hover(&mut app, Some(second));
    frames(&mut app, 3);
    assert!(app.world().resource::<GuiTooltipState>().is_showing());
    assert_eq!(
        app.world().resource::<GuiTooltipState>().source(),
        app.world().get::<GuiRootView>(owner).map(|view| view.0)
    );
    app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
    app.update();
    assert!(!app.world().resource::<GuiTooltipState>().is_showing());
    app.world_mut().get_mut::<Window>(window).unwrap().focused = true;
    frames(&mut app, 6);
    assert!(app.world().resource::<GuiTooltipState>().is_showing());
    app.world_mut().despawn(owner);
    app.update();
    assert!(!app.world().resource::<GuiTooltipState>().is_requested());
}

#[test]
fn post_layout_tooltip_commands_show_before_the_same_frames_projection() {
    fn show_in_post_layout(mut commands: Commands, mut once: Local<bool>) {
        if !*once {
            commands.write_message(GuiTooltipCommand::Show {
                text: "Same frame".into(),
                position: Some(Vec2::new(42., 56.)),
            });
            *once = true;
        }
    }
    let (mut app, _, _) = fixture(true);
    app.add_systems(
        Update,
        show_in_post_layout.in_set(crate::GuiSystems::PostLayout),
    );
    app.update();
    assert!(app.world().resource::<GuiTooltipState>().is_showing());
    assert_eq!(at(&app), Vec2::new(42., 56.));
    assert_eq!(
        app.world()
            .get::<crate::GuiText>(view(&app))
            .unwrap()
            .value(),
        "Same frame"
    );
}

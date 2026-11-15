use super::*;
use crate::{FairyGuiPlugin, GuiObjectInfo, GuiTransform};
use bevy::ecs::system::RunSystemOnce;
use bevy::ecs::system::SystemState;
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
fn with_tooltip(app: &mut App, edit: impl FnOnce(&mut GuiTooltip)) {
    let mut state = SystemState::<GuiTooltip>::new(app.world_mut());
    edit(&mut state.get_mut(app.world_mut()).unwrap());
}
fn hide(app: &mut App) {
    with_tooltip(app, |tooltip| {
        tooltip.hide();
        assert!(!tooltip.is_requested() && !tooltip.is_showing());
    });
}
fn show_view(app: &mut App, view: Entity, position: Option<Vec2>) {
    with_tooltip(app, |tooltip| {
        assert!(tooltip.show_view(view, position));
        assert!(tooltip.is_requested());
    });
}
fn show(app: &mut App, position: Option<Vec2>) {
    with_tooltip(app, |tooltip| {
        assert!(tooltip.show("Tooltip text", position));
        assert!(tooltip.is_requested());
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
    if let Some(mut object) = app.world_mut().gui_mut(target) {
        object.set_tooltips(text.map(str::to_owned))
    }
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
    hide(&mut app);
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
    with_tooltip(&mut app, |tooltip| tooltip.dispose());
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
    if let Some(mut object) = app.world_mut().gui_mut(owner) {
        object.set_visible(false)
    }
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
    hide(&mut app);
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
    show_view(&mut app, button, Some(Vec2::new(100., 100.)));
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
        .run_system_once(|mut popup: crate::GuiPopup| popup.hide_all())
        .unwrap();
    app.update();
    assert!(app.world().resource::<GuiTooltipState>().is_showing());
    hide(&mut app);
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
    show_view(&mut app, owner, None);
    frames(&mut app, 4);
    assert!(app.world().resource::<GuiTooltipState>().is_requested());
    load(&mut app, &asset);
    app.update();
    let pane = app.world().get::<GuiRootView>(owner).unwrap().0;
    assert_eq!(view(&app), pane);
    assert!(app.world().resource::<GuiTooltipState>().is_showing());
    hide(&mut app);
    app.update();
    assert!(
        !app.world()
            .get::<crate::GuiDisplay>(pane)
            .unwrap()
            .on_stage()
    );
    show_view(&mut app, owner, None);
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
    show_view(&mut app, owner, None);
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
fn post_layout_tooltip_calls_show_before_the_same_frames_projection() {
    fn show_in_post_layout(mut tooltip: GuiTooltip, mut once: Local<bool>) {
        if !*once {
            assert!(tooltip.show("Same frame", Some(Vec2::new(42., 56.))));
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

#[test]
fn direct_calls_validate_before_replacing_and_hide_and_dispose_immediately() {
    let (mut app, _, _) = fixture(true);
    show(&mut app, None);
    app.update();
    let pane = view(&app);
    let object = app.world().get::<GObjectRef>(pane).unwrap().0;
    let unrelated = app.world_mut().spawn_empty().id();
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
    with_tooltip(&mut app, |tooltip| {
        assert!(tooltip.is_showing());
        assert!(!tooltip.show("Invalid", Some(Vec2::new(f32::NAN, 0.))));
        assert!(!tooltip.show_view(pane, Some(Vec2::splat(f32::INFINITY))));
        assert!(!tooltip.show_view(unrelated, None));
        assert!(!tooltip.show_view(stage_view, None));
        assert!(tooltip.is_showing() && tooltip.is_requested());
        tooltip.hide();
        assert!(!tooltip.is_showing() && !tooltip.is_requested());
    });
    assert!(
        !app.world()
            .non_send::<GuiRuntime>()
            .store()
            .is_on_stage(object)
    );
    // A canceled or replaced request cannot be replayed by a later scheduler run.
    with_tooltip(&mut app, |tooltip| {
        assert!(tooltip.show("Discard", None));
        tooltip.hide();
        assert!(tooltip.show("Last request", Some(Vec2::splat(25.))));
    });
    app.update();
    assert_eq!(view(&app), pane);
    assert_eq!(at(&app), Vec2::splat(25.));
    assert_eq!(
        app.world().get::<crate::GuiText>(pane).unwrap().value(),
        "Last request"
    );
    with_tooltip(&mut app, |tooltip| {
        tooltip.dispose();
        assert!(!tooltip.is_showing() && !tooltip.is_requested());
    });
    assert!(
        !app.world()
            .non_send::<GuiRuntime>()
            .store()
            .contains(object)
    );
    app.update();
    assert!(app.world().get_entity(pane).is_err());
}

#[test]
fn typed_click_observer_can_show_a_tooltip_in_the_same_frame() {
    let (mut app, asset, _) = fixture(true);
    let (_, button) = control(&mut app, asset);
    app.world_mut().entity_mut(button).observe(
        |_: On<crate::GuiClick>, mut tooltip: GuiTooltip| {
            assert!(tooltip.show("Clicked help", Some(Vec2::new(32., 48.))));
            assert!(tooltip.is_requested());
        },
    );
    hover(&mut app, Some(button));
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    app.update();
    assert!(app.world().resource::<GuiTooltipState>().is_showing());
    assert_eq!(at(&app), Vec2::new(32., 48.));
    assert_eq!(
        app.world()
            .get::<crate::GuiText>(view(&app))
            .unwrap()
            .value(),
        "Clicked help"
    );
}

#[test]
fn direct_view_requests_reject_removed_and_despawned_owners_before_sync() {
    let (mut app, asset, _) = fixture(true);
    let root = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), "Component3"))
        .id();
    let window = app
        .world_mut()
        .spawn(windows::FairyGuiWindow::new(asset, "Component3"))
        .id();
    app.update();
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .ecs_root_object(root)
            .is_some()
    );
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .ecs_window_object(window)
            .is_some()
    );
    show(&mut app, None);
    app.update();
    let pane = view(&app);
    app.world_mut().entity_mut(root).remove::<FairyGuiRoot>();
    app.world_mut()
        .entity_mut(window)
        .remove::<windows::FairyGuiWindow>();
    with_tooltip(&mut app, |tooltip| {
        assert!(!tooltip.show_view(root, None));
        assert!(!tooltip.show_view(window, None));
        assert!(tooltip.is_showing());
    });
    app.world_mut().despawn(root);
    app.world_mut().despawn(window);
    with_tooltip(&mut app, |tooltip| {
        assert!(!tooltip.show_view(root, None));
        assert!(!tooltip.show_view(window, None));
        assert!(tooltip.is_showing());
    });
    app.update();
    assert_eq!(view(&app), pane);
    assert!(app.world().resource::<GuiTooltipState>().is_showing());
}

use super::*;
use bevy::input::touch::TouchPhase;

#[test]
fn input_snapshot_exposes_entities_independent_buttons_and_stable_change_ticks() {
    let (mut app, window) = setup();
    let (first, _) = target(&mut app, 1);
    let (second, _) = target(&mut app, 2);
    app.world_mut().entity_mut(second).insert(Hovered(false));
    app.update();
    assert_eq!(
        app.world().resource::<GuiInputState>().hovered(),
        Some(first)
    );
    let tick = app
        .world()
        .get_resource_ref::<GuiInputState>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .get_resource_ref::<GuiInputState>()
            .unwrap()
            .last_changed(),
        tick
    );
    press(&mut app);
    app.world_mut().entity_mut(first).insert(Hovered(false));
    app.world_mut().entity_mut(second).insert(Hovered(true));
    press_right(&mut app);
    let input = app.world().resource::<GuiInputState>();
    assert_eq!(input.hovered(), Some(second));
    assert_eq!(input.captured(), Some(first));
    assert_eq!(
        input
            .captures()
            .iter()
            .map(GuiPointerCapture::pointer)
            .collect::<Vec<_>>(),
        [
            GuiPointerId::Mouse(MouseButton::Left),
            GuiPointerId::Mouse(MouseButton::Right)
        ]
    );
    assert_eq!(
        input
            .capture(GuiPointerId::Mouse(MouseButton::Right))
            .unwrap()
            .target(),
        second
    );
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(Vec2::new(12., 10.)));
    app.update();
    let capture = app
        .world()
        .resource::<GuiInputState>()
        .capture(GuiPointerId::Mouse(MouseButton::Left))
        .unwrap();
    assert_eq!(capture.start_position(), Vec2::splat(10.));
    assert_eq!(capture.position(), Vec2::new(12., 10.));
    assert_eq!(capture.window(), window);
    assert_eq!(capture.hovered(), Some(second));
    assert!(capture.is_dragging());
    assert!(!capture.suppresses_click());
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(Vec2::new(80., 10.)));
    app.update();
    assert!(
        app.world()
            .resource::<GuiInputState>()
            .capture(GuiPointerId::Mouse(MouseButton::Left))
            .unwrap()
            .suppresses_click()
    );
    release(&mut app);
    assert_eq!(
        app.world().resource::<GuiInputState>().captured(),
        Some(second)
    );
    release_right(&mut app);
    assert!(
        app.world()
            .resource::<GuiInputState>()
            .captures()
            .is_empty()
    );
}

#[test]
fn touch_snapshot_is_ordered_and_tracks_current_hits_separately_from_capture() {
    let (mut app, window) = touches::fixture();
    let (first, _) = target(&mut app, 1);
    let (second, _) = target(&mut app, 2);
    touches::hits(&mut app, 20, &[first]);
    touches::send(&mut app, window, 20, TouchPhase::Started, Vec2::splat(10.));
    touches::hits(&mut app, 3, &[second]);
    touches::send(&mut app, window, 3, TouchPhase::Started, Vec2::splat(30.));
    let input = app.world().resource::<GuiInputState>();
    assert_eq!(input.captured(), Some(second));
    assert_eq!(
        input
            .captures()
            .iter()
            .map(GuiPointerCapture::pointer)
            .collect::<Vec<_>>(),
        [GuiPointerId::Touch(3), GuiPointerId::Touch(20)]
    );
    touches::hits(&mut app, 20, &[second]);
    touches::send(&mut app, window, 20, TouchPhase::Moved, Vec2::splat(45.));
    let input = app.world().resource::<GuiInputState>();
    assert_eq!(input.captured_touch(20), Some(first));
    assert_eq!(input.hovered_touch(20), Some(second));
    let capture = input.capture(GuiPointerId::Touch(20)).unwrap();
    assert_eq!(capture.start_position(), Vec2::splat(10.));
    assert_eq!(capture.position(), Vec2::splat(45.));
    assert_eq!(capture.window(), window);
    assert!(capture.is_dragging() && capture.suppresses_click());
    touches::send(&mut app, window, 3, TouchPhase::Canceled, Vec2::splat(30.));
    assert_eq!(
        app.world().resource::<GuiInputState>().captured(),
        Some(first)
    );
    app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
    app.update();
    let input = app.world().resource::<GuiInputState>();
    assert!(!input.wants_pointer_input());
    assert!(input.captures().is_empty());
    assert_eq!(input.hovered(), None);
}

#[test]
fn changing_the_input_window_releases_old_mouse_captures_without_cross_window_drags() {
    let (mut app, original_window) = setup();
    let (entity, _) = target(&mut app, 1);
    press(&mut app);
    press_right(&mut app);
    assert!(
        app.world()
            .resource::<GuiInputState>()
            .captures()
            .iter()
            .all(|capture| capture.window() == original_window)
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .drain_events()
        .for_each(drop);
    let mut other = Window {
        focused: true,
        ..default()
    };
    other.set_cursor_position(Some(Vec2::new(300., 200.)));
    let other = app.world_mut().spawn(other).id();
    app.world_mut().resource_mut::<GuiInputSettings>().window = Some(other);
    app.update();
    assert!(
        app.world()
            .resource::<GuiInputState>()
            .captures()
            .is_empty()
    );
    let events: Vec<_> = app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .drain_events()
        .filter(|event| event.is_at_target())
        .collect();
    assert!(!events.iter().any(|event| matches!(
        event.kind,
        EventKind::MouseDrag | EventKind::Click | EventKind::RightClick
    )));
    for kind in [EventKind::MouseUp, EventKind::RightMouseUp] {
        let event = events.iter().find(|event| event.kind == kind).unwrap();
        assert_eq!(event.pointer, Some(fairygui_core::Point::new(10., 10.)));
    }
    release(&mut app);
    release_right(&mut app);
    press(&mut app);
    let input = app.world().resource::<GuiInputState>();
    assert_eq!(input.captured(), Some(entity));
    assert_eq!(
        input
            .capture(GuiPointerId::Mouse(MouseButton::Left))
            .unwrap()
            .window(),
        other
    );
}

#[test]
fn observers_read_current_capture_and_sync_filters_hidden_or_despawned_roots() {
    #[derive(Resource, Default)]
    struct Presses(Vec<Option<Entity>>);
    fn pressed(event: On<GuiEvent>, input: Res<GuiInputState>, mut presses: ResMut<Presses>) {
        if event.event.kind == EventKind::MouseDown {
            presses.0.push(input.captured());
        }
    }
    #[derive(Resource)]
    struct Mutation {
        owner: Entity,
        hide: bool,
    }
    fn mutate(
        mutation: Option<Res<Mutation>>,
        mut roots: Query<&mut FairyGuiRoot>,
        mut commands: Commands,
    ) {
        let Some(mutation) = mutation else { return };
        if mutation.hide {
            roots.get_mut(mutation.owner).unwrap().active = false;
        } else {
            commands.entity(mutation.owner).despawn();
        }
        commands.remove_resource::<Mutation>();
    }
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .init_resource::<Presses>()
    .add_message::<MouseWheel>()
    .add_systems(Update, mutate.in_set(GuiSystems::Respond));
    let mut window = Window {
        focused: true,
        ..default()
    };
    window.set_cursor_position(Some(Vec2::splat(10.)));
    app.world_mut().spawn((window, PrimaryWindow));
    let package = fairygui_bevy::core::FuiPackage::parse(
        "Guide",
        include_bytes!("../../../fairygui-demo/assets/ui/Guide.fui"),
    )
    .unwrap();
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package,
            res_key: "ui/Guide".into(),
        });
    let owner = app.world_mut().spawn(FairyGuiRoot::new(asset, "Main")).id();
    app.update();
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    app.world_mut()
        .entity_mut(view)
        .insert((
            Hovered(true),
            ComputedStackIndex(100),
            RelativeCursorPosition {
                cursor_over: true,
                normalized: Some(Vec2::ZERO),
            },
        ))
        .observe(pressed);
    press(&mut app);
    assert_eq!(
        app.world().resource::<GuiInputState>().captured(),
        Some(view)
    );
    assert_eq!(app.world().resource::<Presses>().0, [Some(view)]);
    app.insert_resource(Mutation { owner, hide: true });
    app.update();
    let input = app.world().resource::<GuiInputState>();
    assert_eq!(input.captured(), None);
    assert_eq!(input.hovered(), None);
    assert!(!input.wants_pointer_input());
    assert!(
        app.world().get_entity(view).is_ok(),
        "inactive roots retain their views"
    );
    app.update(); // Native cancellation is routed by the next Input phase.
    app.world_mut()
        .get_mut::<FairyGuiRoot>(owner)
        .unwrap()
        .active = true;
    release(&mut app);
    press(&mut app);
    assert_eq!(
        app.world().resource::<GuiInputState>().captured(),
        Some(view)
    );
    app.insert_resource(Mutation { owner, hide: false });
    app.update();
    assert!(app.world().get_entity(view).is_err());
    let input = app.world().resource::<GuiInputState>();
    assert_eq!(input.captured(), None);
    assert_eq!(input.hovered(), None);
    assert!(!input.wants_pointer_input());
    assert_eq!(
        app.world().resource::<Presses>().0,
        [Some(view), Some(view)]
    );
}

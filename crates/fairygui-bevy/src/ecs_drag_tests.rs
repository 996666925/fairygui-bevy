use super::*;
use crate::{
    FairyGuiPlugin, GuiDrag, GuiDragSource, GuiDragState, GuiDrop, GuiDropTarget, GuiIcon,
    GuiWorldExt,
};
use bevy::ecs::system::SystemState;
use bevy::input::touch::{TouchInput, TouchPhase};
use bevy::picking::hover::Hovered;
use bevy::ui::{ComputedStackIndex, RelativeCursorPosition};

fn start(app: &mut App, source: Entity, touch_id: Option<u64>) -> bool {
    let mut system = SystemState::<GuiDrag>::new(app.world_mut());
    let result = system.get_mut(app.world_mut()).unwrap().start(
        source,
        GuiDragSource {
            icon: None,
            data: Some("explicit".into()),
        },
        touch_id,
    );
    assert_eq!(
        system.get_mut(app.world_mut()).unwrap().is_dragging(),
        app.world().non_send::<GuiRuntime>().is_dragging()
    );
    system.apply(app.world_mut());
    result
}
fn cancel(app: &mut App) -> bool {
    let mut system = SystemState::<GuiDrag>::new(app.world_mut());
    let mut drag = system.get_mut(app.world_mut()).unwrap();
    let result = drag.cancel();
    assert!(!drag.is_dragging());
    system.apply(app.world_mut());
    result
}

#[derive(Resource, Default)]
struct Drops(Vec<GuiDrop>);
fn receive(
    event: On<GuiDrop>,
    mut drops: ResMut<Drops>,
    _commands: Commands,
    mut gui_edit: crate::GuiMut,
) {
    drops.0.push(event.event().clone());
    gui_edit.edit(event.entity, |object: &mut crate::GuiObjectMut| {
        object.set_icon(event.data.clone().unwrap_or_default())
    });
}

fn fixture() -> (App, Entity, Entity, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .init_resource::<Drops>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let mut window = Window {
        focused: true,
        ..default()
    };
    window.set_cursor_position(Some(Vec2::splat(10.)));
    let window = app.world_mut().spawn((window, PrimaryWindow)).id();
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
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "Demo_Drag&Drop"))
        .id();
    app.update();
    app.update();
    let runtime = app.world().non_send::<GuiRuntime>();
    let root = app.world().get::<GuiRootInstance>(owner).unwrap().object;
    let source = runtime.store().child_by_name(root, "b").unwrap();
    let receiver = runtime.store().child_by_name(root, "c").unwrap();
    let source = runtime.entity_of(source).unwrap();
    let receiver = runtime.entity_of(receiver).unwrap();
    let icon = app
        .world()
        .get::<GuiIcon>(source)
        .unwrap()
        .value()
        .to_owned();
    assert!(!icon.is_empty());
    app.world_mut().entity_mut(source).insert(GuiDragSource {
        icon: Some(icon.clone()),
        data: Some(icon),
    });
    app.world_mut()
        .entity_mut(receiver)
        .insert(GuiDropTarget)
        .observe(receive);
    hover(&mut app, source);
    (app, window, owner, source, receiver)
}
fn hover(app: &mut App, target: Entity) {
    let entities: Vec<_> = app
        .world_mut()
        .query::<(Entity, &GObjectRef)>()
        .iter(app.world())
        .map(|(entity, _)| entity)
        .collect();
    for entity in entities {
        app.world_mut()
            .entity_mut(entity)
            .insert(Hovered(entity == target));
    }
    app.world_mut().entity_mut(target).insert((
        ComputedStackIndex(100),
        RelativeCursorPosition {
            cursor_over: true,
            normalized: Some(Vec2::ZERO),
        },
    ));
}
fn mouse(app: &mut App, down: bool) {
    if down {
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
    } else {
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
    }
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
}
fn move_mouse(app: &mut App, window: Entity, position: Vec2) {
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(position));
    app.update();
}

fn record_drop(event: On<GuiDrop>, mut drops: ResMut<Drops>) {
    drops.0.push(event.event().clone());
}

fn owned_control(app: &mut App, asset: Handle<FuiAsset>, window: bool) -> Entity {
    let mut entity = app.world_mut().spawn_empty();
    if window {
        let mut source = crate::FairyGuiWindow::new(asset, "Button1");
        source.center_on_create = false;
        source.draggable = false;
        entity.insert(source);
    } else {
        entity.insert(FairyGuiRoot::new(asset, "Button1"));
    }
    entity.id()
}

#[test]
fn owner_drag_components_start_from_descendants_and_deliver_to_the_configured_receiver() {
    for windows in [false, true] {
        let (mut app, window, page, old_source, old_receiver) = fixture();
        app.world_mut()
            .entity_mut(old_source)
            .remove::<GuiDragSource>();
        app.world_mut()
            .entity_mut(old_receiver)
            .remove::<GuiDropTarget>();
        let asset = app.world().get::<FairyGuiRoot>(page).unwrap().asset.clone();
        let source = owned_control(&mut app, asset.clone(), windows);
        let target = owned_control(&mut app, asset, windows);
        app.world_mut().entity_mut(source).insert((
            GuiDragSource {
                icon: None,
                data: Some("owner".into()),
            },
            crate::GuiDraggable::default(),
        ));
        app.world_mut()
            .entity_mut(target)
            .insert(GuiDropTarget)
            .observe(record_drop);
        app.update();
        app.update();
        let mut query = SystemState::<crate::GuiQuery>::new(app.world_mut());
        let gui = query.get(app.world()).unwrap();
        let source_view = gui.view(source).unwrap();
        let target_view = gui.view(target).unwrap();
        let pressed = gui.child_by_name(source, "title").unwrap();
        let released = gui.child_by_name(target, "title").unwrap();
        let initial_position = gui.transform(source).unwrap().position();
        for round in 0..3 {
            if round == 1 {
                app.world_mut()
                    .entity_mut(source_view)
                    .insert(GuiDragSource {
                        icon: None,
                        data: Some("view".into()),
                    });
                app.world_mut()
                    .entity_mut(target_view)
                    .insert(GuiDropTarget)
                    .observe(record_drop);
            } else if round == 2 {
                app.world_mut()
                    .entity_mut(source_view)
                    .remove::<GuiDragSource>();
                app.world_mut()
                    .entity_mut(target_view)
                    .remove::<GuiDropTarget>();
            }
            hover(&mut app, pressed);
            move_mouse(&mut app, window, Vec2::splat(10.));
            mouse(&mut app, true);
            move_mouse(&mut app, window, Vec2::splat(40.));
            let state = app.world().resource::<GuiDragState>();
            assert_eq!(
                state.source(),
                Some(source_view),
                "windows={windows}, round={round}"
            );
            assert_eq!(
                state.data(),
                Some(if round == 1 { "view" } else { "owner" })
            );
            assert!(
                !app.world()
                    .get::<crate::GuiObjectDragState>(source)
                    .unwrap()
                    .is_dragging()
            );
            let gui = query.get(app.world()).unwrap();
            assert_eq!(gui.transform(source).unwrap().position(), initial_position);
            hover(&mut app, released);
            move_mouse(&mut app, window, Vec2::splat(70.));
            let receiver = if round == 1 { target_view } else { target };
            assert_eq!(
                app.world().resource::<GuiDragState>().target(),
                Some(receiver)
            );
            mouse(&mut app, false);
            let drops = &app.world().resource::<Drops>().0;
            assert_eq!(drops.len(), round + 1);
            assert_eq!(
                (drops[round].entity, drops[round].source),
                (receiver, source_view)
            );
        }
    }
}

#[test]
fn owner_drag_policies_wait_for_assets_and_rebind_after_template_replacement() {
    use bevy::ecs::system::RunSystemOnce;
    for windows in [false, true] {
        let (mut app, _, page, source, target) = fixture();
        app.world_mut().entity_mut(source).remove::<GuiDragSource>();
        app.world_mut().entity_mut(target).remove::<GuiDropTarget>();
        let asset = app.world().get::<FairyGuiRoot>(page).unwrap().asset.clone();
        let pending = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
        let owner = owned_control(&mut app, pending.clone(), windows);
        app.world_mut()
            .entity_mut(owner)
            .insert((GuiDragSource::default(), GuiDropTarget));
        app.update();
        let runtime = app.world().non_send::<GuiRuntime>();
        assert!(runtime.drag_source_bindings.is_empty());
        assert!(runtime.drop_targets.is_empty());
        let loaded = {
            let assets = app.world().resource::<Assets<FuiAsset>>();
            let source = assets.get(&asset).unwrap();
            FuiAsset {
                package: source.package.clone(),
                res_key: source.res_key.clone(),
            }
        };
        app.world_mut()
            .resource_mut::<Assets<FuiAsset>>()
            .insert(pending.id(), loaded)
            .unwrap();
        app.update();
        let mut query = SystemState::<crate::GuiQuery>::new(app.world_mut());
        let view = query.get(app.world()).unwrap().view(owner).unwrap();
        let object = app.world().get::<GObjectRef>(view).unwrap().0;
        let runtime = app.world().non_send::<GuiRuntime>();
        assert!(runtime.drag_source_bindings.contains(&object));
        assert_eq!(runtime.ecs_drop_target(object), Some((object, owner)));
        // A copied reference must not override the owner's declaration.
        app.world_mut()
            .spawn((GObjectRef(object), GuiDragSource::default(), GuiDropTarget));
        if windows {
            app.world_mut()
                .get_mut::<crate::FairyGuiWindow>(owner)
                .unwrap()
                .item_name = "Button2".into();
        } else {
            app.world_mut()
                .get_mut::<FairyGuiRoot>(owner)
                .unwrap()
                .item_name = "Button2".into();
        }
        app.world_mut()
            .run_system_once(prepare_drag_targets)
            .unwrap();
        app.world_mut()
            .run_system_once(crate::runtime::object_drag::prepare_object_drags)
            .unwrap();
        let runtime = app.world().non_send::<GuiRuntime>();
        assert!(!runtime.drag_source_bindings.contains(&object));
        assert_eq!(runtime.ecs_drop_target(object), None);
        app.update();
        let new_view = query.get(app.world()).unwrap().view(owner).unwrap();
        assert_ne!(view, new_view);
        let new_object = app.world().get::<GObjectRef>(new_view).unwrap().0;
        let runtime = app.world().non_send::<GuiRuntime>();
        assert!(!runtime.drag_source_bindings.contains(&object));
        assert!(runtime.drag_source_bindings.contains(&new_object));
        assert_eq!(
            runtime.ecs_drop_target(new_object),
            Some((new_object, owner))
        );
    }
}

#[test]
fn owner_drag_session_cancels_on_source_invalidation_before_lifecycle_sync() {
    use bevy::ecs::system::RunSystemOnce;
    for windows in [false, true] {
        for change in 0..3 {
            let (mut app, window, page, old_source, _) = fixture();
            app.world_mut()
                .entity_mut(old_source)
                .remove::<GuiDragSource>();
            let asset = app.world().get::<FairyGuiRoot>(page).unwrap().asset.clone();
            let owner = owned_control(&mut app, asset, windows);
            app.world_mut()
                .entity_mut(owner)
                .insert(GuiDragSource::default());
            app.update();
            let mut query = SystemState::<crate::GuiQuery>::new(app.world_mut());
            let gui = query.get(app.world()).unwrap();
            let view = gui.view(owner).unwrap();
            let pressed = gui.child_by_name(owner, "title").unwrap();
            hover(&mut app, pressed);
            mouse(&mut app, true);
            move_mouse(&mut app, window, Vec2::splat(40.));
            assert_eq!(app.world().resource::<GuiDragState>().source(), Some(view));
            // Removing configuration disables future starts; the active session
            // continues while its declaring owner still identifies this view.
            app.world_mut().entity_mut(owner).remove::<GuiDragSource>();
            app.update();
            assert!(app.world().resource::<GuiDragState>().is_dragging());
            if change == 2 {
                app.world_mut().despawn(owner);
            } else if windows {
                if change == 0 {
                    app.world_mut()
                        .get_mut::<crate::FairyGuiWindow>(owner)
                        .unwrap()
                        .item_name = "Button2".into();
                } else {
                    let source = app
                        .world()
                        .get::<crate::FairyGuiWindow>(owner)
                        .unwrap()
                        .clone();
                    app.world_mut()
                        .entity_mut(owner)
                        .remove::<crate::FairyGuiWindow>()
                        .insert(source);
                }
            } else if change == 0 {
                app.world_mut()
                    .get_mut::<FairyGuiRoot>(owner)
                    .unwrap()
                    .item_name = "Button2".into();
            } else {
                let source = app.world().get::<FairyGuiRoot>(owner).unwrap().clone();
                app.world_mut()
                    .entity_mut(owner)
                    .remove::<FairyGuiRoot>()
                    .insert(source);
            }
            app.world_mut()
                .run_system_once(prepare_drag_targets)
                .unwrap();
            assert!(
                !app.world().non_send::<GuiRuntime>().is_dragging(),
                "windows={windows}, change={change}"
            );
            assert!(app.world().resource::<Drops>().0.is_empty());
        }
    }
}

#[test]
fn owner_drag_components_use_the_captured_finger_and_owner_drop_observer() {
    let (mut app, window, page, source, _) = fixture();
    app.world_mut().entity_mut(source).remove::<GuiDragSource>();
    app.world_mut().entity_mut(page).insert(GuiDragSource {
        icon: None,
        data: Some("touch-owner".into()),
    });
    let asset = app.world().get::<FairyGuiRoot>(page).unwrap().asset.clone();
    let target = owned_control(&mut app, asset, false);
    app.world_mut()
        .entity_mut(target)
        .insert(GuiDropTarget)
        .observe(record_drop);
    app.update();
    let mut query = SystemState::<crate::GuiQuery>::new(app.world_mut());
    let gui = query.get(app.world()).unwrap();
    let source_view = gui.view(page).unwrap();
    let released = gui.child_by_name(target, "title").unwrap();
    app.init_resource::<bevy::picking::hover::HoverMap>();
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(None);
    touch(
        &mut app,
        window,
        7,
        source,
        TouchPhase::Started,
        Vec2::splat(10.),
    );
    touch(
        &mut app,
        window,
        7,
        source,
        TouchPhase::Moved,
        Vec2::splat(40.),
    );
    assert_eq!(
        app.world().resource::<GuiDragState>().source(),
        Some(source_view)
    );
    touch(
        &mut app,
        window,
        7,
        released,
        TouchPhase::Ended,
        Vec2::splat(70.),
    );
    let drops = &app.world().resource::<Drops>().0;
    assert_eq!(drops.len(), 1);
    assert_eq!(
        (drops[0].entity, drops[0].source, drops[0].touch_id),
        (target, source_view, Some(7))
    );
    assert_eq!(drops[0].data.as_deref(), Some("touch-owner"));
}

#[test]
fn ecs_drag_threshold_proxy_nearest_receiver_same_frame_icon_and_repeated_cleanup() {
    let (mut app, window, owner, source, receiver) = fixture();
    let root_view = app.world().get::<crate::GuiRootView>(owner).unwrap().0;
    app.world_mut()
        .entity_mut(root_view)
        .insert(GuiDropTarget)
        .observe(receive);
    let icon = app
        .world()
        .get::<GuiIcon>(source)
        .unwrap()
        .value()
        .to_owned();
    for round in 0..2 {
        hover(&mut app, source);
        move_mouse(&mut app, window, Vec2::splat(10.));
        mouse(&mut app, true);
        move_mouse(&mut app, window, Vec2::splat(12.));
        assert!(!app.world().resource::<GuiDragState>().is_dragging());
        move_mouse(&mut app, window, Vec2::splat(40.));
        let state = app.world().resource::<GuiDragState>();
        assert_eq!(state.source(), Some(source));
        assert_eq!(state.data(), Some(icon.as_str()));
        let agent = state.agent().unwrap();
        let model = app.world().get::<GObjectRef>(agent).unwrap().0;
        let runtime = app.world().non_send::<GuiRuntime>();
        let proxy = runtime.store().object(model).unwrap();
        assert_eq!(proxy.size(), fairygui_core::Size::new(100., 100.));
        assert_eq!(proxy.pivot(), fairygui_core::Point::new(0.5, 0.5));
        assert!(proxy.pivot_as_anchor() && !proxy.touchable());
        assert!(!runtime.popup_stack().contains(&model));
        assert_eq!(app.world().get::<GlobalZIndex>(agent).unwrap().0, 1_000_000);
        hover(&mut app, receiver);
        move_mouse(&mut app, window, Vec2::new(70., 60.));
        assert_eq!(
            app.world().resource::<GuiDragState>().target(),
            Some(receiver)
        );
        let tick = app
            .world()
            .get_resource_ref::<GuiDragState>()
            .unwrap()
            .last_changed();
        app.update();
        assert_eq!(
            app.world()
                .get_resource_ref::<GuiDragState>()
                .unwrap()
                .last_changed(),
            tick
        );
        mouse(&mut app, false);
        let drops = &app.world().resource::<Drops>().0;
        assert_eq!(drops.len(), round + 1);
        let event = drops.last().unwrap();
        assert_eq!(
            (event.entity, event.source, event.touch_id),
            (receiver, source, None)
        );
        assert_eq!(event.position, Vec2::new(70., 60.));
        assert_eq!(app.world().get::<GuiIcon>(receiver).unwrap().value(), icon);
        assert!(!app.world().resource::<GuiDragState>().is_dragging());
        assert!(app.world().get_entity(agent).is_err());
        let runtime = app.world().non_send::<GuiRuntime>();
        assert!(!runtime.store().contains(model));
        assert!(!runtime.object_assets.contains_key(&model));
    }
}

#[test]
fn ecs_drag_cancellation_source_disposal_and_hidden_source_never_deliver_or_restart() {
    for policy in 0..7 {
        let (mut app, window, owner, source, receiver) = fixture();
        mouse(&mut app, true);
        move_mouse(&mut app, window, Vec2::splat(40.));
        let agent = app.world().resource::<GuiDragState>().agent().unwrap();
        hover(&mut app, receiver);
        move_mouse(&mut app, window, Vec2::splat(70.));
        match policy {
            0 => {
                assert!(cancel(&mut app));
            }
            1 => {
                app.world_mut().despawn(owner);
            }
            2 => {
                if let Some(mut object) = app.world_mut().gui_mut(source) {
                    object.set_visible(false)
                }
            }
            3 => {
                app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
            }
            4 => {
                app.world_mut()
                    .get_mut::<FairyGuiRoot>(owner)
                    .unwrap()
                    .active = false;
            }
            5 => {
                if let Some(mut object) = app.world_mut().gui_mut(source) {
                    object.set_touchable(false)
                }
            }
            _ => {
                let handle = app
                    .world()
                    .get::<FairyGuiRoot>(owner)
                    .unwrap()
                    .asset
                    .clone();
                app.world_mut()
                    .insert_resource(crate::GuiGlobalWaitSettings::new(handle, "WindowA"));
                app.world_mut()
                    .resource_mut::<crate::GuiGlobalWaitSettings>()
                    .visible = true;
            }
        }
        app.update();
        assert!(
            !app.world().resource::<GuiDragState>().is_dragging(),
            "policy={policy}"
        );
        assert!(app.world().get_entity(agent).is_err());
        app.update();
        assert!(!app.world().resource::<GuiDragState>().is_dragging());
        mouse(&mut app, false);
        assert!(app.world().resource::<Drops>().0.is_empty());
    }
}

#[test]
fn ecs_drag_explicit_starts_require_owning_live_pointer_and_unmarked_release_is_ignored() {
    let (mut app, window, _, source, receiver) = fixture();
    assert!(!start(&mut app, source, None));
    app.update();
    assert!(!app.world().resource::<GuiDragState>().is_dragging());
    app.world_mut().entity_mut(source).remove::<GuiDragSource>();
    mouse(&mut app, true);
    assert!(!start(&mut app, receiver, None));
    app.update();
    assert!(!app.world().resource::<GuiDragState>().is_dragging());
    assert!(start(&mut app, source, None));
    assert!(!start(&mut app, receiver, None));
    app.update();
    assert!(app.world().resource::<GuiDragState>().is_dragging());
    assert_eq!(app.world().resource::<GuiDragState>().agent(), None);
    app.world_mut()
        .entity_mut(receiver)
        .remove::<GuiDropTarget>();
    hover(&mut app, receiver);
    move_mouse(&mut app, window, Vec2::splat(70.));
    mouse(&mut app, false);
    assert!(!app.world().resource::<GuiDragState>().is_dragging());
    assert!(app.world().resource::<Drops>().0.is_empty());
    assert!(!start(&mut app, source, None));
    app.update();
    assert!(!app.world().resource::<GuiDragState>().is_dragging());
}

#[test]
fn ecs_drag_observer_starts_immediately_and_direct_cancel_reclaims_proxy_without_a_drop() {
    fn begin<E: GuiEventType>(event: On<E>, icons: Query<&GuiIcon>, mut drag: GuiDrag) {
        let begin = match event.event.kind {
            fairygui_core::object::EventKind::MouseDown => event.event.touch_id.is_none(),
            fairygui_core::object::EventKind::TouchBegin => true,
            _ => false,
        };
        if !begin {
            return;
        }
        assert!(!drag.is_dragging());
        let configuration = GuiDragSource {
            icon: Some(icons.get(event.entity).unwrap().value().into()),
            data: Some("observer".into()),
        };
        assert!(drag.start(event.entity, configuration.clone(), event.event.touch_id));
        assert!(drag.is_dragging());
        assert!(!drag.start(event.entity, configuration, event.event.touch_id));
    }
    for touch_input in [false, true] {
        let (mut app, window, _, source, receiver) = fixture();
        app.world_mut()
            .entity_mut(source)
            .remove::<GuiDragSource>()
            .observe(begin::<GuiMouseDown>)
            .observe(begin::<GuiTouchBegin>);
        if touch_input {
            app.init_resource::<bevy::picking::hover::HoverMap>();
            app.world_mut()
                .get_mut::<Window>(window)
                .unwrap()
                .set_cursor_position(None);
            touch(
                &mut app,
                window,
                7,
                source,
                TouchPhase::Started,
                Vec2::splat(10.),
            );
        } else {
            mouse(&mut app, true);
        }
        let state = app.world().resource::<GuiDragState>();
        assert_eq!(state.source(), Some(source));
        assert_eq!(state.data(), Some("observer"));
        assert_eq!(state.touch_id(), touch_input.then_some(7));
        let agent = state.agent().unwrap();
        let model = app.world().get::<GObjectRef>(agent).unwrap().0;
        assert!(cancel(&mut app));
        assert!(!cancel(&mut app));
        assert!(!app.world().non_send::<GuiRuntime>().store().contains(model));
        assert!(app.world().get_entity(agent).is_err());
        app.update();
        assert!(!app.world().resource::<GuiDragState>().is_dragging());
        if touch_input {
            touch(
                &mut app,
                window,
                7,
                receiver,
                TouchPhase::Moved,
                Vec2::splat(70.),
            );
            touch(
                &mut app,
                window,
                7,
                receiver,
                TouchPhase::Ended,
                Vec2::splat(70.),
            );
        } else {
            hover(&mut app, receiver);
            move_mouse(&mut app, window, Vec2::splat(70.));
            mouse(&mut app, false);
        }
        assert!(app.world().resource::<Drops>().0.is_empty());
    }
}

#[test]
fn ecs_drag_direct_start_resolves_owners_and_does_not_retain_missing_or_pending_targets() {
    let (mut app, _, owner, source, _) = fixture();
    app.world_mut().entity_mut(source).remove::<GuiDragSource>();
    let handle = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
    let pending = app
        .world_mut()
        .spawn(FairyGuiRoot::new(handle.clone(), "Demo_Drag&Drop"))
        .id();
    app.update();
    mouse(&mut app, true);
    assert!(!start(&mut app, pending, None));
    assert!(!start(&mut app, Entity::PLACEHOLDER, None));
    assert!(start(&mut app, owner, None));
    assert!(app.world().non_send::<GuiRuntime>().is_dragging());
    app.update();
    assert_eq!(
        app.world().resource::<GuiDragState>().source(),
        Some(app.world().get::<crate::GuiRootView>(owner).unwrap().0)
    );
    assert!(cancel(&mut app));
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
    app.update();
    app.update();
    assert!(app.world().get::<crate::GuiRootView>(pending).is_some());
    assert!(!app.world().resource::<GuiDragState>().is_dragging());
    assert!(!app.world().non_send::<GuiRuntime>().is_dragging());
}

#[test]
fn ecs_drag_release_resolves_remaining_parent_marker_and_live_receiver_eligibility() {
    for blocked in [false, true] {
        let (mut app, window, owner, source, receiver) = fixture();
        let root = app.world().get::<crate::GuiRootView>(owner).unwrap().0;
        app.world_mut()
            .entity_mut(root)
            .insert(GuiDropTarget)
            .observe(receive);
        mouse(&mut app, true);
        move_mouse(&mut app, window, Vec2::splat(40.));
        hover(&mut app, receiver);
        move_mouse(&mut app, window, Vec2::splat(70.));
        assert_eq!(
            app.world().resource::<GuiDragState>().target(),
            Some(receiver)
        );
        app.world_mut()
            .entity_mut(receiver)
            .remove::<GuiDropTarget>();
        if blocked {
            let model = app.world().get::<GObjectRef>(root).unwrap().0;
            app.world_mut()
                .non_send_mut::<GuiRuntime>()
                .store_mut()
                .set_visible(model, false);
        }
        mouse(&mut app, false);
        let drops = &app.world().resource::<Drops>().0;
        assert_eq!(drops.len(), usize::from(!blocked));
        if !blocked {
            assert_eq!((drops[0].entity, drops[0].source), (root, source));
        }
    }
}

fn touch(
    app: &mut App,
    window: Entity,
    id: u64,
    target: Entity,
    phase: TouchPhase,
    position: Vec2,
) {
    use bevy::picking::{backend::HitData, hover::HoverMap, pointer::PointerId};
    app.world_mut().resource_mut::<HoverMap>().insert(
        PointerId::Touch(id),
        [(
            target,
            HitData::new(Entity::PLACEHOLDER, 0., Some(Vec3::ZERO), None),
        )]
        .into_iter()
        .collect(),
    );
    app.world_mut().write_message(TouchInput {
        window,
        id,
        phase,
        position,
        force: None,
    });
    app.update();
}
#[test]
fn ecs_drag_touch_owner_survives_another_finger_release_and_cancellation_cleans_proxy() {
    for cancel in [false, true] {
        let (mut app, window, _, source, receiver) = fixture();
        app.init_resource::<bevy::picking::hover::HoverMap>();
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .set_cursor_position(None);
        touch(
            &mut app,
            window,
            7,
            source,
            TouchPhase::Started,
            Vec2::splat(10.),
        );
        touch(
            &mut app,
            window,
            7,
            source,
            TouchPhase::Moved,
            Vec2::splat(40.),
        );
        let state = app.world().resource::<GuiDragState>();
        assert_eq!(state.touch_id(), Some(7));
        let agent = state.agent().unwrap();
        touch(
            &mut app,
            window,
            8,
            receiver,
            TouchPhase::Started,
            Vec2::splat(70.),
        );
        touch(
            &mut app,
            window,
            8,
            receiver,
            TouchPhase::Ended,
            Vec2::splat(70.),
        );
        assert_eq!(app.world().resource::<GuiDragState>().touch_id(), Some(7));
        touch(
            &mut app,
            window,
            7,
            receiver,
            if cancel {
                TouchPhase::Canceled
            } else {
                TouchPhase::Ended
            },
            Vec2::splat(70.),
        );
        assert!(app.world().get_entity(agent).is_err());
        assert!(!app.world().resource::<GuiDragState>().is_dragging());
        let drops = &app.world().resource::<Drops>().0;
        assert_eq!(drops.len(), usize::from(!cancel));
        if !cancel {
            assert_eq!(drops[0].touch_id, Some(7));
        }
    }
}

#[test]
fn direct_drag_rejects_changed_or_removed_owner_templates_before_lifecycle_sync() {
    for change in 0..4 {
        let (mut app, _, owner, source, _) = fixture();
        app.world_mut().entity_mut(source).remove::<GuiDragSource>();
        mouse(&mut app, true);
        match change {
            0 => {
                app.world_mut()
                    .get_mut::<FairyGuiRoot>(owner)
                    .unwrap()
                    .item_name = "Demo_Text".into()
            }
            1 => {
                app.world_mut().entity_mut(owner).remove::<FairyGuiRoot>();
            }
            2 => {
                app.world_mut().despawn(owner);
            }
            _ => {
                let source = app.world().get::<FairyGuiRoot>(owner).unwrap().clone();
                app.world_mut()
                    .entity_mut(owner)
                    .remove::<FairyGuiRoot>()
                    .insert(source);
            }
        }
        assert!(!start(&mut app, owner, None), "change={change}");
        assert!(!app.world().non_send::<GuiRuntime>().is_dragging());
        // The capture remains usable through its explicit retained control view.
        assert!(start(&mut app, source, None));
        assert!(cancel(&mut app));
    }
}

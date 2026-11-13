use super::*;
use crate::{
    FairyGuiPlugin, GuiDragCommand, GuiDragSource, GuiDragState, GuiDrop, GuiDropTarget, GuiIcon,
    GuiObjectAction, GuiObjectCommand,
};
use bevy::input::touch::{TouchInput, TouchPhase};
use bevy::picking::hover::Hovered;
use bevy::ui::{ComputedStackIndex, RelativeCursorPosition};

#[derive(Resource, Default)]
struct Drops(Vec<GuiDrop>);
fn receive(event: On<GuiDrop>, mut drops: ResMut<Drops>, mut commands: Commands) {
    drops.0.push(event.event().clone());
    commands.write_message(GuiObjectCommand::new(
        event.entity,
        GuiObjectAction::SetIcon(event.data.clone().unwrap_or_default()),
    ));
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
                app.world_mut().write_message(GuiDragCommand::Cancel);
            }
            1 => {
                app.world_mut().despawn(owner);
            }
            2 => {
                app.world_mut().write_message(GuiObjectCommand::new(
                    source,
                    GuiObjectAction::SetVisible(false),
                ));
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
                app.world_mut().write_message(GuiObjectCommand::new(
                    source,
                    GuiObjectAction::SetTouchable(false),
                ));
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
                    .write_message(crate::GuiGlobalWaitCommand::Show { message: None });
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
    let start = |source| GuiDragCommand::Start {
        source,
        icon: None,
        data: Some("explicit".into()),
        touch_id: None,
    };
    app.world_mut().write_message(start(source));
    app.update();
    assert!(!app.world().resource::<GuiDragState>().is_dragging());
    app.world_mut().entity_mut(source).remove::<GuiDragSource>();
    mouse(&mut app, true);
    app.world_mut().write_message(start(receiver));
    app.update();
    assert!(!app.world().resource::<GuiDragState>().is_dragging());
    app.world_mut().write_message(start(source));
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
    app.world_mut().write_message(start(source));
    app.update();
    assert!(!app.world().resource::<GuiDragState>().is_dragging());
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

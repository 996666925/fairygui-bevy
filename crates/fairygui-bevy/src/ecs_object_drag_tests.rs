use super::*;
use crate::{FairyGuiPlugin, GuiEvent, GuiWorldExt};
use bevy::input::touch::{TouchInput, TouchPhase};
use bevy::picking::{backend::HitData, hover::HoverMap, hover::Hovered, pointer::PointerId};
use bevy::ui::{ComputedStackIndex, RelativeCursorPosition};
use bevy::window::PrimaryWindow;
use fairygui_core::object::EventKind;

#[derive(Resource, Default)]
struct Events(Vec<(EventKind, Option<u64>)>);
fn observe(event: On<GuiEvent>, mut events: ResMut<Events>) {
    if !event.event.bubbling
        && matches!(
            event.event.kind,
            EventKind::DragStart | EventKind::DragMove | EventKind::DragEnd | EventKind::Click
        )
    {
        events
            .0
            .push((event.event.kind.clone(), event.event.touch_id));
    }
}
fn fixture() -> (App, Entity, Entity, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .init_resource::<Events>()
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
    let page = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "Demo_Drag&Drop"))
        .id();
    app.update();
    app.update();
    let runtime = app.world().non_send::<GuiRuntime>();
    let root = app.world().get::<GuiRootInstance>(page).unwrap().object;
    let control = runtime.store().child_by_name(root, "a").unwrap();
    let bounds = runtime.store().child_by_name(root, "bounds").unwrap();
    let control = runtime.entity_of(control).unwrap();
    let bounds = runtime.entity_of(bounds).unwrap();
    app.world_mut()
        .entity_mut(control)
        .insert(GuiDraggable::default())
        .observe(observe);
    hover(&mut app, control);
    app.update();
    (app, window, page, control, bounds)
}
fn hover(app: &mut App, target: Entity) {
    let entities: Vec<_> = app
        .world_mut()
        .query::<(Entity, &GObjectRef)>()
        .iter(app.world())
        .map(|(e, _)| e)
        .collect();
    for e in entities {
        app.world_mut().entity_mut(e).insert(Hovered(e == target));
    }
    app.world_mut().entity_mut(target).insert((
        ComputedStackIndex(100),
        RelativeCursorPosition {
            cursor_over: true,
            normalized: Some(Vec2::ZERO),
        },
    ));
}
fn at(app: &App, control: Entity) -> Vec2 {
    let id = app.world().get::<GObjectRef>(control).unwrap().0;
    let p = app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .object(id)
        .unwrap()
        .position();
    Vec2::new(p.x, p.y)
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
fn move_mouse(app: &mut App, window: Entity, pos: Vec2) {
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(pos));
    app.update();
}
#[test]
fn draggable_threshold_native_start_events_rounding_state_ticks_and_release_suppress_click() {
    let (mut app, window, _, control, _) = fixture();
    let origin = at(&app, control);
    mouse(&mut app, true);
    move_mouse(&mut app, window, Vec2::splat(12.));
    assert_eq!(at(&app, control), origin);
    assert!(
        !app.world()
            .get::<GuiObjectDragState>(control)
            .unwrap()
            .is_dragging()
    );
    move_mouse(&mut app, window, Vec2::splat(40.));
    assert_eq!(at(&app, control), origin);
    assert_eq!(
        app.world().resource::<Events>().0,
        [(EventKind::DragStart, None), (EventKind::DragMove, None)]
    );
    move_mouse(&mut app, window, Vec2::new(65.4, 74.6));
    assert_eq!(at(&app, control), origin + Vec2::new(25., 35.));
    let tick = app
        .world()
        .entity(control)
        .get_ref::<GuiObjectDragState>()
        .unwrap()
        .last_changed();
    let count = app.world().resource::<Events>().0.len();
    app.update();
    assert_eq!(
        app.world()
            .entity(control)
            .get_ref::<GuiObjectDragState>()
            .unwrap()
            .last_changed(),
        tick
    );
    assert_eq!(app.world().resource::<Events>().0.len(), count);
    mouse(&mut app, false);
    assert!(
        !app.world()
            .get::<GuiObjectDragState>(control)
            .unwrap()
            .is_dragging()
    );
    assert_eq!(
        app.world().resource::<Events>().0.last(),
        Some(&(EventKind::DragEnd, None))
    );
    assert!(
        !app.world()
            .resource::<Events>()
            .0
            .iter()
            .any(|(kind, _)| *kind == EventKind::Click)
    );
}

#[test]
fn draggable_control_bounds_and_live_stage_bounds_clamp_and_component_removal_cancels_once() {
    let (mut app, window, _, control, bounds) = fixture();
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .resolution
        .set(12000., 12000.);
    app.world_mut().entity_mut(control).insert(GuiDraggable {
        bounds: Some(GuiDragBounds::Control(bounds)),
    });
    mouse(&mut app, true);
    move_mouse(&mut app, window, Vec2::splat(40.));
    move_mouse(&mut app, window, Vec2::splat(9000.));
    let runtime = app.world().non_send::<GuiRuntime>();
    let id = app.world().get::<GObjectRef>(control).unwrap().0;
    let bound = app.world().get::<GObjectRef>(bounds).unwrap().0;
    let b = runtime.store().object(bound).unwrap();
    let o = runtime.store().object(id).unwrap();
    assert_eq!(o.x() + o.width(), b.x() + b.width());
    assert_eq!(o.y() + o.height(), b.y() + b.height());
    app.world_mut().entity_mut(control).insert(GuiDraggable {
        bounds: Some(GuiDragBounds::Stage(fairygui_core::geom::Rect::new(
            30., 40., 5., 5.,
        ))),
    });
    app.update();
    move_mouse(&mut app, window, Vec2::splat(9001.));
    assert_eq!(at(&app, control), Vec2::new(30., 40.));
    app.world_mut().entity_mut(control).remove::<GuiDraggable>();
    app.update();
    assert!(app.world().get::<GuiObjectDragState>(control).is_none());
    move_mouse(&mut app, window, Vec2::splat(9010.));
    mouse(&mut app, false);
    assert_eq!(at(&app, control), Vec2::new(30., 40.));
    assert_eq!(
        app.world()
            .resource::<Events>()
            .0
            .iter()
            .filter(|(kind, _)| *kind == EventKind::DragEnd)
            .count(),
        1
    );
}

#[test]
fn draggable_hidden_offstage_modal_and_reparented_sources_stop_without_resuming() {
    for policy in 0..5 {
        let (mut app, window, page, control, _) = fixture();
        mouse(&mut app, true);
        move_mouse(&mut app, window, Vec2::splat(40.));
        assert!(
            app.world()
                .get::<GuiObjectDragState>(control)
                .unwrap()
                .is_dragging()
        );
        match policy {
            0 => {
                if let Some(mut object) = app.world_mut().gui_mut(control) {
                    object.set_visible(false)
                }
            }
            1 => {
                app.world_mut()
                    .get_mut::<FairyGuiRoot>(page)
                    .unwrap()
                    .active = false;
            }
            2 => {
                app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
            }
            3 => {
                let handle = app.world().get::<FairyGuiRoot>(page).unwrap().asset.clone();
                app.world_mut()
                    .insert_resource(crate::GuiGlobalWaitSettings::new(handle, "WindowA"));
                app.world_mut()
                    .write_message(crate::GuiGlobalWaitCommand::Show { message: None });
            }
            _ => {
                let id = app.world().get::<GObjectRef>(control).unwrap().0;
                let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
                let parent = runtime
                    .store_mut()
                    .create(fairygui_core::package::ObjectType::Component);
                runtime.store_mut().set_parent(id, Some(parent));
            }
        }
        app.update();
        app.update();
        assert!(
            !app.world()
                .get::<GuiObjectDragState>(control)
                .unwrap()
                .is_dragging(),
            "policy={policy}"
        );
        assert_eq!(
            app.world()
                .resource::<Events>()
                .0
                .iter()
                .filter(|(kind, _)| *kind == EventKind::DragEnd)
                .count(),
            1
        );
    }
}

fn touch(
    app: &mut App,
    window: Entity,
    control: Entity,
    id: u64,
    phase: TouchPhase,
    position: Vec2,
) {
    app.world_mut().resource_mut::<HoverMap>().insert(
        PointerId::Touch(id),
        [(
            control,
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
fn draggable_touch_owner_excludes_second_finger_and_reports_cancel_end_once() {
    let (mut app, window, _, control, _) = fixture();
    app.init_resource::<HoverMap>();
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(None);
    touch(
        &mut app,
        window,
        control,
        7,
        TouchPhase::Started,
        Vec2::splat(10.),
    );
    touch(
        &mut app,
        window,
        control,
        7,
        TouchPhase::Moved,
        Vec2::splat(40.),
    );
    assert_eq!(
        app.world()
            .get::<GuiObjectDragState>(control)
            .unwrap()
            .touch_id(),
        Some(7)
    );
    let start = at(&app, control);
    touch(
        &mut app,
        window,
        control,
        8,
        TouchPhase::Started,
        Vec2::splat(50.),
    );
    touch(
        &mut app,
        window,
        control,
        8,
        TouchPhase::Moved,
        Vec2::splat(100.),
    );
    assert_eq!(at(&app, control), start);
    touch(
        &mut app,
        window,
        control,
        8,
        TouchPhase::Ended,
        Vec2::splat(100.),
    );
    assert!(
        app.world()
            .get::<GuiObjectDragState>(control)
            .unwrap()
            .is_dragging()
    );
    touch(
        &mut app,
        window,
        control,
        7,
        TouchPhase::Moved,
        Vec2::splat(70.),
    );
    assert_eq!(at(&app, control), start + Vec2::splat(30.));
    touch(
        &mut app,
        window,
        control,
        7,
        TouchPhase::Canceled,
        Vec2::splat(70.),
    );
    assert!(
        !app.world()
            .get::<GuiObjectDragState>(control)
            .unwrap()
            .is_dragging()
    );
    assert!(
        app.world()
            .resource::<Events>()
            .0
            .iter()
            .all(|(_, id)| *id == Some(7))
    );
    assert_eq!(
        app.world().resource::<Events>().0.last(),
        Some(&(EventKind::DragEnd, Some(7)))
    );
}

#[test]
fn draggable_distinct_controls_can_move_under_independent_fingers() {
    let (mut app, window, page, first, _) = fixture();
    let runtime = app.world().non_send::<GuiRuntime>();
    let root = app.world().get::<GuiRootInstance>(page).unwrap().object;
    let second = runtime.store().child_by_name(root, "d").unwrap();
    let second = runtime.entity_of(second).unwrap();
    app.world_mut()
        .entity_mut(second)
        .insert(GuiDraggable::default());
    app.init_resource::<HoverMap>();
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(None);
    touch(
        &mut app,
        window,
        first,
        7,
        TouchPhase::Started,
        Vec2::splat(10.),
    );
    touch(
        &mut app,
        window,
        second,
        8,
        TouchPhase::Started,
        Vec2::splat(100.),
    );
    touch(
        &mut app,
        window,
        first,
        7,
        TouchPhase::Moved,
        Vec2::splat(40.),
    );
    touch(
        &mut app,
        window,
        second,
        8,
        TouchPhase::Moved,
        Vec2::splat(130.),
    );
    let a = at(&app, first);
    let b = at(&app, second);
    touch(
        &mut app,
        window,
        first,
        7,
        TouchPhase::Moved,
        Vec2::new(80., 60.),
    );
    touch(
        &mut app,
        window,
        second,
        8,
        TouchPhase::Moved,
        Vec2::new(140., 160.),
    );
    assert_eq!(at(&app, first), a + Vec2::new(40., 20.));
    assert_eq!(at(&app, second), b + Vec2::new(10., 30.));
    touch(
        &mut app,
        window,
        first,
        7,
        TouchPhase::Ended,
        Vec2::new(80., 60.),
    );
    assert!(
        !app.world()
            .get::<GuiObjectDragState>(first)
            .unwrap()
            .is_dragging()
    );
    assert!(
        app.world()
            .get::<GuiObjectDragState>(second)
            .unwrap()
            .is_dragging()
    );
    touch(
        &mut app,
        window,
        second,
        8,
        TouchPhase::Ended,
        Vec2::new(140., 160.),
    );
    assert!(
        !app.world()
            .get::<GuiObjectDragState>(second)
            .unwrap()
            .is_dragging()
    );
}

#[test]
fn draggable_and_proxy_sources_reserve_their_press_before_ancestor_scrolling() {
    for proxy in [false, true] {
        let (mut app, window, _, _, _) = fixture();
        let package = FuiPackage::parse(
            "ListEffect",
            include_bytes!("../../fairygui-demo/assets/ui/ListEffect.fui"),
        )
        .unwrap();
        let handle = app
            .world_mut()
            .resource_mut::<Assets<FuiAsset>>()
            .add(FuiAsset {
                package,
                res_key: "ui/ListEffect".into(),
            });
        let owner = app
            .world_mut()
            .spawn(FairyGuiRoot::new(handle, "Main"))
            .id();
        app.update();
        let runtime = app.world().non_send::<GuiRuntime>();
        let root = app.world().get::<GuiRootInstance>(owner).unwrap().object;
        let list = runtime.store().child_by_name(root, "mailList").unwrap();
        let view = runtime.entity_of(list).unwrap();
        app.world_mut()
            .entity_mut(view)
            .insert(crate::GuiListContent::new(12, |index, row| {
                row.set_text(&format!("row {index}"));
                row.set_size(Vec2::new(200., 80.));
            }));
        app.update();
        let runtime = app.world().non_send::<GuiRuntime>();
        let row = runtime.store().list_item_at(list, 0).unwrap();
        let row = runtime.entity_of(row).unwrap();
        let pane = runtime.store().scroll_pane(list).unwrap();
        let before = pane.display_pos();
        if proxy {
            app.world_mut()
                .entity_mut(row)
                .insert(crate::GuiDragSource::default());
        } else {
            app.world_mut()
                .entity_mut(row)
                .insert(GuiDraggable::default());
        }
        hover(&mut app, row);
        mouse(&mut app, true);
        move_mouse(&mut app, window, Vec2::new(10., 40.));
        move_mouse(&mut app, window, Vec2::new(10., 70.));
        assert_eq!(
            app.world()
                .non_send::<GuiRuntime>()
                .store()
                .scroll_pane(list)
                .unwrap()
                .display_pos(),
            before,
            "proxy={proxy}"
        );
        if proxy {
            assert!(app.world().resource::<crate::GuiDragState>().is_dragging());
        } else {
            assert!(
                app.world()
                    .get::<GuiObjectDragState>(row)
                    .unwrap()
                    .is_dragging()
            );
        }
        mouse(&mut app, false);
    }
}

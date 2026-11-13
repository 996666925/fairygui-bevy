use super::*;
use crate::GuiWorldExt as _;
use crate::{FairyGuiPlugin, FairyGuiRoot, FuiAsset, GuiInputCommand, GuiQuery, GuiVirtualList};
use bevy::ecs::system::RunSystemOnce;
use bevy::ecs::system::SystemState;
use bevy::window::PrimaryWindow;

fn fixture() -> (App, Entity, Entity, Vec<Entity>) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<MouseWheel>()
    .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        std::time::Duration::from_millis(16),
    ));
    let package = fairygui_core::FuiPackage::parse(
        "ScrollPane",
        include_bytes!("../../../fairygui-demo/assets/ui/ScrollPane.fui"),
    )
    .unwrap();
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package,
            res_key: "ui/ScrollPane".into(),
        });
    let owner = app.world_mut().spawn(FairyGuiRoot::new(asset, "Main")).id();
    let mut window = Window {
        focused: true,
        ..default()
    };
    window.set_cursor_position(Some(Vec2::splat(200.)));
    let window = app.world_mut().spawn((window, PrimaryWindow)).id();
    app.update();
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    let list = query
        .get(app.world())
        .unwrap()
        .child_by_name(owner, "list")
        .unwrap();
    let width = app
        .world()
        .get::<crate::GuiScrollState>(list)
        .unwrap()
        .view_size()
        .x;
    app.world_mut()
        .entity_mut(list)
        .insert(GuiVirtualList::new(1000, move |index, row| {
            row.set_size(Vec2::new(width, row.size().y));
            row.set_text(&format!("Item {index}"));
            row.set_scroll_position(Vec2::ZERO);
        }));
    app.update();
    let rows = app
        .world()
        .get::<crate::GuiListState>(list)
        .unwrap()
        .items()
        .to_vec();
    assert!(rows.len() > 2);
    (app, window, list, rows)
}
fn hover(app: &mut App, target: Entity) {
    let mut query = app.world_mut().query_filtered::<Entity, With<Hovered>>();
    let entities: Vec<_> = query.iter(app.world()).collect();
    for entity in entities {
        app.world_mut()
            .entity_mut(entity)
            .insert(Hovered(entity == target));
    }
    app.world_mut().entity_mut(target).insert((
        ComputedStackIndex(999),
        RelativeCursorPosition {
            cursor_over: true,
            normalized: Some(Vec2::ZERO),
        },
    ));
}
fn press(app: &mut App, row: Entity) {
    hover(app, row);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
}
fn move_to(app: &mut App, window: Entity, position: Vec2) {
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(position));
    app.update();
}
fn offset(app: &App, entity: Entity) -> Vec2 {
    app.world()
        .get::<crate::GuiScrollState>(entity)
        .unwrap()
        .display_position()
}

#[test]
fn pending_nested_cancellation_does_not_restart_and_two_fingers_choose_independent_panes() {
    use bevy::input::touch::{TouchInput, TouchPhase};
    use bevy::picking::{backend::HitData, hover::HoverMap};
    let (mut app, window, list, rows) = fixture();
    press(&mut app, rows[0]);
    app.world_mut()
        .write_message(GuiInputCommand::CancelScrollDrag(rows[0]));
    app.world_mut()
        .write_message(GuiInputCommand::CancelScrollDrag(list));
    app.update();
    move_to(&mut app, window, Vec2::new(80., 80.));
    assert_eq!(offset(&app, rows[0]), Vec2::ZERO);
    assert_eq!(offset(&app, list), Vec2::ZERO);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(None);
    app.init_resource::<HoverMap>();
    for (id, row) in [(11, rows[0]), (22, rows[1]), (33, rows[2])] {
        app.world_mut().resource_mut::<HoverMap>().insert(
            PointerId::Touch(id),
            [(
                row,
                HitData::new(Entity::PLACEHOLDER, 0., Some(Vec3::ZERO), None),
            )]
            .into(),
        );
        app.world_mut().write_message(TouchInput {
            id,
            window,
            phase: TouchPhase::Started,
            position: Vec2::splat(200.),
            force: None,
        });
    }
    app.update();
    for (id, position) in [(11, Vec2::new(120., 198.)), (22, Vec2::new(198., 120.))] {
        app.world_mut().write_message(TouchInput {
            id,
            window,
            phase: TouchPhase::Moved,
            position,
            force: None,
        });
        app.update();
    }
    assert!((offset(&app, rows[0]).x - 80.).abs() < 0.01);
    assert!((offset(&app, list).y - 80.).abs() < 0.01);
    assert_eq!(offset(&app, rows[1]), Vec2::ZERO);
    // A third finger cannot steal the already owned outer pane.
    app.world_mut().write_message(TouchInput {
        id: 33,
        window,
        phase: TouchPhase::Moved,
        position: Vec2::new(198., 60.),
        force: None,
    });
    app.update();
    assert!((offset(&app, list).y - 80.).abs() < 0.01);
    app.world_mut()
        .write_message(GuiInputCommand::CancelScrollDrag(rows[0]));
    app.update();
    app.world_mut().write_message(TouchInput {
        id: 11,
        window,
        phase: TouchPhase::Moved,
        position: Vec2::new(40., 198.),
        force: None,
    });
    app.update();
    assert!((offset(&app, rows[0]).x - 80.).abs() < 0.01);
    assert!(
        app.world()
            .get::<crate::GuiScrollState>(list)
            .unwrap()
            .is_dragging()
    );
}

#[test]
fn nested_scroll_axes_choose_the_inner_horizontal_or_outer_vertical_and_lock_the_winner() {
    for (delta, horizontal) in [(Vec2::new(-80., -4.), true), (Vec2::new(-4., -80.), false)] {
        let (mut app, window, list, rows) = fixture();
        press(&mut app, rows[0]);
        move_to(&mut app, window, Vec2::splat(200.) + delta);
        if horizontal {
            assert_eq!(offset(&app, list), Vec2::ZERO);
            assert!((offset(&app, rows[0]).x - 80.).abs() < 0.01);
            assert!(
                app.world()
                    .get::<crate::GuiScrollState>(rows[0])
                    .unwrap()
                    .is_dragging()
            );
            move_to(
                &mut app,
                window,
                Vec2::splat(200.) + delta + Vec2::new(0., -90.),
            );
            assert_eq!(
                offset(&app, list),
                Vec2::ZERO,
                "axis ownership cannot switch mid-drag"
            );
        } else {
            assert!((offset(&app, list).y - 80.).abs() < 0.01);
            assert_eq!(offset(&app, rows[0]), Vec2::ZERO);
            assert!(
                app.world()
                    .get::<crate::GuiScrollState>(list)
                    .unwrap()
                    .is_dragging()
            );
        }
    }
}

#[test]
fn cancel_scroll_command_keeps_animation_and_prevents_capture_restart_or_release_inertia() {
    let (mut app, window, list, rows) = fixture();
    press(&mut app, rows[0]);
    move_to(&mut app, window, Vec2::new(120., 200.));
    assert!(offset(&app, rows[0]).x > 70.);
    app.world_mut()
        .write_message(GuiInputCommand::CancelScrollDrag(rows[0]));
    app.world_mut()
        .run_system_once(crate::input::apply_scroll_drag_commands)
        .unwrap();
    if let Some(mut object) = app.world_mut().gui_mut(rows[0]) {
        object.scroll_to(Vec2::ZERO, true)
    }
    app.update();
    assert!(
        !app.world()
            .get::<crate::GuiScrollState>(rows[0])
            .unwrap()
            .is_dragging()
    );
    assert!(
        app.world()
            .get::<crate::GuiScrollState>(rows[0])
            .unwrap()
            .is_animating()
    );
    move_to(&mut app, window, Vec2::new(20., 100.));
    for _ in 0..40 {
        app.update();
    }
    assert_eq!(offset(&app, rows[0]), Vec2::ZERO);
    assert_eq!(offset(&app, list), Vec2::ZERO);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    app.update();
    assert!(
        !app.world()
            .get::<crate::GuiScrollState>(rows[0])
            .unwrap()
            .is_animating()
    );
    assert!(
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .drain_pull_releases()
            .is_empty()
    );
}

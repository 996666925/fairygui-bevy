//! Joystick through real picking, mouse capture and OS-style touch messages.
use super::*;
use crate::joystick_demo::Joystick;
use bevy::input::touch::{TouchInput, TouchPhase};

#[derive(Resource, Default)]
struct State {
    step: u8,
    frame: u32,
    since: u32,
    page: Option<Entity>,
    views: Vec<Entity>,
    cursor: Option<Vec2>,
    start: Vec2,
    last_center: Vec2,
    home: Vec2,
    viewport: Vec2,
    touches: Vec<TouchInput>,
}
#[derive(Resource, Default)]
struct Completions(usize);
fn record(event: On<GuiTweenComplete>, mut completed: ResMut<Completions>) {
    if event.property == GuiTweenProperty::Xy {
        completed.0 += 1;
    }
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<State>()
        .init_resource::<Completions>()
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(16),
        ))
        .add_systems(First, send_touches.before(PickingSystems::Input))
        .add_systems(
            PreUpdate,
            drive
                .after(InputSystems)
                .before(UiSystems::Focus)
                .before(PickingSystems::ProcessInput),
        );
}
fn send_touches(
    mut state: ResMut<State>,
    mut windows: MessageWriter<bevy::window::WindowEvent>,
    mut touches: MessageWriter<TouchInput>,
) {
    for touch in std::mem::take(&mut state.touches) {
        windows.write(bevy::window::WindowEvent::TouchInput(touch));
        touches.write(touch);
    }
}
fn touch(state: &mut State, window: Entity, id: u64, phase: TouchPhase, position: Vec2) {
    state.touches.push(TouchInput {
        window,
        id,
        phase,
        position,
        force: None,
    });
}
fn center(gui: &GuiQuery, entity: Entity) -> Vec2 {
    let t = gui.transform(entity).unwrap();
    t.position() + t.size() / 2.
}
fn near(actual: Vec2, expected: Vec2) {
    assert!(
        actual.distance(expected) < 0.05,
        "actual {actual:?}, expected {expected:?}"
    );
}

#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    demo: Res<Demo>,
    gui: GuiQuery,
    joysticks: Query<&Joystick>,
    selected: Query<&GuiButtonSelected>,
    completed: Res<Completions>,
    existing: Query<()>,
    assets: Res<AssetServer>,
    mut windows: Query<(Entity, &mut Window)>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
    time: Res<Time<bevy::time::Real>>,
) {
    state.frame += 1;
    assert!(
        time.elapsed_secs_f64() < 45.,
        "joystick smoke timeout at step {}",
        state.step
    );
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    let elapsed = state.frame - state.since;
    let joystick = state.page.and_then(|page| joysticks.get(page).ok());
    let mut next = false;
    match state.step {
        0 => {
            let Some(menu) = demo.menu_owner.filter(|&owner| gui.view(owner).is_some()) else {
                return;
            };
            commands.write_message(GuiObjectCommand::new(
                menu,
                GuiObjectAction::SetVisible(false),
            ));
            state.page = Some(
                commands
                    .spawn(FairyGuiRoot::new(assets.load("ui/Joystick.fui"), "Main"))
                    .id(),
            );
            next = true;
        }
        1 if elapsed >= 15 => {
            let Some(j) = joystick else { return };
            state.views = vec![
                gui.view(state.page.unwrap()).unwrap(),
                j.area,
                j.button,
                j.center,
                j.thumb,
                j.output,
            ];
            state.home = j.home;
            state.viewport = Vec2::new(window.width(), window.height());
            assert!(!selected.get(j.button).unwrap().0);
            near(
                gui.transform(state.page.unwrap()).unwrap().size(),
                Vec2::new(window.width(), window.height()),
            );
            state.start = gui.transform(j.area).unwrap().position() + Vec2::new(140., 100.);
            state.cursor = Some(state.start);
            commands.entity(j.button).observe(record);
            next = true;
        }
        2 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        3 if elapsed >= 3 => {
            let j = joystick.unwrap();
            assert!(selected.get(j.button).unwrap().0);
            near(center(&gui, j.center), state.start);
            near(center(&gui, j.button), state.start);
            state.cursor = Some(state.start + Vec2::new(400., -80.));
            next = true;
        }
        4 if elapsed >= 3 => {
            let j = joystick.unwrap();
            state.last_center = state.start + Vec2::new(400., -80.).normalize() * 150.;
            near(center(&gui, j.button), state.last_center);
            let degrees: f32 = gui.text(j.output).unwrap().parse().unwrap();
            assert!((degrees - (-80_f32).atan2(400.).to_degrees()).abs() < 0.01);
            assert!((gui.transform(j.thumb).unwrap().rotation() - degrees - 90.).abs() < 0.01);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-ecs-joystick.png"));
            // Reverse after reaching the radius: movement is incremental.
            state.cursor = state.cursor.map(|point| point - Vec2::new(10., 0.));
            next = true;
        }
        5 if elapsed >= 8 => {
            let j = joystick.unwrap();
            state.last_center -= Vec2::new(10., 0.);
            near(center(&gui, j.button), state.last_center);
            mouse.release(MouseButton::Left);
            next = true;
        }
        6 if elapsed >= 3 => {
            let j = joystick.unwrap();
            assert!(j.returning);
            assert!(!gui.display(j.center).unwrap().visible());
            assert_eq!(gui.text(j.output), Some(""));
            let point = center(&gui, j.button);
            assert!(point.distance(state.home) > 1. && point.distance(state.last_center) > 1.);
            state.cursor = Some(state.start + Vec2::new(20., -20.));
            mouse.press(MouseButton::Left); // Interrupt the return before completion.
            next = true;
        }
        7 if elapsed >= 24 => {
            let j = joystick.unwrap();
            assert!(!j.returning);
            assert!(selected.get(j.button).unwrap().0);
            near(center(&gui, j.button), state.cursor.unwrap());
            assert_eq!(completed.0, 0, "interrupted return must not notify");
            mouse.release(MouseButton::Left);
            next = true;
        }
        8 if elapsed >= 24 => {
            let j = joystick.unwrap();
            assert!(!j.returning);
            assert!(!selected.get(j.button).unwrap().0);
            assert_eq!(completed.0, 1);
            near(center(&gui, j.button), state.home);
            near(center(&gui, j.center), state.home);
            assert_eq!(gui.transform(j.thumb).unwrap().rotation(), 0.);
            state.cursor = None;
            let point = state.start;
            touch(&mut state, window_entity, 7, TouchPhase::Started, point);
            next = true;
        }
        9 if elapsed >= 4 => {
            let j = joystick.unwrap();
            assert!(selected.get(j.button).unwrap().0);
            near(center(&gui, j.button), state.start);
            let point = state.start + Vec2::new(60., -40.);
            touch(&mut state, window_entity, 8, TouchPhase::Started, point);
            next = true;
        }
        10 if elapsed >= 4 => {
            near(center(&gui, joystick.unwrap().button), state.start);
            let point = state.start + Vec2::new(300., -100.);
            touch(&mut state, window_entity, 8, TouchPhase::Moved, point);
            next = true;
        }
        11 if elapsed >= 4 => {
            near(center(&gui, joystick.unwrap().button), state.start);
            let point = state.start + Vec2::new(300., -100.);
            touch(&mut state, window_entity, 8, TouchPhase::Ended, point);
            let point = state.start + Vec2::new(400., -80.);
            touch(&mut state, window_entity, 7, TouchPhase::Moved, point);
            next = true;
        }
        12 if elapsed >= 4 => {
            let j = joystick.unwrap();
            assert!(
                !j.returning,
                "another finger's release must not release the owner"
            );
            near(
                center(&gui, j.button),
                state.start + Vec2::new(400., -80.).normalize() * 150.,
            );
            let point = state.start + Vec2::new(400., -80.);
            touch(&mut state, window_entity, 7, TouchPhase::Canceled, point);
            next = true;
        }
        13 if elapsed >= 24 => {
            let j = joystick.unwrap();
            near(center(&gui, j.button), state.home);
            assert!(!selected.get(j.button).unwrap().0);
            assert_eq!(completed.0, 2);
            state.cursor = Some(state.start);
            next = true;
        }
        14 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        15 if elapsed >= 4 => {
            state.cursor = Some(state.start + Vec2::new(40., -30.));
            next = true;
        }
        16 if elapsed >= 4 => {
            let j = joystick.unwrap();
            near(center(&gui, j.button), state.start + Vec2::new(40., -30.));
            window.resolution.set_physical_resolution(1440, 900);
            next = true;
        }
        17 if elapsed >= 12 => {
            let j = joystick.unwrap();
            assert!(!j.returning);
            assert!(!selected.get(j.button).unwrap().0);
            near(
                gui.transform(state.page.unwrap()).unwrap().size(),
                Vec2::new(window.width(), window.height()),
            );
            near(center(&gui, j.button), j.home);
            near(
                j.home,
                state.home + Vec2::new(0., window.height() - state.viewport.y),
            );
            assert!((j.home.y - state.home.y).abs() > 1.);
            assert_eq!(gui.text(j.output), Some(""));
            mouse.release(MouseButton::Left);
            commands.entity(state.page.unwrap()).despawn();
            next = true;
        }
        18 if elapsed >= 8 => {
            assert!(state.views.iter().all(|&view| !existing.contains(view)));
            assert_eq!(completed.0, 2);
            println!(
                "demo smoke PASSED: ECS Joystick mouse capture outside area, radius clamp, incremental reversal, interrupted return, natural completion, first-finger ownership, touch cancellation, resize and owned disposal"
            );
            exit.write(AppExit::Success);
            next = true;
        }
        _ => {}
    }
    if next {
        state.step += 1;
        state.since = state.frame;
    }
    window.set_cursor_position(state.cursor);
    move_pointer(window_entity, state.cursor, &mut pointers);
}

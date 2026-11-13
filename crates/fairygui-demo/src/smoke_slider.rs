//! Exported sliders through ECS commands and real Bevy picking/layout.
use super::*;

#[derive(Resource, Default)]
struct State {
    step: u8,
    frame: u32,
    since: u32,
    owners: Vec<Entity>,
    cursor: Option<Vec2>,
}
#[derive(Resource, Default)]
struct Events(Vec<(Entity, f64)>);
fn changed(event: On<GuiEvent>, gui: GuiQuery, mut events: ResMut<Events>) {
    if event.event.kind == fairygui_core::object::EventKind::StateChanged && !event.event.bubbling {
        events
            .0
            .push((event.entity, gui.slider(event.entity).unwrap().value()));
    }
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<State>()
        .init_resource::<Events>()
        .add_systems(
            PreUpdate,
            drive
                .after(InputSystems)
                .before(UiSystems::Focus)
                .before(PickingSystems::ProcessInput),
        );
}
fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 0.001, "{actual} != {expected}");
}
fn verify(gui: &GuiQuery, owner: Entity, vertical: bool, value: f64, length: f32) {
    near(gui.slider(owner).unwrap().value(), value);
    let bar = gui
        .child_by_name(owner, if vertical { "bar_v" } else { "bar" })
        .unwrap();
    let geometry = gui.transform(bar).unwrap();
    near(
        f64::from(if vertical {
            geometry.size().y
        } else {
            geometry.size().x
        }),
        f64::from(length),
    );
    near(
        f64::from(if vertical {
            geometry.position().y
        } else {
            geometry.position().x
        }),
        if vertical { 7. } else { 5. },
    );
}

#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    demo: Res<Demo>,
    gui: GuiQuery,
    events: Res<Events>,
    assets: Res<AssetServer>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    existing: Query<()>,
    mut windows: Query<(Entity, &mut Window), With<bevy::window::PrimaryWindow>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    assert!(
        state.frame < 1800,
        "slider smoke timed out at step {}",
        state.step
    );
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    let center =
        |entity| nodes.get(entity).unwrap().1.transform_point2(Vec2::ZERO) / window.scale_factor();
    let grip_center = |owner| center(gui.child_by_name(owner, "grip").unwrap());
    let elapsed = state.frame - state.since;
    let mut next = false;
    match state.step {
        0 if state.frame >= 30 => {
            let Some(menu) = demo.menu_owner.filter(|&owner| gui.view(owner).is_some()) else {
                return;
            };
            commands.write_message(GuiObjectCommand::new(
                menu,
                GuiObjectAction::SetVisible(false),
            ));
            for (name, position, size, scale) in [
                (
                    "Slider_HZ",
                    Vec2::new(100., 100.),
                    Vec2::new(300., 30.),
                    1.5,
                ),
                (
                    "Slider_VT",
                    Vec2::new(650., 100.),
                    Vec2::new(30., 300.),
                    1.25,
                ),
            ] {
                let owner = commands
                    .spawn(
                        FairyGuiRoot::new(assets.load("ui/Basics.fui"), name)
                            .with_position(position)
                            .with_size(size),
                    )
                    .id();
                for action in [
                    GuiObjectAction::SetScale(Vec2::splat(scale)),
                    GuiObjectAction::SetSliderRange { min: 0., max: 1. },
                    GuiObjectAction::SetSliderValue(0.25),
                ] {
                    commands.write_message(GuiObjectCommand::new(owner, action));
                }
                state.owners.push(owner);
            }
            next = true;
        }
        1 if elapsed >= 16
            && state
                .owners
                .iter()
                .all(|&owner| gui.slider(owner).is_some()) =>
        {
            for &owner in &state.owners {
                commands.entity(gui.view(owner).unwrap()).observe(changed);
            }
            verify(&gui, state.owners[0], false, 0.25, 72.);
            verify(&gui, state.owners[1], true, 0.25, 72.);
            state.cursor = Some(grip_center(state.owners[0]));
            next = true;
        }
        2 | 7 | 16 if elapsed >= 3 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        3 if elapsed >= 3 => {
            verify(&gui, state.owners[0], false, 0.25, 72.);
            assert!(events.0.is_empty(), "grip-down must not jump");
            state.cursor.as_mut().unwrap().x += 54.;
            next = true;
        }
        4 if elapsed >= 4 => {
            verify(&gui, state.owners[0], false, 0.375, 108.);
            assert_eq!(events.0.len(), 1);
            near(events.0[0].1, 0.375);
            mouse.release(MouseButton::Left);
            for action in [
                GuiObjectAction::SetSliderRange { min: 0., max: 10. },
                GuiObjectAction::SetSliderWholeNumbers(true),
                GuiObjectAction::SetSliderValue(2.5),
            ] {
                commands.write_message(GuiObjectCommand::new(state.owners[0], action));
            }
            next = true;
        }
        5 if elapsed >= 4 => {
            verify(&gui, state.owners[0], false, 2.5, 72.);
            assert_eq!(events.0.len(), 1, "setters must not emit input changes");
            state.cursor = Some(grip_center(state.owners[0]));
            next = true;
        }
        6 if elapsed >= 3 => {
            next = true;
        }
        8 if elapsed >= 3 => {
            verify(&gui, state.owners[0], false, 2.5, 72.);
            state.cursor.as_mut().unwrap().x += 54.;
            next = true;
        }
        9 if elapsed >= 4 => {
            verify(&gui, state.owners[0], false, 4., 115.);
            assert_eq!(events.0.len(), 2);
            commands.write_message(GuiObjectCommand::new(
                state.owners[0],
                GuiObjectAction::SetSliderCanDrag(false),
            ));
            next = true;
        }
        10 if elapsed >= 4 => {
            assert!(!gui.slider(state.owners[0]).unwrap().can_drag());
            state.cursor.as_mut().unwrap().x += 54.;
            next = true;
        }
        11 if elapsed >= 4 => {
            verify(&gui, state.owners[0], false, 4., 115.);
            assert_eq!(events.0.len(), 2);
            mouse.release(MouseButton::Left);
            commands.write_message(GuiObjectCommand::new(
                state.owners[0],
                GuiObjectAction::SetSliderChangeOnClick(false),
            ));
            next = true;
        }
        12 if elapsed >= 4 => {
            state.cursor = gui.local_to_global(state.owners[0], Vec2::new(250., 12.));
            next = true;
        }
        13 if elapsed >= 3 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        14 if elapsed >= 4 => {
            verify(&gui, state.owners[0], false, 4., 115.);
            assert_eq!(events.0.len(), 2);
            mouse.release(MouseButton::Left);
            state.cursor = Some(grip_center(state.owners[1]));
            next = true;
        }
        15 if elapsed >= 3 => {
            next = true;
        }
        17 if elapsed >= 3 => {
            verify(&gui, state.owners[1], true, 0.25, 72.);
            state.cursor.as_mut().unwrap().y += 45.;
            next = true;
        }
        18 if elapsed >= 4 => {
            verify(&gui, state.owners[1], true, 0.375, 108.);
            assert_eq!(events.0.len(), 3);
            near(events.0[2].1, 0.375);
            mouse.release(MouseButton::Left);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-slider-ecs.png"));
            next = true;
        }
        19 if elapsed >= 8 => {
            for &owner in &state.owners {
                commands.entity(owner).despawn();
            }
            next = true;
        }
        20 if elapsed >= 5 => {
            assert!(state.owners.iter().all(|&owner| !existing.contains(owner)));
            println!(
                "demo smoke PASSED: ECS slider ranges, scaled horizontal/vertical grip drags, no press jump, fractional setters, input rounding, drag/click switches, current observer snapshots and disposal"
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

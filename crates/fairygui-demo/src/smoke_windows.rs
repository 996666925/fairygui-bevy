//! Actual window close picking, retained reopen, drag, modality and disposal.
use super::*;

#[derive(Resource, Default)]
struct State {
    step: u8,
    frame: u32,
    since: u32,
    owners: Vec<Entity>,
    views: Vec<Entity>,
    cursor: Option<Vec2>,
    initial: Vec2,
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<State>().add_systems(
        PreUpdate,
        drive
            .after(InputSystems)
            .before(UiSystems::Focus)
            .before(PickingSystems::ProcessInput),
    );
}
fn point(
    entity: Entity,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    factor: f32,
    header: bool,
) -> Vec2 {
    let (node, transform) = nodes.get(entity).unwrap();
    let at = if header {
        node.content_box().min + Vec2::new(40., 16.) / node.inverse_scale_factor()
    } else {
        Vec2::ZERO
    };
    transform.transform_point2(at) / factor
}
fn command(commands: &mut Commands, target: Entity, action: GuiWindowAction) {
    commands.write_message(GuiWindowCommand::new(target, action));
}
#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    demo: Res<Demo>,
    gui: GuiQuery,
    states: Query<&GuiWindowState>,
    sources: Query<&FairyGuiWindow>,
    existing: Query<()>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
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
        "ECS window smoke timeout at step {}",
        state.step
    );
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    let elapsed = state.frame - state.since;
    let factor = window.scale_factor();
    let first = state.owners.first().copied();
    let second = state.owners.get(1).copied();
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
            let asset = assets.load("ui/Basics.fui");
            for (item, modal) in [("WindowA", false), ("WindowB", true)] {
                let mut config = FairyGuiWindow::new(asset.clone(), item);
                config.visible = false;
                config.modal = modal;
                state.owners.push(commands.spawn(config).id());
            }
            next = true;
        }
        1 if elapsed >= 20
            && first.and_then(|owner| states.get(owner).ok()).is_some()
            && second.and_then(|owner| states.get(owner).ok()).is_some() =>
        {
            state.views = state
                .owners
                .iter()
                .map(|&owner| states.get(owner).unwrap().view())
                .collect();
            assert!(
                state
                    .owners
                    .iter()
                    .all(|&owner| !states.get(owner).unwrap().is_showing())
            );
            let first = first.unwrap();
            command(&mut commands, first, GuiWindowAction::Show);
            commands.write_message(GuiObjectCommand::new(
                first,
                GuiObjectAction::SetPosition(Vec2::new(200., 130.)),
            ));
            next = true;
        }
        2 if elapsed >= 15 => {
            let first = first.unwrap();
            let state1 = states.get(first).unwrap();
            assert!(state1.is_showing());
            assert_eq!(state1.view(), state.views[0]);
            state.cursor = Some(point(state1.close_button().unwrap(), &nodes, factor, false));
            next = true;
        }
        3 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        4 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        5 if elapsed >= 8 => {
            let first = first.unwrap();
            assert!(!states.get(first).unwrap().is_showing());
            assert!(!sources.get(first).unwrap().visible);
            assert_eq!(states.get(first).unwrap().view(), state.views[0]);
            assert!(existing.contains(state.views[0]));
            command(&mut commands, first, GuiWindowAction::Show);
            next = true;
        }
        6 if elapsed >= 12 => {
            let first = first.unwrap();
            let state1 = states.get(first).unwrap();
            assert!(state1.is_showing());
            assert_eq!(state1.view(), state.views[0]);
            state.initial = gui.transform(first).unwrap().position();
            state.cursor = Some(point(state1.drag_area().unwrap(), &nodes, factor, true));
            next = true;
        }
        7 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        8 if elapsed >= 4 => {
            *state.cursor.as_mut().unwrap() += Vec2::new(110., 50.);
            next = true;
        }
        9 if elapsed >= 8 => {
            let at = gui.transform(first.unwrap()).unwrap().position();
            assert!(
                (at - state.initial).length() > 60.,
                "window did not drag: {at:?}"
            );
            assert!(states.get(first.unwrap()).unwrap().is_dragging());
            mouse.release(MouseButton::Left);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-ecs-window-drag.png"));
            next = true;
        }
        10 if elapsed >= 8 => {
            assert!(!states.get(first.unwrap()).unwrap().is_dragging());
            command(&mut commands, second.unwrap(), GuiWindowAction::Show);
            next = true;
        }
        11 if elapsed >= 12 => {
            let state2 = states.get(second.unwrap()).unwrap();
            assert!(state2.is_showing() && state2.is_modal());
            window.resolution.set(1100., 650.);
            next = true;
        }
        12 if elapsed >= 8 => {
            command(&mut commands, second.unwrap(), GuiWindowAction::Center);
            next = true;
        }
        13 if elapsed >= 8 => {
            let transform = gui.transform(second.unwrap()).unwrap();
            assert!(
                (transform.position()
                    - (Vec2::new(window.width(), window.height()) - transform.size()) / 2.)
                    .length()
                    < 0.01
            );
            command(&mut commands, second.unwrap(), GuiWindowAction::Hide);
            next = true;
        }
        14 if elapsed >= 8 => {
            assert!(!states.get(second.unwrap()).unwrap().is_showing());
            assert!(states.get(first.unwrap()).unwrap().is_showing());
            for &owner in &state.owners {
                commands.entity(owner).despawn();
            }
            next = true;
        }
        15 if elapsed >= 8 => {
            assert!(state.views.iter().all(|&view| !existing.contains(view)));
            println!(
                "demo smoke PASSED: ECS window ownership, actual close button, retained reopen, actual drag, modality, recenter after resize and disposal"
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

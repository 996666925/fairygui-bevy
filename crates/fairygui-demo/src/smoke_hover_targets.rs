//! Actual picking validates direct hover events and unchanged common ancestors.
use super::*;
use bevy::input::touch::{TouchInput, TouchPhase};

#[derive(Resource, Default)]
struct Seen(Vec<(bool, Entity, Option<u64>)>);
fn enter(mut event: On<GuiRollOver>, mut seen: ResMut<Seen>) {
    assert_eq!(event.target, Some(event.entity));
    assert!(!event.event.bubbling);
    seen.0.push((true, event.entity, event.event.touch_id));
    event.propagate(false);
}
fn leave(mut event: On<GuiRollOut>, mut seen: ResMut<Seen>) {
    assert_eq!(event.target, Some(event.entity));
    assert!(!event.event.bubbling);
    seen.0.push((false, event.entity, event.event.touch_id));
    event.propagate(false);
}
#[derive(Resource, Default)]
struct State {
    frame: u32,
    since: u32,
    step: u8,
    sample: u8,
    owner: Option<Entity>,
    root: Option<Entity>,
    buttons: Option<[Entity; 2]>,
    cursor: Option<Vec2>,
    touch_cursor: Option<Vec2>,
}
pub(super) fn install(app: &mut App) {
    app.world_mut().spawn(PointerId::Touch(91));
    app.init_resource::<State>()
        .init_resource::<Seen>()
        .add_systems(
            PreUpdate,
            drive
                .after(InputSystems)
                .before(UiSystems::Focus)
                .before(PickingSystems::ProcessInput),
        );
}
#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    mut seen: ResMut<Seen>,
    demo: Res<Demo>,
    server: Res<AssetServer>,
    gui: GuiQuery,
    mut edit: GuiMut,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    mut windows: Query<(Entity, &mut Window)>,
    mut pointers: MessageWriter<PointerInput>,
    mut touches: MessageWriter<TouchInput>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    assert!(
        state.frame < 600,
        "hover smoke timed out at {}/{}",
        state.sample,
        state.step
    );
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    let elapsed = state.frame - state.since;
    let mut next = false;
    match state.step {
        0 if state.frame > 25 => {
            let Some(menu) = demo.menu_owner.filter(|&owner| gui.view(owner).is_some()) else {
                return;
            };
            edit.get_mut(menu).unwrap().set_visible(false);
            state.cursor = Some(Vec2::ZERO);
            state.owner = Some(
                commands
                    .spawn(
                        FairyGuiRoot::new(server.load("ui/Basics.fui"), "Demo_Button")
                            .with_position(Vec2::new(60., 60.))
                            .with_size(Vec2::new(700., 400.)),
                    )
                    .id(),
            );
            next = true;
        }
        1 if elapsed > 10 && gui.view(state.owner.unwrap()).is_some() => {
            let owner = state.owner.unwrap();
            let root = gui.view(owner).unwrap();
            let buttons = ["n13", "n4"].map(|name| gui.child_by_name(owner, name).unwrap());
            for &child in gui.get(owner).unwrap().children() {
                edit.get_mut(child)
                    .unwrap()
                    .set_visible(buttons.contains(&child));
            }
            for (i, &button) in buttons.iter().enumerate() {
                edit.get_mut(button)
                    .unwrap()
                    .set_position(Vec2::new(50. + i as f32 * 310., 80.));
            }
            for entity in [buttons[0], buttons[1], root] {
                commands.entity(entity).observe(enter).observe(leave);
            }
            commands.queue(move |world: &mut World| {
                let id = world.get::<GObjectRef>(root).unwrap().0;
                world
                    .non_send_mut::<GuiRuntime>()
                    .store_mut()
                    .set_component_opaque(id, true);
            });
            state.root = Some(root);
            state.buttons = Some(buttons);
            next = true;
        }
        2 if elapsed > 10 => {
            let buttons = state.buttons.unwrap();
            let center = |button| {
                nodes.get(button).unwrap().1.transform_point2(Vec2::ZERO) / window.scale_factor()
            };
            seen.0.clear();
            match state.sample {
                0 => state.cursor = Some(center(buttons[0])),
                1 => state.cursor = Some(center(buttons[1])),
                2 => {
                    let (node, transform) = nodes.get(state.root.unwrap()).unwrap();
                    state.cursor = Some(
                        transform.transform_point2(
                            node.content_box().min + Vec2::new(300., 300.) * window.scale_factor(),
                        ) / window.scale_factor(),
                    );
                }
                3 => state.cursor = Some(Vec2::ZERO),
                4..=6 => {
                    let position = center(buttons[usize::from(state.sample != 4)]);
                    state.touch_cursor = Some(position);
                    touches.write(TouchInput {
                        window: window_entity,
                        id: 91,
                        phase: match state.sample {
                            4 => TouchPhase::Started,
                            5 => TouchPhase::Moved,
                            _ => TouchPhase::Ended,
                        },
                        position,
                        force: None,
                    });
                }
                _ => unreachable!(),
            }
            next = true;
        }
        3 if elapsed > 7 => {
            let [first, second] = state.buttons.unwrap();
            let root = state.root.unwrap();
            let touch = (state.sample >= 4).then_some(91);
            let expected: Vec<_> = match state.sample {
                0 | 4 => vec![(true, first, touch), (true, root, touch)],
                1 | 5 => vec![(false, first, touch), (true, second, touch)],
                2 => vec![(false, second, None)],
                3 => vec![(false, root, None)],
                6 => vec![(false, second, touch), (false, root, touch)],
                _ => unreachable!(),
            };
            assert_eq!(seen.0, expected, "sample {}", state.sample);
            let hovered = [matches!(state.sample, 0 | 4), matches!(state.sample, 1 | 5)];
            commands.queue(move |world: &mut World| {
                for (i, entity) in [first, second].into_iter().enumerate() {
                    let id = world.get::<GObjectRef>(entity).unwrap().0;
                    let runtime = world.non_send::<GuiRuntime>();
                    let fairygui_core::widget::ControlData::Button(button) =
                        runtime.store().object(id).unwrap().control()
                    else {
                        panic!("button");
                    };
                    assert_eq!(
                        button.hovered, hovered[i],
                        "button visual hover must match live pointers"
                    );
                }
            });
            println!(
                "hover recipient smoke sample {} PASSED: {:?}",
                state.sample, seen.0
            );
            if state.sample == 5 {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk("target/fairygui-demo-hover-targets.png"));
            }
            if state.sample == 6 {
                next = true;
            } else {
                state.sample += 1;
                state.step = 2;
                state.since = state.frame;
            }
        }
        4 if elapsed > 10 => {
            commands.entity(state.owner.unwrap()).despawn();
            next = true;
        }
        5 if elapsed > 8 => {
            assert!(gui.view(state.owner.unwrap()).is_none());
            println!(
                "demo smoke PASSED: direct mouse/touch hover targets, sibling/common-ancestor deduplication, independent stops and button hover visuals"
            );
            exit.write(AppExit::Success);
        }
        _ => {}
    }
    if next {
        state.step += 1;
        state.since = state.frame;
    }
    window.set_cursor_position(state.cursor);
    move_pointer(window_entity, state.cursor, &mut pointers);
    if state.step <= 3
        && let Some(position) = state.touch_cursor
    {
        let target =
            bevy::camera::RenderTarget::Window(bevy::window::WindowRef::Entity(window_entity))
                .normalize(Some(window_entity))
                .unwrap();
        pointers.write(PointerInput::new(
            PointerId::Touch(91),
            Location { target, position },
            PointerAction::Move { delta: Vec2::ZERO },
        ));
    }
}

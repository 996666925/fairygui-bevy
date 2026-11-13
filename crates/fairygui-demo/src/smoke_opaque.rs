//! Real blank-container pass-through, child bubbling and opaque/alpha policies.
use super::*;

#[derive(Resource, Default)]
struct State {
    frame: u32,
    since: u32,
    step: u8,
    phase: u8,
    owners: Vec<Entity>,
    front: Option<Entity>,
    back: Option<Entity>,
    child: Option<Entity>,
    cursor: Option<Vec2>,
    count: usize,
}
#[derive(Resource, Default)]
struct Clicks(Vec<(Entity, bool)>);
fn clicked(event: On<GuiEvent>, mut clicks: ResMut<Clicks>) {
    if event.event.kind == fairygui_core::object::EventKind::Click {
        clicks.0.push((event.entity, event.event.bubbling));
    }
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<State>()
        .init_resource::<Clicks>()
        .add_systems(
            PreUpdate,
            drive
                .after(InputSystems)
                .before(UiSystems::Focus)
                .before(PickingSystems::ProcessInput),
        );
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn drive(
    mut state: ResMut<State>,
    clicks: Res<Clicks>,
    demo: Res<Demo>,
    assets: Res<AssetServer>,
    mut ui: ParamSet<(
        GuiQuery,
        Query<&mut GuiOpaque>,
        Query<&mut GuiTransform>,
        Query<&mut GuiDisplay>,
        Query<&mut GuiGraph>,
        Query<&mut GuiSortingOrder>,
    )>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    entities: Query<()>,
    input: Res<GuiInputState>,
    mut windows: Query<(Entity, &mut Window), With<bevy::window::PrimaryWindow>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    assert!(
        state.frame < 1800,
        "opaque smoke timed out at step {} phase {}",
        state.step,
        state.phase
    );
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    let elapsed = state.frame - state.since;
    if state.step == 0 && state.frame >= 30 {
        let Some(menu) = demo.menu_owner.and_then(|owner| ui.p0().view(owner)) else {
            return;
        };
        ui.p3().get_mut(menu).unwrap().visible = false;
        let asset = assets.load("ui/Basics.fui");
        for _ in 0..2 {
            state.owners.push(
                commands
                    .spawn(
                        FairyGuiRoot::new(asset.clone(), "Demo_Graph")
                            .with_position(Vec2::new(300., 180.))
                            .with_size(Vec2::new(300., 200.)),
                    )
                    .id(),
            );
        }
        state.step = 1;
        state.since = state.frame;
    } else if state.step == 1
        && elapsed >= 16
        && state
            .owners
            .iter()
            .all(|&owner| ui.p0().view(owner).is_some())
    {
        let back = ui.p0().child_by_name(state.owners[0], "n12").unwrap();
        let child = ui.p0().child_by_name(state.owners[1], "n10").unwrap();
        let front = ui.p0().view(state.owners[1]).unwrap();
        for (index, &owner) in state.owners.iter().enumerate() {
            let children = ui.p0().get(owner).unwrap().children().to_vec();
            for entity in children {
                ui.p3().get_mut(entity).unwrap().visible = index == 0 && entity == back;
            }
            let view = ui.p0().view(owner).unwrap();
            ui.p1().get_mut(view).unwrap().0 = false;
        }
        for (entity, position, size, color) in [
            (
                back,
                Vec2::ZERO,
                Vec2::new(300., 200.),
                Color::srgb(0.2, 0.5, 0.8),
            ),
            (
                child,
                Vec2::new(20., 20.),
                Vec2::new(60., 60.),
                Color::srgb(0.9, 0.3, 0.2),
            ),
        ] {
            {
                let mut q = ui.p2();
                let mut t = q.get_mut(entity).unwrap();
                t.position = position;
                t.size = size;
            }
            *ui.p4().get_mut(entity).unwrap() = GuiGraph {
                kind: GraphType::Rect,
                fill_color: color,
                ..default()
            };
            commands.entity(entity).observe(clicked);
        }
        ui.p5().get_mut(front).unwrap().0 = 100;
        commands.entity(front).observe(clicked);
        state.front = Some(front);
        state.back = Some(back);
        state.child = Some(child);
        state.step = 2;
        state.phase = 0;
        state.since = state.frame;
    } else if (2..=10).contains(&state.step) {
        let front = state.front.unwrap();
        let back = state.back.unwrap();
        let child = state.child.unwrap();
        match state.phase {
            0 => {
                let opaque = matches!(state.step, 3 | 5 | 6 | 7 | 8 | 9 | 10);
                ui.p1().get_mut(front).unwrap().0 = opaque;
                {
                    let mut q = ui.p3();
                    let mut d = q.get_mut(front).unwrap();
                    d.alpha = if state.step == 5 { 0. } else { 1. };
                    d.touchable = state.step != 6;
                    d.visible = state.step != 7;
                    q.get_mut(child).unwrap().visible = state.step == 4;
                }
                if state.step == 8 {
                    let mut q = ui.p2();
                    let mut t = q.get_mut(front).unwrap();
                    t.rotation = 20.;
                    t.pivot = Vec2::splat(0.5);
                }
                if state.step == 9 {
                    ui.p2().get_mut(front).unwrap().size = Vec2::new(340., 220.);
                }
                state.phase = 1;
                state.since = state.frame;
            }
            1 if elapsed >= 12 => {
                let local = if state.step == 4 {
                    Vec2::new(45., 45.)
                } else {
                    Vec2::new(150., 100.)
                };
                let (node, t) = nodes.get(front).unwrap();
                state.cursor = Some(
                    t.transform_point2(node.border_box().min + local / node.inverse_scale_factor())
                        / window.scale_factor(),
                );
                state.count = clicks.0.len();
                state.phase = 2;
                state.since = state.frame;
            }
            2 if elapsed >= 4 => {
                mouse.press(MouseButton::Left);
                state.phase = 3;
                state.since = state.frame;
            }
            3 if elapsed >= 4 => {
                if state.step == 10 {
                    assert_eq!(input.captured(), Some(front));
                    ui.p1().get_mut(front).unwrap().0 = false;
                    state.phase = 4;
                } else {
                    mouse.release(MouseButton::Left);
                    state.phase = 5;
                }
                state.since = state.frame;
            }
            4 if elapsed >= 6 => {
                assert_eq!(input.captured(), Some(front));
                mouse.release(MouseButton::Left);
                state.phase = 5;
                state.since = state.frame;
            }
            5 if elapsed >= 6 => {
                let events = &clicks.0[state.count..];
                let targets: Vec<_> = events
                    .iter()
                    .filter_map(|&(e, b)| (!b).then_some(e))
                    .collect();
                let expected = match state.step {
                    2 | 6 | 7 => Some(back),
                    3 | 5 | 8 | 9 => Some(front),
                    4 => Some(child),
                    10 => None,
                    _ => unreachable!(),
                };
                assert_eq!(
                    targets,
                    expected.into_iter().collect::<Vec<_>>(),
                    "step {} events {events:?}",
                    state.step
                );
                if state.step == 4 {
                    assert!(
                        events.contains(&(front, true)),
                        "child click bubbles into its nonopaque parent"
                    );
                }
                assert_eq!(input.captured(), None);
                assert_eq!(ui.p0().view(state.owners[1]), Some(front));
                assert!(entities.contains(child));
                state.step += 1;
                state.phase = 0;
                state.since = state.frame;
            }
            _ => {}
        }
    } else if state.step == 11 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk("target/fairygui-demo-opaque.png"));
        for &owner in &state.owners {
            commands.entity(owner).despawn();
        }
        state.step = 12;
        state.since = state.frame;
    } else if state.step == 12 && elapsed >= 12 {
        for e in [
            state.front.unwrap(),
            state.back.unwrap(),
            state.child.unwrap(),
        ] {
            assert!(!entities.contains(e));
        }
        eprintln!(
            "opaque smoke PASSED: blank component pass-through, ECS toggles, child target/bubbling, alpha-zero picking, touchable/visibility gates, rotation/resize, retained capture after opaque change without a click, stable views and disposal"
        );
        exit.write(AppExit::Success);
        state.step = 13;
    }
    if let Some(cursor) = state.cursor {
        window.set_cursor_position(Some(cursor));
        pointers.write(PointerInput::new(
            PointerId::Mouse,
            Location {
                target: bevy::camera::RenderTarget::Window(bevy::window::WindowRef::Entity(
                    window_entity,
                ))
                .normalize(Some(window_entity))
                .unwrap(),
                position: cursor,
            },
            PointerAction::Move { delta: Vec2::ZERO },
        ));
    }
}

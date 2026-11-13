//! Actual pointer threshold, Loader proxy, entity observer response and cleanup.
use super::*;

#[derive(Resource, Default)]
struct State {
    step: u8,
    frame: u32,
    since: u32,
    page: Option<Entity>,
    source: Option<Entity>,
    receiver: Option<Entity>,
    agent: Option<Entity>,
    icon: String,
    cursor: Option<Vec2>,
    free: Option<Entity>,
    bounded: Option<Entity>,
    bounds: Option<Entity>,
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
#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    demo: Res<Demo>,
    gui: GuiQuery,
    drag: Res<GuiDragState>,
    sources: Query<&GuiDragSource>,
    motions: Query<&GuiObjectDragState>,
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
        time.elapsed_secs_f64() < 40.,
        "ECS drag smoke timeout at step {}",
        state.step
    );
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    let elapsed = state.frame - state.since;
    let center =
        |entity| nodes.get(entity).unwrap().1.transform_point2(Vec2::ZERO) / window.scale_factor();
    let mut next = false;
    match state.step {
        0 => {
            window.resolution.set(1600., 1000.);
            let Some(menu) = demo.menu_owner.filter(|&owner| gui.view(owner).is_some()) else {
                return;
            };
            commands.write_message(GuiObjectCommand::new(
                menu,
                GuiObjectAction::SetVisible(false),
            ));
            let page = commands
                .spawn(FairyGuiRoot::new(
                    assets.load("ui/Basics.fui"),
                    "Demo_Drag&Drop",
                ))
                .id();
            commands.write_message(GuiObjectCommand::new(
                page,
                GuiObjectAction::SetPosition(Vec2::new(120., 90.)),
            ));
            state.page = Some(page);
            next = true;
        }
        1 if elapsed >= 16 => {
            let Some(source) = gui.child_by_name(state.page.unwrap(), "b") else {
                return;
            };
            let Some(receiver) = gui.child_by_name(state.page.unwrap(), "c") else {
                return;
            };
            if sources.get(source).is_err() {
                return;
            }
            state.source = Some(source);
            state.receiver = Some(receiver);
            state.free = gui.child_by_name(state.page.unwrap(), "a");
            state.bounded = gui.child_by_name(state.page.unwrap(), "d");
            state.bounds = gui.child_by_name(state.page.unwrap(), "bounds");
            state.icon = gui.icon(source).unwrap().to_owned();
            assert!(!state.icon.is_empty());
            assert_eq!(gui.icon(receiver), Some(""));
            state.cursor = Some(center(source));
            next = true;
        }
        2 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        3 if elapsed >= 4 => {
            assert!(!drag.is_dragging());
            *state.cursor.as_mut().unwrap() += Vec2::splat(2.);
            next = true;
        }
        4 if elapsed >= 4 => {
            assert!(!drag.is_dragging(), "drag started below threshold");
            *state.cursor.as_mut().unwrap() += Vec2::new(30., 16.);
            next = true;
        }
        5 if elapsed >= 8 => {
            assert_eq!(drag.source(), state.source);
            let agent = drag.agent().unwrap();
            state.agent = Some(agent);
            let transform = gui.transform(agent).unwrap();
            assert_eq!(transform.size(), Vec2::splat(100.));
            assert_eq!(transform.pivot(), Vec2::splat(0.5));
            assert!(!gui.display(agent).unwrap().touchable());
            assert_eq!(gui.icon(agent), Some(state.icon.as_str()));
            state.cursor = Some(center(state.receiver.unwrap()));
            next = true;
        }
        6 if elapsed >= 8 => {
            assert_eq!(drag.target(), state.receiver);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-ecs-drag-proxy.png"));
            next = true;
        }
        7 if elapsed >= 8 => {
            assert!(drag.is_dragging());
            mouse.release(MouseButton::Left);
            next = true;
        }
        8 if elapsed >= 8 => {
            assert!(!drag.is_dragging());
            assert!(!existing.contains(state.agent.unwrap()));
            assert_eq!(gui.icon(state.receiver.unwrap()), Some(state.icon.as_str()));
            let free = state.free.unwrap();
            state.initial = gui.transform(free).unwrap().position();
            state.cursor = Some(center(free));
            next = true;
        }
        9 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        10 if elapsed >= 4 => {
            *state.cursor.as_mut().unwrap() += Vec2::new(40., 15.);
            next = true;
        }
        11 if elapsed >= 8 => {
            let free = state.free.unwrap();
            assert!(motions.get(free).unwrap().is_dragging());
            assert_eq!(gui.transform(free).unwrap().position(), state.initial);
            *state.cursor.as_mut().unwrap() += Vec2::new(80., 50.);
            next = true;
        }
        12 if elapsed >= 8 => {
            let moved = gui.transform(state.free.unwrap()).unwrap().position();
            assert!((moved - state.initial).abs_diff_eq(Vec2::new(80., 50.), 1.));
            mouse.release(MouseButton::Left);
            next = true;
        }
        13 if elapsed >= 8 => {
            assert!(!motions.get(state.free.unwrap()).unwrap().is_dragging());
            state.cursor = Some(center(state.bounded.unwrap()));
            next = true;
        }
        14 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        15 if elapsed >= 4 => {
            *state.cursor.as_mut().unwrap() += Vec2::new(20., 20.);
            next = true;
        }
        16 if elapsed >= 8 => {
            assert!(motions.get(state.bounded.unwrap()).unwrap().is_dragging());
            state.cursor = Some(Vec2::new(20., 20.));
            next = true;
        }
        17 if elapsed >= 8 => {
            let bounds = gui.transform(state.bounds.unwrap()).unwrap();
            let moved = gui.transform(state.bounded.unwrap()).unwrap();
            assert!(moved.position().abs_diff_eq(bounds.position(), 1.));
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-ecs-bounded-drag.png"));
            next = true;
        }
        18 if elapsed >= 8 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        19 if elapsed >= 8 => {
            assert!(!motions.get(state.bounded.unwrap()).unwrap().is_dragging());
            state.cursor = Some(center(state.source.unwrap()));
            next = true;
        }
        20 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        21 if elapsed >= 4 => {
            *state.cursor.as_mut().unwrap() += Vec2::new(40., 15.);
            next = true;
        }
        22 if elapsed >= 8 => {
            assert!(drag.is_dragging());
            state.agent = drag.agent();
            commands.entity(state.page.unwrap()).despawn();
            next = true;
        }
        23 if elapsed >= 8 => {
            assert!(!drag.is_dragging());
            assert!(!existing.contains(state.agent.unwrap()));
            assert!(!existing.contains(state.source.unwrap()));
            mouse.release(MouseButton::Left);
            println!(
                "demo smoke PASSED: ECS free and bounded control movement, native threshold origin, Loader proxy, entity drop icon response, pointer release, proxy cleanup and source disposal"
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

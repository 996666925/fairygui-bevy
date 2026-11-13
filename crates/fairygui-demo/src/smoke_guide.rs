//! Actual guide start, native reversed-mask input and retained overlay ownership.
use super::*;
#[derive(Resource, Default)]
struct State {
    step: u8,
    frame: u32,
    since: u32,
    page: Option<Entity>,
    layer: Option<Entity>,
    view: Option<Entity>,
    hole: Option<Entity>,
    bag: Option<Entity>,
    cursor: Option<Vec2>,
    initial: Vec2,
    destination: Vec2,
    saw_motion: bool,
    start_button: Option<Entity>,
}
#[derive(Resource, Default)]
struct Clicks(Vec<Entity>);
fn record(event: On<GuiEvent>, mut clicks: ResMut<Clicks>) {
    if event.event.kind == fairygui_core::object::EventKind::Click {
        clicks.0.push(event.entity);
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
#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    demo: Res<Demo>,
    gui: GuiQuery,
    clicks: Res<Clicks>,
    guides: Query<&crate::guide_demo::Guide>,
    roots: Query<&FairyGuiRoot>,
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
        "guide smoke timeout at step {}",
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
            let Some(menu) = demo.menu_owner.filter(|&owner| gui.view(owner).is_some()) else {
                return;
            };
            commands.write_message(GuiObjectCommand::new(
                menu,
                GuiObjectAction::SetVisible(false),
            ));
            state.page = Some(
                commands
                    .spawn(FairyGuiRoot::new(assets.load("ui/Guide.fui"), "Main"))
                    .id(),
            );
            next = true;
        }
        1 if elapsed >= 15 => {
            let page = state.page.unwrap();
            let Ok(guide) = guides.get(page) else { return };
            let Some(hole) = gui.child_by_name(guide.layer, "window") else {
                return;
            };
            assert!(!roots.get(guide.layer).unwrap().active);
            state.layer = Some(guide.layer);
            state.view = gui.view(guide.layer);
            state.hole = Some(hole);
            state.bag = gui.child_by_name(page, "bagBtn");
            state.initial = gui.transform(hole).unwrap().position();
            let start = gui.child_by_name(page, "n2").unwrap();
            state.start_button = Some(start);
            commands.entity(start).observe(record);
            commands.entity(state.bag.unwrap()).observe(record);
            state.cursor = Some(center(start));
            next = true;
        }
        2 | 6 | 9 | 12 | 17 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        3 | 7 | 10 | 13 | 18 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        4 => {
            let layer = state.layer.unwrap();
            let hole = state.hole.unwrap();
            if !roots.get(layer).unwrap().active {
                return;
            }
            let bag = gui.transform(state.bag.unwrap()).unwrap();
            let rect = gui
                .local_to_global_rect(
                    state.bag.unwrap(),
                    fairygui_core::Rect::new(0., 0., bag.size().x, bag.size().y),
                )
                .and_then(|rect| gui.global_to_local_rect(layer, rect))
                .unwrap();
            state.destination = Vec2::new(rect.x, rect.y);
            let position = gui.transform(hole).unwrap().position();
            if position.distance(state.initial) > 1. && position.distance(state.destination) > 1. {
                state.saw_motion = true;
            }
            if position.distance(state.destination) > 0.05 {
                return;
            }
            assert!(
                state.saw_motion,
                "guide must animate through intermediate positions"
            );
            assert_eq!(gui.transform(hole).unwrap().size(), bag.size());
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-ecs-guide.png"));
            // A click outside the cutout must stay on the overlay.
            state.cursor = Some(center(
                gui.child_by_name(state.page.unwrap(), "n2").unwrap(),
            ));
            next = true;
        }
        5 if elapsed >= 8 => {
            next = true;
        }
        8 if elapsed >= 8 => {
            assert_eq!(
                clicks.0,
                [state.start_button.unwrap()],
                "outside cutout must block the underlying Start button"
            );
            assert!(roots.get(state.layer.unwrap()).unwrap().active);
            state.cursor = Some(center(state.bag.unwrap()));
            next = true;
        }
        11 if elapsed >= 8 => {
            assert_eq!(clicks.0, [state.start_button.unwrap(), state.bag.unwrap()]);
            assert!(
                !roots.get(state.layer.unwrap()).unwrap().active,
                "bag click must pass through the reversed mask"
            );
            assert_eq!(gui.view(state.layer.unwrap()), state.view);
            state.cursor = Some(center(
                gui.child_by_name(state.page.unwrap(), "n2").unwrap(),
            ));
            next = true;
        }
        14 if elapsed >= 12 => {
            assert_eq!(
                clicks.0,
                [
                    state.start_button.unwrap(),
                    state.bag.unwrap(),
                    state.start_button.unwrap()
                ]
            );
            assert!(roots.get(state.layer.unwrap()).unwrap().active);
            assert_eq!(gui.view(state.layer.unwrap()), state.view);
            assert_eq!(
                gui.transform(state.hole.unwrap()).unwrap().position(),
                state.destination
            );
            window.resolution.set_physical_resolution(1440, 900);
            next = true;
        }
        15 if elapsed >= 4 => {
            next = true;
        }
        16 if elapsed >= 20 => {
            let layer = state.layer.unwrap();
            let bag = gui.transform(state.bag.unwrap()).unwrap();
            let rect = gui
                .local_to_global_rect(
                    state.bag.unwrap(),
                    fairygui_core::Rect::new(0., 0., bag.size().x, bag.size().y),
                )
                .and_then(|rect| gui.global_to_local_rect(layer, rect))
                .unwrap();
            let destination = Vec2::new(rect.x, rect.y);
            if gui
                .transform(state.hole.unwrap())
                .unwrap()
                .position()
                .distance(destination)
                > 0.05
            {
                return;
            }
            assert_eq!(
                gui.transform(layer).unwrap().size(),
                Vec2::new(window.width(), window.height())
            );
            state.cursor = Some(center(state.bag.unwrap()));
            next = true;
        }
        19 if elapsed >= 8 => {
            assert_eq!(clicks.0.len(), 4);
            assert!(!roots.get(state.layer.unwrap()).unwrap().active);
            commands.entity(state.page.unwrap()).despawn();
            next = true;
        }
        20 if elapsed >= 10 => {
            assert!(!existing.contains(state.layer.unwrap()));
            assert!(!existing.contains(state.view.unwrap()));
            assert!(!existing.contains(state.hole.unwrap()));
            println!(
                "demo smoke PASSED: ECS Guide start, native coordinate alignment, intermediate property tween, reversed-mask input blocking/pass-through, retained reopen, resize retargeting and owned disposal"
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

//! Actual topmost picking under component Ascent, Descent and Arch paint order.
use super::*;

#[derive(Resource, Default)]
struct State {
    frame: u32,
    since: u32,
    step: u8,
    round: usize,
    parent: Option<Entity>,
    buttons: Vec<Entity>,
    cursor: Option<Vec2>,
}
#[derive(Resource, Default)]
struct Clicks(Vec<Entity>);
fn clicked(event: On<GuiEvent>, mut clicks: ResMut<Clicks>) {
    if event.event.kind == fairygui_core::object::EventKind::Click && !event.event.bubbling {
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
    clicks: Res<Clicks>,
    demo: Res<Demo>,
    mut ui: ParamSet<(GuiQuery, Query<&mut GuiRenderOrder>, Query<&mut GuiDisplay>)>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    assets: Res<AssetServer>,
    mut windows: Query<(Entity, &mut Window), With<bevy::window::PrimaryWindow>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    mut commands: Commands,
    mut objects: GuiMut,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    assert!(
        state.frame < 1400,
        "render-order smoke timed out at step {}, round {}",
        state.step,
        state.round
    );
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    let elapsed = state.frame - state.since;
    let mut next = None;
    match state.step {
        0 if state.frame >= 30 => {
            let Some(menu) = demo
                .menu_owner
                .filter(|&owner| ui.p0().view(owner).is_some())
            else {
                return;
            };
            objects.get_mut(menu).unwrap().set_visible(false);
            let parent = commands
                .spawn((
                    FairyGuiRoot::new(assets.load("ui/Basics.fui"), "Component1")
                        .with_position(Vec2::new(250., 200.))
                        .with_size(Vec2::new(500., 200.)),
                    GuiRenderOrder {
                        order: ChildrenRenderOrder::Descent,
                        apex_index: 0,
                    },
                ))
                .id();
            state.parent = Some(parent);
            for title in ["A", "B", "C"] {
                state.buttons.push(
                    commands
                        .spawn((
                            FairyGuiRoot::new(assets.load("ui/Basics.fui"), "Button1")
                                .with_parent(parent)
                                .with_position(Vec2::new(80., 70.))
                                .with_size(Vec2::new(220., 60.)),
                            GuiText::new(title),
                        ))
                        .id(),
                );
            }
            next = Some(1);
        }
        1 if elapsed >= 16
            && state
                .buttons
                .iter()
                .all(|&owner| ui.p0().view(owner).is_some()) =>
        {
            let (views, children) = {
                let gui = ui.p0();
                (
                    state
                        .buttons
                        .iter()
                        .map(|&owner| gui.view(owner).unwrap())
                        .collect::<Vec<_>>(),
                    gui.get(state.parent.unwrap()).unwrap().children().to_vec(),
                )
            };
            for &child in &children {
                if views.contains(&child) {
                    commands.entity(child).observe(clicked);
                } else {
                    ui.p2().get_mut(child).unwrap().visible = false;
                }
            }
            // Independently mounted roots follow ECS query order. Declare the
            // logical A/B/C order explicitly before checking its paint order.
            for &view in &views {
                objects.get_mut(view).unwrap().set_child_index(usize::MAX);
            }
            assert_eq!(
                ui.p0().render_order(state.parent.unwrap()).unwrap().order,
                ChildrenRenderOrder::Descent
            );
            state.cursor = Some(
                nodes.get(views[0]).unwrap().1.transform_point2(Vec2::ZERO) / window.scale_factor(),
            );
            next = Some(2);
        }
        2 if elapsed >= 6 => {
            mouse.press(MouseButton::Left);
            next = Some(3);
        }
        3 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            next = Some(4);
        }
        4 if elapsed >= 5 => {
            let views = {
                let gui = ui.p0();
                state
                    .buttons
                    .iter()
                    .map(|&owner| gui.view(owner).unwrap())
                    .collect::<Vec<_>>()
            };
            let expected = [0, 2, 1, 2, 2, 0, 2];
            assert_eq!(clicks.0.len(), state.round + 1);
            assert_eq!(
                clicks.0[state.round], views[expected[state.round]],
                "paint/pick round {}",
                state.round
            );
            let parent = ui.p0().view(state.parent.unwrap()).unwrap();
            match state.round {
                0 => ui.p1().get_mut(parent).unwrap().order = ChildrenRenderOrder::Ascent,
                1 => {
                    let apex = ui.p0().child_index(views[1]).unwrap() as i32;
                    *ui.p1().get_mut(parent).unwrap() = GuiRenderOrder {
                        order: ChildrenRenderOrder::Arch,
                        apex_index: apex,
                    };
                }
                2 => ui.p2().get_mut(views[1]).unwrap().visible = false,
                3 => {
                    ui.p2().get_mut(views[1]).unwrap().visible = true;
                    ui.p1().get_mut(parent).unwrap().apex_index = i32::MAX;
                }
                4 => ui.p1().get_mut(parent).unwrap().apex_index = -4,
                5 => {
                    objects.get_mut(views[2]).unwrap().set_child_index(0);
                }
                6 => {
                    commands
                        .spawn(Screenshot::primary_window())
                        .observe(save_to_disk("target/fairygui-demo-render-order.png"));
                    next = Some(5);
                }
                _ => unreachable!(),
            }
            if state.round < 6 {
                state.round += 1;
                next = Some(2);
            }
        }
        5 if elapsed >= 6 => {
            commands.entity(state.parent.unwrap()).despawn();
            println!(
                "demo smoke PASSED: mutable component Ascent/Descent/Arch rendering, real topmost picking, hidden apex, negative/oversized apex, logical index changes and retained observers"
            );
            exit.write(AppExit::Success);
            next = Some(6);
        }
        _ => {}
    }
    if let Some(step) = next {
        state.step = step;
        state.since = state.frame;
    }
    window.set_cursor_position(state.cursor);
    move_pointer(window_entity, state.cursor, &mut pointers);
}

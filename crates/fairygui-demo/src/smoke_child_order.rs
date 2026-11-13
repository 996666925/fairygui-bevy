//! Real picking over overlapping ECS-owned controls after sibling reorders.
use super::*;

#[derive(Resource, Default)]
struct State {
    frame: u32,
    since: u32,
    step: u8,
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
    gui: GuiQuery,
    assets: Res<AssetServer>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    mut windows: Query<(Entity, &mut Window), With<bevy::window::PrimaryWindow>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    mut commands: Commands,
    mut objects: GuiMut,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    assert!(
        state.frame < 1200,
        "child-order smoke timeout at step {}",
        state.step
    );
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    let elapsed = state.frame - state.since;
    let mut next = false;
    match state.step {
        0 if state.frame >= 30 => {
            let Some(menu) = demo.menu_owner.filter(|&owner| gui.view(owner).is_some()) else {
                return;
            };
            objects.get_mut(menu).unwrap().set_visible(false);
            let parent = commands
                .spawn(
                    FairyGuiRoot::new(assets.load("ui/Basics.fui"), "Component1")
                        .with_position(Vec2::new(250., 200.))
                        .with_size(Vec2::new(500., 200.)),
                )
                .id();
            state.parent = Some(parent);
            for (title, priority) in [("A", 0), ("B", 1), ("C", 2)] {
                let button = commands
                    .spawn((
                        FairyGuiRoot::new(assets.load("ui/Basics.fui"), "Button1")
                            .with_parent(parent)
                            .with_position(Vec2::new(80., 70.))
                            .with_size(Vec2::new(220., 60.)),
                        GuiText::new(title),
                        GuiSortingOrder(priority),
                    ))
                    .id();
                state.buttons.push(button);
            }
            next = true;
        }
        1 if elapsed >= 16 && state.buttons.iter().all(|&owner| gui.view(owner).is_some()) => {
            for &button in &state.buttons {
                commands.entity(gui.view(button).unwrap()).observe(clicked);
            }
            let view = gui.view(state.buttons[0]).unwrap();
            state.cursor = Some(
                nodes.get(view).unwrap().1.transform_point2(Vec2::ZERO) / window.scale_factor(),
            );
            next = true;
        }
        2 | 6 | 10 | 14 | 18 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        3 | 7 | 11 | 15 | 19 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        4 if elapsed >= 5 => {
            assert_eq!(clicks.0, [gui.view(state.buttons[2]).unwrap()]);
            commands
                .entity(gui.view(state.buttons[0]).unwrap())
                .insert(GuiSortingOrder(5));
            next = true;
        }
        5 if elapsed >= 4 => {
            let view = gui.view(state.buttons[0]).unwrap();
            assert_eq!(gui.sorting_order(view), Some(5));
            assert_eq!(
                gui.get(state.parent.unwrap()).unwrap().children().last(),
                Some(&view)
            );
            assert_eq!(
                objects.get_mut(view).unwrap().set_child_index(0),
                gui.child_index(view)
            );
            next = true;
        }
        8 if elapsed >= 5 => {
            assert_eq!(clicks.0[1], gui.view(state.buttons[0]).unwrap());
            commands
                .entity(gui.view(state.buttons[0]).unwrap())
                .insert(GuiSortingOrder(-1));
            objects
                .get_mut(state.buttons[1])
                .unwrap()
                .set_sorting_order(0);
            objects
                .get_mut(state.buttons[2])
                .unwrap()
                .set_sorting_order(0);
            next = true;
        }
        9 if elapsed >= 4 => {
            let a = gui.view(state.buttons[0]).unwrap();
            objects.get_mut(a).unwrap().set_child_index(usize::MAX);
            next = true;
        }
        12 if elapsed >= 5 => {
            assert_eq!(clicks.0[2], gui.view(state.buttons[0]).unwrap());
            objects
                .get_mut(state.buttons[1])
                .unwrap()
                .swap_with(gui.view(state.buttons[0]).unwrap());
            next = true;
        }
        13 if elapsed >= 4 => {
            next = true;
        }
        16 if elapsed >= 5 => {
            assert_eq!(clicks.0[3], gui.view(state.buttons[1]).unwrap());
            objects
                .get_mut(state.buttons[2])
                .unwrap()
                .set_child_index_before(gui.get(state.parent.unwrap()).unwrap().children().len());
            next = true;
        }
        17 if elapsed >= 4 => {
            let c = gui.view(state.buttons[2]).unwrap();
            assert_eq!(
                gui.get(state.parent.unwrap()).unwrap().children().last(),
                Some(&c)
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-child-order.png"));
            next = true;
        }
        20 if elapsed >= 5 => {
            assert_eq!(clicks.0[4], gui.view(state.buttons[2]).unwrap());
            commands.entity(state.parent.unwrap()).despawn();
            println!(
                "demo smoke PASSED: mutable sibling priority, sorted index restriction, priority reset, direct index/swap/before operations, matching logical order and actual topmost picking"
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

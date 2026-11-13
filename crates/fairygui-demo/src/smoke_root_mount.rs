//! ECS-owned child roots inherit rendered parent transforms, clips and input.
use super::*;
use fairygui_core::object::EventKind;

#[derive(Resource, Default)]
struct State {
    step: u8,
    frame: u32,
    since: u32,
    panels: Vec<Entity>,
    nested: Option<Entity>,
    retained: Option<Entity>,
    cursor: Option<Vec2>,
}
#[derive(Resource, Default)]
struct Events {
    child: usize,
    panels: Vec<Entity>,
}
fn clicked(event: On<GuiEvent>, mut events: ResMut<Events>) {
    if event.event.kind != EventKind::Click {
        return;
    }
    if event.event.bubbling {
        events.panels.push(event.entity);
    } else {
        events.child += 1;
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
#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    demo: Res<Demo>,
    gui: GuiQuery,
    events: Res<Events>,
    assets: Res<AssetServer>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    existing: Query<()>,
    hierarchy: Query<&ChildOf>,
    mut roots: Query<&mut FairyGuiRoot>,
    mut windows: Query<(Entity, &mut Window), With<bevy::window::PrimaryWindow>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    assert!(
        state.frame < 1800,
        "root mount smoke timed out at step {}",
        state.step
    );
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    let center =
        |entity| nodes.get(entity).unwrap().1.transform_point2(Vec2::ZERO) / window.scale_factor();
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
            for (position, scale) in [(Vec2::new(100., 100.), 1.25), (Vec2::new(650., 100.), 1.)] {
                let owner = commands
                    .spawn(
                        FairyGuiRoot::new(assets.load("ui/Basics.fui"), "Component1")
                            .with_size(Vec2::new(300., 180.))
                            .with_position(position),
                    )
                    .id();
                commands.write_message(GuiObjectCommand::new(
                    owner,
                    GuiObjectAction::SetScale(Vec2::splat(scale)),
                ));
                state.panels.push(owner);
            }
            let nested = commands
                .spawn(
                    FairyGuiRoot::new(assets.load("ui/Basics.fui"), "Button1")
                        .with_parent(state.panels[0])
                        .with_position(Vec2::new(20., 60.))
                        .with_size(Vec2::new(140., 36.)),
                )
                .id();
            commands.write_message(GuiObjectCommand::new(
                nested,
                GuiObjectAction::SetText("Mounted child".into()),
            ));
            state.nested = Some(nested);
            next = true;
        }
        1 if elapsed >= 16 => {
            let Some(view) = gui.view(state.nested.unwrap()) else {
                return;
            };
            assert_eq!(gui.get(view).unwrap().parent(), gui.view(state.panels[0]));
            assert_eq!(
                gui.local_to_global(view, Vec2::ZERO),
                Some(Vec2::new(125., 175.))
            );
            commands.entity(view).observe(clicked);
            for &panel in &state.panels {
                commands.entity(gui.view(panel).unwrap()).observe(clicked);
            }
            state.retained = Some(view);
            state.cursor = Some(center(view));
            next = true;
        }
        2 | 6 | 10 | 16 | 20 | 25 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        3 | 7 | 11 | 17 | 21 | 26 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        4 if elapsed >= 5 => {
            assert_eq!(events.child, 1);
            assert_eq!(events.panels, [gui.view(state.panels[0]).unwrap()]);
            let next_parent = gui.view(state.panels[1]).unwrap();
            roots.get_mut(state.nested.unwrap()).unwrap().parent = Some(next_parent);
            next = true;
        }
        5 if elapsed >= 6 => {
            let view = gui.view(state.nested.unwrap()).unwrap();
            assert_eq!(Some(view), state.retained);
            assert_eq!(
                hierarchy.get(view).unwrap().parent(),
                gui.view(state.panels[1]).unwrap()
            );
            state.cursor = Some(center(view));
            next = true;
        }
        8 if elapsed >= 5 => {
            assert_eq!(events.child, 2);
            assert_eq!(events.panels.len(), 2);
            assert_eq!(events.panels[1], gui.view(state.panels[1]).unwrap());
            roots.get_mut(state.panels[1]).unwrap().active = false;
            next = true;
        }
        9 if elapsed >= 5 => {
            assert!(!gui.display(state.nested.unwrap()).unwrap().on_stage());
            assert_eq!(gui.view(state.nested.unwrap()), state.retained);
            next = true;
        }
        12 if elapsed >= 5 => {
            assert_eq!(events.child, 2, "hidden parent excludes child input");
            roots.get_mut(state.panels[1]).unwrap().active = true;
            next = true;
        }
        13 if elapsed >= 5 => {
            assert!(gui.display(state.nested.unwrap()).unwrap().on_stage());
            let mut child = roots.get_mut(state.nested.unwrap()).unwrap();
            child.parent = Some(state.panels[0]);
            child.position = Some(Vec2::new(250., 60.));
            commands.entity(state.panels[1]).despawn();
            next = true;
        }
        14 if elapsed >= 6 => {
            assert_eq!(gui.view(state.nested.unwrap()), state.retained);
            assert!(!existing.contains(state.panels[1]));
            // Its center lies outside the 300px parent's clip.
            state.cursor = Some(center(state.retained.unwrap()));
            next = true;
        }
        15 if elapsed >= 4 => {
            next = true;
        }
        18 if elapsed >= 5 => {
            assert_eq!(events.child, 2, "clipped child area cannot receive input");
            state.cursor = gui.local_to_global(state.nested.unwrap(), Vec2::new(20., 18.));
            next = true;
        }
        19 if elapsed >= 4 => {
            next = true;
        }
        22 if elapsed >= 5 => {
            assert_eq!(events.child, 3);
            assert_eq!(events.panels.len(), 3);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-root-mount-clipped.png"));
            next = true;
        }
        23 if elapsed >= 8 => {
            let mut child = roots.get_mut(state.nested.unwrap()).unwrap();
            child.parent = None;
            child.position = Some(Vec2::new(100., 400.));
            commands.entity(state.panels[0]).despawn();
            next = true;
        }
        24 if elapsed >= 6 => {
            assert_eq!(gui.view(state.nested.unwrap()), state.retained);
            assert!(gui.get(state.nested.unwrap()).unwrap().parent().is_none());
            state.cursor = Some(center(state.retained.unwrap()));
            next = true;
        }
        27 if elapsed >= 5 => {
            assert_eq!(events.child, 4);
            assert_eq!(events.panels.len(), 3);
            commands.entity(state.nested.unwrap()).despawn();
            next = true;
        }
        28 if elapsed >= 4 => {
            assert!(!existing.contains(state.retained.unwrap()));
            println!(
                "demo smoke PASSED: ECS parent owner/view mounts, scaled local geometry, retained reparenting, parent event bubbling, hide/restore input, clipping, same-frame old-parent disposal, top-level promotion and disposal"
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

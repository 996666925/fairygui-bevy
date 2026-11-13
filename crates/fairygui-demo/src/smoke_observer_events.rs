//! Real checkbox input with Bevy-controlled logical event propagation.
use super::*;
use fairygui_core::object::{EventData, EventKind};

#[derive(Resource, Default)]
struct State {
    step: u8,
    frame: u32,
    since: u32,
    owner: Option<Entity>,
    button: Option<Entity>,
    cursor: Option<Vec2>,
}
#[derive(Resource, Default)]
struct Events {
    selected: Vec<bool>,
    parent_clicks: usize,
}
fn child(
    mut event: On<GuiEvent>,
    mut gui: ParamSet<(GuiQuery, Query<&mut GuiText>)>,
    selected: Query<&GuiButtonSelected>,
    mut events: ResMut<Events>,
) {
    if event.event.kind != EventKind::Click {
        return;
    }
    assert_eq!(event.target, Some(event.entity));
    assert!(!event.event.bubbling);
    let checked = selected.get(event.entity).unwrap().0;
    events.selected.push(checked);
    if events.selected.len() == 1 {
        event.propagate(false);
        event.event.data = EventData::Text("stopped".into());
    } else {
        event.event.data = EventData::Text("shared".into());
    }
    let label = {
        let query = gui.p0();
        let root = query.get(event.entity).unwrap().parent().unwrap();
        query.child_by_name(root, "n8").unwrap()
    };
    gui.p1()
        .get_mut(label)
        .unwrap()
        .set(format!("Local: {checked}"));
}
fn parent(
    event: On<GuiEvent>,
    mut gui: ParamSet<(GuiQuery, Query<&mut GuiText>)>,
    mut events: ResMut<Events>,
) {
    if event.event.kind != EventKind::Click {
        return;
    }
    assert!(event.event.bubbling);
    assert_ne!(event.target, Some(event.entity));
    assert_eq!(event.event.data, EventData::Text("shared".into()));
    events.parent_clicks += 1;
    let label = gui.p0().child_by_name(event.entity, "n8").unwrap();
    gui.p1().get_mut(label).unwrap().set("Parent: false");
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
    mut windows: Query<(Entity, &mut Window), With<bevy::window::PrimaryWindow>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
    mut gui_edit: GuiMut,
) {
    state.frame += 1;
    assert!(
        state.frame < 1400,
        "observer event smoke timed out at step {}",
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
            gui_edit.edit(menu, |object: &mut GuiObjectMut| object.set_visible(false));
            state.owner = Some(
                commands
                    .spawn(
                        FairyGuiRoot::new(assets.load("ui/Basics.fui"), "Demo_Button")
                            .with_position(Vec2::new(70., 70.)),
                    )
                    .id(),
            );
            next = true;
        }
        1 if elapsed >= 16 => {
            let Some(button) = gui.child_by_name(state.owner.unwrap(), "n4") else {
                return;
            };
            commands.entity(button).observe(child);
            commands
                .entity(gui.view(state.owner.unwrap()).unwrap())
                .observe(parent);
            state.button = Some(button);
            state.cursor = Some(
                nodes.get(button).unwrap().1.transform_point2(Vec2::ZERO) / window.scale_factor(),
            );
            next = true;
        }
        2 | 6 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        3 | 7 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        4 if elapsed >= 6 => {
            assert_eq!(events.selected, [true]);
            assert_eq!(events.parent_clicks, 0);
            let label = gui.child_by_name(state.owner.unwrap(), "n8").unwrap();
            assert_eq!(gui.text(label), Some("Local: true"));
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-observer-stopped.png"));
            next = true;
        }
        5 if elapsed >= 8 => {
            next = true;
        }
        8 if elapsed >= 6 => {
            assert_eq!(events.selected, [true, false]);
            assert_eq!(events.parent_clicks, 1);
            let label = gui.child_by_name(state.owner.unwrap(), "n8").unwrap();
            assert_eq!(gui.text(label), Some("Parent: false"));
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-observer-bubbled.png"));
            next = true;
        }
        9 if elapsed >= 8 => {
            commands.entity(state.owner.unwrap()).despawn();
            next = true;
        }
        10 if elapsed >= 4 => {
            assert!(!existing.contains(state.button.unwrap()));
            println!(
                "demo smoke PASSED: actual checkbox picking, current selection snapshots, child observer propagation stop, shared payload at parent, same-frame commands, repeated independent events and disposal"
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

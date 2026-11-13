//! Real Slider rows inside an ECS virtual list, with native press boundaries.
use super::*;

#[derive(Resource, Default)]
struct State {
    step: u8,
    frame: u32,
    since: u32,
    owners: Vec<Entity>,
    list: Option<Entity>,
    slider: Option<Entity>,
    cursor: Option<Vec2>,
    value: f64,
}
#[derive(Resource, Default)]
struct Events {
    pane_presses: usize,
    values: Vec<f64>,
}
fn pane_pressed(event: On<GuiEvent>, mut events: ResMut<Events>) {
    if event.event.kind == fairygui_core::object::EventKind::MouseDown {
        events.pane_presses += 1;
    }
}
fn changed(event: On<GuiEvent>, gui: GuiQuery, mut events: ResMut<Events>) {
    if event.event.kind == fairygui_core::object::EventKind::StateChanged && !event.event.bubbling {
        events
            .values
            .push(gui.slider(event.entity).unwrap().value());
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
    mut windows: Query<(Entity, &mut Window), With<bevy::window::PrimaryWindow>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
    mut gui_edit: GuiMut,
) {
    state.frame += 1;
    assert!(
        state.frame < 1800,
        "slider scroll smoke timed out at step {}",
        state.step
    );
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    let elapsed = state.frame - state.since;
    let center =
        |entity| nodes.get(entity).unwrap().1.transform_point2(Vec2::ZERO) / window.scale_factor();
    let offset = || {
        gui.scroll(state.list.unwrap())
            .unwrap()
            .display_position()
            .y
    };
    let value = || gui.slider(state.slider.unwrap()).unwrap().value();
    let mut next = false;
    match state.step {
        0 if state.frame >= 30 => {
            let Some(menu) = demo.menu_owner.filter(|&owner| gui.view(owner).is_some()) else {
                return;
            };
            gui_edit.edit(menu, |object: &mut GuiObjectMut| object.set_visible(false));
            let prefab = commands
                .spawn(FairyGuiRoot::new(assets.load("ui/Basics.fui"), "Slider_HZ"))
                .id();
            commands.entity(prefab).insert(GuiDisplay::hidden());
            let page = commands
                .spawn((
                    FairyGuiRoot::new(assets.load("ui/ListEffect.fui"), "Main")
                        .with_position(Vec2::new(200., 30.)),
                    list_effect_demo::Bound,
                ))
                .id();
            state.owners = vec![prefab, page];
            next = true;
        }
        1 if elapsed >= 16 && gui.slider(state.owners[0]).is_some() => {
            let Some(list) = gui.child_by_name(state.owners[1], "mailList") else {
                return;
            };
            let width = gui.scroll(list).unwrap().view_size().x;
            commands
                .entity(list)
                .insert(
                    GuiVirtualList::new(60, move |_, row| {
                        row.set_size(Vec2::new(width, 36.));
                    })
                    .with_item_provider(|_| Some("ui://Basics/Slider_HZ".into())),
                )
                .observe(pane_pressed);
            state.list = Some(list);
            next = true;
        }
        2 if elapsed >= 16 => {
            let list = gui.list(state.list.unwrap()).unwrap();
            assert_eq!(list.item_count(), 60);
            assert!(list.items().len() < 25);
            let slider = list.items()[5];
            assert_eq!(gui.slider(slider).unwrap().value(), 50.);
            state.slider = Some(slider);
            commands.entity(slider).observe(changed);
            state.cursor = gui.local_to_global(slider, Vec2::new(300., 12.));
            next = true;
        }
        3 | 7 | 11 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        4 if elapsed >= 4 => {
            assert!(value() > 50.);
            assert_eq!((events.pane_presses, events.values.len()), (1, 1));
            state.value = value();
            state.cursor.as_mut().unwrap().y -= 100.;
            next = true;
        }
        5 if elapsed >= 4 => {
            assert!((offset() - 100.).abs() < 0.1);
            assert_eq!(value(), state.value);
            mouse.release(MouseButton::Left);
            gui_edit.edit(state.list.unwrap(), |object: &mut GuiObjectMut| {
                object.scroll_to(Vec2::ZERO, false)
            });
            gui_edit.edit(state.slider.unwrap(), |object: &mut GuiObjectMut| {
                object.set_slider_value(50.)
            });
            next = true;
        }
        6 if elapsed >= 8 => {
            assert!(offset().abs() < 0.1);
            let grip = gui.child_by_name(state.slider.unwrap(), "grip").unwrap();
            state.cursor = Some(center(grip));
            next = true;
        }
        8 if elapsed >= 4 => {
            assert_eq!((events.pane_presses, events.values.len()), (1, 1));
            assert_eq!(value(), 50.);
            let track = gui.transform(state.slider.unwrap()).unwrap().size().x - 12.;
            *state.cursor.as_mut().unwrap() += Vec2::new(track / 4., -40.);
            next = true;
        }
        9 if elapsed >= 4 => {
            assert!((value() - 75.).abs() < 0.001);
            assert!(offset().abs() < 0.1);
            assert_eq!((events.pane_presses, events.values.len()), (1, 2));
            mouse.release(MouseButton::Left);
            gui_edit.edit(state.slider.unwrap(), |object: &mut GuiObjectMut| {
                object.set_slider_change_on_click(false)
            });
            next = true;
        }
        10 if elapsed >= 4 => {
            state.cursor = gui.local_to_global(state.slider.unwrap(), Vec2::new(330., 12.));
            next = true;
        }
        12 if elapsed >= 4 => {
            assert!((value() - 75.).abs() < 0.001);
            assert_eq!((events.pane_presses, events.values.len()), (2, 2));
            state.cursor.as_mut().unwrap().y -= 100.;
            next = true;
        }
        13 if elapsed >= 4 => {
            assert!((offset() - 100.).abs() < 0.1);
            assert!((value() - 75.).abs() < 0.001);
            mouse.release(MouseButton::Left);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-slider-scroll.png"));
            next = true;
        }
        14 if elapsed >= 8 => {
            for &owner in &state.owners {
                commands.entity(owner).despawn();
            }
            next = true;
        }
        15 if elapsed >= 4 => {
            assert!(state.owners.iter().all(|&owner| !existing.contains(owner)));
            println!(
                "demo smoke PASSED: real virtual slider rows, track press change with parent scrolling, grip press boundary and exclusive dragging, disabled track changes with parent scrolling, disposal"
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

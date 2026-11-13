//! Real mouse routes through nested horizontal rows and a vertical list.
use super::*;
#[derive(Resource, Default)]
struct State {
    step: u8,
    frame: u32,
    since: u32,
    owner: Option<Entity>,
    list: Option<Entity>,
    row: Option<Entity>,
    rows: Vec<Entity>,
    cursor: Option<Vec2>,
    start: Vec2,
    expected: String,
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
    memberships: Query<&GuiListRow>,
    existing: Query<()>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    assets: Res<AssetServer>,
    mut windows: Query<(Entity, &mut Window)>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
    time: Res<Time<bevy::time::Real>>,
    mut gui_edit: GuiMut,
) {
    state.frame += 1;
    assert!(
        time.elapsed_secs_f64() < 50.,
        "nested scroll smoke timeout step {}",
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
            gui_edit.edit(menu, |object: &mut GuiObjectMut| object.set_visible(false));
            let owner = commands
                .spawn(
                    FairyGuiRoot::new(assets.load("ui/ScrollPane.fui"), "Main")
                        .with_position(Vec2::new(40., 30.)),
                )
                .id();

            state.owner = Some(owner);
            next = true;
        }
        1 if elapsed >= 16 => {
            let Some(list) = gui.child_by_name(state.owner.unwrap(), "list") else {
                return;
            };
            let Some(content) = gui.list(list).filter(|list| list.item_count() == 1000) else {
                return;
            };
            assert!(content.items().len() < 20);
            state.list = Some(list);
            state.rows = content.items().to_vec();
            let row = content.items()[2];
            state.row = Some(row);
            assert_eq!(gui.text(row), Some("Item 2"));
            state.start = center(row) + Vec2::new(120., 0.);
            state.cursor = Some(state.start);
            next = true;
        }
        2 | 6 | 10 | 14 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        3 if elapsed >= 4 => {
            state.cursor = Some(state.start - Vec2::new(280., 2.));
            next = true;
        }
        4 if elapsed >= 8 => {
            let row = state.row.unwrap();
            assert!(gui.scroll(row).unwrap().display_position().x > 200.);
            assert_eq!(
                gui.scroll(state.list.unwrap())
                    .unwrap()
                    .display_position()
                    .y,
                0.
            );
            mouse.release(MouseButton::Left);
            next = true;
        }
        5 if elapsed >= 35 => {
            let row = state.row.unwrap();
            state.cursor = Some(center(gui.child_by_name(row, "b0").unwrap()));
            next = true;
        }
        7 | 11 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        8 if elapsed >= 10 => {
            assert_eq!(
                gui.text(gui.child_by_name(state.owner.unwrap(), "txt").unwrap()),
                Some("Stick Item 2")
            );
            state.cursor = Some(center(gui.child_by_name(state.row.unwrap(), "b1").unwrap()));
            next = true;
        }
        9 if elapsed >= 4 => {
            next = true;
        }
        12 if elapsed >= 10 => {
            assert_eq!(
                gui.text(gui.child_by_name(state.owner.unwrap(), "txt").unwrap()),
                Some("Delete Item 2")
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-ecs-nested-scroll.png"));
            state.start = center(state.rows[3]);
            state.cursor = Some(state.start);
            next = true;
        }
        13 if elapsed >= 4 => {
            next = true;
        }
        15 if elapsed >= 4 => {
            state.cursor = Some(state.start - Vec2::new(2., 140.));
            next = true;
        }
        16 if elapsed >= 35 => {
            assert_eq!(
                gui.scroll(state.row.unwrap()).unwrap().display_position().x,
                0.
            );
            assert_eq!(
                gui.scroll(state.list.unwrap())
                    .unwrap()
                    .display_position()
                    .y,
                0.,
                "closing press must cancel outer candidate"
            );
            mouse.release(MouseButton::Left);
            state.cursor = Some(center(state.rows[4]));
            state.start = state.cursor.unwrap();
            next = true;
        }
        17 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        18 if elapsed >= 4 => {
            state.cursor = Some(state.start - Vec2::new(2., 160.));
            next = true;
        }
        19 if elapsed >= 8 => {
            assert!(
                gui.scroll(state.list.unwrap())
                    .unwrap()
                    .display_position()
                    .y
                    > 100.
            );
            mouse.release(MouseButton::Left);
            commands.write_message(GuiInputCommand::CancelScrollDrag(state.list.unwrap()));
            gui_edit.edit(state.list.unwrap(), |object: &mut GuiObjectMut| {
                object.scroll_to(Vec2::new(0., 7000.), false)
            });
            next = true;
        }
        // Recycled row observers must use the new logical row's text.
        20 if elapsed >= 12 => {
            let content = gui.list(state.list.unwrap()).unwrap();
            let row = content.items()[2];
            assert!(memberships.get(row).unwrap().index() > 50);
            assert_eq!(gui.scroll(row).unwrap().display_position().x, 0.);
            state.rows.extend_from_slice(content.items());
            state.row = Some(row);
            state.expected = format!("Stick {}", gui.text(row).unwrap());
            state.start = center(row) + Vec2::new(120., 0.);
            state.cursor = Some(state.start);
            next = true;
        }
        21 if elapsed >= 8 => {
            // Run a second actual horizontal drag on the rebound row.
            mouse.press(MouseButton::Left);
            next = true;
        }
        22 if elapsed >= 4 => {
            state.cursor = Some(state.start - Vec2::new(280., 0.));
            next = true;
        }
        23 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        24 if elapsed >= 35 => {
            state.cursor = Some(center(gui.child_by_name(state.row.unwrap(), "b0").unwrap()));
            next = true;
        }
        25 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        26 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        27 if elapsed >= 10 => {
            assert_eq!(
                gui.text(gui.child_by_name(state.owner.unwrap(), "txt").unwrap()),
                Some(state.expected.as_str())
            );
            commands.entity(state.owner.unwrap()).despawn();
            next = true;
        }
        28 if elapsed >= 10 => {
            assert!(state.rows.iter().all(|&row| !existing.contains(row)));
            println!(
                "demo smoke PASSED: nested axis arbitration, 1000 ECS virtual rows, actual horizontal/vertical drags, Stick/Delete entity observers, closing-press cancellation, recycled row reset and disposal"
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

//! Capture native delayed mail transitions and exercise retained-row selection.
use super::*;

#[derive(Resource, Default)]
struct State {
    step: u8,
    frame: u32,
    since: u32,
    started: f32,
    owner: Option<Entity>,
    list: Option<Entity>,
    rows: Vec<Entity>,
    cursor: Option<Vec2>,
    completions: Vec<GuiTransitionComplete>,
    atlas: Option<Handle<Image>>,
    atlas_ready: Option<u32>,
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
    existing: Query<()>,
    selection: Query<&GuiListSelection>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    assets: Res<AssetServer>,
    images: Res<Assets<Image>>,
    mut windows: Query<(Entity, &mut Window)>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
    mut completions: MessageReader<GuiTransitionComplete>,
    time: Res<Time<bevy::time::Real>>,
) {
    state.frame += 1;
    assert!(
        time.elapsed_secs_f64() < 40.,
        "list effect smoke timeout at step {}",
        state.step
    );
    state.completions.extend(completions.read().cloned());
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
            let atlas = state
                .atlas
                .get_or_insert_with(|| assets.load("ui/ListEffect_atlas0.png"));
            if !images.contains(atlas.id()) {
                return;
            }
            let frame = state.frame;
            let ready = *state.atlas_ready.get_or_insert(frame);
            if frame - ready < 4 {
                return;
            }
            commands.write_message(GuiObjectCommand::new(
                menu,
                GuiObjectAction::SetVisible(false),
            ));
            let owner = commands
                .spawn(FairyGuiRoot::new(assets.load("ui/ListEffect.fui"), "Main"))
                .id();
            commands.write_message(GuiObjectCommand::new(
                owner,
                GuiObjectAction::SetPosition(Vec2::new(200., 30.)),
            ));
            state.owner = Some(owner);
            next = true;
        }
        1 => {
            let Some(list) = gui.child_by_name(state.owner.unwrap(), "mailList") else {
                return;
            };
            let Some(content) = gui.list(list).filter(|list| list.items().len() == 10) else {
                return;
            };
            let row = content.items()[0];
            if gui
                .transition(row, "t0")
                .is_none_or(|transition| transition.play_generation() == 0)
            {
                return;
            }
            state.rows = content.items().to_vec();
            state.list = Some(list);
            state.started = time.elapsed_secs();
            assert!(gui.display(row).unwrap().visible());
            assert!(gui.transform(row).unwrap().position().x > 300.);
            for (index, &row) in state.rows.iter().enumerate() {
                assert_eq!(
                    gui.transition(row, "t0").unwrap().play_generation(),
                    u64::from(index < 4)
                );
                assert_eq!(
                    gui.display(row).unwrap().visible(),
                    index == 0 || index >= 4
                );
                assert_eq!(gui.text(row), Some("Mail title here"));
                assert_eq!(
                    gui.controller(row, "IsRead").unwrap().selected_index(),
                    i32::from(index % 2 == 0)
                );
                assert_eq!(
                    gui.controller(row, "c1").unwrap().selected_index(),
                    i32::from(index % 3 == 0)
                );
            }
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-ecs-list-effect-start.png",
                ));
            next = true;
        }
        2 if time.elapsed_secs() - state.started >= 1.2
            && state.rows.iter().all(|&row| {
                gui.transition(row, "t0")
                    .is_some_and(|transition| !transition.is_playing())
            }) =>
        {
            for (index, &row) in state.rows.iter().enumerate() {
                assert!(gui.display(row).unwrap().visible());
                assert_eq!(
                    gui.transform(row).unwrap().position(),
                    Vec2::new(0., index as f32 * 125.)
                );
                assert!(!gui.transition(row, "t0").unwrap().is_playing());
            }
            assert_eq!(
                state
                    .completions
                    .iter()
                    .filter(|completion| state.rows.contains(&completion.entity))
                    .count(),
                4
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-ecs-list-effect.png"));
            state.cursor = Some(center(state.rows[1]));
            next = true;
        }
        3 | 7 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        4 | 8 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        5 if elapsed >= 10 => {
            assert_eq!(selection.get(state.list.unwrap()).unwrap().0, [1]);
            commands.write_message(GuiObjectCommand::new(
                state.list.unwrap(),
                GuiObjectAction::ScrollTo {
                    position: Vec2::new(0., 500.),
                    animated: false,
                },
            ));
            next = true;
        }
        6 if elapsed >= 12 => {
            let list = state.list.unwrap();
            assert_eq!(gui.is_child_in_view(list, state.rows[4]), Some(true));
            assert_eq!(
                gui.transition(state.rows[4], "t0")
                    .unwrap()
                    .play_generation(),
                0
            );
            state.cursor = Some(center(state.rows[5]));
            next = true;
        }
        9 if elapsed >= 10 => {
            assert_eq!(selection.get(state.list.unwrap()).unwrap().0, [5]);
            commands.entity(state.owner.unwrap()).despawn();
            next = true;
        }
        10 if elapsed >= 10 => {
            assert!(state.rows.iter().all(|&row| !existing.contains(row)));
            println!(
                "demo smoke PASSED: ECS ListEffect mail data, four visible staggered transitions, delayed visibility, final positions, exactly-once completion, actual selection before/after scrolling and owned disposal"
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

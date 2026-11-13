//! Real button observers control direct and Loader movies through ECS.
use super::*;

#[derive(Resource, Default)]
struct State {
    step: u8,
    frame: u32,
    since: u32,
    page: Option<Entity>,
    controls: Vec<Entity>,
    cursor: Option<Vec2>,
}
#[derive(Resource, Default)]
struct Results {
    clicks: u8,
    completed: Vec<(Entity, i32)>,
}
#[derive(Component)]
struct Controls([Entity; 2]);

fn click(
    event: On<GuiEvent>,
    controls: Query<&Controls>,
    mut results: ResMut<Results>,
    mut commands: MessageWriter<GuiObjectCommand>,
) {
    if event.event.kind != fairygui_core::object::EventKind::Click {
        return;
    }
    let Ok(controls) = controls.get(event.entity) else {
        return;
    };
    results.clicks += 1;
    for entity in controls.0 {
        for action in if results.clicks == 1 {
            vec![
                GuiObjectAction::SetAnimationPlaying(false),
                GuiObjectAction::SetAnimationFrame(1),
            ]
        } else {
            vec![
                GuiObjectAction::SetAnimationPlaying(true),
                GuiObjectAction::SetAnimationTimeScale(100.),
                GuiObjectAction::SetAnimationPlaySettings(GuiAnimationPlaySettings {
                    start: 1,
                    end: 2,
                    times: 1,
                    end_at: 3,
                }),
            ]
        } {
            commands.write(GuiObjectCommand::new(entity, action));
        }
    }
}
fn completed(
    event: On<GuiAnimationComplete>,
    gui: GuiQuery,
    mut results: ResMut<Results>,
    mut commands: MessageWriter<GuiObjectCommand>,
) {
    let state = gui.animation(event.entity).unwrap();
    assert!(state.is_ended() && state.is_playing());
    assert_eq!(state.frame(), 3);
    results.completed.push((event.entity, state.frame()));
    commands.write(GuiObjectCommand::new(
        event.entity,
        GuiObjectAction::SetAnimationPlaying(false),
    ));
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<State>()
        .init_resource::<Results>()
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(16),
        ))
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
    results: Res<Results>,
    demo: Res<Demo>,
    gui: GuiQuery,
    images: Query<&ImageNode>,
    players: Query<&fairygui_bevy::movie::MovieClipPlayer>,
    children: Query<&Children>,
    existing: Query<()>,
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
        "animation smoke timeout step {}",
        state.step
    );
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    let elapsed = state.frame - state.since;
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
            let page = commands
                .spawn(FairyGuiRoot::new(
                    assets.load("ui/Basics.fui"),
                    "Demo_MovieClip",
                ))
                .id();
            commands.write_message(GuiObjectCommand::new(
                page,
                GuiObjectAction::SetPosition(Vec2::new(50., 60.)),
            ));
            state.page = Some(page);
            next = true;
        }
        1 if elapsed >= 15 => {
            let page = state.page.unwrap();
            let Some(movie) = gui.child_by_name(page, "n8") else {
                return;
            };
            let button = gui.child_by_name(page, "n18").unwrap();
            let loader = gui.child_by_name(button, "icon").unwrap();
            state.controls = vec![movie, loader];
            for &entity in &state.controls {
                assert!(gui.animation(entity).unwrap().frame_count() > 3);
                commands.entity(entity).observe(completed);
            }
            commands
                .entity(button)
                .insert(Controls([movie, loader]))
                .observe(click);
            state.cursor = gui.local_to_global(button, gui.transform(button).unwrap().size() / 2.);
            next = true;
        }
        2 | 5 if elapsed >= 4 => {
            mouse.press(MouseButton::Left);
            next = true;
        }
        3 | 6 if elapsed >= 4 => {
            mouse.release(MouseButton::Left);
            next = true;
        }
        4 if elapsed >= 12 => {
            assert_eq!(results.clicks, 1);
            for &entity in &state.controls {
                let current = gui.animation(entity).unwrap();
                assert!(!current.is_playing());
                assert_eq!(current.frame(), 1);
            }
            next = true;
        }
        7 if elapsed >= 12 => {
            assert_eq!(results.clicks, 2);
            let mut expected: Vec<_> = state.controls.iter().map(|&entity| (entity, 3)).collect();
            expected.sort_by_key(|(entity, _)| *entity);
            let mut actual = results.completed.clone();
            actual.sort_by_key(|(entity, _)| *entity);
            assert_eq!(actual, expected);
            for &entity in &state.controls {
                let current = gui.animation(entity).unwrap();
                assert!(
                    !current.is_playing(),
                    "completion observer's command must apply"
                );
                assert!(current.is_ended());
                assert_eq!(current.frame(), 3);
                let texture = players.get(entity).unwrap().current().unwrap();
                let image = children
                    .get(entity)
                    .unwrap()
                    .iter()
                    .find_map(|child| images.get(child).ok())
                    .unwrap();
                assert_eq!(
                    image.rect,
                    Some(Rect::new(
                        texture.rect.x,
                        texture.rect.y,
                        texture.rect.x + texture.rect.width,
                        texture.rect.y + texture.rect.height
                    ))
                );
            }
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-ecs-animation.png"));
            next = true;
        }
        8 if elapsed >= 8 => {
            let loader = state.controls[1];
            commands.write_message(GuiObjectCommand::new(
                loader,
                GuiObjectAction::SetIcon("ui://9leh0eyfrpmbf".into()),
            ));
            next = true;
        }
        9 if elapsed >= 6 => {
            let loader = state.controls[1];
            assert_eq!(gui.animation(loader).unwrap().frame_count(), 0);
            assert!(players.get(loader).is_err());
            assert!(existing.contains(loader));
            commands.write_message(GuiObjectCommand::new(
                loader,
                GuiObjectAction::SetIcon("ui://9leh0eyfhixt1v".into()),
            ));
            commands.write_message(GuiObjectCommand::new(
                loader,
                GuiObjectAction::SetAnimationPlaySettings(GuiAnimationPlaySettings {
                    start: 2,
                    ..default()
                }),
            ));
            next = true;
        }
        10 if elapsed >= 6 => {
            let loader = state.controls[1];
            assert_eq!(gui.animation(loader).unwrap().frame(), 2);
            assert!(!gui.animation(loader).unwrap().is_playing());
            assert!(!gui.animation(loader).unwrap().is_ended());
            assert_eq!(results.completed.len(), 2);
            commands.entity(state.page.unwrap()).despawn();
            next = true;
        }
        11 if elapsed >= 8 => {
            assert!(
                state
                    .controls
                    .iter()
                    .all(|&entity| !existing.contains(entity))
            );
            assert_eq!(results.completed.len(), 2);
            println!(
                "demo smoke PASSED: ECS animation button observers, pause/frame, finite direct and Loader playback, entity completion with final snapshots, callback commands, rendered terminal frame, retained Loader source replacement and disposal"
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

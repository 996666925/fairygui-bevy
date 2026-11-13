//! Progress target/display state and tween completion in a rendered window.
use super::*;

#[derive(Resource, Default)]
struct State {
    step: u8,
    frame: u32,
    since: u32,
    owners: Vec<Entity>,
    displayed: Vec<f64>,
}
#[derive(Resource, Default)]
struct Events(Vec<Entity>);
fn completed(event: On<GuiProgressTweenComplete>, gui: GuiQuery, mut events: ResMut<Events>) {
    let progress = gui.progress(event.entity).unwrap();
    assert_eq!(progress.value(), event.value);
    assert_eq!(progress.displayed_value(), event.value);
    assert!(!progress.is_tweening());
    events.0.push(event.entity);
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<State>()
        .init_resource::<Events>()
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(100),
        ))
        .add_systems(PreUpdate, drive);
}
#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    demo: Res<Demo>,
    gui: GuiQuery,
    events: Res<Events>,
    assets: Res<AssetServer>,
    nodes: Query<&ComputedNode>,
    existing: Query<()>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
    mut gui_edit: GuiMut,
) {
    state.frame += 1;
    assert!(
        state.frame < 1200,
        "progress smoke timed out at step {}",
        state.step
    );
    let elapsed = state.frame - state.since;
    let mut next = false;
    match state.step {
        0 if state.frame >= 30 => {
            let Some(menu) = demo.menu_owner.filter(|&owner| gui.view(owner).is_some()) else {
                return;
            };
            gui_edit.edit(menu, |object: &mut GuiObjectMut| object.set_visible(false));
            let horizontal = commands
                .spawn(
                    FairyGuiRoot::new(assets.load("ui/Basics.fui"), "ProgressBar")
                        .with_position(Vec2::new(100., 100.))
                        .with_size(Vec2::new(500., 24.)),
                )
                .id();
            let radial = commands
                .spawn(
                    FairyGuiRoot::new(assets.load("ui/Cooldown.fui"), "Button1")
                        .with_position(Vec2::new(750., 100.)),
                )
                .id();

            state.owners = vec![horizontal, radial];
            for &owner in &state.owners {
                commands
                    .entity(owner)
                    .insert(GuiProgressState::new(0., 1., 0.25));
            }
            next = true;
        }
        1 if elapsed >= 16
            && state
                .owners
                .iter()
                .all(|&owner| gui.progress(owner).is_some()) =>
        {
            gui_edit
                .get_mut(state.owners[1])
                .unwrap()
                .set_scale(Vec2::splat(3.));
            for &owner in &state.owners {
                let progress = gui.progress(owner).unwrap();
                assert_eq!((progress.value(), progress.displayed_value()), (0.25, 0.25));
                commands.entity(gui.view(owner).unwrap()).observe(completed);
                gui_edit.edit(owner, |object: &mut GuiObjectMut| {
                    object.tween_progress(0.75, 4.)
                });
            }
            next = true;
        }
        2 if gui.progress(state.owners[0]).unwrap().displayed_value() >= 0.4 => {
            for &owner in &state.owners {
                let progress = gui.progress(owner).unwrap();
                assert_eq!(progress.value(), 0.75);
                assert!(progress.is_tweening() && progress.displayed_value() < 0.7);
                gui_edit.edit(owner, |object: &mut GuiObjectMut| {
                    object.stop_progress_tween()
                });
            }
            assert!(events.0.is_empty());
            next = true;
        }
        3 if elapsed >= 3 => {
            for &owner in &state.owners {
                let progress = gui.progress(owner).unwrap();
                assert!(!progress.is_tweening());
                let bar = gui.child_by_name(owner, "bar").unwrap();
                let geometry = gui.transform(bar).unwrap();
                if owner == state.owners[0] {
                    let expected = (495. * progress.percent()).round() as f32;
                    assert_eq!(geometry.size().x, expected);
                    let node = nodes.get(bar).unwrap();
                    let rendered = node.size().x * node.inverse_scale_factor();
                    // Bevy rounds the two physical pixel edges independently.
                    assert!(
                        (rendered - expected).abs() <= node.inverse_scale_factor(),
                        "progress width: model={expected}, rendered={rendered}, physical pixel={}",
                        node.inverse_scale_factor()
                    );
                } else {
                    assert_eq!(
                        geometry.size(),
                        Vec2::splat(54.),
                        "radial fill preserves image dimensions"
                    );
                }
            }
            state.displayed = state
                .owners
                .iter()
                .map(|&owner| gui.progress(owner).unwrap().displayed_value())
                .collect();
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-progress-tween.png"));
            next = true;
        }
        4 if elapsed >= 8 => {
            for (&owner, &displayed) in state.owners.iter().zip(&state.displayed) {
                assert_eq!(gui.progress(owner).unwrap().displayed_value(), displayed);
                gui_edit.edit(owner, |object: &mut GuiObjectMut| {
                    object.tween_progress(0.25, 0.2)
                });
            }
            next = true;
        }
        5 if elapsed >= 6 && events.0.len() == 2 => {
            for &owner in &state.owners {
                assert!(events.0.contains(&gui.view(owner).unwrap()));
                assert_eq!(gui.progress(owner).unwrap().displayed_value(), 0.25);
                commands.entity(owner).despawn();
            }
            next = true;
        }
        6 if elapsed >= 4 => {
            assert!(state.owners.iter().all(|&owner| !existing.contains(owner)));
            println!(
                "demo smoke PASSED: ECS fractional progress targets, animated display, rendered bar dimensions, radial fill, frozen display, completion observers and disposal"
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
}

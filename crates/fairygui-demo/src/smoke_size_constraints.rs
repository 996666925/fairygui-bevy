//! Rendered center stability while clamping and restoring requested dimensions.
use super::*;
#[derive(Resource, Default)]
struct State {
    frame: u32,
    since: u32,
    step: u8,
    owners: Vec<Entity>,
    centers: Vec<Vec2>,
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<State>().add_systems(PreUpdate, drive);
}
#[allow(clippy::too_many_arguments)]
fn drive(
    mut state: ResMut<State>,
    demo: Res<Demo>,
    gui: GuiQuery,
    assets: Res<AssetServer>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    mut objects: GuiMut,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    assert!(
        state.frame < 1000,
        "size constraints timeout at {}",
        state.step
    );
    let elapsed = state.frame - state.since;
    let mut next = false;
    match state.step {
        0 if state.frame >= 30 => {
            let Some(menu) = demo.menu_owner.filter(|&owner| gui.view(owner).is_some()) else {
                return;
            };
            objects.get_mut(menu).unwrap().set_visible(false);
            for x in [160., 510., 860.] {
                state.owners.push(
                    commands
                        .spawn((
                            FairyGuiRoot::new(assets.load("ui/Basics.fui"), "Button1")
                                .with_position(Vec2::new(x, 240.))
                                .with_size(Vec2::new(160., 80.)),
                            GuiText::new("Size constraints"),
                        ))
                        .id(),
                );
            }
            next = true;
        }
        1 if elapsed >= 16 && state.owners.iter().all(|&owner| gui.view(owner).is_some()) => {
            for (index, &owner) in state.owners.iter().enumerate() {
                objects
                    .get_mut(owner)
                    .unwrap()
                    .set_pivot(Vec2::splat(0.5), index == 2);
                commands.entity(owner).insert(GuiSizeConstraints {
                    min: Vec2::new(80., 40.),
                    max: Vec2::new(220., 120.),
                });
            }
            next = true;
        }
        2 if elapsed >= 12 => {
            let owners = state.owners.clone();
            for owner in owners {
                let view = gui.view(owner).unwrap();
                let center = nodes.get(view).unwrap().1.transform_point2(Vec2::ZERO);
                state.centers.push(center);
                let mut t = *gui.transform(view).unwrap();
                t.size = Vec2::new(320., 200.);
                commands.entity(view).insert(t);
            }
            next = true;
        }
        3 if elapsed >= 12 => {
            check(&state, &gui, &nodes, Vec2::new(220., 120.));
            for &owner in &state.owners {
                objects.get_mut(owner).unwrap().set_width(180.);
            }
            next = true;
        }
        4 if elapsed >= 12 => {
            check(&state, &gui, &nodes, Vec2::new(180., 120.));
            for &owner in &state.owners {
                commands.entity(owner).insert(GuiSizeConstraints::default());
            }
            next = true;
        }
        5 if elapsed >= 12 => {
            check(&state, &gui, &nodes, Vec2::new(180., 200.));
            for &owner in &state.owners {
                commands.entity(owner).insert(GuiSizeConstraints {
                    min: Vec2::new(240., 260.),
                    max: Vec2::ZERO,
                });
            }
            next = true;
        }
        6 if elapsed >= 12 => {
            check(&state, &gui, &nodes, Vec2::new(240., 260.));
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-size-constraints.png"));
            next = true;
        }
        7 if elapsed >= 20 => {
            check(&state, &gui, &nodes, Vec2::new(240., 260.));
            eprintln!(
                "size constraints smoke PASSED: ECS min/max, retained requests, direct single-axis edits, restored dimensions, pivot and anchor center stability"
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
fn check(
    state: &State,
    gui: &GuiQuery,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    expected: Vec2,
) {
    for (&owner, &center) in state.owners.iter().zip(&state.centers) {
        let view = gui.view(owner).unwrap();
        let (node, transform) = nodes.get(view).unwrap();
        assert_eq!(gui.transform(owner).unwrap().size(), expected);
        assert!((node.size() * node.inverse_scale_factor()).distance(expected) <= 1.);
        assert!(
            transform.transform_point2(Vec2::ZERO).distance(center) <= 1.,
            "pivot shifted while resizing"
        );
    }
}

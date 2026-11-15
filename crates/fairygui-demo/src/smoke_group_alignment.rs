//! Rendered GGroup bounds and center relations after retained ECS edits.
use super::*;

#[derive(Resource, Default)]
struct State {
    frame: u32,
    since: u32,
    step: u8,
    panels: Vec<Entity>,
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
        state.frame < 1200,
        "group alignment timeout at step {}",
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
            for x in [70., 450., 830.] {
                state.panels.push(
                    commands
                        .spawn(
                            FairyGuiRoot::new(
                                assets.load("ui/ModalWaiting.fui"),
                                "WindowModalWaiting",
                            )
                            .with_position(Vec2::new(x, 150.))
                            .with_size(Vec2::new(300., 300.)),
                        )
                        .id(),
                );
            }
            next = true;
        }
        1 if elapsed >= 16 && state.panels.iter().all(|&owner| gui.view(owner).is_some()) => {
            for &owner in &state.panels {
                check(&gui, &nodes, owner, 90., 80.);
            }
            for (index, &owner) in state.panels.iter().enumerate() {
                let image = gui.child_by_name(owner, "n1").unwrap();
                objects.get_mut(image).unwrap().set_size(Vec2::new(
                    90. + index as f32 * 60.,
                    80. + index as f32 * 20.,
                ));
            }
            next = true;
        }
        2 if elapsed >= 12 => {
            for (index, &owner) in state.panels.iter().enumerate() {
                check(
                    &gui,
                    &nodes,
                    owner,
                    90. + index as f32 * 60.,
                    80. + index as f32 * 20.,
                );
                objects
                    .get_mut(owner)
                    .unwrap()
                    .set_size(Vec2::new(340., 360.));
            }
            next = true;
        }
        3 if elapsed >= 12 => {
            for (index, &owner) in state.panels.iter().enumerate() {
                check(
                    &gui,
                    &nodes,
                    owner,
                    90. + index as f32 * 60.,
                    80. + index as f32 * 20.,
                );
            }
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-group-alignment.png"));
            next = true;
        }
        4 if elapsed >= 20 => {
            for (index, &owner) in state.panels.iter().enumerate() {
                check(
                    &gui,
                    &nodes,
                    owner,
                    90. + index as f32 * 60.,
                    80. + index as f32 * 20.,
                );
            }
            info!(
                "demo smoke PASSED: rendered group centering, member bounds changes, parent resizing and stable retained geometry"
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
    gui: &GuiQuery,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    owner: Entity,
    width: f32,
    height: f32,
) {
    let root = gui.view(owner).unwrap();
    let group = gui.child_by_name(owner, "n4").unwrap();
    let image = gui.child_by_name(owner, "n1").unwrap();
    let movie = gui.child_by_name(owner, "n3").unwrap();
    let size = gui.transform(owner).unwrap().size();
    let expected = (size - Vec2::new(width, height)) * 0.5;
    assert_eq!(
        gui.transform(group).unwrap().size(),
        Vec2::new(width, height)
    );
    assert_eq!(gui.transform(group).unwrap().position(), expected);
    assert_eq!(gui.transform(image).unwrap().position(), expected);
    assert_eq!(
        gui.transform(movie).unwrap().position(),
        expected + Vec2::new(22., 26.)
    );
    let center = |entity| nodes.get(entity).unwrap().1.transform_point2(Vec2::ZERO);
    // UI rectangles round each physical axis at fractional window DPI.
    assert!(
        center(image).distance(center(root)) <= 1.,
        "rendered image must stay centered: size={width}x{height}, root={:?}, group={:?}, image={:?}, root node={:?}, image node={:?}, image gui={:?}",
        center(root),
        center(group),
        center(image),
        nodes.get(root).unwrap().0.size(),
        nodes.get(image).unwrap().0.size(),
        gui.transform(image).unwrap()
    );
    assert!(
        center(group).distance(center(root)) <= 1.,
        "rendered group rectangle must stay centered"
    );
    let (node, _) = nodes.get(image).unwrap();
    assert!((node.size().x * node.inverse_scale_factor() - width).abs() < 0.5);
}

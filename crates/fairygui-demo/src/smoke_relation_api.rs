//! Public entity relation editing against real Bevy layout and exported children.
use super::*;

#[derive(Resource, Default)]
struct State {
    frame: u32,
    since: u32,
    step: u8,
    owners: Vec<Entity>,
    initial_center: Vec2,
    current_center: Vec2,
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
        "runtime relation API timeout at {}",
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
            for (position, title) in [
                (Vec2::new(100., 180.), "Target"),
                (Vec2::new(500., 180.), "Related object"),
            ] {
                state.owners.push(
                    commands
                        .spawn((
                            FairyGuiRoot::new(assets.load("ui/Basics.fui"), "Button1")
                                .with_position(position)
                                .with_size(Vec2::new(160., 80.)),
                            GuiText::new(title),
                        ))
                        .id(),
                );
            }
            next = true;
        }
        1 if elapsed >= 16 && state.owners.iter().all(|&owner| gui.view(owner).is_some()) => {
            let [target, owner] = state.owners[..] else {
                unreachable!()
            };
            state.initial_center = nodes
                .get(gui.view(owner).unwrap())
                .unwrap()
                .1
                .transform_point2(Vec2::ZERO);
            for kind in [
                RelationType::LeftLeft,
                RelationType::TopTop,
                RelationType::Size,
            ] {
                assert!(objects.add_relation(owner, target, kind, false));
            }
            assert!(!objects.add_relation(
                gui.view(owner).unwrap(),
                target,
                RelationType::Size,
                true
            ));
            assert!(objects.has_relation(owner, gui.view(target).unwrap(), RelationType::Size));
            objects
                .get_mut(target)
                .unwrap()
                .set_position(Vec2::new(130., 210.));
            objects
                .get_mut(target)
                .unwrap()
                .set_size(Vec2::new(220., 100.));
            next = true;
        }
        2 if elapsed >= 12 => {
            check(
                &state,
                &gui,
                &nodes,
                Vec2::new(530., 210.),
                Vec2::new(220., 100.),
                Vec2::new(60., 40.),
            );
            let [target, owner] = state.owners[..] else {
                unreachable!()
            };
            assert!(objects.remove_relation(owner, target, RelationType::Width));
            assert!(!objects.has_relation(owner, target, RelationType::Size));
            assert!(objects.has_relation(owner, target, RelationType::Height));
            objects
                .get_mut(target)
                .unwrap()
                .set_position(Vec2::new(140., 215.));
            objects
                .get_mut(target)
                .unwrap()
                .set_size(Vec2::new(260., 140.));
            next = true;
        }
        3 if elapsed >= 12 => {
            check(
                &state,
                &gui,
                &nodes,
                Vec2::new(540., 215.),
                Vec2::new(220., 140.),
                Vec2::new(70., 65.),
            );
            let [target, owner] = state.owners[..] else {
                unreachable!()
            };
            assert!(objects.clear_relations_to(owner, target));
            objects
                .get_mut(target)
                .unwrap()
                .set_position(Vec2::new(180., 250.));
            objects
                .get_mut(target)
                .unwrap()
                .set_size(Vec2::new(300., 180.));
            next = true;
        }
        4 if elapsed >= 12 => {
            check(
                &state,
                &gui,
                &nodes,
                Vec2::new(540., 215.),
                Vec2::new(220., 140.),
                Vec2::new(70., 65.),
            );
            let [target, owner] = state.owners[..] else {
                unreachable!()
            };
            assert!(objects.add_relation(owner, target, RelationType::Size, false));
            objects
                .get_mut(target)
                .unwrap()
                .set_size(Vec2::new(340., 200.));
            next = true;
        }
        5 if elapsed >= 12 => {
            check(
                &state,
                &gui,
                &nodes,
                Vec2::new(540., 215.),
                Vec2::new(260., 160.),
                Vec2::new(90., 75.),
            );
            let [target, owner] = state.owners[..] else {
                unreachable!()
            };
            state.current_center = nodes
                .get(gui.view(owner).unwrap())
                .unwrap()
                .1
                .transform_point2(Vec2::ZERO);
            commands.entity(target).despawn();
            next = true;
        }
        6 if elapsed >= 12 => {
            let owner = state.owners[1];
            assert!(objects.clear_relations(owner));
            assert!(!objects.clear_relations(owner));
            let center = nodes
                .get(gui.view(owner).unwrap())
                .unwrap()
                .1
                .transform_point2(Vec2::ZERO);
            assert!(center.distance(state.current_center) <= 1.);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-relation-api.png"));
            next = true;
        }
        7 if elapsed >= 20 => {
            let owner = state.owners[1];
            let title = gui.child_by_name(owner, "title").unwrap();
            assert!(objects.clear_relations(title));
            assert!(objects.add_relation(owner, title, RelationType::RightExtRight, false));
            objects
                .get_mut(owner)
                .unwrap()
                .set_pivot(Vec2::splat(0.5), false);
            objects.get_mut(owner).unwrap().set_width(400.);
            objects.get_mut(title).unwrap().set_width(300.);
            commands
                .entity(gui.view(owner).unwrap())
                .insert(GuiSizeConstraints {
                    min: Vec2::new(70., 50.),
                    max: Vec2::new(240., 190.),
                });
            next = true;
        }
        8 if elapsed >= 12 => {
            check_child_extension(&state, &gui, &nodes, 240.);
            let title = gui.child_by_name(state.owners[1], "title").unwrap();
            objects.get_mut(title).unwrap().set_width(30.);
            next = true;
        }
        9 if elapsed >= 12 => {
            check_child_extension(&state, &gui, &nodes, 70.);
            let title = gui.child_by_name(state.owners[1], "title").unwrap();
            objects.get_mut(title).unwrap().set_width(180.);
            next = true;
        }
        10 if elapsed >= 12 => {
            check_child_extension(&state, &gui, &nodes, 180.);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-child-relations.png"));
            next = true;
        }
        11 if elapsed >= 20 => {
            info!(
                "demo smoke PASSED: typed entity relation add/remove/clear, root/view aliases, rendered geometry, target disposal, child extension design sizes and clamped pivot stability"
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

fn check_child_extension(
    state: &State,
    gui: &GuiQuery,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    width: f32,
) {
    let owner = state.owners[1];
    let root = gui.view(owner).unwrap();
    assert_eq!(gui.transform(root).unwrap().size(), Vec2::new(width, 160.));
    assert_eq!(
        gui.transform(root).unwrap().position(),
        Vec2::new(670. - width * 0.5, 215.)
    );
    let (node, transform) = nodes.get(root).unwrap();
    let center = transform.transform_point2(Vec2::ZERO);
    assert!(center.distance(state.current_center) <= 1.);
    assert!((node.size().x * node.inverse_scale_factor() - width).abs() < 0.5);
    for name in ["n1", "n2", "n3"] {
        let child = gui.child_by_name(owner, name).unwrap();
        assert_eq!(gui.transform(child).unwrap().size().x, width);
        let actual = nodes.get(child).unwrap().1.transform_point2(Vec2::ZERO);
        assert!(actual.distance(center) <= 1., "background {name}");
    }
}

fn check(
    state: &State,
    gui: &GuiQuery,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    position: Vec2,
    size: Vec2,
    delta: Vec2,
) {
    let owner = state.owners[1];
    let view = gui.view(owner).unwrap();
    let transform = gui.transform(owner).unwrap();
    assert_eq!(transform.position(), position);
    assert_eq!(transform.size(), size);
    let (node, render) = nodes.get(view).unwrap();
    let center = render.transform_point2(Vec2::ZERO);
    let expected = state.initial_center + delta / node.inverse_scale_factor();
    assert!(
        center.distance(expected) <= 1.,
        "render={center:?}, expected={expected:?}"
    );
    for name in ["n1", "n2", "n3", "title"] {
        let child = gui.child_by_name(owner, name).unwrap();
        assert_eq!(gui.transform(child).unwrap().size(), size);
        let actual = nodes.get(child).unwrap().1.transform_point2(Vec2::ZERO);
        assert!(actual.distance(center) <= 1., "exported child {name}");
    }
}

//! Rendered GGroup bounds and center relations after retained ECS edits.
use super::*;

#[derive(Resource, Default)]
struct State {
    frame: u32,
    since: u32,
    step: u8,
    panels: Vec<Entity>,
    buttons: Vec<Entity>,
    member_sizes: Vec<(Vec2, Vec2)>,
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
            let panels = state.panels.clone();
            for (index, owner) in panels.into_iter().enumerate() {
                let group = gui.child_by_name(owner, "n4").unwrap();
                let image = gui.child_by_name(owner, "n1").unwrap();
                let movie = gui.child_by_name(owner, "n3").unwrap();
                state.member_sizes.push((
                    gui.transform(image).unwrap().size(),
                    gui.transform(movie).unwrap().size(),
                ));
                let mut transform = *gui.transform(group).unwrap();
                transform.size = Vec2::new(260., 120.);
                commands.entity(group).insert((
                    transform,
                    GuiGroupLayout {
                        layout: if index == 2 {
                            GroupLayoutType::Vertical
                        } else {
                            GroupLayoutType::Horizontal
                        },
                        auto_size_disabled: true,
                        main_grid_index: match index {
                            1 => Some(1),
                            2 => Some(0),
                            _ => None,
                        },
                        column_gap: 7,
                        line_gap: 9,
                        ..default()
                    },
                ));
            }
            next = true;
        }
        5 if elapsed >= 12 => {
            for (index, &owner) in state.panels.iter().enumerate() {
                check_fixed(
                    &gui,
                    &nodes,
                    owner,
                    index,
                    state.member_sizes[index],
                    Vec2::new(260., 120.),
                );
                let group = gui.child_by_name(owner, "n4").unwrap();
                let mut transform = *gui.transform(group).unwrap();
                transform.size = Vec2::new(300., 160.);
                commands.entity(group).insert(transform);
            }
            next = true;
        }
        6 if elapsed >= 12 => {
            for (index, &owner) in state.panels.iter().enumerate() {
                check_fixed(
                    &gui,
                    &nodes,
                    owner,
                    index,
                    state.member_sizes[index],
                    Vec2::new(300., 160.),
                );
            }
            let owner = state.panels[1];
            let group = gui.child_by_name(owner, "n4").unwrap();
            let movie = gui.child_by_name(owner, "n3").unwrap();
            let mut layout = *gui.group_layout(group).unwrap();
            layout.exclude_invisibles = true;
            let mut display = *gui.display(movie).unwrap();
            display.visible = false;
            commands.entity(group).insert(layout);
            commands.entity(movie).insert(display);
            next = true;
        }
        7 if elapsed >= 12 => {
            let owner = state.panels[1];
            let image = gui.child_by_name(owner, "n1").unwrap();
            let movie = gui.child_by_name(owner, "n3").unwrap();
            assert_eq!(gui.transform(image).unwrap().size().x, 300.);
            assert!(!gui.display(movie).unwrap().display_visible());
            let mut display = *gui.display(movie).unwrap();
            display.visible = true;
            commands.entity(movie).insert(display);
            next = true;
        }
        8 if elapsed >= 12 => {
            let owner = state.panels[1];
            let image = gui.child_by_name(owner, "n1").unwrap();
            let movie = gui.child_by_name(owner, "n3").unwrap();
            assert_eq!(gui.transform(image).unwrap().size().x, 251.);
            assert_eq!(gui.transform(movie).unwrap().size().x, 42.);
            assert!(gui.display(movie).unwrap().display_visible());
            for &owner in &state.panels {
                check_projection(&gui, &nodes, owner);
            }
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-group-resize.png"));
            next = true;
        }
        9 if elapsed >= 20 => {
            for (index, pivot) in [0., 0.5, 1.].into_iter().enumerate() {
                state.buttons.push(
                    commands
                        .spawn((
                            FairyGuiRoot::new(assets.load("ui/Basics.fui"), "Button1")
                                .with_position(Vec2::new(70. + index as f32 * 380., 530.))
                                .with_size(Vec2::new(160., 60.)),
                            GuiText::new(format!("Group pivot {pivot}")),
                        ))
                        .id(),
                );
            }
            next = true;
        }
        10 if elapsed >= 16 && state.buttons.iter().all(|&owner| gui.view(owner).is_some()) => {
            for (index, &owner) in state.buttons.iter().enumerate() {
                // Use the actual exported parent-size relations on the button
                // backgrounds/title, then measure their common group bounds.
                commands.queue(move |world: &mut World| {
                    let mut runtime = world.non_send_mut::<GuiRuntime>();
                    let root = runtime.ecs_root_object(owner).unwrap();
                    let store = runtime.store_mut();
                    let members = store.object(root).unwrap().children().to_vec();
                    let group = store.create(fairygui_core::package::ObjectType::Group);
                    store.set_name(group, "pivot_group");
                    store.set_parent(group, Some(root));
                    for member in members {
                        store.set_group(member, Some(group));
                        store.set_pivot(member, index as f32 * 0.5, index as f32 * 0.5, false);
                    }
                });
            }
            next = true;
        }
        11 if elapsed >= 12 => {
            for &owner in &state.buttons {
                let mut transform = *gui.transform(owner).unwrap();
                transform.size = Vec2::new(280., 100.);
                commands.entity(owner).insert(transform);
            }
            next = true;
        }
        12 if elapsed >= 12 => {
            check_pivot_groups(&state, &gui, &nodes, Vec2::new(280., 100.));
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-group-pivot-alignment.png",
                ));
            next = true;
        }
        13 if elapsed >= 20 => {
            for &owner in &state.buttons {
                objects
                    .get_mut(owner)
                    .unwrap()
                    .set_size(Vec2::new(160., 60.));
            }
            next = true;
        }
        14 if elapsed >= 12 => {
            check_pivot_groups(&state, &gui, &nodes, Vec2::new(160., 60.));
            for &owner in &state.panels {
                let group = gui.child_by_name(owner, "n4").unwrap();
                let mut transform = *gui.transform(group).unwrap();
                // Resizing moves the centered X through its self relation.
                // An application Y edit must retain that updated X.
                transform.size.x += 40.;
                transform.position.y += 13.;
                commands.entity(group).insert(transform);
            }
            next = true;
        }
        15 if elapsed >= 12 => {
            check_axis_edits(
                &state,
                &gui,
                &nodes,
                Vec2::new(340., 160.),
                Vec2::new(0., 13.),
            );
            for &owner in &state.panels {
                let group = gui.child_by_name(owner, "n4").unwrap();
                let mut transform = *gui.transform(group).unwrap();
                transform.size.y += 40.;
                transform.position.x += 17.;
                commands.entity(group).insert(transform);
            }
            next = true;
        }
        16 if elapsed >= 12 => {
            check_axis_edits(
                &state,
                &gui,
                &nodes,
                Vec2::new(340., 200.),
                Vec2::new(17., 13.),
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    "target/fairygui-demo-group-axis-alignment.png",
                ));
            next = true;
        }
        17 if elapsed >= 20 => {
            check_axis_edits(
                &state,
                &gui,
                &nodes,
                Vec2::new(340., 200.),
                Vec2::new(17., 13.),
            );
            info!(
                "demo smoke PASSED: rendered group alignment, ECS fixed layouts, proportional/main-grid resizing, hidden members, pivoted size relations, simultaneous size/position axis edits and retained geometry"
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

fn check_axis_edits(
    state: &State,
    gui: &GuiQuery,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    size: Vec2,
    offset: Vec2,
) {
    for &owner in &state.panels {
        let group = gui.child_by_name(owner, "n4").unwrap();
        let transform = gui.transform(group).unwrap();
        assert_eq!(transform.size(), size);
        assert_eq!(
            transform.position(),
            (gui.transform(owner).unwrap().size() - size) * 0.5 + offset
        );
        check_projection(gui, nodes, owner);
    }
}

fn check_pivot_groups(
    state: &State,
    gui: &GuiQuery,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    size: Vec2,
) {
    for &owner in &state.buttons {
        let root = gui.view(owner).unwrap();
        let center = nodes.get(root).unwrap().1.transform_point2(Vec2::ZERO);
        for name in ["pivot_group", "n1", "n2", "n3", "title"] {
            let member = gui.child_by_name(owner, name).unwrap();
            let transform = gui.transform(member).unwrap();
            assert_eq!(transform.position(), Vec2::ZERO, "{name}");
            assert_eq!(transform.size(), size, "{name}");
            let actual = nodes.get(member).unwrap().1.transform_point2(Vec2::ZERO);
            assert!(
                actual.distance(center) <= 1.,
                "{name}: render={actual:?}, expected={center:?}"
            );
        }
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

fn check_fixed(
    gui: &GuiQuery,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    owner: Entity,
    index: usize,
    original: (Vec2, Vec2),
    expected_size: Vec2,
) {
    let group = gui.child_by_name(owner, "n4").unwrap();
    let image = gui.child_by_name(owner, "n1").unwrap();
    let movie = gui.child_by_name(owner, "n3").unwrap();
    let image_size = gui.transform(image).unwrap().size();
    let movie_size = gui.transform(movie).unwrap().size();
    let group_position = gui.transform(group).unwrap().position();
    assert_eq!(gui.transform(group).unwrap().size(), expected_size);
    assert_eq!(
        group_position,
        (gui.transform(owner).unwrap().size() - expected_size) * 0.5
    );
    if index == 2 {
        assert_eq!(image_size.y + movie_size.y + 9., expected_size.y);
        assert_eq!(movie_size.y, original.1.y);
        assert_eq!(gui.transform(image).unwrap().position().y, group_position.y);
        assert_eq!(
            gui.transform(movie).unwrap().position().y,
            group_position.y + image_size.y + 9.
        );
    } else {
        assert_eq!(image_size.x + movie_size.x + 7., expected_size.x);
        if index == 1 {
            assert_eq!(image_size.x, original.0.x);
        } else {
            assert!(
                (image_size.x
                    - (expected_size.x - 7.) * original.0.x / (original.0.x + original.1.x))
                    .abs()
                    <= 1.
            );
        }
        assert_eq!(gui.transform(image).unwrap().position().x, group_position.x);
        assert_eq!(
            gui.transform(movie).unwrap().position().x,
            group_position.x + image_size.x + 7.
        );
    }
    check_projection(gui, nodes, owner);
}

fn check_projection(
    gui: &GuiQuery,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    owner: Entity,
) {
    let root = gui.view(owner).unwrap();
    let (root_node, root_transform) = nodes.get(root).unwrap();
    let root_size = gui.transform(root).unwrap().size();
    for name in ["n4", "n1", "n3"] {
        let child = gui.child_by_name(owner, name).unwrap();
        let transform = gui.transform(child).unwrap();
        let center = (transform.position() + transform.size() * 0.5 - root_size * 0.5)
            / root_node.inverse_scale_factor();
        let expected = root_transform.transform_point2(center);
        let actual = nodes.get(child).unwrap().1.transform_point2(Vec2::ZERO);
        assert!(
            actual.distance(expected) <= 1.,
            "group member {name}: render={actual:?}, expected={expected:?}"
        );
    }
}

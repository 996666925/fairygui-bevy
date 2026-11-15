//! Exported parent relations retain their object identity across hierarchy edits.
use super::*;
#[derive(Resource, Default)]
struct State {
    frame: u32,
    since: u32,
    step: u8,
    roots: Vec<Entity>,
    title: Option<Entity>,
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
    mut runtime: NonSendMut<GuiRuntime>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    references: Query<&GObjectRef>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    assert!(
        state.frame < 1200,
        "relation target timeout at {}",
        state.step
    );
    let elapsed = state.frame - state.since;
    let mut next = false;
    match state.step {
        0 if state.frame >= 30 => {
            let Some(menu) = demo.menu_owner.and_then(|owner| gui.view(owner)) else {
                return;
            };
            let mut display = *gui.display(menu).unwrap();
            display.visible = false;
            commands.entity(menu).insert(display);
            for x in [100., 650.] {
                state.roots.push(
                    commands
                        .spawn((
                            FairyGuiRoot::new(assets.load("ui/Basics.fui"), "Button1")
                                .with_position(Vec2::new(x, 180.))
                                .with_size(Vec2::new(180., 100.)),
                            GuiText::new("Retained target"),
                        ))
                        .id(),
                );
            }
            next = true;
        }
        1 if elapsed >= 16 && state.roots.iter().all(|&id| gui.view(id).is_some()) => {
            let title = gui.child_by_name(state.roots[0], "title").unwrap();
            let id = references.get(title).unwrap().0;
            runtime.store_mut().set_child_index(id, 0);
            state.title = Some(title);
            next = true;
        }
        2 if elapsed >= 12 => {
            let id = references.get(state.title.unwrap()).unwrap().0;
            let target = references.get(gui.view(state.roots[1]).unwrap()).unwrap().0;
            runtime.store_mut().set_parent(id, Some(target));
            next = true;
        }
        3 if elapsed >= 12 => {
            for (index, &owner) in state.roots.iter().enumerate() {
                let view = gui.view(owner).unwrap();
                let mut t = *gui.transform(view).unwrap();
                t.size = if index == 0 {
                    Vec2::new(300., 140.)
                } else {
                    Vec2::new(400., 240.)
                };
                commands.entity(view).insert(t);
            }
            next = true;
        }
        4 if elapsed >= 12 => {
            check(
                &gui,
                &nodes,
                state.title.unwrap(),
                state.roots[1],
                Vec2::new(300., 140.),
            );
            let title_id = references.get(state.title.unwrap()).unwrap().0;
            let source = references.get(gui.view(state.roots[0]).unwrap()).unwrap().0;
            assert_ne!(
                runtime.store().object(title_id).unwrap().parent(),
                Some(source)
            );
            commands.entity(state.roots[0]).despawn();
            next = true;
        }
        5 if elapsed >= 12 => {
            let view = gui.view(state.roots[1]).unwrap();
            let mut t = *gui.transform(view).unwrap();
            t.size = Vec2::new(450., 280.);
            commands.entity(view).insert(t);
            next = true;
        }
        6 if elapsed >= 12 => {
            check(
                &gui,
                &nodes,
                state.title.unwrap(),
                state.roots[1],
                Vec2::new(300., 140.),
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk("target/fairygui-demo-relation-targets.png"));
            next = true;
        }
        7 if elapsed >= 20 => {
            check(
                &gui,
                &nodes,
                state.title.unwrap(),
                state.roots[1],
                Vec2::new(300., 140.),
            );
            eprintln!(
                "relation target smoke PASSED: exported bindings, reorder, retained reparenting, original target resize, target disposal and stable rendered geometry"
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
    title: Entity,
    parent: Entity,
    size: Vec2,
) {
    assert_eq!(gui.transform(title).unwrap().size(), size);
    let (node, global) = nodes.get(title).unwrap();
    assert!((node.size() * node.inverse_scale_factor()).distance(size) <= 1.);
    let root = gui.view(parent).unwrap();
    let (root_node, root_global) = nodes.get(root).unwrap();
    let local = gui.transform(title).unwrap();
    let root_size = gui.transform(root).unwrap().size();
    let expected = root_global.transform_point2(
        (local.position() + size * 0.5 - root_size * 0.5) / root_node.inverse_scale_factor(),
    );
    assert!(global.transform_point2(Vec2::ZERO).distance(expected) <= 1.);
}

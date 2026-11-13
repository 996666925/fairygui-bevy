//! ECS graph edits, expanded native circles and real contour pass-through clicks.
use super::*;

#[derive(Resource, Default)]
struct State {
    frame: u32,
    since: u32,
    step: u8,
    owner: Option<Entity>,
    front: Option<Entity>,
    back: Option<Entity>,
    drawing: Option<Entity>,
    child: Option<Entity>,
    cursor: Option<Vec2>,
    click: Option<Click>,
}
struct Click {
    stage: u8,
    since: u32,
    count: usize,
    expected: Entity,
}
#[derive(Resource, Default)]
struct Clicks(Vec<Entity>);
fn clicked(event: On<GuiEvent>, mut clicks: ResMut<Clicks>) {
    if event.event.kind == fairygui_core::object::EventKind::Click && !event.event.bubbling {
        clicks.0.push(event.entity);
    }
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<State>()
        .init_resource::<Clicks>()
        .add_systems(
            PreUpdate,
            drive
                .after(InputSystems)
                .before(UiSystems::Focus)
                .before(PickingSystems::ProcessInput),
        );
}
fn drawing(front: Entity, faces: &Query<(Entity, &ChildOf, &GObjectRef, &Node)>) -> Option<Entity> {
    faces
        .iter()
        .find_map(|(entity, parent, _, _)| (parent.parent() == front).then_some(entity))
}
fn click_at(
    state: &mut State,
    clicks: &Clicks,
    source: Entity,
    local: Vec2,
    expected: Entity,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    scale: f32,
) {
    let (node, transform) = nodes.get(source).unwrap();
    let physical =
        transform.transform_point2(node.border_box().min + local / node.inverse_scale_factor());
    state.cursor = Some(physical / scale);
    state.click = Some(Click {
        stage: 0,
        since: state.frame,
        count: clicks.0.len() + 1,
        expected,
    });
}
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn drive(
    mut state: ResMut<State>,
    clicks: Res<Clicks>,
    demo: Res<Demo>,
    assets: Res<AssetServer>,
    mut ui: ParamSet<(
        GuiQuery,
        Query<&mut GuiGraph>,
        Query<&mut GuiTransform>,
        Query<&mut GuiDisplay>,
        Query<&mut GuiSortingOrder>,
        Query<&mut GuiColor>,
    )>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    faces: Query<(Entity, &ChildOf, &GObjectRef, &Node)>,
    backgrounds: Query<&BackgroundColor>,
    entities: Query<()>,
    mut windows: Query<(Entity, &mut Window), With<bevy::window::PrimaryWindow>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut pointers: MessageWriter<PointerInput>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    state.frame += 1;
    assert!(
        state.frame < 1600,
        "graph-properties smoke timed out at {}",
        state.step
    );
    let Ok((window_entity, mut window)) = windows.single_mut() else {
        return;
    };
    window.focused = true;
    let elapsed = state.frame - state.since;
    let mut next = false;
    if let Some(mut click) = state.click.take() {
        let elapsed = state.frame - click.since;
        match click.stage {
            0 if elapsed >= 2 => {
                mouse.press(MouseButton::Left);
                click.stage = 1;
                click.since = state.frame;
            }
            1 if elapsed >= 2 => {
                mouse.release(MouseButton::Left);
                click.stage = 2;
                click.since = state.frame;
            }
            2 if elapsed >= 5 => {
                assert_eq!(
                    clicks.0.len(),
                    click.count,
                    "click count at step {}",
                    state.step
                );
                assert_eq!(
                    clicks.0.last(),
                    Some(&click.expected),
                    "click target at step {}",
                    state.step
                );
                next = true;
            }
            _ => {}
        }
        if !next {
            state.click = Some(click);
        }
    } else {
        match state.step {
            0 if state.frame >= 30 => {
                let Some(menu) = demo.menu_owner.and_then(|owner| ui.p0().view(owner)) else {
                    return;
                };
                ui.p3().get_mut(menu).unwrap().visible = false;
                state.owner = Some(
                    commands
                        .spawn(
                            FairyGuiRoot::new(assets.load("ui/Basics.fui"), "Demo_Graph")
                                .with_position(Vec2::new(30., 50.)),
                        )
                        .id(),
                );
                next = true;
            }
            1 if elapsed >= 16 && ui.p0().view(state.owner.unwrap()).is_some() => {
                let (front, back, children) = {
                    let gui = ui.p0();
                    let owner = state.owner.unwrap();
                    (
                        gui.child_by_name(owner, "n10").unwrap(),
                        gui.child_by_name(owner, "n12").unwrap(),
                        gui.get(owner).unwrap().children().to_vec(),
                    )
                };
                for child in children {
                    ui.p3().get_mut(child).unwrap().visible = child == front || child == back;
                }
                for (entity, position, size) in [
                    (front, Vec2::new(100., 120.), Vec2::new(200., 80.)),
                    (back, Vec2::new(100., 60.), Vec2::splat(200.)),
                ] {
                    let mut transforms = ui.p2();
                    let mut transform = transforms.get_mut(entity).unwrap();
                    transform.position = position;
                    transform.size = size;
                }
                *ui.p1().get_mut(back).unwrap() = GuiGraph {
                    kind: GraphType::Rect,
                    fill_color: Color::srgb(0.35, 0.35, 0.35),
                    ..default()
                };
                *ui.p1().get_mut(front).unwrap() = GuiGraph {
                    kind: GraphType::Ellipse,
                    line_size: 3,
                    line_color: Color::WHITE,
                    fill_color: Color::srgb(0.1, 0.6, 1.),
                    ..default()
                };
                ui.p4().get_mut(front).unwrap().0 = 1;
                commands.entity(front).observe(clicked);
                commands.entity(back).observe(clicked);
                state.front = Some(front);
                state.back = Some(back);
                state.child = Some(commands.spawn((Node::default(), ChildOf(front))).id());
                next = true;
            }
            2 if elapsed >= 12 => {
                let front = state.front.unwrap();
                assert_eq!(
                    ui.p0().transform(front).unwrap().size(),
                    Vec2::new(200., 80.)
                );
                let surface = drawing(front, &faces).unwrap();
                let node = faces.get(surface).unwrap().3;
                assert_eq!(
                    (node.top, node.width, node.height),
                    (Val::Px(-60.), Val::Px(200.), Val::Px(200.))
                );
                assert!(
                    ui.p0().graph(surface).is_none(),
                    "decorative surface must not become a logical GUI view"
                );
                state.drawing = Some(surface);
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(
                        "target/fairygui-demo-graph-properties-circle.png",
                    ));
                next = true;
            }
            3 => {
                let (front, back) = (state.front.unwrap(), state.back.unwrap());
                click_at(
                    &mut state,
                    &clicks,
                    front,
                    Vec2::new(5., 5.),
                    back,
                    &nodes,
                    window.scale_factor(),
                );
            }
            4 => {
                let front = state.front.unwrap();
                click_at(
                    &mut state,
                    &clicks,
                    front,
                    Vec2::new(100., -40.),
                    front,
                    &nodes,
                    window.scale_factor(),
                );
            }
            5 => {
                let front = state.front.unwrap();
                {
                    let mut graphs = ui.p1();
                    let mut graph = graphs.get_mut(front).unwrap();
                    graph.kind = GraphType::Polygon;
                    graph.polygon_points = vec![
                        Vec2::ZERO,
                        Vec2::new(200., 0.),
                        Vec2::new(200., 60.),
                        Vec2::new(60., 60.),
                        Vec2::new(60., 200.),
                        Vec2::new(0., 200.),
                    ];
                    graph.fill_color = Color::srgb(0.1, 0.8, 0.5);
                }
                let mut transforms = ui.p2();
                let mut transform = transforms.get_mut(front).unwrap();
                transform.position = Vec2::new(100., 60.);
                transform.size = Vec2::splat(200.);
                next = true;
            }
            6 if elapsed >= 12 => {
                assert!(!entities.contains(state.drawing.unwrap()));
                state.drawing = Some(drawing(state.front.unwrap(), &faces).unwrap());
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(
                        "target/fairygui-demo-graph-properties-polygon.png",
                    ));
                next = true;
            }
            7 => {
                let (front, back) = (state.front.unwrap(), state.back.unwrap());
                click_at(
                    &mut state,
                    &clicks,
                    front,
                    Vec2::new(150., 150.),
                    back,
                    &nodes,
                    window.scale_factor(),
                );
            }
            8 => {
                let front = state.front.unwrap();
                click_at(
                    &mut state,
                    &clicks,
                    front,
                    Vec2::new(30., 150.),
                    front,
                    &nodes,
                    window.scale_factor(),
                );
            }
            9 => {
                let front = state.front.unwrap();
                {
                    let mut graphs = ui.p1();
                    let mut graph = graphs.get_mut(front).unwrap();
                    graph.kind = GraphType::RegularPolygon;
                    graph.sides = 5;
                    graph.start_angle = -90.;
                    graph.distances = vec![f32::NAN, 0.5, 1.2];
                    graph.fill_color = Color::srgb(1., 0.65, 0.05);
                }
                ui.p2().get_mut(front).unwrap().size = Vec2::new(300., 120.);
                ui.p2().get_mut(state.back.unwrap()).unwrap().size = Vec2::new(320., 220.);
                next = true;
            }
            10 if elapsed >= 12 => {
                assert_eq!(drawing(state.front.unwrap(), &faces), state.drawing);
                assert!(ui.p0().graph(state.front.unwrap()).unwrap().distances[0].is_nan());
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(
                        "target/fairygui-demo-graph-properties-regular.png",
                    ));
                next = true;
            }
            11 => {
                let (front, back) = (state.front.unwrap(), state.back.unwrap());
                click_at(
                    &mut state,
                    &clicks,
                    front,
                    Vec2::new(220., 60.),
                    back,
                    &nodes,
                    window.scale_factor(),
                );
            }
            12 => {
                let front = state.front.unwrap();
                {
                    let mut graphs = ui.p1();
                    let mut graph = graphs.get_mut(front).unwrap();
                    graph.kind = GraphType::Rect;
                    graph.corner_radius = Some([30., 5., 50., 0.]);
                    graph.fill_color = Color::srgb(1., 0.3, 0.5);
                }
                ui.p2().get_mut(front).unwrap().size = Vec2::new(220., 140.);
                next = true;
            }
            13 if elapsed >= 12 => {
                assert!(!entities.contains(state.drawing.unwrap()));
                assert!(drawing(state.front.unwrap(), &faces).is_none());
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(
                        "target/fairygui-demo-graph-properties-rounded.png",
                    ));
                next = true;
            }
            14 => {
                let (front, back) = (state.front.unwrap(), state.back.unwrap());
                click_at(
                    &mut state,
                    &clicks,
                    front,
                    Vec2::new(110., 70.),
                    back,
                    &nodes,
                    window.scale_factor(),
                );
            }
            15 => {
                let front = state.front.unwrap();
                ui.p5().get_mut(front).unwrap().0 = Color::srgb(0., 0., 1.);
                let mut graphs = ui.p1();
                let mut graph = graphs.get_mut(front).unwrap();
                graph.corner_radius = None;
                graph.fill_color = Color::srgb(0.3, 0.9, 0.2);
                next = true;
            }
            16 if elapsed >= 8 => {
                assert_eq!(
                    ui.p0().color(state.front.unwrap()),
                    Some(Color::srgb(0.3, 0.9, 0.2))
                );
                next = true;
            }
            17 => {
                let front = state.front.unwrap();
                click_at(
                    &mut state,
                    &clicks,
                    front,
                    Vec2::new(110., 70.),
                    front,
                    &nodes,
                    window.scale_factor(),
                );
            }
            18 => {
                let mut graphs = ui.p1();
                let mut graph = graphs.get_mut(state.front.unwrap()).unwrap();
                graph.kind = GraphType::Empty;
                graph.fill_color = Color::srgb(1., 1., 1.);
                next = true;
            }
            19 if elapsed >= 8 => {
                assert!(drawing(state.front.unwrap(), &faces).is_none());
                assert_eq!(
                    backgrounds.get(state.front.unwrap()).unwrap().0,
                    Color::NONE
                );
                assert!(entities.contains(state.child.unwrap()));
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(
                        "target/fairygui-demo-graph-properties-empty.png",
                    ));
                next = true;
            }
            20 => {
                let (front, back) = (state.front.unwrap(), state.back.unwrap());
                click_at(
                    &mut state,
                    &clicks,
                    front,
                    Vec2::new(110., 70.),
                    back,
                    &nodes,
                    window.scale_factor(),
                );
            }
            21 => {
                commands.entity(state.owner.unwrap()).despawn();
                next = true;
            }
            22 if elapsed >= 8 => {
                for entity in [
                    state.front.unwrap(),
                    state.back.unwrap(),
                    state.child.unwrap(),
                ] {
                    assert!(!entities.contains(entity));
                }
                assert_eq!(clicks.0.len(), 8);
                println!(
                    "demo smoke PASSED: writable ECS graph shape/style/color, native expanded circle and out-of-rectangle hits, concave pass-through, regular polygon NaN distances/resize, rounded DrawPath hit policy, Empty transparency, retained polygon surface and disposal"
                );
                exit.write(AppExit::Success);
                next = true;
            }
            _ => {}
        }
    }
    if next {
        state.step += 1;
        state.since = state.frame;
    }
    window.set_cursor_position(state.cursor);
    move_pointer(window_entity, state.cursor, &mut pointers);
}

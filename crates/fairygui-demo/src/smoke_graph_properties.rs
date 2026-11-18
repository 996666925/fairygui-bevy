//! ECS graph edits, expanded native circles and real contour pass-through clicks.
use super::*;
use bevy::render::view::screenshot::ScreenshotCaptured;

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
    pixel_checks: u8,
}
struct Click {
    stage: u8,
    since: u32,
    count: usize,
    expected: Entity,
}
#[derive(Resource, Default)]
struct Clicks(Vec<Entity>);
fn clicked(event: On<GuiClick>, mut clicks: ResMut<Clicks>) {
    if !event.event.bubbling {
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

fn check_graph_layout(
    graph: Entity,
    child: Entity,
    paint: Option<Entity>,
    size: Vec2,
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
) {
    let (node, transform) = nodes.get(graph).unwrap();
    let physical_size = size / node.inverse_scale_factor();
    assert!(
        node.size().abs_diff_eq(physical_size, 1.),
        "graph layout was enlarged by its stroke"
    );
    let expected = transform
        .transform_point2(node.border_box().min + Vec2::new(7., 11.) / node.inverse_scale_factor());
    let (child_node, child_transform) = nodes.get(child).unwrap();
    let actual = child_transform.transform_point2(child_node.border_box().min);
    assert!(
        actual.abs_diff_eq(expected, 1.),
        "graph stroke shifted its application child: {actual:?} != {expected:?}"
    );
    if let Some(paint) = paint {
        let (_, paint_transform) = nodes.get(paint).unwrap();
        assert!(
            paint_transform
                .translation
                .abs_diff_eq(transform.translation, 1.),
            "graph paint must stay centered on its logical view"
        );
    }
}

fn capture_grays(
    commands: &mut Commands,
    source: Entity,
    samples: &[(Vec2, f32)],
    nodes: &Query<(&ComputedNode, &UiGlobalTransform)>,
    path: &'static str,
) {
    let (node, transform) = nodes.get(source).unwrap();
    let samples: Vec<_> = samples
        .iter()
        .map(|&(local, gray)| {
            let physical = transform
                .transform_point2(node.border_box().min + local / node.inverse_scale_factor());
            (physical.floor().as_uvec2(), gray)
        })
        .collect();
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path))
        .observe(
            move |event: On<ScreenshotCaptured>, mut state: ResMut<State>| {
                for &(point, expected) in &samples {
                    let pixel = event
                        .image
                        .get_color_at(point.x, point.y)
                        .unwrap()
                        .to_linear();
                    for channel in [pixel.red, pixel.green, pixel.blue] {
                        assert!(
                            (channel - expected).abs() < 0.025,
                            "{path}: pixel {point:?}: {pixel:?}, expected linear gray {expected}"
                        );
                    }
                }
                state.pixel_checks += 1;
            },
        );
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
                state.child = Some(
                    commands
                        .spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(7.),
                                top: Val::Px(11.),
                                width: Val::Px(4.),
                                height: Val::Px(6.),
                                ..default()
                            },
                            ChildOf(front),
                        ))
                        .id(),
                );
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
                let px = |value| {
                    if let Val::Px(value) = value {
                        value
                    } else {
                        panic!("pixel graph surface")
                    }
                };
                assert!((px(node.top) + 61.).abs() < 0.5);
                assert!((px(node.width) - 202.).abs() < 0.5);
                assert!((px(node.height) - 202.).abs() < 0.5);
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
                assert_eq!(drawing(state.front.unwrap(), &faces), state.drawing);
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
                assert_eq!(drawing(state.front.unwrap(), &faces), state.drawing);
                state.drawing = Some(drawing(state.front.unwrap(), &faces).unwrap());
                check_graph_layout(
                    state.front.unwrap(),
                    state.child.unwrap(),
                    state.drawing,
                    Vec2::new(220., 140.),
                    &nodes,
                );
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
                let mut graphs = ui.p1();
                let mut graph = graphs.get_mut(state.front.unwrap()).unwrap();
                graph.kind = GraphType::Ellipse;
                graph.fill_color = Color::srgba(0.1, 0.6, 1., 0.5);
                next = true;
            }
            22 if elapsed >= 8 => {
                let front = state.front.unwrap();
                assert_eq!(ui.p0().display(front).unwrap().alpha(), 0.5);
                assert!(drawing(front, &faces).is_some());
                ui.p3().get_mut(front).unwrap().alpha = 0.2;
                next = true;
            }
            23 if elapsed >= 8 => {
                let front = state.front.unwrap();
                assert_eq!(ui.p0().display(front).unwrap().alpha(), 0.2);
                assert!(drawing(front, &faces).is_some());
                ui.p2().get_mut(front).unwrap().size.x += 20.;
                next = true;
            }
            24 if elapsed >= 8 => {
                let front = state.front.unwrap();
                assert_eq!(ui.p0().display(front).unwrap().alpha(), 0.5);
                assert!(drawing(front, &faces).is_some());
                ui.p1().get_mut(front).unwrap().fill_color = Color::srgb(0.6, 0.2, 0.8);
                next = true;
            }
            25 if elapsed >= 8 => {
                let front = state.front.unwrap();
                assert_eq!(ui.p0().display(front).unwrap().alpha(), 0.5);
                let mut graphs = ui.p1();
                let mut graph = graphs.get_mut(front).unwrap();
                graph.kind = GraphType::Rect;
                graph.fill_color = Color::srgba(0.6, 0.2, 0.8, 0.25);
                next = true;
            }
            26 if elapsed >= 8 => {
                let front = state.front.unwrap();
                assert_eq!(ui.p0().display(front).unwrap().alpha(), 0.25);
                let surface = drawing(front, &faces).unwrap();
                check_graph_layout(
                    front,
                    state.child.unwrap(),
                    Some(surface),
                    ui.p0().transform(front).unwrap().size(),
                    &nodes,
                );
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(
                        "target/fairygui-demo-graph-properties-alpha.png",
                    ));
                next = true;
            }
            27 if elapsed >= 8 => {
                let front = state.front.unwrap();
                ui.p1().get_mut(front).unwrap().line_size = 10;
                let mut transforms = ui.p2();
                let mut transform = transforms.get_mut(front).unwrap();
                transform.size = Vec2::new(2., 3.);
                transform.rotation = 17.;
                transform.scale = Vec2::new(1.2, 0.8);
                transform.pivot = Vec2::splat(0.5);
                next = true;
            }
            28 if elapsed >= 8 => {
                let front = state.front.unwrap();
                let surface = drawing(front, &faces).unwrap();
                check_graph_layout(
                    front,
                    state.child.unwrap(),
                    Some(surface),
                    Vec2::new(2., 3.),
                    &nodes,
                );
                next = true;
            }
            29 => {
                ui.p2().get_mut(state.front.unwrap()).unwrap().size = Vec2::new(0., 40.);
                next = true;
            }
            30 if elapsed >= 8 => {
                let front = state.front.unwrap();
                assert!(drawing(front, &faces).is_none());
                check_graph_layout(
                    front,
                    state.child.unwrap(),
                    None,
                    Vec2::new(0., 40.),
                    &nodes,
                );
                next = true;
            }
            31 => {
                ui.p2().get_mut(state.front.unwrap()).unwrap().size = Vec2::new(60., 70.);
                next = true;
            }
            32 if elapsed >= 8 => {
                let front = state.front.unwrap();
                let surface = drawing(front, &faces).unwrap();
                check_graph_layout(
                    front,
                    state.child.unwrap(),
                    Some(surface),
                    Vec2::new(60., 70.),
                    &nodes,
                );
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(
                        "target/fairygui-demo-graph-properties-layout.png",
                    ));
                next = true;
            }
            33 if elapsed >= 8 => {
                let mut graphs = ui.p1();
                let mut graph = graphs.get_mut(state.front.unwrap()).unwrap();
                graph.line_color = Color::NONE;
                graph.fill_color = Color::srgb(0.6, 0.2, 0.8);
                next = true;
            }
            34 if elapsed >= 8 => {
                let front = state.front.unwrap();
                let surface = drawing(front, &faces).unwrap();
                let node = faces.get(surface).unwrap().3;
                assert_eq!(
                    (node.left, node.top, node.width, node.height),
                    (Val::Px(4.), Val::Px(4.), Val::Px(52.), Val::Px(62.))
                );
                check_graph_layout(
                    front,
                    state.child.unwrap(),
                    Some(surface),
                    Vec2::new(60., 70.),
                    &nodes,
                );
                ui.p1().get_mut(front).unwrap().fill_color = Color::NONE;
                next = true;
            }
            35 if elapsed >= 8 => {
                let front = state.front.unwrap();
                assert!(drawing(front, &faces).is_none());
                check_graph_layout(
                    front,
                    state.child.unwrap(),
                    None,
                    Vec2::new(60., 70.),
                    &nodes,
                );
                next = true;
            }
            36 => {
                let front = state.front.unwrap();
                click_at(
                    &mut state,
                    &clicks,
                    front,
                    Vec2::new(30., 35.),
                    front,
                    &nodes,
                    window.scale_factor(),
                );
            }
            37 => {
                let mut graphs = ui.p1();
                let mut graph = graphs.get_mut(state.front.unwrap()).unwrap();
                graph.fill_color = Color::srgba(0.6, 0.2, 0.8, 0.25);
                graph.line_color = Color::WHITE;
                next = true;
            }
            38 if elapsed >= 8 => {
                let front = state.front.unwrap();
                let surface = drawing(front, &faces).unwrap();
                check_graph_layout(
                    front,
                    state.child.unwrap(),
                    Some(surface),
                    Vec2::new(60., 70.),
                    &nodes,
                );
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk("target/fairygui-demo-native-rectangle.png"));
                next = true;
            }
            39 if elapsed >= 8 => {
                let mut graphs = ui.p1();
                let mut graph = graphs.get_mut(state.front.unwrap()).unwrap();
                graph.kind = GraphType::Ellipse;
                graph.line_color = Color::NONE;
                graph.fill_color = Color::srgb(0.6, 0.2, 0.8);
                next = true;
            }
            40 if elapsed >= 8 => {
                let front = state.front.unwrap();
                let surface = drawing(front, &faces).unwrap();
                let node = faces.get(surface).unwrap().3;
                assert_eq!(node.border, UiRect::ZERO);
                let Val::Px(width) = node.width else {
                    panic!("pixel circle surface")
                };
                assert!(
                    (width - 52.).abs() < 0.5,
                    "transparent line must inset the circle path"
                );
                check_graph_layout(
                    front,
                    state.child.unwrap(),
                    Some(surface),
                    Vec2::new(60., 70.),
                    &nodes,
                );
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk("target/fairygui-demo-native-circle-inset.png"));
                next = true;
            }
            41 => {
                let front = state.front.unwrap();
                click_at(
                    &mut state,
                    &clicks,
                    front,
                    Vec2::new(1., 35.),
                    front,
                    &nodes,
                    window.scale_factor(),
                );
            }
            42 => {
                ui.p1().get_mut(state.front.unwrap()).unwrap().line_size = 60;
                next = true;
            }
            43 if elapsed >= 8 => {
                let front = state.front.unwrap();
                assert!(drawing(front, &faces).is_none());
                check_graph_layout(
                    front,
                    state.child.unwrap(),
                    None,
                    Vec2::new(60., 70.),
                    &nodes,
                );
                next = true;
            }
            44 => {
                let front = state.front.unwrap();
                click_at(
                    &mut state,
                    &clicks,
                    front,
                    Vec2::new(30., 35.),
                    front,
                    &nodes,
                    window.scale_factor(),
                );
            }
            45 => {
                let mut graphs = ui.p1();
                let mut graph = graphs.get_mut(state.front.unwrap()).unwrap();
                graph.line_size = 80;
                graph.line_color = Color::WHITE;
                graph.fill_color = Color::srgba(0.6, 0.2, 0.8, 0.5);
                next = true;
            }
            46 if elapsed >= 8 => {
                let front = state.front.unwrap();
                let surface = drawing(front, &faces).unwrap();
                check_graph_layout(
                    front,
                    state.child.unwrap(),
                    Some(surface),
                    Vec2::new(60., 70.),
                    &nodes,
                );
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(
                        "target/fairygui-demo-native-circle-thick-stroke.png",
                    ));
                next = true;
            }
            47 if elapsed >= 8 => {
                let mut graphs = ui.p1();
                let mut graph = graphs.get_mut(state.front.unwrap()).unwrap();
                graph.kind = GraphType::Rect;
                graph.corner_radius = Some([0.; 4]);
                graph.line_size = 10;
                next = true;
            }
            48 if elapsed >= 8 => {
                let front = state.front.unwrap();
                let surface = drawing(front, &faces).unwrap();
                state.drawing = Some(surface);
                let node = faces.get(surface).unwrap().3;
                assert_eq!(
                    (node.left, node.top, node.width, node.height),
                    (Val::Px(-6.), Val::Px(-6.), Val::Px(72.), Val::Px(82.))
                );
                assert_eq!(node.border, UiRect::ZERO);
                check_graph_layout(
                    front,
                    state.child.unwrap(),
                    Some(surface),
                    Vec2::new(60., 70.),
                    &nodes,
                );
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(
                        "target/fairygui-demo-native-rounded-zero-radii.png",
                    ));
                next = true;
            }
            49 => {
                let (front, back) = (state.front.unwrap(), state.back.unwrap());
                click_at(
                    &mut state,
                    &clicks,
                    front,
                    Vec2::new(30., 35.),
                    back,
                    &nodes,
                    window.scale_factor(),
                );
            }
            50 => {
                let mut graphs = ui.p1();
                let mut graph = graphs.get_mut(state.front.unwrap()).unwrap();
                graph.corner_radius = Some([100., 60., 80., 40.]);
                graph.line_size = 3;
                next = true;
            }
            51 if elapsed >= 8 => {
                let front = state.front.unwrap();
                assert_eq!(
                    ui.p0().graph(front).unwrap().corner_radius,
                    Some([100., 60., 80., 40.])
                );
                assert_eq!(drawing(front, &faces), state.drawing);
                check_graph_layout(
                    front,
                    state.child.unwrap(),
                    None,
                    Vec2::new(60., 70.),
                    &nodes,
                );
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(
                        "target/fairygui-demo-native-rounded-oversized.png",
                    ));
                next = true;
            }
            52 if elapsed >= 8 => {
                ui.p2().get_mut(state.front.unwrap()).unwrap().size.x = 0.;
                next = true;
            }
            53 if elapsed >= 8 => {
                let front = state.front.unwrap();
                assert!(drawing(front, &faces).is_none());
                check_graph_layout(
                    front,
                    state.child.unwrap(),
                    None,
                    Vec2::new(0., 70.),
                    &nodes,
                );
                ui.p2().get_mut(front).unwrap().size.x = 60.;
                let mut graphs = ui.p1();
                let mut graph = graphs.get_mut(front).unwrap();
                graph.corner_radius = Some([8., 12., 16., 20.]);
                graph.line_color = Color::NONE;
                next = true;
            }
            54 if elapsed >= 8 => {
                let front = state.front.unwrap();
                let surface = drawing(front, &faces).unwrap();
                let node = faces.get(surface).unwrap().3;
                assert_eq!(
                    (node.left, node.top, node.width, node.height),
                    (Val::Px(-1.), Val::Px(-1.), Val::Px(62.), Val::Px(72.))
                );
                check_graph_layout(
                    front,
                    state.child.unwrap(),
                    Some(surface),
                    Vec2::new(60., 70.),
                    &nodes,
                );
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(
                        "target/fairygui-demo-native-rounded-transparent-line.png",
                    ));
                next = true;
            }
            55 if elapsed >= 8 => {
                let (front, back) = (state.front.unwrap(), state.back.unwrap());
                ui.p1().get_mut(back).unwrap().fill_color = Color::BLACK;
                ui.p3().get_mut(back).unwrap().alpha = 1.;
                *ui.p1().get_mut(front).unwrap() = GuiGraph {
                    kind: GraphType::Polygon,
                    line_size: 0,
                    fill_color: Color::srgba(1., 1., 1., 0.5),
                    polygon_points: vec![
                        Vec2::ZERO,
                        Vec2::splat(100.),
                        Vec2::new(0., 100.),
                        Vec2::new(100., 0.),
                    ],
                    ..default()
                };
                let mut transforms = ui.p2();
                for (entity, position, size) in [
                    (back, Vec2::new(80., 40.), Vec2::splat(180.)),
                    (front, Vec2::new(100., 60.), Vec2::splat(100.)),
                ] {
                    let mut transform = transforms.get_mut(entity).unwrap();
                    transform.position = position;
                    transform.size = size;
                    transform.scale = Vec2::ONE;
                    transform.skew = Vec2::ZERO;
                    transform.pivot = Vec2::ZERO;
                    transform.pivot_as_anchor = false;
                    transform.rotation = 0.;
                }
                next = true;
            }
            56 if elapsed >= 12 => {
                let front = state.front.unwrap();
                check_graph_layout(
                    front,
                    state.child.unwrap(),
                    drawing(front, &faces),
                    Vec2::splat(100.),
                    &nodes,
                );
                capture_grays(
                    &mut commands,
                    front,
                    &[(Vec2::new(90., 50.), 0.5), (Vec2::new(25., 90.), 0.)],
                    &nodes,
                    "target/fairygui-demo-native-polygon-fill.png",
                );
                next = true;
            }
            57 => {
                let (front, back) = (state.front.unwrap(), state.back.unwrap());
                click_at(
                    &mut state,
                    &clicks,
                    front,
                    Vec2::new(90., 50.),
                    back,
                    &nodes,
                    window.scale_factor(),
                );
            }
            58 => {
                let front = state.front.unwrap();
                click_at(
                    &mut state,
                    &clicks,
                    front,
                    Vec2::new(25., 90.),
                    front,
                    &nodes,
                    window.scale_factor(),
                );
            }
            59 if state.pixel_checks >= 1 => {
                ui.p1()
                    .get_mut(state.front.unwrap())
                    .unwrap()
                    .polygon_points = vec![
                    Vec2::ZERO,
                    Vec2::new(100., 0.),
                    Vec2::splat(100.),
                    Vec2::new(0., 100.),
                ];
                next = true;
            }
            60 if elapsed >= 8 => {
                capture_grays(
                    &mut commands,
                    state.front.unwrap(),
                    &[
                        Vec2::splat(25.),
                        Vec2::splat(50.),
                        Vec2::new(30., 70.),
                        Vec2::new(70., 30.),
                        Vec2::splat(75.),
                    ]
                    .map(|p| (p, 0.5)),
                    &nodes,
                    "target/fairygui-demo-native-polygon-shared-edges.png",
                );
                next = true;
            }
            61 if state.pixel_checks >= 2 => {
                ui.p1()
                    .get_mut(state.front.unwrap())
                    .unwrap()
                    .polygon_points = vec![
                    Vec2::ZERO,
                    Vec2::new(100., 0.),
                    Vec2::splat(100.),
                    Vec2::new(0., 100.),
                    Vec2::ZERO,
                    Vec2::new(100., 0.),
                    Vec2::splat(100.),
                    Vec2::new(0., 100.),
                ];
                next = true;
            }
            62 if elapsed >= 8 => {
                capture_grays(
                    &mut commands,
                    state.front.unwrap(),
                    &[
                        Vec2::splat(25.),
                        Vec2::splat(50.),
                        Vec2::new(30., 70.),
                        Vec2::new(70., 30.),
                        Vec2::splat(75.),
                    ]
                    .map(|p| (p, 0.75)),
                    &nodes,
                    "target/fairygui-demo-native-polygon-overdraw.png",
                );
                next = true;
            }
            63 if state.pixel_checks >= 3 => {
                let front = state.front.unwrap();
                {
                    let mut graphs = ui.p1();
                    let mut graph = graphs.get_mut(front).unwrap();
                    graph.kind = GraphType::RegularPolygon;
                    graph.sides = 7;
                    graph.start_angle = 17.;
                    graph.distances = vec![1., -1., f32::NAN, 0., 1.3, -0.5];
                }
                ui.p2().get_mut(front).unwrap().size = Vec2::new(120., 90.);
                next = true;
            }
            64 if elapsed >= 8 => {
                let front = state.front.unwrap();
                assert!(ui.p0().graph(front).unwrap().distances[2].is_nan());
                check_graph_layout(
                    front,
                    state.child.unwrap(),
                    None,
                    Vec2::new(120., 90.),
                    &nodes,
                );
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(
                        "target/fairygui-demo-native-regular-polygon-fill.png",
                    ));
                next = true;
            }
            65 if elapsed >= 8 => {
                let (front, back) = (state.front.unwrap(), state.back.unwrap());
                for (entity, color) in
                    [(front, Color::srgba(1., 1., 1., 0.5)), (back, Color::BLACK)]
                {
                    *ui.p1().get_mut(entity).unwrap() = GuiGraph {
                        kind: GraphType::Rect,
                        fill_color: color,
                        ..default()
                    };
                }
                {
                    let mut transforms = ui.p2();
                    let mut t = transforms.get_mut(front).unwrap();
                    t.position = Vec2::new(250., 180.);
                    t.size = Vec2::new(120., 90.);
                    t.pivot = Vec2::splat(0.5);
                    t.pivot_as_anchor = true;
                    t.rotation = 17.;
                    t.scale = Vec2::new(1.2, 0.8);
                    t.skew = Vec2::new(20., -12.);
                    let mut t = transforms.get_mut(back).unwrap();
                    t.position = Vec2::ZERO;
                    t.size = Vec2::new(900., 500.);
                }
                next = true;
            }
            66 if elapsed >= 12 => {
                let front = state.front.unwrap();
                let owner = state.owner.unwrap();
                let gui = ui.p0();
                let root = gui.view(owner).unwrap();
                let (n, t) = nodes.get(root).unwrap();
                let (front_n, front_t) = nodes.get(front).unwrap();
                for p in [Vec2::ZERO, Vec2::new(30., 20.), Vec2::new(60., 45.)] {
                    let native = gui.local_to_global(front, p).unwrap();
                    let local = gui.global_to_local(owner, native).unwrap();
                    let expected =
                        t.transform_point2(n.border_box().min + local / n.inverse_scale_factor());
                    let actual = front_t.transform_point2(
                        front_n.border_box().min
                            + (p + Vec2::new(60., 45.)) / front_n.inverse_scale_factor(),
                    );
                    assert!(
                        actual.abs_diff_eq(expected, 1.),
                        "skew model/render mismatch: {actual:?} vs {expected:?}"
                    );
                }
                check_graph_layout(
                    front,
                    state.child.unwrap(),
                    drawing(front, &faces),
                    Vec2::new(120., 90.),
                    &nodes,
                );
                capture_grays(
                    &mut commands,
                    front,
                    &[
                        (Vec2::new(30., 25.), 0.5),
                        (Vec2::new(80., 55.), 0.5),
                        (Vec2::new(-12., 45.), 0.),
                        (Vec2::new(132., 45.), 0.),
                    ],
                    &nodes,
                    "target/fairygui-demo-skew-transform.png",
                );
                next = true;
            }
            67 if state.pixel_checks >= 4 => {
                let front = state.front.unwrap();
                click_at(
                    &mut state,
                    &clicks,
                    front,
                    Vec2::new(30., 25.),
                    front,
                    &nodes,
                    window.scale_factor(),
                );
            }
            68 => {
                let (front, back) = (state.front.unwrap(), state.back.unwrap());
                click_at(
                    &mut state,
                    &clicks,
                    front,
                    Vec2::new(-12., 45.),
                    back,
                    &nodes,
                    window.scale_factor(),
                );
            }
            69 => {
                let front = state.front.unwrap();
                {
                    let mut transforms = ui.p2();
                    let mut t = transforms.get_mut(front).unwrap();
                    t.skew = Vec2::ZERO;
                    t.position = Vec2::new(250.5, 180.5);
                }
                commands.entity(front).insert(GuiPixelSnapping(true));
                next = true;
            }
            70 if elapsed >= 8 => {
                let front = state.front.unwrap();
                let gui = ui.p0();
                assert_eq!(gui.pixel_snapping(front), Some(true));
                assert_eq!(
                    gui.transform(front).unwrap().position,
                    Vec2::new(250.5, 180.5)
                );
                let native = gui.local_to_global(front, Vec2::ZERO).unwrap();
                let local = gui.global_to_local(state.owner.unwrap(), native).unwrap();
                assert!(local.abs_diff_eq(Vec2::new(251., 181.), 0.001));
                check_graph_layout(
                    front,
                    state.child.unwrap(),
                    drawing(front, &faces),
                    Vec2::new(120., 90.),
                    &nodes,
                );
                let owner = gui.view(state.owner.unwrap()).unwrap();
                let (on, ot) = nodes.get(owner).unwrap();
                let expected =
                    ot.transform_point2(on.border_box().min + local / on.inverse_scale_factor());
                let (_, ft) = nodes.get(front).unwrap();
                assert!(
                    ft.translation.abs_diff_eq(expected, 0.001),
                    "snapped rendering {actual:?} vs {expected:?}",
                    actual = ft.translation
                );
                capture_grays(
                    &mut commands,
                    front,
                    &[(Vec2::new(30., 25.), 0.5), (Vec2::new(80., 55.), 0.5)],
                    &nodes,
                    "target/fairygui-demo-pixel-snapping.png",
                );
                next = true;
            }
            71 if state.pixel_checks >= 5 => {
                commands
                    .entity(state.front.unwrap())
                    .insert(GuiPixelSnapping(false));
                next = true;
            }
            72 if elapsed >= 8 => {
                let front = state.front.unwrap();
                let gui = ui.p0();
                assert_eq!(gui.pixel_snapping(front), Some(false));
                let global = gui.local_to_global(front, Vec2::ZERO).unwrap();
                let local = gui.global_to_local(state.owner.unwrap(), global).unwrap();
                assert!(local.abs_diff_eq(Vec2::new(250.5, 180.5), 0.001));
                let owner = gui.view(state.owner.unwrap()).unwrap();
                let (on, ot) = nodes.get(owner).unwrap();
                let expected =
                    ot.transform_point2(on.border_box().min + local / on.inverse_scale_factor());
                let (_, ft) = nodes.get(front).unwrap();
                assert!(
                    ft.translation.abs_diff_eq(expected, 0.001),
                    "fractional rendering did not restore"
                );
                commands.entity(state.owner.unwrap()).despawn();
                next = true;
            }
            73 if elapsed >= 8 => {
                for entity in [
                    state.front.unwrap(),
                    state.back.unwrap(),
                    state.child.unwrap(),
                ] {
                    assert!(!entities.contains(entity));
                }
                assert_eq!(clicks.0.len(), 16);
                assert_eq!(state.pixel_checks, 5);
                println!(
                    "demo smoke PASSED: native primitive paths and triangle fills, self-intersection paint versus hit policy, GPU pixel checks, skew and pixel snapping rendering/model/child geometry, clicks, fractional restore and disposal"
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

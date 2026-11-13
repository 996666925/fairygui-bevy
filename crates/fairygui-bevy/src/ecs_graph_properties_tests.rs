use crate::prelude::*;
use crate::{
    core::widget::ControlData,
    graph::{GraphRender, GraphSurface},
};
use bevy::{ecs::system::SystemState, prelude::*};

fn fixture(loaded: bool) -> (App, Entity, Handle<FuiAsset>) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let handle = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(handle.clone(), "Demo_Graph"))
        .id();
    if loaded {
        load(&mut app, &handle);
        app.update();
    }
    (app, owner, handle)
}
fn load(app: &mut App, handle: &Handle<FuiAsset>) {
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(
            handle.id(),
            FuiAsset {
                package: crate::core::FuiPackage::parse(
                    "Basics",
                    include_bytes!("../../fairygui-demo/assets/ui/Basics.fui"),
                )
                .unwrap(),
                res_key: "ui/Basics".into(),
            },
        )
        .unwrap();
}
fn graph(app: &mut App, owner: Entity) -> Entity {
    SystemState::<GuiQuery>::new(app.world_mut())
        .get(app.world())
        .unwrap()
        .child_by_name(owner, "n10")
        .unwrap()
}
fn surface(app: &mut App, graph: Entity) -> Option<Entity> {
    app.world_mut()
        .query::<(Entity, &GraphSurface, &ChildOf)>()
        .iter(app.world())
        .find_map(|(entity, _, parent)| (parent.parent() == graph).then_some(entity))
}

#[test]
fn initialize_graph_configuration_preserves_native_nan_defaults_and_stable_geometry_snapshots() {
    fn initialize(
        roots: Query<Entity, Added<GuiRootView>>,
        mut ui: ParamSet<(GuiQuery, Query<&mut GuiGraph>)>,
    ) {
        for owner in &roots {
            let view = ui.p0().child_by_name(owner, "n10").unwrap();
            let mut graphs = ui.p1();
            let mut graph = graphs.get_mut(view).unwrap();
            graph.kind = GraphType::RegularPolygon;
            graph.sides = 5;
            graph.start_angle = -90.;
            graph.distances = vec![f32::NAN, 0.5];
            graph.fill_color = Color::srgb(0.2, 0.6, 1.);
        }
    }
    let (mut app, owner, asset) = fixture(false);
    app.add_systems(Update, initialize.in_set(GuiSystems::Initialize));
    app.update();
    load(&mut app, &asset);
    app.update();
    let graph = graph(&mut app, owner);
    let drawn = surface(&mut app, graph).unwrap();
    assert!(app.world().get::<GuiGraph>(drawn).is_none());
    assert!(app.world().get::<GuiTransform>(drawn).is_none());
    assert!(app.world().get::<GuiColor>(drawn).is_none());
    assert!(
        app.world()
            .get::<crate::graph::GraphSurface>(drawn)
            .is_some()
    );
    let state = app.world().get::<GuiGraph>(graph).unwrap();
    assert_eq!(state.kind, GraphType::RegularPolygon);
    assert!(state.distances[0].is_nan());
    assert_eq!(state, &state.clone());
    let property_tick = app
        .world()
        .entity(graph)
        .get_ref::<GuiGraph>()
        .unwrap()
        .last_changed();
    let render_tick = app
        .world()
        .entity(graph)
        .get_ref::<GraphRender>()
        .unwrap()
        .last_changed();
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(surface(&mut app, graph), Some(drawn));
    assert_eq!(
        app.world()
            .entity(graph)
            .get_ref::<GuiGraph>()
            .unwrap()
            .last_changed(),
        property_tick
    );
    assert_eq!(
        app.world()
            .entity(graph)
            .get_ref::<GraphRender>()
            .unwrap()
            .last_changed(),
        render_tick
    );
}

#[test]
fn graph_fields_merge_native_changes_and_color_aliases_keep_their_precedence() {
    let (mut app, owner, _) = fixture(true);
    let graph = graph(&mut app, owner);
    app.world_mut().get_mut::<GuiColor>(graph).unwrap().0 = Color::srgb(1., 0., 0.);
    app.world_mut()
        .get_mut::<GuiGraph>(graph)
        .unwrap()
        .fill_color = Color::srgb(0., 1., 0.);
    app.update();
    assert_eq!(
        app.world().get::<GuiColor>(graph).unwrap().0,
        Color::srgb(0., 1., 0.)
    );
    let object = app.world().get::<GObjectRef>(graph).unwrap().0;
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_control_color(object, crate::core::color::Color::new(0., 0., 1., 1.), None);
    app.world_mut()
        .get_mut::<GuiGraph>(graph)
        .unwrap()
        .line_size = 7;
    app.update();
    let state = app.world().get::<GuiGraph>(graph).unwrap();
    assert_eq!(state.fill_color, Color::srgb(0., 0., 1.));
    assert_eq!(state.line_size, 7);
    let mut data = match app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .object(object)
        .unwrap()
        .control()
    {
        ControlData::Graph(data) => data.clone(),
        _ => unreachable!(),
    };
    data.line_size = 9;
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_graph(object, data);
    app.world_mut()
        .get_mut::<GuiGraph>(graph)
        .unwrap()
        .fill_color = Color::srgb(0.7, 0.2, 0.1);
    app.update();
    assert_eq!(app.world().get::<GuiGraph>(graph).unwrap().line_size, 9);
}

#[test]
fn invalid_geometry_fields_restore_previous_values_while_valid_style_edits_apply() {
    let (mut app, owner, _) = fixture(true);
    let graph = graph(&mut app, owner);
    let previous = app.world().get::<GuiGraph>(graph).unwrap().clone();
    {
        let mut state = app.world_mut().get_mut::<GuiGraph>(graph).unwrap();
        state.polygon_points = vec![Vec2::new(f32::NAN, 2.)];
        state.distances = vec![f32::INFINITY];
        state.start_angle = f32::NAN;
        state.corner_radius = Some([f32::NAN; 4]);
        state.line_color = Color::srgb(f32::NAN, 0., 0.);
        state.fill_color = Color::srgb(0.2, 0.8, 0.4);
        state.line_size = 3;
    }
    app.update();
    let state = app.world().get::<GuiGraph>(graph).unwrap();
    assert_eq!(state.polygon_points, previous.polygon_points);
    assert_eq!(state.distances, previous.distances);
    assert_eq!(state.start_angle, previous.start_angle);
    assert_eq!(state.corner_radius, previous.corner_radius);
    assert_eq!(state.line_color, previous.line_color);
    assert_eq!(state.line_size, 3);
    assert_eq!(state.fill_color, Color::srgb(0.2, 0.8, 0.4));
}

#[test]
fn direct_graph_edits_keep_identity_and_switch_surfaces_without_losing_application_children() {
    let (mut app, owner, _) = fixture(true);
    let graph = graph(&mut app, owner);
    let object = app.world().get::<GObjectRef>(graph).unwrap().0;
    let child = app
        .world_mut()
        .spawn((Node::default(), ChildOf(graph)))
        .id();
    app.world_mut().get_mut::<GuiTransform>(graph).unwrap().size = Vec2::new(120., 60.);
    app.world_mut().gui_mut(graph).unwrap().set_graph(GuiGraph {
        kind: GraphType::Ellipse,
        fill_color: Color::WHITE,
        ..default()
    });
    let ControlData::Graph(data) = app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .object(object)
        .unwrap()
        .control()
    else {
        panic!()
    };
    assert_eq!(data.graph_kind(), GraphType::Ellipse);
    app.update();
    let circle = surface(&mut app, graph).unwrap();
    assert_eq!(
        app.world().get::<GraphSurface>(circle).unwrap().canvas,
        Vec4::new(0., -30., 120., 120.)
    );
    for kind in [GraphType::Polygon, GraphType::Rect, GraphType::Empty] {
        {
            let mut state = app.world_mut().get_mut::<GuiGraph>(graph).unwrap();
            state.kind = kind;
            state.polygon_points = vec![Vec2::ZERO, Vec2::new(120., 0.), Vec2::new(20., 60.)];
        }
        app.update();
        assert!(app.world().get_entity(child).is_ok());
        assert_eq!(
            app.world().get::<GuiTransform>(graph).unwrap().size,
            Vec2::new(120., 60.)
        );
    }
    assert!(app.world().get_entity(circle).is_err());
    assert!(surface(&mut app, graph).is_none());
    assert_eq!(
        app.world().get::<BackgroundColor>(graph).unwrap().0,
        Color::NONE
    );
    let fill = app
        .world()
        .get::<GuiGraph>(graph)
        .unwrap()
        .fill_color
        .to_srgba();
    assert!(
        (fill.red - 1.).abs() < 0.0001
            && (fill.green - 1.).abs() < 0.0001
            && (fill.blue - 1.).abs() < 0.0001
    );
    assert_eq!(fill.alpha, 1.);
    app.world_mut().despawn(owner);
    app.update();
    assert!(app.world().get_entity(graph).is_err());
    assert!(app.world().get_entity(child).is_err());
}

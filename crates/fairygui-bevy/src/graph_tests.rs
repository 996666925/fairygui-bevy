use super::*;
use fairygui_core::{color::Color as GuiColor, package::ObjectType};

fn fixture() -> (App, GObjectId, Entity) {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()));
    install(&mut app);
    let mut gui = GuiRuntime::default();
    let id = gui.store_mut().create(ObjectType::Graph);
    gui.store_mut().set_size(id, 100.0, 80.0);
    gui.store_mut().set_graph(
        id,
        GraphData {
            graph_type: 3,
            line_size: 3,
            line_color: GuiColor::new(0.0, 0.0, 0.0, 1.0),
            fill_color: GuiColor::WHITE,
            polygon_points: Some(vec![0.0, 0.0, 100.0, 0.0, 40.0, 80.0]),
            ..default()
        },
    );
    let entity = app
        .world_mut()
        .spawn((Node::default(), GObjectRef(id)))
        .id();
    app.insert_non_send(gui).add_systems(
        Update,
        (sync_graphs, crate::runtime::sync_appearance).chain(),
    );
    (app, id, entity)
}

#[test]
fn polygon_material_preserves_surface_buffer_and_updates_geometry_color_and_ancestor_alpha() {
    let (mut app, id, root) = fixture();
    app.update();
    let render = app.world().get::<GraphRender>(root).unwrap();
    let surface = render.surface.unwrap();
    let handle = render.material.as_ref().unwrap().clone();
    let material = app
        .world()
        .resource::<Assets<PolygonMaterial>>()
        .get(&handle)
        .unwrap();
    let buffer = material.geometry.clone();
    assert_eq!(material.uniform.counts.x, 3);
    assert!(material.uniform.counts.y > 0);
    assert!(material.uniform.canvas.x < 0.0);
    assert_eq!(app.world().get::<ChildOf>(surface).unwrap().parent(), root);
    assert_eq!(
        app.world()
            .get::<BackgroundColor>(surface)
            .unwrap()
            .0
            .to_linear()
            .alpha,
        0.0,
        "polygon surface must not inherit a rectangular graph background"
    );
    let parent = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let parent = gui.store_mut().create(ObjectType::Component);
        gui.store_mut().set_parent(id, Some(parent));
        gui.store_mut().set_alpha(parent, 0.5);
        gui.store_mut()
            .set_control_color(id, GuiColor::new(1.0, 0.0, 0.0, 0.5), None);
        parent
    };
    app.update();
    let material = app
        .world()
        .resource::<Assets<PolygonMaterial>>()
        .get(&handle)
        .unwrap();
    assert_eq!(material.uniform.fill, Vec4::new(1.0, 0.0, 0.0, 0.25));
    assert_eq!(material.uniform.stroke.w, 0.5);
    assert_eq!(material.geometry, buffer);
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut().set_graph(
            id,
            GraphData {
                graph_type: 4,
                sides: 7,
                start_angle: -90.0,
                fill_color: GuiColor::WHITE,
                ..default()
            },
        );
        gui.store_mut().set_size(id, 200.0, 120.0);
        gui.store_mut().set_alpha(parent, 1.0);
    }
    app.update();
    let render = app.world().get::<GraphRender>(root).unwrap();
    assert_eq!(render.surface, Some(surface));
    assert_eq!(render.material.as_ref(), Some(&handle));
    let material = app
        .world()
        .resource::<Assets<PolygonMaterial>>()
        .get(&handle)
        .unwrap();
    assert_eq!(material.uniform.counts.x, 7);
    assert_eq!(material.geometry, buffer);
    assert_eq!(material.uniform.fill.w, 1.0);
}

#[test]
fn changing_polygon_to_basic_or_empty_releases_surface_and_invalid_geometry_is_bounded() {
    let (mut app, id, root) = fixture();
    app.update();
    let surface = app
        .world()
        .get::<GraphRender>(root)
        .unwrap()
        .surface
        .unwrap();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_graph(
            id,
            GraphData {
                graph_type: 1,
                fill_color: GuiColor::WHITE,
                ..default()
            },
        );
    app.update();
    assert!(app.world().get_entity(surface).is_err());
    assert!(
        app.world()
            .get::<GraphRender>(root)
            .unwrap()
            .surface
            .is_none()
    );
    assert_eq!(
        app.world()
            .get::<BackgroundColor>(root)
            .unwrap()
            .0
            .to_linear(),
        Color::WHITE.to_linear()
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_graph(
            id,
            GraphData {
                graph_type: 3,
                polygon_points: Some(vec![f32::INFINITY, 0.0, 10.0, 20.0]),
                ..default()
            },
        );
    for _ in 0..3 {
        app.update();
    }
    assert!(
        app.world()
            .get::<GraphRender>(root)
            .unwrap()
            .surface
            .is_none()
    );
    assert_eq!(app.world().resource::<Assets<PolygonMaterial>>().len(), 0);
}

#[test]
fn native_circle_surface_keeps_its_width_based_bounds_tint_and_entity_across_resizes() {
    let (mut app, id, root) = fixture();
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_graph(
            id,
            GraphData {
                graph_type: 2,
                line_size: 4,
                line_color: GuiColor::WHITE,
                fill_color: GuiColor::new(1., 0., 0., 0.5),
                ..default()
            },
        );
    app.update();
    let surface = app
        .world()
        .get::<GraphRender>(root)
        .unwrap()
        .surface
        .unwrap();
    let node = app.world().get::<Node>(surface).unwrap();
    assert_eq!(
        (node.left, node.top, node.width, node.height),
        (Val::Px(0.), Val::Px(-10.), Val::Px(100.), Val::Px(100.))
    );
    assert_eq!(node.border, UiRect::all(Val::Px(4.)));
    assert!(
        app.world()
            .get::<BackgroundColor>(root)
            .is_none_or(|bg| bg.0 == Color::NONE)
    );
    assert_eq!(app.world().resource::<Assets<PolygonMaterial>>().len(), 0);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_alpha(id, 0.5);
    app.update();
    assert_eq!(
        app.world()
            .get::<BackgroundColor>(surface)
            .unwrap()
            .0
            .to_linear()
            .alpha,
        0.25
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_size(id, 40., 100.);
    app.update();
    assert_eq!(
        app.world().get::<GraphRender>(root).unwrap().surface,
        Some(surface)
    );
    assert_eq!(
        app.world().get::<GraphSurface>(surface).unwrap().canvas,
        Vec4::new(0., 30., 40., 40.)
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_size(id, 40., 0.);
    app.update();
    assert!(app.world().get_entity(surface).is_err());
    assert!(
        app.world()
            .get::<GraphRender>(root)
            .unwrap()
            .surface
            .is_none()
    );
}

#[test]
fn switching_circle_polygon_rectangle_and_empty_reclaims_only_owned_drawing_surfaces() {
    let (mut app, id, root) = fixture();
    let child = app.world_mut().spawn((Node::default(), ChildOf(root))).id();
    app.update();
    let polygon = app
        .world()
        .get::<GraphRender>(root)
        .unwrap()
        .surface
        .unwrap();
    for kind in [2, 3, 1, 0] {
        let old = app.world().get::<GraphRender>(root).unwrap().surface;
        let mut data = match app
            .world()
            .non_send::<GuiRuntime>()
            .store()
            .object(id)
            .unwrap()
            .control()
        {
            ControlData::Graph(data) => data.clone(),
            _ => unreachable!(),
        };
        data.graph_type = kind;
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_graph(id, data);
        app.update();
        assert!(app.world().get_entity(child).is_ok());
        if let Some(old) = old {
            assert!(app.world().get_entity(old).is_err());
        }
        if kind == 0 {
            assert_eq!(
                app.world().get::<BackgroundColor>(root).unwrap().0,
                Color::NONE
            );
        }
    }
    assert!(app.world().get_entity(polygon).is_err());
}

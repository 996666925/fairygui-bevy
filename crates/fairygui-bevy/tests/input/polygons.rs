use super::*;
use fairygui_core::{color::Color, widget::GraphData};

#[test]
fn polygon_cutout_passes_to_lower_target_and_geometry_changes_apply_before_the_next_click() {
    let (mut app, _) = setup();
    let (_, below) = target(&mut app, 1);
    let graph = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let graph = gui.store_mut().create(ObjectType::Graph);
        gui.store_mut().set_size(graph, 100.0, 100.0);
        gui.store_mut().set_graph(
            graph,
            GraphData {
                graph_type: 3,
                polygon_points: Some(vec![
                    0.0, 0.0, 100.0, 0.0, 100.0, 30.0, 30.0, 30.0, 30.0, 100.0, 0.0, 100.0,
                ]),
                fill_color: Color::TRANSPARENT,
                ..default()
            },
        );
        gui.store_mut().attach_to_stage(graph);
        graph
    };
    let view = app
        .world_mut()
        .spawn((
            GObjectRef(graph),
            Hovered(true),
            ComputedNode::default(),
            ComputedStackIndex(2),
            RelativeCursorPosition {
                cursor_over: true,
                normalized: Some(Vec2::ZERO),
            },
        ))
        .id();
    press(&mut app);
    release(&mut app);
    assert_eq!(
        clicks(&mut app)
            .iter()
            .map(|e| e.target)
            .collect::<Vec<_>>(),
        vec![below]
    );
    app.world_mut()
        .get_mut::<RelativeCursorPosition>(view)
        .unwrap()
        .normalized = Some(Vec2::new(-0.35, 0.2));
    press(&mut app);
    release(&mut app);
    assert_eq!(
        clicks(&mut app)
            .iter()
            .map(|e| e.target)
            .collect::<Vec<_>>(),
        vec![graph],
        "native hit shape is independent of transparent fill"
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_graph(
            graph,
            GraphData {
                graph_type: 4,
                sides: 3,
                start_angle: 0.0,
                ..default()
            },
        );
    press(&mut app);
    release(&mut app);
    assert_eq!(
        clicks(&mut app)
            .iter()
            .map(|e| e.target)
            .collect::<Vec<_>>(),
        vec![below]
    );
}

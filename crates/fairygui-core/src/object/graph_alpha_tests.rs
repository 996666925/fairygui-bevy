use super::*;
use crate::widget::GraphData;

#[test]
fn graph_redraw_alpha_matches_original_fill_resize_and_color_operations() {
    let mut store = ObjectStore::new();
    let mut graph = None;
    let mut kind = 0;
    let mut states = 0;
    for line in include_str!("../../tests/fixtures/graph-alpha-reference.txt").lines() {
        if line.starts_with('#') {
            continue;
        }
        if let Some(value) = line.strip_prefix("case ") {
            store = ObjectStore::new();
            let id = store.create(ObjectType::Graph);
            store.set_size(id, 100., 80.);
            graph = Some(id);
            kind = value.parse().unwrap();
            continue;
        }
        let id = graph.unwrap();
        let (operation, expected) = line.split_once(" | ").unwrap();
        let words: Vec<_> = operation.split_whitespace().collect();
        let v: f32 = words[1].parse().unwrap();
        match words[0] {
            "alpha" => store.set_alpha(id, v),
            "size" => store.set_size(id, v, words[2].parse().unwrap()),
            "draw" => {
                store.set_graph(
                    id,
                    GraphData {
                        graph_type: kind,
                        line_size: 2,
                        line_color: Color::WHITE,
                        fill_color: Color::new(18. / 255., 52. / 255., 86. / 255., v),
                        polygon_points: Some(vec![0., 0., 100., 0., 50., 80.]),
                        sides: 5,
                        start_angle: -90.,
                        ..Default::default()
                    },
                );
            }
            "color" => {
                store.set_control_color(id, Color::new(18. / 255., 52. / 255., 86. / 255., v), None)
            }
            "stroke" => {
                let ControlData::Graph(data) = store.object(id).unwrap().control() else {
                    unreachable!()
                };
                let mut data = data.clone();
                data.line_color = Color::new(1., 1., 1., v);
                store.set_graph(id, data);
            }
            _ => panic!("{operation}"),
        }
        let expected: Vec<f32> = expected
            .split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect();
        assert!(
            (store.object(id).unwrap().alpha() - expected[0]).abs() < 1e-6,
            "kind {kind}, {operation}: expected {}, got {}",
            expected[0],
            store.object(id).unwrap().alpha()
        );
        let object = store.object(id).unwrap();
        let ControlData::Graph(data) = object.control() else {
            unreachable!()
        };
        let drawn = data.graph_type != 0 && object.width() != 0. && object.height() != 0.;
        let fill = if drawn { data.drawn_fill_color().a } else { 0. };
        let stroke = if drawn { data.line_color.a } else { 0. };
        assert!(
            (fill - expected[1]).abs() < 1e-6,
            "kind {kind}, {operation}: fill"
        );
        assert!(
            (stroke - expected[2]).abs() < 1e-6,
            "kind {kind}, {operation}: stroke"
        );
        states += 1;
    }
    assert_eq!(states, 48);
}

#[test]
fn exported_graph_fill_alpha_is_applied_before_materialization() {
    let package = crate::package::FuiPackage::parse(
        "Basics",
        include_bytes!("../../tests/fixtures/Basics.fui"),
    )
    .unwrap();
    let mut store = ObjectStore::new();
    let root = crate::widget::create_from_package(
        &mut store,
        &package,
        package.item_index_by_name("Demo_Graph").unwrap(),
        &crate::package::LoadContext::none(),
    )
    .unwrap();
    let graph = store.child_by_name(root, "n15").unwrap();
    let object = store.object(graph).unwrap();
    let ControlData::Graph(data) = object.control() else {
        unreachable!()
    };
    assert_eq!(data.fill_color.a, 104. / 255.);
    assert_eq!(object.alpha(), 104. / 255.);
    assert_eq!(data.drawn_fill_color().a, 1.);
}

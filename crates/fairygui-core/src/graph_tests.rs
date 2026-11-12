use super::*;

#[test]
fn native_polygon_points_and_stroke_mesh_match_reference() {
    let mut name = "";
    let mut size = Size::ZERO;
    let mut data = GraphData::default();
    for line in include_str!("../tests/fixtures/polygon-reference.txt")
        .lines()
        .filter(|line| !line.starts_with('#'))
    {
        let mut parts = line.split_whitespace();
        match parts.next().unwrap() {
            "case" => {
                name = parts.next().unwrap();
                let numbers: Vec<f32> = parts.map(|v| v.parse().unwrap()).collect();
                size = Size::new(numbers[1], numbers[2]);
                data = GraphData {
                    graph_type: numbers[0] as i32,
                    line_size: numbers[3] as i32,
                    sides: numbers[4] as i32,
                    start_angle: numbers[5],
                    ..Default::default()
                };
            }
            "raw" => data.polygon_points = Some(parts.map(|v| v.parse().unwrap()).collect()),
            "dist" => {
                let distances: Vec<f32> = parts.map(|v| v.parse().unwrap()).collect();
                data.distances = (!distances.is_empty()).then_some(distances);
            }
            "hit" => {
                let x = parts.next().unwrap().parse().unwrap();
                let y = parts.next().unwrap().parse().unwrap();
                let expected = parts.next().unwrap() == "1";
                assert_eq!(
                    polygon_contains(&polygon_points(&data, size), Point::new(x, y)),
                    expected,
                    "{name} hit {x},{y}"
                );
            }
            kind => {
                let points = polygon_points(&data, size);
                if kind == "indices" {
                    let expected: Vec<usize> = parts.map(|v| v.parse().unwrap()).collect();
                    assert_eq!(
                        polygon_stroke(&points, data.line_size as f32).indices,
                        expected,
                        "{name} indices"
                    );
                    continue;
                }
                let expected: Vec<f64> = parts.map(|v| v.parse().unwrap()).collect();
                let actual: Vec<f64> = if kind == "vertices" {
                    polygon_stroke(&points, data.line_size as f32)
                        .vertices
                        .iter()
                        .flat_map(|p| [f64::from(p.x), f64::from(p.y)])
                        .collect()
                } else {
                    assert_eq!(kind, "points");
                    points.iter().flat_map(|p| [p.x, p.y]).collect()
                };
                assert_eq!(actual.len(), expected.len(), "{name} {kind}");
                for (i, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                    assert!(
                        (actual - expected).abs() < 0.0002,
                        "{name} {kind}[{i}]: {actual} != {expected}"
                    );
                }
            }
        }
    }
}

#[test]
fn polygon_hits_follow_concavity_and_ignore_fill_and_stroke_style() {
    let data = GraphData {
        graph_type: 3,
        polygon_points: Some(vec![
            0.0, 0.0, 100.0, 0.0, 100.0, 30.0, 30.0, 30.0, 30.0, 100.0, 0.0, 100.0,
        ]),
        ..Default::default()
    };
    let points = polygon_points(&data, Size::new(100.0, 100.0));
    assert!(polygon_contains(&points, Point::new(70.0, 15.0)));
    assert!(polygon_contains(&points, Point::new(15.0, 70.0)));
    assert!(!polygon_contains(&points, Point::new(70.0, 70.0)));
    assert_eq!(polygon_points(&data, Size::new(200.0, 300.0)), points);
    assert!(
        polygon_stroke(&[PolygonPoint::new(0.0, 0.0); 2], 3.0)
            .indices
            .is_empty()
    );
}

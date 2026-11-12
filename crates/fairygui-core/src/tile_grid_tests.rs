use super::*;
fn sample(indice: i32) -> Vec<TileGridCell> {
    split(
        Rect::new(40.0, 40.0, 20.0, 20.0),
        Size::new(100.0, 100.0),
        Size::new(300.0, 200.0),
        indice,
    )
}
#[test]
fn corners_keep_source_size_and_never_tile() {
    let cells = sample(-1);
    assert_eq!(cells[0].dest, Rect::new(0.0, 0.0, 40.0, 40.0));
    assert_eq!(cells[8].dest, Rect::new(260.0, 160.0, 40.0, 40.0));
    assert!([0, 2, 6, 8].iter().all(|&i| !cells[i].tiled));
}
#[test]
fn middle_stretches_to_remaining_area() {
    assert_eq!(sample(0)[4].dest, Rect::new(40.0, 40.0, 220.0, 120.0));
}
#[test]
fn cells_cover_both_target_axes() {
    let cells = sample(0);
    assert_eq!(cells[..3].iter().map(|c| c.dest.width).sum::<f32>(), 300.0);
    assert_eq!(
        [0, 3, 6].iter().map(|&i| cells[i].dest.height).sum::<f32>(),
        200.0
    );
}
#[test]
fn native_five_bits_select_edges_then_center() {
    for (bit, index) in [(0, 1), (1, 7), (2, 3), (3, 5), (4, 4)] {
        let cells = sample(1 << bit);
        assert_eq!(cells.iter().filter(|c| c.tiled).count(), 1);
        assert!(cells[index].tiled);
    }
}
#[test]
fn small_target_compresses_borders_and_has_no_center() {
    let cells = split(
        Rect::new(10.0, 15.0, 70.0, 60.0),
        Size::new(100.0, 100.0),
        Size::new(15.0, 100.0),
        0,
    );
    assert_eq!(cells[0].dest, Rect::new(0.0, 0.0, 5.0, 15.0));
    assert_eq!(cells[1].dest.width, 0.0);
    assert_eq!(cells[2].dest.width, 10.0);
}
#[test]
fn tile_tail_is_cropped_in_both_position_and_source() {
    let cell = TileGridCell {
        dest: Rect::new(4.0, 7.0, 45.0, 25.0),
        src: Rect::new(20.0, 30.0, 20.0, 10.0),
        tiled: true,
        tile_size: Size::new(20.0, 10.0),
    };
    assert_eq!(tile_count(&cell, false), 3);
    assert_eq!(tile_count(&cell, true), 3);
    assert_eq!(
        tiles(&cell).last(),
        Some(&(
            Rect::new(44.0, 27.0, 5.0, 5.0),
            Rect::new(20.0, 30.0, 5.0, 5.0)
        ))
    );
}
#[test]
fn empty_targets_and_zero_tile_steps_emit_no_tiles() {
    assert!(
        split(
            Rect::new(4.0, 4.0, 2.0, 2.0),
            Size::new(10.0, 10.0),
            Size::ZERO,
            31
        )
        .is_empty()
    );
    let mut cell = sample(31)[4];
    cell.tile_size = Size::ZERO;
    assert!(tiles(&cell).is_empty());
}

#[test]
fn unmodified_native_mesh_axes_and_tile_quads_match() {
    let mut actual_axes = None;
    let mut actual_quads = Vec::new();
    let mut cursor = 0;
    let mut case = "";
    for line in include_str!("../tests/fixtures/nine-grid-reference.txt")
        .lines()
        .filter(|l| !l.starts_with('#'))
    {
        if let Some(input) = line.strip_prefix("case ") {
            if !actual_quads.is_empty() {
                assert_eq!(cursor, actual_quads.len(), "{case}: unconsumed quads");
            }
            case = input;
            cursor = 0;
            let n: Vec<f32> = input
                .split_whitespace()
                .map(|n| n.parse().unwrap())
                .collect();
            let source = Size::new(n[0], n[1]);
            let grid = Rect::new(n[2], n[3], n[4], n[5]);
            let target = Size::new(n[6], n[7]);
            let uv = Rect::new(
                (n[13] - n[9]) / n[15],
                (n[14] - n[10]) / n[16],
                n[0] / n[15],
                n[1] / n[16],
            );
            let clip = Rect::new(n[13] / n[15], n[14] / n[16], n[11] / n[15], n[12] / n[16]);
            actual_axes = Some(axes(
                grid,
                target,
                GridTexture {
                    source,
                    virtual_uv: uv,
                    clip_uv: clip,
                    atlas: Size::new(n[15], n[16]),
                },
            ));
            actual_quads = if n[8] == 0.0 {
                Vec::new()
            } else {
                let axes = actual_axes.unwrap();
                split(grid, source, target, n[8] as i32)
                    .into_iter()
                    .enumerate()
                    .flat_map(|(index, mut cell)| {
                        let col = index % 3;
                        let row = index / 3;
                        cell.src = Rect::new(
                            ((axes.u[col] - f64::from(uv.x)) * f64::from(n[15])) as f32,
                            ((axes.v[row] - f64::from(uv.y)) * f64::from(n[16])) as f32,
                            ((axes.u[col + 1] - axes.u[col]) * f64::from(n[15])) as f32,
                            ((axes.v[row + 1] - axes.v[row]) * f64::from(n[16])) as f32,
                        );
                        tiles(&cell)
                    })
                    .collect()
            };
        } else if let Some(values) = line.strip_prefix("axes ") {
            let expected: Vec<f64> = values
                .split_whitespace()
                .map(|n| n.parse().unwrap())
                .collect();
            let actual = actual_axes.unwrap();
            for (index, (a, b)) in actual
                .x
                .into_iter()
                .chain(actual.y)
                .chain(actual.u)
                .chain(actual.v)
                .zip(expected)
                .enumerate()
            {
                assert!((a - b).abs() < 1e-6, "{case} axis[{index}] {a} != {b}");
            }
        } else if let Some(values) = line.strip_prefix("quad ") {
            let expected: Vec<f32> = values
                .split_whitespace()
                .map(|n| n.parse().unwrap())
                .collect();
            let (d, s) = actual_quads[cursor];
            cursor += 1;
            for (index, (a, b)) in [d.x, d.y, d.width, d.height, s.x, s.y, s.width, s.height]
                .into_iter()
                .zip(expected)
                .enumerate()
            {
                assert!(
                    (a - b).abs() < 0.0001,
                    "{case} quad {} [{index}] {a} != {b}",
                    cursor - 1
                );
            }
        } else if let Some(values) = line.strip_prefix("image ") {
            let numbers: Vec<i32> = values
                .split_whitespace()
                .map(|n| n.parse().unwrap())
                .collect();
            assert_eq!(numbers[5], i32::from(numbers[0] != 0));
        } else {
            panic!("unknown trace line {line}");
        }
    }
    assert_eq!(cursor, actual_quads.len());
}

use super::*;

#[test]
fn all_native_fill_mask_coordinates_match_unmodified_fill_utils() {
    for line in include_str!("../tests/fixtures/fill-reference.txt")
        .lines()
        .filter(|line| !line.starts_with('#'))
    {
        let (input, expected) = line.split_once(" |").unwrap();
        let input: Vec<_> = input.split_whitespace().collect();
        let method = FillMethod::from_code(input[0].parse().unwrap());
        let origin = input[1].parse().unwrap();
        let cw = input[2] == "1";
        let amount: f64 = input[3].parse().unwrap();
        let size = Size::new(input[4].parse().unwrap(), input[5].parse().unwrap());
        let actual: Vec<_> = fill_polygon(size, method, origin, cw, amount)
            .iter()
            .flat_map(|p| [p.x, p.y])
            .collect();
        let expected: Vec<f64> = expected
            .split_whitespace()
            .map(|p| p.parse().unwrap())
            .collect();
        assert_eq!(actual.len(), expected.len(), "{input:?}");
        for (i, (actual, expected)) in actual.iter().zip(expected).enumerate() {
            assert!(
                (actual - expected).abs() < 1e-8,
                "{input:?} [{i}]: {actual} != {expected}"
            );
        }
    }
}

#[test]
fn zero_dimensions_and_nonfinite_amounts_have_no_fill() {
    for size in [Size::ZERO, Size::new(0.0, 100.0), Size::new(100.0, 0.0)] {
        assert!(fill_polygon(size, FillMethod::Radial360, 0, true, 0.5).is_empty());
    }
    assert!(
        fill_polygon(
            Size::new(10.0, 10.0),
            FillMethod::Radial90,
            0,
            true,
            f32::NAN
        )
        .is_empty()
    );
}

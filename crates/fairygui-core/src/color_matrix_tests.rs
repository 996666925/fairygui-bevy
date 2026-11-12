use super::*;

#[test]
fn native_parameters_have_zero_defaults_additive_brightness_and_distinct_gray_weights() {
    use crate::{
        object::ObjectStore,
        package::ObjectType,
        transition::{ActionType, TransitionValue},
    };
    assert!(ColorFilterParams::default().is_noop());
    assert!(ColorFilterParams::default().to_matrix().is_identity());
    let matrix = ColorFilterParams {
        brightness: 0.25,
        ..Default::default()
    }
    .to_matrix();
    assert_eq!(
        matrix.apply(Color::new(0.0, 0.25, 0.5, 0.4)),
        Color::new(0.25, 0.5, 0.75, 0.4)
    );
    let contrast = ColorFilterParams {
        contrast: -1.0,
        ..Default::default()
    }
    .to_matrix();
    assert_eq!(contrast.apply(Color::WHITE).r, 128.0 / 255.0);
    let desaturated = ColorFilterParams {
        saturation: -1.0,
        ..Default::default()
    }
    .to_matrix();
    assert_eq!(desaturated.apply(Color::new(1.0, 0.0, 0.0, 1.0)).r, 0.299);
    assert_eq!(
        ColorMatrix::LAYA_GRAY
            .apply(Color::new(1.0, 0.0, 0.0, 1.0))
            .r,
        0.3086
    );
    let mut store = ObjectStore::new();
    let id = store.create(ObjectType::Image);
    let start = TransitionValue::ColorFilter([0.0; 4]);
    let end = TransitionValue::ColorFilter([0.0, 0.0, -1.0, 0.0]);
    start
        .lerp(&end, 0.5)
        .apply(&mut store, id, ActionType::ColorFilter);
    let mid = store
        .object(id)
        .unwrap()
        .color_filter()
        .unwrap()
        .to_matrix()
        .apply(Color::new(1.0, 0.0, 0.0, 0.5));
    assert!((mid.r - 0.6495).abs() < 1e-6 && (mid.g - 0.1495).abs() < 1e-6);
    assert_eq!(mid.a, 0.5);
}

#[test]
fn executable_native_color_matrices_samples_and_legacy_grayed_policy_match() {
    let mut matrix = ColorMatrix::IDENTITY;
    let mut cases = 0;
    let mut samples = 0;
    let mut states = 0;
    for line in include_str!("../tests/fixtures/color-filter-reference.txt").lines() {
        if line.starts_with('#') {
            continue;
        }
        let words: Vec<_> = line.split_whitespace().collect();
        let numbers = |from| {
            words[from..]
                .iter()
                .map(|s| s.parse::<f32>().unwrap())
                .collect::<Vec<_>>()
        };
        match words[0] {
            "case" => {
                let p = numbers(2);
                matrix = ColorFilterParams {
                    brightness: p[0],
                    contrast: p[1],
                    saturation: p[2],
                    hue: p[3],
                }
                .to_matrix();
                cases += 1;
            }
            "matrix" => {
                for (got, expected) in matrix.values.into_iter().zip(numbers(1)) {
                    assert!(
                        (got - expected).abs() < 2e-6,
                        "case {cases}: {got} != {expected}"
                    );
                }
            }
            "sample" => {
                let p = numbers(1);
                let c = matrix.apply(Color::new(p[0], p[1], p[2], p[3]));
                for (got, expected) in [c.r, c.g, c.b, c.a].into_iter().zip(p[4..].iter().copied())
                {
                    assert!(
                        (got - expected).abs() < 2e-6,
                        "case {cases} sample {samples}: {got} != {expected}"
                    );
                }
                samples += 1;
            }
            "policy" => {
                let p = match words[1] {
                    "color" => [0.15, -0.2, 0.7, 0.4],
                    "restored" => [-0.3, 0.2, -0.4, -0.8],
                    _ => [0.0; 4],
                };
                let m = if matches!(words[1], "grayed" | "changed_while_gray") {
                    ColorMatrix::LAYA_GRAY
                } else {
                    ColorFilterParams {
                        brightness: p[0],
                        contrast: p[1],
                        saturation: p[2],
                        hue: p[3],
                    }
                    .to_matrix()
                };
                for (got, expected) in m.values.into_iter().zip(numbers(2)) {
                    assert!((got - expected).abs() < 2e-6);
                }
                states += 1;
            }
            _ => panic!("unexpected reference line: {line}"),
        }
    }
    assert_eq!((cases, samples, states), (48, 336, 5));
}

//! 缓动函数。
//!
//! 类型编号与官方 `EaseType` 一致，因此描述里记录的类型可以直接使用。

use std::f32::consts::PI;

/// 缓动类型编号。
pub type EaseType = i32;

/// 弹性与回弹类缓动使用的默认超调幅度。
const DEFAULT_AMPLITUDE: f32 = 1.70158;

/// 按类型计算缓动后的进度。
///
/// `time` 是已过去的时长，`duration` 是总时长，返回值在 0~1 之间
/// （弹性与回弹类会短暂超出这个范围，这是它们的特性）。
pub fn evaluate(ease_type: EaseType, time: f32, duration: f32) -> f32 {
    if duration <= 0.0 {
        return 1.0;
    }

    let t = (time / duration).clamp(0.0, 1.0);

    match ease_type {
        1 => 1.0 - (t * PI / 2.0).cos(),
        2 => (t * PI / 2.0).sin(),
        3 => -((PI * t).cos() - 1.0) / 2.0,

        4 => t * t,
        5 => -t * (t - 2.0),
        6 => {
            let t = t * 2.0;
            if t < 1.0 {
                t * t / 2.0
            } else {
                let t = t - 2.0;
                (t * t - 2.0) / -2.0
            }
        }

        7 => t * t * t,
        8 => {
            let t = t - 1.0;
            t * t * t + 1.0
        }
        9 => {
            let t = t * 2.0;
            if t < 1.0 {
                t * t * t / 2.0
            } else {
                let t = t - 2.0;
                (t * t * t + 2.0) / 2.0
            }
        }

        10 => t * t * t * t,
        11 => {
            let t = t - 1.0;
            1.0 - t * t * t * t
        }
        12 => {
            let t = t * 2.0;
            if t < 1.0 {
                t * t * t * t / 2.0
            } else {
                let t = t - 2.0;
                (t * t * t * t - 2.0) / -2.0
            }
        }

        13 => t * t * t * t * t,
        14 => {
            let t = t - 1.0;
            t * t * t * t * t + 1.0
        }
        15 => {
            let t = t * 2.0;
            if t < 1.0 {
                t * t * t * t * t / 2.0
            } else {
                let t = t - 2.0;
                (t * t * t * t * t + 2.0) / 2.0
            }
        }

        16 => {
            if t == 0.0 {
                0.0
            } else {
                2.0f32.powf(10.0 * (t - 1.0))
            }
        }
        17 => {
            if t == 1.0 {
                1.0
            } else {
                1.0 - 2.0f32.powf(-10.0 * t)
            }
        }
        18 => {
            if t == 0.0 {
                0.0
            } else if t == 1.0 {
                1.0
            } else {
                let t = t * 2.0;
                if t < 1.0 {
                    2.0f32.powf(10.0 * (t - 1.0)) / 2.0
                } else {
                    (2.0 - 2.0f32.powf(-10.0 * (t - 1.0))) / 2.0
                }
            }
        }

        19 => 1.0 - (1.0 - t * t).sqrt(),
        20 => (1.0 - (t - 1.0) * (t - 1.0)).sqrt(),
        21 => {
            let t = t * 2.0;
            if t < 1.0 {
                (1.0 - (1.0 - t * t).sqrt()) / 2.0
            } else {
                let t = t - 2.0;
                ((1.0 - t * t).sqrt() + 1.0) / 2.0
            }
        }

        22 => elastic(t, DEFAULT_AMPLITUDE, 0.3, true),
        23 => elastic(t, DEFAULT_AMPLITUDE, 0.3, false),
        24 => {
            if t < 0.5 {
                elastic(t * 2.0, DEFAULT_AMPLITUDE, 0.45, true) / 2.0
            } else {
                elastic(t * 2.0 - 1.0, DEFAULT_AMPLITUDE, 0.45, false) / 2.0 + 0.5
            }
        }

        25 => back_in(t, DEFAULT_AMPLITUDE),
        26 => back_out(t, DEFAULT_AMPLITUDE),
        27 => back_in_out(t, DEFAULT_AMPLITUDE),

        28 => 1.0 - bounce_out(1.0 - t),
        29 => bounce_out(t),
        30 => {
            if t < 0.5 {
                (1.0 - bounce_out(1.0 - t * 2.0)) / 2.0
            } else {
                bounce_out(t * 2.0 - 1.0) / 2.0 + 0.5
            }
        }

        // 0 与未知类型都按线性处理。
        _ => t,
    }
}

fn elastic(t: f32, amplitude: f32, period: f32, entering: bool) -> f32 {
    if t == 0.0 || t == 1.0 {
        return t;
    }

    let s = period / 4.0;
    let value = amplitude * 2.0f32.powf(-10.0 * t) * ((t - s) * (2.0 * PI) / period).sin() + 1.0;

    if entering { value } else { 1.0 - (value - 1.0) }
}

fn back_in(t: f32, overshoot: f32) -> f32 {
    t * t * ((overshoot + 1.0) * t - overshoot)
}

fn back_out(t: f32, overshoot: f32) -> f32 {
    let t = t - 1.0;
    t * t * ((overshoot + 1.0) * t + overshoot) + 1.0
}

fn back_in_out(t: f32, overshoot: f32) -> f32 {
    let overshoot = overshoot * 1.525;
    let t = t * 2.0;
    if t < 1.0 {
        (t * t * ((overshoot + 1.0) * t - overshoot)) / 2.0
    } else {
        let t = t - 2.0;
        (t * t * ((overshoot + 1.0) * t + overshoot) + 2.0) / 2.0
    }
}

fn bounce_out(t: f32) -> f32 {
    if t < 1.0 / 2.75 {
        7.5625 * t * t
    } else if t < 2.0 / 2.75 {
        let t = t - 1.5 / 2.75;
        7.5625 * t * t + 0.75
    } else if t < 2.5 / 2.75 {
        let t = t - 2.25 / 2.75;
        7.5625 * t * t + 0.9375
    } else {
        let t = t - 2.625 / 2.75;
        7.5625 * t * t + 0.984375
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints_are_always_reached() {
        for ease_type in 0..=30 {
            let start = evaluate(ease_type, 0.0, 1.0);
            let end = evaluate(ease_type, 1.0, 1.0);

            assert!(
                start.abs() < 1e-4,
                "类型 {ease_type} 的起点应当为 0，实际 {start}"
            );
            assert!(
                (end - 1.0).abs() < 1e-4,
                "类型 {ease_type} 的终点应当为 1，实际 {end}"
            );
        }
    }

    #[test]
    fn linear_matches_progress() {
        assert!((evaluate(0, 0.25, 1.0) - 0.25).abs() < 1e-6);
        assert!((evaluate(0, 0.75, 1.0) - 0.75).abs() < 1e-6);
    }

    #[test]
    fn quad_in_starts_slow() {
        // 二次缓入在前半段应当落后于线性。
        assert!(evaluate(4, 0.25, 1.0) < 0.25);
    }

    #[test]
    fn quad_out_starts_fast() {
        assert!(evaluate(5, 0.25, 1.0) > 0.25);
    }

    #[test]
    fn elastic_and_back_overshoot() {
        // 这两类会短暂越过终点，这是它们的特性而非缺陷。
        let peak = (0..=100)
            .map(|i| evaluate(26, i as f32 / 100.0, 1.0))
            .fold(f32::MIN, f32::max);
        assert!(peak > 1.0, "回弹类应当有超调");
    }

    #[test]
    fn zero_duration_finishes_immediately() {
        assert_eq!(evaluate(4, 0.0, 0.0), 1.0);
    }

    #[test]
    fn unknown_type_falls_back_to_linear() {
        assert!((evaluate(999, 0.3, 1.0) - 0.3).abs() < 1e-6);
    }

    #[test]
    fn progress_is_clamped_to_the_duration() {
        // 超出时长时不应继续外推。
        let beyond = evaluate(4, 5.0, 1.0);
        let at_end = evaluate(4, 1.0, 1.0);
        assert!((beyond - at_end).abs() < 1e-6);
    }
}

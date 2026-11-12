//! FairyGUI display/FillUtils polygon masks (MIT, fairygui.com).
use crate::{Size, graph::PolygonPoint, widget::FillMethod};
type Points = Vec<PolygonPoint>;
fn points(values: &[[f64; 2]]) -> Points {
    values
        .iter()
        .map(|p| PolygonPoint::new(p[0], p[1]))
        .collect()
}
fn move_points(points: &mut Points, x: f64, y: f64) {
    for p in points {
        p.x += x;
        p.y += y;
    }
}
fn append(points: &mut Points, values: &[[f64; 2]]) {
    points.extend(values.iter().map(|p| PolygonPoint::new(p[0], p[1])));
}

/// Image.doFill's local mask. It crops the drawn texture without changing its
/// layout/input rectangle. Near-complete amounts follow native's 0.9999 cutoff.
pub fn fill_polygon(
    size: Size,
    method: FillMethod,
    origin: u8,
    clockwise: bool,
    amount: impl Into<f64>,
) -> Points {
    let w = f64::from(size.width);
    let h = f64::from(size.height);
    let a = amount.into();
    if w <= 0.0 || h <= 0.0 || !a.is_finite() {
        return Vec::new();
    }
    if method == FillMethod::None || a >= 0.9999 {
        return points(&[[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]]);
    }
    if a <= 0.0 {
        return Vec::new();
    }
    match method {
        FillMethod::Horizontal => {
            let w2 = w * a;
            if origin == 0 || origin == 2 {
                points(&[[0.0, 0.0], [w2, 0.0], [w2, h], [0.0, h]])
            } else {
                points(&[[w, 0.0], [w, h], [w - w2, h], [w - w2, 0.0]])
            }
        }
        FillMethod::Vertical => {
            let h2 = h * a;
            if origin == 0 || origin == 2 {
                points(&[[0.0, 0.0], [0.0, h2], [w, h2], [w, 0.0]])
            } else {
                points(&[[0.0, h], [w, h], [w, h - h2], [0.0, h - h2]])
            }
        }
        FillMethod::Radial90 => radial90(w, h, origin, clockwise, a),
        FillMethod::Radial180 => radial180(w, h, origin, clockwise, a),
        FillMethod::Radial360 => radial360(w, h, origin, clockwise, a),
        _ => Vec::new(),
    }
}

fn radial90(w: f64, h: f64, origin: u8, cw: bool, mut a: f64) -> Points {
    if cw && (origin == 1 || origin == 2) || !cw && (origin == 0 || origin == 3) {
        a = 1.0 - a;
    }
    let h2 = w * (std::f64::consts::FRAC_PI_2 * a).tan();
    let v2 = (h2 - h) / h2;
    match (origin, cw, h2 <= h) {
        (0, true, true) => points(&[[0.0, 0.0], [w, h2], [w, 0.0]]),
        (0, true, false) => points(&[[0.0, 0.0], [w * (1.0 - v2), h], [w, h], [w, 0.0]]),
        (0, false, true) => points(&[[0.0, 0.0], [w, h2], [w, h], [0.0, h]]),
        (0, false, false) => points(&[[0.0, 0.0], [w * (1.0 - v2), h], [0.0, h]]),
        (1, true, true) => points(&[[w, 0.0], [0.0, h2], [0.0, h], [w, h]]),
        (1, true, false) => points(&[[w, 0.0], [w * v2, h], [w, h]]),
        (1, false, true) => points(&[[w, 0.0], [0.0, h2], [0.0, 0.0]]),
        (1, false, false) => points(&[[w, 0.0], [w * v2, h], [0.0, h], [0.0, 0.0]]),
        (2, true, true) => points(&[[0.0, h], [w, h - h2], [w, 0.0], [0.0, 0.0]]),
        (2, true, false) => points(&[[0.0, h], [w * (1.0 - v2), 0.0], [0.0, 0.0]]),
        (2, false, true) => points(&[[0.0, h], [w, h - h2], [w, h]]),
        (2, false, false) => points(&[[0.0, h], [w * (1.0 - v2), 0.0], [w, 0.0], [w, h]]),
        (3, true, true) => points(&[[w, h], [0.0, h - h2], [0.0, h]]),
        (3, true, false) => points(&[[w, h], [w * v2, 0.0], [0.0, 0.0], [0.0, h]]),
        (3, false, true) => points(&[[w, h], [0.0, h - h2], [0.0, 0.0], [w, 0.0]]),
        (3, false, false) => points(&[[w, h], [w * v2, 0.0], [w, 0.0]]),
        _ => Vec::new(),
    }
}

fn radial180(w: f64, h: f64, origin: u8, cw: bool, a: f64) -> Points {
    let first = a <= 0.5;
    let a = if first { a / 0.5 } else { (a - 0.5) / 0.5 };
    match origin {
        0 => {
            let mut p = radial90(w / 2.0, h, if first == cw { 0 } else { 1 }, cw, a);
            if first {
                if cw {
                    move_points(&mut p, w / 2.0, 0.0);
                }
            } else if cw {
                append(&mut p, &[[w, h], [w, 0.0]]);
            } else {
                move_points(&mut p, w / 2.0, 0.0);
                append(&mut p, &[[0.0, h], [0.0, 0.0]]);
            }
            p
        }
        1 => {
            let mut p = radial90(w / 2.0, h, if first == cw { 3 } else { 2 }, cw, a);
            if first {
                if !cw {
                    move_points(&mut p, w / 2.0, 0.0);
                }
            } else if cw {
                move_points(&mut p, w / 2.0, 0.0);
                append(&mut p, &[[0.0, 0.0], [0.0, h]]);
            } else {
                append(&mut p, &[[w, 0.0], [w, h]]);
            }
            p
        }
        2 => {
            let mut p = radial90(w, h / 2.0, if first == cw { 2 } else { 0 }, cw, a);
            if first {
                if !cw {
                    move_points(&mut p, 0.0, h / 2.0);
                }
            } else if cw {
                move_points(&mut p, 0.0, h / 2.0);
                append(&mut p, &[[w, 0.0], [0.0, 0.0]]);
            } else {
                append(&mut p, &[[w, h], [0.0, h]]);
            }
            p
        }
        3 => {
            let mut p = radial90(w, h / 2.0, if first == cw { 1 } else { 3 }, cw, a);
            if first {
                if cw {
                    move_points(&mut p, 0.0, h / 2.0);
                }
            } else if cw {
                append(&mut p, &[[0.0, h], [w, h]]);
            } else {
                move_points(&mut p, 0.0, h / 2.0);
                append(&mut p, &[[0.0, 0.0], [w, 0.0]]);
            }
            p
        }
        _ => Vec::new(),
    }
}

fn radial360(w: f64, h: f64, origin: u8, cw: bool, a: f64) -> Points {
    let first = a <= 0.5;
    let a = if first { a / 0.5 } else { (a - 0.5) / 0.5 };
    match origin {
        0 => {
            let mut p = radial180(w / 2.0, h, if first == cw { 2 } else { 3 }, cw, a);
            if first {
                if cw {
                    move_points(&mut p, w / 2.0, 0.0);
                }
            } else if cw {
                append(&mut p, &[[w, h], [w, 0.0], [w / 2.0, 0.0]]);
            } else {
                move_points(&mut p, w / 2.0, 0.0);
                append(&mut p, &[[0.0, h], [0.0, 0.0], [w / 2.0, 0.0]]);
            }
            p
        }
        1 => {
            let mut p = radial180(w / 2.0, h, if first == cw { 3 } else { 2 }, cw, a);
            if first {
                if !cw {
                    move_points(&mut p, w / 2.0, 0.0);
                }
            } else if cw {
                move_points(&mut p, w / 2.0, 0.0);
                append(&mut p, &[[0.0, 0.0], [0.0, h], [w / 2.0, h]]);
            } else {
                append(&mut p, &[[w, 0.0], [w, h], [w / 2.0, h]]);
            }
            p
        }
        2 => {
            let mut p = radial180(w, h / 2.0, if first == cw { 1 } else { 0 }, cw, a);
            if first {
                if !cw {
                    move_points(&mut p, 0.0, h / 2.0);
                }
            } else if cw {
                move_points(&mut p, 0.0, h / 2.0);
                append(&mut p, &[[w, 0.0], [0.0, 0.0], [0.0, h / 2.0]]);
            } else {
                append(&mut p, &[[w, h], [0.0, h], [0.0, h / 2.0]]);
            }
            p
        }
        3 => {
            let mut p = radial180(w, h / 2.0, if first == cw { 0 } else { 1 }, cw, a);
            if first {
                if cw {
                    move_points(&mut p, 0.0, h / 2.0);
                }
            } else if cw {
                append(&mut p, &[[0.0, h], [w, h], [w, h / 2.0]]);
            } else {
                move_points(&mut p, 0.0, h / 2.0);
                append(&mut p, &[[0.0, 0.0], [w, 0.0], [w, h / 2.0]]);
            }
            p
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
#[path = "fill_tests.rs"]
mod tests;

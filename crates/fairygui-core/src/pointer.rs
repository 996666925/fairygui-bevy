//! Native Laya contact release bookkeeping, independent of event recipients.
use crate::Point;

/// Laya's double-click interval and per-axis pointer distance, in milliseconds
/// and logical stage pixels. Release distance is inclusive; repeated-click
/// distance and elapsed time are strict comparisons.
pub const DOUBLE_CLICK_INTERVAL_MS: f64 = 350.;
pub const CLICK_TEST_THRESHOLD: f32 = 10.;

#[derive(Debug, Clone, Copy)]
pub struct ClickRelease {
    pub time_ms: f64,
    pub start: Point,
    pub position: Point,
    pub button: u8,
    pub has_target: bool,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClickEnd {
    pub count: u8,
    pub cancelled: bool,
}

/// One native touch ID's history. It deliberately does not remember a target:
/// a rapid release over another object at the same position is a double click.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClickHistory {
    position: Point,
    time_ms: f64,
    button: u8,
}
impl ClickHistory {
    pub fn end(&mut self, release: ClickRelease) -> ClickEnd {
        let cancelled = release.cancelled
            || !release.has_target
            || (release.position.x - release.start.x).abs() > CLICK_TEST_THRESHOLD
            || (release.position.y - release.start.y).abs() > CLICK_TEST_THRESHOLD;
        if cancelled {
            // Keep the native zero-time sentinel and the old position/button.
            self.time_ms = 0.;
            return ClickEnd {
                count: 1,
                cancelled: true,
            };
        }
        let double = release.time_ms - self.time_ms < DOUBLE_CLICK_INTERVAL_MS
            && (release.position.x - self.position.x).abs() < CLICK_TEST_THRESHOLD
            && (release.position.y - self.position.y).abs() < CLICK_TEST_THRESHOLD
            && self.button == release.button;
        self.time_ms = release.time_ms;
        self.position = release.position;
        self.button = release.button;
        ClickEnd {
            count: if double { 2 } else { 1 },
            cancelled: false,
        }
    }

    /// Expired histories can be reclaimed without changing a later release.
    pub fn is_recent(&self, time_ms: f64) -> bool {
        time_ms - self.time_ms < DOUBLE_CLICK_INTERVAL_MS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_counts_and_cancellation_match_unmodified_native_touch_info() {
        let mut history = ClickHistory::default();
        let mut count = 0;
        for line in include_str!("../tests/fixtures/click-reference.txt").lines() {
            let p: Vec<_> = line.split_whitespace().collect();
            match p.first().copied() {
                Some("case") => {
                    history = ClickHistory::default();
                    count += 1;
                }
                Some("release") => {
                    let number = |i: usize| p[i].parse::<f64>().unwrap();
                    let actual = history.end(ClickRelease {
                        time_ms: number(1),
                        start: Point::new(number(2) as f32, number(3) as f32),
                        position: Point::new(number(4) as f32, number(5) as f32),
                        button: number(6) as u8,
                        has_target: p[7] == "1",
                        cancelled: p[8] == "1",
                    });
                    assert_eq!(
                        actual,
                        ClickEnd {
                            count: number(9) as u8,
                            cancelled: p[10] == "1"
                        },
                        "{line}"
                    );
                    assert_eq!(history.time_ms, number(11), "{line}");
                    assert_eq!(
                        history.position,
                        Point::new(number(12) as f32, number(13) as f32),
                        "{line}"
                    );
                    assert_eq!(history.button, number(14) as u8, "{line}");
                }
                _ => {}
            }
        }
        assert_eq!(count, 110);
    }
}

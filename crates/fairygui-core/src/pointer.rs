//! Native Laya press paths, click recipients and contact release bookkeeping.
use crate::Point;
use crate::object::{GObjectId, ObjectStore};

/// The native press-time ancestor path used by TouchInfo.clickTest. Object IDs
/// retain their generations, so disposal cannot redirect a press to a reused slot.
#[derive(Debug, Default)]
pub struct ClickTargets(Vec<GObjectId>);

/// Native MouseUp delivery: the current staged hit path, followed by saved
/// press recipients absent from that entire path. The two phases have distinct
/// stop-propagation behavior in TouchInfo.bubble.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct PointerReleaseRoute {
    pub bubble: Vec<GObjectId>,
    pub pressed: Vec<GObjectId>,
}

/// Direct hover notifications below the first common ancestor. Every recipient
/// is its own event target; these are independent notifications, not bubbling.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct HoverChange {
    pub exited: Vec<GObjectId>,
    pub entered: Vec<GObjectId>,
}

impl HoverChange {
    /// Native InputManager.handleRollOver reads both current ancestor chains.
    /// Existing outgoing nodes are notified even if off stage. Incoming nodes
    /// must still be staged. A disposed previous target has no remaining chain.
    pub fn between(
        store: &ObjectStore,
        previous: Option<GObjectId>,
        next: Option<GObjectId>,
    ) -> Self {
        let mut exited = Vec::new();
        let mut current = previous;
        while let Some(id) = current {
            let Some(object) = store.object(id) else {
                break;
            };
            exited.push(id);
            current = object.parent();
        }
        let mut entered = Vec::new();
        current = next;
        while let Some(id) = current {
            if let Some(index) = exited.iter().position(|&old| old == id) {
                exited.truncate(index);
                break;
            }
            let Some(object) = store.object(id) else {
                break;
            };
            entered.push(id);
            current = object.parent();
        }
        entered.retain(|&id| store.is_on_stage(id));
        Self { exited, entered }
    }
}

#[cfg(test)]
#[path = "pointer_hover_tests.rs"]
mod hover_tests;

impl ClickTargets {
    /// Record the logical target and its ancestors before press handlers mutate
    /// the tree. A built-in mouse-down propagation stop bounds this path.
    pub fn begin(store: &ObjectStore, target: GObjectId, stop: Option<GObjectId>) -> Self {
        let mut targets = Vec::new();
        let mut current = Some(target);
        while let Some(id) = current {
            let Some(object) = store.object(id) else {
                break;
            };
            targets.push(id);
            if Some(id) == stop {
                break;
            }
            current = object.parent();
        }
        Self(targets)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Saved press-time recipients, including the mouse-down stop boundary.
    pub fn targets(&self) -> &[GObjectId] {
        &self.0
    }

    /// Build both MouseUp phases without consuming the path needed by clickTest.
    /// Saved press recipients exclude the complete staged release path.
    /// Saved recipients can be off stage. Disposed identities retain their
    /// position because an inherited stop still terminates at that empty slot.
    pub fn release_route(&self, store: &ObjectStore, target: GObjectId) -> PointerReleaseRoute {
        let mut bubble = Vec::new();
        let mut current = Some(target);
        while let Some(id) = current {
            if store.is_on_stage(id) {
                bubble.push(id);
            }
            current = store.object(id).and_then(|object| object.parent());
        }
        let pressed = self
            .0
            .iter()
            .copied()
            .filter(|id| !bubble.contains(id))
            .collect();
        PointerReleaseRoute { bubble, pressed }
    }

    /// Consume one press using native clickTest: prefer the original target
    /// while its display tree remains staged, regardless of the release hit or
    /// its visible/touchable flags. Otherwise walk the release target's current
    /// ancestors to the first staged member of the saved path.
    pub fn resolve(
        &mut self,
        store: &ObjectStore,
        release: Option<GObjectId>,
        cancelled: bool,
    ) -> Option<GObjectId> {
        let targets = std::mem::take(&mut self.0);
        if cancelled {
            return None;
        }
        let &original = targets.first()?;
        if store.is_on_stage(original) {
            return Some(original);
        }
        let mut current = release;
        while let Some(id) = current {
            if targets.contains(&id) && store.is_on_stage(id) {
                return Some(id);
            }
            current = store.object(id).and_then(|object| object.parent());
        }
        None
    }
}

#[cfg(test)]
#[path = "pointer_target_tests.rs"]
mod target_tests;

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

use super::*;
use fairygui_core::pointer::{ClickEnd, ClickHistory, ClickRelease};

/// Separate windows and physical touch IDs cannot combine into a click pair.
/// Mouse buttons share a history, so an intervening other button breaks a pair.
#[derive(Debug, Default)]
pub(super) struct ClickTracker {
    histories: HashMap<(Entity, Option<u64>), ClickHistory>,
}
impl ClickTracker {
    pub fn expire(&mut self, time_ms: f64) {
        self.histories
            .retain(|_, history| history.is_recent(time_ms));
    }

    pub fn end(&mut self, window: Entity, touch: Option<u64>, release: ClickRelease) -> ClickEnd {
        self.histories
            .entry((window, touch))
            .or_default()
            .end(release)
    }
}

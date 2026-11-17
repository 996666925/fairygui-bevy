use super::ScrollPane;

#[cfg(test)]
#[path = "event_tests.rs"]
mod tests;

/// Native target-only ScrollPane notifications. Commands coalesce until the
/// next refresh; accepted drag samples and tween ticks each publish a Scroll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollEvent {
    /// One deferred refresh after one or more position commands. The adapter
    /// acknowledges callback position edits without recursively notifying again.
    Refresh,
    Scroll,
    End,
}

impl ScrollPane {
    pub(super) fn flush_scroll_refresh(&mut self) {
        if std::mem::take(&mut self.motion.scroll_refresh) {
            self.motion.programmatic_tween_announced = self.animation.is_some();
            self.motion.events.push(ScrollEvent::Refresh);
        }
    }

    pub(super) fn scroll_tick(&mut self, ended: bool) {
        self.flush_scroll_refresh();
        self.motion.events.push(ScrollEvent::Scroll);
        if ended {
            self.motion.programmatic_tween_announced = false;
            self.motion.events.push(ScrollEvent::End);
        }
    }

    /// killTween emits Scroll then End for a programmatic tween, and only End
    /// for inertia. Geometry changes and stationary input do not imply an end.
    pub(super) fn interrupt_scroll_events(&mut self) {
        // Position commands are applied eagerly by the model, but native
        // refresh has not started their tween until its first notification.
        // Coalescing an animated command into an immediate one has no End.
        if self.animation.is_some() && self.motion.programmatic_tween_announced
            || self.motion.tween.is_some()
        {
            if self.animation.is_some() {
                self.motion.events.push(ScrollEvent::Scroll);
            }
            self.motion.events.push(ScrollEvent::End);
        }
        self.motion.programmatic_tween_announced = false;
    }

    pub fn drain_scroll_events(&mut self) -> impl Iterator<Item = ScrollEvent> + '_ {
        self.flush_scroll_refresh();
        self.motion.events.drain(..)
    }

    pub(crate) fn acknowledge_scroll_refresh(&mut self) {
        self.motion.scroll_refresh = false;
        self.motion.programmatic_tween_announced = self.animation.is_some();
    }
}

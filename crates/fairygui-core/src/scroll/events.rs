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
        self.emit_scroll_tick(ended, true);
    }

    /// A refresh and the first tween sample may share a backend frame. Native
    /// dispatch already emitted Scroll for the refresh, so suppress the
    /// intermediate duplicate. A final sample still emits its own Scroll
    /// before ScrollEnd, matching tweenUpdate.
    pub(super) fn scroll_tick_after_refresh(&mut self, ended: bool) {
        self.emit_scroll_tick(ended, ended);
    }

    fn emit_scroll_tick(&mut self, ended: bool, emit_scroll: bool) {
        if emit_scroll {
            self.motion.events.push(ScrollEvent::Scroll);
        }
        if ended {
            self.motion.programmatic_tween_announced = false;
            self.motion.events.push(ScrollEvent::End);
        }
    }

    /// killTween emits Scroll then End for a programmatic tween, and only End
    /// for inertia. Geometry changes and stationary input do not imply an end.
    pub(super) fn interrupt_scroll_events(&mut self) {
        // A queued animated request has no started tween to end. Only an
        // already announced programmatic tween or release emits cancellation.
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
        // Native refresh2 runs once more after Scroll callbacks, without a
        // second refresh notification for position writes made by the callback.
        if let Some(animated) = self.motion.pending_command.take() {
            self.apply_command_position(self.pos, animated && self.motion.refresh_animated);
        }
        self.motion.scroll_refresh = false;
        self.motion.programmatic_tween_announced = self.animation.is_some();
    }
}

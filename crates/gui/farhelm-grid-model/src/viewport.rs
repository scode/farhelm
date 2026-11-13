//! A scroll position names retained terminal lines, not their changing indices.
//!
//! Output and cap eviction change the buffer's first index. Holding an absolute
//! anchor keeps the same text visible until it is evicted; following the tail
//! instead tracks the current screen. Relayout and resets invalidate old numbers.

use crate::{Damage, GridModel};

/// One viewport anchor; `None` follows live output at the screen's top line.
/// Eviction past the anchor clamps the view to the oldest retained line without
/// resuming following. Navigation changes that anchor; reset/resize/alt transitions
/// clear it in `synchronize` because those transitions invalidate its line numbers.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Viewport {
    anchor: Option<u64>,
    alternate: bool,
}

impl Viewport {
    /// Apply invalidations once alongside the owner's accumulated model damage.
    /// Size and alternate-screen transitions change which line numbers are useful;
    /// ordinary history appends and eviction deliberately leave the anchor alone.
    pub fn synchronize(&mut self, model: &GridModel, damage: &Damage) -> bool {
        let alternate = model.state().is_some_and(|state| state.alt_screen);
        let reset = damage.size || damage.history_reset || self.alternate != alternate;
        self.alternate = alternate;
        if reset {
            self.anchor = None;
        }
        reset
    }

    /// The oldest line navigation can show; primary history is hidden on alt-screen.
    pub fn first_line(&self, model: &GridModel) -> u64 {
        if model.state().is_some_and(|state| state.alt_screen) {
            model.screen_base()
        } else {
            model
                .history()
                .front()
                .map_or(model.screen_base(), |line| line.number)
        }
    }

    /// Resolve the anchored top against the currently retained range without
    /// rewriting the anchor when eviction moves the oldest retained line past it.
    pub fn top(&self, model: &GridModel) -> u64 {
        self.anchor
            .unwrap_or(model.screen_base())
            .clamp(self.first_line(model), model.screen_base())
    }

    /// Following is an explicit mode, not an equality test against today's bottom.
    pub fn is_following(&self) -> bool {
        self.anchor.is_none()
    }

    /// Move to a visible line; reaching the current screen resumes following.
    pub fn scroll_to(&mut self, model: &GridModel, top: u64) -> bool {
        let top = top.clamp(self.first_line(model), model.screen_base());
        let next = (top < model.screen_base()).then_some(top);
        let changed = self.anchor != next;
        self.anchor = next;
        changed
    }

    /// Scroll signed whole lines without overflowing at either numeric boundary.
    /// Positive deltas move toward live output, negative ones into retained history.
    pub fn scroll_lines(&mut self, model: &GridModel, lines: i64) -> bool {
        if lines == 0 {
            return false;
        }
        let top = self.top(model);
        let next = if lines < 0 {
            top.saturating_sub(lines.unsigned_abs())
        } else {
            top.saturating_add(lines as u64)
        };
        self.scroll_to(model, next)
    }

    /// Input and explicit bottom navigation release an old scroll anchor.
    pub fn follow(&mut self) -> bool {
        self.anchor.take().is_some()
    }
}

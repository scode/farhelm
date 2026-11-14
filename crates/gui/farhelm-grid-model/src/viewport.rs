//! A scroll position names retained terminal lines, not their changing indices.
//!
//! Output and cap eviction change the buffer's first index. Holding an absolute
//! anchor keeps the same text visible until it is evicted; following the tail
//! instead tracks the current screen. Relayout and resets invalidate old numbers.
//!
//! Navigation counts in positions, not line numbers: position `n + 1` is line
//! `n`, so the row above the oldest retained line has a position of its own even
//! when that line is numbered 0. That row is the notice line, shown when recovery
//! lost some history (`history_trimmed`), as the old app's view put it. The
//! supervisor numbers a rebuilt emulator's history from 0, so a recovered
//! terminal's oldest line is usually line 0, and a notice numbered one line
//! before it would land on that line and hide it.

use crate::{Damage, GridModel};

/// What sits at a navigation position: a retained line by its number, or the
/// notice line above the oldest one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewRow {
    Notice,
    Line(u64),
}

/// The navigation position of line `number`.
pub fn position_of(number: u64) -> u64 {
    // Line numbers are bounded by the wire format's safe integers, far below
    // `u64::MAX`; saturating keeps the function total anyway.
    number.saturating_add(1)
}

/// One viewport anchor, a navigation position; `None` follows live output at
/// the screen's top line. Eviction past the anchor clamps the view to the oldest
/// row navigation can show without resuming following. Navigation changes that
/// anchor; reset/resize/alt transitions clear it in `synchronize` because those
/// transitions invalidate its line numbers.
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

    /// The oldest position navigation can show; primary history is hidden on
    /// alt-screen. When recovery lost some history, that is the notice line's
    /// ([`Viewport::notice`]), one before the oldest retained line.
    pub fn first(&self, model: &GridModel) -> u64 {
        if model.state().is_some_and(|state| state.alt_screen) {
            return self.bottom(model);
        }
        let oldest = model
            .history()
            .front()
            .map_or(model.screen_base(), |line| line.number);
        let oldest = position_of(oldest);
        if model.state().is_some_and(|state| state.history_trimmed) {
            oldest - 1
        } else {
            oldest
        }
    }

    /// The position of the screen's top line: the view's top while it follows,
    /// and the bound a scrolled view's top stays below (reaching it follows).
    pub fn bottom(&self, model: &GridModel) -> u64 {
        position_of(model.screen_base())
    }

    /// The notice line's position, when recovery lost some history and the
    /// primary screen is shown.
    pub fn notice(&self, model: &GridModel) -> Option<u64> {
        let trimmed = model
            .state()
            .is_some_and(|state| state.history_trimmed && !state.alt_screen);
        trimmed.then(|| self.first(model))
    }

    /// What position `position` shows above the screen: the notice line, or a
    /// line by its number (which the model may no longer hold).
    pub fn row_at(&self, model: &GridModel, position: u64) -> ViewRow {
        if Some(position) == self.notice(model) {
            ViewRow::Notice
        } else {
            ViewRow::Line(position.saturating_sub(1))
        }
    }

    /// Resolve the anchored top against the currently retained range without
    /// rewriting the anchor when eviction moves the oldest retained line past it.
    pub fn top(&self, model: &GridModel) -> u64 {
        self.anchor
            .unwrap_or(self.bottom(model))
            .clamp(self.first(model), self.bottom(model))
    }

    /// Following is an explicit mode, not an equality test against today's bottom.
    pub fn is_following(&self) -> bool {
        self.anchor.is_none()
    }

    /// Move the top to a position; reaching the current screen resumes following.
    pub fn scroll_to(&mut self, model: &GridModel, top: u64) -> bool {
        let top = top.clamp(self.first(model), self.bottom(model));
        let next = (top < self.bottom(model)).then_some(top);
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

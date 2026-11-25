//! A scroll position names retained terminal lines, not their changing indices.
//!
//! Output and cap eviction change the buffer's first index. Holding an absolute
//! anchor keeps the same text visible until it is evicted; following the tail
//! instead tracks the current screen. Relayout and resets invalidate old numbers.
//!
//! Line `n` starts at position `n + POSITION_PREFIX`, leaving room above line
//! zero for a recovery notice and up to a grid's worth of blank rows. This
//! lets the view scroll its oldest content past the rows clipped by a lift.

use crate::{Damage, GridModel};
use farhelm_proto::grid::GRID_MAX_DIMENSION;

/// Enough room before line zero for a notice and the maximum possible lift.
const POSITION_PREFIX: u64 = GRID_MAX_DIMENSION as u64 + 1;

/// The content at a navigation position, including padding before any retained text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewRow {
    Blank,
    Notice,
    Line(u64),
}

/// The navigation position of line `number`.
pub fn position_of(number: u64) -> u64 {
    // Line numbers are bounded by the wire format's safe integers, far below
    // `u64::MAX`; saturating keeps the function total anyway.
    number.saturating_add(POSITION_PREFIX)
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
    top_padding: u16,
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

    /// The first content position. Primary history is hidden on a live
    /// alternate screen. When recovery lost some history, that is the notice line's
    /// ([`Viewport::notice`]), one before the oldest retained line.
    fn first_content(&self, model: &GridModel) -> u64 {
        if model.hides_primary_history() {
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

    /// The oldest navigation position includes the blank rows needed by the pane's lift.
    pub fn first(&self, model: &GridModel) -> u64 {
        self.first_content(model) - u64::from(self.top_padding)
    }

    /// Rows reserved for the pane's lift. Page navigation subtracts them from the visible row count.
    pub fn top_padding(&self) -> u16 {
        self.top_padding
    }

    /// Reserve room to scroll content into view when the pane clips whole rows.
    /// The anchor stays attached to its line, with the usual clamp to the new range.
    pub fn set_top_padding(&mut self, rows: u16) -> bool {
        let rows = rows.min(GRID_MAX_DIMENSION);
        std::mem::replace(&mut self.top_padding, rows) != rows
    }

    /// The position of the screen's top line: the view's top while it follows,
    /// and the bound a scrolled view's top stays below (reaching it follows).
    pub fn bottom(&self, model: &GridModel) -> u64 {
        position_of(model.screen_base())
    }

    /// The notice line's position when there is a recovery gap above reachable output.
    pub fn notice(&self, model: &GridModel) -> Option<u64> {
        let trimmed = !model.hides_primary_history()
            && model.state().is_some_and(|state| state.history_trimmed);
        trimmed.then(|| self.first_content(model))
    }

    /// Resolve a navigation position to a retained line, the notice or blank padding.
    pub fn row_at(&self, model: &GridModel, position: u64) -> ViewRow {
        if position < self.first_content(model) {
            ViewRow::Blank
        } else if Some(position) == self.notice(model) {
            ViewRow::Notice
        } else {
            ViewRow::Line(position - POSITION_PREFIX)
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

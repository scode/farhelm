//! Cell-boundary copy rules, independent of selection gestures or UI state.
//!
//! Adapted from Farhelm's `assets/grid-buffer.js` at the pinned revision. Its
//! selection rules derive from xterm.js 6.0.0 (MIT); see LICENSE-PORTED.

use crate::{GridModel, GridRow};
use std::ops::Range;

// Keep byte-identical to grid-buffer.js's pinned WORD_SEPARATORS (xterm's
// default wordSeparator); adding punctuation changes double-click semantics.
const WORD_SEPARATORS: &str = " ()[]{}',\"`";

/// A cell boundary on an absolute line, including the boundary after its end.
///
/// A retained line's number survives ordinary scrolling. Positions become
/// invalid after `history_reset`; the UI must clear its selection then.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub line: u64,
    pub col: u16,
}

/// Linear selection joins soft wraps; a rectangle copies each row separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    Linear,
    Column,
}

/// Ordered absolute endpoints, independent of the viewport and pointer gesture.
/// Linear endpoints are half-open; column ranges include both endpoint rows.
/// Keeping a range separate from its gesture lets output repaint it without
/// changing the text the user's press selected.
/// Construct with `Selection::new`: methods require `start <= end`. Public fields
/// let the terminal view inspect a range; callers must preserve that ordering
/// if they change an endpoint directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub start: Position,
    pub end: Position,
    pub mode: SelectionMode,
}

impl Selection {
    /// Normalize endpoint order while preserving the two columns of a rectangle.
    pub fn new(a: Position, b: Position, mode: SelectionMode) -> Self {
        let (start, end) = if a <= b { (a, b) } else { (b, a) };
        Self { start, end, mode }
    }

    /// Empty geometry must not overwrite a clipboard that another app owns.
    pub fn is_empty(self) -> bool {
        match self.mode {
            SelectionMode::Linear => self.start == self.end,
            SelectionMode::Column => self.start.col == self.end.col,
        }
    }

    /// The selected columns on one visible row, clipped to the current viewport.
    /// Copy still uses each retained row's original width through selection_text.
    pub fn columns(self, line: u64, cols: u16) -> Range<u16> {
        if line < self.start.line || line > self.end.line {
            return 0..0;
        }
        let (from, to) = match self.mode {
            SelectionMode::Column => (
                self.start.col.min(self.end.col),
                self.start.col.max(self.end.col),
            ),
            SelectionMode::Linear => (
                if line == self.start.line {
                    self.start.col
                } else {
                    0
                },
                if line == self.end.line {
                    self.end.col
                } else {
                    cols
                },
            ),
        };
        from.min(cols)..to.min(cols)
    }

    /// Eviction drops a prefix, never the surviving suffix of a selection.
    /// A linear range resumes at column zero; a rectangle keeps its left corner.
    pub fn trim_before(self, first: u64) -> Option<Self> {
        if self.end.line < first {
            return None;
        }
        let mut next = self;
        if next.start.line < first {
            next.start.line = first;
            if next.mode == SelectionMode::Linear {
                next.start.col = 0;
            }
        }
        Some(next)
    }
}

/// A UTF-16 unit mapped back to its wire cell owner, matching the pinned JS
/// algorithm. Surrogate pairs and combining marks share that owner's range.
/// Keeping code units matters for space-plus-combining fragments: the source's
/// space search can end inside a fragment while still selecting the whole cell.
pub(crate) struct WordCell {
    pub position: Position,
    pub width: u16,
    pub unit: u16,
}

impl GridModel {
    /// The pinned client's bounded double-click word, spanning only soft wraps.
    /// Spaces select a run of spaces. A clicked separator starts a word itself
    /// ("(foo)" clicked on '(' selects "(foo"). Searches reach at most 2048
    /// cells in each direction plus the clicked row, even for huge wrapped output.
    pub fn word_range(&self, position: Position) -> Option<Selection> {
        self.selection_line(position.line)?;
        let (first, last) = self.wrapped_span(position.line, usize::MAX, 2048);
        let cells = self.word_cells(first, last);
        let at = cells.iter().position(|cell| {
            cell.position.line == position.line
                && position.col >= cell.position.col
                && position.col < cell.position.col + cell.width
        })?;
        let cell_start = |mut i: usize| {
            while i > 0 && cells[i - 1].position == cells[i].position {
                i -= 1;
            }
            i
        };
        let cell_end = |mut i: usize| {
            while i + 1 < cells.len() && cells[i + 1].position == cells[i].position {
                i += 1;
            }
            i + 1
        };
        let separator = |i: usize| {
            WORD_SEPARATORS
                .encode_utf16()
                .any(|unit| unit == cells[i].unit)
        };
        let mut start = at;
        let mut end = cell_end(at);
        if cells[at].unit == 32 {
            while start > 0 && cells[start - 1].unit == 32 {
                start -= 1;
            }
            while end < cells.len() && cells[end].unit == 32 {
                end += 1;
            }
        } else {
            while start > 0 && !separator(cell_start(start - 1)) {
                start = cell_start(start - 1);
            }
            while end < cells.len() && !separator(end) {
                end = cell_end(end);
            }
        }
        let first = &cells[start];
        let last = &cells[end - 1];
        Some(Selection::new(
            first.position,
            Position {
                line: last.position.line,
                col: last.position.col + last.width,
            },
            SelectionMode::Linear,
        ))
    }

    /// Triple-click selects the whole logical line, with the pinned client's
    /// 1000-row search cap in each direction. Primary history is invisible while
    /// the alternate screen is active, even if those rows remain retained.
    pub fn line_range(&self, position: Position) -> Option<Selection> {
        self.selection_line(position.line)?;
        let (first, last) = self.wrapped_span(position.line, 1000, usize::MAX);
        let (cols, _) = self.selection_line(last)?;
        Some(Selection::new(
            Position {
                line: first,
                col: 0,
            },
            Position {
                line: last,
                col: cols,
            },
            SelectionMode::Linear,
        ))
    }

    /// Selection and link lookup see only the currently displayed buffer. The
    /// retained primary history must not offer copy/link targets on alt screen.
    pub(crate) fn selection_line(&self, line: u64) -> Option<(u16, &GridRow)> {
        if line < self.screen_base() && self.state().is_some_and(|state| state.alt_screen) {
            return None;
        }
        self.line(line)
    }

    /// Follow wrap markers in both directions with independent row/cell budgets.
    /// History is indexed by absolute line, so each step is constant time.
    fn wrapped_span(&self, at: u64, limit: usize, cells: usize) -> (u64, u64) {
        let mut first = at;
        let mut budget = cells;
        for _ in 0..limit {
            let Some(previous) = first.checked_sub(1) else {
                break;
            };
            let Some((cols, row)) = self.selection_line(previous) else {
                break;
            };
            if !row.wraps || budget < usize::from(cols) {
                break;
            }
            budget -= usize::from(cols);
            first = previous;
        }
        let mut last = at;
        budget = cells;
        for _ in 0..limit {
            let Some((_, row)) = self.selection_line(last) else {
                break;
            };
            let Some(next) = last.checked_add(1) else {
                break;
            };
            let Some((cols, _)) = self.selection_line(next).filter(|_| row.wraps) else {
                break;
            };
            if budget < usize::from(cols) {
                break;
            }
            budget -= usize::from(cols);
            last = next;
        }
        (first, last)
    }

    /// Map a caller-bounded word or URL window to UTF-16 cell owners. Uncovered
    /// wide-wrap padding is not text and cannot become a spurious boundary.
    pub(crate) fn word_cells(&self, first: u64, last: u64) -> Vec<WordCell> {
        let mut cells = Vec::new();
        for line in first..=last {
            let Some((cols, row)) = self.selection_line(line) else {
                continue;
            };
            let mut col = 0;
            let blank = |cells: &mut Vec<WordCell>, from: u16, to: u16| {
                cells.extend((from..to).map(|col| WordCell {
                    position: Position { line, col },
                    width: 1,
                    unit: 32,
                }));
            };
            for run in &row.runs {
                blank(&mut cells, col, run.col);
                if run.is_plain_flow() {
                    cells.extend(run.text.bytes().enumerate().map(|(offset, byte)| WordCell {
                        position: Position {
                            line,
                            col: run.col + offset as u16,
                        },
                        width: 1,
                        unit: u16::from(byte),
                    }));
                } else {
                    cells.extend(run.text.encode_utf16().map(|unit| WordCell {
                        position: Position { line, col: run.col },
                        width: run.width,
                        unit,
                    }));
                }
                col = run.col + run.width;
            }
            let end = cols - u16::from(self.pads_wide_wrap(line, cols, row));
            blank(&mut cells, col, end);
        }
        cells
    }

    /// Copy a range using Farhelm's cell rules, with LF between logical lines.
    ///
    /// Endpoints may arrive in either order. Linear ranges are half-open and
    /// right-trim hard line endings, but preserve blanks at a soft wrap and
    /// each history row's original width. Wide-character spacer cells add no
    /// text. A rectangle copies the same columns on every included row and
    /// trims each row. Nonbreaking spaces copy as ordinary spaces.
    ///
    /// Evicted lines contribute nothing. Work visits only the selected range
    /// intersecting retained lines, even for endpoints far outside the buffer.
    pub fn selection_text(&self, start: Position, end: Position, mode: SelectionMode) -> String {
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        let left = start.col.min(end.col);
        let right = start.col.max(end.col);
        if mode == SelectionMode::Column && left == right {
            return String::new();
        }
        let mut lines: Vec<String> = Vec::new();
        let mut join_next = false;
        let Some(last_offset) = self.screen().len().checked_sub(1) else {
            return String::new();
        };
        // History eviction removes only a prefix; retained history and the
        // screen form one interval. Clamp before iterating so dragging a
        // one-line selection does not scan the entire history cap. Saturation
        // excludes unrepresentable line numbers without wrapping to zero.
        let first = self
            .history()
            .front()
            .map_or(self.screen_base(), |line| line.number);
        let last = self.screen_base().saturating_add(last_offset as u64);
        for number in start.line.max(first)..=end.line.min(last) {
            let Some((cols, row)) = self.line(number) else {
                continue;
            };
            let (from, to) = match mode {
                SelectionMode::Linear => (
                    if number == start.line {
                        start.col.min(cols)
                    } else {
                        0
                    },
                    if number == end.line {
                        end.col.min(cols)
                    } else {
                        cols
                    },
                ),
                SelectionMode::Column => (left.min(cols), right.min(cols)),
            };
            let mut text = row.text(from, to);
            let continues = mode == SelectionMode::Linear && row.wraps && number != end.line;
            if !continues {
                text.truncate(text.trim_end_matches(' ').len());
            } else if to == cols && self.pads_wide_wrap(number, cols, row) {
                text.pop();
            }
            if join_next && let Some(last) = lines.last_mut() {
                last.push_str(&text);
            } else {
                lines.push(text);
            }
            join_next = continues;
        }
        lines.join("\n").replace('\u{00a0}', " ")
    }

    /// Distinguish the emulator's unused wrap-padding cell from a space the
    /// program actually printed. A two-cell non-ASCII run on the next row
    /// cannot fit in that last cell, so only the uncovered padding is omitted.
    pub(crate) fn pads_wide_wrap(&self, number: u64, cols: u16, row: &GridRow) -> bool {
        row.wraps
            && row.end() < cols
            && number
                .checked_add(1)
                .and_then(|next| self.line(next))
                .is_some_and(|(_, next)| {
                    next.runs
                        .first()
                        .is_some_and(|run| run.col == 0 && run.width == 2 && !run.is_plain_flow())
                })
    }
}

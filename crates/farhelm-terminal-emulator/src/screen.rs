//! The screen as plain text, shaped like the tmux capture that agent status
//! detection reads today.
//!
//! The supervisor's status sampler reads each agent's visible screen with
//! `tmux capture-pane -p`, trims it with `retain_pane_tail`
//! (`crates/farhelm-supervisor/src/tmux.rs`), and hands the text to a
//! per-agent screen reader that recognizes prompts and questions by their
//! shape. Those readers were written and tested against tmux's output, down
//! to details such as a blank draft row capturing as nothing at all
//! (`agent_kind/mod.rs`, the Codex prompt-row rules). So the text here
//! reproduces what tmux 3.7c produces for the same screen, as observed
//! against it:
//!
//! - Only the visible screen, and for a full-screen program the alternate
//!   screen (modes 1049, 47 and 1047), since that is what shows.
//! - One line per screen row, each ending in a newline, blank rows
//!   included. A row the program wrapped stays two lines.
//! - Trailing spaces are removed from every row, whether the program wrote
//!   them or they were never written, and whatever their colors.
//! - A wide character appears once; the cell it spills into adds nothing. A
//!   wide character that did not fit at the end of a row leaves a blank
//!   there and starts the next row.
//! - Combining characters follow their base character.
//! - A tab that moved the cursor over blank cells appears as one tab
//!   character instead of those cells, as tmux records it. A tab whose cells
//!   were written over since appears as spaces, as tmux turns it into
//!   spaces, and a tab sent with the cursor already in the last column,
//!   which moves nothing, appears as nothing.
//!
//! Tabs are where this falls short of tmux. tmux records each tab with the
//! cells it covered and updates that record as cells are erased, inserted or
//! deleted; alacritty keeps only a tab character in the cell where the tab
//! started. The span is recovered here from the current tab stops and the
//! cells after the tab, which matches tmux until a program erases part of a
//! tab's span, shifts it with inserted or deleted characters, or changes the
//! tab stops it crossed. Then the capture of that row can differ from tmux's
//! in its spacing (`tests/screen.rs` pins those cases). Matching tmux there
//! would mean tracking every tab's span through every edit.
//!
//! The comparison against a real tmux, on recorded sessions, follows in a
//! later change; the tests here pin the shape on hand-built screens.

use alacritty_terminal::Term;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::{Cell, Flags};

/// The visible screen as `tmux capture-pane -p` prints it (see the module
/// docs).
pub(crate) fn screen_text<L>(term: &Term<L>) -> String {
    let grid = term.grid();
    let columns = grid.columns();
    let landings = tab_landings(term.tab_stops(), columns);
    let mut text = String::new();
    for line in 0..grid.screen_lines() {
        let row = &grid[Line(line as i32)];
        let row_start = text.len();
        let mut column = 0;
        while column < columns {
            let cell = &row[Column(column)];
            column += 1;
            if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            if cell.flags.contains(Flags::LEADING_WIDE_CHAR_SPACER) {
                text.push(' ');
                continue;
            }
            if cell.c == '\t' {
                // alacritty marks only the cell where the tab started and
                // leaves the cells it moved over blank; tmux keeps the tab as
                // one cell covering them, which prints as the tab alone. The
                // tab moved the cursor to the next tab stop, or to the last
                // column if there was none; the cells in between are its span.
                let tab = column - 1;
                let landing = landings[tab];
                if landing > tab && (column..landing).all(|span| is_blank(&row[Column(span)])) {
                    text.push('\t');
                    column = landing;
                } else {
                    text.push(' ');
                }
                continue;
            }
            text.push(cell.c);
            if let Some(marks) = cell.zerowidth() {
                text.extend(marks);
            }
        }
        let trimmed = text[row_start..].trim_end_matches(' ').len();
        text.truncate(row_start + trimmed);
        text.push('\n');
    }
    text
}

/// For each column, where a tab sent from it lands: the first tab stop after
/// it, or the last column if there is none (as alacritty's `put_tab` moves).
///
/// Computed once per capture, in one pass from the right, so that a screen
/// full of tabs costs a capture time linear in its cells. Searching for the
/// next stop at each tab instead made a capture quadratic in the row width
/// on a screen a program filled with tabs after clearing the tab stops.
fn tab_landings(tab_stops: &[bool], columns: usize) -> Vec<usize> {
    let mut landings = vec![columns.saturating_sub(1); columns];
    let mut next_stop = None;
    for column in (0..columns).rev() {
        if let Some(stop) = next_stop {
            landings[column] = stop;
        }
        if tab_stops.get(column).copied().unwrap_or(false) {
            next_stop = Some(column);
        }
    }
    landings
}

/// Whether a cell holds nothing a capture would show but a space.
fn is_blank(cell: &Cell) -> bool {
    cell.c == ' ' && cell.zerowidth().is_none()
}

/// The last `max_bytes` of [`screen_text`] after trimming trailing
/// whitespace: what `retain_pane_tail` makes of tmux's capture for the
/// status sampler. The cut lands on a character boundary, so the result may
/// be a little shorter than `max_bytes`.
pub(crate) fn screen_tail(text: &str, max_bytes: usize) -> &str {
    let trimmed = text.trim_end();
    if trimmed.len() <= max_bytes {
        return trimmed;
    }
    let mut start = trimmed.len() - max_bytes;
    while !trimmed.is_char_boundary(start) {
        start += 1;
    }
    &trimmed[start..]
}

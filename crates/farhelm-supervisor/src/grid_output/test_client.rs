//! A grid client for tests: applies a supervisor's grid stream the way the
//! browser's grid client does (`crates/farhelm-ui/assets/grid-model.js`),
//! so that a test can compare what a viewer ends up showing with what the
//! emulator holds.
//!
//! Every message is decoded with the format's own decoder and passed
//! through [`GridStreamCheck`] before it is applied, so a stream that a
//! client would refuse fails the test at the message that broke it. The
//! model keeps rows as [`CellRow`]s, links resolved, so they compare
//! directly with what `super::screen` and a [`super::HistoryCursor`] read
//! from an emulator.

use std::collections::{BTreeMap, VecDeque};

use farhelm_proto::grid::{
    ClipboardTarget, Cursor, GridOutput, GridStreamCheck, Link, Row, TerminalState,
};
use farhelm_terminal_emulator::Emulator;

use super::{CellRow, CellRun, HistoryCursor};

/// What a viewer shows after the messages it was sent, and a record of the
/// messages themselves.
#[derive(Default)]
pub(crate) struct TestClient {
    check: GridStreamCheck,
    pub(crate) cols: u16,
    pub(crate) rows: u16,
    pub(crate) history_cap: usize,
    /// The committed screen, top row first.
    pub(crate) screen: Vec<CellRow>,
    pub(crate) cursor: Option<Cursor>,
    /// Rows received for the next frame.
    pending: BTreeMap<u16, CellRow>,
    /// History, oldest first: each line's number, the width it was written
    /// at, and the line.
    pub(crate) history: VecDeque<(u64, u16, CellRow)>,
    pub(crate) state: Option<TerminalState>,
    /// Completed clipboard writes, in order.
    pub(crate) clipboard: Vec<(ClipboardTarget, String)>,
    /// The clipboard write whose last part has not arrived.
    partial_clipboard: Option<(ClipboardTarget, String)>,
    /// Every message's `t`, in order.
    pub(crate) kinds: Vec<&'static str>,
    /// How many `history_reset`s arrived.
    pub(crate) resets: usize,
    /// The largest `shift` any frame had.
    pub(crate) max_shift: u16,
}

impl TestClient {
    pub(crate) fn new() -> TestClient {
        TestClient::default()
    }

    /// Decode, check and apply one data frame's payload, and return the
    /// message. Panics on anything a client would refuse.
    pub(crate) fn apply(&mut self, bytes: &[u8]) -> GridOutput {
        let message = GridOutput::decode(bytes).expect("a grid message that decodes");
        self.check
            .accept(&message)
            .unwrap_or_else(|error| panic!("a stream the client refuses: {error}"));
        self.kinds.push(kind(&message));
        match &message {
            GridOutput::Hello {
                cols,
                rows,
                history_cap,
            } => {
                self.cols = *cols;
                self.rows = *rows;
                self.history_cap = *history_cap as usize;
                self.screen = vec![CellRow::default(); usize::from(*rows)];
            }
            GridOutput::Rows { rows, links } => {
                for screen_row in rows {
                    self.pending
                        .insert(screen_row.y, resolve(&screen_row.row, links));
                }
            }
            GridOutput::Frame {
                cols,
                rows,
                shift,
                cursor,
                ..
            } => {
                if (*cols, *rows) != (self.cols, self.rows) {
                    self.cols = *cols;
                    self.rows = *rows;
                    self.screen = vec![CellRow::default(); usize::from(*rows)];
                } else if *shift > 0 {
                    self.screen.drain(..usize::from(*shift));
                    self.screen.resize(usize::from(*rows), CellRow::default());
                }
                self.max_shift = self.max_shift.max(*shift);
                for (y, row) in std::mem::take(&mut self.pending) {
                    self.screen[usize::from(y)] = row;
                }
                self.cursor = Some(*cursor);
            }
            GridOutput::History {
                first,
                cols,
                rows,
                links,
            } => {
                for (at, row) in rows.iter().enumerate() {
                    self.history
                        .push_back((first + at as u64, *cols, resolve(row, links)));
                    if self.history.len() > self.history_cap {
                        self.history.pop_front();
                    }
                }
            }
            GridOutput::HistoryReset => {
                self.history.clear();
                self.resets += 1;
            }
            GridOutput::State(state) => self.state = Some(*state),
            GridOutput::Clipboard { target, text, last } => {
                let mut partial = self
                    .partial_clipboard
                    .take()
                    .unwrap_or((*target, String::new()));
                partial.1.push_str(text);
                if *last {
                    self.clipboard.push(partial);
                } else {
                    self.partial_clipboard = Some(partial);
                }
            }
        }
        message
    }

    /// The history lines' rows, oldest first, without their numbers.
    pub(crate) fn history_rows(&self) -> Vec<CellRow> {
        self.history.iter().map(|(_, _, row)| row.clone()).collect()
    }

    /// Whether this client shows what `emulator` holds: its screen, cursor
    /// and every history line it keeps, at the emulator's size.
    pub(crate) fn shows(&self, emulator: &Emulator) -> bool {
        let screen = super::screen(emulator);
        (self.cols, self.rows) == (screen.cols, screen.rows)
            && self.screen == screen.lines
            && self.cursor == Some(screen.cursor)
            && self.history_rows() == held_history(emulator)
    }
}

/// Every history line `emulator` holds, oldest first.
pub(crate) fn held_history(emulator: &Emulator) -> Vec<CellRow> {
    HistoryCursor::new().read(emulator, 0, usize::MAX).lines
}

/// A row's text as it shows: each run's text at its column, blanks between,
/// trailing blanks dropped. A wide character's second cell adds nothing.
pub(crate) fn row_text(row: &CellRow) -> String {
    let mut text = String::new();
    let mut column = 0u16;
    for run in &row.runs {
        while column < run.col {
            text.push(' ');
            column += 1;
        }
        text.push_str(&run.text);
        column = run.col + run.width;
    }
    text
}

/// `row` with each run's link looked up in `links`.
fn resolve(row: &Row, links: &[Link]) -> CellRow {
    CellRow {
        wraps: row.wraps,
        runs: row
            .runs
            .iter()
            .map(|run| CellRun {
                col: run.col,
                width: run.width,
                text: run.text.clone(),
                style: run.style,
                link: run.link.map(|index| links[usize::from(index)].clone()),
            })
            .collect(),
    }
}

/// A message's `t`.
pub(crate) fn kind(message: &GridOutput) -> &'static str {
    match message {
        GridOutput::Hello { .. } => "hello",
        GridOutput::Rows { .. } => "rows",
        GridOutput::Frame { .. } => "frame",
        GridOutput::History { .. } => "history",
        GridOutput::HistoryReset => "history_reset",
        GridOutput::State(_) => "state",
        GridOutput::Clipboard { .. } => "clipboard",
    }
}

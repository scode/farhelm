//! One terminal attachment's screen and scrollback, for the GPUI desktop app's terminal.
//!
//! The supervisor owns terminal emulation. This crate only applies its grid
//! protocol: rows become visible at their committing frame, history keeps its
//! original widths, and every payload passes the upstream decoder and ordering
//! checker before changing the model. A reconnect must create a new model.
//!
//! History is decoded eagerly by the public protocol decoder. Callers should
//! schedule payload processing away from a UI thread when measuring catch-up;
//! this crate does not provide networking, time slicing, or a terminal emulator.
//!
//! "Pinned" in this crate's comments means the old web client as it stood at the
//! migration's reference commit 24bc0580 (`grid-model.js`, `grid-buffer.js`,
//! `terminal-links.js`), whose behavior this model reproduces.

mod link_disclosure;
mod links;
mod selection;
mod viewport;

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

pub use farhelm_proto::grid::{
    ClipboardTarget, Color, Cursor, CursorShape, ExitState, GridError, GridOutput, Link, MouseMode,
    Style, TerminalState, style_flags,
};
use farhelm_proto::grid::{GridStreamCheck, Row};
pub use link_disclosure::LinkDisclosure;
pub use links::{LinkKind, TerminalLink, web_target};
pub use selection::{Position, Selection, SelectionMode};
pub use viewport::{ViewRow, Viewport, position_of};

/// Compile one of this crate's constant patterns.
///
/// The patterns are string literals (plus the fixed JavaScript whitespace class), so compiling can only fail through
/// a programming error, and every pattern is compiled by the tests that exercise it. This holds the crate's one
/// `expect` outside tests, instead of each pattern site carrying its own.
#[allow(
    clippy::expect_used,
    reason = "constant regex literals; a bad pattern is a programming error the tests catch"
)]
fn constant_regex(pattern: &str) -> regex::Regex {
    regex::Regex::new(pattern).expect("constant regex compiles")
}

/// An owned run with its OSC 8 link resolved out of the message-local table.
///
/// Printable ASCII has one byte per cell; every other run is one indivisible
/// cell's text with width one or two. Renderers must use `width`, not a font's
/// measured advance or a Unicode width calculation, to place subsequent runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridRun {
    /// First covered column in the row, zero-based.
    pub col: u16,
    /// Occupied cells, independent of UTF-8 length or shaped glyph advance.
    pub width: u16,
    /// Received text, including controls removed only at drawing time.
    pub text: String,
    /// Unmodified wire style; theme color resolution belongs to the renderer.
    pub style: Style,
    /// Resolved OSC 8 identity and URI shared by all runs using that message's
    /// table entry. The target remains untrusted data, not an instruction to
    /// open a URL; sharing prevents long targets being copied for every cell.
    pub link: Option<Arc<Link>>,
}

impl GridRun {
    /// Printable ASCII with one byte per cell, which can flow at cell width.
    /// All other runs occupy one indivisible box, including ASCII controls.
    pub fn is_plain_flow(&self) -> bool {
        self.text.len() == usize::from(self.width)
            && self.text.bytes().all(|byte| (b' '..=b'~').contains(&byte))
    }
}

/// A complete row; uncovered columns are blank in the default style.
///
/// `wraps` means this row continues on the next, including any uncovered cells
/// up to its original width. It is independent of whether the next row is
/// currently retained in history.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GridRow {
    pub wraps: bool,
    pub runs: Vec<GridRun>,
}

impl GridRow {
    /// Keep shared references only to links this row uses. Retaining a row
    /// never retains its message's unrelated rows or unused link targets, and
    /// adjacent runs pointing at one target do not duplicate its strings.
    fn resolve(row: Row, links: &[Arc<Link>]) -> Self {
        Self {
            wraps: row.wraps,
            runs: row
                .runs
                .into_iter()
                .map(|run| GridRun {
                    col: run.col,
                    width: run.width,
                    text: run.text,
                    style: run.style,
                    link: run.link.map(|index| links[usize::from(index)].clone()),
                })
                .collect(),
        }
    }

    /// The boundary after the last transmitted cell, before implicit blanks.
    pub fn end(&self) -> u16 {
        self.runs.last().map_or(0, |run| run.col + run.width)
    }

    /// Text between cell boundaries, preserving blanks and the received text.
    ///
    /// A wide character contributes once, at its first cell. Starting on its
    /// spacer cell omits it; ending on that cell includes it. Combining marks
    /// stay with their base. Callers bound `from` and `to` to the row's width.
    /// Publicly constructed rows must preserve the protocol's sorted,
    /// nonoverlapping runs; the attachment model guarantees that invariant.
    pub fn text(&self, from: u16, to: u16) -> String {
        if from >= to {
            return String::new();
        }
        let mut out = String::new();
        let mut col = from;
        for run in &self.runs {
            let end = run.col + run.width;
            if end <= from {
                continue;
            }
            if run.col >= to {
                break;
            }
            if run.col > col {
                out.extend(std::iter::repeat_n(' ', usize::from(run.col - col)));
            }
            if run.is_plain_flow() {
                let first = from.max(run.col);
                let last = to.min(end);
                out.push_str(&run.text[usize::from(first - run.col)..usize::from(last - run.col)]);
                col = last;
            } else {
                if run.col >= from {
                    out.push_str(&run.text);
                }
                col = to.min(end);
            }
        }
        if col < to {
            out.extend(std::iter::repeat_n(' ', usize::from(to - col)));
        }
        out
    }
}

/// One retained history line, whose width survives later screen resizes.
///
/// `number` anchors scrolling and selection while older history is evicted.
/// A `history_reset` invalidates every such anchor, even if the rebuilt history
/// reuses the same numbers. Observe [`Damage::history_reset`] to clear anchors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryLine {
    pub number: u64,
    pub cols: u16,
    pub row: GridRow,
}

/// Changes since the renderer last called [`GridModel::take_damage`].
///
/// Rows are in the latest screen's coordinates. A nonzero `shift` can move a
/// cached screen before repainting `rows`; `all` supersedes both. History and
/// state change independently of committed screen frames.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Damage {
    pub all: bool,
    pub size: bool,
    pub rows: BTreeSet<u16>,
    pub shift: u16,
    pub cursor: bool,
    pub state: bool,
    pub history: bool,
    /// Clear selection and viewport anchors even if line numbers are reused.
    pub history_reset: bool,
    /// The catch-up barrier opened; the owner may replace its placeholder.
    pub visibility: bool,
}

impl Damage {
    /// Translate accumulated row damage when a later frame scrolls it.
    fn scroll(&mut self, shift: u16, rows: u16) {
        if self.all {
            return;
        }
        self.shift = self.shift.saturating_add(shift);
        if self.shift >= rows {
            self.all = true;
            self.rows.clear();
            return;
        }
        self.rows = self
            .rows
            .iter()
            .filter_map(|y| y.checked_sub(shift))
            .collect();
        self.rows.extend(rows - shift..rows);
    }
}

/// Immediate effects the attachment owner must deliver outside the model.
#[must_use = "completed OSC 52 writes are returned only once"]
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ApplyEffects {
    /// A completed write to either OSC 52 target means the program handled its
    /// own copy gesture. Primary writes suppress guidance without replacing the
    /// desktop clipboard. Partial chunks do neither.
    pub clipboard_written: bool,
    /// A completed OSC 52 clipboard write. Primary-selection writes are
    /// validated but ignored, so they cannot replace what the user copied.
    pub clipboard_text: Option<String>,
}

/// A single attachment's committed screen and bounded scrollback.
///
/// `apply` refuses malformed or misordered input before mutation. The owner
/// must end the attachment on any error, as required by the wire protocol.
/// Pending screen rows never reach `screen()` before a frame commits them.
#[derive(Debug, Default)]
pub struct GridModel {
    check: GridStreamCheck,
    cols: u16,
    rows: u16,
    history_cap: usize,
    screen: Vec<GridRow>,
    pending: BTreeMap<u16, GridRow>,
    update_open: bool,
    history: VecDeque<HistoryLine>,
    history_next: Option<u64>,
    cursor: Option<Cursor>,
    state: Option<TerminalState>,
    revision: Option<u64>,
    clipboard_open: bool,
    partial_clipboard: String,
    replay_complete: bool,
    damage: Damage,
}

impl GridModel {
    /// Start an attachment behind its catch-up placeholder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Decode, check, and apply one unchanged WebSocket binary payload.
    ///
    /// All bounds come from the pinned `farhelm_proto::grid`; this crate adds
    /// no alternate wire encoding or stricter history-count limit. Completed
    /// clipboard text is returned directly so unconsumed writes cannot build
    /// an unbounded queue inside the model.
    pub fn apply(&mut self, payload: &[u8]) -> Result<ApplyEffects, GridError> {
        self.apply_output(GridOutput::decode(payload)?)
    }

    /// Check and apply one message that was already decoded.
    ///
    /// The GPUI app's helm client decodes each binary frame on its own runtime
    /// and hands the GUI a `GridOutput`, so decoding twice on the GUI thread
    /// would only cost time. The ordering checker still sees every message
    /// before the model changes, exactly as for [`GridModel::apply`]: decoding
    /// elsewhere never skips the protocol's sequencing rules.
    pub fn apply_output(&mut self, message: GridOutput) -> Result<ApplyEffects, GridError> {
        self.check.accept(&message)?;
        let mut effects = ApplyEffects::default();
        match message {
            GridOutput::Hello {
                cols,
                rows,
                history_cap,
            } => {
                self.cols = cols;
                self.rows = rows;
                self.history_cap = history_cap as usize;
                self.screen = vec![GridRow::default(); usize::from(rows)];
                self.damage.all = true;
                self.damage.size = true;
            }
            GridOutput::Rows { rows, links } => {
                let links: Vec<_> = links.into_iter().map(Arc::new).collect();
                self.update_open = true;
                for row in rows {
                    self.pending
                        .insert(row.y, GridRow::resolve(row.row, &links));
                }
            }
            GridOutput::Frame {
                rev,
                cols,
                rows,
                shift,
                cursor,
            } => {
                if (cols, rows) != (self.cols, self.rows) {
                    // Resizing discards every old row. The sender supplies
                    // all nonblank rows at the new size in this same update.
                    self.cols = cols;
                    self.rows = rows;
                    self.screen = vec![GridRow::default(); usize::from(rows)];
                    self.damage.all = true;
                    self.damage.size = true;
                } else if shift > 0 {
                    self.screen.drain(..usize::from(shift));
                    self.screen.resize(usize::from(rows), GridRow::default());
                    self.damage.scroll(shift, rows);
                }
                for (y, row) in std::mem::take(&mut self.pending) {
                    self.screen[usize::from(y)] = row;
                    self.damage.rows.insert(y);
                }
                self.update_open = false;
                self.revision = Some(rev);
                self.cursor = Some(cursor);
                self.damage.cursor = true;
            }
            GridOutput::History {
                first,
                cols,
                rows,
                links,
            } => {
                let links: Vec<_> = links.into_iter().map(Arc::new).collect();
                self.history_next = Some(first + rows.len() as u64);
                // Even at cap zero the append settles screen numbering.
                // Avoid resolving rows that cannot survive this append.
                let skip = rows.len().saturating_sub(self.history_cap);
                for (offset, row) in rows.into_iter().enumerate().skip(skip) {
                    if self.history.len() == self.history_cap {
                        self.history.pop_front();
                    }
                    self.history.push_back(HistoryLine {
                        number: first + offset as u64,
                        cols,
                        row: GridRow::resolve(row, &links),
                    });
                }
                self.damage.history = true;
            }
            GridOutput::HistoryReset => {
                self.history.clear();
                self.history_next = None;
                self.damage.history = true;
                self.damage.history_reset = true;
            }
            GridOutput::State(state) => {
                self.state = Some(state);
                self.damage.state = true;
            }
            GridOutput::Clipboard { target, text, last } => {
                self.clipboard_open = !last;
                effects.clipboard_written = last;
                if target == ClipboardTarget::Clipboard {
                    self.partial_clipboard.push_str(&text);
                    if last {
                        effects.clipboard_text = Some(std::mem::take(&mut self.partial_clipboard));
                    }
                }
            }
        }
        Ok(effects)
    }

    /// Release the placeholder after the helm's separate `replay_complete`.
    ///
    /// That notice is a WebSocket text control, not a `GridOutput` variant.
    /// The attachment owner calls this only when it receives the notice.
    /// Refuse an incomplete first screen or an interrupted multipart update;
    /// an empty history is valid and needs no history message.
    pub fn mark_replay_complete(&mut self) -> Result<(), GridError> {
        if self.revision.is_none()
            || self.state.is_none()
            || self.update_open
            || self.clipboard_open
        {
            return Err(GridError::Bound(
                "replay_complete before a complete grid screen".into(),
            ));
        }
        if !self.replay_complete {
            self.damage.visibility = true;
        }
        self.replay_complete = true;
        Ok(())
    }

    /// Whether a valid `replay_complete` marker has reached this model.
    ///
    /// This tracks only that marker; the attachment owner decides any other
    /// placeholder-release policy, such as size, timeout, or failure endings.
    pub fn is_visible(&self) -> bool {
        self.replay_complete
    }

    /// The latest committed size; `(0, 0)` means hello has not arrived.
    pub fn size(&self) -> (u16, u16) {
        (self.cols, self.rows)
    }

    /// The advertised retention cap, applied after every history append.
    pub fn history_cap(&self) -> usize {
        self.history_cap
    }

    /// The screen, top row first, containing no uncommitted updates.
    pub fn screen(&self) -> &[GridRow] {
        &self.screen
    }

    /// History, oldest first, with each line's original width and number.
    pub fn history(&self) -> &VecDeque<HistoryLine> {
        &self.history
    }

    /// The line after the newest accepted history line: screen row zero.
    ///
    /// Absolute line numbers remain stable while output scrolls and history is
    /// evicted. A reset invalidates anchors. Before any history arrives the
    /// base is zero; catch-up visibility prevents users anchoring to that
    /// provisional numbering during attach.
    pub fn screen_base(&self) -> u64 {
        self.history_next.unwrap_or(0)
    }

    /// A retained history or screen line by absolute number, with its width.
    ///
    /// Missing/evicted lines return `None`, allowing anchored views to detect
    /// that they must clamp. A history reset requires dropping old anchors.
    pub fn line(&self, number: u64) -> Option<(u16, &GridRow)> {
        if number >= self.screen_base() {
            let offset = usize::try_from(number - self.screen_base()).ok()?;
            return self.screen.get(offset).map(|row| (self.cols, row));
        }
        let first = self.history.front()?.number;
        let offset = usize::try_from(number.checked_sub(first)?).ok()?;
        self.history.get(offset).map(|line| (line.cols, &line.row))
    }

    /// Cursor from the latest committed frame, absent before the first frame.
    pub fn cursor(&self) -> Option<Cursor> {
        self.cursor
    }

    /// The program's whole state, including default-color overrides and exit.
    pub fn state(&self) -> Option<TerminalState> {
        self.state
    }

    /// Last committed revision; revisions may skip but cannot repeat.
    pub fn revision(&self) -> Option<u64> {
        self.revision
    }

    /// Drain accumulated repaint work without changing grid contents.
    pub fn take_damage(&mut self) -> Damage {
        std::mem::take(&mut self.damage)
    }

    /// Screen rows as plain text for structured automation, without history.
    ///
    /// This reads committed contents even during catch-up. Callers that expose
    /// only the visible UI should first check `is_visible()`.
    pub fn terminal_text(&self) -> String {
        self.screen
            .iter()
            .map(|row| row.text(0, self.cols).trim_end_matches(' ').to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Remove bidi formatting controls before shaping a run in visual cell order.
///
/// The model and copied text retain the received characters. Rendering must
/// not let a program reorder already-positioned cells through embedding or
/// isolate controls. An unchanged run borrows its existing allocation.
pub fn display_text(text: &str) -> Cow<'_, str> {
    /// These affect bidirectional layout rather than occupying terminal cells.
    fn control(ch: char) -> bool {
        matches!(ch, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
    }
    if text.chars().any(control) {
        Cow::Owned(text.chars().filter(|ch| !control(*ch)).collect())
    } else {
        Cow::Borrowed(text)
    }
}

//! [`Emulator`]: one terminal's emulator, and the operations the supervisor
//! drives it with.

use std::time::Instant;

use alacritty_terminal::Term;
use alacritty_terminal::grid::{Dimensions, Grid};
use alacritty_terminal::term::cell::Cell;
use alacritty_terminal::term::{Config, Osc52, TermMode};
use alacritty_terminal::vte::ansi::{Processor, Timeout};

use crate::effects::{self, Effects, FeedMode, Listener};
use crate::modes::Modes;
use crate::queries::QueryHandler;
use crate::screen;
use crate::sync::SyncClock;

/// A terminal's size in character cells. `u16` like the terminal host's own
/// sizes; a zero is treated as one.
///
/// One column is a valid size. A wide character cannot fit in it and is
/// dropped (a vendored patch: upstream alacritty panics), and resizing a
/// screen holding wide characters to one column blanks them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    /// Cells per row.
    pub columns: u16,
    /// Rows on the screen, not counting history.
    pub lines: u16,
}

impl Size {
    /// alacritty's grid needs at least one cell each way; the terminal host
    /// never reports less, but a zero must not panic the supervisor.
    fn at_least_one_cell(self) -> Size {
        Size {
            columns: self.columns.max(1),
            lines: self.lines.max(1),
        }
    }
}

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.screen_lines()
    }

    fn screen_lines(&self) -> usize {
        usize::from(self.lines)
    }

    fn columns(&self) -> usize {
        usize::from(self.columns)
    }
}

/// One terminal's emulator: an `alacritty_terminal` [`Term`] and the vte
/// parser that feeds it, with Farhelm's answers to terminal queries, side
/// effects filtered by [`FeedMode`], a synchronized-output deadline on the
/// caller's clock, and a clean-boundary check for snapshots.
///
/// It does no I/O and owns no timer. Its owner feeds it the program's output,
/// writes the answers it produces to the program, wakes it at
/// [`Emulator::sync_deadline`], and reads the screen, title and modes from it.
/// To survive a supervisor restart it is snapshotted and restored (the
/// `snapshot` module, which reaches into the fields below).
pub struct Emulator {
    pub(crate) term: Term<Listener>,
    pub(crate) parser: Processor<SyncClock>,
    /// The same queue `term` reports into (see `effects`).
    pub(crate) listener: Listener,
    /// Whether a synchronized update is open as far as the handler has seen,
    /// for DECRQM 2026 (see `QueryHandler::sync_open`). Always false when the
    /// emulator is clean, so a snapshot need not record it.
    pub(crate) sync_open: bool,
    /// The most lines of history this emulator keeps, as created.
    pub(crate) scrollback_lines: usize,
    /// Set when this emulator was restored from a snapshot whose history had
    /// been cut short to fit the size limit; see
    /// [`Emulator::history_trimmed`].
    pub(crate) trimmed: Option<TrimMark>,
}

/// Where the primary screen's history counters stood when an emulator was
/// restored from a trimmed snapshot, and how many more lines had to enter
/// history then before the loss stopped showing, so that
/// [`Emulator::history_trimmed`] can tell when the trimmed history no longer
/// matters. A snapshot carries the distance left, so that it survives any
/// number of restores.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TrimMark {
    pub(crate) history_clears: u64,
    pub(crate) lines_scrolled: u64,
    pub(crate) remaining: u64,
}

/// The emulator's alacritty configuration, keeping `scrollback_lines` lines of
/// history (see [`Emulator::new`] for why the rest is fixed).
fn config(scrollback_lines: usize) -> Config {
    Config {
        scrolling_history: scrollback_lines,
        kitty_keyboard: false,
        osc52: Osc52::OnlyCopy,
        ..Config::default()
    }
}

impl Emulator {
    /// An emulator in the state of a fresh terminal of `size`, keeping up to
    /// `scrollback_lines` lines of history above the screen.
    ///
    /// The configuration is fixed, because it is part of what programs see:
    /// the kitty keyboard protocol off (legacy key encoding, as in the browser
    /// terminal today) and OSC 52 copy only (clipboard writes honored, reads
    /// never answered). Both are alacritty's defaults; they are spelled out so
    /// that a change of default upstream cannot change them.
    pub fn new(size: Size, scrollback_lines: usize) -> Emulator {
        let listener = Listener::default();
        Emulator {
            term: Term::new(
                config(scrollback_lines),
                &size.at_least_one_cell(),
                listener.clone(),
            ),
            parser: Processor::new(),
            listener,
            sync_open: false,
            scrollback_lines,
            trimmed: None,
        }
    }

    /// Interpret `bytes` of the program's output, received at `now`, and
    /// deliver what that makes the terminal do to `effects`, as `mode`
    /// allows.
    ///
    /// A synchronized update that was due to be flushed by `now` is flushed
    /// first, so a late wake-up cannot let a stalled update absorb output
    /// that arrived after its deadline.
    ///
    /// How the output is cut into feeds matters only while a synchronized
    /// update is open. Once all of the output has been fed and any open
    /// update flushed ([`Emulator::flush_sync`]), the terminal's state is the
    /// same however it was cut. Before that, vte decides where an update ends
    /// from each call as a whole, so whether an update is still open, and
    /// what has been drawn, can depend on the cut, and on time through the
    /// update's deadline. The one known exception is upstream vte's: a
    /// UTF-8-encoded C1 control (U+0080 to U+009F) split across two feeds is
    /// printed rather than executed, so a later REP (`CSI Ps b`) repeats it
    /// instead of the character before it.
    pub fn feed(&mut self, bytes: &[u8], now: Instant, mode: FeedMode, effects: &mut impl Effects) {
        self.flush_sync_if_due(now, mode, effects);
        let mut handler = QueryHandler {
            term: &mut self.term,
            listener: &self.listener,
            sync_open: &mut self.sync_open,
        };
        self.parser.advance(&mut handler, bytes);
        self.parser.sync_timeout().resolve(now);
        effects::route(self.listener.take(), mode, effects);
    }

    /// When the open synchronized update must be drawn even though the
    /// program has not ended it, or `None` when no update is open.
    ///
    /// vte gives a program 150 ms after an update starts (or restarts) to end
    /// it. The owner wakes at this time and calls
    /// [`Emulator::flush_sync_if_due`]; otherwise only the next feed flushes
    /// an overdue update, and a program that went silent gets no next feed.
    pub fn sync_deadline(&self) -> Option<Instant> {
        self.parser.sync_timeout().deadline()
    }

    /// Flush the open synchronized update if its deadline is at or before
    /// `now`, drawing what the program sent so far. Returns whether it did.
    pub fn flush_sync_if_due(
        &mut self,
        now: Instant,
        mode: FeedMode,
        effects: &mut impl Effects,
    ) -> bool {
        match self.sync_deadline() {
            Some(deadline) if deadline <= now => {
                self.flush_sync(mode, effects);
                true
            }
            _ => false,
        }
    }

    /// Flush the open synchronized update now, if there is one.
    ///
    /// For when no more output can end it: the program exited, or a
    /// recovering supervisor has replayed all the output there is. A frame is
    /// then better drawn incomplete than not at all.
    pub fn flush_sync(&mut self, mode: FeedMode, effects: &mut impl Effects) {
        if !self.parser.sync_timeout().pending_timeout() {
            return;
        }
        let mut handler = QueryHandler {
            term: &mut self.term,
            listener: &self.listener,
            sync_open: &mut self.sync_open,
        };
        self.parser.stop_sync(&mut handler);
        effects::route(self.listener.take(), mode, effects);
    }

    /// Change the terminal's size, reflowing as alacritty does, after
    /// flushing an open synchronized update (delivering its effects to
    /// `effects` as `mode` allows).
    ///
    /// Without the flush, whether an update's frame was drawn before or after
    /// a resize would depend on timing: live, its deadline has usually
    /// flushed it by the time the viewer resizes; in a replay of the journal,
    /// where no time passes, it would still be held across the resize and
    /// drawn at the new size. Flushing first makes both the same.
    ///
    /// So a resize during an open update draws that update's frame
    /// incomplete, like the 150 ms safety flush does. This is an accepted
    /// exception to "frames are not half-drawn except at the safety flush":
    /// the program redraws after the resize anyway, and the flush is what
    /// keeps a rebuilt terminal identical to the live one.
    pub fn resize(&mut self, size: Size, mode: FeedMode, effects: &mut impl Effects) {
        self.flush_sync(mode, effects);
        self.term.resize(size.at_least_one_cell());
    }

    /// The terminal's current size.
    pub fn size(&self) -> Size {
        // The grid's dimensions came from a `Size`, so they fit in `u16`.
        Size {
            columns: self.term.columns() as u16,
            lines: self.term.screen_lines() as u16,
        }
    }

    /// Whether this is a clean boundary: the parser is between sequences with
    /// no partial UTF-8 character, and no synchronized update is open.
    ///
    /// Only here does a snapshot of the emulator, plus the output that
    /// follows, rebuild the same state; a fresh parser would misread the rest
    /// of a sequence it never saw the start of. The parser being in its
    /// ground state alone is not enough: right after an update starts the
    /// parser is between sequences, but the update's bytes so far sit in a
    /// buffer the terminal has not seen yet.
    ///
    /// To snapshot partway through a read, feed it up to
    /// [`crate::snapshot_split_candidate`] and ask this.
    pub fn is_clean(&self) -> bool {
        self.parser.is_ground() && !self.parser.sync_timeout().pending_timeout()
    }

    /// How many bytes of an open synchronized update vte is holding. vte ends
    /// an update early rather than let this reach 2 MiB.
    pub fn sync_buffered_bytes(&self) -> usize {
        self.parser.sync_bytes_count()
    }

    /// The program's current terminal modes.
    pub fn modes(&self) -> Modes {
        Modes::from_term_mode(*self.term.mode())
    }

    /// The visible screen as plain text, shaped like `tmux capture-pane -p`
    /// output (see `screen`).
    pub fn screen_text(&self) -> String {
        screen::screen_text(&self.term)
    }

    /// The last `max_bytes` of [`Emulator::screen_text`] with trailing
    /// whitespace trimmed: what the status sampler reads from tmux today.
    pub fn screen_tail(&self, max_bytes: usize) -> String {
        screen::screen_tail(&self.screen_text(), max_bytes).to_owned()
    }

    /// The title the program last set (OSC 0 or 2), or the empty string if it
    /// set none or a full reset (RIS) cleared it.
    ///
    /// tmux reports a pane's title as `#{pane_title}`, and status detection
    /// matches prefixes of it. These differences remain, none of them visible
    /// to the prefixes it matches:
    ///
    /// - Before the program sets a title, tmux reports the host name.
    /// - vte trims leading and trailing whitespace, which tmux keeps.
    /// - tmux turns `#(` into `_(`, so that a title cannot run a command in
    ///   tmux's own formats; this emulator keeps it.
    /// - A title with invalid UTF-8: vte drops the `;`-separated parts that
    ///   are invalid, where tmux (from its source) ignores the whole
    ///   sequence and keeps the previous title.
    /// - RIS clears the title here, as alacritty does; tmux keeps it.
    ///
    /// Titles are at most 4096 bytes (the vendored cap).
    pub fn title(&self) -> &str {
        self.term.title().unwrap_or("")
    }

    /// The alacritty terminal itself, for reading what the accessors above do
    /// not cover: the grid and its history, cursor, colors.
    pub fn term(&self) -> &Term<Listener> {
        &self.term
    }

    /// The character `CSI Ps b` (REP) would repeat: the last one printed, or
    /// `None` before any. Parser state rather than terminal state, but a
    /// snapshot carries it, since a program may rely on it after a restore.
    pub fn preceding_char(&self) -> Option<char> {
        self.parser.preceding_char()
    }

    /// Whether this terminal's history is shorter than it should be because a
    /// snapshot it was restored from had to drop its oldest lines to fit the
    /// host's size limit (`snapshot` module docs, "Size"), so that the viewer
    /// can say history was trimmed.
    ///
    /// It stays true across further snapshots and restores, and turns false
    /// once the loss no longer shows: when the program clears its history
    /// (`CSI 3 J`, a full reset), or once as many lines as the history held
    /// when it was trimmed have entered it since, counted across restores,
    /// which would have pushed the lost lines out anyway. A Restart seed made
    /// from more lines than the scrollback holds is trimmed the same way.
    pub fn history_trimmed(&self) -> bool {
        self.trim_remaining().is_some()
    }

    /// While [`Emulator::history_trimmed`], how many more lines must enter
    /// history before it no longer is; `None` when it is not trimmed. A
    /// snapshot carries this count, so it goes on counting down across
    /// restores.
    pub fn trim_remaining(&self) -> Option<u64> {
        let mark = self.trimmed?;
        let primary = self.primary_grid();
        let scrolled = primary
            .lines_scrolled_into_history()
            .saturating_sub(mark.lines_scrolled);
        (primary.history_clears() == mark.history_clears && scrolled < mark.remaining)
            .then(|| mark.remaining - scrolled)
    }

    /// Mark this emulator's history trimmed until `remaining` more lines have
    /// entered it (or it is cleared); a `remaining` of 0 clears the mark.
    pub(crate) fn mark_trimmed(&mut self, remaining: u64) {
        let primary = self.primary_grid();
        self.trimmed = (remaining > 0).then(|| TrimMark {
            history_clears: primary.history_clears(),
            lines_scrolled: primary.lines_scrolled_into_history(),
            remaining,
        });
    }

    /// Keep up to `scrollback_lines` lines of history from now on, dropping
    /// the oldest beyond that. A restore starts with only as much as its
    /// snapshot holds, so that a blob cannot make it allocate more, and then
    /// gives the emulator its real scrollback.
    pub(crate) fn set_scrollback(&mut self, scrollback_lines: usize) {
        self.term.set_options(config(scrollback_lines));
        // `set_options` reports the title again; nobody needs to hear it.
        self.listener.take();
        self.scrollback_lines = scrollback_lines;
    }

    /// The primary screen's grid, the one with the history, whichever screen
    /// shows.
    pub(crate) fn primary_grid(&self) -> &Grid<Cell> {
        if self.term.mode().contains(TermMode::ALT_SCREEN) {
            self.term.inactive_grid()
        } else {
            self.term.grid()
        }
    }
}

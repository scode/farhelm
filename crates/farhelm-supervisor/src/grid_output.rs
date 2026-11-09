//! A host terminal's screen and history as grid stream messages
//! (`farhelm_proto::grid`): the pure half of the grid sender.
//!
//! For a terminal in a terminal host the supervisor's emulator holds the
//! screen, so a viewer is sent the emulator's cells rather than the
//! program's bytes. This module reads an [`Emulator`] and says what to send;
//! the per-attachment sender (`service::grid_stream`) owns the attachment,
//! its flow control and its timing, and calls in here with the emulator's
//! lock held. Nothing here does I/O, waits, or reads a clock.
//!
//! - **Cells to runs** ([`row_from_cells`]): one emulator row becomes a
//!   [`CellRow`] of styled runs, the wire's rows with links resolved rather
//!   than indexed, so that two reads of the same cells compare equal.
//!   Printable ASCII of one style shares a run; any other cell (a wide
//!   character, one with combining characters, any non-ASCII character) is
//!   a run of its own, as the format requires. Blank cells in the default
//!   style are left out, which the format reads as blank.
//! - **The screen** ([`Screen`], [`SentScreen`]): what the screen looks
//!   like now, and what the client was last sent, from which
//!   [`SentScreen::update`] makes the next `rows` and `frame` messages:
//!   only rows that differ, with the rows moved up by a `shift` when output
//!   scrolled and that sends fewer of them.
//! - **History** ([`HistoryCursor`]): history lines are numbered by the
//!   vendored counter of lines scrolled into history, so lines that entered
//!   history since the last read are an appended range, however many
//!   screen frames were skipped meanwhile. When the history changed in a
//!   way an append cannot express (a resize or a restored emulator, which
//!   the runtime counts as resizes and rebuilds; an erasure, which the
//!   emulator counts; a palette entry the program redefined, which changes
//!   how lines already sent are colored; or lines the client was owed
//!   scrolling off the far end) the cursor says to reset and starts again
//!   from the oldest line held. [`HistoryCursor::layout_moved`] tells a
//!   caller, before it reads, that a reset is coming, so that it can wait
//!   for a resize drag to settle first.
//! - **Packing** ([`rows_messages`], [`history_messages`]): rows go into
//!   messages of about [`MESSAGE_TARGET`] bytes, each with the links its
//!   rows use; a row whose links cannot all fit loses as many of them as it
//!   must, keeping its text and styles.
//! - **State and clipboard** ([`terminal_state`], [`clipboard_messages`]):
//!   the modes the client acts on, and an OSC 52 write cut into parts.
//!
//! What the client draws from these follows today's terminal where the
//! emulator can say it, with two accepted differences: the default cursor
//! blinks, and a program that explicitly asked for a steady block cursor
//! (`CSI 2 q`) gets a blinking one too, because the emulator cannot tell
//! that style from its default (the snapshot format has the same blind
//! spot); and the emulator has no blinking text, so the format's blink flag
//! is never set.

use std::collections::HashMap;

use farhelm_proto::grid::{
    ClipboardTarget, Color, Cursor, CursorShape, GRID_MAX_CLIPBOARD_BYTES,
    GRID_MAX_CLIPBOARD_PART_BYTES, GRID_MAX_LINKS, GRID_MAX_MESSAGE, GridOutput, Link, MouseMode,
    Row, Run, ScreenRow, Style, TerminalState, style_flags,
};
use farhelm_terminal_emulator::alacritty_terminal::grid::{Dimensions, Grid, Row as TermRow};
use farhelm_terminal_emulator::alacritty_terminal::index::{Column, Line};
use farhelm_terminal_emulator::alacritty_terminal::term::TermMode;
use farhelm_terminal_emulator::alacritty_terminal::term::cell::{Cell, Flags};
use farhelm_terminal_emulator::alacritty_terminal::term::color::Colors;
use farhelm_terminal_emulator::alacritty_terminal::vte::ansi::{
    Color as TermColor, CursorShape as TermCursorShape, CursorStyle, NamedColor, Rgb,
};
use farhelm_terminal_emulator::{Emulator, MouseReporting};

/// About how many binary bytes a `rows` or `history` message is packed
/// to, well below [`GRID_MAX_MESSAGE`].
///
/// The format allows 1 MiB, but the connection's writer queue is bounded by
/// frame count (64 frames), so message size is what bounds the bytes one
/// attachment can have queued: at this size about 2 MiB, the same bound an old
/// terminal's byte stream has with its 32 KiB chunks. It also keeps an ordinary
/// row's message within the 64 KiB a helm admits per terminal data frame today.
/// A single row larger than this (a long row of non-ASCII cells, or one with
/// many links) still goes, alone, in a message of its own.
pub const MESSAGE_TARGET: usize = 32 * 1024;

/// Conservative JSON envelope allowance for clipboard parts. Row messages
/// count their exact binary headers separately.
const ENVELOPE: usize = 128;

// ---- Rows ---------------------------------------------------------------------

/// One run of cells as the sender keeps it: the wire's [`Run`] with its
/// link resolved to the link itself rather than an index into a message's
/// table, so that rows read at different times compare by content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellRun {
    pub col: u16,
    pub width: u16,
    pub text: String,
    pub style: Style,
    pub link: Option<Link>,
}

impl CellRun {
    /// The column after this run's last cell.
    fn end(&self) -> u16 {
        self.col + self.width
    }

    /// Whether this is a run of printable ASCII, one character per cell:
    /// the only kind that may hold more than one cell, and so the only kind
    /// that can grow by a cell.
    fn is_ascii(&self) -> bool {
        self.text.bytes().all(|byte| (0x20..0x7f).contains(&byte))
    }

    /// Whether the run's cells look blank: spaces in the default style with
    /// no link, which the format lets the sender leave out.
    fn is_default_blank(&self) -> bool {
        self.style == Style::PLAIN && self.link.is_none() && self.text.bytes().all(|b| b == b' ')
    }
}

/// One row of cells as the sender keeps it (see [`CellRun`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CellRow {
    /// The row's text continues on the next row (a soft wrap).
    pub wraps: bool,
    pub runs: Vec<CellRun>,
}

impl CellRow {
    /// Whether the client draws this row as a blank one: what every row is
    /// right after `hello` and after a frame that resizes.
    pub fn is_blank(&self) -> bool {
        !self.wraps && self.runs.is_empty()
    }
}

/// One emulator row as a [`CellRow`]: the first `cols` cells of `row`, with
/// `colors` saying which palette entries the program redefined.
///
/// Tabs read as spaces (alacritty keeps a tab's first cell as `\t` for
/// copying). The spacer cell after a wide character is covered by that
/// character's run, and the placeholder alacritty leaves at the end of a row
/// whose wide character wrapped to the next is blank. Blank cells in the
/// default style are not sent: runs of them are dropped, and spaces in the
/// default style at either end of a run are trimmed off it.
pub fn row_from_cells(row: &TermRow<Cell>, cols: usize, colors: &Colors) -> CellRow {
    let cols = cols.min(row.len()).min(usize::from(u16::MAX));
    let mut runs: Vec<CellRun> = Vec::new();
    // Whether the last run is printable ASCII and so may grow by a cell:
    // kept here rather than read from the run's text, which would scan it
    // again for every cell.
    let mut last_ascii = false;
    let mut col = 0;
    while col < cols {
        let cell = &row[Column(col)];
        if cell
            .flags
            .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
        {
            col += 1;
            continue;
        }
        let style = style_of(cell, colors);
        let link = cell.hyperlink().map(|link| Link {
            id: link.id().to_owned(),
            uri: link.uri().to_owned(),
        });
        let character = if cell.c == '\t' { ' ' } else { cell.c };
        let zerowidth = cell.zerowidth().unwrap_or(&[]);
        let wide = cell.flags.contains(Flags::WIDE_CHAR);
        // `col < cols <= u16::MAX`, so the column fits.
        let at = col as u16;
        if !wide && zerowidth.is_empty() && (character == ' ' || character.is_ascii_graphic()) {
            match runs.last_mut() {
                Some(last)
                    if last_ascii
                        && last.end() == at
                        && last.style == style
                        && last.link == link =>
                {
                    last.text.push(character);
                    last.width += 1;
                }
                _ => runs.push(CellRun {
                    col: at,
                    width: 1,
                    text: character.to_string(),
                    style,
                    link,
                }),
            }
            last_ascii = true;
            col += 1;
        } else {
            let mut text = String::with_capacity(4 * (1 + zerowidth.len()));
            text.push(character);
            text.extend(zerowidth);
            // A wide character's spacer is the next cell; one in the last
            // column (which alacritty does not make) is drawn one cell wide
            // rather than past the row.
            let width = if wide && col + 1 < cols { 2 } else { 1 };
            runs.push(CellRun {
                col: at,
                width,
                text,
                style,
                link,
            });
            last_ascii = false;
            col += usize::from(width);
        }
    }
    trim_default_blanks(&mut runs);
    let wraps = cols > 0 && row[Column(cols - 1)].flags.contains(Flags::WRAPLINE);
    CellRow { wraps, runs }
}

/// Drop what draws the same as nothing: runs of default-style spaces, and
/// default-style spaces at either end of a default-style run.
fn trim_default_blanks(runs: &mut Vec<CellRun>) {
    for run in runs.iter_mut() {
        if run.style != Style::PLAIN || run.link.is_some() || !run.is_ascii() {
            continue;
        }
        let leading = run.text.len() - run.text.trim_start_matches(' ').len();
        let kept = run.text.trim_matches(' ').len();
        if kept == 0 {
            continue;
        }
        run.text = run.text[leading..leading + kept].to_owned();
        // Both fit: they are at most the run's width.
        run.col += leading as u16;
        run.width = kept as u16;
    }
    runs.retain(|run| !run.is_default_blank());
}

/// The wire style of a cell: its attributes as [`style_flags`] bits, and its
/// colors as [`wire_color`] gives them.
fn style_of(cell: &Cell, colors: &Colors) -> Style {
    const FLAGS: [(Flags, u16); 11] = [
        (Flags::BOLD, style_flags::BOLD),
        (Flags::DIM, style_flags::DIM),
        (Flags::ITALIC, style_flags::ITALIC),
        (Flags::UNDERLINE, style_flags::UNDERLINE),
        (Flags::DOUBLE_UNDERLINE, style_flags::DOUBLE_UNDERLINE),
        (Flags::UNDERCURL, style_flags::UNDERCURL),
        (Flags::DOTTED_UNDERLINE, style_flags::DOTTED_UNDERLINE),
        (Flags::DASHED_UNDERLINE, style_flags::DASHED_UNDERLINE),
        (Flags::INVERSE, style_flags::INVERSE),
        (Flags::HIDDEN, style_flags::HIDDEN),
        (Flags::STRIKEOUT, style_flags::STRIKETHROUGH),
    ];
    let flags = FLAGS
        .iter()
        .filter(|(flag, _)| cell.flags.contains(*flag))
        .fold(0, |bits, (_, bit)| bits | bit);
    Style {
        flags,
        fg: wire_color(cell.fg, colors),
        bg: wire_color(cell.bg, colors),
        underline_color: cell
            .underline_color()
            .map(|color| wire_color(color, colors))
            .filter(|color| *color != Color::Default),
    }
}

/// A cell color as the wire carries it.
///
/// The default foreground and background (and alacritty's other special
/// named colors) are the default, whose actual color the client takes from
/// its theme or from [`TerminalState`]'s redefined defaults. A palette
/// entry is its index, unless the program redefined it (OSC 4), in which
/// case it is the redefined color as a direct color: the client maps
/// indices through its own theme and never learns the program's palette.
/// alacritty's dim named colors are made when drawing, never stored in a
/// cell; should one appear, it reads as its ordinary color.
fn wire_color(color: TermColor, colors: &Colors) -> Color {
    let index = match color {
        TermColor::Spec(rgb) => return rgb_color(rgb),
        TermColor::Indexed(index) => index,
        TermColor::Named(named) => match palette_index(named) {
            Some(index) => index,
            None => return Color::Default,
        },
    };
    match colors[usize::from(index)] {
        Some(rgb) => rgb_color(rgb),
        None => Color::Palette(index),
    }
}

/// The palette index a named color stands for, `None` for the special
/// colors (foreground, background, cursor and their variants).
fn palette_index(named: NamedColor) -> Option<u8> {
    let number = named as usize;
    if number < 16 {
        return Some(number as u8);
    }
    let dim = NamedColor::DimBlack as usize;
    (dim..dim + 8)
        .contains(&number)
        .then(|| (number - dim) as u8)
}

fn rgb_color(rgb: Rgb) -> Color {
    Color::Rgb {
        r: rgb.r,
        g: rgb.g,
        b: rgb.b,
    }
}

// ---- The screen ---------------------------------------------------------------

/// The visible screen at one moment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen {
    pub cols: u16,
    pub rows: u16,
    /// One per screen row, top first.
    pub lines: Vec<CellRow>,
    pub cursor: Cursor,
    /// The alternate screen shows, so the primary screen's history (what
    /// the rows scroll into) does not move with this screen.
    pub alternate: bool,
}

/// Read the visible screen: the active grid (the alternate screen's while it
/// shows), the cursor on it, and how the cursor looks.
pub fn screen(emulator: &Emulator) -> Screen {
    let term = emulator.term();
    let grid = term.grid();
    let size = emulator.size();
    let cols = usize::from(size.columns);
    let colors = term.colors();
    let lines = (0..usize::from(size.lines))
        .map(|y| row_from_cells(&grid[Line(y as i32)], cols, colors))
        .collect();
    let modes = emulator.modes();
    let point = grid.cursor.point;
    let style = term.cursor_style();
    let (shape, hidden) = match style.shape {
        TermCursorShape::Block | TermCursorShape::HollowBlock => (CursorShape::Block, false),
        TermCursorShape::Underline => (CursorShape::Underline, false),
        TermCursorShape::Beam => (CursorShape::Bar, false),
        TermCursorShape::Hidden => (CursorShape::Block, true),
    };
    let cursor = Cursor {
        x: (point.column.0.min(cols.saturating_sub(1))) as u16,
        y: point.line.0.clamp(0, i32::from(size.lines) - 1) as u16,
        visible: modes.cursor_visible && !hidden,
        shape,
        // Today's terminal blinks unless the program says otherwise, and
        // alacritty's default style is steady (module docs).
        blink: style == CursorStyle::default() || style.blinking,
    };
    Screen {
        cols: size.columns,
        rows: size.lines,
        lines,
        cursor,
        alternate: modes.alternate_screen,
    }
}

/// What a client was last sent of the screen, which the next update is a
/// diff against.
#[derive(Debug, Clone)]
pub struct SentScreen {
    cols: u16,
    rows: u16,
    lines: Vec<CellRow>,
    /// The last frame's cursor, `None` before the first frame, so that the
    /// first update always commits one.
    cursor: Option<Cursor>,
    /// The last frame's `rev`.
    rev: u64,
}

impl SentScreen {
    /// What a client has right after a `hello` of `cols` by `rows`: a blank
    /// screen of that size, and no frame yet.
    pub fn after_hello(cols: u16, rows: u16) -> SentScreen {
        SentScreen {
            cols,
            rows,
            lines: vec![CellRow::default(); usize::from(rows)],
            cursor: None,
            rev: 0,
        }
    }

    /// The messages that bring the client from what it was sent to
    /// `screen`: `rows` messages for the rows that differ, then the `frame`
    /// that commits them. `None` when the client already shows `screen`.
    ///
    /// `scrolled` is how many lines the screen may have scrolled up since
    /// the last update, which the caller allows only once every line that
    /// scrolling pushed into history has been sent (the format's ordering
    /// rule for `shift`). The frame shifts by that many rows when that sends
    /// fewer rows than not shifting; either way the client ends up showing
    /// `screen`. A frame that resizes sends every row that is not blank,
    /// since the client blanks the rows a resizing frame does not update.
    pub fn update(&mut self, screen: &Screen, scrolled: u64) -> Option<Vec<GridOutput>> {
        let resized = (screen.cols, screen.rows) != (self.cols, self.rows);
        let rows = usize::from(screen.rows);
        let blank = CellRow::default();
        let mut shift = 0usize;
        let changed: Vec<usize> = if resized {
            (0..rows).filter(|&y| !screen.lines[y].is_blank()).collect()
        } else {
            let differs = |by: usize| {
                (0..rows)
                    .filter(|&y| screen.lines[y] != *self.lines.get(y + by).unwrap_or(&blank))
                    .collect::<Vec<_>>()
            };
            let unshifted = differs(0);
            let candidate = usize::try_from(scrolled).unwrap_or(usize::MAX).min(rows);
            if candidate > 0 && !unshifted.is_empty() {
                let shifted = differs(candidate);
                if shifted.len() < unshifted.len() {
                    shift = candidate;
                    shifted
                } else {
                    unshifted
                }
            } else {
                unshifted
            }
        };
        // A shift with no row to send is still a change: the client's rows
        // move.
        if !resized && changed.is_empty() && shift == 0 && self.cursor == Some(screen.cursor) {
            return None;
        }
        let rows_to_send: Vec<(u16, &CellRow)> = changed
            .iter()
            .map(|&y| (y as u16, &screen.lines[y]))
            .collect();
        let mut messages = rows_messages(&rows_to_send);
        self.rev += 1;
        messages.push(GridOutput::Frame {
            rev: self.rev,
            cols: screen.cols,
            rows: screen.rows,
            shift: shift as u16,
            cursor: screen.cursor,
        });
        self.cols = screen.cols;
        self.rows = screen.rows;
        self.lines = screen.lines.clone();
        self.cursor = Some(screen.cursor);
        Some(messages)
    }
}

// ---- Terminal state -----------------------------------------------------------

/// The terminal's modes as the `state` message carries them, with `exit`
/// left `None`: the program's exit is the host's to say, not the
/// emulator's, and the caller fills it in.
pub fn terminal_state(emulator: &Emulator) -> TerminalState {
    let modes = emulator.modes();
    let colors = emulator.term().colors();
    TerminalState {
        mouse: match modes.mouse_reporting {
            MouseReporting::Off => MouseMode::Off,
            MouseReporting::Clicks => MouseMode::Click,
            MouseReporting::Drag => MouseMode::Drag,
            MouseReporting::Motion => MouseMode::Motion,
        },
        focus: modes.focus_reporting,
        alt_screen: modes.alternate_screen,
        default_fg: colors[NamedColor::Foreground].map(rgb_color),
        default_bg: colors[NamedColor::Background].map(rgb_color),
        history_trimmed: emulator.history_trimmed(),
        exit: None,
    }
}

// ---- History ------------------------------------------------------------------

/// The primary screen's grid, the one with the history, whichever screen
/// shows.
fn primary_grid(emulator: &Emulator) -> &Grid<Cell> {
    let term = emulator.term();
    if term.mode().contains(TermMode::ALT_SCREEN) {
        term.inactive_grid()
    } else {
        term.grid()
    }
}

/// What must stay the same for history lines sent to a client to still be
/// right: the runtime's count of rebuilds and resizes (`relayouts`), the
/// emulator's count of history erasures, and the palette history rows were
/// colored with (a redefined palette entry travels as a direct color, so
/// redefining it again changes rows already sent). Opaque: callers compare
/// it, nothing more.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    relayouts: u64,
    clears: u64,
    palette: u64,
}

/// The [`Layout`] of `emulator`'s history now, `relayouts` being the
/// runtime's count of rebuilds and resizes.
fn layout_of(emulator: &Emulator, relayouts: u64) -> Layout {
    Layout {
        relayouts,
        clears: primary_grid(emulator).history_clears(),
        palette: palette_fingerprint(emulator.term().colors()),
    }
}

/// A fingerprint of the 256 palette entries a cell color can name, as the
/// program left them (FNV-1a over each entry: redefined or not, and to
/// what). Two palettes that color rows alike fingerprint alike; a change of
/// any entry changes it, short of a 64-bit collision.
fn palette_fingerprint(colors: &Colors) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |byte: u8| {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };
    for index in 0..256 {
        match colors[index] {
            Some(rgb) => {
                mix(1);
                mix(rgb.r);
                mix(rgb.g);
                mix(rgb.b);
            }
            None => mix(0),
        }
    }
    hash
}

/// How history lines are numbered for one client, valid while `layout`
/// holds. While it does, the line `n` lines above the screen keeps the
/// number `base + scrolled - n`, `scrolled` being the vendored count of
/// lines scrolled into history, which only grows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Numbering {
    layout: Layout,
    /// Added to the counter so that the oldest line held is never below
    /// line 0 (a restored emulator can hold more history than it has
    /// counted scrolling in).
    base: u64,
}

/// Why a [`HistoryBatch`] starts the client's history over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryReset {
    /// History was reflowed, renumbered, erased or recolored: a resize, a
    /// restored emulator, `CSI 3 J` and its kin, or a redefined palette
    /// entry. Lines sent before mean nothing now.
    Relayout,
    /// Lines the client was owed scrolled off the far end before they were
    /// sent; an append cannot skip them, so the client starts over from the
    /// oldest line still held.
    Evicted,
}

/// One read of the history lines a client is owed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryBatch {
    /// The client's history must be discarded (`history_reset`) before
    /// these lines are appended.
    pub reset: Option<HistoryReset>,
    /// The number of the first line in `lines`.
    pub first: u64,
    /// The width the lines are written at.
    pub cols: u16,
    /// Lines `first`, `first + 1`, ..., oldest first.
    pub lines: Vec<CellRow>,
    /// The number after the newest line held when this was read: once the
    /// client has every line before it, it has the whole history as read.
    pub end: u64,
}

impl HistoryBatch {
    /// How many lines held at the read are still owed after this batch.
    pub fn owed(&self) -> u64 {
        self.end - (self.first + self.lines.len() as u64)
    }
}

/// Which history lines one client has been sent, and so which it is owed:
/// every line from [`HistoryCursor::next`] to the newest held, under the
/// numbering the client was given.
#[derive(Debug, Clone, Default)]
pub struct HistoryCursor {
    /// `None` until the first read, which numbers the history.
    numbering: Option<Numbering>,
    /// The number of the first line the client has not been sent.
    next: u64,
}

impl HistoryCursor {
    /// A cursor for a client that has no history yet (right after `hello`).
    pub fn new() -> HistoryCursor {
        HistoryCursor::default()
    }

    /// The number of the first line not sent yet.
    pub fn next(&self) -> u64 {
        self.next
    }

    /// Whether the next [`Self::read`] would reset the client's history
    /// because its layout moved (not because lines were evicted), and the
    /// layout it would number the history under. `false` before the first
    /// read, which resets nothing.
    pub fn layout_moved(&self, emulator: &Emulator, relayouts: u64) -> (bool, Layout) {
        let layout = layout_of(emulator, relayouts);
        let moved = self
            .numbering
            .is_some_and(|numbering| numbering.layout != layout);
        (moved, layout)
    }

    /// The lines the client is owed, oldest first, at most `max_lines` of
    /// them, from `emulator` with `relayouts`, the runtime's count of its
    /// rebuilds and resizes (`EmulatorRef::rebuilds`,
    /// `EmulatorRef::resizes`). Counts them as sent.
    ///
    /// The first read numbers the history and starts at its oldest line.
    /// A later read starts where the last one ended, unless the history's
    /// layout changed or the next line owed is no longer held; then it says
    /// to reset ([`HistoryBatch::reset`]) and starts at the oldest line
    /// held, under a new numbering after a relayout.
    pub fn read(&mut self, emulator: &Emulator, relayouts: u64, max_lines: usize) -> HistoryBatch {
        self.read_within(emulator, relayouts, max_lines, usize::MAX)
    }

    /// [`Self::read`], stopping too once the lines read could take
    /// `max_bytes` encoded ([`row_bytes_estimate`], which errs high):
    /// always at least one line when one is owed, and at most one line past
    /// the bound.
    /// For a caller that sends the lines ahead of something that must not
    /// wait long behind them.
    pub fn read_within(
        &mut self,
        emulator: &Emulator,
        relayouts: u64,
        max_lines: usize,
        max_bytes: usize,
    ) -> HistoryBatch {
        let grid = primary_grid(emulator);
        let layout = layout_of(emulator, relayouts);
        let scrolled = grid.lines_scrolled_into_history();
        let held = grid.history_size() as u64;
        let mut reset = None;
        let numbering = match self.numbering {
            Some(numbering) if numbering.layout == layout => numbering,
            previous => {
                if previous.is_some() {
                    reset = Some(HistoryReset::Relayout);
                }
                let numbering = Numbering {
                    layout,
                    base: held.saturating_sub(scrolled),
                };
                self.numbering = Some(numbering);
                self.next = numbering.base + scrolled - held;
                numbering
            }
        };
        let end = numbering.base + scrolled;
        let oldest = end - held;
        if self.next < oldest {
            reset = Some(HistoryReset::Evicted);
            self.next = oldest;
        }
        let first = self.next;
        let to = end.min(first.saturating_add(max_lines as u64));
        let cols = grid.columns();
        let colors = emulator.term().colors();
        let mut lines = Vec::new();
        let mut bytes = 0usize;
        let mut number = first;
        while number < to && (lines.is_empty() || bytes < max_bytes) {
            // Line(-1) is the newest line held, numbered `end - 1`; both
            // stay within the history held, which is far below i32::MAX.
            let line = Line(-((end - number) as i32));
            let row = row_from_cells(&grid[line], cols, colors);
            bytes = bytes.saturating_add(row_bytes_estimate(&row));
            lines.push(row);
            number += 1;
        }
        self.next = number;
        HistoryBatch {
            reset,
            first,
            cols: cols as u16,
            lines,
            end,
        }
    }
}

// ---- Packing rows into messages -------------------------------------------------

/// A link as a key: its `id` and `uri`, which is what makes two runs one
/// link in the format.
type LinkKey = (String, String);

fn link_key(link: &Link) -> LinkKey {
    (link.id.clone(), link.uri.clone())
}

/// Rows being gathered into one message, with the links they use.
struct Pack<T> {
    rows: Vec<T>,
    links: Vec<Link>,
    index: HashMap<LinkKey, u16>,
    /// Exact binary bytes, including this message's fixed envelope.
    size: usize,
    /// Screen entries also carry y; history entries carry only row length.
    row_prefix: usize,
}

impl<T> Pack<T> {
    /// Start accounting with the caller's exact fixed fields. Screen and
    /// history have different row prefixes, but share link-table packing
    /// and the single-row fallback; clipboard remains independent JSON.
    fn new(envelope: usize, row_prefix: usize) -> Pack<T> {
        Pack {
            rows: Vec::new(),
            links: Vec::new(),
            index: HashMap::new(),
            size: envelope,
            row_prefix,
        }
    }

    fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Whether `row` can join this pack: room for its new links and, unless
    /// the pack is empty, for its bytes within [`MESSAGE_TARGET`]. An empty
    /// pack takes any row; [`Pack::wire_row`] makes it fit.
    fn takes(&self, row: &CellRow) -> bool {
        if self.is_empty() {
            return true;
        }
        let new = new_links(row, &self.index);
        let bytes = self.row_prefix
            + cell_row_binary_len(row)
            + new.iter().map(Link::binary_len).sum::<usize>();
        self.links.len() + new.len() <= GRID_MAX_LINKS && self.size + bytes <= MESSAGE_TARGET
    }

    /// `row` with its links as indices into this pack's table, adding the
    /// links it needs. Links that cannot fit, past [`GRID_MAX_LINKS`] or
    /// past [`GRID_MAX_MESSAGE`], are dropped from the runs that use them,
    /// newest first, leaving those runs' text and style (the format's rule
    /// for a row that cannot be sent with all its links).
    fn wire_row(&mut self, row: &CellRow) -> Row {
        let mut wanted = new_links(row, &self.index);
        wanted.truncate(GRID_MAX_LINKS - self.links.len());
        loop {
            let wire = to_wire(row, &self.index, &wanted, self.links.len());
            let bytes = self.row_prefix
                + wire.binary_len()
                + wanted.iter().map(Link::binary_len).sum::<usize>();
            if self.size + bytes <= GRID_MAX_MESSAGE || wanted.is_empty() {
                for link in wanted {
                    self.index.insert(link_key(&link), self.links.len() as u16);
                    self.links.push(link);
                }
                self.size += bytes;
                return wire;
            }
            wanted.pop();
        }
    }
}

/// The distinct links `row` uses that `index` does not hold yet, in the
/// order the row first uses them.
fn new_links(row: &CellRow, index: &HashMap<LinkKey, u16>) -> Vec<Link> {
    let mut new: Vec<Link> = Vec::new();
    for link in row.runs.iter().filter_map(|run| run.link.as_ref()) {
        if !index.contains_key(&link_key(link)) && !new.contains(link) {
            new.push(link.clone());
        }
    }
    new
}

/// `row` on the wire: each run's link as its index in `index`, or among
/// `added` (whose first entry will be index `table_len`), and no link where
/// it is in neither.
fn to_wire(row: &CellRow, index: &HashMap<LinkKey, u16>, added: &[Link], table_len: usize) -> Row {
    Row {
        wraps: row.wraps,
        runs: row
            .runs
            .iter()
            .map(|run| Run {
                col: run.col,
                width: run.width,
                text: run.text.clone(),
                style: run.style,
                link: run.link.as_ref().and_then(|link| {
                    index.get(&link_key(link)).copied().or_else(|| {
                        added
                            .iter()
                            .position(|candidate| candidate == link)
                            .map(|at| (table_len + at) as u16)
                    })
                }),
            })
            .collect(),
    }
}

/// Exact row body bytes before assigning link indices. The pack's admission
/// check only accepts rows whose links all fit; its single-row fallback
/// later counts again after dropping links that exceed the hard cap.
fn cell_row_binary_len(row: &CellRow) -> usize {
    3 + row
        .runs
        .iter()
        .map(|run| Run::binary_len(&run.text, run.style, run.link.is_some()))
        .sum::<usize>()
}

/// A conservative live-read budget for `row` and its links, worked out
/// without encoding it. This retains the former JSON upper bound, which
/// exceeds the binary representation: each run's text escaped at its longest
/// ([`escaped_len_at_most`]), the most a run takes besides
/// (`[col,width,"",[flags,fg,bg,underline],link],` with every number at its
/// widest), the run's link as an `["id","uri"],` entry of the message's
/// table (a link several runs share is counted for each), and the row's own
/// `{"w":false,"r":[...]},`. It leaves out the message around the rows,
/// tens of bytes per message. Cheap enough to count each line while the
/// emulator is held for the read, where encoding every row once more would
/// not be.
fn row_bytes_estimate(row: &CellRow) -> usize {
    /// What a run costs besides its text and link, at most.
    const RUN_OVERHEAD: usize = 64;
    /// What a link's table entry costs besides its id and uri.
    const LINK_OVERHEAD: usize = 8;
    /// What a row costs besides its runs.
    const ROW_OVERHEAD: usize = 20;
    row.runs
        .iter()
        .map(|run| {
            let link = run.link.as_ref().map_or(0, |link| {
                escaped_len_at_most(&link.id) + escaped_len_at_most(&link.uri) + LINK_OVERHEAD
            });
            escaped_len_at_most(&run.text) + RUN_OVERHEAD + link
        })
        .sum::<usize>()
        + ROW_OVERHEAD
}

/// No less than the length of `text` as a JSON string's contents: six
/// bytes for a control character (`\u00XX`, the longest escape), two for a
/// quote or a backslash, one for any other byte (JSON writes other
/// characters as they are).
fn escaped_len_at_most(text: &str) -> usize {
    text.bytes()
        .map(|byte| match byte {
            0x00..=0x1f => 6,
            b'"' | b'\\' => 2,
            _ => 1,
        })
        .sum()
}

/// Screen rows as `rows` messages of about [`MESSAGE_TARGET`] bytes each,
/// in the order given. No message for no rows.
pub fn rows_messages(rows: &[(u16, &CellRow)]) -> Vec<GridOutput> {
    let mut messages = Vec::new();
    let mut pack: Pack<ScreenRow> = Pack::new(8, 6);
    for (y, row) in rows {
        if !pack.takes(row) {
            let full = std::mem::replace(&mut pack, Pack::new(8, 6));
            messages.push(GridOutput::Rows {
                rows: full.rows,
                links: full.links,
            });
        }
        let wire = pack.wire_row(row);
        pack.rows.push(ScreenRow { y: *y, row: wire });
    }
    if !pack.is_empty() {
        messages.push(GridOutput::Rows {
            rows: pack.rows,
            links: pack.links,
        });
    }
    messages
}

/// History lines `first`, `first + 1`, ..., written at width `cols`, as
/// `history` messages of about [`MESSAGE_TARGET`] bytes each. No message
/// for no lines.
pub fn history_messages(first: u64, cols: u16, lines: &[CellRow]) -> Vec<GridOutput> {
    let mut messages = Vec::new();
    let mut pack: Pack<Row> = Pack::new(18, 4);
    let mut pack_first = first;
    for line in lines {
        if !pack.takes(line) {
            let full = std::mem::replace(&mut pack, Pack::new(18, 4));
            let count = full.rows.len() as u64;
            messages.push(GridOutput::History {
                first: pack_first,
                cols,
                rows: full.rows,
                links: full.links,
            });
            pack_first += count;
        }
        let wire = pack.wire_row(line);
        pack.rows.push(wire);
    }
    if !pack.is_empty() {
        messages.push(GridOutput::History {
            first: pack_first,
            cols,
            rows: pack.rows,
            links: pack.links,
        });
    }
    messages
}

// ---- Clipboard ----------------------------------------------------------------

/// The emulator's clipboard target as the wire names it: OSC 52's `p` and
/// `s` (the emulator's selection) are the primary selection.
pub fn clipboard_target(target: farhelm_terminal_emulator::ClipboardTarget) -> ClipboardTarget {
    match target {
        farhelm_terminal_emulator::ClipboardTarget::Clipboard => ClipboardTarget::Clipboard,
        farhelm_terminal_emulator::ClipboardTarget::Selection => ClipboardTarget::Primary,
    }
}

/// One clipboard write as its `clipboard` parts: each at most
/// [`GRID_MAX_CLIPBOARD_PART_BYTES`] of text and within
/// [`GRID_MAX_MESSAGE`] once JSON escapes it, cut only between characters,
/// the last marked `last`. An empty write is one empty part. A write over
/// [`GRID_MAX_CLIPBOARD_BYTES`] has no parts: the format has the sender drop
/// it rather than send part of it.
pub fn clipboard_messages(target: ClipboardTarget, text: &str) -> Vec<GridOutput> {
    if text.len() > GRID_MAX_CLIPBOARD_BYTES {
        return Vec::new();
    }
    let escaped_budget = GRID_MAX_MESSAGE - ENVELOPE;
    let mut parts = Vec::new();
    let mut start = 0;
    let mut escaped = 0;
    for (at, character) in text.char_indices() {
        let cost = escaped_len(character);
        let raw = at + character.len_utf8() - start;
        if raw > GRID_MAX_CLIPBOARD_PART_BYTES || escaped + cost > escaped_budget {
            parts.push(&text[start..at]);
            start = at;
            escaped = 0;
        }
        escaped += cost;
    }
    parts.push(&text[start..]);
    let count = parts.len();
    parts
        .into_iter()
        .enumerate()
        .map(|(at, part)| GridOutput::Clipboard {
            target,
            text: part.to_owned(),
            last: at + 1 == count,
        })
        .collect()
}

/// What one character takes inside a JSON string as `serde_json` writes it.
fn escaped_len(character: char) -> usize {
    match character {
        '"' | '\\' | '\u{8}' | '\u{c}' | '\n' | '\r' | '\t' => 2,
        '\0'..='\u{1f}' => 6,
        other => other.len_utf8(),
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) mod test_client;

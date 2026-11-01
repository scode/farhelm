//! Version 1's escape-sequence stream: ordinary terminal output that, fed to
//! a fresh emulator of the snapshot's size, rebuilds an emulator's state; and
//! the fixups for the few states no sequence can produce.
//!
//! Ported from the snapshot spike's encoder, which passed every round trip
//! over real agent sessions and synthetic recordings built to reach unusual
//! state. The stream is the format's core because it does not depend on the
//! emulator's internals: any emulator that interprets these sequences as
//! alacritty 0.26.0 does can consume it. The sequences used are SGR, OSC 8
//! (hyperlinks), CUP, CUF and CHA, ECH, LF, DECSC and DECRC, DECSTBM, mode 1049
//! and the other DEC private and ANSI modes listed in `paint`, TBC and HTS,
//! OSC 2 and `CSI 22 t` (title and title stack), OSC 4, 10, 11 and 12, DECSCUSR,
//! DECKPAM, SO, and charset designations. They are all a reader accepts
//! (`super::replay`), so this version can never write another.
//!
//! The order of the stream matters, and `paint` follows it:
//!
//! 1. The screens, oldest history line first, painted onto a blank screen
//!    with the full scroll region, a plain pen and ASCII character sets.
//!    History is made by scrolling painted lines off the top.
//! 2. The cursors and saved cursors of both screens (saving is DECSC, so the
//!    saved cursor is placed first and saved, then overwritten).
//!    A cursor with a wrap pending has its last cell reprinted, which raises
//!    the flag because autowrap is on until the stream's end. A program
//!    raises it by printing into the last column with autowrap on, and a
//!    reflowing resize can raise it with autowrap off; the reprint makes the
//!    same state either way, since the stream turns autowrap off afterwards
//!    if the program had.
//! 3. State that does not affect printing: tab stops, palette, title stack and
//!    title, cursor style, modes.
//! 4. The REP character, by printing it into a blank cell and erasing it.
//! 5. Scroll region and origin mode, then the active cursor, then the active
//!    character set and the modes that change how printing works (insert,
//!    newline, autowrap off), last because they would change everything
//!    painted before them.
//!
//! Fixups are applied after the stream, through the patched `Term`. The spike
//! found these states unreachable by any sequence: wide-character halves left
//! alone by an erase or an insert, a wrap flag away from the last column, a
//! wide character in the last column, a pending wrap away from the last column,
//! a cursor outside the scroll region under origin mode, the primary screen's
//! saved cursor while the alternate screen shows, and the REP character when
//! no blank cell can absorb the print that sets it. Real agent sessions needed
//! none; synthetic ones a few per snapshot.

use alacritty_terminal::Term;
use alacritty_terminal::grid::{Cursor, Dimensions, Grid, Row};
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::TermMode;
use alacritty_terminal::term::cell::{Cell, Flags, Hyperlink};
use alacritty_terminal::term::color::COUNT as COLOR_COUNT;
use alacritty_terminal::vte::ansi::{
    CharsetIndex, Color, CursorShape, CursorStyle, NamedColor, StandardCharset,
};
use unicode_width::UnicodeWidthChar;

use super::wire;
use super::{CELL_FIXUP_HEADER_BYTES, CursorFixup, Fixups, GridSel, grid_number};
use crate::effects::Listener;

/// The cell flags SGR sets on the cursor's template, and so on a printed cell.
const SGR_FLAGS: Flags = Flags::BOLD
    .union(Flags::DIM)
    .union(Flags::ITALIC)
    .union(Flags::ALL_UNDERLINES)
    .union(Flags::INVERSE)
    .union(Flags::HIDDEN)
    .union(Flags::STRIKEOUT);

/// A wide character printed where the painter needs a wide character's
/// effect (a leading spacer) and overwrites the character itself.
const WIDE_PLACEHOLDER: char = '\u{4e2d}';

/// The modes the stream sets with `CSI ? n h` or resets with `CSI ? n l`
/// when they differ from a fresh terminal's: those that do not change how
/// printing works. Origin mode, autowrap and the alternate screen have their
/// own places in the stream.
const PRIVATE_MODES: [(TermMode, u16); 11] = [
    (TermMode::SHOW_CURSOR, 25),
    (TermMode::APP_CURSOR, 1),
    (TermMode::MOUSE_REPORT_CLICK, 1000),
    (TermMode::MOUSE_DRAG, 1002),
    (TermMode::MOUSE_MOTION, 1003),
    (TermMode::BRACKETED_PASTE, 2004),
    (TermMode::SGR_MOUSE, 1006),
    (TermMode::UTF8_MOUSE, 1005),
    (TermMode::FOCUS_IN_OUT, 1004),
    (TermMode::ALTERNATE_SCROLL, 1007),
    (TermMode::URGENCY_HINTS, 1042),
];

/// What of the grids to paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Content {
    /// Both screens, and the primary screen's history without its `skip`
    /// oldest lines.
    Cells { skip_history: usize },
    /// Neither: both screens blank and no history. The last resort when the
    /// rest of the state alone barely fits the size limit.
    Nothing,
}

/// Whether painting keeps what it paints, or only measures it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Keep the stream and the fixups, in buffers reserved for `stream` and
    /// `cell_fixups` bytes up front: a measurement says exactly how much a
    /// painting takes, so that its buffers never grow past it (a `Vec` that
    /// grows doubles, and could hold twice the blob for a moment).
    Keep { stream: usize, cell_fixups: usize },
    /// Count their encoded bytes and drop them as painting goes, so that
    /// finding how much history fits the size limit never holds a stream
    /// larger than the limit: a hostile state can paint to hundreds of
    /// megabytes.
    Measure,
}

impl Mode {
    /// Keep, without a measurement to reserve by.
    pub(crate) const KEEP: Mode = Mode::Keep {
        stream: 0,
        cell_fixups: 0,
    };

    fn keeps(self) -> bool {
        matches!(self, Mode::Keep { .. })
    }
}

/// The stream and fixups of one emulator state, or with [`Mode::Measure`]
/// only their sizes (the stream and cell fixups empty). Cell fixups are kept
/// encoded, as the blob holds them: a hostile state has hundreds of
/// thousands, and as records in memory they would take about twice their
/// encoded size beside the blob.
pub(super) struct Painted {
    pub(super) stream: Vec<u8>,
    pub(super) fixups: Fixups,
    /// The stream's length, kept or not.
    pub(super) stream_len: usize,
    /// The encoded length of the cell fixups, kept or not.
    pub(super) cell_fixup_bytes: usize,
    /// How many cell fixups there are, kept or not.
    pub(super) cell_fixup_count: usize,
    /// For each history line painted, oldest first, the stream and cell
    /// fixup bytes before it: leaving out the lines before line `k` saves
    /// about `history_costs[k] - history_costs[0]` bytes.
    pub(super) history_costs: Vec<usize>,
}

/// Paint `term`'s whole state, with `content` deciding how much of the grids,
/// and the `skip_titles` oldest title stack entries left out. `preceding_char`
/// is the parser's REP character.
///
/// The caller guarantees a clean boundary; the stream assumes a parser in
/// its ground state with no update open.
pub(super) fn paint(
    term: &Term<Listener>,
    preceding_char: Option<char>,
    content: Content,
    skip_titles: usize,
    painting: Mode,
) -> Painted {
    let columns = term.columns();
    let lines = term.screen_lines();
    let mode = *term.mode();
    let alternate = mode.contains(TermMode::ALT_SCREEN);
    let (primary, alternate_grid) = if alternate {
        (term.inactive_grid(), term.grid())
    } else {
        (term.grid(), term.inactive_grid())
    };
    let mut painter = Painter::new(columns, lines, content != Content::Nothing, painting);
    let blank = Row::new(columns);
    let skip_history = match content {
        Content::Cells { skip_history } => skip_history,
        Content::Nothing => usize::MAX,
    };

    // 1 and 2: the screens and their cursors.
    if !alternate {
        // The alternate screen's saved cursor outlives leaving it (alacritty
        // clears the alternate screen's cells on entry, not its saved
        // cursor), so a later DECRC there can observe it.
        if !cursor_eq(&alternate_grid.saved_cursor, &Cursor::default()) {
            painter.w.raw("\x1b[?1049h");
            painter.set_cursor(
                &alternate_grid.saved_cursor,
                alternate_grid,
                GridSel::Inactive,
                true,
                None,
            );
            painter.w.raw("\x1b7");
            painter.w.ascii_charsets();
            painter.w.plain();
            painter.w.raw("\x1b[?1049l");
            painter.w.known(0, 0);
        }
        let rows = grid_rows(primary, skip_history, &blank, painter.cells);
        painter.paint_rows(&rows, GridSel::Active, history_len(primary, skip_history));
        painter.set_cursor(&primary.saved_cursor, primary, GridSel::Active, true, None);
        painter.w.raw("\x1b7");
        painter.w.ascii_charsets();
    } else {
        let rows = grid_rows(primary, skip_history, &blank, painter.cells);
        painter.paint_rows(&rows, GridSel::Inactive, history_len(primary, skip_history));
        let fixed_before = painter.fixups.cursors.len();
        painter.set_cursor(&primary.cursor, primary, GridSel::Inactive, false, None);
        let cursor_fixed = painter.fixups.cursors.len() > fixed_before;
        // Entering the alternate screen saves the primary cursor, as the
        // stream placed it, over the primary saved cursor: a saved cursor
        // that differs from the cursor, or a cursor the stream could not
        // place (a fixup), needs a fixup of its own.
        if cursor_fixed || !cursor_eq(&primary.saved_cursor, &primary.cursor) {
            painter.fixups.cursors.push(CursorFixup {
                grid: GridSel::Inactive,
                saved: true,
                cursor: primary.saved_cursor.clone(),
            });
        }
        painter.w.raw("\x1b[?1049h");
        painter.w.ascii_charsets();
        painter.w.plain();
        painter.w.raw("\x1b[2J");
        painter.w.pos = None;
        let rows = grid_rows(alternate_grid, 0, &blank, painter.cells);
        painter.paint_rows(&rows, GridSel::Active, 0);
        painter.set_cursor(
            &alternate_grid.saved_cursor,
            alternate_grid,
            GridSel::Active,
            true,
            None,
        );
        painter.w.raw("\x1b7");
        painter.w.ascii_charsets();
    }
    painter.w.plain();

    // 3: state that does not affect printing.
    let tabs = term.tab_stops();
    if tabs
        .iter()
        .enumerate()
        .any(|(column, &stop)| stop != (column % 8 == 0))
    {
        painter.w.raw("\x1b[3g");
        for (column, &stop) in tabs.iter().enumerate() {
            if stop {
                painter.w.cup(0, column);
                painter.w.raw("\x1bH");
            }
        }
    }

    // OSC 4 for the 256 palette entries, OSC 10, 11 and 12 for foreground,
    // background and cursor. The slots beyond (dim and bright variants) are
    // derived by alacritty and no sequence sets them.
    let colors = term.colors();
    for index in 0..COLOR_COUNT {
        if let Some(rgb) = colors[index] {
            let spec = format!("rgb:{:02x}/{:02x}/{:02x}", rgb.r, rgb.g, rgb.b);
            match index {
                0..=255 => painter.w.raw(&format!("\x1b]4;{index};{spec}\x1b\\")),
                256..=258 => painter
                    .w
                    .raw(&format!("\x1b]{};{spec}\x1b\\", 10 + index - 256)),
                _ => {}
            }
        }
    }

    // The title stack, then the title. An entry without a title can only
    // sit below every entry with one (it is pushed while no title was set,
    // and only a full reset unsets the title, emptying the stack too), which
    // is also the only place `CSI 22 t` on a fresh terminal can push one;
    // likewise the title itself is unset only when no entry has one.
    let stack = &term.title_stack()[skip_titles.min(term.title_stack().len())..];
    let untitled = stack.iter().take_while(|title| title.is_none()).count();
    for _ in 0..untitled {
        painter.w.raw("\x1b[22t");
    }
    for title in stack[untitled..].iter().flatten() {
        painter.w.raw(&format!("\x1b]2;{title}\x1b\\\x1b[22t"));
    }
    if let Some(title) = term.title() {
        painter.w.raw(&format!("\x1b]2;{title}\x1b\\"));
    }

    // Only the effective cursor style is readable: a program's explicit
    // default style restores as no style, which looks the same, since the
    // emulator's default never changes. Only DECSCUSR's three shapes can be
    // set by a program.
    let style = term.cursor_style();
    if style != CursorStyle::default() {
        let base = match style.shape {
            CursorShape::Block => Some(1),
            CursorShape::Underline => Some(3),
            CursorShape::Beam => Some(5),
            CursorShape::HollowBlock | CursorShape::Hidden => None,
        };
        if let Some(base) = base {
            let steady = if style.blinking { 0 } else { 1 };
            painter.w.raw(&format!("\x1b[{} q", base + steady));
        }
    }

    let fresh = TermMode::default();
    for (flag, number) in PRIVATE_MODES {
        if mode.contains(flag) != fresh.contains(flag) {
            let set = if mode.contains(flag) { 'h' } else { 'l' };
            painter.w.raw(&format!("\x1b[?{number}{set}"));
        }
    }
    if mode.contains(TermMode::APP_KEYPAD) {
        painter.w.raw("\x1b=");
    }

    // 4: the REP character is whatever was printed last. Print it into blank
    // cells and erase them again, unless the active cursor's pending wrap
    // makes the stream print last anyway (checked below).
    let active = term.grid();
    let final_print = active.cursor.input_needs_wrap;
    if !final_print && painter.w.last_printed != preceding_char {
        let placed = preceding_char.is_some_and(|c| painter.print_rep_char(active, c));
        if !placed {
            painter.fixups.preceding_char = Some(preceding_char);
        }
    }

    // 5: scroll region, then origin mode, which homes the cursor inside it.
    let region = term.scroll_region().clone();
    if region != (Line(0)..Line(lines as i32)) {
        // DECSTBM needs top < bottom, so a region of one line, or none, only
        // exists because the bottom was clamped to the screen; reproduce it
        // the same way.
        let clamped_bottom = region.end.0 as usize == lines;
        if region.start < region.end && (region.end.0 - region.start.0 > 1 || clamped_bottom) {
            let bottom = if clamped_bottom {
                lines + 1
            } else {
                region.end.0 as usize
            };
            painter
                .w
                .raw(&format!("\x1b[{};{bottom}r", region.start.0 + 1));
        } else if region.start.0 as usize == lines {
            painter.w.raw(&format!("\x1b[{};{}r", lines + 1, lines + 2));
        }
        painter.w.pos = None;
    }
    let origin = mode.contains(TermMode::ORIGIN);
    if origin {
        painter.w.raw("\x1b[?6h");
        painter.w.pos = None;
    }

    let origin_region = origin.then_some(region.start.0..region.end.0);
    painter.set_cursor(
        &active.cursor,
        active,
        GridSel::Active,
        false,
        origin_region,
    );
    if final_print && painter.w.last_printed != preceding_char {
        painter.fixups.preceding_char = Some(preceding_char);
    }
    // Only SI and SO choose a slot, so G2 and G3 are never active.
    if term.active_charset() == CharsetIndex::G1 {
        painter.w.raw("\x0e");
    }

    // Last, the modes that change what printing does.
    if mode.contains(TermMode::INSERT) {
        painter.w.raw("\x1b[4h");
    }
    if mode.contains(TermMode::LINE_FEED_NEW_LINE) {
        painter.w.raw("\x1b[20h");
    }
    if !mode.contains(TermMode::LINE_WRAP) {
        painter.w.raw("\x1b[?7l");
    }

    painter.finish()
}

/// Paint only the primary screen's history, without its `skip_history`
/// oldest lines, above a blank screen, leaving everything else as a fresh
/// terminal has it: the Restart seed (`Emulator::history_seed`).
pub(super) fn paint_history(term: &Term<Listener>, skip_history: usize, mode: Mode) -> Painted {
    let columns = term.columns();
    let lines = term.screen_lines();
    let primary = if term.mode().contains(TermMode::ALT_SCREEN) {
        term.inactive_grid()
    } else {
        term.grid()
    };
    let mut painter = Painter::new(columns, lines, true, mode);
    let blank = Row::new(columns);
    let history = history_len(primary, skip_history);
    let mut rows = grid_rows(primary, skip_history, &blank, true);
    // Blank rows in place of the screen push every history row into history.
    rows.truncate(history);
    rows.extend((0..lines).map(|line| (line as i32, &blank)));
    painter.paint_rows(&rows, GridSel::Active, history);
    // Painting moved the cursor, saved it (for wrapped lines) and printed:
    // put the cursor, the saved cursor and the REP character back to a fresh
    // terminal's.
    painter.w.plain();
    painter.w.raw("\x1b[H\x1b7");
    if painter.w.last_printed.is_some() {
        painter.fixups.preceding_char = Some(None);
    }
    painter.finish()
}

/// The rows of `grid` to paint, oldest first, each with its line number in
/// `grid`: history without its `skip` oldest lines, then the screen. With
/// `cells` false, every row is `blank`.
fn grid_rows<'a>(
    grid: &'a Grid<Cell>,
    skip: usize,
    blank: &'a Row<Cell>,
    cells: bool,
) -> Vec<(i32, &'a Row<Cell>)> {
    let first = -(history_len(grid, skip) as i32);
    (first..grid.screen_lines() as i32)
        .map(|line| (line, if cells { &grid[Line(line)] } else { blank }))
        .collect()
}

/// How many history lines of `grid` remain without its `skip` oldest.
fn history_len(grid: &Grid<Cell>, skip: usize) -> usize {
    grid.history_size().saturating_sub(skip)
}

// ---------------------------------------------------------------------------
// Cell classification: which cells printing can produce, and how.

/// Whether a cell's extra storage is empty: it can be allocated yet hold
/// nothing (after `clear_wide`), which looks the same as none.
fn extra_empty(cell: &Cell) -> bool {
    cell.zerowidth().is_none_or(<[char]>::is_empty)
        && cell.underline_color().is_none()
        && cell.hyperlink().is_none()
}

/// Whether a cell is what a fresh screen holds: nothing to paint.
fn is_default(cell: &Cell) -> bool {
    cell.c == ' '
        && cell.fg == Color::Named(NamedColor::Foreground)
        && cell.bg == Color::Named(NamedColor::Background)
        && cell.flags.is_empty()
        && extra_empty(cell)
}

/// Two cells look the same: `Cell`'s equality, except that empty extra
/// storage equals none.
pub(super) fn cell_eq(a: &Cell, b: &Cell) -> bool {
    a.c == b.c
        && a.fg == b.fg
        && a.bg == b.bg
        && a.flags == b.flags
        && a.zerowidth().unwrap_or(&[]) == b.zerowidth().unwrap_or(&[])
        && a.underline_color() == b.underline_color()
        && a.hyperlink() == b.hyperlink()
}

/// Two cursors look the same (see [`cell_eq`] for their templates).
pub(super) fn cursor_eq(a: &Cursor<Cell>, b: &Cursor<Cell>) -> bool {
    a.point == b.point
        && cell_eq(&a.template, &b.template)
        && a.input_needs_wrap == b.input_needs_wrap
        && CHARSET_SLOTS
            .iter()
            .all(|&slot| a.charsets[slot] == b.charsets[slot])
}

const CHARSET_SLOTS: [CharsetIndex; 4] = [
    CharsetIndex::G0,
    CharsetIndex::G1,
    CharsetIndex::G2,
    CharsetIndex::G3,
];

/// SGR can set a foreground of an ANSI color or the default foreground, but
/// not alacritty's derived slots (dim, bright foreground, cursor).
fn fg_paintable(color: &Color) -> bool {
    match color {
        Color::Named(named) => (*named as usize) < 16 || *named == NamedColor::Foreground,
        Color::Indexed(_) | Color::Spec(_) => true,
    }
}

fn bg_paintable(color: &Color) -> bool {
    match color {
        Color::Named(named) => (*named as usize) < 16 || *named == NamedColor::Background,
        Color::Indexed(_) | Color::Spec(_) => true,
    }
}

/// Whether SGR and OSC 8 can give a cell its colors, attributes and link.
fn pen_paintable(cell: &Cell) -> bool {
    let underlines = (cell.flags & Flags::ALL_UNDERLINES).bits().count_ones();
    fg_paintable(&cell.fg)
        && bg_paintable(&cell.bg)
        && underlines <= 1
        && !matches!(cell.underline_color(), Some(Color::Named(_)))
        && cell.hyperlink().is_none_or(|link| link_paintable(&link))
}

/// OSC 8 cannot carry an `id` holding `:` or `;`, an empty URI, or a URI
/// with a byte that would end the sequence.
fn link_paintable(link: &Hyperlink) -> bool {
    let control = |c: char| (c as u32) < 0x20 || c == '\x7f';
    !link.id().contains([':', ';'])
        && !link.id().chars().any(control)
        && !link.uri().is_empty()
        && !link.uri().chars().any(control)
}

/// Whether printing a cell's zero-width characters after it reproduces them.
fn zerowidth_paintable(cell: &Cell) -> bool {
    cell.zerowidth()
        .unwrap_or(&[])
        .iter()
        .all(|c| c.width() == Some(0))
}

/// How one cell is painted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Unit {
    /// A default cell: nothing to paint.
    Skip,
    /// A character one column wide (or a cell holding a tab), printed as is.
    Single,
    /// A wide character, with its spacer in the next column.
    Pair,
    /// A leading wide-character spacer in the last column, which printing a
    /// wide character there produces.
    Leading,
    /// The spacer half of a `Pair`, painted with it.
    Spacer,
    /// Not producible by printing: a fixup.
    Fix,
}

/// How each cell of a row is painted.
fn classify(row: &Row<Cell>, columns: usize) -> Vec<Unit> {
    let last = columns - 1;
    let mut units = vec![Unit::Fix; columns];
    let mut column = 0;
    while column < columns {
        let cell = &row[Column(column)];
        let wrap_allowed = if column == last {
            Flags::WRAPLINE
        } else {
            Flags::empty()
        };
        if is_default(cell) {
            units[column] = Unit::Skip;
        } else if cell.flags.contains(Flags::WIDE_CHAR) && column < last {
            let spacer = &row[Column(column + 1)];
            let rest = cell.flags - Flags::WIDE_CHAR - SGR_FLAGS;
            let spacer_wrap = if column + 1 == last {
                Flags::WRAPLINE
            } else {
                Flags::empty()
            };
            let spacer_flags = (cell.flags - Flags::WIDE_CHAR) | Flags::WIDE_CHAR_SPACER;
            let spacer_matches = spacer.c == ' '
                && spacer.fg == cell.fg
                && spacer.bg == cell.bg
                && (spacer.flags - (spacer.flags & spacer_wrap)) == spacer_flags
                && spacer.zerowidth().is_none_or(<[char]>::is_empty)
                && spacer.underline_color() == cell.underline_color()
                && spacer.hyperlink() == cell.hyperlink();
            if rest.is_empty()
                && cell.c.width() == Some(2)
                && pen_paintable(cell)
                && zerowidth_paintable(cell)
                && spacer_matches
            {
                units[column] = Unit::Pair;
                units[column + 1] = Unit::Spacer;
                column += 2;
                continue;
            }
        } else if column == last
            && columns >= 2
            && cell.flags - SGR_FLAGS == Flags::LEADING_WIDE_CHAR_SPACER | Flags::WRAPLINE
            && cell.c == ' '
            && cell.zerowidth().is_none_or(<[char]>::is_empty)
            && pen_paintable(cell)
        {
            units[column] = Unit::Leading;
        } else if (cell.flags - SGR_FLAGS - wrap_allowed).is_empty()
            && (cell.c.width() == Some(1) || cell.c == '\t')
            && pen_paintable(cell)
            && zerowidth_paintable(cell)
        {
            units[column] = Unit::Single;
        }
        column += 1;
    }
    units
}

/// A cell that an erase leaves as it is, so the REP trick can print into it
/// and erase it again.
fn erasable(cell: &Cell) -> bool {
    cell.c == ' '
        && cell.fg == Color::Named(NamedColor::Foreground)
        && cell.flags.is_empty()
        && extra_empty(cell)
        && bg_paintable(&cell.bg)
}

// ---------------------------------------------------------------------------
// The writer: emits sequences while tracking what the restoring emulator's
// cursor template, position and character sets will be, so that it can skip
// sequences that change nothing.

/// The SGR-settable part of a cell.
#[derive(Clone, PartialEq)]
struct Pen {
    fg: Color,
    bg: Color,
    flags: Flags,
    underline: Option<Color>,
}

impl Pen {
    fn plain() -> Pen {
        Pen {
            fg: Color::Named(NamedColor::Foreground),
            bg: Color::Named(NamedColor::Background),
            flags: Flags::empty(),
            underline: None,
        }
    }

    fn of(cell: &Cell) -> Pen {
        Pen {
            fg: cell.fg,
            bg: cell.bg,
            flags: cell.flags & SGR_FLAGS,
            underline: cell.underline_color(),
        }
    }
}

/// The SGR parameter for each attribute flag, setting it.
const FLAG_CODES: [(Flags, &str); 11] = [
    (Flags::BOLD, "1"),
    (Flags::DIM, "2"),
    (Flags::ITALIC, "3"),
    (Flags::UNDERLINE, "4"),
    (Flags::DOUBLE_UNDERLINE, "4:2"),
    (Flags::UNDERCURL, "4:3"),
    (Flags::DOTTED_UNDERLINE, "4:4"),
    (Flags::DASHED_UNDERLINE, "4:5"),
    (Flags::INVERSE, "7"),
    (Flags::HIDDEN, "8"),
    (Flags::STRIKEOUT, "9"),
];

/// SGR parameters that take a template from `from` to `to` changing nothing
/// else: attributes turned off (22 turns off bold and dim together, so the
/// one that stays is set again), then on, then each color that differs, its
/// default as 39, 49 or 59.
fn pen_changes(from: &Pen, to: &Pen) -> Vec<String> {
    let mut params = Vec::new();
    let removed = from.flags - to.flags;
    let mut added = to.flags - from.flags;
    let bold_dim = Flags::BOLD | Flags::DIM;
    if removed.intersects(bold_dim) {
        params.push("22".to_owned());
        added |= to.flags & bold_dim;
    }
    if removed.contains(Flags::ITALIC) {
        params.push("23".to_owned());
    }
    // One underline style replaces another, so only removing the last needs
    // its own parameter.
    if removed.intersects(Flags::ALL_UNDERLINES) && !to.flags.intersects(Flags::ALL_UNDERLINES) {
        params.push("24".to_owned());
    }
    for (flag, code) in [
        (Flags::INVERSE, "27"),
        (Flags::HIDDEN, "28"),
        (Flags::STRIKEOUT, "29"),
    ] {
        if removed.contains(flag) {
            params.push(code.to_owned());
        }
    }
    for (flag, code) in FLAG_CODES {
        if added.contains(flag) {
            params.push(code.to_owned());
        }
    }
    if from.fg != to.fg {
        match to.fg {
            Color::Named(NamedColor::Foreground) => params.push("39".to_owned()),
            fg => push_color(&mut params, &fg, 30, 90, 38),
        }
    }
    if from.bg != to.bg {
        match to.bg {
            Color::Named(NamedColor::Background) => params.push("49".to_owned()),
            bg => push_color(&mut params, &bg, 40, 100, 48),
        }
    }
    if from.underline != to.underline {
        match &to.underline {
            Some(underline) => push_color(&mut params, underline, 0, 0, 58),
            None => params.push("59".to_owned()),
        }
    }
    params
}

/// SGR parameters that reset the template and set all of `pen` (without an
/// underline color: see [`Writer::set_pen`]).
fn pen_reset(pen: &Pen) -> Vec<String> {
    let mut params = vec!["0".to_owned()];
    for (flag, code) in FLAG_CODES {
        if pen.flags.contains(flag) {
            params.push(code.to_owned());
        }
    }
    push_color(&mut params, &pen.fg, 30, 90, 38);
    push_color(&mut params, &pen.bg, 40, 100, 48);
    params
}

/// Appends SGR parameters for `color`: `base + n` for the first eight ANSI
/// colors, `bright + n - 8` for the next eight, `extended;5;n` or
/// `extended;2;r;g;b` for the rest, nothing for the default.
fn push_color(params: &mut Vec<String>, color: &Color, base: u8, bright: u8, extended: u8) {
    match color {
        Color::Named(named) if (*named as usize) < 8 => {
            params.push(format!("{}", base as usize + *named as usize))
        }
        Color::Named(named) if (*named as usize) < 16 => {
            params.push(format!("{}", bright as usize + *named as usize - 8))
        }
        Color::Named(_) => {}
        Color::Indexed(index) => params.push(format!("{extended};5;{index}")),
        Color::Spec(rgb) => params.push(format!("{extended};2;{};{};{}", rgb.r, rgb.g, rgb.b)),
    }
}

struct Writer {
    out: Vec<u8>,
    /// Bytes already dropped from `out` while measuring.
    dropped: usize,
    columns: usize,
    /// The restoring emulator's cursor template, as far as SGR sets it.
    pen: Pen,
    /// The restoring emulator's active hyperlink.
    link: Option<Hyperlink>,
    /// The cursor position (line, column), when known and with no wrap
    /// pending.
    pos: Option<(i32, usize)>,
    /// Whether `pos`'s column is certain. After a character that is not
    /// ASCII it is not: the reader's width table may give that character
    /// another width than this build's, and the next cell is then placed
    /// with an absolute column (CHA), so that a changed width moves only the
    /// character itself (module docs of `super::super`, "Character widths").
    column_trusted: bool,
    charsets: [StandardCharset; 4],
    /// The last character printed, which the restoring parser will repeat
    /// for REP.
    last_printed: Option<char>,
}

impl Writer {
    /// Everything written so far, dropped or not.
    fn written(&self) -> usize {
        self.dropped + self.out.len()
    }

    fn raw(&mut self, text: &str) {
        self.out.extend_from_slice(text.as_bytes());
    }

    /// Move to (`line`, `column`): nothing or CUF when the column on the
    /// known line is certain, CHA when only the line is, CUP otherwise.
    fn cup(&mut self, line: i32, column: usize) {
        match self.pos {
            Some((known_line, known_column))
                if known_line == line && self.column_trusted && known_column == column => {}
            Some((known_line, known_column))
                if known_line == line && self.column_trusted && column > known_column =>
            {
                match column - known_column {
                    1 => self.raw("\x1b[C"),
                    n => self.raw(&format!("\x1b[{n}C")),
                }
            }
            Some((known_line, _)) if known_line == line => {
                self.raw(&format!("\x1b[{}G", column + 1))
            }
            _ => self.raw(&format!("\x1b[{};{}H", line + 1, column + 1)),
        }
        self.known(line, column);
    }

    /// The cursor is at (`line`, `column`), certainly.
    fn known(&mut self, line: i32, column: usize) {
        self.pos = Some((line, column));
        self.column_trusted = true;
    }

    /// Make the restoring emulator's template `pen`, with SGR that changes
    /// only what differs, or resets and sets everything when that is
    /// shorter and keeps the underline color (none is wanted).
    ///
    /// An underline color lives in the template's extra, which the cells
    /// printed from it share. A reset clears it, so resetting to change the
    /// foreground and setting the same color again would give every cell
    /// painted after it an extra of its own where the program's cells shared
    /// one, until the restore's extra storage is full and the rest lose their
    /// underline colors. So the color is only ever set when it changes.
    fn set_pen(&mut self, pen: &Pen) {
        if self.pen == *pen {
            return;
        }
        let mut params = pen_changes(&self.pen, pen);
        if pen.underline.is_none() {
            let reset = pen_reset(pen);
            if reset.join(";").len() < params.join(";").len() {
                params = reset;
            }
        }
        // At most 28 parameters, under vte's limit of 32.
        self.raw(&format!("\x1b[{}m", params.join(";")));
        self.pen = pen.clone();
    }

    fn set_link(&mut self, link: Option<Hyperlink>) {
        if self.link == link {
            return;
        }
        match &link {
            Some(link) => self.raw(&format!("\x1b]8;id={};{}\x1b\\", link.id(), link.uri())),
            None => self.raw("\x1b]8;;\x1b\\"),
        }
        self.link = link;
    }

    fn set_charset(&mut self, slot: usize, charset: StandardCharset) {
        if self.charsets[slot] == charset {
            return;
        }
        let intermediate = ['(', ')', '*', '+'][slot];
        let last = match charset {
            StandardCharset::Ascii => 'B',
            StandardCharset::SpecialCharacterAndLineDrawing => '0',
        };
        self.raw(&format!("\x1b{intermediate}{last}"));
        self.charsets[slot] = charset;
    }

    fn ascii_charsets(&mut self) {
        for slot in 0..4 {
            self.set_charset(slot, StandardCharset::Ascii);
        }
    }

    fn plain(&mut self) {
        self.set_pen(&Pen::plain());
        self.set_link(None);
    }

    fn print(&mut self, c: char) {
        let mut buf = [0; 4];
        self.out
            .extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
        self.last_printed = Some(c);
        if !c.is_ascii() {
            self.column_trusted = false;
        }
        let width = c.width().unwrap_or(0);
        if width > 0 {
            self.pos = match self.pos {
                Some((line, column)) if column + width < self.columns => {
                    Some((line, column + width))
                }
                _ => None,
            };
        }
    }

    fn ech(&mut self, count: usize) {
        match count {
            1 => self.raw("\x1b[X"),
            n => self.raw(&format!("\x1b[{n}X")),
        }
    }

    /// Print one unit at (`line`, `column`). The cells it lands on must hold
    /// no wide-character halves yet, so that alacritty's input runs none of
    /// its wide-character cleanup.
    fn paint_unit(&mut self, row: &Row<Cell>, line: i32, column: usize, unit: Unit) {
        if !matches!(unit, Unit::Single | Unit::Pair) {
            return;
        }
        let cell = &row[Column(column)];
        self.cup(line, column);
        self.set_pen(&Pen::of(cell));
        self.set_link(cell.hyperlink());
        let tab = cell.c == '\t';
        self.print(if tab { ' ' } else { cell.c });
        for &mark in cell.zerowidth().unwrap_or(&[]) {
            self.print(mark);
        }
        if tab {
            // HT stores '\t' in the blank cell it starts from (for copying)
            // and keeps that cell's attributes.
            self.pos = None;
            self.cup(line, column);
            self.raw("\t");
            self.pos = None;
        }
    }

    /// Like `paint_unit`, with the cursor already at the unit's column
    /// (perhaps relative to an origin-mode region).
    fn paint_unit_at_cursor(&mut self, row: &Row<Cell>, column: usize) {
        let cell = &row[Column(column)];
        self.set_pen(&Pen::of(cell));
        self.set_link(cell.hyperlink());
        self.print(cell.c);
        for &mark in cell.zerowidth().unwrap_or(&[]) {
            self.print(mark);
        }
        self.pos = None;
    }
}

// ---------------------------------------------------------------------------
// The painter: grids and cursors.

struct Painter {
    w: Writer,
    fixups: Fixups,
    lines: usize,
    /// Whether cell contents are painted; when not, no cell fixup is made
    /// either (`Content::Nothing`).
    cells: bool,
    mode: Mode,
    cell_fixup_bytes: usize,
    cell_fixup_count: usize,
    history_costs: Vec<usize>,
}

impl Painter {
    fn new(columns: usize, lines: usize, cells: bool, mode: Mode) -> Painter {
        let (stream, cell_fixups) = match mode {
            Mode::Keep {
                stream,
                cell_fixups,
            } => (stream, cell_fixups),
            Mode::Measure => (1 << 16, 0),
        };
        let mut fixups = Fixups::default();
        fixups.cells.buf.reserve_exact(cell_fixups);
        Painter {
            w: Writer {
                out: Vec::with_capacity(stream),
                dropped: 0,
                columns,
                pen: Pen::plain(),
                link: None,
                pos: Some((0, 0)),
                column_trusted: true,
                charsets: [StandardCharset::Ascii; 4],
                last_printed: None,
            },
            fixups,
            lines,
            cells,
            mode,
            cell_fixup_bytes: 0,
            cell_fixup_count: 0,
            history_costs: Vec::new(),
        }
    }

    fn finish(mut self) -> Painted {
        let stream_len = self.w.written();
        if !self.mode.keeps() {
            self.w.out.clear();
        }
        Painted {
            stream: self.w.out,
            fixups: self.fixups,
            stream_len,
            cell_fixup_bytes: self.cell_fixup_bytes,
            cell_fixup_count: self.cell_fixup_count,
            history_costs: self.history_costs,
        }
    }

    fn fix_cell(&mut self, grid: GridSel, line: i32, column: usize, cell: &Cell) {
        if !self.cells {
            return;
        }
        self.cell_fixup_count += 1;
        self.cell_fixup_bytes += CELL_FIXUP_HEADER_BYTES + wire::cell_len(cell);
        if self.mode.keeps() {
            let record = &mut self.fixups.cells;
            record.u8(grid_number(grid));
            record.i32(line);
            record.len(column);
            record.cell(cell);
        }
    }

    /// While measuring, drop what has been written so far.
    fn drop_measured(&mut self) {
        if !self.mode.keeps() {
            self.w.dropped += self.w.out.len();
            self.w.out.clear();
        }
    }

    /// Paint `rows` (oldest first, each with its line number in the grid
    /// fixups refer to) onto a blank screen with the full scroll region, a
    /// plain pen and ASCII character sets, scrolling the rows before the last
    /// `lines` into history. The first `history` rows are the history,
    /// whose costs are recorded.
    fn paint_rows(&mut self, rows: &[(i32, &Row<Cell>)], grid: GridSel, history: usize) {
        let lines = self.lines;
        let columns = self.w.columns;
        let last = columns - 1;
        let count = rows.len();
        // Rows that a wrap trick (below) left a stray print in, to be erased
        // before they are painted.
        let mut dirty = vec![false; lines];
        // Whether the last row's wrap already scrolled the screen.
        let mut scrolled = false;
        for (index, &(source_line, row)) in rows.iter().enumerate() {
            self.drop_measured();
            if index < history {
                self.history_costs
                    .push(self.w.written() + self.cell_fixup_bytes);
            }
            let y = index.min(lines - 1) as i32;
            let screen_row = y as usize;
            if index >= lines && !scrolled {
                // Scroll the top row into history. The new bottom row takes
                // the pen's background, so the pen must be plain.
                self.w.plain();
                self.w.cup(lines as i32 - 1, 0);
                self.w.raw("\n");
                dirty[lines - 1] = false;
            }
            scrolled = false;
            if dirty[screen_row] {
                self.w.plain();
                self.w.cup(y, 0);
                self.w.ech(columns);
                dirty[screen_row] = false;
            }
            let units = classify(row, columns);
            for (column, unit) in units.iter().enumerate() {
                if *unit == Unit::Fix {
                    self.fix_cell(grid, source_line, column, &row[Column(column)]);
                }
            }
            let wraps = row[Column(last)].flags.contains(Flags::WRAPLINE);
            let leading = units[last] == Unit::Leading;
            let wrap_trick = wraps && !leading && units[last] != Unit::Fix;
            if wrap_trick {
                // Leave a saved cursor with a wrap pending in the last
                // column; DECRC brings it back once the row is painted, and
                // the next print sets the row's wrap flag.
                self.w.plain();
                self.w.cup(y, last);
                self.w.print('x');
                self.w.raw("\x1b7");
            }
            for (column, unit) in units.iter().enumerate() {
                self.w.paint_unit(row, y, column, *unit);
            }
            if !(leading || wrap_trick) {
                continue;
            }
            let at_bottom = screen_row == lines - 1;
            let region_trick = index == count - 1 && at_bottom;
            // The units at the start of this row that the region trick's
            // stray print (below) lands on.
            let stray = if leading { 2 } else { 1 };
            let stray_end = units[..stray.min(columns)]
                .iter()
                .enumerate()
                .filter(|&(_, &unit)| unit == Unit::Pair)
                .map(|(column, _)| column + 2)
                .fold(stray, usize::max)
                .min(columns);
            if region_trick {
                // Too few lines for a scroll region above the row, or a row
                // so narrow that the stray print, and the repaint after it,
                // reach the very cell whose wrap flag the trick sets (one
                // column, or a wide character filling two): a fixup instead.
                if lines < 3 || stray_end >= columns {
                    self.fix_cell(grid, source_line, last, &row[Column(last)]);
                    continue;
                }
                // Blank the cells the stray print lands on first: printing over
                // a wide character there would make alacritty clear the
                // previous row's leading spacer.
                self.w.plain();
                self.w.cup(y, 0);
                self.w.ech(stray_end);
                // The wrap must not scroll on the last row: with the cursor
                // below the scroll region, a wrap stays on its line.
                self.w.raw(&format!("\x1b[1;{}r", lines - 1));
                self.w.known(0, 0);
            }
            if leading {
                let cell = &row[Column(last)];
                self.w.cup(y, last);
                self.w.set_pen(&Pen::of(cell));
                self.w.set_link(cell.hyperlink());
                self.w.ech(1);
                self.w.print(WIDE_PLACEHOLDER);
            } else {
                self.w.raw("\x1b8");
                self.w.pen = Pen::plain();
                self.w.link = None;
                self.w.print('x');
            }
            self.w.pos = None;
            if region_trick {
                self.w.raw("\x1b[r");
                self.w.known(0, 0);
                // The wrap left a stray print at the start of this same row:
                // repaint the units it touched.
                self.w.plain();
                self.w.cup(y, 0);
                self.w.ech(stray_end);
                for (column, &unit) in units.iter().enumerate().take(stray_end) {
                    self.w.paint_unit(row, y, column, unit);
                }
            } else if at_bottom {
                scrolled = true;
                dirty[lines - 1] = true;
            } else {
                dirty[screen_row + 1] = true;
            }
        }
    }

    /// Make `cursor` the current cursor: its position (with its pending wrap,
    /// if any), template and link, and character sets. `content` is the grid
    /// it belongs to, whose cell a pending wrap reprints. `origin_region` is
    /// the scroll region when origin mode is already on.
    fn set_cursor(
        &mut self,
        cursor: &Cursor<Cell>,
        content: &Grid<Cell>,
        grid: GridSel,
        saved: bool,
        origin_region: Option<std::ops::Range<i32>>,
    ) {
        let last = self.w.columns - 1;
        let line = cursor.point.line.0;
        let column = cursor.point.column.0;
        let fix = |fixups: &mut Fixups| {
            fixups.cursors.push(CursorFixup {
                grid,
                saved,
                cursor: cursor.clone(),
            })
        };
        let template_paintable = pen_paintable(&cursor.template)
            && (cursor.template.flags - SGR_FLAGS) == Flags::empty();
        if !template_paintable {
            fix(&mut self.fixups);
        }
        // Under origin mode CUP cannot leave the scroll region, but DECRC can
        // put the cursor outside it (and an empty region has no inside).
        if let Some(region) = &origin_region
            && !region.contains(&line)
        {
            fix(&mut self.fixups);
            return;
        }
        let origin_top = origin_region.map(|region| region.start);
        let origin = origin_top.unwrap_or(0);
        if cursor.input_needs_wrap {
            if column != last {
                fix(&mut self.fixups);
            } else {
                // Printing into the last column leaves a wrap pending, so
                // reprint that cell (whether a print or a resize raised it).
                let row = &content[Line(line)];
                // Without cell contents the row is blank in the restored
                // screen, so only a blank can be reprinted.
                let units = if self.cells {
                    classify(row, self.w.columns)
                } else {
                    vec![Unit::Skip; self.w.columns]
                };
                let cell = &row[Column(last)];
                let wrapped = cell.flags.contains(Flags::WRAPLINE);
                self.w.pos = None;
                // Autowrap is on here (the stream's own setting comes last),
                // so the reprint raises the flag. A cursor on the last column
                // without the flag (the column written with autowrap off)
                // takes the branch below and is placed, not reprinted.
                match units[last] {
                    Unit::Single if !wrapped && cell.c != '\t' => {
                        self.raw_cup(line - origin, last);
                        self.w.paint_unit_at_cursor(row, last);
                    }
                    Unit::Spacer if !wrapped => {
                        self.raw_cup(line - origin, last - 1);
                        self.w.ech(2);
                        self.w.paint_unit_at_cursor(row, last - 1);
                    }
                    _ => {
                        // Blank the cell first: printing over a wide
                        // character's half would make alacritty clear the
                        // row above's leading spacer (on two columns, the
                        // last column is the second), which no fixup here
                        // restores. ECH clears without that cleanup.
                        self.raw_cup(line - origin, last);
                        self.w.plain();
                        self.w.ech(1);
                        self.w.print(' ');
                        self.fix_cell(grid, line, last, cell);
                        if units[last] == Unit::Spacer {
                            self.fix_cell(grid, line, last - 1, &row[Column(last - 1)]);
                        }
                    }
                }
                self.w.pos = None;
            }
        } else {
            self.w.pos = None;
            self.raw_cup(line - origin, column);
            if origin_top.is_none() {
                self.w.known(line, column);
            }
        }
        // A template the stream cannot give the cursor comes from its fixup;
        // writing its link raw could put any sequence into the stream.
        if template_paintable {
            self.w.set_pen(&Pen::of(&cursor.template));
            self.w.set_link(cursor.template.hyperlink());
        } else {
            self.w.plain();
        }
        for (slot, &index) in CHARSET_SLOTS.iter().enumerate() {
            self.w.set_charset(slot, cursor.charsets[index]);
        }
    }

    fn raw_cup(&mut self, line: i32, column: usize) {
        self.w.raw(&format!("\x1b[{};{}H", line + 1, column + 1));
    }

    /// Leave `c` as the REP character by printing it into blank, erasable
    /// cells of the active screen and erasing them again. Returns false when
    /// no such cells exist, or when `c` is a control the replay parser would
    /// not print.
    fn print_rep_char(&mut self, active: &Grid<Cell>, c: char) -> bool {
        // A character of no width is a control (C0, DEL or C1): printed into
        // the stream, it would be refused (DEL) or executed rather than
        // printed (a C1 control, which vte prints only when its encoding
        // arrives split across two reads), so it goes to the REP fixup.
        let width = c.width();
        if width.is_none() {
            return false;
        }
        let columns = self.w.columns;
        for line in 0..self.lines {
            let row = &active[Line(line as i32)];
            for column in 0..columns.saturating_sub(2) {
                let needed = if width == Some(2) { 2 } else { 1 };
                let first = &row[Column(column)];
                let last = &row[Column(column + needed - 1)];
                if (column..column + needed).all(|x| erasable(&row[Column(x)]))
                    && first.bg == last.bg
                {
                    // A zero-width character joins the cell before it.
                    let print_at = if width == Some(0) { column + 1 } else { column };
                    self.w.plain();
                    self.w.cup(line as i32, print_at);
                    self.w.print(c);
                    self.w.pos = None;
                    self.w.cup(line as i32, column);
                    self.w.set_pen(&Pen {
                        bg: first.bg,
                        ..Pen::plain()
                    });
                    self.w.ech(needed);
                    return true;
                }
            }
        }
        false
    }
}

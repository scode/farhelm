//! Field-by-field comparison of two emulators, covering everything a program
//! or a viewer can observe, including what alacritty keeps private without
//! the vendored accessors (the hidden screen, scroll region, tab stops, title
//! stack, character sets) and the parser's REP character.
//!
//! Ported from the snapshot spike's comparison, which deliberate encoder
//! defects showed catches each kind of loss. Left out on purpose, as state of
//! a viewer rather than of the terminal: selection, the vi-mode cursor and
//! viewport offset, focus, and damage.

use farhelm_terminal_emulator::Emulator;
use farhelm_terminal_emulator::alacritty_terminal::grid::{Cursor, Dimensions, Grid};
use farhelm_terminal_emulator::alacritty_terminal::index::{Column, Line};
use farhelm_terminal_emulator::alacritty_terminal::term::TermMode;
use farhelm_terminal_emulator::alacritty_terminal::term::cell::{Cell, Hyperlink};
use farhelm_terminal_emulator::alacritty_terminal::term::color::COUNT;
use farhelm_terminal_emulator::alacritty_terminal::vte::ansi::CharsetIndex;

/// The differences found, the first few in full.
#[derive(Debug, Default)]
pub struct Diff {
    pub shown: Vec<String>,
    pub total: usize,
}

impl Diff {
    fn push(&mut self, difference: String) {
        self.total += 1;
        if self.shown.len() < 6 {
            self.shown.push(difference);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.total == 0
    }
}

/// Every observable difference between `a` and `b`.
pub fn diff(a: &Emulator, b: &Emulator) -> Diff {
    let mut d = Diff::default();
    if a.size() != b.size() {
        d.push(format!("size {:?} vs {:?}", a.size(), b.size()));
        return d;
    }
    let (ta, tb) = (a.term(), b.term());
    let ignored = TermMode::VI;
    if (*ta.mode() - ignored) != (*tb.mode() - ignored) {
        d.push(format!("mode {:?} vs {:?}", ta.mode(), tb.mode()));
    }
    diff_grid(&mut d, "active", ta.grid(), tb.grid(), true);
    // alacritty clears the alternate screen's cells on entry, so while it is
    // hidden only its saved cursor can ever be observed again; while it
    // shows, the hidden primary screen is wholly observable.
    let alternate = ta.mode().contains(TermMode::ALT_SCREEN);
    diff_grid(
        &mut d,
        "inactive",
        ta.inactive_grid(),
        tb.inactive_grid(),
        alternate,
    );
    if ta.scroll_region() != tb.scroll_region() {
        d.push(format!(
            "scroll region {:?} vs {:?}",
            ta.scroll_region(),
            tb.scroll_region()
        ));
    }
    if ta.tab_stops() != tb.tab_stops() {
        d.push("tab stops".to_owned());
    }
    if ta.active_charset() != tb.active_charset() {
        d.push(format!(
            "active charset {:?} vs {:?}",
            ta.active_charset(),
            tb.active_charset()
        ));
    }
    if ta.title() != tb.title() {
        d.push(format!("title {:?} vs {:?}", ta.title(), tb.title()));
    }
    if ta.title_stack() != tb.title_stack() {
        d.push(format!(
            "title stack {:?} vs {:?}",
            ta.title_stack(),
            tb.title_stack()
        ));
    }
    if ta.keyboard_mode_stacks() != tb.keyboard_mode_stacks() {
        d.push("keyboard mode stacks".to_owned());
    }
    for index in 0..COUNT {
        if ta.colors()[index] != tb.colors()[index] {
            d.push(format!(
                "color {index}: {:?} vs {:?}",
                ta.colors()[index],
                tb.colors()[index]
            ));
        }
    }
    if ta.cursor_style() != tb.cursor_style() {
        d.push(format!(
            "cursor style {:?} vs {:?}",
            ta.cursor_style(),
            tb.cursor_style()
        ));
    }
    if a.is_clean() != b.is_clean() {
        d.push(format!("clean {} vs {}", a.is_clean(), b.is_clean()));
    }
    if a.sync_buffered_bytes() != b.sync_buffered_bytes() {
        d.push(format!(
            "update bytes {} vs {}",
            a.sync_buffered_bytes(),
            b.sync_buffered_bytes()
        ));
    }
    if a.preceding_char() != b.preceding_char() {
        d.push(format!(
            "REP character {:?} vs {:?}",
            a.preceding_char(),
            b.preceding_char()
        ));
    }
    d
}

/// Compare two grids: their saved cursors always, and with `cells` also
/// their cursors and every row, history included.
fn diff_grid(d: &mut Diff, name: &str, a: &Grid<Cell>, b: &Grid<Cell>, cells: bool) {
    if !cursor_eq(&a.saved_cursor, &b.saved_cursor) {
        d.push(format!(
            "{name} saved cursor: {} vs {}",
            show_cursor(&a.saved_cursor),
            show_cursor(&b.saved_cursor)
        ));
    }
    if !cells {
        return;
    }
    if !cursor_eq(&a.cursor, &b.cursor) {
        d.push(format!(
            "{name} cursor: {} vs {}",
            show_cursor(&a.cursor),
            show_cursor(&b.cursor)
        ));
    }
    if a.history_size() != b.history_size() {
        d.push(format!(
            "{name} history {} vs {}",
            a.history_size(),
            b.history_size()
        ));
    }
    let history = a.history_size().min(b.history_size()) as i32;
    for line in -history..a.screen_lines() as i32 {
        for column in 0..a.columns() {
            let (ca, cb) = (
                &a[Line(line)][Column(column)],
                &b[Line(line)][Column(column)],
            );
            if !cell_eq(ca, cb) {
                d.push(format!(
                    "{name} cell {line},{column}: {} vs {}",
                    show_cell(ca),
                    show_cell(cb)
                ));
            }
        }
    }
}

/// Cells look the same: `Cell`'s equality, except that empty extra storage
/// equals none and generated hyperlink ids are interchangeable (see
/// [`link_eq`]).
pub fn cell_eq(a: &Cell, b: &Cell) -> bool {
    // `Cell`'s own equality first: it settles almost every cell without
    // cloning the hyperlinks out of both.
    a == b
        || a.c == b.c
            && a.fg == b.fg
            && a.bg == b.bg
            && a.flags == b.flags
            && a.zerowidth().unwrap_or(&[]) == b.zerowidth().unwrap_or(&[])
            && a.underline_color() == b.underline_color()
            && link_eq(a.hyperlink().as_ref(), b.hyperlink().as_ref())
}

/// Links are the same when their URIs are and their ids are, except that
/// alacritty generates `<n>_alacritty` ids for links sent without one from a
/// counter shared by every emulator in the process, so two emulators fed the
/// same output never agree on them; two generated ids count as equal.
pub fn link_eq(a: Option<&Hyperlink>, b: Option<&Hyperlink>) -> bool {
    let generated = |link: &Hyperlink| {
        link.id()
            .strip_suffix("_alacritty")
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|byte| byte.is_ascii_digit()))
    };
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            a.uri() == b.uri() && (a.id() == b.id() || (generated(a) && generated(b)))
        }
        _ => false,
    }
}

fn cursor_eq(a: &Cursor<Cell>, b: &Cursor<Cell>) -> bool {
    a.point == b.point
        && cell_eq(&a.template, &b.template)
        && a.input_needs_wrap == b.input_needs_wrap
        && CHARSETS
            .iter()
            .all(|&slot| a.charsets[slot] == b.charsets[slot])
}

const CHARSETS: [CharsetIndex; 4] = [
    CharsetIndex::G0,
    CharsetIndex::G1,
    CharsetIndex::G2,
    CharsetIndex::G3,
];

fn show_cell(cell: &Cell) -> String {
    format!(
        "{:?} {:?}/{:?} {:?} zw={:?} ul={:?} link={:?}",
        cell.c,
        cell.fg,
        cell.bg,
        cell.flags,
        cell.zerowidth(),
        cell.underline_color(),
        cell.hyperlink()
            .map(|link| (link.id().to_owned(), link.uri().to_owned()))
    )
}

fn show_cursor(cursor: &Cursor<Cell>) -> String {
    format!(
        "{:?} wrap={} template=({}) charsets={:?}",
        cursor.point,
        cursor.input_needs_wrap,
        show_cell(&cursor.template),
        CHARSETS.map(|slot| cursor.charsets[slot])
    )
}

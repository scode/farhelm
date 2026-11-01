//! A plain-text description of an emulator's observable state, for the
//! golden snapshot fixtures: each fixture blob is checked in beside the
//! description of the state it must restore to, so that what a blob means is
//! pinned in the repository, not only by re-running the script that made it.
//!
//! The text is meant to be read in a review: one line per grid row with its
//! characters, then a line for every cell that is not plain, then the rest of
//! the state. It names alacritty's types through their `Debug` names (flag
//! and mode names, color names), which are not part of any snapshot format;
//! if an alacritty upgrade renames one, change this function so the
//! descriptions read the same, rather than rewriting the fixtures.

use std::fmt::Write as _;

use farhelm_terminal_emulator::Emulator;
use farhelm_terminal_emulator::alacritty_terminal::grid::{Cursor, Dimensions, Grid};
use farhelm_terminal_emulator::alacritty_terminal::index::{Column, Line};
use farhelm_terminal_emulator::alacritty_terminal::term::TermMode;
use farhelm_terminal_emulator::alacritty_terminal::term::cell::{Cell, Flags};
use farhelm_terminal_emulator::alacritty_terminal::term::color::COUNT;
use farhelm_terminal_emulator::alacritty_terminal::vte::ansi::{CharsetIndex, Color, NamedColor};

/// The description of `emulator`.
pub fn describe(emulator: &Emulator) -> String {
    let term = emulator.term();
    let mut out = String::new();
    let size = emulator.size();
    writeln!(out, "size {}x{}", size.columns, size.lines).unwrap();
    writeln!(out, "mode {:?}", *term.mode() - TermMode::VI).unwrap();
    let region = term.scroll_region();
    writeln!(out, "scroll region {}..{}", region.start.0, region.end.0).unwrap();
    let stops: Vec<String> = term
        .tab_stops()
        .iter()
        .enumerate()
        .filter(|&(_, &stop)| stop)
        .map(|(column, _)| column.to_string())
        .collect();
    writeln!(out, "tab stops {}", stops.join(" ")).unwrap();
    writeln!(out, "active charset {:?}", term.active_charset()).unwrap();
    writeln!(out, "title {:?}", term.title()).unwrap();
    writeln!(out, "title stack {:?}", term.title_stack()).unwrap();
    writeln!(out, "cursor style {:?}", term.cursor_style()).unwrap();
    for index in 0..COUNT {
        if let Some(rgb) = term.colors()[index] {
            writeln!(
                out,
                "color {index} #{:02x}{:02x}{:02x}",
                rgb.r, rgb.g, rgb.b
            )
            .unwrap();
        }
    }
    writeln!(out, "REP character {:?}", emulator.preceding_char()).unwrap();
    writeln!(out, "clean {}", emulator.is_clean()).unwrap();
    match emulator.trim_remaining() {
        None => writeln!(out, "history trimmed no").unwrap(),
        Some(lines) => writeln!(out, "history trimmed for {lines} more lines").unwrap(),
    }
    let alternate = term.mode().contains(TermMode::ALT_SCREEN);
    let (primary, other) = if alternate {
        (term.inactive_grid(), term.grid())
    } else {
        (term.grid(), term.inactive_grid())
    };
    describe_grid(&mut out, "primary", primary, true);
    // While hidden, the alternate screen's cells are cleared before they can
    // show again; only its saved cursor matters then.
    describe_grid(&mut out, "alternate", other, alternate);
    out
}

fn describe_grid(out: &mut String, name: &str, grid: &Grid<Cell>, cells: bool) {
    writeln!(out, "{name} saved cursor {}", cursor(&grid.saved_cursor)).unwrap();
    if !cells {
        return;
    }
    writeln!(out, "{name} cursor {}", cursor(&grid.cursor)).unwrap();
    writeln!(out, "{name} history {}", grid.history_size()).unwrap();
    let first = -(grid.history_size() as i32);
    for line in first..grid.screen_lines() as i32 {
        let row = &grid[Line(line)];
        let mut text = String::new();
        for column in 0..grid.columns() {
            let cell = &row[Column(column)];
            text.push(cell.c);
            text.extend(cell.zerowidth().unwrap_or(&[]));
        }
        writeln!(out, "{name} {line:>4} |{text}|").unwrap();
        for column in 0..grid.columns() {
            let cell = &row[Column(column)];
            if !plain(cell) {
                writeln!(out, "{name} {line:>4},{column}: {}", attributes(cell)).unwrap();
            }
        }
    }
}

/// A cell with nothing but its character to describe.
fn plain(cell: &Cell) -> bool {
    cell.fg == Color::Named(NamedColor::Foreground)
        && cell.bg == Color::Named(NamedColor::Background)
        && cell.flags.is_empty()
        && cell.underline_color().is_none()
        && cell.hyperlink().is_none()
}

fn color(color: Color) -> String {
    match color {
        Color::Named(named) => format!("{named:?}"),
        Color::Indexed(index) => format!("i{index}"),
        Color::Spec(rgb) => format!("#{:02x}{:02x}{:02x}", rgb.r, rgb.g, rgb.b),
    }
}

fn attributes(cell: &Cell) -> String {
    let mut text = format!("{:?} fg {} bg {}", cell.c, color(cell.fg), color(cell.bg));
    if cell.flags != Flags::empty() {
        write!(text, " flags {:?}", cell.flags).unwrap();
    }
    if let Some(underline) = cell.underline_color() {
        write!(text, " underline {}", color(underline)).unwrap();
    }
    if let Some(link) = cell.hyperlink() {
        write!(text, " link {:?} {:?}", link.id(), link.uri()).unwrap();
    }
    text
}

fn cursor(cursor: &Cursor<Cell>) -> String {
    let charsets = [
        CharsetIndex::G0,
        CharsetIndex::G1,
        CharsetIndex::G2,
        CharsetIndex::G3,
    ]
    .map(|slot| format!("{:?}", cursor.charsets[slot]));
    format!(
        "{},{}{} template {} charsets {}",
        cursor.point.line.0,
        cursor.point.column.0,
        if cursor.input_needs_wrap {
            " wrap pending"
        } else {
            ""
        },
        attributes(&cursor.template),
        charsets.join(" ")
    )
}

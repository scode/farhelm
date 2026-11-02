//! Snapshot format version 1. Frozen: once shipped, every byte this module
//! writes and every rule it reads by stays as it is, because hosts keep
//! version 1 blobs for as long as they run, and every later supervisor must
//! restore them (`super` module docs, "Compatibility"). A change goes into a
//! new version beside this one.
//!
//! # Layout
//!
//! After the four magic bytes `FHTS` and the format version (a `u16`, 1),
//! all integers big-endian:
//!
//! | Field | Encoding | Meaning |
//! | --- | --- | --- |
//! | flags | `u8` | bit 0: history is trimmed; bit 1: the screens' contents were left out to fit; other bits zero |
//! | columns, lines | `u16`, `u16` | the terminal's size, each 1 to 10,000 (the terminal host's range) |
//! | history | `u32` | how many history lines the stream rebuilds |
//! | trimmed for | `u32` | with flag bit 0, how many more lines must enter history before it no longer counts as trimmed (at least 1); otherwise 0 |
//! | width table | 3 × `u8`, `u64` | the character widths the stream was painted with: the Unicode version of `unicode-width`'s table (major, minor, update), and a fingerprint of the width it gives every character (`super::WidthTable`) |
//! | stream | `u32` length, bytes | escape sequences that rebuild the state in a fresh terminal of this size (`paint`) |
//! | cell fixups | `u32` count, records | grid, line, column, cell: cells to overwrite after the stream |
//! | cursor fixups | `u32` count, records | grid, which cursor, cursor: cursors to overwrite after the stream |
//! | REP character | `u8` tag | 0: as the stream left it; 1: none; 2: followed by the character (`u32`) |
//!
//! The records:
//!
//! - A cell fixup is a grid (`u8`: 0 for the screen that shows, 1 for the
//!   other one), a line (`i32`: 0 the top of the screen, negative into
//!   history, -1 the newest history line), a column (`u32`) and a cell.
//! - A cursor fixup is a grid, which cursor (`u8`: 0 the cursor, 1 the saved
//!   cursor) and a cursor.
//! - A cell is its character (`u32`, a Unicode scalar value), foreground and
//!   background colors, flags (`u16`, the bits `wire::FLAG_BITS` names),
//!   zero-width characters (`u8` count, then each as a `u32`), an underline
//!   color (`u8` 0, or 1 and a color) and a hyperlink (`u8` 0, or 1 and its
//!   `id` and URI, each a `u32` length and UTF-8).
//! - A color is a kind (`u8`) and its value: 0 a named color (`u16`, the
//!   color slot `wire::NAMED_COLORS` gives), 1 a palette index (`u8`), 2 RGB
//!   (three `u8`).
//! - A cursor is its line (`i32`), column (`u32`), whether a wrap is pending
//!   (`u8` 0 or 1, alacritty's `input_needs_wrap`), its template cell, and
//!   the character sets in G0 to G3 (four `u8`: 0 ASCII, 1 DEC line
//!   drawing).
//!
//! Nothing may follow the last field.
//!
//! # Reading
//!
//! A blob is untrusted: the host keeps it without looking inside, and a
//! corrupt or forged one must cost the supervisor a typed error, never a
//! panic, unbounded memory or unbounded work. The rules, in order:
//!
//! 1. The header. A side outside 1 to 10,000, an unknown flag, or a trimmed
//!    flag that disagrees with its count is malformed.
//! 2. Allocation, before anything is allocated. Two screens of this size
//!    over `super::MAX_RESTORE_CELLS` cells are refused as too large. The
//!    history is cut to what fits that bound beside them
//!    (`super::restorable_history`) and to the caller's scrollback,
//!    whichever is less, the oldest lines dropped; history cut by the bound
//!    is marked trimmed, as the encoder marks history it drops. The bound is
//!    the reading build's own policy, not part of this version: a build with
//!    a lower one restores the same blob with less history, so lowering it
//!    never strands the blobs hosts keep. Screens larger than the bound
//!    cannot be restored by any build with it: a terminal over about 2,900
//!    by 2,900, beyond what a supervisor-side emulator holds within its
//!    memory budget.
//! 3. The stream, before any of it is fed: only the sequences `paint` writes,
//!    and no more work than its size allows (`replay`, which says how work
//!    is counted). Then it is fed to a fresh emulator of the recorded size,
//!    keeping the history step 2 allows, with every side effect dropped, in
//!    pieces of at most about 64 KiB (`replay::FEED_PIECE`) so that the
//!    effects a piece queues before they are dropped stay small, and must
//!    leave it at a clean boundary with exactly that much history. Then the
//!    emulator gets the caller's full scrollback.
//! 4. The fixups. One outside the screen is malformed; one in history
//!    beyond what the restore kept is skipped, since that history was
//!    dropped by step 2. A cell no live output can make is malformed: more
//!    than 16 combining characters (`wire::V1_MAX_ZEROWIDTH`), a link OSC
//!    8 could not carry (an empty URI, a URI with more than 13 `;`, which
//!    is all vte's 16 OSC parameters leave it, a C0 control in either field,
//!    an `id` with `:` or `;`, an `id` over 250 bytes or a URI over 2083),
//!    or a cursor template with combining characters. These limits are
//!    version 1's own, equal to the vendored caps when it was fixed; the
//!    reader never goes by the emulator's current caps.
//! 5. The REP character. Tag 2 carries one the stream cannot set by
//!    printing it: DEL or a C1 control (which vte makes the REP character
//!    when it prints them: DEL always, a C1 control when its encoding
//!    arrives split across reads), or one with no blank cell to print into.
//!    A C0 control there is malformed: vte executes every one.
//!
//! Every extra a restore makes, the stream's as well as the fixups', goes
//! through the restored terminal's extra storage, within the same limit as
//! the program's own, and extras of the same content are shared
//! (`wire::Extras`): the stream is fed in pieces that end wherever the
//! cursor template's extra may have changed, and the template's new extra
//! is swapped for an earlier one of the same content before cells are
//! printed from it. The sharing keeps no extra alive that no cell or cursor
//! holds for long, so a restore charges about what its cells hold.

mod paint;
mod replay;
mod wire;

use std::time::Instant;

use alacritty_terminal::Term;
use alacritty_terminal::grid::{Cursor, Dimensions, Grid};
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Cell;

pub(super) use paint::{Content, Mode};
use wire::{Extras, Reader, Writer, malformed};

use super::{
    MAGIC, MAX_RESTORE_CELLS, MAX_SIDE, Restored, SnapshotError, WidthTable, restorable_history,
};
use crate::effects::{Collected, FeedMode, Listener};
use crate::emulator::{Emulator, Size};

/// This format's version number.
pub(super) const VERSION: u16 = 1;

/// The widest and tallest terminal a version 1 blob describes: the terminal
/// host's limit when version 1 was fixed (`super::MAX_SIDE`), but version
/// 1's own, so that a host that later allows larger terminals does not change
/// what this version reads. The encoder writes the emulator's size, which the
/// host keeps within `MAX_SIDE`; the assertion stops the build if that ever
/// exceeds what version 1 carries.
const V1_MAX_SIDE: u16 = 10_000;
const _: () = assert!(
    MAX_SIDE <= V1_MAX_SIDE,
    "the host allows terminals larger than version 1 describes"
);

const FLAG_HISTORY_TRIMMED: u8 = 1 << 0;
const FLAG_SCREENS_DROPPED: u8 = 1 << 1;

/// The fixed fields before the stream: magic, version, flags, size,
/// history, trimmed count, width table and the stream's length.
const HEADER_BYTES: usize = 4 + 2 + 1 + 2 + 2 + 4 + 4 + 3 + 8 + 4;
/// A cell fixup record's grid, line and column, before its cell.
const CELL_FIXUP_HEADER_BYTES: usize = 1 + 4 + 4;
/// The smallest a cell fixup record can be (a cell of the smallest colors
/// and no extras), for checking counts.
const MIN_CELL_FIXUP_BYTES: usize = CELL_FIXUP_HEADER_BYTES + 13;
/// Likewise for a cursor fixup record.
const MIN_CURSOR_FIXUP_BYTES: usize = 1 + 1 + 4 + 4 + 1 + 13 + 4;

/// Which of a terminal's two grids a fixup targets: the one showing when the
/// snapshot was taken, or the other. The restored terminal shows the same
/// screen, so the meaning carries over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GridSel {
    Active,
    Inactive,
}

/// A cursor the stream cannot place, written after it.
struct CursorFixup {
    grid: GridSel,
    /// The saved cursor (DECSC) rather than the cursor itself.
    saved: bool,
    cursor: Cursor<Cell>,
}

/// The states no escape sequence produces (see `paint`).
#[derive(Default)]
struct Fixups {
    /// The cell fixup records (cells the stream cannot paint, written after
    /// it), encoded as the blob holds them; `paint::Painted` counts them.
    cells: Writer,
    cursors: Vec<CursorFixup>,
    /// The REP character, when the stream leaves a different one: `None`
    /// when it is right.
    preceding_char: Option<Option<char>>,
}

/// How a blob came out of [`encode`] or [`encode_history`].
pub(super) struct Encoded {
    /// The blob; empty when measuring.
    pub(super) blob: Vec<u8>,
    /// The blob's length, kept or not.
    pub(super) len: usize,
    /// See `paint::Painted::history_costs`.
    pub(super) history_costs: Vec<usize>,
    /// The stream's and the cell fixups' encoded lengths, for keeping what
    /// was measured ([`Encoded::keep`]).
    stream_len: usize,
    cell_fixup_bytes: usize,
}

impl Encoded {
    /// The mode that keeps what this measured, its buffers reserved to the
    /// sizes measured.
    pub(super) fn keep(&self) -> Mode {
        Mode::Keep {
            stream: self.stream_len,
            cell_fixups: self.cell_fixup_bytes,
        }
    }
}

/// Encode `emulator`'s whole state, with `content` deciding how much of the
/// grids and `skip_titles` how many of the oldest title stack entries to
/// leave out. `trimmed_for` is recorded as is (0: not trimmed). The emulator
/// must be at a clean boundary.
pub(super) fn encode(
    emulator: &Emulator,
    content: Content,
    skip_titles: usize,
    trimmed_for: u32,
    mode: Mode,
) -> Encoded {
    let painted = paint::paint(
        &emulator.term,
        emulator.parser.preceding_char(),
        content,
        skip_titles,
        mode,
    );
    let mut flags = 0;
    if trimmed_for > 0 {
        flags |= FLAG_HISTORY_TRIMMED;
    }
    let history = match content {
        Content::Cells { skip_history } => emulator
            .primary_grid()
            .history_size()
            .saturating_sub(skip_history),
        Content::Nothing => {
            flags |= FLAG_SCREENS_DROPPED;
            0
        }
    };
    container(&emulator.term, flags, history, trimmed_for, painted, mode)
}

/// Encode only `emulator`'s primary history, without its `skip` oldest
/// lines, above a blank screen with everything else as in a fresh terminal.
/// Any parser state works: the stream starts from a fresh parser.
pub(super) fn encode_history(
    emulator: &Emulator,
    skip: usize,
    trimmed_for: u32,
    mode: Mode,
) -> Encoded {
    let painted = paint::paint_history(&emulator.term, skip, mode);
    let flags = if trimmed_for > 0 {
        FLAG_HISTORY_TRIMMED
    } else {
        0
    };
    let history = emulator.primary_grid().history_size().saturating_sub(skip);
    container(&emulator.term, flags, history, trimmed_for, painted, mode)
}

/// Lay out a painted state as a version 1 blob, or when it was only
/// measured, work out the blob's length.
fn container(
    term: &Term<Listener>,
    flags: u8,
    history: usize,
    trimmed_for: u32,
    painted: paint::Painted,
    mode: Mode,
) -> Encoded {
    let measured = mode == Mode::Measure;
    let fixups = painted.fixups;
    let mut cursors = Writer::default();
    for fixup in &fixups.cursors {
        cursors.u8(grid_number(fixup.grid));
        cursors.u8(u8::from(fixup.saved));
        cursors.cursor(&fixup.cursor);
    }
    let rep_len = match fixups.preceding_char {
        Some(Some(_)) => 5,
        _ => 1,
    };
    let len = HEADER_BYTES
        + painted.stream_len
        + 4
        + painted.cell_fixup_bytes
        + 4
        + cursors.buf.len()
        + rep_len;
    if measured {
        return Encoded {
            blob: Vec::new(),
            len,
            history_costs: painted.history_costs,
            stream_len: painted.stream_len,
            cell_fixup_bytes: painted.cell_fixup_bytes,
        };
    }

    // Filled to exactly its length, so the buffer never grows: with the
    // stream and the fixups it is copied from, a snapshot holds about twice
    // the blob while it is taken, and no more.
    let mut w = Writer::default();
    w.buf.reserve_exact(len);
    w.buf.extend_from_slice(&MAGIC);
    w.u16(VERSION);
    w.u8(flags);
    // The grid's size came from a `Size`, so it fits in `u16`.
    w.u16(term.columns() as u16);
    w.u16(term.screen_lines() as u16);
    w.len(history);
    w.u32(trimmed_for);
    let widths = WidthTable::current();
    let (major, minor, update) = widths.unicode_version;
    w.u8(major);
    w.u8(minor);
    w.u8(update);
    w.u64(widths.fingerprint);
    w.len(painted.stream_len);
    w.buf.extend_from_slice(&painted.stream);
    drop(painted.stream);
    w.len(painted.cell_fixup_count);
    w.buf.extend_from_slice(&fixups.cells.buf);
    drop(fixups.cells);
    w.len(fixups.cursors.len());
    w.buf.extend_from_slice(&cursors.buf);
    match fixups.preceding_char {
        None => w.u8(0),
        Some(None) => w.u8(1),
        Some(Some(c)) => {
            w.u8(2);
            w.char(c);
        }
    }
    debug_assert_eq!(w.buf.len(), len);
    Encoded {
        blob: w.buf,
        len,
        history_costs: painted.history_costs,
        stream_len: painted.stream_len,
        cell_fixup_bytes: painted.cell_fixup_bytes,
    }
}

fn grid_number(grid: GridSel) -> u8 {
    match grid {
        GridSel::Active => 0,
        GridSel::Inactive => 1,
    }
}

/// Restore an emulator from the part of a version 1 blob after its version,
/// keeping up to `scrollback_lines` lines of history.
pub(super) fn decode(body: &[u8], scrollback_lines: usize) -> Result<Restored, SnapshotError> {
    let mut r = Reader::new(body);
    let flags = r.u8()?;
    if flags & !(FLAG_HISTORY_TRIMMED | FLAG_SCREENS_DROPPED) != 0 {
        return Err(malformed(format!("unknown flags {flags:#x}")));
    }
    let columns = r.u16()?;
    let lines = r.u16()?;
    if !(1..=V1_MAX_SIDE).contains(&columns) || !(1..=V1_MAX_SIDE).contains(&lines) {
        return Err(malformed(format!("a terminal of {columns}x{lines}")));
    }
    let history = r.len()?;
    let trimmed_for = r.u32()?;
    if (flags & FLAG_HISTORY_TRIMMED != 0) != (trimmed_for > 0) {
        return Err(malformed("the trimmed flag and its count disagree"));
    }
    let widths = WidthTable {
        unicode_version: (r.u8()?, r.u8()?, r.u8()?),
        fingerprint: r.u64()?,
    };

    // Nothing is allocated before this. The screens must fit this build's
    // bound; history is cut to what fits it, like a smaller scrollback.
    let screens = 2 * u64::from(columns) * u64::from(lines);
    if screens > MAX_RESTORE_CELLS {
        return Err(SnapshotError::TooLarge { cells: screens });
    }
    let wanted_history = history.min(scrollback_lines);
    let kept_history = wanted_history.min(restorable_history(columns, lines));
    let stream = r.bytes()?;
    replay::check(stream, columns, lines, history)?;

    let mut emulator = Emulator::new(Size { columns, lines }, kept_history);
    // The stream answers nothing and tells the user nothing; the title is
    // read from the emulator afterwards. It is fed in pieces that end where
    // the cursor template's extra may have changed, so that each new extra
    // can be shared with an earlier one of the same content before cells are
    // printed from it, and at least every `replay::FEED_PIECE` bytes, so that
    // the effects a piece queues (and drops) stay small.
    let mut extras = Extras::new(emulator.term.extra_storage().clone());
    let now = Instant::now();
    let mut start = 0;
    for (end, changes) in replay::feed_points(stream) {
        emulator.feed(
            &stream[start..end],
            now,
            FeedMode::ReplaySeen,
            &mut Collected::default(),
        );
        if changes {
            extras.share_template(&mut emulator.term.grid_mut().cursor.template);
        }
        start = end;
    }
    // Extras of the cells the stream has since overwritten or dropped (history
    // beyond what is kept) go before the fixups make theirs.
    extras.sweep();
    if !emulator.is_clean() {
        return Err(malformed("the stream ends inside a sequence or update"));
    }
    if emulator.primary_grid().history_size() != kept_history {
        return Err(malformed(format!(
            "the stream rebuilds {} history lines, not {kept_history}",
            emulator.primary_grid().history_size()
        )));
    }

    for _ in 0..r.count(MIN_CELL_FIXUP_BYTES)? {
        let grid = read_grid(&mut r)?;
        let line = r.i32()?;
        let column = r.len()?;
        let cell = r.cell(&mut extras)?;
        let target = select(&mut emulator.term, grid);
        if column >= target.columns() || line >= target.screen_lines() as i32 {
            return Err(malformed(format!(
                "a cell fixup at {line},{column}, off the screen"
            )));
        }
        if line >= -(target.history_size() as i32) {
            target[Line(line)][Column(column)] = cell;
        }
    }
    for _ in 0..r.count(MIN_CURSOR_FIXUP_BYTES)? {
        let grid = read_grid(&mut r)?;
        let saved = r.bool()?;
        let cursor = r.cursor(&mut extras)?;
        let target = select(&mut emulator.term, grid);
        let point = cursor.point;
        if point.line.0 < 0
            || point.line.0 >= target.screen_lines() as i32
            || point.column.0 >= target.columns()
        {
            return Err(malformed(format!(
                "a cursor fixup at {point:?}, off the screen"
            )));
        }
        if saved {
            target.saved_cursor = cursor;
        } else {
            target.cursor = cursor;
        }
    }
    match r.u8()? {
        0 => {}
        1 => emulator.parser.set_preceding_char(None),
        2 => {
            // vte executes every C0 control, so none is ever the character
            // REP repeats; DEL and a C1 control can be (and are why this tag
            // exists for them).
            let c = r.char()?;
            if (c as u32) < 0x20 {
                return Err(malformed(format!("the REP character {c:?}")));
            }
            emulator.parser.set_preceding_char(Some(c));
        }
        other => return Err(malformed(format!("REP character tag {other}"))),
    }
    if !r.is_empty() {
        return Err(malformed("bytes after the last field"));
    }

    emulator.set_scrollback(scrollback_lines);
    // The lost lines would have left this emulator's history after as many
    // new lines as it holds, if that is fewer than the blob says. History this
    // build could not allocate is lost the same way.
    let scrollback = scrollback_lines.max(1) as u64;
    let remaining = if kept_history < wanted_history {
        scrollback
    } else {
        u64::from(trimmed_for).min(scrollback)
    };
    emulator.mark_trimmed(remaining);
    Ok(Restored {
        emulator,
        screens_dropped: flags & FLAG_SCREENS_DROPPED != 0,
        width_table_changed: (widths.fingerprint != WidthTable::current().fingerprint)
            .then_some(widths),
    })
}

fn read_grid(r: &mut Reader<'_>) -> Result<GridSel, SnapshotError> {
    match r.u8()? {
        0 => Ok(GridSel::Active),
        1 => Ok(GridSel::Inactive),
        other => Err(malformed(format!("grid {other}"))),
    }
}

fn select(term: &mut Term<Listener>, grid: GridSel) -> &mut Grid<Cell> {
    match grid {
        GridSel::Active => term.grid_mut(),
        GridSel::Inactive => term.inactive_grid_mut(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The streams the encoder writes where painting costs the most work per
    /// byte stay within half of version 1's work limit: every history row
    /// wrapped (a scroll, a wrap and an erase per row) on one column and on a
    /// wide terminal, rows ending in leading spacers, and rows of tab cells.
    ///
    /// Why: the limit is frozen with version 1, and a stream of the encoder's
    /// that it refused would make the terminal unavailable after a restart.
    /// Restoring alone (`tests/snapshot.rs`) would only notice once the margin
    /// is gone; this notices it shrinking. Spec: `replay`'s module docs.
    #[test]
    fn the_costliest_streams_keep_half_the_work_limit_spare() {
        let cases: Vec<(u16, u16, String)> = vec![
            (1, 4, "x".repeat(3000)),
            (500, 3, "x".repeat(500 * 2000)),
            (9, 3, "abcdefgh中".repeat(2000)),
            (200, 60, "\t|".repeat(100 * 2000)),
        ];
        for (columns, lines, output) in cases {
            let mut emulator = Emulator::new(Size { columns, lines }, 2000);
            emulator.feed(
                output.as_bytes(),
                Instant::now(),
                FeedMode::ReplaySeen,
                &mut Collected::default(),
            );
            assert!(emulator.primary_grid().history_size() >= 1000);
            let blob = emulator.snapshot(16 << 20).unwrap().blob;
            let history = u32::from_be_bytes(blob[11..15].try_into().unwrap()) as usize;
            let len = u32::from_be_bytes(blob[30..34].try_into().unwrap()) as usize;
            let work = replay::check(&blob[34..34 + len], columns, lines, history).unwrap();
            assert!(
                work.counted * 2 <= work.limit,
                "{columns}x{lines}: {} of {}",
                work.counted,
                work.limit
            );
        }
    }
}

//! The snapshot API's contract beyond the round trip: when a snapshot can be
//! taken, how blobs that cannot be restored are refused, the size limit, the
//! character width table, and the Restart seeds.
//!
//! The round trip itself is `snapshot_roundtrip.rs`; the frozen bytes of
//! version 1 are `snapshot_golden.rs`. The layout tests here build version 1
//! blobs by hand from the layout table in `src/snapshot/v1/mod.rs`, so they
//! check the decoder against the documented layout rather than against the
//! encoder.

mod common;

use common::compare::{cell_eq, diff};
use common::{emulator, feed};
use farhelm_terminal_emulator::alacritty_terminal::grid::Dimensions;
use farhelm_terminal_emulator::alacritty_terminal::index::{Column, Line};
use farhelm_terminal_emulator::alacritty_terminal::term::TermMode;
use farhelm_terminal_emulator::alacritty_terminal::term::cell::Flags;
use farhelm_terminal_emulator::alacritty_terminal::vte::ansi::{Color, NamedColor};
use farhelm_terminal_emulator::{
    Emulator, FORMAT_VERSION, MAX_RESTORE_CELLS, MIN_SNAPSHOT_LIMIT, MouseEncoding, NotClean, Size,
    SnapshotError, WidthTable, history_seed_from_lines,
};

/// Version 1's limits on a fixup cell: the longest hyperlink URI and `id`,
/// in bytes, and the most combining characters. Part of the format, so
/// spelled out here rather than taken from the emulator's caps.
const V1_MAX_URI: usize = 2083;
const V1_MAX_ID: usize = 250;
const V1_MAX_MARKS: usize = 16;

/// The host's `MAX_SNAPSHOT`.
const MAX: usize = 16 * 1024 * 1024;

/// Restore `blob` with the test helpers' scrollback, panicking on refusal.
fn restore(blob: &[u8]) -> farhelm_terminal_emulator::Restored {
    Emulator::restore(blob, 1000).expect("the blob restores")
}

// ---------------------------------------------------------------------------
// When a snapshot can be taken.

/// A snapshot is refused inside an escape sequence, a UTF-8 character or a
/// synchronized update, and taken anywhere clean.
///
/// Why: a snapshot taken there would make the restored emulator misread the
/// rest of the sequence, or lose the update's buffered bytes; the caller
/// must keep its previous snapshot instead. Spec: `Emulator::snapshot`,
/// "When a snapshot can be taken".
#[test]
fn snapshots_are_refused_away_from_clean_boundaries() {
    let cases: &[(&str, &[u8])] = &[
        ("mid escape sequence", b"text\x1b[3"),
        ("mid UTF-8 character", b"\xe4\xb8"),
        ("mid OSC", b"\x1b]2;title"),
        ("inside an update", b"\x1b[?2026hframe"),
    ];
    for (name, output) in cases {
        let mut emulator = emulator(20, 4);
        feed(&mut emulator, output);
        assert_eq!(emulator.snapshot(MAX).map(|_| ()), Err(NotClean), "{name}");
    }
    let mut emulator = emulator(20, 4);
    feed(&mut emulator, b"\x1b[?2026hframe\x1b[?2026l");
    assert!(emulator.snapshot(MAX).is_ok());
}

/// Restoring delivers nothing: no answer to the program and no notice to
/// the user, for the sequences that rebuild the state, and nothing left
/// queued for the first live feed after it.
///
/// Why: the stream sets the title, colors and modes the program set, and
/// re-announcing them (or ringing anything) after a supervisor restart would
/// be the replayed side effect the goal forbids. Spec: `Emulator::restore`.
#[test]
fn restoring_has_no_side_effects() {
    let mut original = emulator(20, 4);
    feed(
        &mut original,
        b"\x1b]2;title\x07\x1b[22t\x1b]4;1;#102030\x07\x07hi",
    );
    let restored = restore(&original.snapshot(MAX).unwrap().blob);
    let mut restored = restored.emulator;
    assert_eq!(restored.title(), "title");
    let effects = feed(&mut restored, b"x");
    assert_eq!(effects, farhelm_terminal_emulator::Collected::default());
}

// ---------------------------------------------------------------------------
// Refusals: what cannot be restored is an error, never a guess or a panic.

/// A blob without the magic bytes, or too short for them and the version,
/// is not a snapshot; one of a version this build does not know is refused
/// as such; and the encoder writes version 1.
///
/// Why: the supervisor shows such a terminal as unavailable, and must be
/// able to tell a newer supervisor's blob (after a downgrade) from garbage.
/// Spec: `SnapshotError`.
#[test]
fn foreign_blobs_and_unknown_versions_are_refused() {
    assert_eq!(FORMAT_VERSION, 1);
    for blob in [&b""[..], b"FHT", b"FHTS\x00", b"XXXX\x00\x01\x00"] {
        assert_eq!(
            Emulator::restore(blob, 10).err(),
            Some(SnapshotError::NotASnapshot),
            "{blob:?}"
        );
    }
    for version in [0u16, 2, 0xFFFF] {
        let mut blob = b"FHTS".to_vec();
        blob.extend_from_slice(&version.to_be_bytes());
        blob.extend_from_slice(&[0; 40]);
        assert_eq!(
            Emulator::restore(&blob, 10).err(),
            Some(SnapshotError::UnknownVersion(version))
        );
    }
}

/// Every prefix of a real blob, and the blob with a byte appended, is
/// refused, without panicking.
///
/// Why: a blob that a crash or a bug cut short must make the terminal
/// unavailable, not restore part of a state or take the supervisor down.
/// Spec: `v1` module, "Layout" (nothing may follow the last field).
#[test]
fn truncated_and_extended_blobs_are_refused() {
    let mut source = emulator(12, 3);
    // A wide character's half left alone by an erase needs a cell fixup, so
    // the blob's fixup sections are not all empty.
    feed(&mut source, "ab中cd\x1b[3D\x1b[X\x1b[2;12Hx".as_bytes());
    common::resize(&mut source, 8, 3);
    let blob = source.snapshot(MAX).unwrap().blob;
    for len in 0..blob.len() {
        assert!(
            Emulator::restore(&blob[..len], 10).is_err(),
            "prefix of {len} bytes"
        );
    }
    let mut extended = blob.clone();
    extended.push(0);
    assert!(matches!(
        Emulator::restore(&extended, 10),
        Err(SnapshotError::Malformed(_))
    ));
    assert!(Emulator::restore(&blob, 10).is_ok());
}

/// Builds version 1 blobs by hand, following the layout table in
/// `src/snapshot/v1/mod.rs`.
struct V1 {
    blob: Vec<u8>,
}

impl V1 {
    /// The header of a terminal of `columns` x `lines` with `flags`, whose
    /// `stream` rebuilds no history, ready for the fixup sections.
    fn new(flags: u8, columns: u16, lines: u16, stream: &[u8]) -> V1 {
        let trimmed_for = u32::from(flags & 1);
        V1::header(flags, columns, lines, 0, trimmed_for, stream)
    }

    /// The header with every field given. The width table names Unicode
    /// 16.0.0 and a fingerprint of 0, which no real table has.
    fn header(
        flags: u8,
        columns: u16,
        lines: u16,
        history: u32,
        trimmed_for: u32,
        stream: &[u8],
    ) -> V1 {
        let mut blob = b"FHTS\x00\x01".to_vec();
        blob.push(flags);
        blob.extend_from_slice(&columns.to_be_bytes());
        blob.extend_from_slice(&lines.to_be_bytes());
        blob.extend_from_slice(&history.to_be_bytes());
        blob.extend_from_slice(&trimmed_for.to_be_bytes());
        blob.extend_from_slice(&[16, 0, 0]);
        blob.extend_from_slice(&0u64.to_be_bytes());
        blob.extend_from_slice(&(stream.len() as u32).to_be_bytes());
        blob.extend_from_slice(stream);
        V1 { blob }
    }

    fn u8(mut self, value: u8) -> V1 {
        self.blob.push(value);
        self
    }

    fn u16(mut self, value: u16) -> V1 {
        self.blob.extend_from_slice(&value.to_be_bytes());
        self
    }

    fn u32(mut self, value: u32) -> V1 {
        self.blob.extend_from_slice(&value.to_be_bytes());
        self
    }

    fn i32(mut self, value: i32) -> V1 {
        self.blob.extend_from_slice(&value.to_be_bytes());
        self
    }

    fn bytes(self, bytes: &[u8]) -> V1 {
        let mut this = self.u32(bytes.len() as u32);
        this.blob.extend_from_slice(bytes);
        this
    }

    /// A cell of `c`: foreground named slot 1 (red), background indexed 42,
    /// flags bold (bit 1), one zero-width character, underline color
    /// RGB 1,2,3, and a hyperlink.
    fn cell(self, c: char) -> V1 {
        self.u32(c as u32)
            .u8(0)
            .u16(1)
            .u8(1)
            .u8(42)
            .u16(1 << 1)
            .u8(1)
            .u32('\u{301}' as u32)
            .u8(1)
            .u8(2)
            .u8(1)
            .u8(2)
            .u8(3)
            .u8(1)
            .bytes(b"link-id")
            .bytes(b"http://example.test/")
    }

    /// A plain cell with a hyperlink of `id` and `uri`.
    fn plain_cell_with_link(self, id: &[u8], uri: &[u8]) -> V1 {
        self.u32(' ' as u32)
            .u8(0)
            .u16(256)
            .u8(0)
            .u16(257)
            .u16(0)
            .u8(0)
            .u8(0)
            .u8(1)
            .bytes(id)
            .bytes(uri)
    }

    /// A plain cell: default colors (named 256 and 257), nothing else.
    fn plain_cell(self, c: char) -> V1 {
        self.u32(c as u32)
            .u8(0)
            .u16(256)
            .u8(0)
            .u16(257)
            .u16(0)
            .u8(0)
            .u8(0)
            .u8(0)
    }
}

/// A hand-built version 1 blob restores to exactly what its fields say:
/// the stream's state, then a cell fixup (one in history beyond the restoring
/// scrollback skipped), cursor fixups for the cursor and the saved cursor,
/// the REP character, and the trimmed-history and dropped-screens flags.
///
/// Why: this pins the decoder to the documented layout independently of the
/// encoder, so that a decoder change that the encoder happens to mirror
/// still fails here. Spec: the `v1` module's layout table and "Reading".
#[test]
fn a_hand_built_v1_blob_restores_as_its_layout_says() {
    let blob = V1::header(
        0b11,
        10,
        3,
        1,
        5,
        b"one\n\x1b[Gtwo\n\x1b[Gthree\n\x1b[Gfour",
    )
    .u32(2)
    // A cell fixup on the showing screen, line 1, column 4.
    .u8(0)
    .i32(1)
    .u32(4)
    .cell('Z')
    // One in history line -50, beyond what is kept: skipped.
    .u8(0)
    .i32(-50)
    .u32(0)
    .plain_cell('Q')
    .u32(2)
    // The cursor: line 2, column 9, wrap pending, plain template, G0
    // line drawing.
    .u8(0)
    .u8(0)
    .i32(2)
    .u32(9)
    .u8(1)
    .plain_cell(' ')
    .u8(1)
    .u8(0)
    .u8(0)
    .u8(0)
    // The saved cursor: line 1, column 1.
    .u8(0)
    .u8(1)
    .i32(1)
    .u32(1)
    .u8(0)
    .plain_cell(' ')
    .u8(0)
    .u8(0)
    .u8(0)
    .u8(0)
    // REP character 'R'.
    .u8(2)
    .u32('R' as u32)
    .blob;

    let restored = Emulator::restore(&blob, 1).expect("the blob restores");
    assert!(restored.screens_dropped);
    assert_eq!(
        restored.width_table_changed,
        Some(WidthTable {
            unicode_version: (16, 0, 0),
            fingerprint: 0
        })
    );
    let emulator = restored.emulator;
    assert!(emulator.history_trimmed());
    let grid = emulator.term().grid();
    assert_eq!(grid.history_size(), 1, "the line the stream scrolled off");
    assert_eq!(emulator.screen_text(), "two\nthreZ\u{301}\nfour\n");
    let cell = &grid[Line(1)][Column(4)];
    assert_eq!(cell.fg, Color::Named(NamedColor::Red));
    assert_eq!(cell.bg, Color::Indexed(42));
    assert_eq!(cell.flags, Flags::BOLD);
    assert_eq!(cell.zerowidth(), Some(&['\u{301}'][..]));
    let link = cell.hyperlink().expect("the fixup's link");
    assert_eq!((link.id(), link.uri()), ("link-id", "http://example.test/"));
    assert_eq!(grid.cursor.point.line, Line(2));
    assert_eq!(grid.cursor.point.column, Column(9));
    assert!(grid.cursor.input_needs_wrap);
    assert_eq!(
        grid.cursor.charsets[farhelm_terminal_emulator::alacritty_terminal::vte::ansi::CharsetIndex::G0],
        farhelm_terminal_emulator::alacritty_terminal::vte::ansi::StandardCharset::SpecialCharacterAndLineDrawing
    );
    assert_eq!(grid.saved_cursor.point.line, Line(1));
    assert_eq!(emulator.preceding_char(), Some('R'));
    assert!(emulator.is_clean());
}

/// Each way a version 1 blob can break its layout is refused as malformed.
///
/// Why: the decoder must never guess at a value outside the format (a
/// named color it does not know, an unknown flag) or write outside the
/// grid; every such blob means the terminal is unavailable. Spec: the `v1`
/// module.
#[test]
fn malformed_v1_blobs_are_refused() {
    let base = |stream: &[u8]| V1::new(0, 10, 3, stream);
    // A whole record, so that only the count can be what is refused.
    let mut marks = base(b"")
        .u32(1)
        .u8(0)
        .i32(0)
        .u32(0)
        .u32('e' as u32)
        .u8(0)
        .u16(256)
        .u8(0)
        .u16(257)
        .u16(0)
        .u8(V1_MAX_MARKS as u8 + 1);
    for _ in 0..=V1_MAX_MARKS {
        marks = marks.u32(0x301);
    }
    let seventeen_marks = marks.u8(0).u8(0).u32(0).u8(0).blob;
    let cases: Vec<(&str, Vec<u8>)> = vec![
        (
            "unknown flag",
            V1::new(0b100, 10, 3, b"").u32(0).u32(0).u8(0).blob,
        ),
        ("no columns", V1::new(0, 0, 3, b"").u32(0).u32(0).u8(0).blob),
        (
            "columns past the host's limit",
            V1::new(0, 10_001, 3, b"").u32(0).u32(0).u8(0).blob,
        ),
        (
            "trimmed without a count",
            V1::header(1, 10, 3, 0, 0, b"").u32(0).u32(0).u8(0).blob,
        ),
        (
            "a count without the trimmed flag",
            V1::header(0, 10, 3, 0, 9, b"").u32(0).u32(0).u8(0).blob,
        ),
        (
            "less history than the header says",
            V1::header(0, 10, 3, 5, 0, b"x").u32(0).u32(0).u8(0).blob,
        ),
        (
            "stream mid-sequence",
            base(b"\x1b[3").u32(0).u32(0).u8(0).blob,
        ),
        (
            "stream in an update",
            base(b"\x1b[?2026h").u32(0).u32(0).u8(0).blob,
        ),
        (
            "unknown grid",
            base(b"")
                .u32(1)
                .u8(2)
                .i32(0)
                .u32(0)
                .plain_cell('x')
                .u32(0)
                .u8(0)
                .blob,
        ),
        (
            "fixup off the right",
            base(b"")
                .u32(1)
                .u8(0)
                .i32(0)
                .u32(10)
                .plain_cell('x')
                .u32(0)
                .u8(0)
                .blob,
        ),
        (
            "fixup below the screen",
            base(b"")
                .u32(1)
                .u8(0)
                .i32(3)
                .u32(0)
                .plain_cell('x')
                .u32(0)
                .u8(0)
                .blob,
        ),
        (
            "unknown named color",
            base(b"")
                .u32(1)
                .u8(0)
                .i32(0)
                .u32(0)
                .u32('x' as u32)
                .u8(0)
                .u16(999)
                .blob,
        ),
        (
            "unknown color kind",
            base(b"")
                .u32(1)
                .u8(0)
                .i32(0)
                .u32(0)
                .u32('x' as u32)
                .u8(3)
                .blob,
        ),
        (
            "not a character",
            base(b"").u32(1).u8(0).i32(0).u32(0).u32(0xD800).blob,
        ),
        (
            "unknown cell flag",
            base(b"")
                .u32(1)
                .u8(0)
                .i32(0)
                .u32(0)
                .u32('x' as u32)
                .u8(1)
                .u8(0)
                .u8(1)
                .u8(0)
                .u16(1 << 15)
                .blob,
        ),
        ("count beyond the blob", base(b"").u32(1_000_000).blob),
        (
            "cursor off the screen",
            base(b"")
                .u32(0)
                .u32(1)
                .u8(0)
                .u8(0)
                .i32(5)
                .u32(0)
                .u8(0)
                .plain_cell(' ')
                .u32(0)
                .u8(0)
                .blob,
        ),
        ("boolean of 2", base(b"").u32(0).u32(1).u8(0).u8(2).blob),
        (
            "a wrap flag of 2",
            base(b"")
                .u32(0)
                .u32(1)
                .u8(0)
                .u8(0)
                .i32(0)
                .u32(9)
                .u8(2)
                .plain_cell(' ')
                .u8(0)
                .u8(0)
                .u8(0)
                .u8(0)
                .u8(0)
                .blob,
        ),
        ("unknown REP tag", base(b"").u32(0).u32(0).u8(3).blob),
        (
            "an escape as the REP character",
            base(b"").u32(0).u32(0).u8(2).u32(0x1b).blob,
        ),
        (
            "a carriage return as the REP character",
            base(b"").u32(0).u32(0).u8(2).u32(0x0d).blob,
        ),
        (
            "NUL as the REP character",
            base(b"").u32(0).u32(0).u8(2).u32(0).blob,
        ),
        (
            "seventeen combining characters (version 1 carries 16)",
            seventeen_marks,
        ),
        (
            "a cursor template with a combining character",
            base(b"")
                .u32(0)
                .u32(1)
                .u8(0)
                .u8(0)
                .i32(0)
                .u32(0)
                .u8(0)
                .u32(' ' as u32)
                .u8(0)
                .u16(256)
                .u8(0)
                .u16(257)
                .u16(0)
                .u8(1)
                .u32(0x301)
                .u8(0)
                .u8(0)
                .u8(0)
                .u8(0)
                .u8(0)
                .u8(0)
                .u8(0)
                .blob,
        ),
    ];
    // Links no OSC 8 can carry, on a cell fixup and on a cursor template
    // (where the review smuggled a sequence into the next snapshot's
    // stream through one).
    // Version 1's own limits (2083 and 250 bytes), written out rather than
    // taken from the emulator's caps, which may change.
    let uri_over = format!("http://e/{}", "u".repeat(V1_MAX_URI - 8));
    let id_over = "i".repeat(V1_MAX_ID + 1);
    let links: Vec<(&str, &[u8], &[u8])> = vec![
        (
            "a control character in the URI",
            b"x",
            b"http://a/\x07\x1b[?2026h",
        ),
        ("an empty URI", b"x", b""),
        ("a colon in the id", b"a:b", b"http://a/"),
        ("a semicolon in the id", b"a;b", b"http://a/"),
        ("a URI over the cap", b"x", uri_over.as_bytes()),
        ("an id over the cap", id_over.as_bytes(), b"http://a/"),
        (
            "a control character in the id",
            b"a\x1b\\\x1b[2J",
            b"http://a/",
        ),
        ("a bell in the id", b"x\x07", b"http://a/"),
        (
            "fourteen semicolons in the URI",
            b"x",
            b"http://a/;;;;;;;;;;;;;;",
        ),
    ];
    let mut cases = cases;
    for (name, id, uri) in links {
        let linked = |v1: V1| {
            v1.u32(' ' as u32)
                .u8(0)
                .u16(256)
                .u8(0)
                .u16(257)
                .u16(0)
                .u8(0)
                .u8(0)
                .u8(1)
                .bytes(id)
                .bytes(uri)
        };
        let cell = linked(base(b"").u32(1).u8(0).i32(0).u32(0));
        cases.push((name, cell.u32(0).u8(0).blob));
        let template = linked(base(b"").u32(0).u32(1).u8(0).u8(0).i32(0).u32(0).u8(0));
        cases.push((name, template.u8(0).u8(0).u8(0).u8(0).u8(0).blob));
    }
    for (name, blob) in cases {
        assert!(
            matches!(
                Emulator::restore(&blob, 10),
                Err(SnapshotError::Malformed(_))
            ),
            "{name}"
        );
    }
}

// ---------------------------------------------------------------------------
// The size limit.

/// An emulator with `lines` lines of colorful history in a 40x5 terminal.
fn colorful_history(lines: usize) -> Emulator {
    let mut emulator = Emulator::new(
        Size {
            columns: 40,
            lines: 5,
        },
        10_000,
    );
    for n in 0..lines {
        let line = format!(
            "\x1b[38;5;{}m\x1b[48;2;{n};2;3mline {n:05} of history\r\n",
            n % 256
        );
        feed(&mut emulator, line.as_bytes());
    }
    emulator
}

/// A snapshot that would exceed the limit drops its oldest history lines,
/// as few as fit, and keeps everything else; the restored emulator says
/// its history was trimmed, and keeps saying so through further snapshots,
/// until the program clears its history.
///
/// Why: the host refuses a blob over `MAX_SNAPSHOT`, and a snapshot must
/// never fail for size, while the user should be told that history is
/// missing rather than find it silently shorter. Spec: `snapshot` module
/// docs, "Size", and `Emulator::history_trimmed`.
#[test]
fn oversized_snapshots_drop_the_oldest_history() {
    let original = colorful_history(3000);
    let full = original.snapshot(MAX).unwrap();
    assert_eq!(full.dropped_history_lines, 0);
    let limit = full.blob.len() / 3;
    let snapshot = original.snapshot(limit).unwrap();
    assert!(snapshot.blob.len() <= limit);
    assert!(snapshot.dropped_history_lines > 1000);
    assert!(!snapshot.dropped_screens);
    // As few lines as fit: a line takes about 60 bytes here, so the blob is
    // within a few lines of the limit.
    assert!(
        snapshot.blob.len() + 1000 > limit,
        "{} of {limit} bytes",
        snapshot.blob.len()
    );

    let restored = restore(&snapshot.blob).emulator;
    assert!(restored.history_trimmed());
    assert!(!original.history_trimmed());
    let (a, b) = (original.term().grid(), restored.term().grid());
    assert_eq!(
        b.history_size(),
        a.history_size() - snapshot.dropped_history_lines
    );
    for line in -(b.history_size() as i32)..b.screen_lines() as i32 {
        for column in 0..b.columns() {
            let (x, y) = (
                &a[Line(line)][Column(column)],
                &b[Line(line)][Column(column)],
            );
            assert!(cell_eq(x, y), "line {line} column {column}");
        }
    }

    // Sticky across a snapshot that needs no trimming.
    let again = restore(&restored.snapshot(MAX).unwrap().blob).emulator;
    assert!(again.history_trimmed());
    let mut again = again;
    feed(&mut again, b"\x1b[3J");
    assert!(!again.history_trimmed(), "the program cleared its history");
}

/// The trimmed mark also ends once as many lines as the history holds have
/// entered it since the restore, which would have pushed out the lost lines
/// anyway.
///
/// Why: otherwise a terminal restored once from a trimmed snapshot would say
/// so for the rest of its life. Spec: `Emulator::history_trimmed`.
#[test]
fn the_trimmed_mark_ends_when_the_history_has_turned_over() {
    let original = colorful_history(300);
    let blob = original.snapshot(4096).unwrap().blob;
    let mut restored = Emulator::restore(&blob, 100).unwrap().emulator;
    assert!(restored.history_trimmed());
    feed(&mut restored, "\r\n".repeat(99).as_bytes());
    assert!(restored.history_trimmed());
    feed(&mut restored, b"\r\n");
    assert!(!restored.history_trimmed());
}

/// When the title stack is what makes a snapshot too large, it leaves out
/// the oldest titles and keeps the screen; when the screen alone is too
/// large, it leaves out both screens' contents and says so, keeping modes,
/// title and cursors. The result fits either way.
///
/// Why: "never fail for size" must hold even for a stack of maximal titles
/// or a huge terminal full of styled cells, losing the least useful state
/// first; after losing the screens the supervisor asks the program to
/// redraw. Spec: `snapshot` module docs, "Size", steps 2 and 3.
#[test]
fn the_last_resorts_still_fit() {
    let mut big = Emulator::new(
        Size {
            columns: 300,
            lines: 100,
        },
        100,
    );
    let mut screen = String::from("\x1b[?2004h\x1b]2;kept\x07");
    for n in 0..30_000 {
        screen.push_str(&format!("\x1b[38;2;{};{};1m#", n % 256, n / 256 % 256));
    }
    feed(&mut big, screen.as_bytes());
    let snapshot = big.snapshot(MIN_SNAPSHOT_LIMIT).unwrap();
    assert!(snapshot.dropped_screens);
    assert!(snapshot.blob.len() <= MIN_SNAPSHOT_LIMIT);
    let restored = restore(&snapshot.blob);
    assert!(restored.screens_dropped);
    let emulator = restored.emulator;
    assert_eq!(emulator.screen_text().trim(), "");
    assert_eq!(emulator.title(), "kept");
    assert!(emulator.modes().bracketed_paste);
    assert_eq!(
        emulator.term().grid().cursor.point,
        big.term().grid().cursor.point
    );

    let mut titled = emulator_with_title_stack();
    feed(&mut titled, b"the screen");
    let snapshot = titled.snapshot(MIN_SNAPSHOT_LIMIT).unwrap();
    assert!(snapshot.dropped_titles > 0);
    assert!(!snapshot.dropped_screens);
    assert!(snapshot.blob.len() <= MIN_SNAPSHOT_LIMIT);
    let restored = restore(&snapshot.blob).emulator;
    let stack = titled.term().title_stack();
    assert_eq!(
        restored.term().title_stack(),
        &stack[snapshot.dropped_titles..]
    );
    assert!(restored.screen_text().contains("the screen"));
}

/// An emulator with 200 titles of 4000 bytes on its title stack.
fn emulator_with_title_stack() -> Emulator {
    let mut emulator = emulator(20, 3);
    for n in 0..200 {
        let title = format!("\x1b]2;{n:04}{}\x07\x1b[22t", "t".repeat(3996));
        feed(&mut emulator, title.as_bytes());
    }
    emulator
}

// ---------------------------------------------------------------------------
// Character widths.

/// A blob painted with another character width table restores all the
/// same, and says which table it was; the table is told apart by its
/// fingerprint, not by the Unicode version it names.
///
/// Why: refusing it would make every running terminal unavailable after a
/// `unicode-width` upgrade; the supervisor instead asks the program to
/// redraw. And the crate has changed widths without changing the version it
/// names, so only a fingerprint of the widths notices. Spec: `snapshot`
/// module docs, "Character widths", and `WidthTable`.
#[test]
fn a_different_width_table_restores_and_is_reported() {
    let mut original = emulator(20, 4);
    feed(&mut original, "wide 中 and 😀 text\r\nmore".as_bytes());
    let mut blob = original.snapshot(MAX).unwrap().blob;
    assert!(restore(&blob).width_table_changed.is_none());
    // Magic (4), version (2), flags (1), size (4), history (4), trimmed
    // (4), then the Unicode version (3) and the fingerprint (8).
    let fingerprint = 22..30;
    let mut changed = u64::from_be_bytes(blob[fingerprint.clone()].try_into().unwrap());
    changed ^= 1;
    blob[fingerprint].copy_from_slice(&changed.to_be_bytes());
    let restored = restore(&blob);
    let current = WidthTable::current();
    assert_eq!(
        restored.width_table_changed,
        Some(WidthTable {
            unicode_version: current.unicode_version,
            fingerprint: changed,
        })
    );
    assert!(diff(&original, &restored.emulator).is_empty());
}

/// The width table's fingerprint is FNV-1a over the width of every
/// character, as documented, so that it means the same in every build.
///
/// Why: blobs carry it forever; a fingerprint computed differently by a later
/// build would report every old blob as painted with another table.
#[test]
fn the_width_fingerprint_is_as_documented() {
    let current = WidthTable::current();
    // Recomputed here independently: a byte per scalar value, 255 for none.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for code in 0..=0x10_FFFFu32 {
        let Some(c) = char::from_u32(code) else {
            continue;
        };
        let width = unicode_width::UnicodeWidthChar::width(c).map_or(255, |width| width as u8);
        hash = (hash ^ u64::from(width)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    assert_eq!(current.fingerprint, hash);
}

// ---------------------------------------------------------------------------
// Restart seeds.

/// The seed keeps the primary screen's history, cell for cell, above a
/// blank screen with everything else as in a fresh terminal: default modes
/// and colors, no title, cursor and saved cursor home, the primary screen
/// showing, no REP character. It works mid-sequence, and while the
/// alternate screen shows.
///
/// Why: Restart starts a new program in a new host that must keep the old
/// terminal's scrollback but not its last frame or the old program's modes
/// (a new shell must not inherit mouse reporting or bracketed paste).
/// Spec: `Emulator::history_seed`, and the goal's "Restart".
#[test]
fn the_history_seed_keeps_history_and_nothing_else() {
    let mut original = colorful_history(50);
    feed(
        &mut original,
        concat!(
            "\x1b]8;id=a;http://a.test/\x1b\\linked\x1b]8;;\x1b\\ and a line long enough to wrap around\r\n",
            "\x1b[?2004h\x1b[?1000h\x1b[?1h\x1b]2;old title\x07\x1b]11;#ffffff\x07\x1b[2;3r",
            "\x1b[?1049hfull-screen frame\x1b[3;5H\x1b[3",
        )
        .as_bytes(),
    );
    assert!(!original.is_clean());
    let seed = original.history_seed(MAX);
    assert_eq!(seed.dropped_history_lines, 0);
    let restored = restore(&seed.blob);
    let emulator = restored.emulator;

    let fresh = Emulator::new(original.size(), 1000);
    let (term, fresh_term) = (emulator.term(), fresh.term());
    assert_eq!(term.mode(), fresh_term.mode());
    assert_eq!(term.title(), None);
    assert_eq!(term.scroll_region(), fresh_term.scroll_region());
    assert_eq!(term.colors()[NamedColor::Background as usize], None);
    assert_eq!(emulator.screen_text().trim(), "");
    let grid = term.grid();
    assert_eq!(grid.cursor.point, fresh_term.grid().cursor.point);
    assert_eq!(
        grid.saved_cursor.point,
        fresh_term.grid().saved_cursor.point
    );
    assert!(!grid.saved_cursor.input_needs_wrap);
    assert_eq!(emulator.preceding_char(), None);
    assert!(emulator.is_clean());

    // The history is the old primary screen's, line for line; its visible
    // frame is gone.
    let old = original.term().inactive_grid();
    assert!(!term.mode().contains(TermMode::ALT_SCREEN));
    assert_eq!(grid.history_size(), old.history_size());
    for line in 0..old.history_size() as i32 {
        let line = -1 - line;
        for column in 0..old.columns() {
            let (a, b) = (
                &old[Line(line)][Column(column)],
                &grid[Line(line)][Column(column)],
            );
            assert!(cell_eq(a, b), "line {line} column {column}");
        }
    }
}

/// A seed from plain history lines with SGR styles, as tmux's capture
/// prints them, holds those lines in its history, styled and wrapped to the
/// new width, above a blank screen.
///
/// Why: restarting an old (tmux) terminal into a terminal host keeps the
/// tmux pane's history this way. Spec: `history_seed_from_lines`.
#[test]
fn a_seed_from_styled_lines_holds_them_in_history() {
    let lines: Vec<&[u8]> = vec![
        b"plain line",
        b"\x1b[31mred\x1b[0m and \x1b[1mbold",
        b"a line much longer than the twenty columns of the new terminal",
        b"",
        b"last",
    ];
    let seed = history_seed_from_lines(
        Size {
            columns: 20,
            lines: 4,
        },
        1000,
        lines,
        MAX,
    );
    let emulator = restore(&seed.blob).emulator;
    assert_eq!(emulator.screen_text().trim(), "");
    let grid = emulator.term().grid();
    let text: Vec<String> = (-(grid.history_size() as i32)..0)
        .map(|line| {
            let row = &grid[Line(line)];
            (0..grid.columns())
                .map(|column| row[Column(column)].c)
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect();
    assert_eq!(
        text,
        [
            "plain line",
            "red and bold",
            "a line much longer t",
            "han the twenty colum",
            "ns of the new termin",
            "al",
            "",
            "last",
        ]
    );
    let history = grid.history_size() as i32;
    assert_eq!(
        grid[Line(1 - history)][Column(0)].fg,
        Color::Named(NamedColor::Red)
    );
    assert_eq!(
        grid[Line(1 - history)][Column(3)].fg,
        Color::Named(NamedColor::Foreground)
    );
    assert!(
        grid[Line(1 - history)][Column(8)]
            .flags
            .contains(Flags::BOLD)
    );
    assert!(
        !grid[Line(2 - history)][Column(0)]
            .flags
            .contains(Flags::BOLD),
        "styles end with their line"
    );
    assert_eq!(
        emulator.term().mode(),
        Emulator::new(
            Size {
                columns: 20,
                lines: 4
            },
            10
        )
        .term()
        .mode()
    );
}

// ---------------------------------------------------------------------------
// Bounded decoding.

/// A blob's size fields are checked before anything is allocated: sides
/// past the host's 10,000 are malformed, and two screens over
/// `MAX_RESTORE_CELLS` cells are refused as too large. The 27-byte blob a
/// review built (65,535 by 65,535, in the layout before this one) is among
/// them.
///
/// Why: such a blob made the restore allocate about 100 GB and abort the
/// shared supervisor, where the module promises a typed error. The check
/// must come before the emulator is made, so these blobs return at once.
#[test]
fn oversized_blobs_are_refused_before_allocating() {
    let mut review = b"FHTS\x00\x01\x00\xff\xff\xff\xff\x11\x00\x00".to_vec();
    review.extend_from_slice(&[0; 13]);
    assert_eq!(review.len(), 27);
    assert!(matches!(
        Emulator::restore(&review, 0),
        Err(SnapshotError::Malformed(_))
    ));

    let screens = |columns, lines| {
        V1::header(0, columns, lines, 0, 0, b"")
            .u32(0)
            .u32(0)
            .u8(0)
            .blob
    };
    assert_eq!(
        Emulator::restore(&screens(10_000, 10_000), 0).err(),
        Some(SnapshotError::TooLarge {
            cells: 2 * 10_000 * 10_000
        })
    );
    assert!(matches!(
        Emulator::restore(&screens(3_000, 3_000), 0),
        Err(SnapshotError::TooLarge { cells }) if cells > MAX_RESTORE_CELLS
    ));
}

/// History beyond what `MAX_RESTORE_CELLS` holds is left out of a restore
/// like history beyond a smaller scrollback, and the restored history is
/// marked trimmed; the bound counts the thousand rows alacritty allocates
/// ahead of a history as soon as it has one line.
///
/// Why: a review found an 880-byte blob of 10,000 by 837 with one history
/// line passing the old bound at 16.75 million cells and then allocating
/// 619 MiB, since that one line made alacritty allocate a thousand rows of
/// 10,000 cells. And refusing such blobs would have made the bound part of
/// version 1, so that no later build could lower it without stranding the
/// blobs hosts keep; dropping history keeps it this build's policy. Spec:
/// `MAX_RESTORE_CELLS`, the `v1` module's "Reading".
#[test]
fn history_past_the_restore_bound_is_dropped_and_marked_trimmed() {
    // The review's blob: the screens fit, and its one history line would
    // need a thousand rows more than fit, so it keeps none.
    let review = V1::header(0, 10_000, 837, 1, 0, "\n".repeat(837).as_bytes())
        .u32(0)
        .u32(0)
        .u8(0)
        .blob;
    let restored = Emulator::restore(&review, 12_000).expect("restores without its history");
    assert_eq!(restored.emulator.term().grid().history_size(), 0);
    assert!(restored.emulator.history_trimmed());

    // 1,000 columns: room for 16,000 lines of history at ten lines of
    // screen, in whole steps of a thousand.
    let tall = V1::header(0, 1_000, 10, 20_000, 0, "\n".repeat(20_009).as_bytes())
        .u32(0)
        .u32(0)
        .u8(0)
        .blob;
    let restored = Emulator::restore(&tall, 20_000).unwrap().emulator;
    assert_eq!(restored.term().grid().history_size(), 16_000);
    assert!(restored.history_trimmed());
    // A scrollback below the bound keeps what it holds, as before, and
    // nothing is lost to the bound.
    let restored = Emulator::restore(&tall, 100).unwrap().emulator;
    assert_eq!(restored.term().grid().history_size(), 100);
    assert!(!restored.history_trimmed());
}

/// A terminal wider than `MAX_RESTORE_CELLS` allows at its full history is
/// snapshotted with its oldest history dropped to fit (in whole steps of
/// alacritty's thousand-row allocation), so its blob restores whole.
///
/// Why: the encoder must not write blobs its own restore cuts short.
#[test]
fn the_encoder_keeps_within_the_restore_bound() {
    let mut wide = Emulator::new(
        Size {
            columns: 4000,
            lines: 2,
        },
        5000,
    );
    feed(&mut wide, "x\r\n".repeat(5000).as_bytes());
    assert_eq!(wide.term().grid().history_size(), 4999);
    let snapshot = wide.snapshot(MAX).unwrap();
    let kept = 4999 - snapshot.dropped_history_lines;
    assert!(snapshot.dropped_history_lines > 0);
    assert!(4000 * (2 * 2 + kept.div_ceil(1000) as u64 * 1000) <= MAX_RESTORE_CELLS);
    let restored = Emulator::restore(&snapshot.blob, 5000).unwrap();
    assert_eq!(restored.emulator.term().grid().history_size(), kept);
}

// ---------------------------------------------------------------------------
// Trimmed history across restores.

/// The trimmed mark ends after as many new lines as the history held when
/// it was trimmed, counted across any number of snapshots and restores in
/// between.
///
/// Why: version 1 carries how far the mark has to go; without that, every
/// restore started the count again, and a terminal restored often enough
/// said history was trimmed for good. Spec: `Emulator::history_trimmed`,
/// and the `v1` layout's "trimmed for".
#[test]
fn the_trimmed_mark_survives_restores_with_its_progress() {
    let mut original = Emulator::new(
        Size {
            columns: 20,
            lines: 3,
        },
        100,
    );
    for n in 0..110 {
        feed(&mut original, format!("line {n}\r\n").as_bytes());
    }
    let first = original.snapshot(256).unwrap();
    assert!(first.dropped_history_lines > 0);
    let mut emulator = Emulator::restore(&first.blob, 100).unwrap().emulator;
    feed(&mut emulator, "\r\n".repeat(60).as_bytes());
    assert!(emulator.history_trimmed());
    let blob = emulator.snapshot(MAX).unwrap().blob;
    let mut emulator = Emulator::restore(&blob, 100).unwrap().emulator;
    feed(&mut emulator, "\r\n".repeat(39).as_bytes());
    assert!(emulator.history_trimmed(), "99 lines since the trim");
    let blob = emulator.snapshot(MAX).unwrap().blob;
    let mut emulator = Emulator::restore(&blob, 100).unwrap().emulator;
    feed(&mut emulator, b"\r\n");
    assert!(!emulator.history_trimmed(), "100 lines since the trim");
}

/// A seed from more lines than the scrollback holds keeps the newest and
/// says its history was trimmed.
///
/// Why: lines dropped quietly would make a restarted terminal's history
/// look complete when it is not. Spec: `history_seed_from_lines`.
#[test]
fn a_seed_from_too_many_lines_is_trimmed() {
    let lines: Vec<String> = (0..50).map(|n| format!("line {n}")).collect();
    let size = Size {
        columns: 20,
        lines: 3,
    };
    let seed = history_seed_from_lines(size, 30, lines.iter().map(|line| line.as_bytes()), MAX);
    let restored = Emulator::restore(&seed.blob, 30).unwrap().emulator;
    assert!(restored.history_trimmed());
    let grid = restored.term().grid();
    assert_eq!(grid.history_size(), 30);
    assert_eq!(grid[Line(-1)][Column(5)].c, '4');
    let seed = history_seed_from_lines(size, 60, lines.iter().map(|line| line.as_bytes()), MAX);
    assert!(
        !Emulator::restore(&seed.blob, 60)
            .unwrap()
            .emulator
            .history_trimmed()
    );
}

// ---------------------------------------------------------------------------
// Round-trip losses found by review.

/// One step of a scripted terminal: output, or a resize.
enum Op {
    Out(&'static str),
    Size(u16, u16),
}

/// An emulator of `columns` by `lines` after `ops`.
fn scripted(columns: u16, lines: u16, ops: &[Op]) -> Emulator {
    let mut emulator = Emulator::new(Size { columns, lines }, 50);
    for op in ops {
        match *op {
            Op::Out(text) => {
                feed(&mut emulator, text.as_bytes());
            }
            Op::Size(columns, lines) => {
                common::resize(&mut emulator, columns, lines);
            }
        }
    }
    emulator
}

/// Snapshot `emulator`, restore it, and return what differs.
fn round_trip_differences(emulator: &Emulator) -> Vec<String> {
    let snapshot = emulator.snapshot(MAX).expect("clean");
    let restored = Emulator::restore(&snapshot.blob, 50).expect("restores");
    diff(emulator, &restored.emulator).shown
}

/// The states a review found the format lost or could not encode, from
/// ordinary output and resizes; each now round-trips.
///
/// - A one-column grid holding a leading wide-character spacer with a wrap
///   (reached through a resize) made the encoder panic.
/// - On the bottom row, the stray print of the wrap trick landed on a wide
///   character and cleared the row above's leading spacer.
/// - On one column, the wrap trick's erase cleared the wrap flag it had
///   just set.
/// - Under the alternate screen, a primary cursor the stream could not
///   place (a pending wrap away from the last column) was saved over the
///   primary saved cursor as the stream placed it.
/// - On two columns, reprinting a cursor's pending wrap over a wide
///   character's half cleared the leading spacer at the end of the row
///   above.
///
/// Why: a panic in the shared supervisor's periodic snapshot takes every
/// terminal with it, and each loss is a terminal that differs after a
/// supervisor restart. Spec: `paint`.
#[test]
fn the_review_reproducers_round_trip() {
    use Op::{Out, Size as Resize};
    let cases: Vec<(&str, u16, u16, Vec<Op>)> = vec![
        (
            "one-column leading spacer",
            16,
            6,
            vec![
                Out("\x1b[5;11Hab中"),
                Resize(5, 4),
                Out("\x1b[3J\x1b[3b\x1b[A"),
                Resize(9, 1),
                Resize(1, 5),
                Out("\x1b[L"),
            ],
        ),
        (
            "bottom-row stray print on a wide character",
            8,
            4,
            vec![
                Out("😀"),
                Out("\x1b[3b"),
                Resize(3, 3),
                Resize(3, 6),
                Resize(3, 3),
                Out("\x1b[T"),
            ],
        ),
        (
            "one-column wrap flag",
            8,
            4,
            vec![
                Resize(1, 4),
                Out("abcdefghij"),
                Out("\x1b[r"),
                Out("\x1b[L"),
            ],
        ),
        (
            "primary saved cursor under the alternate screen",
            8,
            4,
            vec![
                Out("\x1b[1;80H"),
                Resize(1, 4),
                Out("\x1b[?1049h"),
                Resize(9, 1),
            ],
        ),
        // The pending wrap's reprint on two columns, where the last column is
        // the second and printing over a wide character's half there cleared
        // the row above's leading spacer: for the cursor, the saved cursor,
        // and the primary saved cursor under the alternate screen.
        ("two-column pending wrap", 2, 4, vec![Out("a中中\x1bM")]),
        (
            "two-column saved pending wrap",
            2,
            2,
            vec![
                Out("😀"),
                Out("\x1b7"),
                Out("\x1b[G"),
                Out("\x1b[I"),
                Out("\x1b[2b"),
            ],
        ),
        (
            "two-column pending wrap across the alternate screen",
            2,
            5,
            vec![
                Out("\x1b[I"),
                Out("😀"),
                Out("\x1b[?1049h\x1b[2J"),
                Out("\x1b[?1049l"),
                Out("\x1b(0q\x1b(B"),
            ],
        ),
    ];
    for (name, columns, lines, ops) in cases {
        let emulator = scripted(columns, lines, &ops);
        assert!(emulator.is_clean(), "{name}");
        assert_eq!(
            round_trip_differences(&emulator),
            Vec::<String>::new(),
            "{name}"
        );
    }
}

/// Output and resizes drawn from the operations that reach awkward grid
/// states (wide characters and their halves, wraps, tabs, inserts and
/// deletes, scrolls near the bottom, saved cursors, the alternate screen,
/// regions, one- and two-column sizes), from 8x4 and from two columns:
/// every clean state round-trips, and the original and the restored
/// emulator stay the same through a few more operations. The review's
/// experiment, with 20,000 sequences here and 300,000 in the ignored test.
///
/// Why: the generated round-trip property mixes long streams, which rarely
/// pass through these small, sharp states, where every loss above was
/// found; and a saved cursor or a pending wrap that restored wrong may only
/// show once more output arrives.
#[test]
fn short_scripts_round_trip() {
    short_scripts(20_000, 7);
}

/// [`short_scripts_round_trip`] at the review's size. A few minutes in
/// release.
#[test]
#[ignore = "minutes; run in release"]
fn short_scripts_round_trip_at_review_size() {
    short_scripts(300_000, 7);
}

/// Run `count` random scripts (see [`short_scripts_round_trip`]).
fn short_scripts(count: usize, seed: u64) {
    const OPS: &[&str] = &[
        "\x1b[5@",
        "\x1b[3P",
        "\x1b[2X",
        "\x1b[K",
        "\x1b[1K",
        "\x1b[J",
        "\x1b[1J",
        "\x1b[L",
        "\x1b[M",
        "\x1b[S",
        "\x1b[T",
        "中",
        "😀",
        "ab",
        "abcdefghij",
        "e\u{301}",
        "\x1b[3b",
        "\t",
        "\r\n",
        "\x1bM",
        "\x1b7",
        "\x1b8",
        "\x1b[?1049h",
        "\x1b[?1049l",
        "\x1b[2;3r",
        "\x1b[r",
        "\x1b[4h",
        "\x1b[4l",
        "\x1b[?7l",
        "\x1b[?7h",
        "\x1b[1;80H",
        "\x1b[99;1H",
        "\x1b[99;80H",
        "\x1b[A",
        "\x1b[3J",
        "x",
        "\x1b[?6h",
        "\x1b[?6l",
        "\x1b[44m",
        "\x1b[0m",
        "\x1b]8;id=a;http://a/\x1b\\",
        "\x1b]8;;\x1b\\",
        "\x1b[I",
        "\x1b[G",
        "\x1b[58;5;3m",
    ];
    let mut r = common::synthetic::Rng::new(seed);
    let draw = |r: &mut common::synthetic::Rng, count: usize| {
        let mut ops = Vec::new();
        for _ in 0..count {
            if r.chance(0.2) {
                ops.push(Op::Size(r.range(1, 9) as u16, r.range(1, 6) as u16));
            } else {
                ops.push(Op::Out(r.pick(OPS)));
            }
        }
        ops
    };
    let shown = |ops: &[Op]| {
        ops.iter()
            .map(|op| match op {
                Op::Out(text) => format!("{text:?}"),
                Op::Size(columns, lines) => format!("resize {columns}x{lines}"),
            })
            .collect::<Vec<String>>()
            .join(", ")
    };
    for case in 0..count {
        let count = r.range(1, 14) as usize;
        let ops = draw(&mut r, count);
        let (columns, lines) = if r.chance(0.5) {
            (8, 4)
        } else {
            (2, r.range(1, 6) as u16)
        };
        let mut emulator = scripted(columns, lines, &ops);
        if !emulator.is_clean() {
            continue;
        }
        let snapshot = emulator.snapshot(MAX).expect("clean");
        let mut restored = Emulator::restore(&snapshot.blob, 50)
            .expect("restores")
            .emulator;
        let differences = diff(&emulator, &restored).shown;
        if !differences.is_empty() {
            panic!(
                "case {case} from {columns}x{lines}: {} differ: {differences:?}",
                shown(&ops)
            );
        }
        let after = draw(&mut r, 6);
        for emulator in [&mut emulator, &mut restored] {
            for op in &after {
                match *op {
                    Op::Out(text) => {
                        feed(emulator, text.as_bytes());
                    }
                    Op::Size(columns, lines) => {
                        common::resize(emulator, columns, lines);
                    }
                }
            }
        }
        let differences = diff(&emulator, &restored).shown;
        if !differences.is_empty() {
            panic!(
                "case {case} from {columns}x{lines}: {} then {} differ: {differences:?}",
                shown(&ops),
                shown(&after)
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The extra storage through a restore.

/// A row of wide characters under one underline color, each head erased
/// alone, leaves fixup cells that all share the live terminal's one extra;
/// they restore sharing one extra too, rather than one each, and the
/// storage stays within its limit.
///
/// Why: a review found this encoder-produced blob restoring to 103 MB of
/// extras against a 64 MiB limit, from a terminal that held 160 bytes of
/// them: fixups made an extra per cell, past every check. Spec: the `v1`
/// module's "Reading", and vendor/README.md on the extra storage.
#[test]
fn fixups_share_extras_and_respect_the_storage() {
    use farhelm_terminal_emulator::alacritty_terminal::term::{
        CELL_EXTRA_COST, MAX_EXTRA_STORAGE_BYTES,
    };
    let mut live = Emulator::new(
        Size {
            columns: 200,
            lines: 60,
        },
        2000,
    );
    let mut row = String::from("\x1b[58;5;1m");
    row.push_str(&"中".repeat(100));
    row.push('\r');
    for column in (1..=200).step_by(2) {
        row.push_str(&format!("\x1b[{column}G\x1b[X"));
    }
    row.push_str("\r\n");
    feed(&mut live, row.repeat(1000).as_bytes());
    assert_eq!(live.term().extra_storage().used(), CELL_EXTRA_COST);
    let snapshot = live.snapshot(MAX).unwrap();
    assert_eq!(snapshot.dropped_history_lines, 0);
    let restored = restore(&snapshot.blob).emulator;
    assert!(restored.term().extra_storage().used() <= 4 * CELL_EXTRA_COST);
    assert!(diff(&live, &restored).is_empty());

    // Distinct extras on more fixup cells than the storage holds: the
    // restore stops making them at the limit.
    let cells = MAX_EXTRA_STORAGE_BYTES / CELL_EXTRA_COST + 1000;
    let mut blob = V1::header(0, 1000, 600, 0, 0, b"").u32(cells as u32);
    for n in 0..cells {
        blob = blob
            .u8(0)
            .i32((n / 1000 % 600) as i32)
            .u32((n % 1000) as u32)
            .u32('x' as u32)
            .u8(0)
            .u16(256)
            .u8(0)
            .u16(257)
            .u16(0)
            .u8(0)
            .u8(1)
            .u8(2)
            .u8((n % 251) as u8)
            .u8((n / 251 % 251) as u8)
            .u8((n / 63001) as u8)
            .u8(0);
    }
    let blob = blob.u32(0).u8(0).blob;
    let restored = Emulator::restore(&blob, 0).unwrap().emulator;
    assert!(restored.term().extra_storage().used() <= MAX_EXTRA_STORAGE_BYTES);
}

/// A terminal that printed a whole history with one underline color while
/// changing only the foreground between characters holds one extra; it
/// restores holding about one too, with every cell's underline color, and
/// nothing dropped.
///
/// Why: a review found the stream resetting every attribute (`SGR 0`) to
/// change the foreground and then setting the underline color again, which
/// gave every restored cell an extra of its own: this terminal's 500,000
/// cells filled the 64 MiB storage after about 419,000 and the rest lost
/// their underline colors, with no flag saying so. Spec: `Writer::set_pen`
/// in `paint`, and the `snapshot` module's "Extra storage".
#[test]
fn changing_the_foreground_keeps_one_shared_underline_color() {
    use farhelm_terminal_emulator::alacritty_terminal::term::CELL_EXTRA_COST;
    let mut live = Emulator::new(
        Size {
            columns: 200,
            lines: 60,
        },
        2500,
    );
    feed(&mut live, b"\x1b[4;58;5;1m");
    let row = format!("{}\r\n", "\x1b[31mx\x1b[32mx".repeat(100));
    for _ in 0..26 {
        feed(&mut live, row.repeat(100).as_bytes());
    }
    assert_eq!(live.term().grid().history_size(), 2500);
    assert_eq!(live.term().extra_storage().used(), CELL_EXTRA_COST);
    let snapshot = live.snapshot(MAX).unwrap();
    assert_eq!(snapshot.dropped_history_lines, 0);
    let restored = Emulator::restore(&snapshot.blob, 2500).unwrap().emulator;
    assert!(restored.term().extra_storage().used() <= 2 * CELL_EXTRA_COST);
    assert!(diff(&live, &restored).is_empty());
}

/// Cells that share an extra in the live terminal because the program got
/// it back with DECRC, interleaved with cells of another underline color,
/// restore sharing extras by content: no more of them than the live
/// terminal has, however the stream reaches each color.
///
/// Why: the stream paints cells in its own order, and every change of
/// underline color makes a new extra, so a restore that only followed the
/// stream would make one per change where the program shared one; equal
/// extras the stream makes are shared like the fixups' (`wire::Extras`).
/// Spec: the `snapshot` module's "Extra storage".
#[test]
fn extras_the_stream_makes_are_shared_by_content() {
    use farhelm_terminal_emulator::alacritty_terminal::term::CELL_EXTRA_COST;
    let mut live = Emulator::new(
        Size {
            columns: 100,
            lines: 30,
        },
        100,
    );
    feed(&mut live, b"\x1b[58;5;1m\x1b7");
    let mut output = String::new();
    for line in 1..=20 {
        for column in (1..=100).step_by(2) {
            // DECRC brings back the template with the first color's extra;
            // the second color is a new copy every time.
            output.push_str(&format!("\x1b8\x1b[{line};{column}Ha\x1b[58;5;2mb"));
        }
    }
    feed(&mut live, output.as_bytes());
    let live_used = live.term().extra_storage().used();
    assert!(live_used >= 1000 * CELL_EXTRA_COST, "{live_used}");
    let restored = restore(&live.snapshot(MAX).unwrap().blob).emulator;
    assert!(restored.term().extra_storage().used() <= 3 * CELL_EXTRA_COST);
    assert!(diff(&live, &restored).is_empty());
}

/// A cursor template whose link the stream cannot write (a URI holding
/// DEL, which OSC 8 carries but `paint` does not put in a stream) is
/// restored through its fixup, and the stream holds no link for it.
///
/// Why: the encoder used to write a template's link raw even when it could
/// not paint it, so a link holding control characters (from a forged blob,
/// before the reader refused those) put any sequence it held into every
/// later snapshot. Spec: `Painter::set_cursor` in `paint`.
#[test]
fn an_unpaintable_template_link_stays_out_of_the_stream() {
    let mut live = emulator(20, 3);
    feed(&mut live, b"\x1b]8;id=d;http://a/\x7f\x1b\\");
    let link = live.term().grid().cursor.template.hyperlink().unwrap();
    assert_eq!(link.uri(), "http://a/\u{7f}");
    let blob = live.snapshot(MAX).unwrap().blob;
    let len = u32::from_be_bytes(blob[30..34].try_into().unwrap()) as usize;
    let stream = &blob[34..34 + len];
    assert!(!stream.contains(&0x7f));
    assert!(!stream.windows(4).any(|window| window == b"\x1b]8;"));
    assert!(diff(&live, &restore(&blob).emulator).is_empty());
}

// ---------------------------------------------------------------------------
// The stream's reading rules.

/// A stream holding a sequence version 1 never writes is refused before
/// any of it is fed, REP above all: the review's blob is a 1x1 terminal
/// whose stream prints one character and then repeats it with
/// `CSI 65535 b` 131,071 times.
///
/// Why: those 1 MiB made a restore do about 8.6 billion operations on the
/// shared supervisor before returning, and passed every check made after
/// feeding. This test would take minutes if the stream were fed; refused
/// first, it takes milliseconds. Spec: the `v1` module's "Reading".
#[test]
fn streams_with_sequences_version_1_never_writes_are_refused() {
    let mut review = b"x".to_vec();
    for _ in 0..131_071 {
        review.extend_from_slice(b"\x1b[65535b");
    }
    let blob = V1::new(0, 1, 1, &review).u32(0).u32(0).u8(0).blob;
    let Err(SnapshotError::Malformed(why)) = Emulator::restore(&blob, 0) else {
        panic!("REP is refused as malformed");
    };
    assert!(why.contains("never writes"), "{why}");
    for stream in [&b"\x1b]52;c;aGk=\x1b\\"[..], b"\x1bc", b"\x1b#8", b"a\rb"] {
        let blob = V1::new(0, 10, 3, stream).u32(0).u32(0).u8(0).blob;
        assert!(
            matches!(
                Emulator::restore(&blob, 0),
                Err(SnapshotError::Malformed(_))
            ),
            "{}",
            stream.escape_ascii()
        );
    }
}

/// Streams of sequences version 1 does write, but repeated far beyond what
/// any painting needs, are refused before feeding as taking too much work:
/// line feeds scrolling a row of 10,000 cells each, erases and switches of
/// a large screen, prints in insert mode, and tabs after the tab stops were
/// cleared.
///
/// Why: these are the amplifications left once REP is refused: each costs a
/// few bytes and a row or a screen of work, so a 16 MiB stream of them would
/// hold the supervisor for minutes. The limit counts the work, not the
/// bytes, so each is caught whatever its length. Spec: the `v1` module's
/// "Reading".
#[test]
fn streams_that_would_take_too_much_work_are_refused() {
    let cases: Vec<(&str, u16, u16, Vec<u8>)> = vec![
        ("line feeds", 10_000, 1, b"\n".repeat(1 << 22)),
        (
            "erasing a screen",
            1_000,
            1_000,
            b"\x1b[?1049h"
                .iter()
                .chain(&b"\x1b[2J".repeat(10_000))
                .copied()
                .collect(),
        ),
        (
            "switching screens",
            1_000,
            1_000,
            b"\x1b[?1049h\x1b[?1049l".repeat(10_000),
        ),
        (
            "printing in insert mode",
            10_000,
            1,
            b"\x1b[4h"
                .iter()
                .chain(&b"x\x1b[G".repeat(100_000))
                .copied()
                .collect(),
        ),
        (
            "tabs without tab stops",
            10_000,
            1,
            b"\x1b[3g"
                .iter()
                .chain(&b"\t\x1b[G".repeat(100_000))
                .copied()
                .collect(),
        ),
    ];
    for (name, columns, lines, stream) in cases {
        let blob = V1::new(0, columns, lines, &stream).u32(0).u32(0).u8(0).blob;
        assert!(
            matches!(
                Emulator::restore(&blob, 0),
                Err(SnapshotError::TooMuchWork { .. })
            ),
            "{name}"
        );
    }
}

/// The encoder's streams stay within the work limit where painting costs
/// the most per byte: every row of history wrapped (a scroll, a wrap and an
/// erase per row), on one column and on a wide terminal, and rows of wide
/// characters ending in leading spacers.
///
/// Why: a limit that refused a real snapshot would make that terminal
/// unavailable after the next supervisor restart; the round-trip tests
/// cover ordinary output, and these are the shapes closest to the limit.
/// Spec: the `v1` module's "Reading".
#[test]
fn the_encoder_s_costliest_streams_restore() {
    let cases: Vec<(u16, u16, String)> = vec![
        (1, 4, "x".repeat(3000)),
        (500, 3, "x".repeat(500 * 2000)),
        (9, 3, "abcdefgh中".repeat(2000)),
        (200, 60, "\t|".repeat(100 * 2000)),
    ];
    for (columns, lines, output) in cases {
        let mut live = Emulator::new(Size { columns, lines }, 2000);
        feed(&mut live, output.as_bytes());
        assert!(live.term().grid().history_size() >= 1000);
        let blob = live.snapshot(MAX).unwrap().blob;
        let restored = Emulator::restore(&blob, 2000).expect("an encoder's blob restores");
        assert!(
            diff(&live, &restored.emulator).is_empty(),
            "{columns}x{lines}"
        );
    }
}

/// The reading rules that refuse a forged blob still accept what live output
/// makes next to it: DEL and a C1 control as the REP character (tag 2), and
/// a URI with as many `;` as OSC 8 can carry (13).
///
/// Why: the refusals above must not reach a blob a real terminal can
/// produce, which would make it unavailable after a restart. Spec: the `v1`
/// module's "Reading".
#[test]
fn what_live_output_makes_next_to_the_refusals_restores() {
    for c in ['\u{7f}', '\u{85}'] {
        let blob = V1::new(0, 10, 3, b"")
            .u32(0)
            .u32(0)
            .u8(2)
            .u32(c as u32)
            .blob;
        let restored = Emulator::restore(&blob, 10).expect("restores").emulator;
        assert_eq!(restored.preceding_char(), Some(c));
    }
    let blob = V1::new(0, 10, 3, b"")
        .u32(1)
        .u8(0)
        .i32(0)
        .u32(0)
        .plain_cell_with_link(b"x", b"http://a/;;;;;;;;;;;;;")
        .u32(0)
        .u8(0)
        .blob;
    let restored = Emulator::restore(&blob, 10).expect("restores").emulator;
    let link = restored.term().grid()[Line(0)][Column(0)]
        .hyperlink()
        .unwrap();
    assert_eq!(link.uri().matches(';').count(), 13);
    // And the next snapshot carries it unchanged.
    let again = restore(&restored.snapshot(MAX).unwrap().blob).emulator;
    assert!(diff(&restored, &again).is_empty());
}

/// A fixup cell at each of version 1's limits restores with all of it: 16
/// combining characters, a 250-byte `id` and a 2083-byte URI; one past any
/// of them is refused (the cases in `malformed_v1_blobs_are_refused`).
///
/// Why: these limits are the format's own, fixed with version 1, and a blob
/// at them is one a terminal of today writes; a later build that tied them to
/// a smaller emulator cap would make those blobs unrestorable. Spec: the
/// `v1` module's "Reading".
#[test]
fn fixups_at_version_1_s_limits_restore_whole() {
    let id = "i".repeat(V1_MAX_ID);
    let uri = format!("http://e/{}", "u".repeat(V1_MAX_URI - 9));
    let mut blob = V1::new(0, 10, 3, b"")
        .u32(1)
        .u8(0)
        .i32(0)
        .u32(0)
        .u32('e' as u32)
        .u8(0)
        .u16(256)
        .u8(0)
        .u16(257)
        .u16(0)
        .u8(V1_MAX_MARKS as u8);
    for _ in 0..V1_MAX_MARKS {
        blob = blob.u32(0x301);
    }
    let blob = blob
        .u8(0)
        .u8(1)
        .bytes(id.as_bytes())
        .bytes(uri.as_bytes())
        .u32(0)
        .u8(0)
        .blob;
    let restored = Emulator::restore(&blob, 10).expect("restores").emulator;
    let cell = &restored.term().grid()[Line(0)][Column(0)];
    assert_eq!(cell.zerowidth().map(<[char]>::len), Some(V1_MAX_MARKS));
    let link = cell.hyperlink().expect("the link");
    assert_eq!((link.id().len(), link.uri().len()), (V1_MAX_ID, V1_MAX_URI));
}

/// Live output that sends an OSC 8 link, and an OSC 2 title, with 20 `;`
/// keeps at most 13 of them in the link's URI (vte keeps 16 parameters of an
/// OSC), and the terminal round-trips.
///
/// Why: the reader refuses a fixup URI with more than 13 `;` because live
/// output cannot make one; this pins that premise on the vendored parser, so
/// that a parser change keeping more parameters fails here instead of
/// silently producing blobs the reader refuses. Spec: `wire::check_link`.
#[test]
fn live_output_keeps_no_more_semicolons_than_version_1_carries() {
    let mut live = emulator(40, 3);
    let semicolons = ";x".repeat(20);
    feed(
        &mut live,
        format!("\x1b]8;id=a;http://a/{semicolons}\x1b\\linked\x1b]8;;\x1b\\").as_bytes(),
    );
    feed(
        &mut live,
        format!("\x1b]2;title{semicolons}\x1b\\").as_bytes(),
    );
    let link = live.term().grid()[Line(0)][Column(0)]
        .hyperlink()
        .expect("the premise: the cell is linked");
    assert!(link.uri().matches(';').count() <= 13, "{}", link.uri());
    assert!(live.title().matches(';').count() <= 14, "{}", live.title());
    let restored = restore(&live.snapshot(MAX).unwrap().blob).emulator;
    assert!(diff(&live, &restored).is_empty());
}

/// Output that leaves DEL or a C1 control as the character REP repeats
/// round-trips, and REP after the restore repeats what it repeats live:
/// DEL as the last byte of ordinary output (vte prints DEL, and alacritty
/// shows nothing for it), and U+0085 whose encoding arrived split across two
/// reads (the one way vte prints a C1 control).
///
/// Why: the encoder wrote such a character into its stream to set the REP
/// character, where the reader refuses DEL and executes a C1 control: any
/// output ending in DEL made a blob no supervisor could restore, and the
/// split C1 control made REP repeat the wrong character after a restore. A
/// review found the first through the generated round-trip property, which
/// hits it only when proptest's random seed generates DEL at the right
/// place. Spec: `Painter::print_rep_char`.
#[test]
fn control_characters_as_the_rep_character_round_trip() {
    for output in [&b"a\x7f"[..], b"hello\x7f", b"ls\r\n\x7f"] {
        let mut live = emulator(20, 4);
        feed(&mut live, output);
        assert_eq!(live.preceding_char(), Some('\u{7f}'), "the premise");
        let restored = restore(&live.snapshot(MAX).unwrap().blob).emulator;
        assert!(
            diff(&live, &restored).is_empty(),
            "{}",
            output.escape_ascii()
        );
    }

    let mut live = emulator(10, 1);
    feed(&mut live, b"x\xc2");
    feed(&mut live, b"\x85");
    assert!(live.is_clean());
    assert_eq!(live.preceding_char(), Some('\u{85}'), "the premise");
    let mut restored = restore(&live.snapshot(MAX).unwrap().blob).emulator;
    assert!(diff(&live, &restored).is_empty());
    feed(&mut live, b"\x1b[3b");
    feed(&mut restored, b"\x1b[3b");
    assert_eq!(restored.screen_text(), live.screen_text());
    assert!(diff(&live, &restored).is_empty());
}

/// A terminal well under the extra storage limit, whose every cell has a
/// combining mark and an underline color of its own, restores whole.
///
/// Why: the restore's sharing table kept every extra the cursor template
/// passed through alive, beside the cells' own, so this terminal (about
/// 60% of the limit) restored with 30,285 cells stripped of their mark and
/// color: a loss the source terminal never had. Spec: `wire::Extras`.
#[test]
fn sharing_extras_keeps_nothing_the_cells_do_not_hold() {
    use farhelm_terminal_emulator::alacritty_terminal::term::{
        CELL_EXTRA_COST, MAX_EXTRA_STORAGE_BYTES,
    };
    let mut live = Emulator::new(
        Size {
            columns: 200,
            lines: 60,
        },
        1200,
    );
    let mut n = 0u32;
    for _ in 0..1260 {
        let mut row = String::new();
        for _ in 0..200 {
            let (r, g, b) = (n % 256, n / 256 % 256, n / 65536);
            row.push_str(&format!("\x1b[58;2;{r};{g};{b}me\u{301}"));
            n += 1;
        }
        row.push_str("\r\n");
        feed(&mut live, row.as_bytes());
    }
    // The premises: a full history, every cell with an extra of its own, and
    // the storage well under its limit.
    assert_eq!(live.term().grid().history_size(), 1200);
    let used = live.term().extra_storage().used();
    assert!(used >= 1259 * 200 * CELL_EXTRA_COST, "{used}");
    assert!(used < MAX_EXTRA_STORAGE_BYTES * 2 / 3, "{used}");
    let snapshot = live.snapshot(MAX).unwrap();
    assert_eq!(snapshot.dropped_history_lines, 0);
    let restored = Emulator::restore(&snapshot.blob, 1200).unwrap().emulator;
    assert!(diff(&live, &restored).is_empty());
    assert!(restored.term().extra_storage().used() <= used);
}

/// A cursor on the last column round-trips with the wrap flag exactly as the
/// live terminal has it, whether the column was written with autowrap on or
/// off and whatever autowrap is afterwards, for a narrow character, a wide
/// one, a blank, a wide character dropped for want of room, and a line
/// narrowed by a resize under the cursor; and the output after the restore
/// (a print, an erase to the right) does the same in both.
///
/// Why: a program raises the flag by printing into the last column with
/// autowrap on, and a reflowing resize can raise it with autowrap off; the
/// stream reproduces a raised flag by reprinting the last column (with
/// autowrap on, which the stream turns off only at its end), so a cursor
/// whose flag is clear must be placed there, not reprinted, or the restored
/// terminal would wrap where the live one overwrites. Spec: `paint`, step 2.
#[test]
fn the_wrap_flag_round_trips_whatever_autowrap_was() {
    // What leaves the cursor on the last column of a 6x3 terminal: output
    // there, or `abcdef` on 8 columns and a resize to 6.
    let lasts = [
        Some("abcdef"),
        Some("abcd中"),
        Some("abc\x1b[1;6H "),
        Some("abcde中"),
        None,
    ];
    let modes = [
        ("", ""),
        ("", "\x1b[?7l"),
        ("\x1b[?7l", ""),
        ("\x1b[?7l", "\x1b[?7h"),
    ];
    // For each case: whether the cursor is on the last column, and the flag.
    let mut states = Vec::new();
    for last in lasts {
        for (before, after) in modes {
            let mut live;
            match last {
                Some(last) => {
                    live = emulator(6, 3);
                    feed(&mut live, format!("{before}{last}{after}").as_bytes());
                }
                None => {
                    live = emulator(8, 3);
                    feed(&mut live, format!("{before}abcdef").as_bytes());
                    common::resize(&mut live, 6, 3);
                    feed(&mut live, after.as_bytes());
                }
            }
            let case = format!("{last:?} {before:?} {after:?}");
            let cursor = &live.term().grid().cursor;
            states.push((cursor.point.column.0 == 5, cursor.input_needs_wrap));
            let mut restored = restore(&live.snapshot(MAX).unwrap().blob).emulator;
            assert_eq!(
                restored.term().grid().cursor.input_needs_wrap,
                live.term().grid().cursor.input_needs_wrap,
                "{case}"
            );
            assert!(diff(&live, &restored).is_empty(), "{case}");
            for emulator in [&mut live, &mut restored] {
                feed(emulator, b"x\x1b[Kyz");
            }
            assert!(diff(&live, &restored).is_empty(), "{case} then more");
        }
    }
    // The premise: the cases include a raised flag, and a cursor on the last
    // column without it (not only cursors that wrapped to the next line).
    assert!(states.contains(&(true, true)));
    assert!(states.contains(&(true, false)));
    // And the resize raises it with autowrap off (the fifth case, written
    // with autowrap off and left so).
    let mut resized = emulator(8, 3);
    feed(&mut resized, b"\x1b[?7labcdef");
    common::resize(&mut resized, 6, 3);
    assert!(resized.term().grid().cursor.input_needs_wrap);
    assert!(!resized.term().mode().contains(TermMode::LINE_WRAP));
}

/// Every combination of the UTF-8 (1005) and SGR (1006) mouse encodings
/// round-trips, both set included, under click reporting and the alternate
/// screen alike; and resetting either one afterwards leaves the restored
/// terminal in the same encoding as the live one.
///
/// Why: the two are independent modes (the vendored patch, as in tmux), so
/// a terminal can hold both, and the stream sets each on its own; a restore
/// that kept only one would encode the program's mouse reports differently
/// once it turned SGR off after a supervisor restart. Spec: `paint`'s
/// private modes, and `Modes::mouse_encoding`.
#[test]
fn both_mouse_encodings_set_round_trip() {
    let encodings = [
        "",
        "\x1b[?1005h",
        "\x1b[?1006h",
        "\x1b[?1006h\x1b[?1005h",
        "\x1b[?1005h\x1b[?1006h",
    ];
    for screen in ["", "\x1b[?1049h"] {
        for set in encodings {
            for reset in ["", "\x1b[?1006l", "\x1b[?1005l"] {
                let case = format!("{screen:?} {set:?} then {reset:?}");
                let mut live = emulator(20, 4);
                feed(
                    &mut live,
                    format!("{screen}\x1b[?1000h{set}text").as_bytes(),
                );
                let mut restored = restore(&live.snapshot(MAX).unwrap().blob).emulator;
                assert!(diff(&live, &restored).is_empty(), "{case}");
                for emulator in [&mut live, &mut restored] {
                    feed(emulator, reset.as_bytes());
                }
                assert_eq!(restored.modes(), live.modes(), "{case}");
                assert!(diff(&live, &restored).is_empty(), "{case}");
            }
        }
    }
    // The state this exists for: both set, in either order, restores with
    // both set, and SGR reported until 1006 is reset.
    let both = TermMode::SGR_MOUSE | TermMode::UTF8_MOUSE;
    for set in ["\x1b[?1006h\x1b[?1005h", "\x1b[?1005h\x1b[?1006h"] {
        let mut live = emulator(20, 4);
        feed(&mut live, format!("\x1b[?1000h{set}").as_bytes());
        assert!(live.term().mode().contains(both), "{set:?}");
        let mut restored = restore(&live.snapshot(MAX).unwrap().blob).emulator;
        assert!(restored.term().mode().contains(both), "{set:?}");
        let encoding = |emulator: &Emulator| emulator.modes().mouse_encoding;
        assert_eq!(encoding(&restored), MouseEncoding::Sgr, "{set:?}");
        feed(&mut restored, b"\x1b[?1006l");
        assert_eq!(encoding(&restored), MouseEncoding::Utf8, "{set:?}");
    }
}

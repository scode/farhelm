//! Hostile output: the vendored caps hold when output arrives through this
//! crate, chunked the way the supervisor will read it.
//!
//! One program must not be able to grow the shared supervisor's memory
//! without bound. The caps live in the vendored crates (vendor/README.md,
//! "Caps on hostile output"), which test them on their own; these tests
//! check that they still hold behind this crate's feeding, and assert the
//! bounds on what is stored (cell contents, title length, buffered bytes)
//! rather than on process memory, which other tests running in parallel
//! would make meaningless.
//!
//! Each cap here bounds one piece of state. The whole terminal's resident
//! memory is measured against the per-terminal budget by the ignored tests in
//! `memory.rs` (crate docs, "Memory budget").

mod common;

use std::time::Instant;

use common::{emulator, feed};
use farhelm_terminal_emulator::alacritty_terminal::grid::Dimensions;
use farhelm_terminal_emulator::alacritty_terminal::index::{Column, Line};
use farhelm_terminal_emulator::alacritty_terminal::term::cell::{Cell, MAX_ZEROWIDTH_CHARS};
use farhelm_terminal_emulator::alacritty_terminal::term::{
    CELL_EXTRA_COST, HYPERLINK_COST, MAX_EXTRA_STORAGE_BYTES, MAX_HYPERLINK_ID_BYTES,
    MAX_HYPERLINK_URI_BYTES, MAX_TITLE_BYTES,
};
use farhelm_terminal_emulator::alacritty_terminal::vte::OSC_PAYLOAD_LIMIT;
use farhelm_terminal_emulator::{ClipboardTarget, Collected, Emulator, Notice, Size};

const MIB: usize = 1024 * 1024;

/// Feed `output` in reads of 64 KiB, the size of one host output chunk, and
/// collect every effect.
fn feed_in_reads(emulator: &mut Emulator, output: &[u8]) -> Collected {
    let mut all = Collected::default();
    for read in output.chunks(64 * 1024) {
        let effects = feed(emulator, read);
        all.replies.extend(effects.replies);
        all.notices.extend(effects.notices);
    }
    all
}

/// The cell at `line`, `column` of the screen.
fn cell(emulator: &Emulator, line: i32, column: usize) -> &Cell {
    &emulator.term().grid()[Line(line)][Column(column)]
}

/// The URI of the hyperlink on a cell, if any.
fn link(emulator: &Emulator, line: i32, column: usize) -> Option<String> {
    cell(emulator, line, column)
        .hyperlink()
        .map(|link| link.uri().to_owned())
}

/// A flood of combining characters onto one cell keeps only the first
/// `MAX_ZEROWIDTH_CHARS`, and the cursor does not move.
///
/// Why: upstream appends every combining character to the cell, four bytes
/// each, so one cell could grow without limit.
#[test]
fn a_combining_character_flood_is_capped_per_cell() {
    let mut emulator = emulator(20, 3);
    let flood: String = std::iter::once('e')
        .chain(std::iter::repeat_n('\u{301}', 200_000))
        .collect();
    feed_in_reads(&mut emulator, flood.as_bytes());
    assert_eq!(
        cell(&emulator, 0, 0).zerowidth().map(<[char]>::len),
        Some(MAX_ZEROWIDTH_CHARS)
    );
    assert_eq!(emulator.term().grid().cursor.point.column, Column(1));
    let expected: String = std::iter::once('e')
        .chain(std::iter::repeat_n('\u{301}', MAX_ZEROWIDTH_CHARS))
        .collect();
    assert_eq!(emulator.screen_tail(4096), expected);
}

/// An 8 MB OSC 52 clipboard write, under the payload limit, is delivered
/// whole; one over the limit is dropped whole rather than truncated; and the
/// parser is back to normal after both.
///
/// Why: the limit sits above every copy the browser terminal accepts, so a
/// real 8 MB copy still works, while a larger payload stops being buffered
/// and must not reach the clipboard as corrupt, truncated data.
#[test]
fn clipboard_writes_are_bounded_by_the_payload_limit() {
    let mut emulator = emulator(20, 3);
    // "eHh4" is base64 for "xxx": 8 MiB of it decode to 6 MiB of text.
    let payload = "eHh4".repeat(2 * MIB);
    let effects = feed_in_reads(&mut emulator, format!("\x1b]52;c;{payload}\x07").as_bytes());
    let [Notice::Clipboard { target, text }] = &effects.notices[..] else {
        panic!(
            "expected one clipboard write, got {} notices",
            effects.notices.len()
        );
    };
    assert_eq!(*target, ClipboardTarget::Clipboard);
    assert_eq!(text.len(), 6 * MIB);
    assert!(text.bytes().all(|byte| byte == b'x'));
    assert!(emulator.is_clean());

    let oversized = "eHh4".repeat(OSC_PAYLOAD_LIMIT / 4 + 1);
    let effects = feed_in_reads(
        &mut emulator,
        format!("\x1b]52;c;{oversized}\x07").as_bytes(),
    );
    assert_eq!(
        effects,
        Collected::default(),
        "an oversized write is dropped"
    );
    assert!(emulator.is_clean());

    let effects = feed(&mut emulator, b"\x1b]52;c;aGk=\x07after");
    assert_eq!(
        effects.notices,
        vec![Notice::Clipboard {
            target: ClipboardTarget::Clipboard,
            text: "hi".to_owned()
        }]
    );
    assert_eq!(emulator.screen_tail(100), "after");
}

/// An 8 MB title is cut to `MAX_TITLE_BYTES`, on a character boundary, and so
/// is every title pushed onto the title stack.
///
/// Why: the title stack holds up to 4096 titles, so uncapped 10 MiB titles
/// would let one program pin tens of gigabytes; and status detection reads
/// the title, so a cut must not split a character.
#[test]
fn titles_are_capped() {
    let mut emulator = emulator(20, 3);
    let effects = feed_in_reads(
        &mut emulator,
        format!("\x1b]2;{}\x07", "A".repeat(8 * MIB)).as_bytes(),
    );
    assert_eq!(effects.notices, vec![Notice::Title]);
    assert_eq!(emulator.title().len(), MAX_TITLE_BYTES);
    assert!(emulator.title().bytes().all(|byte| byte == b'A'));

    for _ in 0..3 {
        feed_in_reads(
            &mut emulator,
            format!("\x1b[22t\x1b]2;{}\x07", "B".repeat(8 * MIB)).as_bytes(),
        );
    }
    let stack = emulator.term().title_stack();
    assert_eq!(stack.len(), 3);
    assert!(stack.iter().all(|title| {
        title
            .as_ref()
            .is_some_and(|title| title.len() <= MAX_TITLE_BYTES)
    }));

    // "中" is three bytes, so 4096 bytes would end inside one.
    feed(
        &mut emulator,
        format!("\x1b]2;{}\x07", "中".repeat(2000)).as_bytes(),
    );
    assert_eq!(emulator.title().len(), MAX_TITLE_BYTES / 3 * 3);
    assert!(emulator.title().chars().all(|c| c == '中'));
}

/// A hyperlink with a URI or id over its limit is not stored: it ends the
/// active link, and the text after it is unlinked. Links at the limits are
/// kept, and a link too long for the parser to buffer at all ends the
/// active link too.
///
/// Why: every cell printed under a link holds it, and a program can start a
/// new link before every character, so the link's length multiplies into the
/// grid's memory.
#[test]
fn long_hyperlinks_are_not_stored() {
    let mut emulator = emulator(40, 3);
    let longest_uri = format!("http://{}", "u".repeat(MAX_HYPERLINK_URI_BYTES - 7));
    let longest_id = "i".repeat(MAX_HYPERLINK_ID_BYTES);
    let start = "\x1b]8;id=a;http://ok\x1b\\";
    let output = [
        // Columns 0 to 4: at the limits, over them, and over the parser's.
        format!("{start}A\x1b]8;;{longest_uri}\x1b\\B"),
        format!("{start}C\x1b]8;;{longest_uri}u\x1b\\D"),
        format!("{start}E\x1b]8;id={longest_id};http://id\x1b\\F"),
        format!("{start}G\x1b]8;id={longest_id}i;http://id\x1b\\H"),
        format!(
            "{start}I\x1b]8;;http://{}\x1b\\J",
            "u".repeat(OSC_PAYLOAD_LIMIT)
        ),
    ]
    .concat();
    feed_in_reads(&mut emulator, output.as_bytes());

    let ok = Some("http://ok".to_owned());
    assert_eq!(link(&emulator, 0, 0), ok);
    assert_eq!(
        link(&emulator, 0, 1),
        Some(longest_uri),
        "a URI at the limit"
    );
    assert_eq!(link(&emulator, 0, 2), ok);
    assert_eq!(link(&emulator, 0, 3), None, "a URI over the limit");
    assert_eq!(link(&emulator, 0, 4), ok);
    assert_eq!(
        link(&emulator, 0, 5),
        Some("http://id".to_owned()),
        "an id at the limit"
    );
    assert_eq!(link(&emulator, 0, 6), ok);
    assert_eq!(link(&emulator, 0, 7), None, "an id over the limit");
    assert_eq!(link(&emulator, 0, 8), ok);
    assert_eq!(link(&emulator, 0, 9), None, "a link the parser dropped");
    assert_eq!(emulator.screen_tail(100), "ABCDEFGHIJ");
}

/// A synchronized update that grows past vte's 2 MiB buffer is ended and
/// drawn rather than buffered further, and the buffer never holds more than
/// 2 MiB.
///
/// Why: a program that opens an update and never ends it, while printing
/// without pause, would otherwise grow the buffer for as long as it runs;
/// the deadline alone does not help, since output that keeps arriving keeps
/// being buffered until the owner's wake-up.
#[test]
fn an_endless_update_is_bounded_by_the_sync_buffer() {
    let mut emulator = emulator(20, 3);
    feed(&mut emulator, b"\x1b[?2026h");
    let line = "0123456789abcdefghi\r\n".repeat(64 * 1024 / 21);
    let mut buffered_most = 0;
    for _ in 0..48 {
        feed(&mut emulator, line.as_bytes());
        buffered_most = buffered_most.max(emulator.sync_buffered_bytes());
        assert!(emulator.sync_buffered_bytes() < 2 * MIB);
    }
    assert!(
        buffered_most > MIB,
        "the update was buffered before it overflowed"
    );
    assert_eq!(
        emulator.sync_deadline(),
        None,
        "the overflow ended the update"
    );
    assert!(emulator.is_clean());
    assert!(emulator.screen_text().contains("0123456789abcdefghi"));
}

/// A screen full of tab cells whose tab stops were then cleared is
/// captured in about the time a screen of plain characters of the same size
/// takes, on the widest screen the host allows.
///
/// Why: status detection captures agents' screens on a schedule, so a
/// capture that costs the supervisor quadratic time in the row width is a
/// cost one program can impose on every sample. An earlier version searched
/// for each tab's next stop separately, which here means scanning to the end
/// of the row from every cell, hundreds of times as long as the plain screen
/// (measured at about 5,500 times in debug and 700 to 1,000 in release).
/// The oracle is that ratio rather than a time, so that it holds in debug
/// and release builds and on a loaded machine; the linear capture runs at
/// about the plain screen's speed, far under the bound. Spec:
/// `tab_landings` in `src/screen.rs`.
#[test]
fn a_screen_full_of_tabs_captures_in_linear_time() {
    const COLUMNS: usize = 10_000;
    const LINES: usize = 10;
    /// How many times slower than the plain screen the tab screen may be.
    const MAX_RATIO: u32 = 20;

    // A tab stop in every column, a tab from every column, then no stops.
    let mut tabs = emulator(COLUMNS as u16, LINES as u16);
    let mut output = Vec::new();
    for column in 1..=COLUMNS {
        output.extend_from_slice(format!("\x1b[1;{column}H\x1bH").as_bytes());
    }
    for line in 1..=LINES {
        output.extend_from_slice(format!("\x1b[{line};1H").as_bytes());
        output.extend(std::iter::repeat_n(b'\t', COLUMNS));
    }
    output.extend_from_slice(b"\x1b[3g");
    feed(&mut tabs, &output);
    assert_eq!(cell(&tabs, 0, 0).c, '\t', "the premise: tabs in the cells");
    assert_eq!(cell(&tabs, LINES as i32 - 1, COLUMNS - 1).c, '\t');
    assert!(
        !tabs.term().tab_stops().contains(&true),
        "the premise: no tab stops left"
    );

    let mut plain = emulator(COLUMNS as u16, LINES as u16);
    let mut output = Vec::new();
    for line in 1..=LINES {
        output.extend_from_slice(format!("\x1b[{line};1H").as_bytes());
        output.extend(std::iter::repeat_n(b'x', COLUMNS));
    }
    feed(&mut plain, &output);

    // The fastest of a few captures each, so that one capture slowed by the
    // machine's load does not decide the ratio.
    let fastest = |emulator: &Emulator| {
        (0..3)
            .map(|_| {
                let started = Instant::now();
                let text = emulator.screen_text();
                (started.elapsed(), text)
            })
            .min_by_key(|(took, _)| *took)
            .expect("three captures")
    };
    let (plain_took, plain_text) = fastest(&plain);
    let (tabs_took, tabs_text) = fastest(&tabs);

    assert_eq!(
        plain_text,
        format!("{}\n", "x".repeat(COLUMNS)).repeat(LINES)
    );
    // With no stops left, every tab's span reaches the last column and holds
    // other tabs, so each captures as a space; only the one next to the last
    // column has an empty span, and captures as a tab, as tmux records it.
    let row = format!("{}\t\n", " ".repeat(COLUMNS - 2));
    assert!(tabs_text == row.repeat(LINES), "unexpected capture shape");
    assert!(
        tabs_took <= plain_took * MAX_RATIO,
        "tabs took {tabs_took:?}, plain {plain_took:?}: more than {MAX_RATIO} times as long"
    );
}

/// A new hyperlink before every character, and a combining mark on every
/// cell, stop attaching once the terminal's cell extras take
/// `MAX_EXTRA_STORAGE_BYTES`: the text after that prints unlinked and without
/// its marks. A snapshot restores what was there, charged to the restored
/// terminal's own storage. Once the text is cleared and its rows reused, the
/// storage is back and links attach again.
///
/// The rows matter, not the clearing alone: alacritty can keep up to a
/// thousand rows that left the grid, cells and all, to reuse them, and their
/// extras stay alive (and counted) until new output reaches and resets them,
/// which takes about that many new lines; so the test prints them.
///
/// Why: without the bound, a program doing either over a full scrollback
/// gives every cell an allocation of its own, about 390 MiB per terminal, or
/// about 6 GB with the longest links (vendor/README.md); the per-cell length
/// caps alone cannot stop it.
#[test]
fn floods_of_cell_extras_are_bounded() {
    let size = Size {
        columns: 200,
        lines: 100,
    };
    let mut emulator = Emulator::new(size, 1200);
    let link = |n: usize| (format!("{n:06}"), format!("http://x/{n:06}"));
    let per_link = |n: usize| {
        let (id, uri) = link(n);
        id.len() + uri.len() + HYPERLINK_COST + CELL_EXTRA_COST
    };
    let fits = MAX_EXTRA_STORAGE_BYTES / per_link(0);
    let mut output = String::new();
    for n in 0..fits + 100 {
        let (id, uri) = link(n);
        output.push_str(&format!("\x1b]8;id={id};{uri}\x1b\\x"));
    }
    feed_in_reads(&mut emulator, output.as_bytes());
    let storage = emulator.term().extra_storage().used();
    assert_eq!(storage, fits * per_link(0));
    let grid = emulator.term().grid();
    let linked = (-(grid.history_size() as i32)..100)
        .flat_map(|line| (0..200).map(move |column| (line, column)))
        .filter(|&(line, column)| grid[Line(line)][Column(column)].hyperlink().is_some())
        .count();
    assert_eq!(linked, fits);

    let restored = Emulator::restore(&emulator.snapshot(16 << 20).unwrap().blob, 1200)
        .unwrap()
        .emulator;
    assert_eq!(restored.term().extra_storage().used(), storage);

    feed(&mut emulator, b"\x1b[H\x1b[2J\x1b[3J");
    feed(&mut emulator, b"\x1b]8;;\x1b\\");
    feed(&mut emulator, "\r\n".repeat(1300).as_bytes());
    assert_eq!(emulator.term().extra_storage().used(), 0);
    feed(&mut emulator, b"\x1b]8;id=again;http://x/\x1b\\y");
    assert!(cell(&emulator, 99, 0).hyperlink().is_some());

    // Room in history for every cell, so that no row is reused (and its
    // extras given back) before the storage fills.
    let mut marked = Emulator::new(size, 2200);
    let fits = MAX_EXTRA_STORAGE_BYTES / CELL_EXTRA_COST;
    let cells = fits + 100;
    feed_in_reads(&mut marked, "e\u{301}".repeat(cells).as_bytes());
    assert_eq!(marked.term().extra_storage().used(), fits * CELL_EXTRA_COST);
    let last = cell(&marked, 99, (cells - 1) % 200);
    assert_eq!(
        (last.c, last.zerowidth()),
        ('e', None),
        "the last mark dropped"
    );
}

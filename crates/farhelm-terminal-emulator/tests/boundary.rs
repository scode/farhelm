//! Clean boundaries and the snapshot split candidate, and what feeding in
//! different reads does and does not change.
//!
//! A snapshot taken anywhere but a clean boundary cannot be continued: the
//! rebuilt parser would misread the rest of a sequence it never saw the start
//! of, or draw an update's bytes it never buffered. When a snapshot is due,
//! the supervisor will feed a read up to `snapshot_split_candidate`, ask the
//! emulator `is_clean()`, and snapshot only if it says so. These tests check
//! that the candidate is clean on the reads agents produce, that an unclean
//! one is simply reported unclean, and that splitting reads (there or
//! anywhere) leaves the same terminal once all output is fed and any open
//! update flushed.

mod common;

use common::output::stream;
use common::{emulator, feed, resize};
use farhelm_terminal_emulator::alacritty_terminal::grid::{Dimensions, Grid};
use farhelm_terminal_emulator::alacritty_terminal::index::Line;
use farhelm_terminal_emulator::alacritty_terminal::term::cell::Cell;
use farhelm_terminal_emulator::alacritty_terminal::term::color::COUNT;
use farhelm_terminal_emulator::{Collected, Emulator, FeedMode, snapshot_split_candidate};
use proptest::prelude::*;

/// The emulator after `output`.
fn fed(output: &[u8]) -> Emulator {
    let mut emulator = emulator(20, 5);
    feed(&mut emulator, output);
    emulator
}

/// Where two emulators' terminal states differ, if anywhere, as far as the
/// crate's public API shows them: every cell of both screens and the
/// history, cursors and saved cursors, the history counters, modes, cursor
/// style, tab stops, scroll region, active character set, all colors, title,
/// title stack, kitty keyboard stacks and cleanliness.
///
/// Not reachable, so not compared: vte's own state, which the emulator keeps
/// private (the character REP would repeat, a partial sequence's parameters,
/// the buffered bytes of an open update beyond their count), and the
/// emulator's own record of an open update for DECRQM 2026, which only shows
/// in answers.
///
/// Cell by cell because `Grid`'s own equality asserts that neither grid's ring
/// buffer has rotated, which any output that scrolls breaks.
fn differences(a: &Emulator, b: &Emulator) -> Option<String> {
    fn grid(name: &str, a: &Grid<Cell>, b: &Grid<Cell>) -> Option<String> {
        if a.history_size() != b.history_size() {
            return Some(format!(
                "{name} history {} vs {}",
                a.history_size(),
                b.history_size()
            ));
        }
        for line in -(a.history_size() as i32)..a.screen_lines() as i32 {
            if a[Line(line)] != b[Line(line)] {
                return Some(format!("{name} line {line}"));
            }
        }
        if a.cursor != b.cursor || a.saved_cursor != b.saved_cursor {
            return Some(format!("{name} cursor"));
        }
        (a.lines_scrolled_into_history() != b.lines_scrolled_into_history()
            || a.history_clears() != b.history_clears())
        .then(|| format!("{name} history counters"))
    }
    /// The name of the first field whose values differ.
    fn first(fields: &[(&str, bool)]) -> Option<String> {
        fields
            .iter()
            .find(|(_, same)| !same)
            .map(|(name, _)| (*name).to_owned())
    }
    let (ta, tb) = (a.term(), b.term());
    grid("active", ta.grid(), tb.grid())
        .or_else(|| grid("inactive", ta.inactive_grid(), tb.inactive_grid()))
        .or_else(|| {
            (0..COUNT)
                .find(|&index| ta.colors()[index] != tb.colors()[index])
                .map(|index| format!("color {index}"))
        })
        .or_else(|| {
            first(&[
                ("modes", ta.mode() == tb.mode()),
                ("cursor style", ta.cursor_style() == tb.cursor_style()),
                ("tab stops", ta.tab_stops() == tb.tab_stops()),
                ("scroll region", ta.scroll_region() == tb.scroll_region()),
                ("active charset", ta.active_charset() == tb.active_charset()),
                ("title", a.title() == b.title()),
                ("title stack", ta.title_stack() == tb.title_stack()),
                (
                    "keyboard mode stacks",
                    ta.keyboard_mode_stacks() == tb.keyboard_mode_stacks(),
                ),
                ("cleanliness", a.is_clean() == b.is_clean()),
            ])
        })
}

/// One event of a terminal's journal: output, or a resize between outputs.
/// A read never spans a resize, as in the host's journal.
#[derive(Debug, Clone)]
enum Event {
    Output(Vec<u8>),
    Resize(u16, u16),
}

/// A journal of output and resizes, and the lengths of the reads its output
/// arrives in.
fn journal() -> impl Strategy<Value = (Vec<Event>, Vec<usize>)> {
    let event = prop_oneof![
        4 => stream(30).prop_map(Event::Output),
        1 => (1u16..30, 1u16..8).prop_map(|(columns, lines)| Event::Resize(columns, lines)),
    ];
    (
        prop::collection::vec(event, 0..6),
        prop::collection::vec(1usize..48, 1..64),
    )
}

/// How a journal's output is cut into feeds.
#[derive(Debug, Clone, Copy)]
enum Cut {
    /// Each output event in one feed.
    Whole,
    /// One byte per feed.
    Bytewise,
    /// In reads of the generated lengths.
    Chunked,
    /// In those reads, each split at its snapshot candidate.
    Split,
}

/// Feed `events` to a fresh 20x5 emulator, cut as `cut` says, resizing where
/// the journal does, then flush any open update as a recovering supervisor
/// does at the end; return the emulator and everything it produced.
fn replay(events: &[Event], reads: &[usize], cut: Cut) -> (Emulator, Collected) {
    let mut emulator = emulator(20, 5);
    let mut all = Collected::default();
    let mut keep = |effects: Collected| {
        all.replies.extend(effects.replies);
        all.notices.extend(effects.notices);
    };
    let mut lengths = reads.iter().cycle();
    for event in events {
        match event {
            Event::Resize(columns, lines) => keep(resize(&mut emulator, *columns, *lines)),
            Event::Output(output) => match cut {
                Cut::Whole => keep(feed(&mut emulator, output)),
                Cut::Bytewise => {
                    for byte in output {
                        keep(feed(&mut emulator, std::slice::from_ref(byte)));
                    }
                }
                Cut::Chunked | Cut::Split => {
                    let mut start = 0;
                    while start < output.len() {
                        let end = (start + lengths.next().expect("cycles")).min(output.len());
                        let read = &output[start..end];
                        let at = match cut {
                            Cut::Split => snapshot_split_candidate(read).unwrap_or(read.len()),
                            _ => read.len(),
                        };
                        keep(feed(&mut emulator, &read[..at]));
                        keep(feed(&mut emulator, &read[at..]));
                        start = end;
                    }
                }
            },
        }
    }
    let mut flushed = Collected::default();
    emulator.flush_sync(FeedMode::Live, &mut flushed);
    keep(flushed);
    (emulator, all)
}

/// `replies` with the state in DECRQM 2026 answers blanked: whether an update
/// is open when a query is parsed can depend on the cut (see
/// `Emulator::feed`), so that answer rightly differs.
fn without_update_state(replies: &[u8]) -> Vec<u8> {
    let mut normalized = replies.to_vec();
    for state in [b"\x1b[?2026;1$y", b"\x1b[?2026;2$y"] {
        let mut at = 0;
        while let Some(found) = normalized[at..]
            .windows(state.len())
            .position(|window| window == state)
        {
            normalized[at + found + 8] = b'_';
            at += found + state.len();
        }
    }
    normalized
}

/// The parser state alone does not make a boundary clean: right after an
/// update starts the parser is between sequences, but the update is open.
///
/// Why: the snapshot spike found that checking only that the update buffer
/// is empty, or only the parser state, accepts exactly this point, and a
/// snapshot there would lose the update. Spec: `Emulator::is_clean`.
#[test]
fn clean_means_ground_with_no_open_update() {
    let cases: &[(&str, &[u8], bool)] = &[
        ("fresh", b"", true),
        ("after text", b"hello", true),
        ("after a complete sequence", b"\x1b[1mhi\x1b[0m", true),
        ("mid escape sequence", b"hello\x1b[3", false),
        ("after a lone ESC", b"\x1b", false),
        ("mid UTF-8 character", b"\xe4\xb8", false),
        ("mid OSC", b"\x1b]2;title", false),
        ("right after an update starts", b"\x1b[?2026h", false),
        ("inside an update", b"\x1b[?2026hframe", false),
        ("after an update ends", b"\x1b[?2026hframe\x1b[?2026l", true),
        ("after a cancelled sequence", b"\x1b[12\x18", true),
    ];
    for (name, output, clean) in cases {
        assert_eq!(fed(output).is_clean(), *clean, "{name}");
    }
}

/// On the reads agents produce, feeding up to the split candidate leaves the
/// emulator clean: before an update still open at the read's end, after a
/// frame that ended, and before a sequence the read ends inside, whether
/// the emulator was clean at the read's start or inside an update.
///
/// Why: these are the snapshot opportunities the snapshot spike found in
/// every read of real agent output; the candidate rule exists to find them.
/// Spec: `snapshot_split_candidate`.
#[test]
fn the_candidate_is_clean_on_agent_shaped_reads() {
    // (output before the read, the read)
    let cases: &[(&[u8], &[u8])] = &[
        (b"", b"\x1b[?2026hone\x1b[?2026l text \x1b[?2026htwo\x1b[1m"),
        (b"\x1b[?2026hframe", b"rest\x1b[?2026l text \x1b[?2026htwo"),
        (b"", b"\x1b[1mbold\x1b[0m more \x1b[3"),
        (b"", b"text \x1b[?2026hpartial \x1b[1mbold"),
        (b"\x1b[3", b"1mtext\x1b[?2026hframe"),
        (b"\xe4\xb8", b"\xadthe \x1b[1mtext\x1b[?2026h"),
    ];
    for (before, read) in cases {
        let mut emulator = fed(before);
        let at = snapshot_split_candidate(read).expect("each read has an ESC");
        feed(&mut emulator, &read[..at]);
        assert!(
            emulator.is_clean(),
            "{before:?} then {read:?}, split at {at}"
        );
    }
}

/// A candidate that is not clean is reported unclean by `is_clean`, and
/// feeding the rest of the read then ends as feeding the read whole does.
///
/// Why: the candidate is only a hint; the supervisor relies on `is_clean`,
/// asked of the emulator itself, to refuse it, and on the split costing
/// nothing when it does. Spec: `snapshot_split_candidate`,
/// `Emulator::is_clean`.
#[test]
fn an_unclean_candidate_is_reported_unclean() {
    let before = b"\x1b[?2026hframe";
    let read = b" more \x1b[1mbold\x1b[?2026l";
    let mut split = fed(before);
    let at = snapshot_split_candidate(read).expect("the read has an ESC");
    feed(&mut split, &read[..at]);
    assert!(!split.is_clean(), "inside the open update");
    feed(&mut split, &read[at..]);

    let mut whole = fed(before);
    feed(&mut whole, read);
    assert_eq!(differences(&split, &whole), None);
    assert!(split.is_clean());
}

/// Known difference, upstream vte's: a C1 control encoded in UTF-8 and split
/// across two feeds is printed rather than executed, so a later REP repeats
/// it (drawing nothing) instead of the character before it. Fed whole, REP
/// repeats that character.
///
/// Why: this is the one known case where the cut into reads still changes
/// the terminal after a final flush, and the docs say so
/// (`Emulator::feed`, vendor/README.md). Pinning it makes a vte upgrade that
/// changes it show up here, so those docs get updated. Spec:
/// `Emulator::feed`.
#[test]
fn a_split_encoded_c1_control_changes_what_rep_repeats() {
    let bytes = b"x\xc2\x85\x1b[3b";
    let mut whole = emulator(10, 1);
    feed(&mut whole, bytes);
    assert_eq!(whole.screen_text(), "xxxx\n");

    let mut split = emulator(10, 1);
    feed(&mut split, &bytes[..2]);
    feed(&mut split, &bytes[2..]);
    assert_eq!(
        split.screen_text(),
        "x\n",
        "upstream behavior; see vendor/README.md"
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Fed whole, one byte at a time, in reads of varying size, or in those
    /// reads split at their snapshot candidates, with the journal's resizes
    /// between outputs, the emulator ends in the same state once every open
    /// update is flushed, and produces the same answers and notices in the
    /// same order (DECRQM 2026 answers aside).
    ///
    /// Why: a recovering supervisor rebuilds a terminal by feeding the
    /// journal in different reads from the live one, and flushes any open
    /// update at the end; the result must not depend on the cut. Before that
    /// flush it can (`Emulator::feed`), which is also why `resize` flushes an
    /// open update first. A UTF-8-encoded C1 control split across reads is
    /// the one known exception (upstream vte prints it, so a later REP
    /// repeats it), so journals containing one are left out. Spec:
    /// `Emulator::feed`, `Emulator::resize`.
    #[test]
    fn the_cut_into_reads_does_not_matter((events, reads) in journal()) {
        prop_assume!(!events.iter().any(|event| matches!(
            event,
            Event::Output(output)
                if output.windows(2).any(|pair| pair[0] == 0xc2 && (0x80..=0x9f).contains(&pair[1]))
        )));
        let (whole, whole_effects) = replay(&events, &reads, Cut::Whole);
        for cut in [Cut::Bytewise, Cut::Chunked, Cut::Split] {
            let (other, effects) = replay(&events, &reads, cut);
            prop_assert_eq!(differences(&whole, &other), None, "{:?} feeding", cut);
            prop_assert_eq!(&effects.notices, &whole_effects.notices, "{:?} feeding", cut);
            prop_assert_eq!(
                without_update_state(&effects.replies),
                without_update_state(&whole_effects.replies),
                "{:?} feeding",
                cut
            );
        }
    }
}

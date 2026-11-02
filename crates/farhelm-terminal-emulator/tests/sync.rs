//! Synchronized output (mode 2026): an update is held until it ends, and
//! flushed at its deadline or when no more output can end it.
//!
//! Codex and Claude Code draw every frame inside a synchronized update. vte
//! holds the frame until the end marker, and records a 150 ms deadline after
//! which it should draw anyway, but nothing wakes it at that deadline: the
//! emulator's owner has to. These tests drive the deadline with hand-made
//! instants; nothing waits.

mod common;

use std::time::Duration;

use common::{emulator, feed, t0};
use farhelm_terminal_emulator::alacritty_terminal::grid::Dimensions;
use farhelm_terminal_emulator::{Collected, FeedMode};

const BEGIN: &[u8] = b"\x1b[?2026h";
const END: &[u8] = b"\x1b[?2026l";
const DEADLINE: Duration = Duration::from_millis(150);

/// An update whose end marker never comes is drawn at its deadline, not
/// before, and the emulator is clean afterwards.
///
/// Why: a program that dies or stalls mid-frame would otherwise leave the
/// screen frozen on the previous frame forever, while one that is merely
/// slow must not have half a frame drawn early. Spec:
/// `Emulator::sync_deadline`, `Emulator::flush_sync_if_due`.
#[test]
fn a_missing_end_marker_is_flushed_at_the_deadline() {
    let mut emulator = emulator(20, 3);
    let mut effects = Collected::default();
    emulator.feed(
        &[BEGIN, b"frame"].concat(),
        t0(),
        FeedMode::Live,
        &mut effects,
    );
    assert_eq!(emulator.screen_tail(100), "", "the frame is held");
    assert_eq!(emulator.sync_deadline(), Some(t0() + DEADLINE));
    assert!(
        !emulator.is_clean(),
        "an open update is not a clean boundary"
    );

    let just_before = t0() + DEADLINE - Duration::from_millis(1);
    assert!(!emulator.flush_sync_if_due(just_before, FeedMode::Live, &mut effects));
    assert_eq!(
        emulator.screen_tail(100),
        "",
        "still held one millisecond early"
    );

    assert!(emulator.flush_sync_if_due(t0() + DEADLINE, FeedMode::Live, &mut effects));
    assert_eq!(emulator.screen_tail(100), "frame");
    assert_eq!(emulator.sync_deadline(), None);
    assert!(emulator.is_clean());
}

/// When the program exits with an update open, flushing draws the partial
/// frame at once, without waiting for the deadline, and answers the queries
/// inside it.
///
/// Why: no more output can end the update, so waiting would only delay the
/// program's last frame. Spec: `Emulator::flush_sync`.
#[test]
fn a_child_exit_flushes_an_open_update() {
    let mut emulator = emulator(20, 3);
    feed(&mut emulator, &[BEGIN, b"last words\x1b[c"].concat());
    assert_eq!(emulator.screen_tail(100), "");

    let mut effects = Collected::default();
    emulator.flush_sync(FeedMode::Live, &mut effects);
    assert_eq!(emulator.screen_tail(100), "last words");
    assert_eq!(effects.replies, b"\x1b[?1;2c");
    assert!(emulator.is_clean());

    // Flushing with nothing open does nothing.
    let mut effects = Collected::default();
    emulator.flush_sync(FeedMode::Live, &mut effects);
    assert_eq!(effects, Collected::default());
}

/// Output that arrives after an open update's deadline flushes the update
/// first, even if the owner's wake-up has not happened yet.
///
/// Why: the deadline is a promise that the frame is drawn by then; a late
/// timer must not let the stalled update swallow newer output.
#[test]
fn output_after_the_deadline_flushes_the_update_first() {
    let mut emulator = emulator(20, 3);
    feed(&mut emulator, &[BEGIN, b"held"].concat());
    let mut effects = Collected::default();
    emulator.feed(b" after", t0() + DEADLINE * 2, FeedMode::Live, &mut effects);
    assert_eq!(emulator.screen_tail(100), "held after");
    assert_eq!(emulator.sync_deadline(), None);
}

/// An update that ends within the same output is drawn at once and leaves no
/// deadline.
///
/// Why: the common case for agents; it must not depend on any timer.
#[test]
fn a_complete_update_is_drawn_immediately() {
    let mut emulator = emulator(20, 3);
    feed(&mut emulator, &[BEGIN, b"whole", END].concat());
    assert_eq!(emulator.screen_tail(100), "whole");
    assert_eq!(emulator.sync_deadline(), None);
    assert!(emulator.is_clean());
}

/// A new update start inside an open update restarts the deadline from the
/// time of the output that contained it.
///
/// Why: vte extends an update this way, and the emulator measures the new
/// deadline on the caller's clock, as it does the first.
#[test]
fn a_restarted_update_moves_the_deadline() {
    let mut emulator = emulator(20, 3);
    feed(&mut emulator, &[BEGIN, b"one"].concat());
    let later = t0() + Duration::from_millis(100);
    let mut effects = Collected::default();
    emulator.feed(
        &[END, BEGIN, b"two"].concat(),
        later,
        FeedMode::Live,
        &mut effects,
    );
    assert_eq!(emulator.screen_tail(100), "one", "the first frame is drawn");
    assert_eq!(emulator.sync_deadline(), Some(later + DEADLINE));
}

/// A resize flushes an open update first: the held frame is drawn at the
/// old size, its queries are answered as the resize's mode allows, and no
/// update is left open.
///
/// Why: without it, a frame held across a resize would be drawn before the
/// resize live (where the deadline has usually passed) and after it in a
/// replay of the journal (where no time passes), and the rebuilt screen
/// would differ from the live one. Spec: `Emulator::resize`.
#[test]
fn a_resize_flushes_an_open_update_first() {
    for (mode, replies) in [
        (FeedMode::Live, b"\x1b[?1;2c".to_vec()),
        (FeedMode::ReplaySeen, vec![]),
    ] {
        let mut emulator = emulator(20, 3);
        feed(&mut emulator, &[BEGIN, b"\x1b[1;15HX\x1b[c"].concat());
        let mut effects = Collected::default();
        emulator.resize(
            farhelm_terminal_emulator::Size {
                columns: 10,
                lines: 3,
            },
            mode,
            &mut effects,
        );
        assert_eq!(effects.replies, replies, "{mode:?}");
        assert_eq!(emulator.sync_deadline(), None, "{mode:?}");
        assert!(emulator.is_clean(), "{mode:?}");
        // Drawn at column 15 of 20 and then reflowed: the row's first ten
        // (blank) cells move up into history and `X` lands at column 5,
        // rather than at the last of 10 columns (`         X`), where
        // drawing after the resize would have clamped it.
        assert_eq!(emulator.screen_text(), "    X\n\n\n", "{mode:?}");
        assert_eq!(emulator.term().grid().history_size(), 1, "{mode:?}");
    }
}

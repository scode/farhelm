//! Side effects by feed mode: what a query, a bell, a clipboard write
//! and a title change do in live output and in each kind of replay.
//!
//! A recovering supervisor rebuilds its emulator by feeding it output again.
//! Bytes a previous supervisor already handled must not answer the program a
//! second time or ring the bell again; bytes nobody handled must still get
//! their answers, because the program may be waiting, but not a bell or a
//! clipboard write that would arrive long after the fact.

mod common;

use common::{emulator, feed_in};
use farhelm_terminal_emulator::{ClipboardTarget, Collected, FeedMode, Notice};

/// A query, a bell, an OSC 52 write of "hi", and a title, in one write.
const EVERYTHING: &[u8] = b"\x1b[c\x07\x1b]52;c;aGk=\x07\x1b]2;working\x07";

/// Live output answers the query and reports the bell, the clipboard write
/// and the title change, in order.
///
/// Why: this is normal operation; every effect has somewhere to go.
/// Spec: `FeedMode::Live`.
#[test]
fn live_output_answers_and_reports_everything() {
    let mut emulator = emulator(80, 24);
    let effects = feed_in(&mut emulator, EVERYTHING, FeedMode::Live);
    assert_eq!(
        effects,
        Collected {
            replies: b"\x1b[?1;2c".to_vec(),
            notices: vec![
                Notice::Bell,
                Notice::Clipboard {
                    target: ClipboardTarget::Clipboard,
                    text: "hi".to_owned()
                },
                Notice::Title,
            ],
        }
    );
    assert_eq!(emulator.title(), "working");
}

/// Replaying output a previous supervisor already received answers nothing
/// and reports nothing, but still changes the terminal's state.
///
/// Why: the program got its answers and the user their bell the first time;
/// repeating either is the bug replay suppression exists to prevent. The
/// state must still change, or the rebuilt emulator would differ from the
/// one it replaces. Spec: `FeedMode::ReplaySeen`.
#[test]
fn replaying_seen_output_has_no_side_effects() {
    let mut emulator = emulator(80, 24);
    let effects = feed_in(
        &mut emulator,
        &[EVERYTHING, b"text\x1b[?2004h"].concat(),
        FeedMode::ReplaySeen,
    );
    assert_eq!(effects, Collected::default());
    assert_eq!(emulator.title(), "working");
    assert!(emulator.modes().bracketed_paste);
    assert_eq!(emulator.screen_tail(100), "text");
}

/// Replaying output no supervisor received answers its queries but drops
/// the bell, the clipboard write and the title notice.
///
/// Why: a program that asked during a supervisor's downtime may still be
/// waiting, while a bell or a clipboard change delivered minutes late would
/// only surprise the user. The title is read with `title()` once the replay
/// ends, never reported mid-replay. Spec: `FeedMode::ReplayUnseen`.
#[test]
fn replaying_unseen_output_answers_queries_only() {
    let mut emulator = emulator(80, 24);
    let effects = feed_in(&mut emulator, EVERYTHING, FeedMode::ReplayUnseen);
    assert_eq!(
        effects,
        Collected {
            replies: b"\x1b[?1;2c".to_vec(),
            notices: vec![],
        }
    );
    assert_eq!(emulator.title(), "working");
}

/// OSC 52 names its clipboard: `c` the clipboard, `p` and `s` the selection;
/// a payload that is not base64 of UTF-8 text is dropped.
///
/// Why: the clipboard write is handed on as text, and a malformed one must
/// not become a write of garbage. Spec: `Notice::Clipboard`.
#[test]
fn clipboard_writes_name_their_target() {
    let mut emulator = emulator(80, 24);
    let effects = feed_in(
        &mut emulator,
        b"\x1b]52;p;aGk=\x07\x1b]52;s;aGk=\x07\x1b]52;c;!!!\x07\x1b]52;c;/w==\x07",
        FeedMode::Live,
    );
    let selection = Notice::Clipboard {
        target: ClipboardTarget::Selection,
        text: "hi".to_owned(),
    };
    assert_eq!(effects.notices, vec![selection.clone(), selection]);
}

/// Title notices cover every way the title changes: set, set to empty, and
/// popped back from the title stack.
///
/// Why: a title set by popping (`CSI 23 t`) changes what status detection
/// reads just as much as an OSC 2 does.
#[test]
fn every_title_change_is_reported() {
    let mut emulator = emulator(80, 24);
    let effects = feed_in(
        &mut emulator,
        b"\x1b]2;first\x07\x1b[22t\x1b]0;second\x07\x1b[23t",
        FeedMode::Live,
    );
    assert_eq!(
        effects.notices,
        vec![Notice::Title, Notice::Title, Notice::Title]
    );
    assert_eq!(emulator.title(), "first");
    let effects = feed_in(&mut emulator, b"\x1b]2;\x07", FeedMode::Live);
    assert_eq!(effects.notices, vec![Notice::Title]);
    assert_eq!(emulator.title(), "");
}

/// A synchronized update's side effects happen when the update is drawn, in
/// the mode of the feed (or flush) that draws it.
///
/// Why: vte holds an update's bytes until it ends, then interprets them all
/// at once, so effects cannot follow the mode each byte arrived in. An
/// update that straddles the point where replay of seen output ends is
/// therefore treated as unseen: a query inside its seen part is answered
/// again. That duplicate is part of the delivery ambiguity accepted after a
/// supervisor crash; this test pins it so that a change is deliberate.
/// Spec: `FeedMode`.
#[test]
fn an_update_takes_the_mode_of_the_feed_that_ends_it() {
    let mut emulator = emulator(80, 24);
    let seen = feed_in(
        &mut emulator,
        b"\x1b[?2026h\x1b[c\x07",
        FeedMode::ReplaySeen,
    );
    assert_eq!(seen, Collected::default());
    let unseen = feed_in(&mut emulator, b"\x1b[?2026l", FeedMode::ReplayUnseen);
    assert_eq!(
        unseen,
        Collected {
            replies: b"\x1b[?1;2c".to_vec(),
            notices: vec![],
        }
    );
}

/// A full reset (RIS) that clears a title set in an earlier feed reports the
/// change live, reports nothing in either replay mode, and reports nothing
/// when there was no title to clear.
///
/// Why: alacritty clears the title on RIS without raising any event, so an
/// owner that publishes titles from notices would keep showing the old one.
/// The notice goes through the same mode policy as every other title change.
/// Spec: `QueryHandler::reset_state` in `src/queries.rs`, `Notice::Title`.
#[test]
fn a_reset_that_clears_the_title_is_reported_live_only() {
    for (mode, expected) in [
        (FeedMode::Live, vec![Notice::Title]),
        (FeedMode::ReplaySeen, vec![]),
        (FeedMode::ReplayUnseen, vec![]),
    ] {
        let mut emulator = emulator(80, 24);
        feed_in(&mut emulator, b"\x1b]2;working\x07", FeedMode::Live);
        let effects = feed_in(&mut emulator, b"\x1bc", mode);
        assert_eq!(effects.notices, expected, "{mode:?}");
        assert_eq!(emulator.title(), "", "{mode:?}");
    }

    let mut untitled = emulator(80, 24);
    assert_eq!(
        feed_in(&mut untitled, b"\x1bc", FeedMode::Live),
        Collected::default()
    );
}

//! Shared helpers for the emulator's integration tests.
//!
//! Every feed in these tests happens at one fixed instant unless a test
//! passes its own: nothing here waits, and the only clock that matters is the
//! one the synchronized-output tests step by hand.

#![allow(dead_code)]

pub mod compare;
pub mod describe;
pub mod output;
pub mod roundtrip;
pub mod synthetic;

use std::sync::OnceLock;
use std::time::Instant;

use farhelm_terminal_emulator::{Collected, Emulator, FeedMode, Size};

/// The instant every helper feed claims to happen at.
pub fn t0() -> Instant {
    static T0: OnceLock<Instant> = OnceLock::new();
    *T0.get_or_init(Instant::now)
}

/// An emulator of `columns` by `lines` with a small history.
pub fn emulator(columns: u16, lines: u16) -> Emulator {
    Emulator::new(Size { columns, lines }, 1000)
}

/// Feed `bytes` in `mode` and return everything the feed produced.
pub fn feed_in(emulator: &mut Emulator, bytes: &[u8], mode: FeedMode) -> Collected {
    let mut effects = Collected::default();
    emulator.feed(bytes, t0(), mode, &mut effects);
    effects
}

/// Feed `bytes` live and return everything the feed produced.
pub fn feed(emulator: &mut Emulator, bytes: &[u8]) -> Collected {
    feed_in(emulator, bytes, FeedMode::Live)
}

/// Resize `emulator` live to `columns` by `lines`, returning what flushing
/// an open synchronized update produced.
pub fn resize(emulator: &mut Emulator, columns: u16, lines: u16) -> Collected {
    let mut effects = Collected::default();
    emulator.resize(Size { columns, lines }, FeedMode::Live, &mut effects);
    effects
}

/// The answer a fresh 80x24 emulator gives to `query`, as text for readable
/// failures.
pub fn answer(query: &str) -> String {
    let mut emulator = emulator(80, 24);
    answer_from(&mut emulator, query)
}

/// The answer `emulator` gives to `query`.
pub fn answer_from(emulator: &mut Emulator, query: &str) -> String {
    String::from_utf8(feed(emulator, query.as_bytes()).replies).expect("answers are text")
}

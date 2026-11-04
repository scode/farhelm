//! Snapshot encode timings at the supervisor's full scrollback.
//!
//! Run in release through the test recorder with
//! `cargo nextest run -p farhelm-terminal-emulator --release --run-ignored only -E 'binary(encode_speed)' --no-capture`.
//! The one test measures its fixtures in sequence, so nextest does not time
//! them against one another. Compare runs on the same machine with its load
//! recorded; elapsed time is reported, never asserted. These are encoder
//! timings, not a measurement of a live terminal's input latency.

use std::fmt::Write;
use std::hint::black_box;
use std::time::{Duration, Instant};

use farhelm_terminal_emulator::alacritty_terminal::grid::Dimensions;
use farhelm_terminal_emulator::{Collected, Emulator, FeedMode, Size, Snapshot};

const COLUMNS: u16 = 200;
const LINES: u16 = 60;
const SCROLLBACK: usize = 12_000;
const MIB: usize = 1024 * 1024;
const SNAPSHOT_LIMIT: usize = 16 * MIB;
const SAMPLES: usize = 5;

/// Output shapes that separate ordinary cell scans, frequent pen changes,
/// and the history-trimming ladder. Their generators and geometry stay fixed
/// so an encoder change cannot silently make its benchmark easier.
#[derive(Clone, Copy, Debug)]
enum Fixture {
    Typical,
    TrueColor,
    ImageLike,
}

impl Fixture {
    /// A stable label for comparing the same fixture across recorded runs.
    fn name(self) -> &'static str {
        match self {
            Self::Typical => "typical",
            Self::TrueColor => "true-color",
            Self::ImageLike => "image-like",
        }
    }

    /// One physical row's output, without its newline. Colored fixtures use
    /// ASCII cells only, so 200 generated characters occupy exactly 200 columns.
    fn line(self, n: usize) -> String {
        match self {
            Self::Typical => {
                // Keep this copy of memory.rs's ordinary-output fixture here:
                // moving it into common would couple every test binary to a
                // benchmark that only this binary and the memory test need.
                let words = "the quick brown fox jumps over the lazy dog ".repeat(5);
                format!(
                    "\x1b[3{}m{n:06}\x1b[0m {}\x1b[1m中文\x1b[0m{}",
                    n % 8,
                    &words[..150],
                    &words[..36]
                )
            }
            Self::TrueColor => {
                let mut line = String::new();
                for column in (0..usize::from(COLUMNS)).step_by(5) {
                    let run = column / 5;
                    write!(
                        line,
                        "\x1b[38;2;{};{};{}m",
                        n % 256,
                        (run * 17) % 256,
                        (n / 256 + run * 7) % 256
                    )
                    .unwrap();
                    if run % 4 == 0 {
                        write!(
                            line,
                            "\x1b[48;2;{};{};{}m",
                            (n * 3 + run) % 256,
                            (run * 23) % 256,
                            (n / 256 + run * 11) % 256
                        )
                        .unwrap();
                    }
                    line.push_str("abcde");
                }
                line
            }
            Self::ImageLike => {
                let mut line = String::new();
                for column in 0..usize::from(COLUMNS) {
                    write!(
                        line,
                        "\x1b[38;2;{};{};{};48;2;{};{};{}mx",
                        (n * 31 + column * 17) % 256,
                        (n * 13 + column * 29) % 256,
                        (n * 7 + column * 43) % 256,
                        (n * 19 + column * 37) % 256,
                        (n * 23 + column * 11) % 256,
                        (n * 41 + column * 5) % 256
                    )
                    .unwrap();
                }
                line
            }
        }
    }

    /// Pin which part of encoding this fixture measures before collecting
    /// timings: ordinary and true-color output fit, while image-like output
    /// must trim history without dropping the screen or any titles.
    fn assert_shape(self, snapshot: &Snapshot) {
        let name = self.name();
        assert!(
            snapshot.blob.len() <= SNAPSHOT_LIMIT,
            "{name}: blob over limit"
        );
        assert!(!snapshot.dropped_screens, "{name}: screens dropped");
        assert_eq!(snapshot.dropped_titles, 0, "{name}: titles dropped");
        match self {
            Self::Typical => {
                assert_eq!(snapshot.dropped_history_lines, 0, "{name}: history trimmed");
                assert!(
                    snapshot.blob.len() > 2 * MIB,
                    "{name}: unexpectedly small blob"
                );
            }
            Self::TrueColor => {
                assert_eq!(snapshot.dropped_history_lines, 0, "{name}: history trimmed");
                assert!(
                    (11_000_000..15_000_000).contains(&snapshot.blob.len()),
                    "{name}: expected a blob near 12.6 MB, got {} bytes",
                    snapshot.blob.len()
                );
            }
            Self::ImageLike => {
                assert!(
                    snapshot.dropped_history_lines > 0,
                    "{name}: no history trimmed"
                );
                assert!(
                    snapshot.dropped_history_lines < SCROLLBACK,
                    "{name}: all history dropped"
                );
                assert!(
                    snapshot.blob.len() > SNAPSHOT_LIMIT - 64 * 1024,
                    "{name}: trimmed blob did not approach its limit"
                );
            }
        }
    }
}

/// Build the entire screen and history before timing anything, without a
/// second buffer holding the whole generated stream. Every feed shares one
/// instant: the fixture has no synchronized update or other clock stimulus.
fn filled(fixture: Fixture) -> Emulator {
    let mut emulator = Emulator::new(
        Size {
            columns: COLUMNS,
            lines: LINES,
        },
        SCROLLBACK,
    );
    let total_lines = SCROLLBACK + usize::from(LINES);
    let now = Instant::now();
    let mut read = Vec::with_capacity(128 * 1024);
    for n in 0..total_lines {
        read.extend_from_slice(fixture.line(n).as_bytes());
        read.extend_from_slice(b"\r\n");
        if read.len() >= 64 * 1024 || n + 1 == total_lines {
            emulator.feed(&read, now, FeedMode::Live, &mut Collected::default());
            read.clear();
        }
    }
    assert_eq!(
        emulator.term().grid().history_size(),
        SCROLLBACK,
        "{}: history not full",
        fixture.name()
    );
    assert!(
        emulator.is_clean(),
        "{}: unfinished sequence",
        fixture.name()
    );
    assert!(emulator.term().title_stack().is_empty());
    emulator
}

/// Time only the encode, using an untimed encode to establish the fixture's
/// shape and warm per-process setup. Byte comparisons happen after the timer
/// stops; a faster run that changes its result is refused, with bounded output.
fn measure(fixture: Fixture) {
    let emulator = filled(fixture);
    let reference = emulator.snapshot(SNAPSHOT_LIMIT).expect("clean fixture");
    fixture.assert_shape(&reference);

    let mut elapsed = [Duration::ZERO; SAMPLES];
    for duration in &mut elapsed {
        let started = Instant::now();
        let snapshot = black_box(&emulator)
            .snapshot(black_box(SNAPSHOT_LIMIT))
            .expect("clean fixture");
        *duration = started.elapsed();
        assert!(
            snapshot.blob == reference.blob,
            "{}: repeated encode changed its bytes",
            fixture.name()
        );
        assert_eq!(
            snapshot.dropped_history_lines,
            reference.dropped_history_lines
        );
        assert_eq!(snapshot.dropped_screens, reference.dropped_screens);
        assert_eq!(snapshot.dropped_titles, reference.dropped_titles);
    }
    elapsed.sort_unstable();
    println!(
        "{}: samples={SAMPLES}, median={:.6}s, min={:.6}s, max={:.6}s, spread={:.6}s, \
         blob_bytes={}, dropped_history_lines={}",
        fixture.name(),
        elapsed[SAMPLES / 2].as_secs_f64(),
        elapsed[0].as_secs_f64(),
        elapsed[SAMPLES - 1].as_secs_f64(),
        (elapsed[SAMPLES - 1] - elapsed[0]).as_secs_f64(),
        reference.blob.len(),
        reference.dropped_history_lines
    );
}

/// Compare fitting and over-limit encodes on fixed full-scrollback states.
/// This is an ignored measurement tool because machine load determines the
/// timings; fixture shape and repeated bytes, rather than speed, are asserted.
#[test]
#[ignore = "encoder benchmark; run in release with --no-capture"]
#[allow(clippy::assertions_on_constants)]
fn snapshot_encode_at_full_scrollback() {
    // Ordinary debug test discovery must still compile this ignored tool;
    // reject a debug measurement only when someone explicitly runs it.
    assert!(
        !cfg!(debug_assertions),
        "run in release so the timings describe the shipped encoder"
    );
    for fixture in [Fixture::Typical, Fixture::TrueColor, Fixture::ImageLike] {
        measure(fixture);
    }
}

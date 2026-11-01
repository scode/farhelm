//! Resident memory of one emulator at Farhelm's full scrollback, against the
//! per-terminal budget (crate docs, "Memory budget").
//!
//! Every supervisor-side terminal is one emulator in the shared supervisor,
//! so its worst case, times the number of terminals, is what one machine's
//! supervisor can be made to hold. These tests fill one emulator of 200
//! columns by 60 lines with 12,000 lines of history (tmux's history limit
//! today) with typical and with hostile output, and check its resident
//! growth against the budget. They also report the snapshot's size and the
//! time to take and restore it at that size.
//!
//! They are ignored, because they measure the test process: run each in a
//! process of its own, which nextest does, and in release mode, since the
//! hostile cases feed tens of megabytes:
//! `cargo nextest run -p farhelm-terminal-emulator --release --run-ignored only -E 'binary(memory)' --no-capture`.
//! Under libtest (`cargo test`) the tests share one process and the numbers
//! mean nothing. Linux only: they read `/proc/self/status`.

#![cfg(target_os = "linux")]

mod common;

use std::time::Instant;

use common::t0;
use farhelm_terminal_emulator::alacritty_terminal::grid::Dimensions;
use farhelm_terminal_emulator::alacritty_terminal::term::{
    MAX_EXTRA_STORAGE_BYTES, MAX_HYPERLINK_ID_BYTES, MAX_HYPERLINK_URI_BYTES,
};
use farhelm_terminal_emulator::{Collected, Emulator, FeedMode, Size, WidthTable};

const COLUMNS: u16 = 200;
const LINES: u16 = 60;
const SCROLLBACK: usize = 12_000;
const MIB: usize = 1024 * 1024;

/// The budget the crate docs state for ordinary output, in MiB.
const TYPICAL_BUDGET_MIB: usize = 80;
/// The threshold for one hostile case that gives cells extras of their own:
/// the grid (56 MiB) plus the vendored extra storage cap
/// (`MAX_EXTRA_STORAGE_BYTES`, 64 MiB) plus 8 MiB of slack. The crate docs'
/// bound for a terminal doing everything hostile at once is higher (about
/// 190 MiB); each case here does one thing.
const HOSTILE_BUDGET_MIB: usize = 128;
/// The most taking a snapshot may raise the process's peak: about twice
/// the 16 MiB blob (the stream and the fixups it is laid out from, then the
/// blob), plus 4 MiB for the allocator and the measuring passes' buffers.
const SNAPSHOT_PEAK_MIB: usize = 36;

/// This process's resident set, in bytes.
fn resident() -> usize {
    status_kib("VmRSS:") * 1024
}

/// The most this process has ever had resident, in bytes.
fn peak_resident() -> usize {
    status_kib("VmHWM:") * 1024
}

/// A `/proc/self/status` field given in kB.
fn status_kib(field: &str) -> usize {
    let status = std::fs::read_to_string("/proc/self/status").expect("/proc/self/status");
    let line = status
        .lines()
        .find(|line| line.starts_with(field))
        .unwrap_or_else(|| panic!("a {field} line"));
    line.split_whitespace()
        .nth(1)
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(|| panic!("{field} in kB"))
}

/// Fill a fresh emulator with `line(n)` for every line of the screen and
/// history, fed in reads of about 64 KiB as they are made (so that no
/// buffer of the whole output skews the measurement), and report its
/// resident growth and, when it ends clean, its snapshot and how far taking
/// it raised the process's peak. Returns the growth in bytes.
fn measure(name: &str, line: impl Fn(usize) -> String) -> Measured {
    // Resident growth means something only for a process that runs this test
    // alone, as nextest's process per test does; under libtest the tests
    // share a process and each other's allocations.
    assert_eq!(
        std::env::var("NEXTEST_EXECUTION_MODE").as_deref(),
        Ok("process-per-test"),
        "run under nextest, which gives each test a process of its own"
    );
    let total_lines = SCROLLBACK + usize::from(LINES);
    let before = resident();
    let mut emulator = Emulator::new(
        Size {
            columns: COLUMNS,
            lines: LINES,
        },
        SCROLLBACK,
    );
    let started = Instant::now();
    let mut read = Vec::with_capacity(128 * 1024);
    let mut output_len = 0;
    for n in 0..total_lines {
        read.extend_from_slice(line(n).as_bytes());
        read.extend_from_slice(b"\r\n");
        if read.len() >= 64 * 1024 || n + 1 == total_lines {
            emulator.feed(&read, t0(), FeedMode::Live, &mut Collected::default());
            output_len += read.len();
            read.clear();
        }
    }
    let fed = started.elapsed();
    drop(read);
    let growth = resident().saturating_sub(before);
    // The premise: the history is full, so the grid is the full scrollback.
    assert_eq!(
        emulator.term().grid().history_size(),
        SCROLLBACK,
        "{name}: history not full"
    );
    let extras = emulator.term().extra_storage().used();

    let titles = emulator.term().title_stack().len();
    let clean = emulator.is_clean();
    let peak_before = peak_resident().max(resident());
    let started = Instant::now();
    let mut snapshot_peak = None;
    let mut shape = None;
    let snapshot = match emulator.snapshot(16 * MIB) {
        Ok(snapshot) => {
            let encoded = started.elapsed();
            let peak = peak_resident().saturating_sub(peak_before);
            snapshot_peak = Some(peak);
            // The stream's length is the version 1 header's last field.
            let stream = u32::from_be_bytes(snapshot.blob[30..34].try_into().unwrap()) as usize;
            shape = Some(Shape {
                blob: snapshot.blob.len(),
                stream,
                dropped_history_lines: snapshot.dropped_history_lines,
            });
            let started = Instant::now();
            let restored = Emulator::restore(&snapshot.blob, SCROLLBACK).expect("restores");
            let decoded = started.elapsed();
            drop(restored);
            format!(
                "snapshot {:.2} MiB ({} history lines dropped) in {encoded:.2?}, \
                 raising the peak by {:.1} MiB; restored in {decoded:.2?}",
                snapshot.blob.len() as f64 / MIB as f64,
                snapshot.dropped_history_lines,
                peak as f64 / MIB as f64,
            )
        }
        Err(_) => "no snapshot (not at a clean boundary)".to_owned(),
    };
    println!(
        "{name}: {:.1} MiB resident for {} MiB of output fed in {fed:.2?}; \
         extra storage {:.1} MiB; {snapshot}",
        growth as f64 / MIB as f64,
        output_len / MIB,
        emulator.term().extra_storage().used() as f64 / MIB as f64,
    );
    Measured {
        growth,
        extras,
        titles,
        clean,
        snapshot_peak,
        shape,
    }
}

/// A snapshot's sizes: the whole blob, its stream, and how much history it
/// left out.
struct Shape {
    blob: usize,
    stream: usize,
    dropped_history_lines: usize,
}

/// What [`measure`] found: resident growth and the extra storage charged,
/// in bytes; the title stack's depth and whether the emulator ended clean;
/// and how far taking the snapshot raised the peak, when it was clean.
struct Measured {
    growth: usize,
    extras: usize,
    titles: usize,
    clean: bool,
    snapshot_peak: Option<usize>,
    shape: Option<Shape>,
}

/// Ordinary output: full lines of text, a few colors per line, now and then
/// a wide character.
///
/// Why: this is what the per-terminal budget for real use rests on: about
/// 24 bytes per cell (12,060 x 200 cells, about 58 MB) plus per-line
/// overhead. Spec: crate docs, "Memory budget".
#[test]
#[ignore = "measures the process; run alone, in release"]
fn typical_output_at_full_scrollback() {
    let Measured {
        growth,
        snapshot_peak,
        ..
    } = measure("typical", |n| {
        let words = "the quick brown fox jumps over the lazy dog ".repeat(5);
        format!(
            "\x1b[3{}m{n:06}\x1b[0m {}\x1b[1m中文\x1b[0m{}",
            n % 8,
            &words[..150],
            &words[..36]
        )
    });
    assert!(growth < TYPICAL_BUDGET_MIB * MIB, "{} MiB", growth / MIB);
    let peak = snapshot_peak.expect("a clean boundary");
    assert!(peak <= SNAPSHOT_PEAK_MIB * MIB, "{} MiB", peak / MIB);
}

/// Hostile: sixteen combining marks (the vendored cap) on every cell, and a
/// different underline color on every cell, the two ways to give each cell
/// its own allocated extra.
///
/// Why: without the extra storage cap these would give each of the 2.4
/// million cells its own allocation (387 MiB measured); with it, combining
/// marks are dropped and underline colors fall back to the default once the
/// cap is reached, and the terminal stays within the hostile budget.
#[test]
#[ignore = "measures the process; run alone, in release"]
fn combining_marks_and_underline_colors_on_every_cell() {
    let Measured { growth, extras, .. } = measure("combining and underline colors", |n| {
        let mut line = String::new();
        for column in 0..usize::from(COLUMNS) {
            line.push_str(&format!(
                "\x1b[58;2;{};{};{}m",
                n % 256,
                column,
                n / 256 % 256
            ));
            line.push('e');
            line.extend(std::iter::repeat_n('\u{301}', 16));
        }
        line
    });
    assert_full(extras);
    assert!(growth < HOSTILE_BUDGET_MIB * MIB, "{} MiB", growth / MIB);
}

/// Hostile: a new maximal hyperlink (URI and id at their caps) before every
/// character, which without the per-terminal extra storage cap would take
/// about 6 GB.
///
/// Why: the storage must stop at `MAX_EXTRA_STORAGE_BYTES` and the rest
/// print unlinked.
#[test]
#[ignore = "measures the process; run alone, in release"]
fn a_new_maximal_hyperlink_on_every_cell() {
    let uri = format!("http://{}", "u".repeat(MAX_HYPERLINK_URI_BYTES - 7));
    let Measured { growth, extras, .. } = measure("hyperlink per cell", |n| {
        let mut line = String::new();
        for column in 0..usize::from(COLUMNS) {
            // Maximal links until past the cap (about 25,000 fit), then
            // short ones: refused links cost nothing, and 2.4 million
            // maximal ones would be 5.6 GB of output to generate.
            let id = format!("{n:08}{column:04}");
            if n < 160 {
                let id = format!("{id:0>width$}", width = MAX_HYPERLINK_ID_BYTES);
                line.push_str(&format!("\x1b]8;id={id};{uri}\x1b\\x"));
            } else {
                line.push_str(&format!("\x1b]8;id={id};u\x1b\\x"));
            }
        }
        line
    });
    assert_full(extras);
    assert!(growth < HOSTILE_BUDGET_MIB * MIB, "{} MiB", growth / MIB);
}

/// Hostile: a saved cursor whose template holds an underline color and a
/// link, restored before every character and its color then removed, which
/// copies the extra the template shares with the cells already printed.
///
/// Why: a review found this loop growing the extra storage without bound
/// (122 MiB after 4,000 rows) before the storage checked such copies; it
/// must stay within the hostile budget like the others.
#[test]
#[ignore = "measures the process; run alone, in release"]
fn a_restored_saved_cursor_before_every_character() {
    let Measured { growth, extras, .. } = measure("saved cursor restored per cell", |n| {
        let mut line = String::new();
        if n == 0 {
            line.push_str("\x1b]8;id=keep;http://k/\x1b\\\x1b[58;5;3m\x1b7\x1b[59m\x1b]8;;\x1b\\");
        }
        for column in 1..=usize::from(COLUMNS) {
            line.push_str(&format!("\x1b8\x1b[{LINES};{column}H\x1b[59mx"));
        }
        line.push_str(&format!("\x1b[{LINES};1H"));
        line
    });
    assert_full(extras);
    assert!(growth < HOSTILE_BUDGET_MIB * MIB, "{} MiB", growth / MIB);
}

/// The hostile cases' premise: the output filled the extra storage (to
/// within 1 MiB: rows leaving history give extras back, and a long link
/// may not fit the last gap), so the measurement is of the storage at its
/// limit; and it stayed within it.
fn assert_full(extras: usize) {
    let slack = MIB;
    assert!(
        extras + slack >= MAX_EXTRA_STORAGE_BYTES,
        "only {} MiB of extras",
        extras / MIB
    );
    assert!(extras <= MAX_EXTRA_STORAGE_BYTES);
}

/// Hostile, outside the grid: a full title stack of maximal titles, then an
/// OSC left open at the payload limit, on top of typical output.
///
/// Why: these are the per-terminal allowances the vendored caps leave
/// outside the grid (34 MiB of payload, about 38 MiB resident: crate docs,
/// "Memory budget"), and they add to the grid's.
#[test]
#[ignore = "measures the process; run alone, in release"]
fn a_full_title_stack_and_an_open_osc() {
    let measured = measure("title stack and open OSC", |n| {
        let mut line = format!("{n:06} {}", "x".repeat(190));
        if n < 4096 {
            line.push_str(&format!("\x1b]2;{n:04}{}\x07\x1b[22t", "t".repeat(4092)));
        }
        if n == SCROLLBACK + usize::from(LINES) - 1 {
            line.push_str("\x1b]2;");
            line.push_str(&"o".repeat(10 * MIB - 8));
        }
        line
    });
    // The premises: the stack is full, and the last OSC is still open.
    assert_eq!(measured.titles, 4096);
    assert!(!measured.clean, "the OSC was ended");
    assert!(
        measured.growth < TYPICAL_BUDGET_MIB * MIB + 48 * MIB,
        "{} MiB",
        measured.growth / MIB
    );
}

/// A snapshot made mostly of cell fixups: a row of wide characters under
/// one underline color with every head erased alone, on every line, so
/// that every other cell is a fixup and the blob is cut to the 16 MiB
/// limit. Taking it raises the peak by at most about twice the blob.
///
/// Why: fixups used to be held as records beside the stream, at about twice
/// their encoded size, and a review measured this state raising the peak by
/// 40 MiB where the budget allows 32. Spec: crate docs, "Memory budget".
#[test]
#[ignore = "measures the process; run alone, in release"]
fn a_snapshot_of_fixups_stays_within_its_transient() {
    let measured = measure("orphaned wide-character halves", |_| {
        let mut line = String::from("\x1b[58;5;1m");
        line.push_str(&"中".repeat(usize::from(COLUMNS) / 2));
        line.push('\r');
        for column in (1..=usize::from(COLUMNS)).step_by(2) {
            line.push_str(&format!("\x1b[{column}G\x1b[X"));
        }
        line
    });
    // The premise: the blob is cut to the limit, and mostly fixups.
    let shape = measured.shape.expect("a clean boundary");
    assert!(shape.dropped_history_lines > 0);
    assert!(shape.blob > 16 * MIB - 64 * 1024, "{} bytes", shape.blob);
    assert!(
        shape.blob - shape.stream > 12 * MIB,
        "{} bytes of stream",
        shape.stream
    );
    let peak = measured.snapshot_peak.expect("a clean boundary");
    assert!(peak <= SNAPSHOT_PEAK_MIB * MIB, "{} MiB", peak / MIB);
}

/// Restoring a forged blob whose 16 MiB stream sets the title over and over
/// (`OSC 2`, seven bytes each, which the reading rules accept) raises the
/// process's peak by little beyond the blob.
///
/// Why: every title change queues an event until the feed that made it
/// returns, and a review measured the restore, then fed the whole stream at
/// once, raising the peak by about 83 MiB, outside the per-terminal budget
/// and for every terminal restored after a supervisor restart. The decoder
/// now feeds at most about 64 KiB at a time. Spec: the `v1` module's
/// "Reading".
#[test]
#[ignore = "measures the process; run alone, in release"]
fn restoring_a_stream_of_titles_queues_little() {
    assert_eq!(
        std::env::var("NEXTEST_EXECUTION_MODE").as_deref(),
        Ok("process-per-test"),
        "run under nextest, which gives each test a process of its own"
    );
    // The blob is built in place, in a buffer of exactly its size, so that
    // nothing freed before the measurement leaves room below the peak for the
    // restore's own growth to hide in.
    let title = b"\x1b]2;a\x1b\\";
    let count = 16 * MIB / title.len();
    let stream_len = count * title.len();
    let widths = WidthTable::current();
    let mut blob = Vec::with_capacity(34 + stream_len + 9);
    blob.extend_from_slice(b"FHTS\x00\x01\x00\x00\x01\x00\x01");
    blob.extend_from_slice(&[0; 8]);
    let (major, minor, update) = widths.unicode_version;
    blob.extend_from_slice(&[major, minor, update]);
    blob.extend_from_slice(&widths.fingerprint.to_be_bytes());
    blob.extend_from_slice(&(stream_len as u32).to_be_bytes());
    for _ in 0..count {
        blob.extend_from_slice(title);
    }
    blob.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(blob.len(), blob.capacity());

    // The premise: the process holds about all it ever has, so any growth
    // during the restore raises the peak.
    let peak_before = peak_resident();
    assert!(
        peak_before.saturating_sub(resident()) <= MIB,
        "{} MiB of headroom below the peak",
        peak_before.saturating_sub(resident()) / MIB
    );
    let started = Instant::now();
    let restored = Emulator::restore(&blob, SCROLLBACK).expect("the reading rules accept it");
    let decoded = started.elapsed();
    let peak = peak_resident().saturating_sub(peak_before);
    assert_eq!(
        restored.emulator.title(),
        "a",
        "the premise: the stream set titles"
    );
    println!(
        "16 MiB of titles: restored in {decoded:.2?}, raising the peak by {:.1} MiB",
        peak as f64 / MIB as f64
    );
    assert!(peak <= 8 * MIB, "{} MiB", peak / MIB);
}

//! The snapshot round trip: snapshot, restore into a fresh emulator, and the
//! two stay equal to an emulator that never stopped, through the rest of the
//! output, resizes and probes (`common::roundtrip` has the procedure).
//!
//! This is the acceptance bar for the snapshot format: a supervisor that
//! restarts rebuilds every host terminal this way, and any state the format
//! loses shows up as a terminal that behaves differently after the restart,
//! often only once the program relies on the lost state (a saved cursor, a
//! tab stop, the character REP repeats). Snapshots are taken only where the
//! emulator says it is clean after being fed up to a read's snapshot
//! candidate or a read's end, as the supervisor takes them. The workspace run
//! covers generated output, a one-column terminal and two short synthetic
//! recordings; the ignored tests run the snapshot spike's full configuration
//! and any private recordings.

mod common;

use std::path::Path;

use common::output::stream;
use common::roundtrip::{Recording, assert_round_trips, round_trip};
use farhelm_terminal_emulator::Size;
use proptest::prelude::*;

/// Read lengths for the recordings, cycled: a few sizes a pty read can have,
/// so that reads end everywhere, including inside sequences and updates.
const READS: &[usize] = &[4096, 1000, 37, 2500, 64, 9000, 1, 300, 4096, 700];

/// Shorter reads, for the short recordings, so that they have dozens of
/// reads to snapshot at.
const SHORT_READS: &[usize] = &[512, 37, 1000, 64, 1, 300, 700];

/// Two synthetic recordings of 30 KB, fed in reads of varied lengths, each
/// snapshotted at up to 48 of its clean points (read ends, split candidates,
/// right after resizes). Sized to take seconds in a debug build; the full
/// configuration is ignored below.
///
/// Why: the synthetic generator reaches the states that need fixups (wide
/// character halves, pending wraps away from the margin, saved cursors that
/// diverged, the REP character with no blank cell), which generated token
/// soup and real sessions rarely do. Spec: `snapshot` module docs, and the
/// `v1` module's layout.
#[test]
fn synthetic_recordings_round_trip() {
    for seed in [1, 5] {
        let recording = Recording::synthetic(seed, 30_000);
        assert_round_trips(
            &format!("synthetic seed {seed}"),
            &recording,
            SHORT_READS,
            48,
            11,
        );
    }
}

/// A one-column terminal, where wide characters are dropped, round-trips
/// like any other, through resizes to and from one column.
///
/// Why: one column is a size the terminal host accepts, and where the
/// vendored patches change alacritty's behavior (a wide character is
/// dropped, reflow to one column blanks wide characters); a restored
/// terminal must make the same choices. Spec: `Size`, and the round trip.
#[test]
fn a_one_column_terminal_round_trips() {
    let output = "ab中c\x1b[1m😀d\x1b[?2026he\x1b[?2026lf\r\n中\x1b[3b\x1b]8;id=a;http://a/\x1b\\g";
    let output = output.repeat(20).into_bytes();
    let one = Size {
        columns: 1,
        lines: 3,
    };
    let wide = Size {
        columns: 5,
        lines: 2,
    };
    let recording = Recording {
        sizes: vec![
            (0, one),
            (output.len() / 3, wide),
            (output.len() * 2 / 3, one),
        ],
        output,
    };
    assert_round_trips("one column", &recording, &[7, 1, 13], 1000, 3);
}

/// A terminal size for generated recordings, small enough that output
/// reaches the edges and scrolls: one column and one line included.
fn size() -> impl Strategy<Value = Size> {
    (1u16..40, 1u16..12).prop_map(|(columns, lines)| Size { columns, lines })
}

/// Generated output with a few resizes at arbitrary offsets (some at the
/// same offset), and the lengths of the reads it arrives in.
fn recording() -> impl Strategy<Value = (Recording, Vec<usize>)> {
    (
        stream(160),
        size(),
        prop::collection::vec((any::<prop::sample::Index>(), size()), 0..4),
        prop::collection::vec(1usize..48, 1..16),
    )
        .prop_map(|(output, initial, resizes, reads)| {
            let len = output.len();
            let mut sizes: Vec<(usize, Size)> = resizes
                .into_iter()
                .map(|(at, size)| (at.index(len + 1), size))
                .collect();
            sizes.sort_by_key(|(at, _)| *at);
            sizes.insert(0, (0, initial));
            (Recording { output, sizes }, reads)
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    /// Generated output with resizes, fed in reads of generated lengths,
    /// round-trips at up to 12 of its clean points.
    ///
    /// Why: the token generator shared with the other property tests mixes
    /// every sequence kind the emulator handles, cut and nested in ways no
    /// recording does, with update starts in every spelling vte accepts, at
    /// sizes down to a single cell. Spec: as above.
    #[test]
    fn generated_output_round_trips((recording, reads) in recording()) {
        let outcome = round_trip(&recording, &reads, 12, 7);
        prop_assert!(
            outcome.failures.is_empty(),
            "{} snapshot points failed; the first: {:#?}",
            outcome.failures.len(),
            outcome.failures.first()
        );
    }
}

/// The snapshot spike's full synthetic configuration, one test per
/// recording so that they run in parallel and each stays within nextest's
/// time limit: eight recordings of 400 KB (the spike's `fuzz-1` to `fuzz-8`),
/// fed in reads of varied lengths, up to 300 snapshot points each. Minutes
/// in all; run them when changing the encoder, in release.
macro_rules! full_size_round_trips {
    ($($name:ident: $seed:expr),* $(,)?) => {
        $(
            #[test]
            #[ignore = "minutes; the spike's full configuration"]
            fn $name() {
                let recording = Recording::synthetic($seed, 400_000);
                assert_round_trips(&format!("synthetic seed {}", $seed), &recording, READS, 300, 11);
            }
        )*
    };
}

full_size_round_trips! {
    full_size_round_trip_seed_1: 1,
    full_size_round_trip_seed_2: 2,
    full_size_round_trip_seed_3: 3,
    full_size_round_trip_seed_4: 4,
    full_size_round_trip_seed_5: 5,
    full_size_round_trip_seed_6: 6,
    full_size_round_trip_seed_7: 7,
    full_size_round_trip_seed_8: 8,
}

/// Recordings of real sessions that cannot be checked in (they hold whatever
/// the session showed), from the directories listed, separated by `:`, in
/// `FARHELM_SNAPSHOT_RECORDINGS`. Each directory holds `raw.bin`, the output,
/// and `events.jsonl`, one JSON object per size with `offset`, `cols` and
/// `rows` fields, the first at offset 0.
///
/// The variable is only read: name it to the test recorder with
/// `--keep-farhelm-env FARHELM_SNAPSHOT_RECORDINGS`.
#[test]
#[ignore = "needs private recordings"]
fn private_recordings_round_trip() {
    let Ok(dirs) = std::env::var("FARHELM_SNAPSHOT_RECORDINGS") else {
        println!("SKIPPED: FARHELM_SNAPSHOT_RECORDINGS names no recordings");
        return;
    };
    for dir in dirs.split(':').filter(|dir| !dir.is_empty()) {
        let recording = load(Path::new(dir));
        // 4 KiB and 1 KiB reads, as a pty gives agents' output, and odd ones.
        assert_round_trips(dir, &recording, &[4096], 300, 11);
        assert_round_trips(dir, &recording, &[1024], 300, 12);
        assert_round_trips(dir, &recording, READS, 300, 13);
    }
}

/// Read a recording directory (see `private_recordings_round_trip`).
fn load(dir: &Path) -> Recording {
    let output = std::fs::read(dir.join("raw.bin")).expect("raw.bin");
    let events = std::fs::read_to_string(dir.join("events.jsonl")).expect("events.jsonl");
    let field = |line: &str, name: &str| -> usize {
        let key = format!("\"{name}\":");
        let start = line
            .find(&key)
            .unwrap_or_else(|| panic!("no {name} in {line}"))
            + key.len();
        let digits: String = line[start..]
            .trim_start()
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        digits
            .parse()
            .unwrap_or_else(|_| panic!("bad {name} in {line}"))
    };
    let sizes = events
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let size = Size {
                columns: field(line, "cols") as u16,
                lines: field(line, "rows") as u16,
            };
            (field(line, "offset"), size)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        sizes.first().map(|(at, _)| *at),
        Some(0),
        "the first size is at offset 0"
    );
    Recording { output, sizes }
}

//! The snapshot round trip, as the goal's acceptance bar states it, in the
//! form the emulator's design takes: a recording is fed in reads, the way the
//! supervisor feeds the host's journal, and a snapshot is taken only where the
//! emulator itself says it is clean after being fed up to a point, those
//! points being each read's snapshot candidate (`snapshot_split_candidate`)
//! and each read's end. A fresh emulator restored from such a snapshot must
//! equal one that was fed the recording without interruption, field by field
//! (`compare`):
//!
//! - right after the restore;
//! - after both get the rest of the recording in the same reads, before any
//!   flush, and then after both get the same probes (queries, and output that
//!   exposes hidden state), whose answers must match too;
//! - after the restored one gets the rest in other reads (each stretch
//!   between resizes as one read) and both flush any open synchronized
//!   update, which is how a recovering supervisor ends a replay. Before that
//!   flush, reads cut differently may differ in whether an update is still
//!   open (`Emulator::feed`).
//!
//! Ported from the snapshot spike's harness. Every feed happens at one fixed
//! instant, so a synchronized update left open stays open until a probe, a
//! resize or a final flush ends it, the same way in every emulator.

use farhelm_terminal_emulator::{Collected, Emulator, FeedMode, Size, snapshot_split_candidate};

use super::compare::{Diff, diff};
use super::synthetic::Rng;
use super::t0;

/// The scrollback the round trips run with: Farhelm's own (tmux's history
/// limit today), so that history-heavy snapshots are what production sees.
pub const SCROLLBACK: usize = 12_000;

/// The size limit snapshots are taken with: the host's `MAX_SNAPSHOT`.
pub const MAX_SNAPSHOT: usize = 16 * 1024 * 1024;

/// Output and the sizes in force from given offsets: `(offset, size)`, the
/// first at offset 0 (the initial size), the rest in journal order. A resize
/// at offset `o` happens after byte `o - 1` and before byte `o`.
pub struct Recording {
    pub output: Vec<u8>,
    pub sizes: Vec<(usize, Size)>,
}

/// The output as escaped ASCII, exact to the byte, for a failing property
/// test's report.
impl std::fmt::Debug for Recording {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Recording")
            .field("output", &self.output.escape_ascii().to_string())
            .field("sizes", &self.sizes)
            .finish()
    }
}

impl Recording {
    /// A synthetic recording (`synthetic::generate`).
    pub fn synthetic(seed: u64, bytes: usize) -> Recording {
        let (output, sizes) = super::synthetic::generate(seed, bytes);
        Recording {
            output,
            sizes: sizes
                .into_iter()
                .map(|(offset, columns, lines)| (offset, Size { columns, lines }))
                .collect(),
        }
    }

    /// Whether the output holds a UTF-8-encoded C1 control (U+0080 to
    /// U+009F). Split across reads, upstream vte prints one rather than
    /// executing it, so that a later REP repeats it: the one known way the
    /// cut into reads changes the state even after a flush
    /// (`Emulator::feed`). The other-reads comparison skips such recordings.
    pub fn has_c1_controls(&self) -> bool {
        self.output
            .windows(2)
            .any(|pair| pair[0] == 0xc2 && (0x80..=0x9f).contains(&pair[1]))
    }
}

/// One thing the supervisor does to an emulator while following the
/// journal.
#[derive(Debug, Clone)]
enum Step {
    /// Feed `output[start..end]` as one call; whether it ends a read.
    Feed(usize, usize, bool),
    /// Resize.
    Resize(Size),
}

/// The steps that feed `recording` in reads of the given `lengths` (cycled),
/// each read ending at the next resize at the latest and fed in two calls
/// split at its snapshot candidate, the way the supervisor feeds a read when
/// a snapshot is due.
fn steps(recording: &Recording, lengths: &[usize]) -> Vec<Step> {
    let mut steps = Vec::new();
    let mut lengths = lengths.iter().copied().filter(|&length| length > 0).cycle();
    let mut start = 0;
    let mut resizes = recording.sizes[1..].iter().peekable();
    loop {
        while let Some(&&(at, size)) = resizes.peek() {
            if at > start {
                break;
            }
            steps.push(Step::Resize(size));
            resizes.next();
        }
        if start >= recording.output.len() {
            break;
        }
        let limit = resizes.peek().map_or(recording.output.len(), |(at, _)| *at);
        let end = (start + lengths.next().unwrap_or(4096)).min(limit);
        let read = &recording.output[start..end];
        let split = start + snapshot_split_candidate(read).unwrap_or(read.len());
        if start < split {
            steps.push(Step::Feed(start, split, split == end));
        }
        if split < end {
            steps.push(Step::Feed(split, end, true));
        }
        start = end;
    }
    steps
}

/// Apply one step, delivering whatever it makes the emulator do nowhere.
fn apply(emulator: &mut Emulator, recording: &Recording, step: &Step) {
    let mut effects = Collected::default();
    match *step {
        Step::Feed(from, to, _) => emulator.feed(
            &recording.output[from..to],
            t0(),
            FeedMode::Live,
            &mut effects,
        ),
        Step::Resize(size) => emulator.resize(size, FeedMode::Live, &mut effects),
    }
}

/// An emulator in the recording's initial state, before any step.
fn fresh(recording: &Recording) -> Emulator {
    Emulator::new(recording.sizes[0].1, SCROLLBACK)
}

/// An emulator after all of `steps`.
fn after(recording: &Recording, steps: &[Step]) -> Emulator {
    let mut emulator = fresh(recording);
    for step in steps {
        apply(&mut emulator, recording, step);
    }
    emulator
}

/// Queries, the answers to which must match: device attributes, status and
/// cursor position, sizes, the kitty keyboard query, DECRQM over the private
/// and ANSI modes, colors, a clipboard read.
const QUERIES: &str = concat!(
    "\x1b[c\x1b[>c\x1b[5n\x1b[6n\x1b[18t\x1b[14t\x1b[?u",
    "\x1b[?1$p\x1b[?3$p\x1b[?5$p\x1b[?6$p\x1b[?7$p\x1b[?12$p\x1b[?25$p\x1b[?47$p\x1b[?1000$p\x1b[?1002$p",
    "\x1b[?1003$p\x1b[?1004$p\x1b[?1005$p\x1b[?1006$p\x1b[?1007$p\x1b[?1042$p\x1b[?1049$p\x1b[?2004$p",
    "\x1b[?2026$p\x1b[4$p\x1b[20$p\x1b]10;?\x07\x1b]11;?\x07\x1b]12;?\x1b\\\x1b]52;c;?\x07",
    "\x1b]4;1;?\x07\x1b]4;200;?\x1b\\",
);

/// Output after the queries that exposes hidden state: saved cursors on both
/// screens, REP, tab stops, character sets, the scroll region, insert and
/// wrap modes, and scrollback reflow, with cursor reports between.
const CONTINUE: &str = concat!(
    "\x1b8R\x1b[3b\tT\tT\x1b[2Z<\x0eqx\x0fqx",
    "\r\nA long line that should wrap across the right margin at least once at narrow widths ...",
    "\x1bM\x1bM\x1b[S\x1b[T\x1b[2L\x1b[2M\x1bD\x1bE\x1b[6n",
    "\x1b[?1049h\x1b8A\x1b[6n\x1b[?1049l\x1b8B\x1b[6n\x1b[?1049h\x1b[?1049l\x1b[6n\x1b[?6$p",
    "\x1b[3S\x1b[6n\x1b[22t\x1b[23t\x1b]8;id=probe;http://probe\x1b\\link\x1b]8;;\x1b\\",
);

/// Probe `emulator` and return everything it answered and reported: flush
/// any open update (as the supervisor's deadline would), send the queries
/// and the continued output, then shrink, print, and grow back.
pub fn probe(emulator: &mut Emulator) -> Collected {
    let mut effects = Collected::default();
    emulator.flush_sync(FeedMode::Live, &mut effects);
    for chunk in [QUERIES, CONTINUE] {
        emulator.feed(chunk.as_bytes(), t0(), FeedMode::Live, &mut effects);
    }
    let size = emulator.size();
    let smaller = Size {
        columns: size.columns.saturating_sub(7).max(1),
        lines: size.lines.saturating_sub(3).max(1),
    };
    emulator.resize(smaller, FeedMode::Live, &mut effects);
    emulator.feed(b"\x1b[6nZ\r\nW\x1b[6n", t0(), FeedMode::Live, &mut effects);
    emulator.resize(size, FeedMode::Live, &mut effects);
    emulator.feed(b"\x1b[6n\x1b8Q\x1b[6n", t0(), FeedMode::Live, &mut effects);
    effects
}

/// Where a snapshot point lies: after which step, and of what kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Before any output.
    Start,
    /// At a read's snapshot candidate, inside the read.
    Candidate,
    /// At a read's end.
    ReadEnd,
    /// Right after a resize (which flushes an open update first).
    AfterResize,
}

/// What a round trip found wrong at one snapshot point.
#[derive(Debug)]
pub struct Failure {
    /// The point: after how many steps, at what offset, of what kind.
    pub after_steps: usize,
    pub offset: usize,
    pub kind: Kind,
    pub at_snapshot: Diff,
    pub after_tail_same_reads: Diff,
    pub after_probes: Diff,
    pub answers: Option<String>,
    pub after_tail_other_reads_and_flush: Option<Diff>,
}

/// What [`round_trip`] did.
#[derive(Debug, Default)]
pub struct Outcome {
    pub failures: Vec<Failure>,
    /// How many points were snapshotted, by kind.
    pub checked: Vec<(Kind, usize)>,
    /// How many clean points there were to choose from.
    pub clean_points: usize,
}

/// Round-trip `recording` fed in reads of `lengths`, at up to `limit` of
/// its clean points, chosen with `seed` (the first and last always among
/// them).
pub fn round_trip(recording: &Recording, lengths: &[usize], limit: usize, seed: u64) -> Outcome {
    let steps = steps(recording, lengths);
    // The output offset after each number of steps.
    let mut offsets = vec![0];
    for step in &steps {
        let last = *offsets.last().expect("starts with 0");
        offsets.push(match step {
            Step::Feed(_, to, _) => *to,
            Step::Resize(_) => last,
        });
    }
    let kind_after = |count: usize| match count.checked_sub(1).map(|index| &steps[index]) {
        None => Kind::Start,
        Some(Step::Resize(_)) => Kind::AfterResize,
        Some(Step::Feed(_, _, true)) => Kind::ReadEnd,
        Some(Step::Feed(_, _, false)) => Kind::Candidate,
    };

    // Which points are clean, asked of an emulator fed the steps.
    let mut probe_emulator = fresh(recording);
    let mut clean = vec![probe_emulator.is_clean()];
    for step in &steps {
        apply(&mut probe_emulator, recording, step);
        clean.push(probe_emulator.is_clean());
    }
    let clean_points: Vec<usize> = (0..clean.len()).filter(|&count| clean[count]).collect();
    let mut chosen = clean_points.clone();
    if chosen.len() > limit {
        let mut r = Rng::new(seed);
        let first = chosen[0];
        let last = *chosen.last().expect("at least the start is clean");
        let mut sample: Vec<usize> = (0..limit.saturating_sub(2))
            .map(|_| *r.pick(&clean_points))
            .collect();
        sample.extend([first, last]);
        sample.sort_unstable();
        sample.dedup();
        chosen = sample;
    }

    // The uninterrupted emulator: fed every step, then flushed, or probed.
    let reference = probe_emulator;
    let len = recording.output.len();
    debug_assert_eq!(offsets.last(), Some(&len));
    let mut probed = after(recording, &steps);
    let reference_answers = probe(&mut probed);
    let mut flushed = after(recording, &steps);
    flushed.flush_sync(FeedMode::Live, &mut Collected::default());
    let other_reads = !recording.has_c1_controls();

    let mut outcome = Outcome {
        clean_points: clean_points.len(),
        ..Outcome::default()
    };
    let mut live = fresh(recording);
    let mut done = 0;
    for &count in &chosen {
        for step in &steps[done..count] {
            apply(&mut live, recording, step);
        }
        done = count;
        let kind = kind_after(count);
        match outcome.checked.iter_mut().find(|(seen, _)| *seen == kind) {
            Some((_, n)) => *n += 1,
            None => outcome.checked.push((kind, 1)),
        }
        let blob = live
            .snapshot(MAX_SNAPSHOT)
            .expect("the point is clean")
            .blob;

        let mut same = Emulator::restore(&blob, SCROLLBACK)
            .expect("the blob restores")
            .emulator;
        let at_snapshot = diff(&live, &same);
        for step in &steps[count..] {
            apply(&mut same, recording, step);
        }
        let after_tail_same_reads = diff(&reference, &same);
        let answers = compare_answers(&reference_answers, &probe(&mut same));
        let after_probes = diff(&probed, &same);

        let after_tail_other_reads_and_flush = other_reads.then(|| {
            let mut other = Emulator::restore(&blob, SCROLLBACK)
                .expect("the blob restores")
                .emulator;
            // Each stretch between resizes as one read.
            let mut pending = offsets[count];
            let mut effects = Collected::default();
            for (index, step) in steps.iter().enumerate().skip(count) {
                if let Step::Resize(size) = *step {
                    let at = offsets[index];
                    other.feed(
                        &recording.output[pending..at],
                        t0(),
                        FeedMode::Live,
                        &mut effects,
                    );
                    pending = at;
                    other.resize(size, FeedMode::Live, &mut effects);
                }
            }
            other.feed(
                &recording.output[pending..],
                t0(),
                FeedMode::Live,
                &mut effects,
            );
            other.flush_sync(FeedMode::Live, &mut effects);
            diff(&flushed, &other)
        });

        let failed = !(at_snapshot.is_empty()
            && after_tail_same_reads.is_empty()
            && after_probes.is_empty()
            && answers.is_none()
            && after_tail_other_reads_and_flush
                .as_ref()
                .is_none_or(Diff::is_empty));
        if failed {
            outcome.failures.push(Failure {
                after_steps: count,
                offset: offsets[count],
                kind,
                at_snapshot,
                after_tail_same_reads,
                after_probes,
                answers,
                after_tail_other_reads_and_flush,
            });
        }
    }
    outcome
}

/// The first difference between two probes' effects, if any. Notices are
/// compared too: a title or clipboard write the probe causes must be the
/// same.
fn compare_answers(expected: &Collected, got: &Collected) -> Option<String> {
    if expected.replies != got.replies {
        return Some(format!(
            "answers {:?} vs {:?}",
            String::from_utf8_lossy(&expected.replies),
            String::from_utf8_lossy(&got.replies)
        ));
    }
    (expected.notices != got.notices)
        .then(|| format!("notices {:?} vs {:?}", expected.notices, got.notices))
}

/// Panic with a readable report when any round trip failed; print what was
/// checked.
pub fn assert_round_trips(
    name: &str,
    recording: &Recording,
    lengths: &[usize],
    limit: usize,
    seed: u64,
) {
    let outcome = round_trip(recording, lengths, limit, seed);
    println!(
        "{name}: {} bytes, {} clean points, checked {:?}",
        recording.output.len(),
        outcome.clean_points,
        outcome.checked
    );
    if outcome.failures.is_empty() {
        return;
    }
    let checked: usize = outcome.checked.iter().map(|(_, n)| n).sum();
    let shown: Vec<String> = outcome
        .failures
        .iter()
        .take(3)
        .map(|failure| format!("{failure:#?}"))
        .collect();
    panic!(
        "{name}: {} of {checked} snapshot points failed the round trip; the first:\n{}",
        outcome.failures.len(),
        shown.join("\n")
    );
}

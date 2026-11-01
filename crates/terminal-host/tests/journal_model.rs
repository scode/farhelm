//! Model-based property tests of the journal (SPEC.md, "Journal").
//!
//! The model keeps every event forever: each output byte and each resize, in
//! order. After every operation the journal must agree with it: what a read
//! returns is exactly the model's events after the read's position, except
//! where the journal says a gap replaced them, and the journal retains
//! exactly the records the model computes it must (the latest ones that fit
//! the cap, from the snapshot on), no fewer and no more. Reads come from
//! retained positions, which continue where they are, and from trimmed,
//! between-events and past-the-end positions, which must rebuild from the
//! stored snapshot. Random operation sequences, many offsets per sequence
//! and partial streams interleaved with new output are what find the
//! off-by-ones a hand-written case list would miss. Some operations print far more than one 64 KiB output block, and
//! some caps hold several blocks, so reads, trims and snapshots cross block
//! boundaries all the time.

use proptest::prelude::*;
use terminal_host::journal::{Cursor, Journal};
use terminal_host::protocol::{
    ChildExit, HostMessage, MAX_OUTPUT_CHUNK, MIN_JOURNAL_CAP, Position, RECORD_COST, Size,
    SnapshotRejection,
};

/// One event the model keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Event {
    Byte(u8),
    Resize(Size),
}

/// Everything that ever happened, plus the snapshot the journal should hold.
#[derive(Debug, Clone)]
struct Model {
    events: Vec<Event>,
    /// `positions[i]` is the journal position after the first `i` events;
    /// kept alongside so lookups stay linear in the test's total work.
    positions: Vec<Position>,
    /// `bytes[i]` is event `i`'s byte, 0 for a resize: with `positions`,
    /// which says whether a stretch holds a resize, a received `Output` is
    /// checked with one slice comparison rather than event by event.
    bytes: Vec<u8>,
    /// `costs[i]` is what the first `i` events cost against the cap (a byte
    /// 1, a resize `RECORD_COST`), so that what a stretch of events costs is
    /// one subtraction.
    costs: Vec<u64>,
    /// The journal's cap.
    cap: u64,
    /// The first event the journal must still retain, as the model computes
    /// it: never before the snapshot, and otherwise the earliest event from
    /// which everything up to the end fits the cap. It only ever moves
    /// forward, as trimming does.
    retained: usize,
    /// Index into `events` of the snapshot's position, and its blob.
    snapshot: Option<(usize, Vec<u8>)>,
    initial_snapshot: Option<Vec<u8>>,
    /// The size given at spawn.
    initial_size: Size,
    exit: Option<(usize, ChildExit)>,
}

impl Model {
    /// The journal position after the first `index` events.
    fn position(&self, index: usize) -> Position {
        self.positions[index]
    }

    /// Record one event. Call [`Model::trim`] after the operation's events.
    fn push(&mut self, event: Event) {
        let mut next = *self.positions.last().expect("starts with (0, 0)");
        let mut cost = *self.costs.last().expect("starts with 0");
        match event {
            Event::Byte(byte) => {
                next.offset += 1;
                cost += 1;
                self.bytes.push(byte);
            }
            Event::Resize(_) => {
                next.resizes += 1;
                cost += RECORD_COST;
                self.bytes.push(0);
            }
        }
        self.events.push(event);
        self.positions.push(next);
        self.costs.push(cost);
    }

    /// What the events from `from` to the end cost against the cap.
    fn cost_from(&self, from: usize) -> u64 {
        self.costs.last().expect("starts with 0") - self.costs[from]
    }

    /// Move the retained start forward as far as the cap requires after new
    /// events: to the earliest event from which the rest fits, if that is
    /// past where it is.
    fn trim(&mut self) {
        let total = *self.costs.last().expect("starts with 0");
        let fits = self.costs.partition_point(|&cost| total - cost > self.cap);
        self.retained = self.retained.max(fits);
    }

    /// The model boundary with exactly this position, if there is one.
    /// Positions grow in both coordinates, so this is a binary search.
    fn boundary(&self, position: Position) -> Option<usize> {
        let i = self
            .positions
            .partition_point(|p| (p.offset, p.resizes) < (position.offset, position.resizes));
        (self.positions.get(i) == Some(&position)).then_some(i)
    }

    /// The first model boundary the journal says it still retains.
    fn first_retained(&self, journal: &Journal) -> usize {
        let start = journal.retained_start();
        self.positions
            .partition_point(|p| p.offset < start.offset || p.resizes < start.resizes)
    }
}

#[derive(Debug, Clone)]
enum Op {
    Output(Vec<u8>),
    /// Output of this many bytes, from this seed: more than an output block
    /// holds. (A pattern rather than random bytes, which would make every
    /// case slow to generate and shrink; the byte values still differ.)
    BigOutput(usize, u8),
    Resize(u16, u16),
    /// Offer a snapshot at the event boundary this far (as a fraction of
    /// 1000) between the snapshot's position and the end.
    Snapshot(u16, Vec<u8>),
    /// Offer a snapshot at an arbitrary, probably invalid, position.
    BadSnapshot(u64, u64),
    Exit(i32),
    /// Advance the long-lived cursor by this many messages. Often only one
    /// or two, so that the stream stops between a snapshot record and what
    /// follows it, where a newer snapshot can overtake it.
    Stream(u8),
    /// Check a fresh read from the event boundary this far into the
    /// retained range.
    ReadFrom(u16),
    /// Check a fresh read from the event boundary this far (as a fraction of
    /// 1000) from the stored snapshot (or the start) to the retained start:
    /// a client whose own state was trimmed or replaced by the snapshot.
    ReadTrimmed(u16),
    /// Check a fresh read from a position this far (as fractions of 1000)
    /// across a little more than the offsets and resize indices there are:
    /// mostly not an event boundary at all, sometimes past the end.
    ReadAnywhere(u16, u16),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        6 => proptest::collection::vec(any::<u8>(), 1..3000).prop_map(Op::Output),
        1 => (MAX_OUTPUT_CHUNK / 2..2 * MAX_OUTPUT_CHUNK, any::<u8>())
            .prop_map(|(len, seed)| Op::BigOutput(len, seed)),
        2 => (1u16..400, 1u16..200).prop_map(|(c, r)| Op::Resize(c, r)),
        2 => (0u16..=1000, proptest::collection::vec(any::<u8>(), 0..64))
            .prop_map(|(at, blob)| Op::Snapshot(at, blob)),
        1 => (0u64..20_000, 0u64..40).prop_map(|(o, r)| Op::BadSnapshot(o, r)),
        1 => any::<i32>().prop_map(Op::Exit),
        3 => prop_oneof![1u8..3, any::<u8>()].prop_map(Op::Stream),
        2 => (0u16..=1000).prop_map(Op::ReadFrom),
        1 => (0u16..=1000).prop_map(Op::ReadTrimmed),
        1 => (0u16..=1000, 0u16..=1000).prop_map(|(o, r)| Op::ReadAnywhere(o, r)),
    ]
}

/// What a client rebuilt from a stream: the events it learned, in order,
/// with gaps, the exit and the snapshot it started from marked.
#[derive(Debug, Default)]
struct Received {
    /// The model index of the snapshot record that was sent, if any,
    /// checked against the model when it arrived.
    snapshot: Option<usize>,
    items: Vec<Item>,
    /// The position the next record must be at, as a client tracks it: set
    /// by the read's own position, a snapshot record or a gap, and advanced
    /// by each output byte and resize.
    position: Position,
}

impl Received {
    /// A receiver for a read from `position`.
    fn from(position: Position) -> Self {
        Self {
            position,
            ..Self::default()
        }
    }

    fn gaps(&self) -> usize {
        self.items
            .iter()
            .filter(|item| matches!(item, Item::Gap { .. }))
            .count()
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Item {
    /// One `Output`'s bytes, kept whole: a stream can carry hundreds of
    /// kilobytes, and an item per byte made the test slow.
    Output(Vec<u8>),
    Resize(Size),
    /// Events from one position to another were trimmed; the size and
    /// resize index in force after the gap.
    Gap {
        from: u64,
        to: u64,
        resizes: u64,
        size: Size,
    },
    /// The exit record, where it arrived among the events.
    Exit(u64, ChildExit),
}

/// Pull every message the cursor has now, checking each one's own claims as
/// it goes: every output and resize is at the position the receiver has
/// reached, and a gap starts there and ends exactly where the model says
/// retained records start at that moment.
fn drain(
    journal: &Journal,
    model: &Model,
    cursor: &mut Cursor,
    limit: Option<usize>,
    out: &mut Received,
) {
    let mut taken = 0;
    while limit.is_none_or(|limit| taken < limit) {
        let Some(message) = journal.next(cursor) else {
            break;
        };
        taken += 1;
        match message {
            HostMessage::SnapshotRecord { position, blob } => {
                assert!(
                    out.items.is_empty() && out.snapshot.is_none(),
                    "snapshot first, once"
                );
                // A stream sends the snapshot stored when it reaches that
                // step, which may be newer than when the read started.
                let (at, expected) =
                    stored_snapshot(model).expect("a snapshot record means the journal holds one");
                assert_eq!(
                    (position, blob),
                    (model.position(at), expected),
                    "the stored snapshot"
                );
                out.snapshot = Some(at);
                out.position = position;
            }
            HostMessage::Output { offset, data } => {
                assert!(!data.is_empty() && data.len() <= 64 * 1024);
                assert_eq!(
                    offset, out.position.offset,
                    "output at the receiver's offset"
                );
                out.position.offset += data.len() as u64;
                out.items.push(Item::Output(data));
            }
            HostMessage::Resized {
                offset,
                index,
                size,
            } => {
                assert_eq!(
                    (offset, index),
                    (out.position.offset, out.position.resizes),
                    "a resize at the receiver's position"
                );
                out.position.resizes += 1;
                out.items.push(Item::Resize(size));
            }
            HostMessage::Gap {
                from,
                to,
                resizes,
                size,
            } => {
                assert_eq!(
                    from, out.position.offset,
                    "a gap starts where the receiver is"
                );
                let resume = Position {
                    offset: to,
                    resizes,
                };
                assert_eq!(
                    resume,
                    model.position(model.retained),
                    "a gap ends where retained records start"
                );
                out.position = resume;
                out.items.push(Item::Gap {
                    from,
                    to,
                    resizes,
                    size,
                });
            }
            HostMessage::Exited { offset, exit } => {
                assert!(
                    offset <= out.position.offset,
                    "the exit follows what came before it"
                );
                out.items.push(Item::Exit(offset, exit));
            }
            other => panic!("unexpected stream message {other:?}"),
        }
    }
}

/// The snapshot the journal holds per the model: its model index and blob.
fn stored_snapshot(model: &Model) -> Option<(usize, Vec<u8>)> {
    model
        .snapshot
        .clone()
        .or_else(|| model.initial_snapshot.clone().map(|blob| (0, blob)))
}

/// Whether the journal must report a gap, per the model: something after
/// the stored snapshot (or after the start, without one) was trimmed.
///
/// `retained` never lies before the stored snapshot, and every event costs
/// something, so `retained` past the snapshot means exactly that. Kept
/// independent of `Journal::has_gap`, so a stale gap flag (one left set
/// after a snapshot that follows a trim, say) fails the model test instead
/// of agreeing with itself.
fn model_has_gap(model: &Model) -> bool {
    model.retained > stored_snapshot(model).map_or(0, |(at, _)| at)
}

/// Check that what a client received from a read starting at model index
/// `from` is the model's events after it, with gaps standing in only for
/// trimmed stretches, and the exit, once, exactly between the events before
/// its position and those after.
fn check_received(model: &Model, from: usize, received: &Received) {
    // Where the received events start in the model: after the snapshot when
    // one was sent (checked against the model as it arrived), else at the
    // read's own position.
    let mut index = received.snapshot.unwrap_or(from);
    let exit_at = model.exit.map(|(at, _)| at);
    let mut exit_seen = false;
    for item in &received.items {
        match item {
            Item::Output(data) => {
                let end = index + data.len();
                assert!(
                    end <= model.events.len()
                        && model.position(index).resizes == model.position(end).resizes
                        && model.bytes[index..end] == data[..],
                    "output at model index {index}..{end} is not the model's bytes there"
                );
                // Every byte on the right side of the exit: all at or after
                // it once it was sent, all before it otherwise.
                if let Some(at) = exit_at {
                    assert!(
                        if exit_seen {
                            index >= at
                        } else {
                            index + data.len() <= at
                        },
                        "output at {index}..{} is on the wrong side of the exit at {at}",
                        index + data.len()
                    );
                }
                index += data.len();
            }
            Item::Resize(size) => {
                assert_eq!(
                    model.events.get(index),
                    Some(&Event::Resize(*size)),
                    "event at model index {index}"
                );
                if let Some(at) = exit_at {
                    assert_eq!(
                        exit_seen,
                        index >= at,
                        "event {index} is on the right side of the exit at {at}"
                    );
                }
                index += 1;
            }
            Item::Exit(offset, exit) => {
                let (at, expected) = model.exit.expect("an exit record means the child exited");
                assert!(!exit_seen, "exit sent exactly once");
                assert!(index >= at, "the exit comes after the events before it");
                assert_eq!((*offset, *exit), (model.position(at).offset, expected));
                exit_seen = true;
            }
            Item::Gap {
                to, resizes, size, ..
            } => {
                // The gap ends at the model boundary with that position.
                let target = Position {
                    offset: *to,
                    resizes: *resizes,
                };
                let end = model
                    .boundary(target)
                    .expect("a gap ends at a model boundary");
                assert!(end >= index, "a gap moves forward");
                index = end;
                let in_force = model.events[..index]
                    .iter()
                    .rev()
                    .find_map(|event| match event {
                        Event::Resize(size) => Some(*size),
                        Event::Byte(_) => None,
                    });
                assert_eq!(
                    *size,
                    in_force.unwrap_or(model.initial_size),
                    "size in force after the gap"
                );
            }
        }
    }
    assert_eq!(index, model.events.len(), "the stream reaches the end");
    assert_eq!(
        exit_seen,
        model.exit.is_some(),
        "the exit is sent once it happened"
    );
}

/// Check a fresh read from `position` against SPEC.md, "Reading".
///
/// A position the retained tail continues from (an event boundary at or
/// after the retained start, other than the stored snapshot's own) gets
/// exactly the model's events after it, with no snapshot record and no gap.
/// Every other position (one whose events were trimmed or replaced by the
/// snapshot, one between events or past the end, and the stored snapshot's
/// own) gets a rebuild: the stored snapshot record if there is one, then a
/// gap from the snapshot's offset exactly when something was trimmed, then
/// everything after it. The receiver for a rebuild starts at the snapshot's
/// position (or (0, 0) without one), not at the read's, so a stream that
/// left out the snapshot record and gapped from the read's own position
/// fails here even when every record after it is right.
fn check_read(journal: &Journal, model: &Model, position: Position) -> Result<(), TestCaseError> {
    let stored = stored_snapshot(model).map(|(at, _)| at);
    let continues = model
        .boundary(position)
        .filter(|&index| index >= model.retained && Some(index) != stored);
    let from = continues.unwrap_or(stored.unwrap_or(0));
    let mut cursor = journal.read(position);
    let mut received = Received::from(model.position(from));
    drain(journal, model, &mut cursor, None, &mut received);
    check_received(model, from, &received);
    if continues.is_some() {
        prop_assert!(
            received.snapshot.is_none(),
            "no snapshot resent from {:?}",
            position
        );
        prop_assert_eq!(received.gaps(), 0, "nothing skipped from {:?}", position);
    } else {
        prop_assert_eq!(
            received.snapshot,
            stored,
            "a rebuild from {:?} starts with the stored snapshot",
            position
        );
        prop_assert_eq!(
            received.gaps(),
            usize::from(model_has_gap(model)),
            "a rebuild from {:?} gaps exactly when something was trimmed",
            position
        );
        if let Some(Item::Gap { from: gap_from, .. }) = received.items.first() {
            prop_assert_eq!(
                *gap_from,
                model.position(from).offset,
                "a rebuild's gap starts at the snapshot's offset"
            );
        }
    }
    Ok(())
}

/// The rejection reason SPEC.md's order of conditions gives for a snapshot
/// at `position` that the model knows to be invalid (its blob is never too
/// large here): before the stored snapshot, else outside retained output,
/// else inconsistent with the resizes.
fn expected_rejection(model: &Model, position: Position) -> SnapshotRejection {
    let snapshot = model
        .snapshot
        .as_ref()
        .map_or(Position::default(), |(at, _)| model.position(*at));
    let start = model.position(model.retained);
    let end = model.position(model.events.len());
    if position.offset < snapshot.offset || position.resizes < snapshot.resizes {
        SnapshotRejection::BeforeCurrent
    } else if position.offset < start.offset || position.offset > end.offset {
        SnapshotRejection::OutsideRetained
    } else {
        SnapshotRejection::InconsistentResizes
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 400, ..ProptestConfig::default() })]

    /// The journal agrees with a model that keeps everything, under random
    /// sequences of output, resizes, snapshots, exits and reads, with a cap
    /// small enough to force trimming.
    ///
    /// Why: the journal is the part of the host a client's recovery rests
    /// on, and it outlives every client version. Spec: after every
    /// operation the journal retains exactly what the model computes (from
    /// the snapshot on, the latest records that fit the cap) and its cost is
    /// theirs, never above the cap; a read from any retained position, and
    /// from nothing, returns exactly the model's events after it, with gaps
    /// only where output was trimmed; a long-lived stream interleaved with
    /// new output does the same; valid snapshots are accepted, and invalid
    /// ones rejected for the reason the order of SPEC.md's conditions gives,
    /// with nothing changed; a read from a trimmed position, from between
    /// events or past the end, and from nothing rebuilds from the stored
    /// snapshot record (see `check_read`); the exit is sent exactly once per
    /// read.
    #[test]
    fn the_journal_agrees_with_a_model_that_keeps_everything(
        cap_extra in prop_oneof![0u64..20_000, 60_000u64..200_000],
        initial in proptest::option::of(proptest::collection::vec(any::<u8>(), 0..32)),
        ops in proptest::collection::vec(op(), 1..60),
    ) {
        let start = Size { cols: 80, rows: 24 };
        let cap = MIN_JOURNAL_CAP / 16 + cap_extra; // can be below the spawn minimum: more trimming
        let mut journal = Journal::new(start, cap, initial.clone());
        let mut model = Model {
            events: Vec::new(),
            positions: vec![Position::default()],
            bytes: Vec::new(),
            costs: vec![0],
            cap,
            retained: 0,
            snapshot: None,
            initial_snapshot: initial,
            initial_size: start,
            exit: None,
        };
        let mut stream = journal.read(Position::default());
        let mut streamed = Received::default();

        for op in ops {
            match op {
                Op::Output(data) => {
                    journal.push_output(&data);
                    for byte in data {
                        model.push(Event::Byte(byte));
                    }
                    model.trim();
                }
                Op::BigOutput(len, seed) => {
                    let data: Vec<u8> = (0..len).map(|i| seed.wrapping_add(i as u8)).collect();
                    journal.push_output(&data);
                    for byte in data {
                        model.push(Event::Byte(byte));
                    }
                    model.trim();
                }
                Op::Resize(cols, rows) => {
                    let size = Size { cols, rows };
                    journal.push_resize(size);
                    model.push(Event::Resize(size));
                    model.trim();
                }
                Op::Snapshot(at, blob) => {
                    // Pick a model boundary inside what is retained.
                    let first = model.retained;
                    let span = model.events.len() - first;
                    let index = first + span * at as usize / 1000;
                    let position = model.position(index);
                    let result = journal.accept_snapshot(position, blob.clone());
                    prop_assert_eq!(result, Ok(()), "a boundary inside the retained tail is accepted");
                    model.snapshot = Some((index, blob));
                    model.retained = index;
                }
                Op::BadSnapshot(offset, resizes) => {
                    let position = Position { offset, resizes };
                    let before = (journal.snapshot_position(), journal.tail_start(), journal.cost());
                    let valid = model
                        .boundary(position)
                        .is_some_and(|i| i >= model.retained);
                    if !valid {
                        let result = journal.accept_snapshot(position, vec![1]);
                        prop_assert_eq!(
                            result,
                            Err(expected_rejection(&model, position)),
                            "{:?}", position
                        );
                        prop_assert_eq!(before, (journal.snapshot_position(), journal.tail_start(), journal.cost()));
                    }
                }
                Op::Exit(code) => {
                    if model.exit.is_none() {
                        model.exit = Some((model.events.len(), ChildExit::Code(code)));
                    }
                    journal.push_exit(ChildExit::Code(code));
                }
                Op::Stream(n) => drain(&journal, &model, &mut stream, Some(n as usize), &mut streamed),
                Op::ReadFrom(at) => {
                    let retained_from = model.retained;
                    let index = retained_from + (model.events.len() - retained_from) * at as usize / 1000;
                    check_read(&journal, &model, model.position(index))?;
                }
                Op::ReadTrimmed(at) => {
                    let base = stored_snapshot(&model).map_or(0, |(at, _)| at);
                    let index = base + (model.retained - base) * at as usize / 1000;
                    check_read(&journal, &model, model.position(index))?;
                }
                Op::ReadAnywhere(offset, resizes) => {
                    let end = model.position(model.events.len());
                    let position = Position {
                        offset: (end.offset + 2) * u64::from(offset) / 1000,
                        resizes: (end.resizes + 1) * u64::from(resizes) / 1000,
                    };
                    check_read(&journal, &model, position)?;
                }
            }
            prop_assert_eq!(
                model.first_retained(&journal),
                model.retained,
                "the journal retains exactly what the cap and the snapshot leave"
            );
            prop_assert_eq!(journal.cost(), model.cost_from(model.retained));
            prop_assert!(journal.cost() <= cap, "cost {} above cap {}", journal.cost(), cap);
            prop_assert_eq!(
                journal.has_gap(),
                model_has_gap(&model),
                "the journal reports a gap exactly when something after the snapshot was trimmed"
            );
        }

        // A client with nothing rebuilds everything that is left: the stored
        // snapshot, a gap exactly when something was trimmed, the rest, and
        // the exit once.
        // (`check_received`, which `check_read` calls, checks the exit.)
        check_read(&journal, &model, Position::default())?;

        // The long-lived stream, drained to the end, also agrees.
        drain(&journal, &model, &mut stream, None, &mut streamed);
        check_received(&model, 0, &streamed);
    }
}

/// A fixed sequence checked by hand, so a failure of the model test has a
/// readable neighbor.
///
/// Why: the property test proves agreement, but a reader needs one example
/// of the stream's exact shape. Spec: output, a resize at offset 3, more
/// output and an exit stream back in order, the resize first at its offset;
/// a snapshot at (3, 1) trims everything before it; a fresh read then gets
/// the snapshot and only what follows.
#[test]
fn a_small_journal_streams_and_trims_exactly() {
    let mut journal = Journal::new(Size { cols: 80, rows: 24 }, MIN_JOURNAL_CAP, None);
    journal.push_output(b"abc");
    journal.push_resize(Size {
        cols: 100,
        rows: 30,
    });
    journal.push_output(b"de");
    journal.push_exit(ChildExit::Signal(15));
    let mut cursor = journal.read(Position::default());
    let mut messages = Vec::new();
    while let Some(message) = journal.next(&mut cursor) {
        messages.push(message);
    }
    assert_eq!(
        messages,
        vec![
            HostMessage::Output {
                offset: 0,
                data: b"abc".to_vec()
            },
            HostMessage::Resized {
                offset: 3,
                index: 0,
                size: Size {
                    cols: 100,
                    rows: 30
                }
            },
            HostMessage::Output {
                offset: 3,
                data: b"de".to_vec()
            },
            HostMessage::Exited {
                offset: 5,
                exit: ChildExit::Signal(15)
            },
        ]
    );
    assert_eq!(
        journal.accept_snapshot(
            Position {
                offset: 4,
                resizes: 0
            },
            b"x".to_vec()
        ),
        Err(SnapshotRejection::InconsistentResizes),
        "a position past the resize at offset 3 must count it"
    );
    journal
        .accept_snapshot(
            Position {
                offset: 3,
                resizes: 1,
            },
            b"snap".to_vec(),
        )
        .expect("a consistent position inside the tail");
    let mut cursor = journal.read(Position::default());
    let mut messages = Vec::new();
    while let Some(message) = journal.next(&mut cursor) {
        messages.push(message);
    }
    assert_eq!(
        messages,
        vec![
            HostMessage::SnapshotRecord {
                position: Position {
                    offset: 3,
                    resizes: 1
                },
                blob: b"snap".to_vec()
            },
            HostMessage::Output {
                offset: 3,
                data: b"de".to_vec()
            },
            HostMessage::Exited {
                offset: 5,
                exit: ChildExit::Signal(15)
            },
        ]
    );
}

/// A snapshot over the size limit is rejected, and resizes alone are
/// trimmed when they exceed the cap.
///
/// Why: both are bounds a hostile or confused client could otherwise use to
/// grow the host without limit. Spec: a blob of `MAX_SNAPSHOT + 1` bytes is
/// rejected as too large; enough resizes with no output trim the oldest
/// resizes, leave the cost within the cap, and a read reports the gap with
/// the size in force after it; a cursor reports the position it reached.
#[test]
fn oversized_snapshots_and_resize_floods_are_bounded() {
    let mut journal = Journal::new(Size { cols: 80, rows: 24 }, 64 * 64, None);
    assert_eq!(
        journal.accept_snapshot(
            Position::default(),
            vec![0; terminal_host::protocol::MAX_SNAPSHOT + 1]
        ),
        Err(SnapshotRejection::TooLarge)
    );
    for rows in 1..=100u16 {
        journal.push_resize(Size { cols: 80, rows });
    }
    assert!(journal.cost() <= 64 * 64);
    assert!(journal.has_gap());
    let mut cursor = journal.read(Position::default());
    // The cap holds 64 resize records, so the first 36 were dropped: the
    // gap resumes at index 36 with the size the 36th resize (rows 36) set,
    // not the initial 80x24.
    let first = journal.next(&mut cursor).expect("a gap");
    assert_eq!(
        first,
        HostMessage::Gap {
            from: 0,
            to: 0,
            resizes: 36,
            size: Size { cols: 80, rows: 36 },
        }
    );
    while journal.next(&mut cursor).is_some() {}
    assert_eq!(
        cursor.position(),
        Position {
            offset: 0,
            resizes: 100
        }
    );
}

/// Every message a fresh cursor yields for a read from `position`.
fn read_all(journal: &Journal, position: Position) -> Vec<HostMessage> {
    let mut cursor = journal.read(position);
    std::iter::from_fn(|| journal.next(&mut cursor)).collect()
}

/// The accessors `Info` is built from report exactly what the journal
/// holds, and trimming cuts output from the front only as far as the cap
/// requires.
///
/// Why: a client deciding how to recover reads these numbers (SPEC.md,
/// "Host messages", `InfoReply`). The model test checks what is retained,
/// its cost and the gap flag after every operation, but not every accessor
/// on its own, and only by chance output that fills the cap exactly and
/// trims nothing; this case shows both in one readable place. Spec: a new
/// journal has no resizes, no gap, no cost, no exit and retained output
/// starting at 0; 100 bytes and two resizes cost 100 + 2 *
/// `RECORD_COST`; the exit is reported once journaled; a snapshot past both
/// resizes and all output is reported at its position and leaves a cost of
/// 0; output filling the cap exactly trims nothing, and 10 bytes more trim
/// exactly 10 bytes from the front.
#[test]
fn the_journal_reports_what_it_holds_and_trims_no_more_than_needed() {
    let mut journal = Journal::new(Size { cols: 80, rows: 24 }, MIN_JOURNAL_CAP, None);
    assert_eq!(journal.resize_count(), 0);
    assert_eq!(journal.tail_start(), 0);
    assert!(!journal.has_gap());
    assert_eq!(journal.cost(), 0);
    assert_eq!(journal.exit(), None);
    journal.push_output(&[b'a'; 100]);
    journal.push_resize(Size {
        cols: 100,
        rows: 30,
    });
    journal.push_resize(Size {
        cols: 120,
        rows: 40,
    });
    assert_eq!(journal.resize_count(), 2);
    assert_eq!(journal.cost(), 100 + 2 * RECORD_COST);
    journal.push_exit(ChildExit::Code(7));
    assert_eq!(journal.exit(), Some(ChildExit::Code(7)));
    journal
        .accept_snapshot(
            Position {
                offset: 100,
                resizes: 2,
            },
            Vec::new(),
        )
        .unwrap();
    assert_eq!(
        journal.cost(),
        0,
        "the snapshot dropped everything before it"
    );
    assert_eq!(journal.tail_start(), 100);
    assert_eq!(
        journal.snapshot_position(),
        Position {
            offset: 100,
            resizes: 2
        }
    );
    journal.push_output(&vec![b'b'; MIN_JOURNAL_CAP as usize]);
    assert_eq!(journal.cost(), MIN_JOURNAL_CAP);
    assert!(!journal.has_gap(), "output filling the cap exactly is kept");
    journal.push_output(&[b'c'; 10]);
    assert!(journal.has_gap());
    assert_eq!(journal.tail_start(), 110, "only the 10 bytes over the cap");
    assert_eq!(journal.cost(), MIN_JOURNAL_CAP);
}

/// A snapshot behind the current one in either coordinate is rejected as
/// such.
///
/// Why: the reason code is part of the protocol (SPEC.md,
/// `SnapshotRejected`), and a client acts on it: "before current" means its
/// snapshot is stale, not that it lost track of the tail. Spec: with a
/// snapshot at (10, 1), a snapshot at (5, 1) and one at (12, 0) are both
/// rejected as before the current snapshot.
#[test]
fn a_snapshot_behind_the_current_one_in_either_coordinate_is_before_current() {
    let mut journal = Journal::new(Size { cols: 80, rows: 24 }, MIN_JOURNAL_CAP, None);
    journal.push_output(b"0123456789");
    journal.push_resize(Size {
        cols: 100,
        rows: 30,
    });
    journal.push_output(b"abcdef");
    journal
        .accept_snapshot(
            Position {
                offset: 10,
                resizes: 1,
            },
            b"s".to_vec(),
        )
        .unwrap();
    for (offset, resizes) in [(5, 1), (12, 0)] {
        assert_eq!(
            journal.accept_snapshot(Position { offset, resizes }, b"t".to_vec()),
            Err(SnapshotRejection::BeforeCurrent),
            "({offset}, {resizes})"
        );
    }
}

/// A snapshot whose offset lies outside retained output is rejected as
/// such, ahead of an inconsistent resize index.
///
/// Why: the reason code is part of the protocol (SPEC.md, `SnapshotRejected`,
/// and "Snapshots": the first condition that fails gives the reason), and a
/// client acts on it: "outside retained output" means what it snapshotted
/// was trimmed or has not happened, where "inconsistent" means it lost
/// count of the resizes. Spec: with a 64 KiB cap, after 10 bytes, a resize
/// and a cap's worth more output (so the first 10 bytes and the resize are
/// trimmed), a snapshot at (5, 0) (before retained output, with a resize
/// index that is also inconsistent) and one past the end are both rejected
/// as outside retained output, and one at the end with resize index 0 as
/// inconsistent.
#[test]
fn a_snapshot_outside_retained_output_is_rejected_as_such_first() {
    let mut journal = Journal::new(Size { cols: 80, rows: 24 }, MIN_JOURNAL_CAP, None);
    journal.push_output(b"0123456789");
    journal.push_resize(Size {
        cols: 100,
        rows: 30,
    });
    journal.push_output(&vec![b'x'; MIN_JOURNAL_CAP as usize]);
    assert!(
        journal.tail_start() == 10 && journal.retained_start().resizes == 1,
        "premise: the first bytes and the resize were trimmed"
    );
    let end = journal.end();
    for (offset, resizes, reason) in [
        (5, 0, SnapshotRejection::OutsideRetained),
        (end + 1, 1, SnapshotRejection::OutsideRetained),
        (end, 0, SnapshotRejection::InconsistentResizes),
    ] {
        assert_eq!(
            journal.accept_snapshot(Position { offset, resizes }, b"s".to_vec()),
            Err(reason),
            "({offset}, {resizes})"
        );
    }
}

/// A stream that sent a snapshot record and finds a newer snapshot accepted
/// before it went on sends a gap to the new snapshot's position, even with
/// nothing trimmed.
///
/// Why: what lay between the snapshot the client has and the one now
/// stored is gone from the journal, and a client told nothing would take
/// the records after the new snapshot as continuing from its old one
/// (SPEC.md, "Reading"). Trimming is not what makes the gap here, so this
/// pins the case apart from it. Spec: with an initial snapshot and output
/// "abcdef", a read from (0, 0) yields the snapshot record; a snapshot at
/// (4, 0) is then accepted; the stream continues with a gap from 0 to
/// (4, 0) at the initial size, then output "ef" at 4.
#[test]
fn a_stream_overtaken_by_a_newer_snapshot_sends_a_gap() {
    let size = Size { cols: 80, rows: 24 };
    let mut journal = Journal::new(size, MIN_JOURNAL_CAP, Some(b"initial".to_vec()));
    journal.push_output(b"abcdef");
    let mut cursor = journal.read(Position::default());
    assert!(matches!(
        journal.next(&mut cursor),
        Some(HostMessage::SnapshotRecord { .. })
    ));
    let newer = Position {
        offset: 4,
        resizes: 0,
    };
    journal.accept_snapshot(newer, b"newer".to_vec()).unwrap();
    assert!(!journal.has_gap(), "premise: nothing was trimmed");
    let rest: Vec<HostMessage> = std::iter::from_fn(|| journal.next(&mut cursor)).collect();
    assert_eq!(
        rest,
        vec![
            HostMessage::Gap {
                from: 0,
                to: 4,
                resizes: 0,
                size
            },
            HostMessage::Output {
                offset: 4,
                data: b"ef".to_vec()
            },
        ]
    );
}

/// A read at the stored snapshot's position resends the snapshot, and a
/// read after it within the tail does not.
///
/// Why: a client continuing from where it is must get only what follows,
/// while one starting from the snapshot's position (a client with nothing
/// starts at (0, 0), the initial snapshot's position) must get the
/// snapshot (SPEC.md, "Reading"). Spec: with an initial snapshot and output
/// "abc", a read from (1, 0) yields only output "bc" at 1; a read from
/// (0, 0) yields the snapshot record and then "abc".
#[test]
fn a_read_at_the_snapshot_resends_it_and_one_after_it_does_not() {
    let mut journal = Journal::new(
        Size { cols: 80, rows: 24 },
        MIN_JOURNAL_CAP,
        Some(b"initial".to_vec()),
    );
    journal.push_output(b"abc");
    assert_eq!(
        read_all(
            &journal,
            Position {
                offset: 1,
                resizes: 0
            }
        ),
        vec![HostMessage::Output {
            offset: 1,
            data: b"bc".to_vec()
        }]
    );
    assert_eq!(
        read_all(&journal, Position::default()),
        vec![
            HostMessage::SnapshotRecord {
                position: Position::default(),
                blob: b"initial".to_vec()
            },
            HostMessage::Output {
                offset: 0,
                data: b"abc".to_vec()
            },
        ]
    );
}

/// A snapshot of exactly `MAX_SNAPSHOT` bytes is accepted.
///
/// Why: the limit is part of the permanent contract (SPEC.md, "Resource
/// bounds"); a client sizes its snapshots to it, and one at the limit that a
/// host refused would leave that client unable to trim the journal at all.
/// The test beside it covers one byte more. Spec: a blob of `MAX_SNAPSHOT`
/// bytes at (0, 0) is accepted.
#[test]
fn a_snapshot_at_the_size_limit_is_accepted() {
    let mut journal = Journal::new(Size { cols: 80, rows: 24 }, MIN_JOURNAL_CAP, None);
    assert_eq!(
        journal.accept_snapshot(
            Position::default(),
            vec![0; terminal_host::protocol::MAX_SNAPSHOT]
        ),
        Ok(())
    );
}

/// A run of resizes at one offset can be split: a snapshot and a read may
/// sit before, between or after them, and each sees exactly the resizes on
/// its side.
///
/// Why: resizes with no output between them are normal (a window dragged
/// across sizes), and a position between two of them is valid; reading the
/// rule any other way would make two implementations disagree on which
/// snapshots to accept. Spec: SPEC.md, "Snapshots", condition 4: with two
/// resizes at offset 2, (2, 0), (2, 1) and (2, 2) are all consistent; a
/// read from (2, 1) gets only the second resize and the output after it; a
/// snapshot at (2, 1) is accepted and a fresh read then gets the snapshot,
/// the second resize and the output.
#[test]
fn a_run_of_resizes_at_one_offset_can_be_split() {
    let mut journal = Journal::new(Size { cols: 80, rows: 24 }, MIN_JOURNAL_CAP, None);
    journal.push_output(b"ab");
    let first = Size { cols: 90, rows: 30 };
    let second = Size {
        cols: 100,
        rows: 40,
    };
    journal.push_resize(first);
    journal.push_resize(second);
    journal.push_output(b"c");
    let between = Position {
        offset: 2,
        resizes: 1,
    };
    assert_eq!(
        read_all(&journal, between),
        vec![
            HostMessage::Resized {
                offset: 2,
                index: 1,
                size: second
            },
            HostMessage::Output {
                offset: 2,
                data: b"c".to_vec()
            },
        ]
    );
    journal
        .accept_snapshot(between, b"s".to_vec())
        .expect("a position between resizes at one offset is consistent");
    assert_eq!(
        read_all(&journal, Position::default()),
        vec![
            HostMessage::SnapshotRecord {
                position: between,
                blob: b"s".to_vec()
            },
            HostMessage::Resized {
                offset: 2,
                index: 1,
                size: second
            },
            HostMessage::Output {
                offset: 2,
                data: b"c".to_vec()
            },
        ]
    );
}

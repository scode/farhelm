//! Checkpoints: when a host terminal's runtime snapshots its emulator for
//! the host, and what it remembers about the snapshots it took.
//!
//! The host drops raw output only when it accepts a snapshot, and trims the
//! oldest output once what it keeps since the latest one passes its journal
//! cap (`crates/terminal-host/SPEC.md`, "Journal"). Checkpoints are what
//! keep that tail short: the shorter it is, the less a recovering
//! supervisor replays, and the more of the cap is left for output written
//! while no supervisor is attached before anything is lost to a gap.
//!
//! A checkpoint is due once enough output has passed since the previous
//! attempt ([`CheckpointPolicy`]; the plan's D-8, revised): four times the
//! latest snapshot's size, at least [`CHECKPOINT_MIN_BYTES`] and at most a
//! quarter of the journal cap. Scaling with the snapshot keeps the encoding
//! cost a fixed share of the output, whatever the terminal holds; the floor
//! keeps a nearly empty terminal from encoding every few kilobytes; the
//! ceiling keeps a connected supervisor's tail far below the cap. Nothing
//! makes a checkpoint due by time: a terminal that writes a little now and
//! then has a short tail that replays in milliseconds, and a timer would
//! re-encode its whole scrollback for it.
//!
//! A due checkpoint is taken only where the emulator is clean
//! (`Emulator::is_clean`): within a read, after its split candidate
//! (`farhelm_terminal_emulator::snapshot_split_candidate`) or at its end;
//! between reads, after a synchronized update's deadline draws it, or where
//! the replay finishes. Where it is never clean (a program that stopped
//! mid-sequence and went silent, or one sequence longer than the cap), no
//! checkpoint is taken and the host keeps the previous snapshot and the
//! tail after it, which may then gap.
//!
//! An orderly stop uses a separate threshold: a short tail is left for
//! recovery to replay instead of encoding the whole terminal again. The
//! supervisor checks before admitting a host to its encode slots, and the
//! runtime checks again because an automatic encode may finish meanwhile.

use terminal_host::protocol::{Position, SnapshotRejection};

use super::super::client::ClientError;
use super::super::spawn::DEFAULT_JOURNAL_CAP;

/// The least output that makes a checkpoint due, however small the latest
/// snapshot: a fresh terminal's snapshot is a few kilobytes, and encoding
/// it every few times that would buy nothing a replay of 1 MiB does not.
pub const CHECKPOINT_MIN_BYTES: u64 = 1024 * 1024;

/// How many times the latest snapshot's size of output makes a checkpoint
/// due (within the policy's bounds). Encoding costs roughly in proportion
/// to the snapshot, so this bounds checkpoints' share of the work spent on
/// output.
const SNAPSHOTS_PER_CHECKPOINT: u64 = 4;

/// The bounds on how much output makes a checkpoint due: between
/// `min_bytes` and `max_bytes`, four times the latest snapshot's size
/// ([`Self::for_journal_cap`] gives the production bounds; tests set both
/// to one small value). `stop_min_bytes` independently decides whether an
/// orderly stop saves a new snapshot or leaves a short tail for recovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckpointPolicy {
    /// The floor ([`CHECKPOINT_MIN_BYTES`]).
    pub min_bytes: u64,
    /// The ceiling: a quarter of the host's journal cap. Wins over the
    /// floor when the cap is below four times it.
    pub max_bytes: u64,
    /// The least tail an orderly stop saves with a new snapshot. Smaller
    /// tails are left for recovery to replay; zero asks for every change.
    /// Separate from `min_bytes` so a forced stop checkpoint need not make
    /// automatic checkpoints run for every few bytes as well.
    pub stop_min_bytes: u64,
}

impl CheckpointPolicy {
    /// The policy for a host whose journal cap is `journal_cap`.
    pub fn for_journal_cap(journal_cap: u64) -> Self {
        Self {
            min_bytes: CHECKPOINT_MIN_BYTES,
            max_bytes: journal_cap / 4,
            stop_min_bytes: CHECKPOINT_MIN_BYTES.min(journal_cap / 4),
        }
    }

    /// The processed output a stop should checkpoint, or `None` when a
    /// short replay is cheaper than encoding the whole terminal again.
    /// Resize records carry no output bytes, and an initial seed at (0, 0)
    /// counts as no saved output. Both stop admission and the runtime use
    /// this rule: an automatic checkpoint can shorten the tail while the
    /// stop is waiting for a slot.
    pub(super) fn exit_tail(&self, position: Position, accepted: Option<Position>) -> Option<u64> {
        let tail = position
            .offset
            .saturating_sub(accepted.map_or(0, |at| at.offset));
        (tail >= self.stop_min_bytes).then_some(tail)
    }

    /// The output since the previous attempt that makes a checkpoint due,
    /// given the latest snapshot's size (`None` before any).
    fn after_bytes(&self, latest_snapshot: Option<u64>) -> u64 {
        latest_snapshot
            .map_or(0, |len| len.saturating_mul(SNAPSHOTS_PER_CHECKPOINT))
            .max(self.min_bytes)
            .min(self.max_bytes)
    }
}

/// With [`DEFAULT_JOURNAL_CAP`], automatic checkpoints need 1 MiB to 8 MiB
/// of output and an orderly stop saves tails of at least 1 MiB.
impl Default for CheckpointPolicy {
    fn default() -> Self {
        Self::for_journal_cap(DEFAULT_JOURNAL_CAP)
    }
}

/// What [`super::HostTerminal::checkpoint_now`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckpointOutcome {
    /// The host accepted a snapshot at this position: a recovery replays
    /// only what follows it.
    Accepted(Position),
    /// The host already holds a snapshot this runtime took at this
    /// position, which is where the terminal is: nothing new to take.
    Current(Position),
    /// The processed output since the latest known snapshot was small
    /// enough to replay cheaply. Deliberately leaving that tail is a
    /// successful stop, not a checkpoint that failed to finish.
    Skipped { tail: u64 },
    /// No new snapshot: the host keeps its previous one, if any, and the
    /// tail after it.
    Kept(NotTaken),
}

/// Why [`CheckpointOutcome::Kept`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotTaken {
    /// The emulator is not at a clean boundary (a sequence or a
    /// synchronized update is open).
    NotClean,
    /// The runtime is still replaying what the host journaled before it
    /// adopted the host.
    Replaying,
    /// The host refused the snapshot.
    Rejected(SnapshotRejection),
    /// The deadline passed before the snapshot was encoded or the host
    /// answered. An encode cut short is finished and sent as an ordinary
    /// checkpoint; a request already sent may still be accepted.
    TimedOut,
    /// The connection failed; the runtime reconnects.
    Failed(ClientError),
    /// The encoder failed. When it panicked, the emulator is kept and so
    /// is the host's previous snapshot; when its task was lost, the
    /// emulator went with it and the runtime ends
    /// (`Unavailability::EmulatorLost`).
    EncodeFailed,
}

/// A runtime's checkpoint bookkeeping.
#[derive(Debug)]
pub(super) struct Checkpoints {
    policy: CheckpointPolicy,
    /// The latest snapshot the host is known to hold: one this runtime had
    /// accepted, or the one the host held at adoption when its position
    /// said it had one.
    accepted: Option<Position>,
    /// Where the latest attempt was made (or the host's snapshot at
    /// adoption): output is counted from here.
    attempted_at: Position,
    /// The size of the latest snapshot encoded, or restored at adoption;
    /// `None` before any.
    latest_size: Option<u64>,
    /// A snapshot encoded and not yet sent: its position and blob.
    ready: Option<(Position, Vec<u8>)>,
}

impl Checkpoints {
    /// Bookkeeping for a runtime that adopted a host whose stored snapshot
    /// is at `stored` ((0, 0) when it has none, or only an initial one).
    pub(super) fn new(policy: CheckpointPolicy, stored: Position) -> Self {
        Self {
            policy,
            accepted: (stored != Position::default()).then_some(stored),
            attempted_at: stored,
            latest_size: None,
            ready: None,
        }
    }

    /// Whether a checkpoint is due at `position`: enough output has passed
    /// since the previous attempt, and no snapshot is waiting to be sent.
    pub(super) fn due(&self, position: Position) -> bool {
        self.ready.is_none()
            && position.offset.saturating_sub(self.attempted_at.offset)
                >= self.policy.after_bytes(self.latest_size)
    }

    /// Whether anything changed at `position` since the latest attempt:
    /// a snapshot where the previous attempt was made would be the same.
    pub(super) fn changed(&self, position: Position) -> bool {
        position != self.attempted_at
    }

    /// An attempt starts at `position`: output is counted from here.
    pub(super) fn attempted(&mut self, position: Position) {
        self.attempted_at = position;
    }

    /// A snapshot of `size` bytes was restored (at adoption, or from a
    /// reconnect's stream): it sets the threshold until one is encoded.
    pub(super) fn restored(&mut self, size: usize) {
        self.latest_size = Some(size as u64);
    }

    /// A snapshot of `position` was encoded, to be sent.
    pub(super) fn encoded(&mut self, position: Position, blob: Vec<u8>) {
        self.encoded_now(blob.len());
        self.ready = Some((position, blob));
    }

    /// A snapshot of `size` bytes was encoded, and the caller sends it
    /// itself (an orderly stop's).
    pub(super) fn encoded_now(&mut self, size: usize) {
        self.latest_size = Some(size as u64);
    }

    /// Whether a snapshot waits to be sent.
    pub(super) fn has_ready(&self) -> bool {
        self.ready.is_some()
    }

    /// The snapshot waiting to be sent, if one is.
    pub(super) fn take_ready(&mut self) -> Option<(Position, Vec<u8>)> {
        self.ready.take()
    }

    /// The host accepted the snapshot at `position`.
    pub(super) fn accept(&mut self, position: Position) {
        self.accepted = Some(position);
    }

    /// The latest snapshot the host is known to hold (`accepted`).
    pub(super) fn accepted(&self) -> Option<Position> {
        self.accepted
    }

    /// Apply the stop threshold to the newest accepted snapshot, which
    /// may be newer than the supervisor saw before waiting for a slot.
    pub(super) fn exit_tail(&self, position: Position) -> Option<u64> {
        self.policy.exit_tail(position, self.accepted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(offset: u64) -> Position {
        Position { offset, resizes: 0 }
    }

    const MIB: u64 = 1024 * 1024;

    /// Stop admission and runtime requests use the same boundary, counting
    /// only output after the latest accepted snapshot. A newer automatic
    /// snapshot can make a queued stop checkpoint unnecessary, and resizes
    /// alone must not buy another encode.
    #[test]
    fn the_stop_threshold_leaves_small_tails_for_recovery() {
        let policy = CheckpointPolicy::default();
        assert_eq!(policy.exit_tail(at(MIB - 1), None), None);
        assert_eq!(policy.exit_tail(at(MIB), None), Some(MIB));
        assert_eq!(policy.exit_tail(at(2 * MIB), Some(at(MIB))), Some(MIB));
        assert_eq!(policy.exit_tail(at(2 * MIB), Some(at(2 * MIB - 1))), None);
        assert_eq!(
            policy.exit_tail(
                Position {
                    offset: 0,
                    resizes: 10
                },
                None
            ),
            None
        );
        let forced = CheckpointPolicy {
            stop_min_bytes: 0,
            ..policy
        };
        assert_eq!(forced.exit_tail(at(1), None), Some(1));
        assert_eq!(forced.exit_tail(at(0), None), Some(0));
        assert_eq!(
            CheckpointPolicy::for_journal_cap(64 * 1024).stop_min_bytes,
            16 * 1024
        );
    }

    /// The output that makes a checkpoint due is four times the latest
    /// snapshot's size, within the floor and the ceiling, and the ceiling
    /// wins when they cross.
    ///
    /// Why: the threshold is what bounds checkpoints' cost (encoding a
    /// snapshot costs in proportion to it) and the host's tail while a
    /// supervisor is connected (a quarter of the cap); a threshold that
    /// ignored the snapshot would encode a full scrollback as often as an
    /// empty one, and one past the ceiling would leave less of the cap for
    /// an absence.
    /// Spec: with the default 32 MiB cap, no snapshot yet and a 4 KiB one
    /// give 1 MiB; a 1 MiB snapshot gives 4 MiB; a 3 MiB one gives the
    /// 8 MiB ceiling; with the protocol's smallest cap (64 KiB) the ceiling
    /// of 16 KiB wins over the floor.
    #[test]
    fn the_threshold_scales_with_the_snapshot_within_bounds() {
        let policy = CheckpointPolicy::default();
        assert_eq!(policy.after_bytes(None), MIB);
        assert_eq!(policy.after_bytes(Some(4096)), MIB);
        assert_eq!(policy.after_bytes(Some(MIB)), 4 * MIB);
        assert_eq!(policy.after_bytes(Some(3 * MIB)), 8 * MIB);
        let small = CheckpointPolicy::for_journal_cap(64 * 1024);
        assert_eq!(small.after_bytes(None), 16 * 1024);
        assert_eq!(small.after_bytes(Some(MIB)), 16 * 1024);
    }

    /// A checkpoint falls due only after enough output since the previous
    /// attempt, however long ago that was, and never while one waits to be
    /// sent; the latest snapshot, restored or encoded, sets how much.
    ///
    /// Why: checkpoints cost a snapshot of the whole emulator; one taken
    /// for a short tail buys almost nothing (it replays in milliseconds),
    /// a second before the first is sent is pure cost, and one never taken
    /// lets the host's tail reach its cap.
    /// Spec: with the default policy and nothing restored, 1 MiB - 1 on is
    /// not due and 1 MiB is; a resize alone is not due; after a 2 MiB
    /// snapshot is restored, 1 MiB is no longer due and 8 MiB is; with a
    /// snapshot encoded and not sent, nothing is due; once it is taken,
    /// output counts from the attempt, at a threshold set by the encoded
    /// snapshot's size.
    #[test]
    fn a_checkpoint_falls_due_by_output_only() {
        let mut checkpoints = Checkpoints::new(CheckpointPolicy::default(), at(0));
        assert!(!checkpoints.due(at(0)));
        assert!(!checkpoints.due(at(MIB - 1)));
        assert!(checkpoints.due(at(MIB)));
        assert!(!checkpoints.due(Position {
            offset: 0,
            resizes: 1
        }));
        assert!(checkpoints.changed(Position {
            offset: 0,
            resizes: 1
        }));

        checkpoints.restored(2 * MIB as usize);
        assert!(!checkpoints.due(at(MIB)));
        assert!(checkpoints.due(at(8 * MIB)));

        checkpoints.attempted(at(8 * MIB));
        checkpoints.encoded(at(8 * MIB), vec![0; 1024]);
        assert!(!checkpoints.due(at(100 * MIB)));
        assert_eq!(
            checkpoints.take_ready().map(|(at, _)| at),
            Some(at(8 * MIB))
        );
        assert!(!checkpoints.due(at(9 * MIB - 1)));
        assert!(checkpoints.due(at(9 * MIB)));
    }
}

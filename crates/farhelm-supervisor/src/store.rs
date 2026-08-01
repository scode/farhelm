//! SQLite session persistence: the durable half of the split introduced
//! by M2 (PLAN_M2.md, "Supervisor SQLite"). tmux stays the truth for
//! whether a session's terminal is alive; this module is the truth for
//! the coarser fact that a session exists at all and what its metadata
//! is, which must survive the supervisor process even though tmux (or
//! the whole host) might not have restarted alongside it.
//!
//! Schema versioning uses SQLite's `PRAGMA user_version` rather than a
//! separate version-tracking table. A version table was considered and
//! rejected: until a migration needs its own per-step metadata (applied
//! timestamps, checksums, ...), the pragma is atomic with the database
//! file itself, needs no bootstrap ordering (there is no chicken-and-egg
//! "which table records that the version table exists"), and — unlike a
//! table — cannot itself be missing from an otherwise-valid database.
//!
//! M3 adds the one thing the module docs above say is NOT persisted —
//! and the distinction matters, because it is easy to read as a
//! contradiction. Liveness is still never persisted: tmux remains the
//! only truth for "is this agent running right now". What [`LastOutcome`]
//! records is the supervisor's own last WITNESSED transition (PLAN_M3.md
//! item 2), which is a different kind of fact: it is what lets a reboot —
//! the one event that destroys tmux entirely, taking every probe-able
//! answer with it — be classified as **interrupted** instead of guessed
//! at. A stored outcome answers the question a vanished tmux leaves
//! unanswerable, and otherwise defers to the live probe — with ONE
//! exception, `LastOutcome::Error`, which outranks probing entirely
//! because "the agent never execed" is not something a pane can show: a
//! failed exec leaves an ordinary dead pane, indistinguishable from a
//! command that ran and finished (`service::session_status` spells the
//! full precedence out).
//!
//! Journal mode and synchronous pragmas are left at SQLite's defaults.
//! M3 is where PLAN.md places the explicit crash-safety/atomicity policy
//! for the state store; this module does not invent one ahead of that.

use anyhow::Context;
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// How long a query waits on `SQLITE_BUSY` before giving up.
///
/// A brief window where two supervisor processes both hold this database
/// open is the normal shape of a handoff restart (the old process still
/// running while the new one constructs — see `Supervisor::serve`'s
/// second `reload_sessions` call). Without a busy timeout, SQLite returns
/// `SQLITE_BUSY` to the loser of that overlap immediately, turning an
/// ordinary handoff into a spurious open/query failure; a bounded wait
/// instead lets the loser's request go through once the winner's
/// transaction releases the lock, at the cost of stalling that one call
/// for up to this long in the pathological case.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// The schema's current shape. Bumping this requires a matching migration
/// step in `apply_schema`: version 2 (PLAN_M3.md item 2 — the durable
/// last-known outcome and the boot id) is the first real migration this
/// database has ever had, and the template every later one follows.
const SCHEMA_VERSION: i64 = 2;

/// The sole row id of [`supervisor_meta`](apply_schema) — the table is
/// single-row by construction (a `CHECK` on this value), so every read and
/// write names it explicitly rather than scanning.
const META_ROW_ID: i64 = 0;

/// What the supervisor last WITNESSED about a session's agent, durably
/// (PLAN_M3.md item 2). Not a cached liveness probe: see the module docs
/// for why persisting this is not a contradiction of "liveness is never
/// persisted".
///
/// The states form a deliberate ordering from least to most committed —
/// `Launching` → `Running` → a terminal state (`Exited`, `Interrupted`,
/// `Error`) — and the crash-ordering rule that shapes every writer is that
/// the stored value may LAG reality toward the earlier state but must
/// never LEAD it. That is why `Launching` is committed before the tmux
/// side effect it describes rather than after: a crash straddling a launch
/// then leaves a record that under-claims (a session that may in fact be
/// running is recorded as merely starting, and reload reconciles it
/// against reality) instead of one that over-claims, or — worse — one that
/// still shows the PREVIOUS run's outcome for a session that has since
/// been relaunched.
///
/// Terminal states are sticky, and the rule is enforced HERE — in
/// [`Transition::apply`], evaluated inside the same SQLite transaction
/// that writes the result — rather than by callers. That location is the
/// point: a caller that reads the outcome, decides, and writes later is a
/// TOCTOU, and the concrete loss it produced was a stop's annotation being
/// erased by a concurrent list's plain-exit write (both authorized from
/// `Running`, and the later write won). Once `Exited`, `Interrupted`, or
/// `Error`, an outcome only ever accepts MONOTONIC enrichment: a missing
/// exit code may be filled in by an observation that has one, and a plain
/// exit may gain a stop annotation, but nothing ever loses information
/// (SPEC.md's no-guessing rule: retained knowledge is not a guess).
///
/// One transition crosses terminal classes anyway, and it is not an
/// exception to that rule so much as its sharpest application: a launch
/// sentinel (`Transition::SentinelError`) reclassifies an `Interrupted` or
/// an UNANNOTATED `Exited` straight to `Error`, because both are
/// themselves just INFERENCES from an ordinary dead-or-vanished pane —
/// exactly the evidence class a sentinel is defined to outrank — and
/// protecting a wrong inference with terminal-stickiness would defeat the
/// reason the sentinel is read at all. A genuinely ANNOTATED `Exited` (a
/// real stop) is retained knowledge, not an inference, and is the one
/// terminal state a sentinel still cannot cross; see `Transition::apply`'s
/// own docs for the full rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LastOutcome {
    /// A launch was begun but never confirmed. Written before the tmux
    /// session it describes exists at all, so a row in this state carries
    /// an empty `pane` (see [`StoredSession::pane`]).
    ///
    /// Deliberately NOT terminal and deliberately never converted to one
    /// by the mere absence of side effects: PLAN_M3.md item 2 sends a
    /// launching row whose side effects cannot be found to error-or-retry
    /// territory (item 3's sentinel, item 6's reservation), and SPEC.md's
    /// `Exited` means the agent RAN. A launch that may never have started
    /// is reported as `Unknown` until one of those PRs can say which.
    Launching,
    /// The launch was confirmed: a tmux pane for this session existed at
    /// the moment this was written.
    Running,
    /// The user asked for this session to be stopped and the kill sweep
    /// has not yet reported back (PLAN_M3.md item 4).
    ///
    /// A durable INTENT, not an outcome, and it exists because the window
    /// it covers is seconds long: `kill_process_tree` sends SIGTERM, waits
    /// out a grace period, re-enumerates, and only then SIGKILLs. A crash
    /// anywhere in there used to leave a session that the next startup
    /// classified as a plain exit — silently converting "the user stopped
    /// this" into "the agent finished on its own". With the intent stored
    /// first, reload reconciles it: dead pane (or a vanished terminal)
    /// means the stop landed and the exit is annotated; a still-live pane
    /// means the kill never happened and the intent is cleared; a reboot
    /// straddling it interrupts like any other live session.
    ///
    /// It is also what makes the annotation immune to the concurrent-list
    /// race: a list observing the pane die mid-stop transitions from THIS
    /// state, and [`Transition::ObservedExit`] from `StopRequested` is
    /// defined to produce the annotated exit.
    StopRequested,
    /// The agent ended and the supervisor saw it. `exit_code` is tmux's
    /// own `#{pane_dead_status}` when it had one to give — `None` covers a
    /// signal death and the case where no pane survived to be asked.
    /// `annotation` is SPEC.md's user-legible qualifier, set only by a
    /// user-initiated stop (`farhelm_proto::STOP_ANNOTATION`), which is
    /// what makes "stopped by user" survive a supervisor restart and a
    /// reboot alike.
    Exited {
        exit_code: Option<i32>,
        annotation: Option<String>,
    },
    /// The host rebooted while this session was still live — `Launching`,
    /// `Running`, or `StopRequested`: tmux is gone, so nothing can ever be
    /// probed about this agent again (PLAN_M3.md item 2). Written only by
    /// the boot-id conversion in [`SessionStore::record_boot`], and
    /// otherwise cleared only by a restart or a delete — the one other
    /// route out is a launch sentinel discovered AFTER the conversion
    /// already ran, which reclassifies straight to `Error` rather than
    /// leaving a reboot's mere inference (nothing could be probed) stand
    /// in for a fact this process can actually now name (`Transition`'s
    /// `SentinelError` docs).
    Interrupted,
    /// The agent could not be started at all — the launch shim's
    /// exec-failure sentinel (PLAN_M3.md item 3), read by
    /// `crate::launch::read_launch_sentinel` and committed via
    /// [`Transition::SentinelError`]. `detail` is the shim's own recorded
    /// report (errno, argv0, or which pre-exec step failed), surfaced
    /// verbatim on the wire as `SessionStatus::Error`'s `detail`.
    Error { detail: String },
}

impl LastOutcome {
    /// Whether this state is final for the current launch: no observation
    /// can move it to a different class, only enrich it in place. Public
    /// because `service.rs` needs it to decide what to OBSERVE (a
    /// terminal-outcome session with no pane is not re-observed at all),
    /// never to decide what to write — that decision belongs to
    /// [`Transition::apply`] alone.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            LastOutcome::Exited { .. } | LastOutcome::Interrupted | LastOutcome::Error { .. }
        )
    }

    /// Split into the four columns that store it.
    ///
    /// The state text is a STABLE on-disk vocabulary, deliberately spelled
    /// out here rather than derived from the Rust variant names: renaming
    /// a variant must not silently invalidate every database in the field,
    /// and `from_columns` below is the exact inverse of this table.
    fn columns(&self) -> (&'static str, Option<i32>, Option<&str>, Option<&str>) {
        match self {
            LastOutcome::Launching => ("launching", None, None, None),
            LastOutcome::Running => ("running", None, None, None),
            LastOutcome::StopRequested => ("stop_requested", None, None, None),
            LastOutcome::Exited {
                exit_code,
                annotation,
            } => ("exited", *exit_code, annotation.as_deref(), None),
            LastOutcome::Interrupted => ("interrupted", None, None, None),
            LastOutcome::Error { detail } => ("error", None, None, Some(detail.as_str())),
        }
    }

    /// Reassemble from the four columns.
    ///
    /// An unrecognized `state` is refused rather than defaulted: the
    /// schema version already gates every shape this build understands
    /// (`apply_schema`), so a value outside the vocabulary means the row
    /// is corrupt, and guessing a state for it would be exactly the
    /// fabricated claim `SessionStatus`'s own docs forbid. `Error` with a
    /// missing detail is refused for the same reason — the wire type's
    /// `detail` is a required `String`, so there is no honest value to
    /// substitute.
    fn from_columns(
        state: &str,
        exit_code: Option<i32>,
        annotation: Option<String>,
        error_detail: Option<String>,
    ) -> anyhow::Result<LastOutcome> {
        Ok(match state {
            "launching" => LastOutcome::Launching,
            "running" => LastOutcome::Running,
            "stop_requested" => LastOutcome::StopRequested,
            "exited" => LastOutcome::Exited {
                exit_code,
                annotation,
            },
            "interrupted" => LastOutcome::Interrupted,
            "error" => LastOutcome::Error {
                detail: error_detail.ok_or_else(|| {
                    anyhow::anyhow!("session row has outcome 'error' but no error_detail")
                })?,
            },
            other => anyhow::bail!("session row has unrecognized outcome state {other:?}"),
        })
    }
}

/// Something the supervisor witnessed, offered to a session's durable
/// outcome. The store — never the caller — decides what it means for the
/// state already recorded (see [`Transition::apply`]).
///
/// The split matters: callers observe (a pane died, the user asked for a
/// stop, a launch was confirmed) and the store arbitrates, all inside one
/// transaction. That is what makes two concurrent observers safe without
/// either of them holding a lock across an await.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transition {
    /// A tmux pane for this session exists and is alive. Carries the pane
    /// id because a launch is only confirmed once tmux has assigned one,
    /// and the two facts must land together (a `Running` row with no pane
    /// is exactly what reload treats as an unconfirmed launch).
    ///
    /// Also the reconciliation for a stop that never happened: a live pane
    /// under a `StopRequested` row means the kill did not land before the
    /// crash, so the intent is cleared. Only reload may send this — a
    /// concurrent list observing a still-live pane mid-stop must NOT clear
    /// an intent whose kill sweep is still running, so the list path never
    /// sends `ConfirmRunning` at all.
    ConfirmRunning { pane: String },
    /// The user asked for this session to be stopped, before any killing
    /// starts. See [`LastOutcome::StopRequested`].
    StopRequested,
    /// The agent's process is gone, as observed by anything other than a
    /// stop the supervisor performed: a dead pane, or a terminal that no
    /// longer exists at all (`exit_code: None`).
    ///
    /// From `StopRequested` this yields the ANNOTATED exit — the observer
    /// need not know a stop was in flight, which is precisely why the
    /// annotation can no longer be raced away.
    ObservedExit { exit_code: Option<i32> },
    /// A stop's kill sweep completed against a pane this supervisor
    /// observed ALIVE when the stop began. Yields the annotated exit.
    StopCompleted { exit_code: Option<i32> },
    /// A launching row whose pane was rediscovered by session name, found
    /// dead: the pane id and the exit it evidences, recorded together so
    /// no crash window can leave the pane written but the outcome not (the
    /// gap that would resurrect the row as `Running` on the next reload).
    RediscoveredExit {
        pane: String,
        exit_code: Option<i32>,
    },
    /// The launch shim's exec-failure sentinel was found for this
    /// session's CURRENT launch (`crate::launch::read_launch_sentinel`).
    /// `detail` is the shim's own recorded report, carried straight
    /// through to [`LastOutcome::Error`] and from there to the wire's
    /// `SessionStatus::Error` (PLAN_M3.md item 3).
    ///
    /// `pane`, when `Some`, is whatever pane the CALLER already had in
    /// hand for this row at the moment it read the sentinel — mirrors
    /// [`RediscoveredExit`]'s reasoning for why the pane rides the SAME
    /// commit as the outcome it accompanies, even though `Error`'s status
    /// computation (`service::session_status`) does not itself consult
    /// the pane. Every call site fills this differently: `reload_sessions`
    /// passes whatever its own pane lookup found (by session name for a
    /// row with no stored pane yet, by pane id otherwise — either shape,
    /// not only the empty-pane `Launching` case); `ListSessions` passes
    /// `None` outright (it only ever visits sessions it already tracks a
    /// `Terminal` for, so there is nothing new to rediscover); `StopSession`
    /// passes the pane its own already-loaded `SessionEntry` carries.
    /// Recording it anyway keeps the stored row internally consistent with
    /// what tmux actually shows, for whatever later reads the `pane`
    /// column directly (diagnostics, a future migration) — it is never
    /// load-bearing for `Error`'s own classification.
    SentinelError {
        detail: String,
        pane: Option<String>,
    },
}

impl Transition {
    /// The one place the transition policy lives: given the outcome
    /// currently committed, what — if anything — should replace it.
    ///
    /// `None` means "no change", which is a first-class answer here, not a
    /// failure: refusing a transition is how retained knowledge survives a
    /// later, poorer observation.
    ///
    /// The rules, and why each exists:
    ///
    /// - **Terminal classes never change class from an ORDINARY
    ///   observation.** `Interrupted` and `Error` describe something no
    ///   `ObservedExit`/`RediscoveredExit`/`StopCompleted` can contradict —
    ///   a reboot destroyed the evidence, or the agent never ran at all.
    ///   `SentinelError` is the one transition that still crosses this
    ///   boundary, and only into `Interrupted` or an unannotated `Exited`
    ///   (never into `Error` itself, which stays genuinely immutable); see
    ///   its own bullet below for why that is not a contradiction of this
    ///   one.
    /// - **`Exited` enriches monotonically.** tmux publishes `pane_dead`
    ///   before `pane_dead_status` becomes readable, so the FIRST observer
    ///   of an exit routinely has no code while a later one does; SPEC.md
    ///   requires showing the code when it is known, so a known code
    ///   replaces a missing one and never the reverse. The stop annotation
    ///   is the same shape of enrichment on the other field.
    /// - **A stop in flight owns the annotation.** Both `ObservedExit`
    ///   from `StopRequested` and `StopCompleted` produce it, so whichever
    ///   observer commits first, the answer is the same.
    /// - **Nothing resurrects.** `ConfirmRunning` against a terminal
    ///   outcome is refused rather than reopening a closed session.
    /// - **A sentinel wins over every state EXCEPT a genuine stop
    ///   annotation or an already-recorded error.** `SentinelError`
    ///   reclassifies `Launching`, `Running`, or `StopRequested` straight
    ///   to `Error` (PLAN_M3.md item 3: "the agent never started" outranks
    ///   any exit inference, because a failed exec and a command that ran
    ///   and died leave an identical dead pane behind) — that much needs no
    ///   special case, since none of those are terminal yet. What DOES
    ///   need a special case, and is handled by the two `SentinelError`
    ///   arms placed BEFORE the general terminal-class catch-all below,
    ///   is that a sentinel discovered late (a stop or a list committed an
    ///   INFERRED `Exited` or the reboot conversion committed
    ///   `Interrupted` before anything ever read the file) must still win:
    ///   both were themselves just inferences from an ordinary dead pane
    ///   or a vanished terminal, exactly the evidence a sentinel is
    ///   defined to outrank, and letting terminal-stickiness protect a
    ///   WRONG terminal classification would defeat the entire point of
    ///   reading the sentinel at all. The one thing that DOES block it: an
    ///   `Exited` carrying a stop annotation, because that means a REAL
    ///   run was observed alive long enough for the user to stop it — a
    ///   sentinel and a stop annotation cannot both be true of the same
    ///   launch, so an annotated exit is retained genuine knowledge, not
    ///   an inference to override. An already-`Error` row is simply
    ///   unchanged (idempotent), and `Interrupted`/unannotated-`Exited`
    ///   fall out of the same two arms rather than needing their own.
    ///   The one case this policy does NOT cover on its own — a sentinel
    ///   that must outrank the reboot-interrupted conversion AS IT HAPPENS,
    ///   not just arrive after it — is handled a level up, by
    ///   `SessionStore::record_boot` applying sentinel overrides before
    ///   its blanket interrupt `UPDATE` runs; see that function's docs.
    pub fn apply(&self, current: &LastOutcome) -> Option<LastOutcome> {
        /// Merge into an existing `Exited`, keeping every fact either side
        /// has. Returns `None` when the merge changes nothing.
        fn enrich(
            code: Option<i32>,
            annotation: Option<String>,
            current_code: Option<i32>,
            current_annotation: &Option<String>,
        ) -> Option<LastOutcome> {
            let merged = LastOutcome::Exited {
                exit_code: current_code.or(code),
                annotation: current_annotation.clone().or(annotation),
            };
            (merged
                != LastOutcome::Exited {
                    exit_code: current_code,
                    annotation: current_annotation.clone(),
                })
            .then_some(merged)
        }

        let stopped = || Some(farhelm_proto::STOP_ANNOTATION.to_string());
        let ended = |exit_code: &Option<i32>, annotation: Option<String>| {
            Some(LastOutcome::Exited {
                exit_code: *exit_code,
                annotation,
            })
        };
        use LastOutcome as O;
        use Transition as T;
        match (self, current) {
            // A sentinel supersedes an INFERRED terminal state — `Interrupted`
            // (the reboot conversion) or an `Exited` with NO stop annotation
            // (nothing but a dead-or-vanished pane ever backed either one).
            // Placed before the general terminal-class catch-all below so
            // these two arms see `current` first; every other transition
            // still hits that catch-all unchanged.
            (
                T::SentinelError { detail, .. },
                O::Launching
                | O::Running
                | O::StopRequested
                | O::Interrupted
                | O::Exited {
                    annotation: None, ..
                },
            ) => Some(O::Error {
                detail: detail.clone(),
            }),
            // ...but a GENUINE stop annotation is retained knowledge, not
            // an inference: it means a real run was observed alive long
            // enough for the user to stop it, which cannot be true of the
            // same launch a sentinel also claims never started. And an
            // already-`Error` row is simply unchanged (idempotent).
            (
                T::SentinelError { .. },
                O::Exited {
                    annotation: Some(_),
                    ..
                }
                | O::Error { .. },
            ) => None,

            // Nothing else may reopen or reclassify these.
            (_, O::Interrupted | O::Error { .. }) => None,

            // A confirmed launch never resurrects a closed session, and
            // re-confirming an already-running one writes nothing.
            (T::ConfirmRunning { .. }, O::Exited { .. } | O::Running) => None,
            (T::ConfirmRunning { .. }, O::Launching | O::StopRequested) => Some(O::Running),

            (T::StopRequested, O::Launching | O::Running) => Some(O::StopRequested),
            (T::StopRequested, O::StopRequested | O::Exited { .. }) => None,

            (
                T::ObservedExit { exit_code } | T::RediscoveredExit { exit_code, .. },
                O::Launching | O::Running,
            ) => ended(exit_code, None),
            (
                T::ObservedExit { exit_code } | T::RediscoveredExit { exit_code, .. },
                O::StopRequested,
            ) => ended(exit_code, stopped()),
            (
                T::ObservedExit { exit_code } | T::RediscoveredExit { exit_code, .. },
                O::Exited {
                    exit_code: current_code,
                    annotation,
                },
            ) => enrich(*exit_code, None, *current_code, annotation),

            (T::StopCompleted { exit_code }, O::Launching | O::Running | O::StopRequested) => {
                ended(exit_code, stopped())
            }
            (
                T::StopCompleted { exit_code },
                O::Exited {
                    exit_code: current_code,
                    annotation,
                },
            ) => enrich(*exit_code, stopped(), *current_code, annotation),
        }
    }

    /// The pane id this transition also commits, if any. Kept beside
    /// [`Transition::apply`] so the store writes both halves in one
    /// statement rather than rediscovering which variants carry a pane.
    fn pane(&self) -> Option<&str> {
        match self {
            Transition::ConfirmRunning { pane } | Transition::RediscoveredExit { pane, .. } => {
                Some(pane)
            }
            Transition::SentinelError { pane, .. } => pane.as_deref(),
            _ => None,
        }
    }
}

/// The four columns that spell out one [`LastOutcome`] on disk: the state
/// text, and the three optional fields only some states carry. Named
/// because it is the shape both `LastOutcome::columns` and every read path
/// pass around, not because callers reason about the tuple itself.
type OutcomeColumns = (String, Option<i32>, Option<String>, Option<String>);

/// A failure injected INSIDE [`SessionStore::record_boot`]'s transaction,
/// between the sentinel overrides and the blanket interrupted conversion —
/// only ever called when `interrupt_live` is true, since that is the only
/// case where either of those two statements runs at all.
///
/// This exists for tests that cannot be written any other way: the
/// crash-boundary case PLAN_M3.md item 2 calls out, where a crash between
/// the new boot id and the interrupted conversion would — if they were not
/// one transaction — let the next startup see the new boot id already
/// stored, take the same-boot path, and misclassify every session the
/// reboot actually interrupted; and the sharper version PLAN_M3.md item 3
/// adds, where a crash between the sentinel overrides and the blanket
/// conversion must roll BOTH back together, not leave a partially-applied
/// mix of `error` and `interrupted` rows behind. Returning an error rolls
/// the whole transaction back, which is exactly what a crash at that
/// instant does. Production always passes `None`.
pub type BootTxFault = Arc<dyn Fn() -> anyhow::Result<()> + Send + Sync>;

/// The stored fields the supervisor actually consumes: wire metadata plus
/// the tmux handles a session was created with.
///
/// Not the whole `sessions` row — `created_at` is write-only from this
/// type's perspective. `insert_session` fills it in itself (see
/// `now_unix`) rather than accepting it here, and nothing yet reads it
/// back (it exists for a human inspecting the database, and as a schema
/// field a future migration can build on); adding a field to this struct
/// for it would invite call sites to treat an informational timestamp as
/// load-bearing.
///
/// This is the store's own type rather than a reuse of `SessionInfo`
/// (the wire type) or `service::SessionEntry` (the live in-memory type):
/// the database additionally needs `tmux_name`/`pane`, which `SessionInfo`
/// has no reason to carry over the wire, and must not depend on
/// `SessionEntry`'s shape, which is free to keep evolving (e.g. gaining
/// the restart-gap `terminal: Option<Terminal>` field) independently of
/// what is stored on disk.
///
/// Deliberately missing: `SessionInfo::status`. Liveness is never
/// persisted — tmux is its only truth (module docs above), and a status
/// written at some past moment would be stale the instant the process it
/// described changed state, with nothing to invalidate it on the way
/// back out of SQLite. `service::Supervisor::reload_sessions` always
/// recomputes a freshly loaded row's terminal from a live tmux probe, and
/// `ListSessions` recomputes `status` itself on every reply
/// (`service::session_status`) — a persisted status column would be
/// redundant at best and actively misleading at worst. [`LastOutcome`] is
/// not that column: it records witnessed transitions, not probe results
/// (module docs).
#[derive(Debug, Clone)]
pub struct StoredSession {
    pub id: String,
    pub title: String,
    pub cwd: String,
    pub invocation: String,
    pub tmux_name: String,
    /// The tmux pane, or EMPTY for a row still in [`LastOutcome::Launching`]
    /// — the pane id does not exist until tmux has created the session,
    /// which by item 2's ordering happens strictly after this row is
    /// committed. An empty pane is therefore "not yet confirmed", never
    /// "no pane": `service::Supervisor::reload_sessions` re-discovers it
    /// from tmux when the launch did in fact happen before the crash.
    pub pane: String,
    /// The last transition the supervisor witnessed for this session.
    pub outcome: LastOutcome,
}

/// The supervisor's session database.
///
/// Wraps the connection in `Arc<Mutex<..>>` (a std, not tokio, mutex —
/// every hold is confined to a single synchronous `spawn_blocking`
/// closure, so there is never an await point inside the critical
/// section) so the store can be cloned into request handlers freely while
/// every actual query still runs serialized against the one connection.
/// rusqlite calls are synchronous, and a commit's fsync can block for
/// real disk-flush time; running them inline on an async worker thread
/// would stall that thread's entire share of the runtime — every other
/// session's terminal forwarding included — for the duration of one
/// session's write. `spawn_blocking` is what keeps that cost off the
/// async workers.
#[derive(Clone, Debug)]
pub struct SessionStore {
    conn: Arc<Mutex<Connection>>,
}

/// Bring the database up to [`SCHEMA_VERSION`], creating it from scratch
/// (`user_version` 0, SQLite's default for a database that has never set
/// it) or migrating it forward one step at a time.
///
/// A `user_version` already at [`SCHEMA_VERSION`] is left untouched — the
/// tables are assumed to match, since this build wrote them. A version
/// ABOVE it still means a schema this build does not understand (a
/// downgrade, or a future migration this build predates), and is refused
/// rather than silently misread; that refusal is unchanged by M3, and only
/// its lower boundary moved. Every migration runs in its OWN transaction
/// together with the `user_version` bump that claims it (SQLite journals
/// the pragma with the rest of the write, so an interrupted upgrade leaves
/// the database at the version it actually has, never at a version whose
/// columns are missing).
///
/// Version history:
/// - 1: M2's `sessions` table (metadata and tmux handles only).
/// - 2: PLAN_M3.md item 2 — the durable last-known outcome on `sessions`,
///   and `supervisor_meta` as the home for the last-seen boot id. The
///   metadata table is new rather than a column beside an existing host
///   identity because no host identity is stored anywhere yet; this is
///   that home, and whatever host identity M6's multi-host work needs
///   belongs in the same row.
///
/// `may_migrate` is the caller's assertion that it holds this state
/// directory's exclusivity (see `service::StateDirOwnership`). Upgrading a
/// database another supervisor is CURRENTLY SERVING is not a harmless
/// no-op: the incumbent is by definition an older build, and the moment it
/// restarts it will refuse its own database as "a schema this build does
/// not understand" — a candidate that then loses the exclusivity race has
/// bricked a supervisor it never replaced. So a process without the lock
/// opens a current-version database read/write (it may still serve
/// requests) but refuses to migrate an older one at all.
fn apply_schema(conn: &Connection, may_migrate: bool) -> anyhow::Result<()> {
    let mut version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .context("reading schema version")?;
    if version != SCHEMA_VERSION && !may_migrate {
        anyhow::bail!(
            "supervisor.db is at schema version {version} and needs upgrading to \
             {SCHEMA_VERSION}, but another supervisor holds this state directory; refusing \
             to upgrade a database that is not this process's to change"
        );
    }
    if version == 0 {
        // A fresh database is created directly in its final shape rather
        // than built up by replaying every historical migration: the
        // migrations below exist to preserve DATA, and there is none to
        // preserve here. The two paths must agree on the result, which
        // `migrated_and_fresh_schemas_agree` pins.
        conn.execute_batch(
            "BEGIN;
             CREATE TABLE sessions (
                 id            TEXT PRIMARY KEY,
                 title         TEXT NOT NULL,
                 cwd           TEXT NOT NULL,
                 invocation    TEXT NOT NULL,
                 tmux_name     TEXT NOT NULL UNIQUE,
                 pane          TEXT NOT NULL,
                 created_at    INTEGER NOT NULL,
                 outcome_state TEXT NOT NULL DEFAULT 'launching',
                 exit_code     INTEGER,
                 annotation    TEXT,
                 error_detail  TEXT
             ) STRICT;
             CREATE TABLE supervisor_meta (
                 id      INTEGER PRIMARY KEY CHECK (id = 0),
                 boot_id TEXT
             ) STRICT;
             PRAGMA user_version = 2;
             COMMIT;",
        )
        .context("creating schema")?;
        version = SCHEMA_VERSION;
    }
    if version == 1 {
        // In place, preserving every row: M2-era sessions have no
        // recorded outcome at all, and they adopt `launching` — the
        // EARLIEST state, not a flattering one. That is the conservative
        // direction the crash-ordering rule demands (see `LastOutcome`):
        // claiming `running` for a session that may well have exited
        // during the upgrade would be the record leading reality, and
        // would additionally paint such a session `interrupted` at the
        // next reboot. `launching` under-claims instead, and the very
        // first reload after this migration reconciles each row against a
        // live tmux probe anyway.
        //
        // `DEFAULT 'launching'` on the added column is what backfills
        // those rows; it is repeated in the fresh-database DDL above so a
        // migrated and a freshly created database have identical schemas.
        conn.execute_batch(
            "BEGIN;
             ALTER TABLE sessions ADD COLUMN outcome_state TEXT NOT NULL DEFAULT 'launching';
             ALTER TABLE sessions ADD COLUMN exit_code     INTEGER;
             ALTER TABLE sessions ADD COLUMN annotation    TEXT;
             ALTER TABLE sessions ADD COLUMN error_detail  TEXT;
             CREATE TABLE supervisor_meta (
                 id      INTEGER PRIMARY KEY CHECK (id = 0),
                 boot_id TEXT
             ) STRICT;
             PRAGMA user_version = 2;
             COMMIT;",
        )
        .context("migrating schema from version 1 to 2")?;
        version = 2;
    }
    if version == SCHEMA_VERSION {
        return Ok(());
    }
    anyhow::bail!(
        "supervisor.db has schema version {version}, but this build only understands \
         version {SCHEMA_VERSION}; refusing to open it rather than risk misreading it"
    )
}

/// Seconds since the Unix epoch, for `created_at`'s informational
/// timestamp. Never fails the caller over a clock reading: `created_at`
/// is documented (see the schema) as informational only, nothing in this
/// module's own logic depends on it, so a pre-epoch system clock — the
/// only way `duration_since` errors — degrades to `0` instead of
/// rejecting an otherwise-successful session creation.
fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

impl SessionStore {
    /// Open (or create) the database at `path`, applying the schema if it
    /// is fresh.
    ///
    /// Confidentiality of the rows this stores (an invocation may embed
    /// credentials passed on an agent's command line, exactly like the
    /// launch specs in `crate::write_private_file`) rests on the state
    /// directory's 0700 mode (`ensure_private_dir`), which every caller of
    /// this function is required to have already established. The
    /// `set_permissions` call below narrows the file's own mode too, but
    /// it is a repair for whatever the ambient umask left behind, not the
    /// boundary: rusqlite creates the file itself before this function
    /// gets a chance to touch it, so a permissive umask leaves a
    /// create-then-chmod window that only the private directory actually
    /// closes.
    ///
    /// Creates the database when it does not exist and MIGRATES it when it
    /// is older than this build — but only with `may_migrate`, which the
    /// caller sets from its exclusivity over the state directory; see
    /// `apply_schema` for why upgrading another supervisor's database is a
    /// destructive act rather than a courtesy.
    pub async fn open(path: &Path, may_migrate: bool) -> anyhow::Result<SessionStore> {
        let path = path.to_path_buf();
        let conn = tokio::task::spawn_blocking(move || -> anyhow::Result<Connection> {
            // Explicit flags, not `Connection::open`'s default set: that
            // default includes `SQLITE_OPEN_URI`, which reinterprets a
            // path starting with `file:` as a URI (query parameters,
            // `?mode=...`, and all) instead of a plain filesystem path.
            // `state_dir` is fixed by this process, not attacker input, but
            // a state directory can end up somewhere a caller named after
            // something that happens to start with `file:` regardless —
            // and URI mode is not a feature this module wants at all, so
            // it is left out rather than relied upon to stay harmless.
            // `SQLITE_OPEN_NO_MUTEX` matches `Connection::open`'s own
            // default: this module already serializes every access
            // through its own `Mutex`, so SQLite's internal connection
            // mutex would be redundant locking for no added safety.
            let conn = Connection::open_with_flags(
                &path,
                OpenFlags::SQLITE_OPEN_READ_WRITE
                    | OpenFlags::SQLITE_OPEN_CREATE
                    | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )
            .with_context(|| format!("opening session database {}", path.display()))?;
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                    .with_context(|| format!("restricting mode of {}", path.display()))?;
            }
            // See `BUSY_TIMEOUT`'s docs: this is what turns a handoff-
            // restart's overlapping access into a brief wait instead of an
            // immediate `SQLITE_BUSY` failure.
            conn.busy_timeout(BUSY_TIMEOUT)
                .context("setting sqlite busy timeout")?;
            apply_schema(&conn, may_migrate)?;
            Ok(conn)
        })
        .await
        .context("session store open task panicked")??;
        Ok(SessionStore {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Persist a freshly created session's metadata.
    ///
    /// As of PLAN_M3.md item 2 this runs BEFORE the tmux session exists,
    /// with `outcome: LastOutcome::Launching` and an empty `pane` — the
    /// inversion of M2's ordering, and the whole point of the launching
    /// state: a crash between this commit and the tmux launch must leave
    /// evidence that a launch was attempted, not silence. A failure here
    /// therefore fails the create before anything external has happened
    /// at all (see `service::Supervisor::create_session`).
    pub async fn insert_session(&self, row: StoredSession) -> anyhow::Result<()> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let conn = conn.lock().expect("session db mutex poisoned");
            let (state, exit_code, annotation, error_detail) = row.outcome.columns();
            conn.execute(
                "INSERT INTO sessions \
                 (id, title, cwd, invocation, tmux_name, pane, created_at, \
                  outcome_state, exit_code, annotation, error_detail) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                rusqlite::params![
                    row.id,
                    row.title,
                    row.cwd,
                    row.invocation,
                    row.tmux_name,
                    row.pane,
                    now_unix(),
                    state,
                    exit_code,
                    annotation,
                    error_detail,
                ],
            )
            .context("inserting session row")?;
            Ok(())
        })
        .await
        .context("session insert task panicked")?
    }

    /// Offer one witnessed [`Transition`] to a session, returning the
    /// outcome that is COMMITTED afterwards — whether or not this call is
    /// what put it there.
    ///
    /// Read-decide-write happens inside a single SQLite transaction, so
    /// two observers racing (the classic being a `ListSessions` seeing the
    /// pane die while a `StopSession` is mid-sweep) can never both
    /// authorize from a stale reading. Callers update their in-memory
    /// mirror FROM the returned value rather than from what they intended
    /// to write, which is what keeps the mirror equal to the database even
    /// when a concurrent writer won.
    ///
    /// `Ok(None)` means the row no longer exists (a concurrent delete);
    /// that is not an error, exactly as `delete_session` tolerates a
    /// missing row.
    pub async fn transition(
        &self,
        id: &str,
        transition: Transition,
    ) -> anyhow::Result<Option<LastOutcome>> {
        let committed = self
            .transition_many(vec![(id.to_string(), transition)])
            .await?;
        Ok(committed.into_values().next())
    }

    /// [`SessionStore::transition`] for a whole reconciliation pass, in ONE
    /// transaction.
    ///
    /// Batched because the alternative is one autocommit — and therefore
    /// one journal sync — per session: the first startup after the schema-2
    /// migration reconciles every migrated row at once, and a list pass on
    /// a busy host can observe many exits in the same reply. The returned
    /// map holds the committed outcome for every id that still exists;
    /// ids deleted concurrently are simply absent.
    pub async fn transition_many(
        &self,
        transitions: Vec<(String, Transition)>,
    ) -> anyhow::Result<HashMap<String, LastOutcome>> {
        if transitions.is_empty() {
            return Ok(HashMap::new());
        }
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<HashMap<String, LastOutcome>> {
            let mut conn = conn.lock().expect("session db mutex poisoned");
            let tx = conn
                .transaction()
                .context("beginning outcome transition transaction")?;
            let mut committed = HashMap::new();
            for (id, transition) in transitions {
                let current: Option<OutcomeColumns> = tx
                    .query_row(
                        "SELECT outcome_state, exit_code, annotation, error_detail \
                             FROM sessions WHERE id = ?1",
                        rusqlite::params![id],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                    )
                    .optional()
                    .context("reading the current outcome")?;
                let Some((state, exit_code, annotation, error_detail)) = current else {
                    continue;
                };
                let current =
                    LastOutcome::from_columns(&state, exit_code, annotation, error_detail)
                        .with_context(|| format!("session {id}"))?;
                let next = transition.apply(&current);
                // The pane rides the same statement as the outcome it
                // belongs to (see `Transition::pane`), and is written
                // even when the outcome itself does not move: a
                // rediscovered pane is still worth recording under an
                // outcome that already knew how the session ended.
                match (&next, transition.pane()) {
                    (Some(next), Some(pane)) => {
                        let (state, code, ann, detail) = next.columns();
                        tx.execute(
                            "UPDATE sessions SET pane = ?2, outcome_state = ?3, \
                                 exit_code = ?4, annotation = ?5, error_detail = ?6 \
                                 WHERE id = ?1",
                            rusqlite::params![id, pane, state, code, ann, detail],
                        )
                        .context("recording a transition with its pane")?;
                    }
                    (Some(next), None) => {
                        let (state, code, ann, detail) = next.columns();
                        tx.execute(
                            "UPDATE sessions SET outcome_state = ?2, exit_code = ?3, \
                                 annotation = ?4, error_detail = ?5 WHERE id = ?1",
                            rusqlite::params![id, state, code, ann, detail],
                        )
                        .context("recording a transition")?;
                    }
                    (None, Some(pane)) => {
                        tx.execute(
                            "UPDATE sessions SET pane = ?2 WHERE id = ?1",
                            rusqlite::params![id, pane],
                        )
                        .context("recording a rediscovered pane")?;
                    }
                    (None, None) => {}
                }
                committed.insert(id, next.unwrap_or(current));
            }
            tx.commit()
                .context("committing outcome transitions")
                .map(|()| committed)
        })
        .await
        .context("outcome transition task panicked")?
    }

    /// The boot id stored by the last supervisor that ran against this
    /// database, or `None` for a database written before PLAN_M3.md item 2
    /// existed (or by a build whose host offers no boot id at all).
    ///
    /// `None` is load-bearing, not merely empty: it is the case that must
    /// NOT be read as a reboot. There is no evidence either way, and the
    /// no-guessing rule cuts both ways, so the caller takes the same-boot
    /// path and stores the id from then on (PLAN_M3.md item 2).
    pub async fn boot_id(&self) -> anyhow::Result<Option<String>> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<Option<String>> {
            let conn = conn.lock().expect("session db mutex poisoned");
            let stored: Option<Option<String>> = conn
                .query_row(
                    "SELECT boot_id FROM supervisor_meta WHERE id = ?1",
                    rusqlite::params![META_ROW_ID],
                    |r| r.get(0),
                )
                .optional()
                .context("reading stored boot id")?;
            Ok(stored.flatten())
        })
        .await
        .context("boot id read task panicked")?
    }

    /// Store `boot_id` and — when `interrupt_live` — convert every session
    /// that was still live (launching, running, or with a stop in flight)
    /// to [`LastOutcome::Interrupted`], in ONE transaction.
    ///
    /// A stop that was mid-sweep when the host went down is interrupted
    /// like any other live session: the reboot destroyed the evidence of
    /// whether the kill ever landed, and inventing "stopped by user" for
    /// it would be a claim about a process nothing observed. Sessions
    /// already `Exited` or `Error` keep everything they had.
    ///
    /// The atomicity is the entire contract, and PLAN_M3.md item 2 spells
    /// out the failure it exists to exclude: were the boot id committed
    /// first and the conversion second, a crash in between would leave the
    /// next startup comparing against the ALREADY-UPDATED id, concluding
    /// same-boot, and probing sessions whose terminals a reboot had
    /// already destroyed — every one of them silently misclassified as
    /// exited-unknown rather than interrupted, permanently. Rolling both
    /// back together instead means the next startup simply sees the old id
    /// again and redoes the whole conversion.
    ///
    /// `fault` is the injection point that makes that boundary testable;
    /// see [`BootTxFault`]. Production passes `None`.
    ///
    /// `sentinel_overrides` is PLAN_M3.md item 3's supersession of this
    /// same conversion: a row about to be blanket-converted to
    /// `Interrupted` may instead have a launch sentinel on disk proving
    /// its agent never started at all, and "never started" outranks "lost
    /// to a reboot" (`Transition::apply`'s docs on `SentinelError`) even
    /// though that transition's OWN policy cannot reach far enough to say
    /// so — by the time a normal `Transition` runs, `Interrupted` is
    /// already terminal and refuses every further reclassification. The
    /// override has to land INSIDE this same transaction, before the
    /// blanket `UPDATE`, so the two can never race: this method applies
    /// each override first (flipping that row straight to `error`), and
    /// the blanket `UPDATE`'s `WHERE outcome_state IN (...)` then simply
    /// no longer matches it — no `NOT IN` clause needed, because the row
    /// already left the states that clause selects. A crash between the
    /// overrides and the blanket update rolls the whole transaction back
    /// with the rest of `record_boot`'s atomicity guarantee, so the next
    /// startup simply redoes both steps together. Empty for every caller
    /// that is not converting a reboot (`interrupt_live == false`) or has
    /// no sentinel-bearing rows to override.
    pub async fn record_boot(
        &self,
        boot_id: &str,
        interrupt_live: bool,
        sentinel_overrides: HashMap<String, String>,
        fault: Option<BootTxFault>,
    ) -> anyhow::Result<()> {
        let conn = Arc::clone(&self.conn);
        let boot_id = boot_id.to_string();
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let mut conn = conn.lock().expect("session db mutex poisoned");
            let tx = conn.transaction().context("beginning boot transaction")?;
            tx.execute(
                "INSERT INTO supervisor_meta (id, boot_id) VALUES (?1, ?2) \
                 ON CONFLICT(id) DO UPDATE SET boot_id = excluded.boot_id",
                rusqlite::params![META_ROW_ID, boot_id],
            )
            .context("storing boot id")?;
            if interrupt_live {
                // Sentinel overrides FIRST: each affected row leaves the
                // 'launching'/'running'/'stop_requested' states before the
                // blanket UPDATE below ever runs, which is what makes the
                // two statements mutually exclusive without an explicit
                // `NOT IN` — see this method's own docs.
                for (id, detail) in &sentinel_overrides {
                    tx.execute(
                        "UPDATE sessions SET outcome_state = 'error', exit_code = NULL, \
                         annotation = NULL, error_detail = ?2 \
                         WHERE id = ?1 \
                         AND outcome_state IN ('launching', 'running', 'stop_requested')",
                        rusqlite::params![id, detail],
                    )
                    .context("applying a sentinel override ahead of the boot conversion")?;
                }
                // The fault seam's exact position: after the overrides have
                // been applied, before the blanket conversion runs — see
                // `BootTxFault`'s own docs for why this specific boundary
                // is the one worth a dedicated injection point.
                if let Some(fault) = fault {
                    fault()?;
                }
                tx.execute(
                    "UPDATE sessions SET outcome_state = 'interrupted', exit_code = NULL, \
                     annotation = NULL, error_detail = NULL \
                     WHERE outcome_state IN ('launching', 'running', 'stop_requested')",
                    [],
                )
                .context("converting live sessions to interrupted")?;
            }
            tx.commit().context("committing boot transaction")?;
            Ok(())
        })
        .await
        .context("boot record task panicked")?
    }

    /// Remove a session's row, if any. Deleting an id with no matching row
    /// is success, not an error — `DELETE` affecting zero rows is simply
    /// what SQLite already does, not a promise this module has to keep on
    /// SPEC.md's behalf — and the supervisor's delete handler relies on
    /// exactly that to call this unconditionally rather than checking
    /// existence first (a check-then-delete would just be a second query
    /// racing nothing, since this connection is already serialized
    /// through one mutex).
    pub async fn delete_session(&self, id: &str) -> anyhow::Result<()> {
        let conn = Arc::clone(&self.conn);
        let id = id.to_string();
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let conn = conn.lock().expect("session db mutex poisoned");
            conn.execute("DELETE FROM sessions WHERE id = ?1", rusqlite::params![id])
                .context("deleting session row")?;
            Ok(())
        })
        .await
        .context("session delete task panicked")?
    }

    /// Load every persisted session, for `Supervisor::reload_sessions`
    /// (called both from construction and again from `serve`) to turn into
    /// `SessionEntry`s — live if tmux still knows the session, terminal-
    /// less (the restart gap) otherwise. Order is unspecified; the
    /// in-memory map this feeds is keyed by id anyway.
    pub async fn load_all(&self) -> anyhow::Result<Vec<StoredSession>> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<StoredSession>> {
            let conn = conn.lock().expect("session db mutex poisoned");
            let mut stmt = conn
                .prepare(
                    "SELECT id, title, cwd, invocation, tmux_name, pane, \
                     outcome_state, exit_code, annotation, error_detail FROM sessions",
                )
                .context("preparing session load query")?;
            // Two stages, not one: the outcome columns are reassembled by
            // `LastOutcome::from_columns`, which returns `anyhow::Error`
            // for a corrupt row and so cannot live inside a rusqlite row
            // mapper (whose error type is rusqlite's own). Collecting the
            // raw tuples first keeps the refusal-to-guess behavior and its
            // message intact instead of flattening it into a generic
            // decode failure.
            let raw = stmt
                .query_map([], |r| {
                    Ok((
                        StoredSession {
                            id: r.get(0)?,
                            title: r.get(1)?,
                            cwd: r.get(2)?,
                            invocation: r.get(3)?,
                            tmux_name: r.get(4)?,
                            pane: r.get(5)?,
                            outcome: LastOutcome::Launching,
                        },
                        r.get::<_, String>(6)?,
                        r.get::<_, Option<i32>>(7)?,
                        r.get::<_, Option<String>>(8)?,
                        r.get::<_, Option<String>>(9)?,
                    ))
                })
                .context("querying sessions")?
                .collect::<Result<Vec<_>, rusqlite::Error>>()
                .context("decoding session rows")?;
            raw.into_iter()
                .map(|(mut row, state, exit_code, annotation, error_detail)| {
                    row.outcome =
                        LastOutcome::from_columns(&state, exit_code, annotation, error_detail)
                            .with_context(|| format!("session {}", row.id))?;
                    Ok(row)
                })
                .collect()
        })
        .await
        .context("session load task panicked")?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The v1 schema, verbatim as this crate shipped it before M3 — the
    /// only way to produce a genuine pre-migration database to upgrade,
    /// since `apply_schema` itself no longer knows how to create one.
    /// Copied rather than referenced on purpose: a migration test that
    /// built its "old" fixture from today's code would silently stop
    /// testing the migration the moment today's code changed.
    const V1_SCHEMA: &str = "CREATE TABLE sessions (
             id         TEXT PRIMARY KEY,
             title      TEXT NOT NULL,
             cwd        TEXT NOT NULL,
             invocation TEXT NOT NULL,
             tmux_name  TEXT NOT NULL UNIQUE,
             pane       TEXT NOT NULL,
             created_at INTEGER NOT NULL
         ) STRICT;";

    /// Plant a schema-1 database holding `rows` — an M2-era database as it
    /// would be found on an upgrading host.
    ///
    /// Every column is given a DISTINCT value per row (not a shared
    /// literal), because the migration's contract is that it preserves
    /// data it never mentions: a rebuild-and-copy that dropped or
    /// transposed a column would still pass a check that only looked at
    /// ids.
    fn plant_v1_database(path: &Path, rows: &[(&str, &str, &str)]) {
        let conn = Connection::open(path).expect("create raw db");
        conn.execute_batch(V1_SCHEMA).expect("v1 schema");
        for (index, (id, tmux_name, pane)) in rows.iter().enumerate() {
            conn.execute(
                "INSERT INTO sessions (id, title, cwd, invocation, tmux_name, pane, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    id,
                    format!("title-{id}"),
                    format!("/work/{id}"),
                    format!("agent --for {id}"),
                    tmux_name,
                    pane,
                    1_700_000_000 + index as i64,
                ],
            )
            .expect("insert v1 row");
        }
        conn.pragma_update(None, "user_version", 1).expect("stamp");
    }

    /// A store on a fresh temp database, with the temp directory returned
    /// alongside it because dropping it would delete the database out from
    /// under the store.
    async fn fresh_store() -> (tempfile::TempDir, SessionStore) {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = SessionStore::open(&dir.path().join("supervisor.db"), true)
            .await
            .expect("open");
        (dir, store)
    }

    /// Seed a session in the shape a confirmed launch leaves behind: a
    /// pane and `Running`. The starting point for every transition test,
    /// since that is the state a real session spends its life in.
    async fn insert_running(store: &SessionStore, id: &str) {
        store
            .insert_session(StoredSession {
                id: id.to_string(),
                title: id.to_string(),
                cwd: "/tmp/work".to_string(),
                invocation: "agent".to_string(),
                tmux_name: format!("fh-{id}"),
                pane: "%0".to_string(),
                outcome: LastOutcome::Running,
            })
            .await
            .expect("insert");
    }

    /// Force a session to `outcome` regardless of what the transition
    /// policy would allow — a test fixture, not a code path: seeding an
    /// `Interrupted` or `Error` row through legal transitions alone is
    /// either impossible (nothing writes `Error` yet) or would smuggle the
    /// behavior under test into the setup.
    fn force_outcome(store: &SessionStore, id: &str, outcome: &LastOutcome) {
        let conn = store.conn.lock().expect("db mutex");
        let (state, code, annotation, detail) = outcome.columns();
        conn.execute(
            "UPDATE sessions SET outcome_state = ?2, exit_code = ?3, annotation = ?4, \
             error_detail = ?5 WHERE id = ?1",
            rusqlite::params![id, state, code, annotation, detail],
        )
        .expect("force outcome");
    }

    /// Read one session's outcome back THROUGH the on-disk round trip, so
    /// every assertion in this module is about what a later process would
    /// see rather than about an in-memory value that never reached SQLite.
    async fn outcome_of(store: &SessionStore, id: &str) -> LastOutcome {
        store
            .load_all()
            .await
            .expect("load")
            .into_iter()
            .find(|row| row.id == id)
            .unwrap_or_else(|| panic!("session {id} must still exist"))
            .outcome
    }

    /// Every column of every table, as SQLite itself describes it:
    /// (table, column, type, notnull, default, pk).
    fn columns_of(path: &Path) -> Vec<(String, String, String, i64, Option<String>, i64)> {
        let conn = Connection::open(path).expect("open raw");
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_schema WHERE type = 'table' ORDER BY name")
            .expect("prepare")
            .query_map([], |r| r.get(0))
            .expect("query")
            .collect::<Result<_, _>>()
            .expect("collect");
        let mut out = Vec::new();
        for table in tables {
            let mut stmt = conn
                .prepare(&format!("PRAGMA table_info({table})"))
                .expect("prepare table_info");
            let rows = stmt
                .query_map([], |r| {
                    Ok((
                        table.clone(),
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, i64>(3)?,
                        r.get::<_, Option<String>>(4)?,
                        r.get::<_, i64>(5)?,
                    ))
                })
                .expect("query table_info")
                .collect::<Result<Vec<_>, _>>()
                .expect("collect table_info");
            out.extend(rows);
        }
        out
    }

    /// The transition policy in one table, because the RELATIONSHIPS are
    /// the contract: each rule looks obvious alone, and only side by side
    /// do the two that matter stand out — a stop in flight owning the
    /// annotation no matter which observer commits first, and terminal
    /// outcomes accepting enrichment but never reclassification.
    ///
    /// Pure and exhaustive by construction: every transition is tried
    /// against every state, so a new variant on either side fails to
    /// compile rather than silently going unexercised.
    #[test]
    fn transition_policy_covers_every_state_and_transition() {
        let stopped = || Some(farhelm_proto::STOP_ANNOTATION.to_string());
        let exited = |code, annotation| LastOutcome::Exited {
            exit_code: code,
            annotation,
        };
        let cases: Vec<(LastOutcome, Transition, Option<LastOutcome>)> = vec![
            // A confirmed launch fills in a launching row and clears a
            // stop intent whose kill evidently never landed.
            (
                LastOutcome::Launching,
                Transition::ConfirmRunning { pane: "%1".into() },
                Some(LastOutcome::Running),
            ),
            (
                LastOutcome::StopRequested,
                Transition::ConfirmRunning { pane: "%1".into() },
                Some(LastOutcome::Running),
            ),
            // ...but never reopens a session that already ended.
            (
                exited(Some(1), None),
                Transition::ConfirmRunning { pane: "%1".into() },
                None,
            ),
            (
                LastOutcome::Interrupted,
                Transition::ConfirmRunning { pane: "%1".into() },
                None,
            ),
            // Re-confirming an unchanged running session writes nothing.
            (
                LastOutcome::Running,
                Transition::ConfirmRunning { pane: "%1".into() },
                None,
            ),
            // A stop is only requestable against something still live.
            (
                LastOutcome::Running,
                Transition::StopRequested,
                Some(LastOutcome::StopRequested),
            ),
            (exited(None, None), Transition::StopRequested, None),
            // THE race: a list observing the pane die mid-stop yields the
            // annotated exit without knowing a stop was in flight.
            (
                LastOutcome::StopRequested,
                Transition::ObservedExit { exit_code: Some(0) },
                Some(exited(Some(0), stopped())),
            ),
            // A plain observed exit stays plain.
            (
                LastOutcome::Running,
                Transition::ObservedExit { exit_code: Some(3) },
                Some(exited(Some(3), None)),
            ),
            // Monotonic enrichment: a code fills a gap, never the reverse.
            (
                exited(None, None),
                Transition::ObservedExit { exit_code: Some(3) },
                Some(exited(Some(3), None)),
            ),
            (
                exited(Some(3), None),
                Transition::ObservedExit { exit_code: None },
                None,
            ),
            (
                exited(Some(3), stopped()),
                Transition::ObservedExit { exit_code: Some(9) },
                None,
            ),
            // A completed stop annotates, keeping a code already observed.
            (
                LastOutcome::StopRequested,
                Transition::StopCompleted { exit_code: None },
                Some(exited(None, stopped())),
            ),
            (
                exited(Some(143), None),
                Transition::StopCompleted { exit_code: None },
                Some(exited(Some(143), stopped())),
            ),
            (
                exited(Some(143), stopped()),
                Transition::StopCompleted { exit_code: None },
                None,
            ),
            // Reload's rediscovered pane, dead: plain from running,
            // annotated from an intent that evidently did land.
            (
                LastOutcome::Launching,
                Transition::RediscoveredExit {
                    pane: "%2".into(),
                    exit_code: Some(7),
                },
                Some(exited(Some(7), None)),
            ),
            (
                LastOutcome::StopRequested,
                Transition::RediscoveredExit {
                    pane: "%2".into(),
                    exit_code: None,
                },
                Some(exited(None, stopped())),
            ),
            // Nothing reclassifies a reboot or a failed exec.
            (
                LastOutcome::Interrupted,
                Transition::ObservedExit { exit_code: Some(1) },
                None,
            ),
            (
                LastOutcome::Interrupted,
                Transition::StopCompleted { exit_code: Some(1) },
                None,
            ),
            (
                LastOutcome::Error {
                    detail: "ENOENT".into(),
                },
                Transition::ObservedExit { exit_code: Some(1) },
                None,
            ),
            (
                LastOutcome::Error {
                    detail: "ENOENT".into(),
                },
                Transition::StopCompleted { exit_code: None },
                None,
            ),
            // A sentinel reclassifies every non-terminal state to `Error`
            // — "never started" outranks the inference each of these
            // states would otherwise be probed against (PLAN_M3.md item
            // 3).
            (
                LastOutcome::Launching,
                Transition::SentinelError {
                    detail: "exec_failed errno=2".into(),
                    pane: None,
                },
                Some(LastOutcome::Error {
                    detail: "exec_failed errno=2".into(),
                }),
            ),
            (
                LastOutcome::Running,
                Transition::SentinelError {
                    detail: "exec_failed errno=13".into(),
                    pane: Some("%3".into()),
                },
                Some(LastOutcome::Error {
                    detail: "exec_failed errno=13".into(),
                }),
            ),
            (
                LastOutcome::StopRequested,
                Transition::SentinelError {
                    detail: "exec_failed errno=2".into(),
                    pane: None,
                },
                Some(LastOutcome::Error {
                    detail: "exec_failed errno=2".into(),
                }),
            ),
            // ...and — the sharper half of PLAN_M3.md item 3 — a sentinel
            // discovered LATE still wins against an `Interrupted` or an
            // UNANNOTATED `Exited`, because both are themselves only
            // inferences from an ordinary dead-or-vanished pane: exactly
            // the evidence a sentinel is defined to outrank. This is what
            // stops a stop or a list from locking in a wrong classification
            // by committing an inferred exit before anything ever read the
            // sentinel file.
            (
                exited(Some(0), None),
                Transition::SentinelError {
                    detail: "exec_failed errno=2".into(),
                    pane: None,
                },
                Some(LastOutcome::Error {
                    detail: "exec_failed errno=2".into(),
                }),
            ),
            (
                LastOutcome::Interrupted,
                Transition::SentinelError {
                    detail: "exec_failed errno=2".into(),
                    pane: None,
                },
                Some(LastOutcome::Error {
                    detail: "exec_failed errno=2".into(),
                }),
            ),
            // ...but a GENUINELY annotated exit (a real stop) is retained
            // knowledge, not an inference — a sentinel and a stop
            // annotation cannot both be true of the same launch — so this
            // is the one terminal state a sentinel still cannot cross. An
            // already-`Error` row is simply unchanged (idempotent).
            (
                exited(Some(0), stopped()),
                Transition::SentinelError {
                    detail: "exec_failed errno=2".into(),
                    pane: None,
                },
                None,
            ),
            (
                LastOutcome::Error {
                    detail: "ENOENT".into(),
                },
                Transition::SentinelError {
                    detail: "exec_failed errno=2".into(),
                    pane: None,
                },
                None,
            ),
        ];
        for (current, transition, expected) in cases {
            assert_eq!(
                transition.apply(&current),
                expected,
                "{transition:?} against {current:?}"
            );
        }
    }

    /// The upgrade path M3 owes every host that already ran M2: a
    /// schema-1 database must open in place, keep every session it held
    /// (nothing recreated, nothing dropped, no column silently lost), and
    /// land its rows on the CONSERVATIVE outcome. `Launching` — not
    /// `Running` — is the required answer: an M2-era row records nothing
    /// about whether its agent was still alive, and adopting `Running`
    /// would both lead reality and make the next reboot report these
    /// sessions as interrupted on no evidence at all.
    ///
    /// `created_at` is asserted directly through SQL because
    /// `StoredSession` deliberately does not carry it — a migration that
    /// dropped it would be invisible to every other test in this file.
    ///
    /// The migrated database must also have NO stored boot id, which is
    /// what makes its first M3 startup take the same-boot path rather
    /// than claiming a reboot it has no evidence for (PLAN_M3.md item 2).
    #[tokio::test]
    async fn schema_1_database_migrates_in_place_preserving_every_session() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("supervisor.db");
        plant_v1_database(&db_path, &[("s1", "fh-1", "%0"), ("s2", "fh-2", "%1")]);

        let store = SessionStore::open(&db_path, true)
            .await
            .expect("migrating open");
        let mut rows = store.load_all().await.expect("load");
        rows.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(
            rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            vec!["s1", "s2"],
            "the migration must preserve every M2-era session"
        );
        assert_eq!(rows[0].title, "title-s1");
        assert_eq!(rows[0].cwd, "/work/s1");
        assert_eq!(rows[0].invocation, "agent --for s1");
        assert_eq!(rows[0].tmux_name, "fh-1");
        assert_eq!(rows[0].pane, "%0");
        assert_eq!(rows[1].tmux_name, "fh-2");
        assert_eq!(rows[1].pane, "%1");
        assert!(rows.iter().all(|r| r.outcome == LastOutcome::Launching));
        assert_eq!(
            store.boot_id().await.expect("boot id"),
            None,
            "a migrated database must not pretend to know which boot it came from"
        );

        let created_at: Vec<i64> = {
            let conn = store.conn.lock().expect("db mutex");
            let mut stmt = conn
                .prepare("SELECT created_at FROM sessions ORDER BY id")
                .expect("prepare");
            stmt.query_map([], |r| r.get(0))
                .expect("query")
                .collect::<Result<_, _>>()
                .expect("collect")
        };
        assert_eq!(
            created_at,
            vec![1_700_000_000, 1_700_000_001],
            "a column this build never reads must still survive the migration untouched"
        );
    }

    /// The migration must be DURABLE, not merely applied in memory: a
    /// forgotten `user_version` bump would leave the second open trying to
    /// migrate an already-migrated database, whose `ALTER TABLE` would
    /// fail on the duplicate column — so reopening is what proves the
    /// version was actually committed alongside the columns.
    #[tokio::test]
    async fn a_migrated_database_reopens_at_the_new_version() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("supervisor.db");
        plant_v1_database(&db_path, &[("s1", "fh-1", "%0")]);
        SessionStore::open(&db_path, true).await.expect("migrate");

        let reopened = SessionStore::open(&db_path, true).await.expect("reopen");
        assert_eq!(reopened.load_all().await.expect("load").len(), 1);
        let version: i64 = {
            let conn = reopened.conn.lock().expect("db mutex");
            conn.query_row("PRAGMA user_version", [], |r| r.get(0))
                .expect("read version")
        };
        assert_eq!(version, SCHEMA_VERSION);
    }

    /// A migration that cannot finish must leave NOTHING behind — the
    /// whole point of running it in one transaction with its own version
    /// bump. Provoked with a `supervisor_meta` table already present (a
    /// database some other tool or a half-finished upgrade touched), which
    /// makes the migration's `CREATE TABLE` fail partway through, AFTER
    /// its `ALTER TABLE`s have already run.
    ///
    /// The database must still be a working version 1: same version, no
    /// new columns. Without the transaction it would be a hybrid — new
    /// columns, old version — that the next open would try to migrate
    /// again and fail on forever.
    #[tokio::test]
    async fn a_failed_migration_leaves_the_database_at_its_old_version() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("supervisor.db");
        plant_v1_database(&db_path, &[("s1", "fh-1", "%0")]);
        {
            let conn = Connection::open(&db_path).expect("raw open");
            conn.execute_batch("CREATE TABLE supervisor_meta (nonsense TEXT);")
                .expect("plant a conflicting table");
        }

        SessionStore::open(&db_path, true)
            .await
            .expect_err("the migration must fail on the conflicting table");

        let conn = Connection::open(&db_path).expect("raw open");
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .expect("read version");
        assert_eq!(
            version, 1,
            "a rolled-back migration must not claim version 2"
        );
        let has_outcome_column = conn.prepare("SELECT outcome_state FROM sessions").is_ok();
        assert!(
            !has_outcome_column,
            "the ALTER TABLEs must have rolled back with the rest of the migration"
        );
    }

    /// Upgrading a database that belongs to a RUNNING supervisor is
    /// destructive, not neighbourly: that supervisor is an older build, so
    /// the moment it restarts it will refuse the schema this process
    /// wrote. A process without the state directory's claim must therefore
    /// refuse to migrate at all — while still opening a database that is
    /// already current, since a supervisor may legitimately read and serve
    /// during a handoff.
    #[tokio::test]
    async fn opening_without_the_right_to_migrate_refuses_an_old_database() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("supervisor.db");
        plant_v1_database(&db_path, &[("s1", "fh-1", "%0")]);

        let err = SessionStore::open(&db_path, false)
            .await
            .expect_err("an unclaimed state dir must not be upgraded");
        assert!(
            format!("{err:#}").contains("another supervisor"),
            "the refusal must say why: {err:#}"
        );
        let conn = Connection::open(&db_path).expect("raw open");
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .expect("read version");
        assert_eq!(version, 1, "the incumbent's database must be untouched");

        // An already-current database opens fine without the right to
        // migrate: there is nothing to change.
        SessionStore::open(&db_path, true).await.expect("migrate");
        SessionStore::open(&db_path, false)
            .await
            .expect("a current database needs no migration rights");
    }

    /// A migrated database and a freshly created one must end up with the
    /// SAME schema, or every later migration has two divergent starting
    /// points to reason about — the classic way a migration ladder rots.
    ///
    /// Scope, stated rather than implied: this compares `PRAGMA table_info`
    /// (tables, column order, type, nullability, DEFAULT, primary key) and
    /// separately asserts the properties that pragma does NOT report and
    /// that this schema depends on — `STRICT` typing and
    /// `supervisor_meta`'s single-row `CHECK`. DDL text is deliberately
    /// not compared: `ALTER TABLE ADD COLUMN` produces differently
    /// punctuated SQL for an identical result.
    #[tokio::test]
    async fn migrated_and_fresh_schemas_agree() {
        let dir = tempfile::tempdir().expect("tempdir");
        let migrated = dir.path().join("migrated.db");
        plant_v1_database(&migrated, &[]);
        SessionStore::open(&migrated, true).await.expect("migrate");
        let fresh = dir.path().join("fresh.db");
        SessionStore::open(&fresh, true).await.expect("create");

        assert_eq!(columns_of(&migrated), columns_of(&fresh));

        for path in [&migrated, &fresh] {
            let conn = Connection::open(path).expect("open raw");
            let ddl: Vec<String> = conn
                .prepare("SELECT sql FROM sqlite_schema WHERE type = 'table' ORDER BY name")
                .expect("prepare")
                .query_map([], |r| r.get(0))
                .expect("query")
                .collect::<Result<_, _>>()
                .expect("collect");
            assert!(
                ddl.iter().all(|sql| sql.contains("STRICT")),
                "both tables must keep STRICT typing in {}: {ddl:?}",
                path.display()
            );
            assert!(
                ddl.iter().any(|sql| sql.contains("CHECK")),
                "supervisor_meta must keep its single-row CHECK in {}",
                path.display()
            );
            // The CHECK is not decoration: it is what makes "the metadata
            // row" a single row rather than a convention.
            conn.execute(
                "INSERT INTO supervisor_meta (id, boot_id) VALUES (1, 'x')",
                [],
            )
            .expect_err("a second metadata row must be refused");
        }
    }

    /// Every outcome shape must survive the on-disk round trip — the stop
    /// annotation and the exit code especially, since those are exactly
    /// what SPEC.md promises a user still sees after a supervisor restart
    /// or a reboot. `Error`'s detail rides along even though nothing
    /// writes that state yet (PLAN_M3.md item 3 does), so the column is
    /// proven before its writer exists rather than after.
    #[tokio::test]
    async fn every_outcome_shape_round_trips() {
        let (_dir, store) = fresh_store().await;
        insert_running(&store, "s1").await;
        for outcome in [
            LastOutcome::Launching,
            LastOutcome::Running,
            LastOutcome::StopRequested,
            LastOutcome::Exited {
                exit_code: Some(7),
                annotation: None,
            },
            LastOutcome::Exited {
                exit_code: None,
                annotation: Some(farhelm_proto::STOP_ANNOTATION.to_string()),
            },
            LastOutcome::Interrupted,
            LastOutcome::Error {
                detail: "No such file or directory".to_string(),
            },
        ] {
            force_outcome(&store, "s1", &outcome);
            assert_eq!(outcome_of(&store, "s1").await, outcome);
        }
    }

    /// The committed-result contract: `transition` returns what is
    /// actually stored afterwards, which is what callers copy into their
    /// in-memory mirror. A refused transition must therefore return the
    /// UNCHANGED outcome rather than nothing — a caller that treated
    /// "refused" as "no answer" would leave its mirror stale.
    #[tokio::test]
    async fn transition_returns_the_committed_outcome_even_when_it_refuses() {
        let (_dir, store) = fresh_store().await;
        insert_running(&store, "s1").await;
        force_outcome(&store, "s1", &LastOutcome::Interrupted);

        let committed = store
            .transition("s1", Transition::ObservedExit { exit_code: Some(1) })
            .await
            .expect("transition");
        assert_eq!(committed, Some(LastOutcome::Interrupted));
        assert_eq!(outcome_of(&store, "s1").await, LastOutcome::Interrupted);

        // A row deleted concurrently is not an error and has no committed
        // outcome to report.
        store.delete_session("s1").await.expect("delete");
        assert_eq!(
            store
                .transition("s1", Transition::ObservedExit { exit_code: None })
                .await
                .expect("transition"),
            None
        );
    }

    /// `ConfirmRunning` is the launch-confirmed transition: it must fill
    /// in the pane a launching row could not know yet AND move the
    /// outcome, since reload treats a running row without a pane as an
    /// unconfirmed launch to go hunting for.
    #[tokio::test]
    async fn confirming_a_launch_fills_in_the_pane_and_moves_the_outcome() {
        let (_dir, store) = fresh_store().await;
        store
            .insert_session(StoredSession {
                id: "s1".to_string(),
                title: "demo".to_string(),
                cwd: "/tmp/work".to_string(),
                invocation: "agent".to_string(),
                tmux_name: "fh-1".to_string(),
                pane: String::new(),
                outcome: LastOutcome::Launching,
            })
            .await
            .expect("insert");

        store
            .transition("s1", Transition::ConfirmRunning { pane: "%4".into() })
            .await
            .expect("confirm");
        let rows = store.load_all().await.expect("load");
        assert_eq!(rows[0].pane, "%4");
        assert_eq!(rows[0].outcome, LastOutcome::Running);
    }

    /// A rediscovered dead pane commits the pane AND the exit it evidences
    /// in one operation. Split across two writes, a crash in between would
    /// leave a pane recorded under a still-`Running` row — which the next
    /// reload reads as a live session and, finding the pane dead again,
    /// only then records: harmless once, but the same window reopens on
    /// every restart, and a reboot landing in it interrupts a session that
    /// had actually exited.
    #[tokio::test]
    async fn a_rediscovered_dead_pane_commits_the_pane_with_its_outcome() {
        let (_dir, store) = fresh_store().await;
        store
            .insert_session(StoredSession {
                id: "s1".to_string(),
                title: "demo".to_string(),
                cwd: "/tmp/work".to_string(),
                invocation: "agent".to_string(),
                tmux_name: "fh-1".to_string(),
                pane: String::new(),
                outcome: LastOutcome::Launching,
            })
            .await
            .expect("insert");

        store
            .transition(
                "s1",
                Transition::RediscoveredExit {
                    pane: "%9".into(),
                    exit_code: Some(2),
                },
            )
            .await
            .expect("rediscover");
        let rows = store.load_all().await.expect("load");
        assert_eq!(rows[0].pane, "%9");
        assert_eq!(
            rows[0].outcome,
            LastOutcome::Exited {
                exit_code: Some(2),
                annotation: None
            }
        );
    }

    /// The list-versus-stop race, at the store boundary where it is
    /// decided: whichever observer commits first, the stop annotation
    /// survives. This is the concrete loss seven review lenses converged
    /// on — a list observing the pane die mid-stop used to write a plain
    /// exit over the annotation, so a session the user stopped listed as
    /// one that merely finished.
    ///
    /// Both orders are exercised because the fix is not "the stop wins" —
    /// it is that the STATE, not the caller, decides: a plain exit
    /// observed from a stop-in-flight row is annotated on the spot, and a
    /// completed stop annotates an exit already recorded.
    #[tokio::test]
    async fn a_stop_annotation_survives_a_concurrent_plain_exit_in_either_order() {
        let (_dir, store) = fresh_store().await;

        // Order A: the stop's intent lands first, then the list observes
        // the pane die and knows nothing about the stop.
        insert_running(&store, "a").await;
        store
            .transition("a", Transition::StopRequested)
            .await
            .expect("intent");
        store
            .transition("a", Transition::ObservedExit { exit_code: Some(0) })
            .await
            .expect("list observation");
        assert_eq!(
            outcome_of(&store, "a").await,
            LastOutcome::Exited {
                exit_code: Some(0),
                annotation: Some(farhelm_proto::STOP_ANNOTATION.to_string()),
            }
        );

        // Order B: the list gets there first with a plain exit, and the
        // stop's completion annotates it afterwards without losing the
        // code the list had already captured.
        insert_running(&store, "b").await;
        store
            .transition("b", Transition::ObservedExit { exit_code: Some(0) })
            .await
            .expect("list observation");
        store
            .transition("b", Transition::StopCompleted { exit_code: None })
            .await
            .expect("stop completion");
        assert_eq!(
            outcome_of(&store, "b").await,
            LastOutcome::Exited {
                exit_code: Some(0),
                annotation: Some(farhelm_proto::STOP_ANNOTATION.to_string()),
            }
        );
    }

    /// The other half of the same rule, against REAL rows rather than the
    /// pure policy table: a completed stop must not paint over an outcome
    /// it did not cause. Seeded through the store and re-read after, so
    /// what is asserted is what a later process would load.
    #[tokio::test]
    async fn a_completed_stop_leaves_outcomes_it_did_not_cause_alone() {
        let (_dir, store) = fresh_store().await;
        let annotated = LastOutcome::Exited {
            exit_code: Some(1),
            annotation: Some("stopped by user".to_string()),
        };
        let error = LastOutcome::Error {
            detail: "ENOENT".to_string(),
        };
        for (id, seeded) in [
            (
                "plain",
                LastOutcome::Exited {
                    exit_code: Some(5),
                    annotation: None,
                },
            ),
            ("annotated", annotated.clone()),
            ("interrupted", LastOutcome::Interrupted),
            ("error", error.clone()),
        ] {
            insert_running(&store, id).await;
            force_outcome(&store, id, &seeded);
            store
                .transition(id, Transition::StopCompleted { exit_code: None })
                .await
                .expect("stop");
        }

        assert_eq!(
            outcome_of(&store, "plain").await,
            LastOutcome::Exited {
                exit_code: Some(5),
                annotation: Some(farhelm_proto::STOP_ANNOTATION.to_string()),
            },
            "only the plain exit gains the annotation, keeping its code"
        );
        assert_eq!(outcome_of(&store, "annotated").await, annotated);
        assert_eq!(
            outcome_of(&store, "interrupted").await,
            LastOutcome::Interrupted
        );
        assert_eq!(outcome_of(&store, "error").await, error);
    }

    /// The reboot conversion in its normal (uninterrupted) form: a changed
    /// boot id stores the new id AND converts every still-live session —
    /// launching, running, or with a stop in flight — to interrupted,
    /// while leaving already-ended sessions completely alone. SPEC.md's
    /// retained knowledge is not something a reboot gets to overwrite.
    ///
    /// A stop that was mid-sweep is interrupted rather than annotated on
    /// purpose: the reboot destroyed the evidence of whether the kill ever
    /// landed, so "stopped by user" would be a claim about a process
    /// nothing observed.
    #[tokio::test]
    async fn record_boot_interrupts_live_sessions_and_spares_ended_ones() {
        let (_dir, store) = fresh_store().await;
        for id in ["live", "starting", "stopping", "done", "failed", "already"] {
            insert_running(&store, id).await;
        }
        force_outcome(&store, "starting", &LastOutcome::Launching);
        force_outcome(&store, "stopping", &LastOutcome::StopRequested);
        let done = LastOutcome::Exited {
            exit_code: Some(3),
            annotation: Some(farhelm_proto::STOP_ANNOTATION.to_string()),
        };
        force_outcome(&store, "done", &done);
        let failed = LastOutcome::Error {
            detail: "Permission denied".to_string(),
        };
        force_outcome(&store, "failed", &failed);
        force_outcome(&store, "already", &LastOutcome::Interrupted);

        store
            .record_boot("boot-b", true, HashMap::new(), None)
            .await
            .expect("record boot");

        for id in ["live", "starting", "stopping", "already"] {
            assert_eq!(
                outcome_of(&store, id).await,
                LastOutcome::Interrupted,
                "{id} was live when the host rebooted"
            );
        }
        assert_eq!(outcome_of(&store, "done").await, done);
        assert_eq!(
            outcome_of(&store, "failed").await,
            failed,
            "an exec failure is not something a reboot reclassifies"
        );
        assert_eq!(
            store.boot_id().await.expect("boot id").as_deref(),
            Some("boot-b")
        );
    }

    /// PLAN_M3.md item 3's precedence, pinned at the exact boundary it is
    /// hardest to get right: a row about to be blanket-converted to
    /// `Interrupted` by THIS SAME call must land on `Error` instead when a
    /// sentinel override names it, and every OTHER live row still becomes
    /// `Interrupted` normally. Without `record_boot` applying overrides
    /// before its blanket `UPDATE` (see that method's own docs), this row
    /// would already be `Interrupted` — and therefore terminal and immune
    /// to any further reclassification — by the time a normal
    /// `SentinelError` transition could ever reach it.
    #[tokio::test]
    async fn record_boot_prefers_a_sentinel_override_over_interrupting_that_row() {
        let (_dir, store) = fresh_store().await;
        insert_running(&store, "crashed-launch").await;
        force_outcome(&store, "crashed-launch", &LastOutcome::Launching);
        insert_running(&store, "plain-live").await;

        let mut overrides = HashMap::new();
        overrides.insert(
            "crashed-launch".to_string(),
            "exec_failed argv0=/nope errno=2".to_string(),
        );
        store
            .record_boot("boot-b", true, overrides, None)
            .await
            .expect("record boot with a sentinel override");

        assert_eq!(
            outcome_of(&store, "crashed-launch").await,
            LastOutcome::Error {
                detail: "exec_failed argv0=/nope errno=2".to_string()
            },
            "a sentinel-bearing row must classify error, never interrupted, even though a \
             reboot happened"
        );
        assert_eq!(
            outcome_of(&store, "plain-live").await,
            LastOutcome::Interrupted,
            "a row with no override still gets the ordinary reboot conversion"
        );
    }

    /// The override must win regardless of WHICH non-terminal state a row
    /// was in when the reboot landed — `Launching` (the crash-before-
    /// confirmation case above), but also `Running` (the ordinary case: a
    /// launch that DID confirm before its agent's exec failed) and
    /// `StopRequested` (a stop whose kill sweep never got to report back).
    /// All three are states `record_boot`'s blanket conversion would
    /// otherwise sweep into `Interrupted` in one `UPDATE`, so the override
    /// has to beat that same statement from every one of them, not just
    /// the one case the test above happens to pin.
    #[tokio::test]
    async fn record_boot_override_wins_from_every_non_terminal_state() {
        for seed in [
            LastOutcome::Launching,
            LastOutcome::Running,
            LastOutcome::StopRequested,
        ] {
            let (_dir, store) = fresh_store().await;
            insert_running(&store, "row").await;
            force_outcome(&store, "row", &seed);

            let mut overrides = HashMap::new();
            overrides.insert("row".to_string(), "exec_failed errno=2".to_string());
            store
                .record_boot("boot-b", true, overrides, None)
                .await
                .expect("record boot with a sentinel override");

            assert_eq!(
                outcome_of(&store, "row").await,
                LastOutcome::Error {
                    detail: "exec_failed errno=2".to_string()
                },
                "a sentinel override must win starting from {seed:?}"
            );
        }
    }

    /// PLAN_M3.md item 3's crash-boundary sibling to the item-2 test above:
    /// a fault injected AFTER the sentinel overrides have been applied but
    /// BEFORE the blanket interrupt conversion runs must roll back BOTH —
    /// the override included — not just the conversion. Without this, a
    /// crash at exactly that instant could leave an override applied (a row
    /// durably `error`) while the boot id itself never committed, and the
    /// next startup — seeing the OLD boot id again — would redo the whole
    /// reboot classification from a store that already disagreed with
    /// itself about which sessions were live.
    #[tokio::test]
    async fn a_fault_between_overrides_and_the_blanket_conversion_rolls_back_both() {
        let (_dir, store) = fresh_store().await;
        insert_running(&store, "sentinel-row").await;
        force_outcome(&store, "sentinel-row", &LastOutcome::Launching);
        insert_running(&store, "plain-row").await;

        let mut overrides = HashMap::new();
        overrides.insert(
            "sentinel-row".to_string(),
            "exec_failed errno=2".to_string(),
        );
        let fault: BootTxFault = Arc::new(|| anyhow::bail!("simulated crash after overrides"));
        let err = store
            .record_boot("boot-b", true, overrides.clone(), Some(fault))
            .await
            .expect_err("the injected failure must fail the call");
        assert!(format!("{err:#}").contains("simulated crash"));

        assert_eq!(
            store.boot_id().await.expect("boot id"),
            None,
            "a rolled-back transaction must not leave the new boot id behind either"
        );
        assert_eq!(
            outcome_of(&store, "sentinel-row").await,
            LastOutcome::Launching,
            "the override itself must roll back, not just the blanket conversion"
        );
        assert_eq!(
            outcome_of(&store, "plain-row").await,
            LastOutcome::Running,
            "no half-applied conversion"
        );

        // The retry, with no injected failure: both the override and the
        // ordinary conversion land together.
        store
            .record_boot("boot-b", true, overrides, None)
            .await
            .expect("retry");
        assert_eq!(
            outcome_of(&store, "sentinel-row").await,
            LastOutcome::Error {
                detail: "exec_failed errno=2".to_string()
            }
        );
        assert_eq!(
            outcome_of(&store, "plain-row").await,
            LastOutcome::Interrupted
        );
    }

    /// The store persists a `SentinelError`'s pane alongside its outcome in
    /// ONE statement, exactly like `RediscoveredExit` — a `Launching` row's
    /// pane is empty until something writes it, and a crash between an
    /// outcome write and a separate pane write would resurrect the row as
    /// `Running` (empty pane) on the next reload. Pinned at the store level
    /// (not just via `Transition::apply`'s pure function, which never
    /// touches SQLite) because `Transition::pane`'s wiring into the actual
    /// `UPDATE` statement is exactly what a pure-function test cannot
    /// exercise.
    #[tokio::test]
    async fn sentinel_error_commits_its_pane_with_its_outcome() {
        let (_dir, store) = fresh_store().await;
        insert_running(&store, "row").await;
        force_outcome(&store, "row", &LastOutcome::Launching);

        store
            .transition(
                "row",
                Transition::SentinelError {
                    detail: "exec_failed errno=2".to_string(),
                    pane: Some("%9".to_string()),
                },
            )
            .await
            .expect("transition");

        let row = store
            .load_all()
            .await
            .expect("load")
            .into_iter()
            .find(|r| r.id == "row")
            .expect("row must still exist");
        assert_eq!(
            row.outcome,
            LastOutcome::Error {
                detail: "exec_failed errno=2".to_string()
            }
        );
        assert_eq!(
            row.pane, "%9",
            "the pane must commit in the SAME statement as the Error outcome"
        );
    }

    /// PLAN_M3.md item 3's addition-18 regression: a sentinel-bearing row
    /// that was ALREADY (mis)classified as an inferred `Exited` or
    /// `Interrupted` — reachable in this unmerged stack's own dev databases
    /// from before the sentinel reader existed, since PR4 shipped
    /// `Interrupted`/inferred-`Exited` writers with no reader yet to
    /// supersede them — must still reclassify to `Error` once a later pass
    /// reads the sentinel. Seeded both ways in one test since both are the
    /// same rule.
    #[tokio::test]
    async fn a_sentinel_reclassifies_a_row_already_recorded_as_inferred_exited_or_interrupted() {
        let (_dir, store) = fresh_store().await;
        insert_running(&store, "already-exited").await;
        force_outcome(
            &store,
            "already-exited",
            &LastOutcome::Exited {
                exit_code: Some(1),
                annotation: None,
            },
        );
        insert_running(&store, "already-interrupted").await;
        force_outcome(&store, "already-interrupted", &LastOutcome::Interrupted);

        for id in ["already-exited", "already-interrupted"] {
            store
                .transition(
                    id,
                    Transition::SentinelError {
                        detail: "exec_failed errno=2".to_string(),
                        pane: None,
                    },
                )
                .await
                .expect("transition");
            assert_eq!(
                outcome_of(&store, id).await,
                LastOutcome::Error {
                    detail: "exec_failed errno=2".to_string()
                },
                "{id} must reclassify from an INFERRED terminal state to Error"
            );
        }
    }

    /// The one terminal state a sentinel still cannot cross: a GENUINE stop
    /// annotation, because it is retained knowledge (a real run was
    /// observed alive) rather than an inference a sentinel could outrank.
    #[tokio::test]
    async fn a_sentinel_never_overrides_a_genuinely_annotated_exit() {
        let (_dir, store) = fresh_store().await;
        insert_running(&store, "stopped").await;
        force_outcome(
            &store,
            "stopped",
            &LastOutcome::Exited {
                exit_code: Some(0),
                annotation: Some(farhelm_proto::STOP_ANNOTATION.to_string()),
            },
        );

        let committed = store
            .transition(
                "stopped",
                Transition::SentinelError {
                    detail: "exec_failed errno=2".to_string(),
                    pane: None,
                },
            )
            .await
            .expect("transition");
        assert_eq!(
            committed,
            Some(LastOutcome::Exited {
                exit_code: Some(0),
                annotation: Some(farhelm_proto::STOP_ANNOTATION.to_string()),
            }),
            "a genuinely annotated exit must survive a sentinel unchanged"
        );
    }

    /// The same-boot half of the same call: adopting a boot id (a pre-M3
    /// database's first M3 startup) must store it and change NOTHING else.
    /// A conversion that ran unconditionally would interrupt every live
    /// session on a host that never rebooted.
    #[tokio::test]
    async fn record_boot_without_a_reboot_leaves_every_outcome_untouched() {
        let (_dir, store) = fresh_store().await;
        insert_running(&store, "live").await;
        insert_running(&store, "starting").await;
        force_outcome(&store, "starting", &LastOutcome::Launching);

        store
            .record_boot("boot-a", false, HashMap::new(), None)
            .await
            .expect("adopt");

        assert_eq!(outcome_of(&store, "live").await, LastOutcome::Running);
        assert_eq!(outcome_of(&store, "starting").await, LastOutcome::Launching);
        assert_eq!(
            store.boot_id().await.expect("boot id").as_deref(),
            Some("boot-a")
        );
    }

    /// The crash boundary PLAN_M3.md item 2 requires pinned: a failure
    /// BETWEEN the boot-id write and the interrupted conversion must roll
    /// BOTH back. If the id survived alone, the next startup would compare
    /// against it, conclude same-boot, and quietly misclassify every
    /// session the reboot really did interrupt — permanently, since
    /// nothing ever revisits that decision. Instead the next startup must
    /// still see the OLD id and redo the conversion, which is what the
    /// second half of this test drives.
    #[tokio::test]
    async fn a_failure_inside_the_boot_transaction_leaves_the_next_startup_correct() {
        let (_dir, store) = fresh_store().await;
        store
            .record_boot("boot-a", false, HashMap::new(), None)
            .await
            .expect("first boot");
        insert_running(&store, "live").await;

        let fault: BootTxFault = Arc::new(|| anyhow::bail!("simulated crash mid-transaction"));
        let err = store
            .record_boot("boot-b", true, HashMap::new(), Some(fault))
            .await
            .expect_err("the injected failure must fail the call");
        assert!(format!("{err:#}").contains("simulated crash"));

        assert_eq!(
            store.boot_id().await.expect("boot id").as_deref(),
            Some("boot-a"),
            "a rolled-back conversion must not leave the new boot id behind"
        );
        assert_eq!(
            outcome_of(&store, "live").await,
            LastOutcome::Running,
            "no half-applied conversion"
        );

        // The next startup: same reboot, no injected failure.
        store
            .record_boot("boot-b", true, HashMap::new(), None)
            .await
            .expect("retry");
        assert_eq!(outcome_of(&store, "live").await, LastOutcome::Interrupted);
    }

    /// A row whose `outcome_state` is not in this build's vocabulary is
    /// corruption, and `load_all` must refuse it by name rather than
    /// invent a state for it — the same no-guessing stance `apply_schema`
    /// takes toward an unrecognized schema version. Reachable only by
    /// hand-editing the database (the schema version gates every shape
    /// this build writes), which is exactly why it is worth pinning: the
    /// tempting alternative is a silent `_ => Launching` fallback.
    ///
    /// The `error`-without-detail case is the same refusal for a
    /// half-written row: the wire type's `detail` is a required string, so
    /// there is no honest value to substitute, and the diagnosis must name
    /// the session so a human can find it.
    #[tokio::test]
    async fn load_all_refuses_rows_it_cannot_honestly_decode() {
        let (dir, store) = fresh_store().await;
        insert_running(&store, "s1").await;
        drop(store);
        let db_path = dir.path().join("supervisor.db");

        {
            let conn = Connection::open(&db_path).expect("raw open");
            conn.execute("UPDATE sessions SET outcome_state = 'teleported'", [])
                .expect("corrupt the row");
        }
        let store = SessionStore::open(&db_path, true).await.expect("reopen");
        let err = store.load_all().await.expect_err("must refuse");
        assert!(
            format!("{err:#}").contains("teleported") && format!("{err:#}").contains("s1"),
            "the error must name the value it refused and the session it came from: {err:#}"
        );

        {
            let conn = Connection::open(&db_path).expect("raw open");
            conn.execute(
                "UPDATE sessions SET outcome_state = 'error', error_detail = NULL",
                [],
            )
            .expect("corrupt the row");
        }
        let store = SessionStore::open(&db_path, true).await.expect("reopen");
        let err = store.load_all().await.expect_err("must refuse");
        assert!(
            format!("{err:#}").contains("error_detail") && format!("{err:#}").contains("s1"),
            "an error state with no detail must be diagnosed, not defaulted: {err:#}"
        );
    }

    /// The whole point of this module: a session inserted before the
    /// store is dropped must read back byte-identical from a fresh
    /// `SessionStore` opened on the same path — the on-disk round-trip
    /// `Supervisor::reload_sessions` depends on, both at construction and
    /// again from `serve`.
    #[tokio::test]
    async fn insert_then_reopen_round_trips_a_session() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("supervisor.db");

        let store = SessionStore::open(&db_path, true).await.expect("open");
        store
            .insert_session(StoredSession {
                id: "s1".to_string(),
                title: "demo".to_string(),
                cwd: "/tmp/work".to_string(),
                invocation: "agent --flag".to_string(),
                tmux_name: "fh-abc".to_string(),
                pane: "%3".to_string(),
                outcome: LastOutcome::Running,
            })
            .await
            .expect("insert");
        drop(store);

        let reopened = SessionStore::open(&db_path, true).await.expect("reopen");
        let rows = reopened.load_all().await.expect("load");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "s1");
        assert_eq!(rows[0].title, "demo");
        assert_eq!(rows[0].cwd, "/tmp/work");
        assert_eq!(rows[0].invocation, "agent --flag");
        assert_eq!(rows[0].tmux_name, "fh-abc");
        assert_eq!(rows[0].pane, "%3");
        assert_eq!(rows[0].outcome, LastOutcome::Running);
    }

    /// A fresh database (`user_version` 0) must come up on `user_version`
    /// [`SCHEMA_VERSION`] after `open` — the invariant every other
    /// `SessionStore` method assumes without re-checking on each call.
    #[tokio::test]
    async fn open_stamps_schema_version_on_a_fresh_database() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("supervisor.db");
        let store = SessionStore::open(&db_path, true).await.expect("open");
        let conn = Arc::clone(&store.conn);
        let version: i64 = tokio::task::spawn_blocking(move || {
            conn.lock()
                .unwrap()
                .query_row("PRAGMA user_version", [], |r| r.get(0))
                .unwrap()
        })
        .await
        .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
    }

    /// A database claiming a schema version this build does not
    /// understand must be refused outright rather than opened and
    /// silently misread — the honest failure mode for a version this
    /// build has no migration for (a downgrade, or a future version this
    /// build predates).
    ///
    /// Both a far-future version and version 3 — the very next one, the
    /// realistic downgrade case where an older binary meets a database a
    /// newer one already upgraded — are covered: now that `apply_schema`
    /// has a real migration ladder, the refusal is a fall-through at the
    /// END of that ladder rather than a lone `match` arm, and an
    /// off-by-one there would let exactly the adjacent version through.
    #[tokio::test]
    async fn open_refuses_an_unrecognized_schema_version() {
        for version in [SCHEMA_VERSION + 1, 99] {
            let dir = tempfile::tempdir().expect("tempdir");
            let db_path = dir.path().join("supervisor.db");
            {
                let conn = Connection::open(&db_path).expect("create raw db");
                conn.pragma_update(None, "user_version", version).unwrap();
            }
            let err = SessionStore::open(&db_path, true)
                .await
                .expect_err("unrecognized schema version must be refused");
            assert!(
                format!("{err:#}").contains(&version.to_string()),
                "error must name the unrecognized version: {err:#}"
            );
        }
    }

    /// The confidentiality repair this module performs on top of the
    /// state directory's own 0700 boundary: even a file that starts out
    /// world-writable ends up owner-only after `open`.
    ///
    /// Relying on the test runner's ambient umask here would let this
    /// pass vacuously on any runner whose umask already narrows new files
    /// to 0600 on its own, without ever exercising `open`'s own
    /// `set_permissions` repair. Planting the file at 0o666 BEFORE
    /// calling `open` (rather than mutating the process umask, which
    /// this project's tests must not do) is what forces the repair path
    /// to actually run for the assertion below to mean anything.
    #[tokio::test]
    async fn open_restricts_the_database_files_mode() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("supervisor.db");
        std::fs::write(&db_path, b"").expect("plant a fresh file");
        std::fs::set_permissions(&db_path, std::fs::Permissions::from_mode(0o666))
            .expect("widen the planted file's mode");

        let _store = SessionStore::open(&db_path, true).await.expect("open");
        let mode = std::fs::metadata(&db_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "database file must be repaired to owner-only, got {mode:o}"
        );
    }

    /// A deleted row must actually be gone on reload, and deleting an id
    /// that was never inserted (or was already deleted) must succeed
    /// rather than error — the idempotence `delete_session`'s docs promise.
    #[tokio::test]
    async fn delete_session_removes_the_row_and_tolerates_a_missing_one() {
        let (_dir, store) = fresh_store().await;
        insert_running(&store, "s1").await;

        store.delete_session("s1").await.expect("delete");
        assert!(
            store.load_all().await.expect("load").is_empty(),
            "deleted row must not survive a reload"
        );

        // Deleting again (an already-deleted row is, by now, exactly the
        // same "no matching row" case as an id that never existed at
        // all — one call suffices for both) must not error.
        store
            .delete_session("s1")
            .await
            .expect("deleting an already-deleted row must be idempotent");
    }

    /// `created_at` is written on every insert but read back by nothing in
    /// this module (see `StoredSession`'s docs) — it exists for a human or
    /// a future migration to consult directly. Assert it is at least
    /// wired correctly: a timestamp captured around the insert (before and
    /// after, since the write happens between the two reads) must bracket
    /// the value SQLite actually stored, queried directly rather than
    /// through `StoredSession` (which does not carry the field at all).
    #[tokio::test]
    async fn insert_session_records_created_at_within_the_surrounding_window() {
        let (_dir, store) = fresh_store().await;

        let before = now_unix();
        insert_running(&store, "s1").await;
        let after = now_unix();

        let conn = Arc::clone(&store.conn);
        let created_at: i64 = tokio::task::spawn_blocking(move || {
            conn.lock().unwrap().query_row(
                "SELECT created_at FROM sessions WHERE id = ?1",
                ["s1"],
                |r| r.get(0),
            )
        })
        .await
        .unwrap()
        .unwrap();

        assert!(
            (before..=after).contains(&created_at),
            "created_at {created_at} must fall within [{before}, {after}]"
        );
    }
}

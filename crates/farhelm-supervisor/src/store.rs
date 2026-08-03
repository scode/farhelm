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
//! One more durable fact sits beside the metadata, and it is easy to
//! read as a contradiction of the paragraph above — the distinction
//! matters. Liveness is still never persisted: tmux remains the
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
//! A second kind of durable fact has a lifetime that is not a
//! session's at all: [`Reservation`] (PLAN_M3.md item 6) records
//! that a client-supplied create INTENT was claimed, so a create retried
//! after an ambiguous failure replays its original outcome instead of
//! launching a second agent. A reservation and its launching row are
//! committed TOGETHER, so neither predates the other; what makes the
//! reservation its own table is that it is keyed by the CLIENT's intent
//! key rather than by a session id (the lookup a replay performs happens
//! before any session is known), that it can exist with no session row at
//! all (a create refused before it ever launched), and that it OUTLIVES
//! the session as a tombstone — so a replay for a deleted session can say
//! so instead of duplicating it.
//!
//! The per-session integration snapshot and the conversation identity
//! captured against it form the capture half of the schema (PLAN_M3.md
//! items 7 and 8).
//! The snapshot columns — [`StoredSession::agent_kind`] and
//! [`StoredSession::resume_template`] — are IMMUTABLE: they are written by
//! the insert that creates the row and there is deliberately no update path
//! for them, because re-deriving a kind later would consult a PATH and a
//! filesystem that may since have changed (`crate::agent_kind`'s own docs).
//! The two columns beside them are the mutable half and each has exactly
//! one narrow writer: [`SessionStore::record_first_input`] and
//! [`SessionStore::record_captured_conversation`], both write-once and both
//! conditioned on the column still being NULL, so neither can ever move
//! backwards or overwrite what a concurrent observer already established.
//!
//! The `title` is the one piece of metadata a USER can change after
//! creation (PLAN_M5.md item 3), and its writer —
//! [`SessionStore::set_session_title`] — is deliberately unlike those two:
//! unconditional, because a rename is a deliberate overwrite of a label
//! whose previous value carries no authority, which is what makes
//! concurrent renames last-write-wins rather than write-once.
//!
//! Journal mode and synchronous pragmas are left at SQLite's defaults.
//! The recorded crash-safety/atomicity policy (PLAN_M3.md item 5,
//! implemented in `crate::files`) governs the directly written state
//! FILES; the database's durability settings stay stock (the one
//! deliberate knob is `BUSY_TIMEOUT`, which is about handoff overlap,
//! not durability), and this module does not invent a stricter policy
//! than the plan records.

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
const SCHEMA_VERSION: i64 = 6;

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

/// One client-supplied create intent, durably claimed (PLAN_M3.md item 6).
///
/// The reservation is what makes a retried create idempotent across a
/// dropped reply, a crash, and a supervisor restart alike: it is committed
/// BEFORE any side effect of the launch it describes, and it carries the
/// identities that launch will use — so a retry that finds one still
/// `Pending` knows exactly which session and which tmux session to look
/// for rather than having to guess whether the previous attempt got
/// anywhere.
///
/// Rows here are TOMBSTONES: they outlive the session they created (see
/// [`SessionStore::delete_session_settling_reservations`]), because the
/// question a replay asks — "did this intent already happen?" — still has
/// an answer after the session is gone, and the honest answer is "yes, and
/// it was deleted", never a fresh duplicate.
///
/// Nothing prunes them, and the honest accounting is that each row holds a
/// full copy of its request's canonical fields (`service`'s
/// `create_fingerprint`), so a create with a long invocation stores a long
/// row — bounded by the request caps, not small in principle. Two separate
/// pieces of work would change that and neither is owned here: a DIGEST
/// would shrink each row to a constant size (and stop the invocation from
/// being retained past its session's deletion), while an EXPIRY would
/// bound the row COUNT. They are independent — a digest keeps growth
/// linear in creates, an expiry keeps the text — and only an expiry trades
/// against correctness, since a forgotten key is a key that can duplicate
/// again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reservation {
    pub intent_key: String,
    /// The canonical encoding of every session-shaping request field this
    /// key was claimed with; see `service::create_fingerprint`. Compared
    /// verbatim: equal means "the same request", anything else means the
    /// client reused a key, which is a bug rather than a merge.
    pub fingerprint: String,
    /// The session id assigned when the reservation was made, not when the
    /// session was created — the two are the same id, which is the point:
    /// reconciliation looks this up instead of searching for a session it
    /// has no name for. Every settlement is additionally CONDITIONED on
    /// this id (see [`Settlement`]), so a settlement computed against one
    /// attempt can never land on a reservation that has since been
    /// re-pointed.
    pub session_id: String,
    /// The tmux session name assigned alongside `session_id`, for the same
    /// reason — and read for real: `service`'s pending reconciliation
    /// probes tmux for exactly this name at decision time rather than
    /// trusting a session map that may predate a late-completing create.
    pub tmux_name: String,
    pub outcome: ReservationOutcome,
}

/// A create intent being claimed, and the fingerprint that binds it to one
/// request.
///
/// The same type on both sides of the boundary: `service` builds it from
/// the request, [`SessionStore::insert_session`] commits it beside the
/// launching row it reserves. The reservation's `session_id`/`tmux_name`
/// are taken from that row rather than carried here, so the two can never
/// be committed disagreeing about which session this intent produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentClaim {
    pub intent_key: String,
    pub fingerprint: String,
}

/// One reservation's outcome, addressed by BOTH identities it must match.
///
/// `session_id` is not redundant with `intent_key`: it is the condition
/// that keeps a settlement computed for one attempt from landing on a
/// reservation some other attempt has since re-pointed (see
/// [`SessionStore::settle_reservations`]). Every settlement in this module
/// carries it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settlement {
    pub intent_key: String,
    pub session_id: String,
    pub outcome: ReservationOutcome,
}

/// What [`SessionStore::insert_session`] found when it tried to claim an
/// intent key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Claimed {
    /// The key was free (or the create carried none): the launching row is
    /// committed and this process owns the intent.
    Ours,
    /// The key was already claimed. NOTHING was committed — the session row
    /// is rolled back with the refused claim — and the existing reservation
    /// is returned so the caller can answer from it rather than guessing.
    ///
    /// Reachable only by a racer that bypassed `service`'s per-key lock (a
    /// second supervisor process, which the state-directory claim already
    /// excludes), which is exactly why it is a returned VALUE and not an
    /// error: the caller's honest response is to resolve the existing
    /// reservation, not to fail.
    TakenBy(Box<Reservation>),
}

/// The outcome of trying to take over a pending reservation for a relaunch
/// ([`SessionStore::restart_pending_launch`]).
///
/// The three variants are the three things that can be true by the time
/// the transition actually runs, and distinguishing them is what keeps a
/// racing delete from being resurrected: the caller decided to relaunch
/// against evidence it gathered a moment earlier, and this is the atomic
/// re-check of that decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetryClaim {
    /// The reservation was still pending and its row still un-launched:
    /// a fresh launching row is committed under the same identities and
    /// this process now owns the relaunch.
    Acquired,
    /// The reservation is no longer pending — something settled it first
    /// (most often a concurrent delete, which tombstones as `Created`). The
    /// caller must answer from this outcome, which for a deleted session is
    /// the gone-error rather than a new launch.
    Resolved(Box<Reservation>),
    /// The reserved session row moved past `Launching` while the caller was
    /// deciding: evidence of a launch appeared after all, so the caller
    /// replays instead of relaunching.
    Launched,
}

/// The mutable inputs to a session's restart offer, as the caller read
/// them when it validated the requested mode — the condition
/// [`SessionStore::begin_relaunch`] claims under.
///
/// Only these two, and that is a claim worth stating: kind and resume
/// template are immutable from create (PLAN_M3.md item 7), so a session's
/// offer can only ever change because capture claimed an identity or
/// declared the correlation ambiguous. Conditioning on exactly the fields
/// that can move is what keeps the check tight enough to be meaningful and
/// loose enough not to reject a relaunch over an unrelated write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfferBasis {
    pub captured_conversation: Option<String>,
    pub capture_ambiguous: bool,
}

/// What a session's row said about its PREVIOUS run, handed back by
/// [`SessionStore::begin_relaunch`] so a failed relaunch can put it back
/// ([`SessionStore::abort_relaunch`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriorRun {
    pub outcome: LastOutcome,
    /// The pane the previous run confirmed, or empty for a launch that
    /// never confirmed one. Restored alongside the outcome so an aborted
    /// restart leaves the row describing the same terminal it did before.
    pub pane: String,
    /// Whether the previous run was scope-wrapped
    /// ([`StoredSession::launch_scoped`]).
    ///
    /// Restored with the rest for the same reason the pane is: an aborted
    /// restart must leave the row describing the run that is (or was)
    /// actually there. Dropping it would leave a session whose stop silently
    /// degraded to sweep-only, because the row would then describe the
    /// abandoned generation — whose scope never existed — rather than the
    /// live run, whose does.
    pub scoped: bool,
}

/// The columns [`SessionStore::begin_relaunch`] reads before deciding
/// whether it may open a new generation: the outcome quartet, the pane,
/// the current generation, the captured conversation, the ambiguity flag,
/// and the current launch's scope selection — in that positional order.
///
/// Named only because the tuple is wide enough that clippy (rightly) asks
/// for it; it has exactly one producer and one consumer, both inside that
/// function's transaction.
type RelaunchBasisColumns = (OutcomeColumns, String, i64, Option<String>, i64, i64);

/// The new launch generation a restart claimed, and what it replaced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelaunchClaim {
    pub generation: i64,
    pub prior: PriorRun,
    /// Whether this new generation committed to running under a scope
    /// ([`StoredSession::launch_scoped`]), handed back so the launch that
    /// follows wraps itself exactly as the row now says it did.
    pub scoped: bool,
}

/// What [`SessionStore::begin_relaunch`] found when it tried to open a new
/// launch generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelaunchDecision {
    /// The generation is open and this caller owns the relaunch.
    Claimed(RelaunchClaim),
    /// The session's captured identity or ambiguity verdict changed since
    /// the caller validated the requested mode against them, so the mode
    /// may no longer be the one the session's offer authorizes. Nothing was
    /// written.
    OfferChanged,
    /// The row is gone — a delete committed while the restart was being
    /// prepared. Nothing was written, and nothing may be recreated.
    Gone,
}

/// Where a [`Reservation`] stands: claimed, or resolved one way or the
/// other.
///
/// Terminal states are monotonic — [`SessionStore::settle_reservations`]
/// only ever moves a `Pending` row — so an outcome, once recorded, is what
/// every later replay of that key reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReservationOutcome {
    /// Claimed, with the create still in flight — or interrupted by a
    /// crash, which is indistinguishable from in-flight after the fact and
    /// is exactly why reconciliation exists.
    Pending,
    /// The session named by [`Reservation::session_id`] was created. It
    /// may since have been DELETED; that is not recorded here, because the
    /// session table already answers it (`service`'s replay looks the id up
    /// and reports the gone-error when it is absent) and a second copy of
    /// the same fact could only ever disagree with the first.
    Created,
    /// The create failed, with the error the first attempt reported.
    ///
    /// `kind` rides along with the message because replaying an
    /// `InvalidRequest` as an `Internal` would turn the first attempt's 400
    /// into a 500 for a byte-identical request — the replay must be the
    /// same answer, not merely a similar one.
    Failed {
        kind: farhelm_proto::ErrorKind,
        message: String,
    },
}

impl ReservationOutcome {
    /// Split into the three columns that store it. Like
    /// [`LastOutcome::columns`], the state text is a STABLE on-disk
    /// vocabulary spelled out here rather than derived from variant names.
    fn columns(&self) -> (&'static str, Option<&'static str>, Option<&str>) {
        match self {
            ReservationOutcome::Pending => ("pending", None, None),
            ReservationOutcome::Created => ("created", None, None),
            ReservationOutcome::Failed { kind, message } => {
                ("failed", Some(error_kind_column(*kind)), Some(message))
            }
        }
    }

    /// Reassemble from the three columns, refusing anything outside the
    /// vocabulary rather than defaulting — same no-guessing stance as
    /// [`LastOutcome::from_columns`], and for the same reason: a value this
    /// build does not recognize means the row is corrupt, and a guessed
    /// outcome here would either replay a fabricated success or launch a
    /// duplicate.
    fn from_columns(
        state: &str,
        error_kind: Option<String>,
        error_detail: Option<String>,
    ) -> anyhow::Result<ReservationOutcome> {
        Ok(match state {
            "pending" => ReservationOutcome::Pending,
            "created" => ReservationOutcome::Created,
            "failed" => {
                let kind = error_kind.ok_or_else(|| {
                    anyhow::anyhow!("reservation row is 'failed' but carries no error kind")
                })?;
                ReservationOutcome::Failed {
                    kind: error_kind_from_column(&kind)?,
                    message: error_detail.ok_or_else(|| {
                        anyhow::anyhow!("reservation row is 'failed' but carries no error text")
                    })?,
                }
            }
            other => anyhow::bail!("reservation row has unrecognized state {other:?}"),
        })
    }
}

/// The on-disk spelling of an [`ErrorKind`](farhelm_proto::ErrorKind), for
/// a failed reservation's replay.
///
/// Deliberately its own vocabulary rather than the wire's serde
/// representation: the two happen to agree today, but the wire encoding is
/// free to change with a protocol bump while every database in the field
/// keeps the rows it already has.
fn error_kind_column(kind: farhelm_proto::ErrorKind) -> &'static str {
    use farhelm_proto::ErrorKind as K;
    match kind {
        K::NotFound => "not_found",
        K::InvalidRequest => "invalid_request",
        K::Internal => "internal",
        K::Conflict => "conflict",
    }
}

/// The inverse of [`error_kind_column`]; see
/// [`ReservationOutcome::from_columns`] for why an unrecognized value is
/// refused rather than defaulted to `Internal`.
fn error_kind_from_column(text: &str) -> anyhow::Result<farhelm_proto::ErrorKind> {
    use farhelm_proto::ErrorKind as K;
    Ok(match text {
        "not_found" => K::NotFound,
        "invalid_request" => K::InvalidRequest,
        "internal" => K::Internal,
        "conflict" => K::Conflict,
        other => anyhow::bail!("reservation row has unrecognized error kind {other:?}"),
    })
}

/// The on-disk spelling of an [`AgentKind`](farhelm_proto::AgentKind).
///
/// A STABLE vocabulary owned here, spelled out rather than derived from the
/// Rust variant names, for the same reason [`LastOutcome::columns`] is: a
/// variant rename must not invalidate every database in the field.
///
/// It is also the vocabulary `service::create_fingerprint` persists into
/// reservation rows, and it is shared rather than duplicated because as of
/// PLAN_M3.md item 7 the same kind is written to two durable places at
/// once: two independent spellings that drifted would make an unchanged
/// retry of a create look like a key reuse for every session created
/// before the drift, with nothing at runtime able to notice.
pub(crate) fn agent_kind_column(kind: farhelm_proto::AgentKind) -> &'static str {
    use farhelm_proto::AgentKind as K;
    match kind {
        K::Claude => "claude",
        K::Codex => "codex",
        K::Generic => "generic",
    }
}

/// The inverse of [`agent_kind_column`].
///
/// An unrecognized value is refused rather than defaulted to `Generic`,
/// matching every other decoder in this module: the schema version already
/// gates which shapes this build understands, so a value outside the
/// vocabulary means the row is corrupt — and silently downgrading such a
/// session to "no integration" would discard a captured conversation
/// identity that may be sitting in the very next column.
fn agent_kind_from_column(text: &str) -> anyhow::Result<farhelm_proto::AgentKind> {
    use farhelm_proto::AgentKind as K;
    Ok(match text {
        "claude" => K::Claude,
        "codex" => K::Codex,
        "generic" => K::Generic,
        other => anyhow::bail!("session row has unrecognized agent kind {other:?}"),
    })
}

/// Encode a resume template for its column: a JSON array, or NULL for a
/// session with no resume invocation.
///
/// JSON rather than a delimiter-joined string because the whole point of
/// storing argv structurally is that an element may contain anything —
/// spaces, quotes, a delimiter — and still come back as one element.
fn resume_template_column(template: Option<&[String]>) -> Option<String> {
    template.map(|template| {
        serde_json::to_string(template).expect("a vector of strings always serializes")
    })
}

/// The inverse of [`resume_template_column`]; a value that is present but
/// not a JSON string array is refused rather than dropped, since a session
/// silently losing its resume template would turn a `Resume` offer into a
/// `FreshOnly` one with no explanation anywhere.
fn resume_template_from_column(text: Option<String>) -> anyhow::Result<Option<Vec<String>>> {
    text.map(|text| {
        serde_json::from_str::<Vec<String>>(&text)
            .context("decoding a session's stored resume template")
    })
    .transpose()
}

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
    /// The session's integration snapshot (PLAN_M3.md item 7), resolved
    /// once at create and IMMUTABLE thereafter — no method on this type
    /// updates either field. See the module docs for why.
    pub agent_kind: farhelm_proto::AgentKind,
    /// The resume invocation as an argv vector, JSON-encoded in one
    /// column. Structural rather than a command string so a path with
    /// spaces survives without quoting (`crate::agent_kind`); `None` is a
    /// session with no resume invocation at all, which only a `Generic`
    /// kind can be.
    pub resume_template: Option<Vec<String>>,
    /// The session's working directory with every symlink, `.`, `..`, and
    /// trailing slash resolved away, as it was at create.
    ///
    /// Correlation uses THIS, never [`StoredSession::cwd`], for both the
    /// munged directory name and the recorded-cwd comparison: the agent
    /// reports its own `getcwd()`, which the kernel has already resolved,
    /// so a session created through a symlinked path would otherwise never
    /// match its own records. `cwd` stays exactly as the user spelled it,
    /// because that is what the UI shows and what a create replays.
    /// `None` only for a row that predates this column, which can have no
    /// integration and therefore no correlation either.
    pub canonical_cwd: Option<String>,
    /// The agent conversation this session was found to be running
    /// (PLAN_M3.md item 8), or `None` while nothing has been claimed.
    /// Written once by [`SessionStore::record_captured_conversation`], and
    /// only ever from a COMPLETE post-horizon scan — see `service`'s
    /// `capture_pass` for why a provisional match is never stored.
    pub captured_conversation: Option<String>,
    /// Where the claimed conversation's record was when it was claimed.
    ///
    /// A locator hint, not an identity: it exists so a supervisor restart
    /// can re-verify a captured session's record with one `stat` instead
    /// of re-scanning its whole directory, which would otherwise make
    /// startup cost multiplicative in captured sessions. A stale path
    /// simply fails re-verification, which retains the identity (see
    /// `service`'s `reverify_capture`).
    pub captured_record: Option<String>,
    /// Whether correlation for this session was found AMBIGUOUS and no
    /// identity will ever be claimed for this launch (PLAN_M3.md item 8).
    ///
    /// Durable, and that is the point: the collision that produced it —
    /// a rival session sharing the working directory, a second record in
    /// the window — does not become less ambiguous across a restart, and a
    /// fresh supervisor that happened to see only one of the two
    /// candidates (because the rival's evidence has since been cleaned up)
    /// would otherwise claim an identity on strictly worse evidence than
    /// the pass that bailed. Only a new LAUNCH clears it — a verdict is
    /// about one run's correlation, not about the session forever — which
    /// is what [`SessionStore::begin_relaunch`] does for a relaunch that
    /// is not resuming a captured identity.
    pub capture_ambiguous: bool,
    /// When this supervisor first confirmed delivery of input to the
    /// CURRENT launch, in seconds since the Unix epoch — the correlator
    /// capture keys on, because the agents' records appear at first PROMPT
    /// submission rather than at launch. Durable so that a supervisor
    /// restart landing in the (unbounded) launch-to-first-input gap does
    /// not cost the session its only chance at capture. Written once per
    /// launch by [`SessionStore::record_first_input`], and cleared by a
    /// relaunch that opens a fresh capture window
    /// ([`SessionStore::begin_relaunch`]): this is PER-LAUNCH state, not
    /// conversation metadata — a new run's first prompt is what its record
    /// appears after, and reusing the previous run's anchor would search a
    /// window that closed long ago.
    pub first_input_at: Option<i64>,
    /// Which LAUNCH of this session the row currently describes: 0 for the
    /// session's original launch, incremented once by every relaunch
    /// ([`SessionStore::begin_relaunch`]).
    ///
    /// The fence every durable write about the current run carries. Two
    /// failures it exists to exclude, both real races rather than
    /// theoretical ones: a `ListSessions` pass holding an entry from before
    /// a restart must not record the OLD pane's death as the new run's
    /// outcome, and a capture pass that started against the previous launch
    /// must not commit that launch's identity onto the new one. Every
    /// generation-conditioned write ([`SessionStore::transition_many`],
    /// the capture writers) simply does nothing when the generation it
    /// carries is no longer current.
    ///
    /// Monotonic and never reused, which is also what makes it safe to name
    /// files after (`launch::spec_path_for_launch`): a stale sentinel from
    /// generation N can never be mistaken for generation N+1's, because the
    /// two are different paths rather than the same path written twice.
    pub generation: i64,
    /// Whether this LAUNCH was wrapped in its own systemd transient scope
    /// (PLAN_M3.md item 10), as opposed to relying on the portable sweep
    /// alone.
    ///
    /// The SELECTION only — never the unit's name. The name is a pure
    /// function of [`StoredSession::id`] and [`StoredSession::generation`]
    /// (`crate::scope::unit_name`) and is re-derived at every use, so no
    /// value this database holds can aim a kill at a unit belonging to
    /// another session; see the version-6 migration for the full argument.
    ///
    /// Per-launch, like the generation it is derived alongside: re-decided
    /// and re-recorded by every create and every relaunch, because a host
    /// can gain or lose its user manager between two launches of the same
    /// session and a stale claim would send stop hunting for a unit nothing
    /// ever created.
    ///
    /// Durable so that stop still knows what a launch this supervisor never
    /// performed chose — a restarted supervisor has no memory of the probe
    /// that produced it. `true` is not a promise the unit still EXISTS: the
    /// manager is asked before any signal is aimed at it, so a scope systemd
    /// already collected costs nothing but a fall through to the sweep.
    /// `false` is never a degradation — it is exactly M2's stop.
    pub launch_scoped: bool,
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
/// - 3: PLAN_M3.md item 6 — `create_reservations`, the durable half of
///   create idempotency (see [`Reservation`]). Its own table rather than
///   columns on `sessions` because it is keyed by the client's intent key
///   (the lookup a replay runs before any session is known), because it
///   can exist with no session row at all, and because it outlives the
///   session it created as a tombstone. Its lifetime is the INTENT's, not
///   the session's.
/// - 4: PLAN_M3.md items 7 and 8 — the per-session integration snapshot
///   (`agent_kind`, `resume_template`), the conversation identity captured
///   against it, and the first-input timestamp that capture correlates on.
///   Columns on `sessions` rather than a side table because all four have
///   exactly the session's lifetime and are read on the same paths that
///   already load the row; a join would buy nothing and would let a
///   snapshot outlive (or predate) the session it describes.
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
                 error_detail  TEXT,
                 agent_kind    TEXT NOT NULL DEFAULT 'generic',
                 resume_template       TEXT,
                 canonical_cwd         TEXT,
                 captured_conversation TEXT,
                 captured_record       TEXT,
                 capture_ambiguous     INTEGER NOT NULL DEFAULT 0,
                 first_input_at        INTEGER,
                 generation            INTEGER NOT NULL DEFAULT 0,
                 launch_scoped         INTEGER NOT NULL DEFAULT 0
             ) STRICT;
             CREATE TABLE supervisor_meta (
                 id      INTEGER PRIMARY KEY CHECK (id = 0),
                 boot_id TEXT
             ) STRICT;
             CREATE TABLE create_reservations (
                 intent_key   TEXT PRIMARY KEY,
                 fingerprint  TEXT NOT NULL,
                 state        TEXT NOT NULL,
                 session_id   TEXT NOT NULL,
                 tmux_name    TEXT NOT NULL,
                 error_kind   TEXT,
                 error_detail TEXT,
                 created_at   INTEGER NOT NULL
             ) STRICT;
             CREATE INDEX create_reservations_pending
                 ON create_reservations (session_id) WHERE state = 'pending';
             PRAGMA user_version = 6;
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
    if version == 2 {
        // Purely additive: there is no pre-M3 data to migrate INTO this
        // table, because no build before item 6 ever accepted an intent
        // key. Sessions that already exist simply have no reservation, and
        // a create for one is impossible — a key is claimed at create time
        // or never.
        //
        // The index is PARTIAL, and that is the whole point: this table is
        // immortal (see `Reservation`'s tombstone docs), so it is the one
        // table here that grows without bound, while both queries that are
        // not by primary key touch only PENDING rows — reload's
        // reconciliation worklist and the delete path's settlement. A
        // partial index keeps their cost proportional to creates currently
        // in flight rather than to every create this host has ever done.
        conn.execute_batch(
            "BEGIN;
             CREATE TABLE create_reservations (
                 intent_key   TEXT PRIMARY KEY,
                 fingerprint  TEXT NOT NULL,
                 state        TEXT NOT NULL,
                 session_id   TEXT NOT NULL,
                 tmux_name    TEXT NOT NULL,
                 error_kind   TEXT,
                 error_detail TEXT,
                 created_at   INTEGER NOT NULL
             ) STRICT;
             CREATE INDEX create_reservations_pending
                 ON create_reservations (session_id) WHERE state = 'pending';
             PRAGMA user_version = 3;
             COMMIT;",
        )
        .context("migrating schema from version 2 to 3")?;
        version = 3;
    }
    if version == 3 {
        // Purely additive, and the DEFAULTS are the whole design decision.
        // A pre-item-7 row has no recorded kind, and there is no honest way
        // to invent one: deriving it now from the stored invocation would
        // be exactly the "re-guess it later" that item 7 forbids — the
        // basename that would be recognized today is not necessarily what
        // the session was launched against, and a session that silently
        // acquired an integration would then be offered a resume it can
        // never fill (no first-input time was ever recorded for it either,
        // so capture could not run for it in any case). `generic` with no
        // template is the honest reading: this session predates the
        // snapshot, so restart can only ever offer it a fresh launch.
        conn.execute_batch(
            "BEGIN;
             ALTER TABLE sessions ADD COLUMN agent_kind TEXT NOT NULL DEFAULT 'generic';
             ALTER TABLE sessions ADD COLUMN resume_template       TEXT;
             ALTER TABLE sessions ADD COLUMN canonical_cwd         TEXT;
             ALTER TABLE sessions ADD COLUMN captured_conversation TEXT;
             ALTER TABLE sessions ADD COLUMN captured_record       TEXT;
             ALTER TABLE sessions ADD COLUMN capture_ambiguous     INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE sessions ADD COLUMN first_input_at        INTEGER;
             PRAGMA user_version = 4;
             COMMIT;",
        )
        .context("migrating schema from version 3 to 4")?;
        version = 4;
    }
    if version == 4 {
        // The launch generation (PLAN_M3.md item 9's restart): a monotonic
        // counter, bumped once per relaunch, that every durable write about
        // a session's CURRENT run is conditioned on.
        //
        // `DEFAULT 0` backfills every existing row with the generation its
        // one and only launch has always implicitly had — a session that
        // has never been restarted is on its first launch by definition, so
        // this migration invents nothing. The counter is also what makes
        // the per-launch spec and sentinel paths distinguishable
        // (`launch::spec_path_for_launch`), which is why a pre-generation
        // row's files, named for generation 0, are exactly where this
        // build looks for them.
        conn.execute_batch(
            "BEGIN;
             ALTER TABLE sessions ADD COLUMN generation INTEGER NOT NULL DEFAULT 0;
             PRAGMA user_version = 5;
             COMMIT;",
        )
        .context("migrating schema from version 4 to 5")?;
        version = 5;
    }
    if version == 5 {
        // The cgroup SELECTION one launch made (PLAN_M3.md item 10): 1 for a
        // launch wrapped in its own transient scope, 0 for one that fell
        // back to the process-tree sweep alone.
        //
        // A boolean rather than the unit's name, deliberately. The name is a
        // pure function of the session id and generation
        // (`crate::scope::unit_name`), and this database is a trust boundary
        // like any other input — a row is whatever the last writer, a crash,
        // a downgrade, or a hand-edit left behind. A stored NAME could
        // therefore aim a `systemctl kill` at a unit belonging to some other
        // session (or to something else entirely); a stored BOOLEAN cannot
        // say anything except "this launch was scoped", and the name is
        // re-derived from the row's own identity at every use.
        //
        // 0 is the right backfill and not merely the convenient one: every
        // row predating this column was launched by a build that had no
        // scopes at all, so no unit exists for it and none ever will. Stop
        // for those sessions is sweep-only — exactly what 0 means for a row
        // this build writes too, so old and new fallback rows are
        // indistinguishable by design rather than by accident.
        conn.execute_batch(
            "BEGIN;
             ALTER TABLE sessions ADD COLUMN launch_scoped INTEGER NOT NULL DEFAULT 0;
             PRAGMA user_version = 6;
             COMMIT;",
        )
        .context("migrating schema from version 5 to 6")?;
        version = 6;
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

/// Read one reservation through whatever connection or transaction the
/// caller already holds.
///
/// A free function rather than a method because half its callers are
/// INSIDE a transaction that must not be interrupted by a second lock
/// acquisition: the claim's lost-race path and the relaunch takeover both
/// have to read the reservation as part of their own atomic decision, and
/// a method that took the store's mutex again would deadlock against the
/// guard they are already holding.
/// Apply one [`Settlement`] inside a transaction the caller owns; see
/// [`SessionStore::settle_reservations`] for what the two `WHERE`
/// conditions protect and why a non-matching row is a silent no-op.
///
/// Shared with `delete_session`'s rollback path, which has to settle in
/// the same transaction as the row removal it accompanies.
fn settle_within(conn: &Connection, settlement: &Settlement) -> anyhow::Result<()> {
    let (state, error_kind, error_detail) = settlement.outcome.columns();
    conn.execute(
        "UPDATE create_reservations \
         SET state = ?3, error_kind = ?4, error_detail = ?5 \
         WHERE intent_key = ?1 AND session_id = ?2 AND state = 'pending'",
        rusqlite::params![
            settlement.intent_key,
            settlement.session_id,
            state,
            error_kind,
            error_detail
        ],
    )
    .context("settling a create reservation")?;
    Ok(())
}

/// Insert one session row through whatever transaction the caller owns.
///
/// Shared by the first-time insert and the relaunch takeover, which write
/// the SAME row shape under the same identities — keeping one statement
/// is what stops a column added for one path from being silently absent on
/// the other (a relaunch that dropped the integration snapshot would leave
/// a session that could never resume, with nothing to point at).
fn insert_session_row(conn: &Connection, row: &StoredSession) -> anyhow::Result<()> {
    let (state, exit_code, annotation, error_detail) = row.outcome.columns();
    conn.execute(
        "INSERT INTO sessions \
         (id, title, cwd, invocation, tmux_name, pane, created_at, \
          outcome_state, exit_code, annotation, error_detail, \
          agent_kind, resume_template, canonical_cwd, captured_conversation, \
          captured_record, capture_ambiguous, first_input_at, generation, launch_scoped) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, \
                 ?18, ?19, ?20)",
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
            agent_kind_column(row.agent_kind),
            resume_template_column(row.resume_template.as_deref()),
            row.canonical_cwd,
            row.captured_conversation,
            row.captured_record,
            i64::from(row.capture_ambiguous),
            row.first_input_at,
            row.generation,
            i64::from(row.launch_scoped),
        ],
    )
    .context("inserting session row")?;
    Ok(())
}

/// The column list every session read shares, in the order
/// [`decode_session_row`] expects. Named so the two readers cannot drift
/// apart by one column and start decoding each other's fields.
const SESSION_COLUMNS: &str = "id, title, cwd, invocation, tmux_name, pane, \
                               outcome_state, exit_code, annotation, error_detail, \
                               agent_kind, resume_template, canonical_cwd, \
                               captured_conversation, captured_record, capture_ambiguous, \
                               first_input_at, generation, launch_scoped";

/// The raw columns of one session row, before the fallible decoding that
/// cannot happen inside a rusqlite row mapper (whose error type is
/// rusqlite's own — see `load_all`'s two-stage comment).
///
/// The trailing two members are the raw agent-kind text and the raw
/// resume-template JSON; every other column is already in place on the
/// partially-built `StoredSession`, because only these two (with the
/// outcome) can be REFUSED.
type SessionColumns = (StoredSession, OutcomeColumns, String, Option<String>);

/// Read one row's columns positionally, matching [`SESSION_COLUMNS`].
///
/// The `StoredSession` this produces carries PLACEHOLDER values for every
/// field that needs fallible decoding; [`decode_session_row`] is what
/// replaces them. Splitting the two is what keeps a corrupt outcome or a
/// malformed template refusable with its own message instead of being
/// flattened into a generic rusqlite decode failure.
fn read_session_columns(r: &rusqlite::Row<'_>) -> rusqlite::Result<SessionColumns> {
    Ok((
        StoredSession {
            id: r.get(0)?,
            title: r.get(1)?,
            cwd: r.get(2)?,
            invocation: r.get(3)?,
            tmux_name: r.get(4)?,
            pane: r.get(5)?,
            outcome: LastOutcome::Launching,
            agent_kind: farhelm_proto::AgentKind::Generic,
            resume_template: None,
            canonical_cwd: r.get(12)?,
            captured_conversation: r.get(13)?,
            captured_record: r.get(14)?,
            capture_ambiguous: r.get::<_, i64>(15)? != 0,
            first_input_at: r.get(16)?,
            generation: r.get(17)?,
            launch_scoped: r.get::<_, i64>(18)? != 0,
        },
        (r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?),
        r.get(10)?,
        r.get(11)?,
    ))
}

/// Finish decoding a row read by [`read_session_columns`], refusing rather
/// than guessing on anything outside this build's vocabulary.
///
/// Two SEMANTIC checks run here, not only syntactic ones, and they are
/// deliberately the same invariants `create` enforces (`agent_kind`'s
/// `IntegrationSnapshot::resolve`). The database is a trust boundary like
/// any other input: a row is whatever the last process to write it left
/// behind, plus whatever a crash, a downgrade, or a hand-edit did to it.
/// A row claiming an integrated kind with no `{conversation}` placeholder
/// in its template describes a session that could capture an identity and
/// then be unable to resume with it — SPEC.md's exact-conversation promise
/// silently false — so it is refused at load rather than allowed to reach
/// the restart path and be discovered there.
fn decode_session_row(columns: SessionColumns) -> anyhow::Result<StoredSession> {
    let (mut row, (state, exit_code, annotation, error_detail), kind, template) = columns;
    row.outcome = LastOutcome::from_columns(&state, exit_code, annotation, error_detail)
        .with_context(|| format!("session {}", row.id))?;
    row.agent_kind =
        agent_kind_from_column(&kind).with_context(|| format!("session {}", row.id))?;
    row.resume_template =
        resume_template_from_column(template).with_context(|| format!("session {}", row.id))?;
    if crate::agent_kind::integration_for(row.agent_kind).is_some()
        && !crate::agent_kind::template_has_placeholder(row.resume_template.as_deref())
    {
        anyhow::bail!(
            "session {} is recorded with the integrated agent kind {} but a resume template \
             carrying no {} element; that combination is refused at create and cannot be \
             honored at restart either",
            row.id,
            agent_kind_column(row.agent_kind),
            crate::agent_kind::CONVERSATION_PLACEHOLDER
        );
    }
    Ok(row)
}

fn read_reservation(conn: &Connection, intent_key: &str) -> anyhow::Result<Option<Reservation>> {
    let row = conn
        .query_row(
            "SELECT fingerprint, state, session_id, tmux_name, error_kind, error_detail \
             FROM create_reservations WHERE intent_key = ?1",
            rusqlite::params![intent_key],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                ))
            },
        )
        .optional()
        .context("reading a create reservation")?;
    let Some((fingerprint, state, session_id, tmux_name, error_kind, error_detail)) = row else {
        return Ok(None);
    };
    let outcome = ReservationOutcome::from_columns(&state, error_kind, error_detail)
        .with_context(|| format!("create reservation {intent_key}"))?;
    Ok(Some(Reservation {
        intent_key: intent_key.to_string(),
        fingerprint,
        session_id,
        tmux_name,
        outcome,
    }))
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
    ///
    /// `reservation` claims a client-supplied intent key for this same
    /// launch (PLAN_M3.md item 6), in ONE transaction with the row above.
    /// The atomicity is why the two are not separate calls: a reservation
    /// without its launching row would send a retry hunting for the side
    /// effects of an attempt that was never even recorded, and a launching
    /// row without its reservation would let a retry launch a SECOND
    /// session for the same intent — the exact duplicate this whole
    /// mechanism exists to exclude. `None` is a create with no intent key
    /// (pre-M3 behavior, unchanged) and touches the reservation table not
    /// at all.
    ///
    /// A key already claimed is reported as [`Claimed::TakenBy`] with
    /// NOTHING committed — not as an error and not as an overwrite. The
    /// caller reaches here only after finding no reservation for the key,
    /// so a claim that loses means a racer bypassed `service`'s per-key
    /// collapse; the honest answer is then to resolve the winner's
    /// reservation, which needs the winner's row rather than a failure.
    pub async fn insert_session(
        &self,
        row: StoredSession,
        claim: Option<IntentClaim>,
    ) -> anyhow::Result<Claimed> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<Claimed> {
            let mut conn = conn.lock().expect("session db mutex poisoned");
            let tx = conn
                .transaction()
                .context("beginning the session insert transaction")?;
            if let Some(claim) = &claim {
                // The claim goes FIRST so a lost race costs nothing: the
                // session row is only written once the key is provably
                // ours. `DO NOTHING` (rather than a bare insert) is what
                // turns "someone else has it" into an answer instead of a
                // constraint error there would be no way to inspect.
                let (state, error_kind, error_detail) = ReservationOutcome::Pending.columns();
                let claimed = tx
                    .execute(
                        "INSERT INTO create_reservations \
                         (intent_key, fingerprint, state, session_id, tmux_name, \
                          error_kind, error_detail, created_at) \
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
                         ON CONFLICT(intent_key) DO NOTHING",
                        rusqlite::params![
                            claim.intent_key,
                            claim.fingerprint,
                            state,
                            row.id,
                            row.tmux_name,
                            error_kind,
                            error_detail,
                            now_unix(),
                        ],
                    )
                    .context("claiming the create reservation")?;
                if claimed == 0 {
                    let winner = read_reservation(&tx, &claim.intent_key)?.ok_or_else(|| {
                        anyhow::anyhow!(
                            "intent key {} refused the claim but has no reservation row",
                            claim.intent_key
                        )
                    })?;
                    // Rolled back, not committed: no session row, no claim.
                    return Ok(Claimed::TakenBy(Box::new(winner)));
                }
            }
            insert_session_row(&tx, &row)?;
            tx.commit().context("committing the session insert")?;
            Ok(Claimed::Ours)
        })
        .await
        .context("session insert task panicked")?
    }

    /// Record an intent that failed BEFORE it ever had a session row —
    /// a create refused by validation (PLAN_M3.md item 6's replay contract
    /// has no validation exception: acceptance 7's "a failed create
    /// replays its original error" covers a bad working directory exactly
    /// like a failed launch).
    ///
    /// The reserved identities are still stored, even though nothing was
    /// ever launched under them, because the columns describe the intent's
    /// assigned identity rather than a session that exists — and a `Failed`
    /// row is never reconciled against them.
    ///
    /// A key claimed concurrently wins: `DO NOTHING` leaves the existing
    /// row alone, and the caller finds it on its next lookup. Recording a
    /// failure over someone else's live claim would be strictly worse than
    /// letting this one attempt's error go unrecorded.
    pub async fn record_failed_intent(
        &self,
        claim: IntentClaim,
        session_id: &str,
        tmux_name: &str,
        kind: farhelm_proto::ErrorKind,
        message: &str,
    ) -> anyhow::Result<()> {
        let conn = Arc::clone(&self.conn);
        let session_id = session_id.to_string();
        let tmux_name = tmux_name.to_string();
        let message = message.to_string();
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let conn = conn.lock().expect("session db mutex poisoned");
            let outcome = ReservationOutcome::Failed { kind, message };
            let (state, error_kind, error_detail) = outcome.columns();
            conn.execute(
                "INSERT INTO create_reservations \
                 (intent_key, fingerprint, state, session_id, tmux_name, \
                  error_kind, error_detail, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
                 ON CONFLICT(intent_key) DO NOTHING",
                rusqlite::params![
                    claim.intent_key,
                    claim.fingerprint,
                    state,
                    session_id,
                    tmux_name,
                    error_kind,
                    error_detail,
                    now_unix(),
                ],
            )
            .context("recording a refused create against its intent key")?;
            Ok(())
        })
        .await
        .context("failed-intent record task panicked")?
    }

    /// Take over a pending reservation for a RELAUNCH, atomically
    /// re-checking the decision the caller made against evidence it
    /// gathered a moment ago (PLAN_M3.md item 6).
    ///
    /// The caller decides to relaunch by observing that nothing was ever
    /// launched under the reserved identities. Between that observation and
    /// this call, two things can have changed, and both must lose: a
    /// concurrent DELETE can have tombstoned the reservation (relaunching
    /// then would resurrect a session the user threw away), and the
    /// reserved row can have moved past `Launching` (evidence appeared, so
    /// the honest answer is a replay). Both conditions are re-tested INSIDE
    /// this transaction, which is what makes the takeover a real state
    /// transition rather than a check followed by a hopeful write.
    ///
    /// On [`RetryClaim::Acquired`] the previous launching row — which by
    /// the conditions above described nothing — is replaced by `row`, under
    /// the same id and tmux name the reservation already carries.
    pub async fn restart_pending_launch(
        &self,
        row: StoredSession,
        intent_key: &str,
    ) -> anyhow::Result<RetryClaim> {
        let conn = Arc::clone(&self.conn);
        let intent_key = intent_key.to_string();
        tokio::task::spawn_blocking(move || -> anyhow::Result<RetryClaim> {
            let mut conn = conn.lock().expect("session db mutex poisoned");
            let tx = conn
                .transaction()
                .context("beginning the relaunch takeover transaction")?;
            let reservation = read_reservation(&tx, &intent_key)?.ok_or_else(|| {
                anyhow::anyhow!("create reservation {intent_key} vanished before its relaunch")
            })?;
            if reservation.outcome != ReservationOutcome::Pending
                || reservation.session_id != row.id
            {
                return Ok(RetryClaim::Resolved(Box::new(reservation)));
            }
            let current: Option<(String, String)> = tx
                .query_row(
                    "SELECT outcome_state, pane FROM sessions WHERE id = ?1",
                    rusqlite::params![row.id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()
                .context("reading the reserved session's current state")?;
            // The same durable-row predicate `service`'s
            // `reserved_launch_evidence` applies, restated here because
            // this is where it becomes a TRANSITION rather than a reading:
            // a recorded pane means something saw this session in tmux, and
            // any outcome past `launching` means the same — except
            // `interrupted`, which the reboot conversion blankets over
            // never-launched rows too and which therefore proves nothing on
            // its own. Anything showing evidence refuses the takeover, so
            // the caller replays instead of starting a second agent.
            if current.is_some_and(|(state, pane)| {
                !pane.is_empty() || !matches!(state.as_str(), "launching" | "interrupted")
            }) {
                return Ok(RetryClaim::Launched);
            }
            tx.execute(
                "DELETE FROM sessions WHERE id = ?1",
                rusqlite::params![row.id],
            )
            .context("clearing the interrupted attempt's launching row")?;
            insert_session_row(&tx, &row)
                .context("re-inserting the launching row for a relaunch")?;
            tx.commit().context("committing the relaunch takeover")?;
            Ok(RetryClaim::Acquired)
        })
        .await
        .context("relaunch takeover task panicked")?
    }

    /// Open a NEW launch generation on an existing session (PLAN_M3.md item
    /// 9's restart), committed BEFORE the relaunch touches anything
    /// external — and only if the session still looks the way the caller
    /// validated it.
    ///
    /// This is the one write in this module that deliberately moves a
    /// TERMINAL outcome backwards, which is why it is a method of its own
    /// rather than a [`Transition`]: `Transition::apply` arbitrates
    /// OBSERVATIONS, and no observation may ever reopen an `Exited`,
    /// `Interrupted`, or `Error` row (see its docs). A restart is not an
    /// observation — it is a user-authorized new run of the same session,
    /// and the previous run's outcome stops describing anything the moment
    /// it begins.
    ///
    /// ## The offer condition
    ///
    /// `basis` is what the caller's mode validation was decided against:
    /// the captured identity and the ambiguity verdict, the only two
    /// mutable inputs to a session's restart offer (kind and template are
    /// immutable from create). The claim is CONDITIONAL on both still
    /// holding, which is what makes "validate the offer, then relaunch"
    /// atomic rather than merely sequential: a capture pass that commits
    /// `Resume` in between turns this into [`RelaunchDecision::OfferChanged`]
    /// and the caller refuses with a conflict, instead of launching the
    /// fresh agent the user chose against a session that has meanwhile
    /// become resumable.
    ///
    /// ## What the new generation clears, and why each
    ///
    /// - `generation` itself increments, monotonically. Every durable write
    ///   about the current run carries it (see [`StoredSession::generation`]),
    ///   so this single increment is what invalidates every in-flight
    ///   observation of the run being replaced.
    /// - the outcome becomes `Launching`, the same pre-side-effect
    ///   generation a create commits, so a crash straddling the relaunch
    ///   can never leave the PREVIOUS run's outcome standing over a session
    ///   that has since been relaunched (item 2's ordering rule);
    /// - the stop annotation goes with it, because it describes how the
    ///   previous run ended (item 4). The prior outcome is RETURNED rather
    ///   than merely dropped, so a relaunch that fails before touching
    ///   anything external can put it back verbatim
    ///   ([`SessionStore::abort_relaunch`]) — item 4's "only a SUCCESSFUL
    ///   restart clears it" is enforced by that pair, not by this call
    ///   alone;
    /// - the exit code and the error detail go for the same reason;
    /// - the pane is emptied, because the relaunch has not confirmed one
    ///   yet.
    /// - `reset_capture` additionally clears `first_input_at`, the captured
    ///   identity, its record locator, and the ambiguity verdict. Those are
    ///   PER-LAUNCH correlation state: a fresh (or fallback-template) run
    ///   starts a conversation of its own, and keeping the previous run's
    ///   first-input anchor would point the correlator at a window that
    ///   closed long ago — while keeping a stale ambiguity would deny the
    ///   new run any capture at all. A `Resume` relaunch passes `false`,
    ///   because reverifying the identity it is resuming is exactly what
    ///   the capture pass must go on doing.
    /// - `launch_scoped` is re-decided from `scope_available`, because the
    ///   selection belongs to a launch and not to a session (PLAN_M3.md item
    ///   10): a host that lost its user manager between two launches must
    ///   not leave the new run claiming a scope nothing created.
    ///
    /// The immutable create-time snapshot (kind, template, invocation, cwd)
    /// is untouched in every case.
    pub async fn begin_relaunch(
        &self,
        id: &str,
        basis: OfferBasis,
        reset_capture: bool,
        scope_available: bool,
    ) -> anyhow::Result<RelaunchDecision> {
        let conn = Arc::clone(&self.conn);
        let id = id.to_string();
        tokio::task::spawn_blocking(move || -> anyhow::Result<RelaunchDecision> {
            let mut conn = conn.lock().expect("session db mutex poisoned");
            let tx = conn
                .transaction()
                .context("beginning the relaunch generation transaction")?;
            let current: Option<RelaunchBasisColumns> = tx
                .query_row(
                    "SELECT outcome_state, exit_code, annotation, error_detail, pane, \
                     generation, captured_conversation, capture_ambiguous, launch_scoped \
                     FROM sessions WHERE id = ?1",
                    rusqlite::params![id],
                    |r| {
                        Ok((
                            (r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?),
                            r.get(4)?,
                            r.get(5)?,
                            r.get(6)?,
                            r.get(7)?,
                            r.get(8)?,
                        ))
                    },
                )
                .optional()
                .context("reading the session a restart is about to relaunch")?;
            let Some((
                (state, exit_code, annotation, error_detail),
                pane,
                generation,
                captured_conversation,
                capture_ambiguous,
                scoped,
            )) = current
            else {
                return Ok(RelaunchDecision::Gone);
            };
            if captured_conversation != basis.captured_conversation
                || (capture_ambiguous != 0) != basis.capture_ambiguous
            {
                return Ok(RelaunchDecision::OfferChanged);
            }
            let prior = PriorRun {
                outcome: LastOutcome::from_columns(&state, exit_code, annotation, error_detail)
                    .with_context(|| format!("session {id}"))?,
                pane,
                scoped: scoped != 0,
            };
            let generation = generation + 1;
            let (state, exit_code, annotation, error_detail) = LastOutcome::Launching.columns();
            // One statement rather than two near-identical ones: the capture
            // columns are cleared by an expression that is a no-op when the
            // relaunch is resuming, so the SQL cannot drift between the two
            // cases the way two copies of it could.
            tx.execute(
                "UPDATE sessions SET outcome_state = ?2, exit_code = ?3, annotation = ?4, \
                 error_detail = ?5, pane = '', generation = ?6, launch_scoped = ?7, \
                 first_input_at = CASE WHEN ?8 THEN NULL ELSE first_input_at END, \
                 captured_conversation = \
                     CASE WHEN ?8 THEN NULL ELSE captured_conversation END, \
                 captured_record = CASE WHEN ?8 THEN NULL ELSE captured_record END, \
                 capture_ambiguous = CASE WHEN ?8 THEN 0 ELSE capture_ambiguous END \
                 WHERE id = ?1",
                rusqlite::params![
                    id,
                    state,
                    exit_code,
                    annotation,
                    error_detail,
                    generation,
                    i64::from(scope_available),
                    i64::from(reset_capture),
                ],
            )
            .context("opening a new launch generation for a restart")?;
            tx.commit().context("committing the launch generation")?;
            Ok(RelaunchDecision::Claimed(RelaunchClaim {
                generation,
                prior,
                scoped: scope_available,
            }))
        })
        .await
        .context("relaunch generation task panicked")?
    }

    /// Put back the outcome a [`SessionStore::begin_relaunch`] replaced,
    /// for a relaunch that failed BEFORE it could change anything outside
    /// this database.
    ///
    /// This is the other half of item 4's contract: a restart clears the
    /// previous run's annotation (and exit code, and error detail) only if
    /// it SUCCEEDS, and the generation has to be opened before any side
    /// effect for the ordering rule — so the only way to have both is to
    /// restore on the failures that are provably harmless to restore on.
    /// The caller decides which those are: a launch artifact that could not
    /// be cleared, a spec that never landed, a tmux refusal with the
    /// session CONFIRMED absent. An ambiguous failure — anything that may
    /// have left an agent running — must NOT restore, because the row would
    /// then describe a run that is not the one actually alive; those stay
    /// `Launching` for reload to reconcile.
    ///
    /// Conditional on `generation` still being current, so a restore can
    /// never step on a LATER restart that has since claimed the session:
    /// by the time this loses that race, the newer generation's own outcome
    /// is the truth and this one's prior run is ancient history. The
    /// generation itself is deliberately NOT rolled back — it is monotonic,
    /// and a reused number is exactly what would let a stale sentinel or a
    /// stale observation land on a future launch.
    ///
    /// `Ok(false)` means the restore did not apply (the row is gone, or a
    /// newer generation owns it); the caller reports its original failure
    /// either way.
    pub async fn abort_relaunch(
        &self,
        id: &str,
        generation: i64,
        prior: &PriorRun,
    ) -> anyhow::Result<bool> {
        let conn = Arc::clone(&self.conn);
        let id = id.to_string();
        let (state, exit_code, annotation, error_detail) = prior.outcome.columns();
        let (state, annotation, error_detail) = (
            state.to_string(),
            annotation.map(str::to_string),
            error_detail.map(str::to_string),
        );
        let pane = prior.pane.clone();
        let scoped = i64::from(prior.scoped);
        tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
            let conn = conn.lock().expect("session db mutex poisoned");
            let restored = conn
                .execute(
                    "UPDATE sessions SET outcome_state = ?2, exit_code = ?3, annotation = ?4, \
                     error_detail = ?5, pane = ?6, launch_scoped = ?8 \
                     WHERE id = ?1 AND generation = ?7",
                    rusqlite::params![
                        id,
                        state,
                        exit_code,
                        annotation,
                        error_detail,
                        pane,
                        generation,
                        scoped
                    ],
                )
                .context("restoring the outcome a failed restart replaced")?;
            Ok(restored > 0)
        })
        .await
        .context("relaunch abort task panicked")?
    }

    /// One session's stored row, if it still exists.
    ///
    /// The DURABLE answer to "does this session exist", which every replay
    /// asks: the in-memory session map is a mirror that a delete updates
    /// only after its own commit, so a replay landing in that window would
    /// otherwise hand back a live-looking session whose row is already
    /// gone.
    pub async fn session(&self, id: &str) -> anyhow::Result<Option<StoredSession>> {
        let conn = Arc::clone(&self.conn);
        let id = id.to_string();
        tokio::task::spawn_blocking(move || -> anyhow::Result<Option<StoredSession>> {
            let conn = conn.lock().expect("session db mutex poisoned");
            // Two stages for the same reason `load_all` uses them: a
            // corrupt outcome must be refused with its own message rather
            // than flattened into a rusqlite decode failure.
            let raw = conn
                .query_row(
                    &format!("SELECT {SESSION_COLUMNS} FROM sessions WHERE id = ?1"),
                    rusqlite::params![id],
                    read_session_columns,
                )
                .optional()
                .context("reading a session row")?;
            raw.map(decode_session_row).transpose()
        })
        .await
        .context("session read task panicked")?
    }

    /// The reservation for `intent_key`, if this state directory has ever
    /// seen it — the one lookup `service::Supervisor::create_session`'s
    /// state machine runs before deciding whether a create is a first
    /// attempt, a replay, or a retry with reconciling to do.
    pub async fn reservation(&self, intent_key: &str) -> anyhow::Result<Option<Reservation>> {
        let conn = Arc::clone(&self.conn);
        let intent_key = intent_key.to_string();
        tokio::task::spawn_blocking(move || -> anyhow::Result<Option<Reservation>> {
            let conn = conn.lock().expect("session db mutex poisoned");
            read_reservation(&conn, &intent_key)
        })
        .await
        .context("reservation read task panicked")?
    }

    /// Every reservation still `Pending` — the reconciliation worklist
    /// `service::Supervisor::reload_sessions` resolves against the session
    /// map it has just rebuilt from tmux.
    ///
    /// Only pending rows, because a settled outcome is final: nothing a
    /// reload observes could move a `Created` or `Failed` row, and loading
    /// them anyway would mean scanning every create this state directory
    /// has ever done, on every startup, forever.
    pub async fn pending_reservations(&self) -> anyhow::Result<Vec<Reservation>> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<Reservation>> {
            let conn = conn.lock().expect("session db mutex poisoned");
            let mut stmt = conn
                .prepare(
                    "SELECT intent_key, fingerprint, session_id, tmux_name \
                     FROM create_reservations WHERE state = 'pending'",
                )
                .context("preparing the pending-reservation query")?;
            let rows = stmt
                .query_map([], |r| {
                    Ok(Reservation {
                        intent_key: r.get(0)?,
                        fingerprint: r.get(1)?,
                        session_id: r.get(2)?,
                        tmux_name: r.get(3)?,
                        outcome: ReservationOutcome::Pending,
                    })
                })
                .context("querying pending reservations")?
                .collect::<Result<Vec<_>, rusqlite::Error>>()
                .context("decoding pending reservations")?;
            Ok(rows)
        })
        .await
        .context("pending reservation query task panicked")?
    }

    /// Record the outcome of one or more reservations, in ONE transaction.
    ///
    /// Two conditions guard every write, and both are about a settlement
    /// arriving later than the world it was computed against:
    ///
    /// - **`state = 'pending'`** makes settlement MONOTONIC: an outcome,
    ///   once recorded, is what every later replay reports. What this
    ///   protects against is not a live create racing a reload — a reload
    ///   only ever runs before this process serves anything, so that race
    ///   cannot happen — but the repeated and STALE settlements that
    ///   genuinely do occur: a retry re-settling what a previous attempt
    ///   already recorded, and a settlement computed before a concurrent
    ///   delete tombstoned the same row.
    /// - **`session_id = ?`** ([`Settlement`]) keeps a settlement from
    ///   landing on a reservation that has since been re-pointed at a
    ///   different attempt's identities.
    ///
    /// A row matching neither is left alone rather than reported: there is
    /// no honest repair for "the reservation I was told about is not the
    /// one that is there", and inventing one would be worse than the no-op.
    ///
    /// Batched for the same reason [`SessionStore::transition_many`] is:
    /// one reload can settle a whole startup's worth of interrupted
    /// creates, and a journal sync per row buys nothing. The batch is
    /// atomic — a failure part-way through settles NONE of them, so a
    /// reload never leaves half its reconciliation recorded.
    pub async fn settle_reservations(&self, settlements: Vec<Settlement>) -> anyhow::Result<()> {
        if settlements.is_empty() {
            return Ok(());
        }
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let mut conn = conn.lock().expect("session db mutex poisoned");
            let tx = conn
                .transaction()
                .context("beginning the reservation settlement transaction")?;
            for settlement in settlements {
                settle_within(&tx, &settlement)?;
            }
            tx.commit().context("committing reservation settlements")?;
            Ok(())
        })
        .await
        .context("reservation settlement task panicked")?
    }

    /// [`SessionStore::delete_session`] for the DELETE path, settling this
    /// session's still-pending reservations in the SAME transaction as the
    /// row removal (PLAN_M3.md item 6's tombstone rule).
    ///
    /// The settlement direction is `Created`, and it is a statement of
    /// fact rather than a courtesy: a session cannot be deleted without
    /// having existed, so a reservation still claiming to be in flight for
    /// it was merely never told its launch had succeeded. Recording that
    /// here is what keeps the tombstone honest — a later replay finds
    /// `Created` with no session behind it and reports the gone-error,
    /// where leaving the row `Pending` would instead tell the retry "the
    /// crashed attempt never launched" and produce exactly the duplicate
    /// this mechanism exists to exclude.
    ///
    /// The reservation ROW is deliberately kept; see [`Reservation`]'s docs
    /// on tombstones. This is a separate method from plain
    /// [`SessionStore::delete_session`] because that one is ALSO the create
    /// path's rollback (`service`'s `abandon_launching_record`), where the
    /// launch provably did not happen and settling its reservation
    /// `Created` would be a lie in the opposite direction.
    pub async fn delete_session_settling_reservations(&self, id: &str) -> anyhow::Result<()> {
        let conn = Arc::clone(&self.conn);
        let id = id.to_string();
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let mut conn = conn.lock().expect("session db mutex poisoned");
            let tx = conn
                .transaction()
                .context("beginning the session delete transaction")?;
            tx.execute(
                "UPDATE create_reservations SET state = 'created' \
                 WHERE session_id = ?1 AND state = 'pending'",
                rusqlite::params![id],
            )
            .context("settling the deleted session's reservations")?;
            tx.execute("DELETE FROM sessions WHERE id = ?1", rusqlite::params![id])
                .context("deleting session row")?;
            tx.commit().context("committing the session delete")?;
            Ok(())
        })
        .await
        .context("session delete task panicked")?
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
    /// `generation` is the launch this observation is ABOUT — the value the
    /// caller read from the row or the session entry it observed. A
    /// transition whose generation is no longer the row's current one is
    /// silently dropped (the committed outcome is still returned), because
    /// it describes a run this session has already moved past: the classic
    /// case is a `ListSessions` pass that cloned an entry, went to tmux,
    /// and came back with the OLD pane's death after a restart replaced it
    /// (see [`StoredSession::generation`]).
    ///
    /// `Ok(None)` means the row no longer exists (a concurrent delete);
    /// that is not an error, exactly as `delete_session` tolerates a
    /// missing row.
    pub async fn transition(
        &self,
        id: &str,
        generation: i64,
        transition: Transition,
    ) -> anyhow::Result<Option<LastOutcome>> {
        let committed = self
            .transition_many(vec![(id.to_string(), generation, transition)])
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
        transitions: Vec<(String, i64, Transition)>,
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
            for (id, generation, transition) in transitions {
                let current: Option<(OutcomeColumns, i64)> = tx
                    .query_row(
                        "SELECT outcome_state, exit_code, annotation, error_detail, generation \
                             FROM sessions WHERE id = ?1",
                        rusqlite::params![id],
                        |r| Ok(((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?), r.get(4)?)),
                    )
                    .optional()
                    .context("reading the current outcome")?;
                let Some(((state, exit_code, annotation, error_detail), current_generation)) =
                    current
                else {
                    continue;
                };
                let current =
                    LastOutcome::from_columns(&state, exit_code, annotation, error_detail)
                        .with_context(|| format!("session {id}"))?;
                // The generation fence (see `StoredSession::generation`).
                // Reported as the CURRENT outcome rather than as an error
                // or an absence: the caller's observation was simply about
                // a run this session has moved past, and what it should
                // mirror is what is true now.
                if generation != current_generation {
                    committed.insert(id, current);
                    continue;
                }
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

    /// Set a session's title (PLAN_M5.md item 3's durable half), reporting
    /// whether a row was there to update.
    ///
    /// The title is the one piece of session metadata a user may change
    /// after creation, which is why this is the store's only unconditional
    /// metadata UPDATE: the capture columns beside it are write-once by SQL
    /// predicate (see [`SessionStore::record_captured_conversation`]) and
    /// the snapshot columns have no update path at all, but a rename is a
    /// deliberate overwrite of a label whose previous value carries no
    /// authority. Concurrent renames are therefore last-write-wins, with no
    /// version token to make one of them fail — `ControlMsg::RenameSession`
    /// argues that choice out.
    ///
    /// Deliberately NOT fenced by `generation`, unlike every capture and
    /// outcome write: those record something witnessed about ONE launch, so
    /// a write from a pass that straddled a restart would attach the
    /// previous run's conclusion to the new one. A title belongs to the
    /// SESSION across all of its launches, so fencing it would make a
    /// rename issued moments before a restart silently vanish.
    ///
    /// `false` means no such row — a session deleted out from under the
    /// caller. Reported rather than swallowed so the handler can answer
    /// `NotFound` instead of confirming a rename that changed nothing.
    pub async fn set_session_title(&self, id: &str, title: &str) -> anyhow::Result<bool> {
        let conn = Arc::clone(&self.conn);
        let id = id.to_string();
        let title = title.to_string();
        tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
            let conn = conn.lock().expect("session db mutex poisoned");
            Ok(conn
                .execute(
                    "UPDATE sessions SET title = ?2 WHERE id = ?1",
                    rusqlite::params![id, title],
                )
                .context("renaming a session")?
                > 0)
        })
        .await
        .context("session rename task panicked")?
    }

    /// Record when this session first had input forwarded to it
    /// (PLAN_M3.md item 8's correlator), if nothing has recorded it yet.
    ///
    /// Write-once by SQL predicate rather than by caller discipline, and
    /// the direction matters: the FIRST input is what the agents' records
    /// appear after, so a later observation must never move the timestamp
    /// forward — that would slide the capture window past the very record
    /// it exists to match. The in-memory mirror on the session entry
    /// enforces the same thing for the common case; this predicate is what
    /// makes it true across a restart, where the mirror is gone and the
    /// stored value is all there is.
    ///
    /// Deliberately NOT part of `Transition`: this is not something
    /// witnessed about the agent's lifecycle, it is a fact about what this
    /// supervisor did, and folding it into the outcome state machine would
    /// mean every transition had to reason about a field none of them can
    /// change.
    /// `generation` fences the write to the launch the caller observed: a
    /// relaunch clears this anchor for its own new window (see
    /// [`SessionStore::begin_relaunch`]), and an input frame that was
    /// in flight across that boundary must not write the previous run's
    /// anchor back onto it.
    pub async fn record_first_input(
        &self,
        id: &str,
        generation: i64,
        at_unix: i64,
    ) -> anyhow::Result<()> {
        let conn = Arc::clone(&self.conn);
        let id = id.to_string();
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let conn = conn.lock().expect("session db mutex poisoned");
            conn.execute(
                "UPDATE sessions SET first_input_at = ?2 \
                 WHERE id = ?1 AND first_input_at IS NULL AND generation = ?3",
                rusqlite::params![id, at_unix, generation],
            )
            .context("recording a session's first-input time")?;
            Ok(())
        })
        .await
        .context("first-input record task panicked")?
    }

    /// Claim a conversation identity for a session (PLAN_M3.md item 8), if
    /// none is claimed yet and nothing has declared the correlation
    /// ambiguous.
    ///
    /// Write-once for the reason SPEC.md's resume promise is per-session
    /// and exact: once an identity is claimed, the only thing a later scan
    /// could honestly do is confirm it. A fork writes a NEW id to a NEW
    /// record, and letting that overwrite this column would silently move
    /// a session onto a conversation it never ran — precisely the
    /// silently-wrong-conversation resume the capture design exists to
    /// exclude. Re-verification after an append therefore compares; it
    /// never rewrites.
    ///
    /// The `capture_ambiguous = 0` condition is the durable half of
    /// ambiguity dominance: a claim computed before something (this
    /// process, or a previous one) found the correlation ambiguous must
    /// LOSE, and the only place that can be arbitrated without a race is
    /// inside this transaction.
    ///
    /// Returns the identity now committed, whoever wrote it, so an
    /// in-memory mirror follows what the database actually says rather
    /// than what this caller intended (the same rule `transition`
    /// follows). That read-back is also what makes it safe to advertise
    /// `RestartOffer::Resume`: the offer means "there is a stored identity
    /// this restart can fill in", and only a value read back out of the
    /// committed row establishes that. `Ok(None)` means either the row is
    /// gone (a concurrent delete) or the claim lost to an ambiguity —
    /// neither is an error, and the caller distinguishes them by looking
    /// at nothing at all: in both cases it must not advertise Resume.
    /// `generation` fences the claim to the launch the correlating pass
    /// actually observed: a pass that started before a restart must not
    /// commit the previous run's identity onto the new one, which for a
    /// Fresh relaunch would be exactly the silently-wrong-conversation
    /// resume SPEC.md forbids. A fenced-out claim reads back whatever the
    /// current generation holds, so the caller's mirror still follows the
    /// database.
    pub async fn record_captured_conversation(
        &self,
        id: &str,
        generation: i64,
        conversation: &str,
        record: &Path,
    ) -> anyhow::Result<Option<String>> {
        let conn = Arc::clone(&self.conn);
        let id = id.to_string();
        let conversation = conversation.to_string();
        let record = record.to_string_lossy().into_owned();
        tokio::task::spawn_blocking(move || -> anyhow::Result<Option<String>> {
            let mut conn = conn.lock().expect("session db mutex poisoned");
            let tx = conn
                .transaction()
                .context("beginning the capture transaction")?;
            tx.execute(
                "UPDATE sessions SET captured_conversation = ?2, captured_record = ?3 \
                 WHERE id = ?1 AND captured_conversation IS NULL AND capture_ambiguous = 0 \
                 AND generation = ?4",
                rusqlite::params![id, conversation, record, generation],
            )
            .context("recording a captured conversation identity")?;
            let committed: Option<Option<String>> = tx
                .query_row(
                    "SELECT captured_conversation FROM sessions WHERE id = ?1",
                    rusqlite::params![id],
                    |r| r.get(0),
                )
                .optional()
                .context("reading back the captured conversation identity")?;
            tx.commit().context("committing the capture")?;
            Ok(committed.flatten())
        })
        .await
        .context("capture record task panicked")?
    }

    /// Record durably that this session's correlation was AMBIGUOUS, so no
    /// identity will ever be claimed for this launch (PLAN_M3.md item 8).
    ///
    /// Ambiguity DOMINATES, which is why this write has no precondition
    /// beyond the row existing: it can never be wrong to refuse, and the
    /// only monotonic direction is toward refusing. Its durability is what
    /// keeps a restart from re-deciding on worse evidence — see
    /// [`StoredSession::capture_ambiguous`].
    ///
    /// It also clears any identity a racing writer had just claimed. That
    /// looks like a violation of the write-once rule above and is in fact
    /// the same rule: `captured_conversation` is write-once against
    /// *later, poorer* evidence, and an ambiguity is never poorer — it is
    /// the discovery that the claim should not have been made. In
    /// practice the two cannot race (passes are serialized), so this is
    /// the belt to that suspenders.
    /// `generation` fences it like every other capture write: an ambiguity
    /// established about one run says nothing about the next one, and a
    /// relaunch that opened a fresh capture window has already cleared it.
    pub async fn record_capture_ambiguous(&self, id: &str, generation: i64) -> anyhow::Result<()> {
        let conn = Arc::clone(&self.conn);
        let id = id.to_string();
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let conn = conn.lock().expect("session db mutex poisoned");
            conn.execute(
                "UPDATE sessions SET capture_ambiguous = 1, captured_conversation = NULL, \
                 captured_record = NULL WHERE id = ?1 AND generation = ?2",
                rusqlite::params![id, generation],
            )
            .context("recording an ambiguous conversation correlation")?;
            Ok(())
        })
        .await
        .context("capture ambiguity task panicked")?
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
    ///
    /// This is the CREATE-ROLLBACK delete, not the user-facing one, and
    /// `settlement` is what makes the distinction safe to express in one
    /// method: the rollback runs where a launch provably did NOT happen, so
    /// its reservation is settled `Failed` (or left pending — see
    /// `service`'s retention rules), never told a session was created the
    /// way [`SessionStore::delete_session_settling_reservations`] does.
    ///
    /// The settlement rides the SAME transaction as the removal because
    /// the two are one fact: a rollback whose settlement did not commit
    /// would leave a reservation pointing at a row that no longer exists,
    /// and the retry that found it would relaunch under an intent the
    /// client was already told had failed. `None` is a rollback with no
    /// intent key to settle.
    pub async fn delete_session(
        &self,
        id: &str,
        settlement: Option<Settlement>,
    ) -> anyhow::Result<()> {
        let conn = Arc::clone(&self.conn);
        let id = id.to_string();
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let mut conn = conn.lock().expect("session db mutex poisoned");
            let tx = conn
                .transaction()
                .context("beginning the launch rollback transaction")?;
            tx.execute("DELETE FROM sessions WHERE id = ?1", rusqlite::params![id])
                .context("deleting session row")?;
            if let Some(settlement) = &settlement {
                settle_within(&tx, settlement)?;
            }
            tx.commit().context("committing the launch rollback")?;
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
                .prepare(&format!("SELECT {SESSION_COLUMNS} FROM sessions"))
                .context("preparing session load query")?;
            // Two stages, not one: the outcome, kind, and template columns
            // are reassembled by functions that return `anyhow::Error` for
            // a corrupt row and so cannot live inside a rusqlite row mapper
            // (whose error type is rusqlite's own). Collecting the raw
            // tuples first keeps the refusal-to-guess behavior and its
            // message intact instead of flattening it into a generic decode
            // failure.
            let raw = stmt
                .query_map([], read_session_columns)
                .context("querying sessions")?
                .collect::<Result<Vec<_>, rusqlite::Error>>()
                .context("decoding session rows")?;
            raw.into_iter().map(decode_session_row).collect()
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
        insert_running_with_scope(store, id, false).await;
    }

    /// Seed a running session with an explicit `launch_scoped` value —
    /// `insert_running`'s twin, needed because every other fixture in this
    /// module hardcodes `false` and the PLAN_M3.md item 10 tests below need
    /// to start from either side (a prior scoped launch, or a prior
    /// unscoped one) before driving a relaunch across it.
    async fn insert_running_with_scope(store: &SessionStore, id: &str, scoped: bool) {
        store
            .insert_session(
                StoredSession {
                    id: id.to_string(),
                    title: id.to_string(),
                    cwd: "/tmp/work".to_string(),
                    invocation: "agent".to_string(),
                    tmux_name: format!("fh-{id}"),
                    pane: "%0".to_string(),
                    outcome: LastOutcome::Running,
                    agent_kind: farhelm_proto::AgentKind::Generic,
                    resume_template: None,
                    canonical_cwd: None,
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: scoped,
                },
                None,
            )
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

    /// Every index, as SQLite itself describes it: the creating DDL (which
    /// is the only place a PARTIAL index's `WHERE` clause appears) plus the
    /// columns and uniqueness `index_list`/`index_info` report.
    ///
    /// Both, not either: the DDL alone would not notice a difference in
    /// what SQLite actually built from it, and the pragmas alone cannot see
    /// the predicate that makes an index partial in the first place.
    fn indexes_of(path: &Path) -> Vec<(String, Option<String>, i64, Vec<String>)> {
        let conn = Connection::open(path).expect("open raw");
        let named: Vec<(String, Option<String>)> = conn
            .prepare("SELECT name, sql FROM sqlite_schema WHERE type = 'index' ORDER BY name")
            .expect("prepare")
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .expect("query")
            .collect::<Result<_, _>>()
            .expect("collect");
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_schema WHERE type = 'table' ORDER BY name")
            .expect("prepare")
            .query_map([], |r| r.get(0))
            .expect("query")
            .collect::<Result<_, _>>()
            .expect("collect");
        let mut unique: HashMap<String, i64> = HashMap::new();
        for table in tables {
            let mut stmt = conn
                .prepare(&format!("PRAGMA index_list({table})"))
                .expect("prepare index_list");
            for row in stmt
                .query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, i64>(2)?)))
                .expect("query index_list")
            {
                let (name, is_unique) = row.expect("index_list row");
                unique.insert(name, is_unique);
            }
        }
        named
            .into_iter()
            .map(|(name, sql)| {
                let mut stmt = conn
                    .prepare(&format!("PRAGMA index_info({name})"))
                    .expect("prepare index_info");
                let columns: Vec<String> = stmt
                    .query_map([], |r| r.get::<_, Option<String>>(2))
                    .expect("query index_info")
                    .map(|c| c.expect("index_info row").unwrap_or_default())
                    .collect();
                let is_unique = unique.get(&name).copied().unwrap_or(-1);
                (name, sql, is_unique, columns)
            })
            .collect()
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

    /// The same rule at the 2 → 3 step, which is where every FUTURE
    /// migration will live: a failure leaves the database at the version it
    /// actually has, with none of the step's objects half-created.
    ///
    /// Worth pinning separately from the 1 → 2 test because this step
    /// creates an INDEX as well as a table, and an index is exactly the
    /// kind of object it is tempting to add outside the transaction. The
    /// conflict is planted on the index name rather than the table so the
    /// rollback has to undo a `CREATE TABLE` that itself succeeded.
    #[tokio::test]
    async fn a_failed_schema_3_migration_leaves_the_database_at_version_2() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("supervisor.db");
        plant_v1_database(&db_path, &[("s1", "fh-1", "%0")]);
        {
            // Migrate to 2 only, then block the 3 step. `PRAGMA
            // user_version = 2` is what stops `apply_schema` from
            // continuing past the step under test.
            let conn = Connection::open(&db_path).expect("raw open");
            conn.execute_batch(
                "ALTER TABLE sessions ADD COLUMN outcome_state TEXT NOT NULL DEFAULT 'launching';
                 ALTER TABLE sessions ADD COLUMN exit_code     INTEGER;
                 ALTER TABLE sessions ADD COLUMN annotation    TEXT;
                 ALTER TABLE sessions ADD COLUMN error_detail  TEXT;
                 CREATE TABLE supervisor_meta (
                     id      INTEGER PRIMARY KEY CHECK (id = 0),
                     boot_id TEXT
                 ) STRICT;
                 CREATE TABLE decoy (x TEXT);
                 CREATE INDEX create_reservations_pending ON decoy (x);
                 PRAGMA user_version = 2;",
            )
            .expect("plant a version-2 database with a conflicting index name");
        }

        SessionStore::open(&db_path, true)
            .await
            .expect_err("the migration must fail on the conflicting index name");

        let conn = Connection::open(&db_path).expect("raw open");
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .expect("read version");
        assert_eq!(
            version, 2,
            "a rolled-back migration must not claim version 3"
        );
        assert!(
            conn.prepare("SELECT intent_key FROM create_reservations")
                .is_err(),
            "the table must have rolled back with the index that failed after it"
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
        // Indexes too, not just columns: the reservation table's index is
        // PARTIAL, so a migration that created an unqualified one would
        // still match column-for-column while quietly indexing an immortal
        // table in full. `sqlite_schema.sql` is what carries the `WHERE`
        // clause at all — `index_list`/`index_info` describe the columns
        // and uniqueness but not the predicate — so both are compared.
        assert_eq!(indexes_of(&migrated), indexes_of(&fresh));

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
            .transition("s1", 0, Transition::ObservedExit { exit_code: Some(1) })
            .await
            .expect("transition");
        assert_eq!(committed, Some(LastOutcome::Interrupted));
        assert_eq!(outcome_of(&store, "s1").await, LastOutcome::Interrupted);

        // A row deleted concurrently is not an error and has no committed
        // outcome to report.
        store.delete_session("s1", None).await.expect("delete");
        assert_eq!(
            store
                .transition("s1", 0, Transition::ObservedExit { exit_code: None })
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
            .insert_session(
                StoredSession {
                    id: "s1".to_string(),
                    title: "demo".to_string(),
                    cwd: "/tmp/work".to_string(),
                    invocation: "agent".to_string(),
                    tmux_name: "fh-1".to_string(),
                    pane: String::new(),
                    outcome: LastOutcome::Launching,
                    agent_kind: farhelm_proto::AgentKind::Generic,
                    resume_template: None,
                    canonical_cwd: None,
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: false,
                },
                None,
            )
            .await
            .expect("insert");

        store
            .transition("s1", 0, Transition::ConfirmRunning { pane: "%4".into() })
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
            .insert_session(
                StoredSession {
                    id: "s1".to_string(),
                    title: "demo".to_string(),
                    cwd: "/tmp/work".to_string(),
                    invocation: "agent".to_string(),
                    tmux_name: "fh-1".to_string(),
                    pane: String::new(),
                    outcome: LastOutcome::Launching,
                    agent_kind: farhelm_proto::AgentKind::Generic,
                    resume_template: None,
                    canonical_cwd: None,
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: false,
                },
                None,
            )
            .await
            .expect("insert");

        store
            .transition(
                "s1",
                0,
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
            .transition("a", 0, Transition::StopRequested)
            .await
            .expect("intent");
        store
            .transition("a", 0, Transition::ObservedExit { exit_code: Some(0) })
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
            .transition("b", 0, Transition::ObservedExit { exit_code: Some(0) })
            .await
            .expect("list observation");
        store
            .transition("b", 0, Transition::StopCompleted { exit_code: None })
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
                .transition(id, 0, Transition::StopCompleted { exit_code: None })
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
                0,
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
                    0,
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
                0,
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
            .insert_session(
                StoredSession {
                    id: "s1".to_string(),
                    title: "demo".to_string(),
                    cwd: "/tmp/work".to_string(),
                    invocation: "agent --flag".to_string(),
                    tmux_name: "fh-abc".to_string(),
                    pane: "%3".to_string(),
                    outcome: LastOutcome::Running,
                    agent_kind: farhelm_proto::AgentKind::Generic,
                    resume_template: None,
                    canonical_cwd: None,
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: false,
                },
                None,
            )
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

        store.delete_session("s1", None).await.expect("delete");
        assert!(
            store.load_all().await.expect("load").is_empty(),
            "deleted row must not survive a reload"
        );

        // Deleting again (an already-deleted row is, by now, exactly the
        // same "no matching row" case as an id that never existed at
        // all — one call suffices for both) must not error.
        store
            .delete_session("s1", None)
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

    // -----------------------------------------------------------------
    // PLAN_M3.md item 6: create reservations.
    // -----------------------------------------------------------------

    /// A launching row in the shape `create_session` commits one.
    fn launching_row(id: &str) -> StoredSession {
        StoredSession {
            canonical_cwd: None,
            captured_record: None,
            capture_ambiguous: false,
            id: id.to_string(),
            title: id.to_string(),
            cwd: "/tmp/work".to_string(),
            invocation: "agent".to_string(),
            tmux_name: format!("fh-{id}"),
            pane: String::new(),
            outcome: LastOutcome::Launching,
            agent_kind: farhelm_proto::AgentKind::Generic,
            resume_template: None,
            captured_conversation: None,
            first_input_at: None,
            generation: 0,
            launch_scoped: false,
        }
    }

    /// Seed one reserved launch: a launching session row plus the pending
    /// claim that reserved it, exactly as `create_session`'s first step
    /// commits them.
    async fn insert_reserved(store: &SessionStore, id: &str, key: &str, fingerprint: &str) {
        let claimed = store
            .insert_session(
                launching_row(id),
                Some(IntentClaim {
                    intent_key: key.to_string(),
                    fingerprint: fingerprint.to_string(),
                }),
            )
            .await
            .expect("insert with claim");
        assert_eq!(claimed, Claimed::Ours, "the key {key} must have been free");
    }

    /// One settlement for a reservation seeded by [`insert_reserved`].
    fn settlement(key: &str, id: &str, outcome: ReservationOutcome) -> Settlement {
        Settlement {
            intent_key: key.to_string(),
            session_id: id.to_string(),
            outcome,
        }
    }

    /// Read one reservation, insisting it exists.
    async fn reservation_of(store: &SessionStore, key: &str) -> Reservation {
        store
            .reservation(key)
            .await
            .expect("read")
            .unwrap_or_else(|| panic!("reservation {key} must exist"))
    }

    /// Every reservation shape survives the on-disk round trip, error kind
    /// included.
    ///
    /// The kind is the part worth pinning: a replay must reproduce the
    /// FIRST attempt's answer exactly, and a kind that decayed to
    /// `Internal` in storage would silently turn a 400 into a 500 for a
    /// byte-identical request. Every `ErrorKind` is exercised so a new
    /// variant with no on-disk spelling fails here rather than in the field.
    #[tokio::test]
    async fn every_reservation_outcome_shape_round_trips() {
        let (_dir, store) = fresh_store().await;
        let mut expected = vec![
            ("pending-key", ReservationOutcome::Pending),
            ("created-key", ReservationOutcome::Created),
        ];
        expected.extend(
            [
                farhelm_proto::ErrorKind::NotFound,
                farhelm_proto::ErrorKind::InvalidRequest,
                farhelm_proto::ErrorKind::Internal,
                farhelm_proto::ErrorKind::Conflict,
            ]
            .into_iter()
            .zip(["failed-0", "failed-1", "failed-2", "failed-3"])
            .map(|(kind, key)| {
                (
                    key,
                    ReservationOutcome::Failed {
                        kind,
                        message: format!("it failed with {kind:?}"),
                    },
                )
            }),
        );

        for (index, (key, outcome)) in expected.iter().enumerate() {
            let id = format!("s{index}");
            insert_reserved(&store, &id, key, "fp").await;
            if *outcome != ReservationOutcome::Pending {
                store
                    .settle_reservations(vec![settlement(key, &id, outcome.clone())])
                    .await
                    .expect("settle");
            }
        }

        for (index, (key, outcome)) in expected.iter().enumerate() {
            let read = reservation_of(&store, key).await;
            assert_eq!(read.outcome, *outcome, "outcome of {key}");
            assert_eq!(read.session_id, format!("s{index}"));
            assert_eq!(read.tmux_name, format!("fh-s{index}"));
            assert_eq!(read.fingerprint, "fp");
        }
        assert_eq!(
            store.reservation("never-claimed").await.expect("read"),
            None,
            "a key this state directory has never seen has no reservation"
        );
    }

    /// A key already claimed loses the race as a VALUE, not an error, and
    /// takes nothing with it.
    ///
    /// Both halves matter. The session row must roll back — one left
    /// behind by a refused claim is an orphan nothing will ever reconcile,
    /// while the caller was told the create failed. And the answer must
    /// carry the WINNER's reservation, because that is the only honest
    /// reply available to the loser: it is exactly what a replay of that
    /// key returns.
    #[tokio::test]
    async fn a_lost_claim_race_rolls_back_and_reports_the_winner() {
        let (_dir, store) = fresh_store().await;
        insert_reserved(&store, "first", "shared-key", "fp").await;

        let claimed = store
            .insert_session(
                launching_row("second"),
                Some(IntentClaim {
                    intent_key: "shared-key".to_string(),
                    fingerprint: "fp".to_string(),
                }),
            )
            .await
            .expect("a lost claim is an answer, not a failure");
        let Claimed::TakenBy(winner) = claimed else {
            panic!("a key already claimed must report its winner: {claimed:?}");
        };
        assert_eq!(winner.session_id, "first");
        assert_eq!(winner.outcome, ReservationOutcome::Pending);

        let ids: Vec<String> = store
            .load_all()
            .await
            .expect("load")
            .into_iter()
            .map(|row| row.id)
            .collect();
        assert_eq!(
            ids,
            vec!["first".to_string()],
            "the refused insert must leave no session row behind"
        );
        assert_eq!(
            reservation_of(&store, "shared-key").await.session_id,
            "first",
            "and must not re-point the existing intent at a new session"
        );
    }

    /// Settlement is monotonic AND identity-conditioned; both guards are
    /// about a settlement arriving later than the world it was computed
    /// against.
    ///
    /// Monotonicity is what makes repeated and stale settlements harmless —
    /// a retry re-settling what a previous attempt already recorded, or a
    /// settlement computed before a concurrent delete tombstoned the same
    /// row. (It is NOT protecting against a reload racing a live create:
    /// reload only runs before this process serves anything.) The identity
    /// condition covers the other direction: a settlement computed for one
    /// attempt must not land on a reservation now pointing at another.
    #[tokio::test]
    async fn settling_a_reservation_is_monotonic_and_identity_conditioned() {
        let (_dir, store) = fresh_store().await;
        insert_reserved(&store, "s1", "key", "fp").await;

        // Wrong session id: refused, silently, leaving the row pending.
        store
            .settle_reservations(vec![settlement(
                "key",
                "some-other-session",
                ReservationOutcome::Created,
            )])
            .await
            .expect("a mismatched settlement is a no-op, not an error");
        assert_eq!(
            reservation_of(&store, "key").await.outcome,
            ReservationOutcome::Pending,
            "a settlement naming a different session must not land"
        );

        store
            .settle_reservations(vec![settlement("key", "s1", ReservationOutcome::Created)])
            .await
            .expect("first settlement");
        store
            .settle_reservations(vec![settlement(
                "key",
                "s1",
                ReservationOutcome::Failed {
                    kind: farhelm_proto::ErrorKind::Internal,
                    message: "a later, losing answer".to_string(),
                },
            )])
            .await
            .expect("second settlement must be accepted as a no-op, not an error");
        assert_eq!(
            reservation_of(&store, "key").await.outcome,
            ReservationOutcome::Created
        );

        // A key with no row at all is likewise a no-op: nothing can make a
        // reservation appear retroactively.
        store
            .settle_reservations(vec![settlement(
                "no-such-key",
                "s1",
                ReservationOutcome::Created,
            )])
            .await
            .expect("settling an unknown key is a no-op");
    }

    /// A settlement batch is all-or-nothing.
    ///
    /// Reload settles a whole startup's worth of reconciliation in one
    /// call, and a partially-applied batch would leave some intents
    /// recorded and others not, with nothing to say which — the next
    /// startup would have to redo an unknown subset. Forced here with a
    /// trigger that refuses one specific row, which is the only way to make
    /// a mid-batch failure happen on demand.
    #[tokio::test]
    async fn a_failed_settlement_rolls_the_whole_batch_back() {
        let (dir, store) = fresh_store().await;
        insert_reserved(&store, "s1", "first-key", "fp").await;
        insert_reserved(&store, "s2", "poisoned-key", "fp").await;
        {
            let conn = Connection::open(dir.path().join("supervisor.db")).expect("open raw");
            conn.execute_batch(
                "CREATE TRIGGER refuse_poisoned BEFORE UPDATE ON create_reservations \
                 WHEN NEW.intent_key = 'poisoned-key' \
                 BEGIN SELECT RAISE(ABORT, 'refused by test trigger'); END;",
            )
            .expect("plant the trigger");
        }

        store
            .settle_reservations(vec![
                settlement("first-key", "s1", ReservationOutcome::Created),
                settlement("poisoned-key", "s2", ReservationOutcome::Created),
            ])
            .await
            .expect_err("the poisoned row must fail the batch");

        assert_eq!(
            reservation_of(&store, "first-key").await.outcome,
            ReservationOutcome::Pending,
            "a settlement that committed while a later one failed would leave this reload's \
             reconciliation half-recorded"
        );
    }

    /// `pending_reservations` is the reload worklist, so it must return
    /// exactly the rows reload can still act on — and none of the settled
    /// ones, which would grow without bound over a state directory's life.
    #[tokio::test]
    async fn pending_reservations_lists_only_the_unsettled_ones() {
        let (_dir, store) = fresh_store().await;
        insert_reserved(&store, "s1", "still-pending", "fp").await;
        insert_reserved(&store, "s2", "already-created", "fp").await;
        store
            .settle_reservations(vec![settlement(
                "already-created",
                "s2",
                ReservationOutcome::Created,
            )])
            .await
            .expect("settle");

        let pending = store.pending_reservations().await.expect("list");
        assert_eq!(pending.len(), 1, "got {pending:?}");
        assert_eq!(pending[0].intent_key, "still-pending");
        assert_eq!(pending[0].session_id, "s1");
        assert_eq!(pending[0].outcome, ReservationOutcome::Pending);
    }

    /// Deleting a session TOMBSTONES its reservations rather than
    /// removing them, and settles a still-pending one as created.
    ///
    /// Both halves are the tombstone rule (PLAN_M3.md item 6). Keeping the
    /// row is what lets a later replay say "that session was deleted"
    /// instead of silently creating a second one; settling a pending row
    /// is what keeps a retry from concluding "the crashed attempt never
    /// launched" about a session that demonstrably existed — nothing can
    /// be deleted without having been created.
    #[tokio::test]
    async fn deleting_a_session_settles_and_keeps_its_reservations() {
        let (_dir, store) = fresh_store().await;
        insert_reserved(&store, "s1", "pending-key", "fp").await;
        insert_reserved(&store, "s2", "settled-key", "fp").await;
        store
            .settle_reservations(vec![settlement(
                "settled-key",
                "s2",
                ReservationOutcome::Created,
            )])
            .await
            .expect("settle");

        for id in ["s1", "s2"] {
            store
                .delete_session_settling_reservations(id)
                .await
                .expect("delete");
        }

        assert!(
            store.load_all().await.expect("load").is_empty(),
            "both session rows are gone"
        );
        for key in ["pending-key", "settled-key"] {
            assert_eq!(
                reservation_of(&store, key).await.outcome,
                ReservationOutcome::Created,
                "{key}: a deleted session's intent is settled as created, whatever it was before"
            );
        }
    }

    /// The delete's two writes are one transaction: a failure leaves the
    /// session AND its reservation exactly as they were.
    ///
    /// The direction that would hurt is a tombstone recorded over a session
    /// that then survived the delete — a live session whose intent key
    /// reports it as gone, permanently. Forced with a trigger that refuses
    /// the row removal, the only way to fail the second half on demand.
    #[tokio::test]
    async fn a_failed_delete_leaves_both_the_session_and_its_reservation() {
        let (dir, store) = fresh_store().await;
        insert_reserved(&store, "s1", "key", "fp").await;
        {
            let conn = Connection::open(dir.path().join("supervisor.db")).expect("open raw");
            conn.execute_batch(
                "CREATE TRIGGER refuse_delete BEFORE DELETE ON sessions \
                 BEGIN SELECT RAISE(ABORT, 'refused by test trigger'); END;",
            )
            .expect("plant the trigger");
        }

        store
            .delete_session_settling_reservations("s1")
            .await
            .expect_err("the refused row removal must fail the delete");

        assert_eq!(
            store.load_all().await.expect("load").len(),
            1,
            "the session survives its failed delete"
        );
        assert_eq!(
            reservation_of(&store, "key").await.outcome,
            ReservationOutcome::Pending,
            "and its intent key must not have been tombstoned over a session that still exists"
        );
    }

    /// The create path's ROLLBACK delete settles what the caller tells it
    /// to, in the same transaction — and settles nothing when told nothing.
    ///
    /// The `None` case is the one with teeth: a rollback for a launch that
    /// may still be reconcilable must leave its reservation pending, since
    /// recording `Created` there would claim a session that never existed
    /// and recording `Failed` would close an intent whose agent might be
    /// running.
    #[tokio::test]
    async fn the_rollback_delete_settles_only_what_it_is_given() {
        let (_dir, store) = fresh_store().await;
        insert_reserved(&store, "s1", "unsettled", "fp").await;
        insert_reserved(&store, "s2", "settled", "fp").await;

        store
            .delete_session("s1", None)
            .await
            .expect("rollback delete");
        store
            .delete_session(
                "s2",
                Some(settlement(
                    "settled",
                    "s2",
                    ReservationOutcome::Failed {
                        kind: farhelm_proto::ErrorKind::InvalidRequest,
                        message: "the spec never landed".to_string(),
                    },
                )),
            )
            .await
            .expect("rollback delete with settlement");

        assert_eq!(
            reservation_of(&store, "unsettled").await.outcome,
            ReservationOutcome::Pending
        );
        assert_eq!(
            reservation_of(&store, "settled").await.outcome,
            ReservationOutcome::Failed {
                kind: farhelm_proto::ErrorKind::InvalidRequest,
                message: "the spec never landed".to_string(),
            }
        );
        assert!(store.load_all().await.expect("load").is_empty());
    }

    /// A refused intent is recorded with no session row at all — the shape
    /// a create rejected by validation leaves behind, so its retry replays
    /// the refusal instead of re-deriving one from a filesystem that may
    /// have changed.
    #[tokio::test]
    async fn a_refused_intent_records_without_a_session_row() {
        let (_dir, store) = fresh_store().await;
        store
            .record_failed_intent(
                IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint: "fp".to_string(),
                },
                "s1",
                "fh-s1",
                farhelm_proto::ErrorKind::InvalidRequest,
                "working directory does not exist: /nope",
            )
            .await
            .expect("record");

        assert!(
            store.load_all().await.expect("load").is_empty(),
            "a validation refusal never had a session"
        );
        assert_eq!(
            reservation_of(&store, "key").await.outcome,
            ReservationOutcome::Failed {
                kind: farhelm_proto::ErrorKind::InvalidRequest,
                message: "working directory does not exist: /nope".to_string(),
            }
        );

        // A key someone else claimed in the meantime is left alone: a
        // refusal must never overwrite a live claim.
        insert_reserved(&store, "s2", "live", "fp").await;
        store
            .record_failed_intent(
                IntentClaim {
                    intent_key: "live".to_string(),
                    fingerprint: "fp".to_string(),
                },
                "s3",
                "fh-s3",
                farhelm_proto::ErrorKind::Internal,
                "this must not land",
            )
            .await
            .expect("record");
        let live = reservation_of(&store, "live").await;
        assert_eq!(live.outcome, ReservationOutcome::Pending);
        assert_eq!(live.session_id, "s2");
    }

    /// The relaunch takeover is a real conditional transition: it acquires
    /// only while BOTH the reservation is still pending and its row is
    /// still un-launched, and reports which condition failed otherwise.
    ///
    /// This is what keeps a racing delete from being resurrected. The
    /// caller decides to relaunch from evidence gathered a moment earlier;
    /// if a delete tombstoned the reservation in between, relaunching would
    /// recreate a session the user deliberately threw away, and if the
    /// launch landed late, relaunching would start a second agent beside
    /// the first.
    #[tokio::test]
    async fn the_relaunch_takeover_re_checks_both_of_its_conditions() {
        let (_dir, store) = fresh_store().await;

        // Acquired: pending reservation, launching row.
        insert_reserved(&store, "s1", "acquire", "fp").await;
        assert_eq!(
            store
                .restart_pending_launch(launching_row("s1"), "acquire")
                .await
                .expect("takeover"),
            RetryClaim::Acquired
        );
        assert_eq!(
            store.session("s1").await.expect("read").unwrap().outcome,
            LastOutcome::Launching,
            "the takeover leaves a fresh launching row under the same id"
        );

        // Resolved: a delete tombstoned the reservation first.
        insert_reserved(&store, "s2", "deleted", "fp").await;
        store
            .delete_session_settling_reservations("s2")
            .await
            .expect("the racing delete");
        let claim = store
            .restart_pending_launch(launching_row("s2"), "deleted")
            .await
            .expect("takeover");
        let RetryClaim::Resolved(settled) = claim else {
            panic!("a tombstoned reservation must refuse the takeover: {claim:?}");
        };
        assert_eq!(settled.outcome, ReservationOutcome::Created);
        assert!(
            store.session("s2").await.expect("read").is_none(),
            "and must not have resurrected the deleted session"
        );

        // Launched: the reserved row moved past launching in the meantime.
        insert_reserved(&store, "s3", "late", "fp").await;
        store
            .transition(
                "s3",
                0,
                Transition::ConfirmRunning {
                    pane: "%1".to_string(),
                },
            )
            .await
            .expect("the late-landing launch");
        assert_eq!(
            store
                .restart_pending_launch(launching_row("s3"), "late")
                .await
                .expect("takeover"),
            RetryClaim::Launched
        );
        assert_eq!(
            store.session("s3").await.expect("read").unwrap().pane,
            "%1",
            "the late launch's own record must survive the refused takeover"
        );

        // Acquired even from `interrupted`, when no pane was ever
        // recorded: the reboot conversion blankets never-launched rows
        // too, so that status is not evidence of anything and the takeover
        // must not read it as a launch. An interrupted row that DOES carry
        // a pane is the opposite case and refuses.
        insert_reserved(&store, "s4", "rebooted", "fp").await;
        force_outcome(&store, "s4", &LastOutcome::Interrupted);
        assert_eq!(
            store
                .restart_pending_launch(launching_row("s4"), "rebooted")
                .await
                .expect("takeover"),
            RetryClaim::Acquired
        );
        insert_reserved(&store, "s5", "rebooted-live", "fp").await;
        {
            let conn = store.conn.lock().expect("db mutex");
            conn.execute("UPDATE sessions SET pane = '%7' WHERE id = 's5'", [])
                .expect("record a pane");
        }
        force_outcome(&store, "s5", &LastOutcome::Interrupted);
        assert_eq!(
            store
                .restart_pending_launch(launching_row("s5"), "rebooted-live")
                .await
                .expect("takeover"),
            RetryClaim::Launched,
            "an interrupted row WITH a pane was seen in tmux and must not be relaunched over"
        );
    }

    /// A reservation row this build cannot honestly decode is refused, not
    /// guessed at — same stance as `load_all`'s, and for a sharper reason:
    /// a guessed outcome here either replays a success that never happened
    /// or launches a duplicate.
    ///
    /// The matrix covers every way a `failed` row can be incomplete,
    /// because each has its own tempting default (`Internal`, an empty
    /// message) and every one of them would be a fabrication.
    #[tokio::test]
    async fn a_corrupt_reservation_row_is_refused_rather_than_repaired() {
        let cases = [
            (
                "UPDATE create_reservations SET state = 'half-done'",
                "half-done",
            ),
            (
                "UPDATE create_reservations SET state = 'failed', error_kind = NULL, \
                 error_detail = 'x'",
                "no error kind",
            ),
            (
                "UPDATE create_reservations SET state = 'failed', error_kind = 'teapot', \
                 error_detail = 'x'",
                "teapot",
            ),
            (
                "UPDATE create_reservations SET state = 'failed', error_kind = 'internal', \
                 error_detail = NULL",
                "no error text",
            ),
        ];
        for (corruption, expected) in cases {
            let (dir, store) = fresh_store().await;
            insert_reserved(&store, "s1", "key", "fp").await;
            {
                let conn = Connection::open(dir.path().join("supervisor.db")).expect("open raw");
                conn.execute(corruption, []).expect("plant corruption");
            }

            let error = store
                .reservation("key")
                .await
                .expect_err("a row outside the vocabulary must not be guessed at");
            let rendered = format!("{error:#}");
            assert!(
                rendered.contains(expected) && rendered.contains("key"),
                "the refusal must name the problem and the row: {rendered}"
            );
        }
    }

    /// The integration snapshot has to make the round trip through SQLite
    /// intact, and the interesting half is the template: it is an argv
    /// VECTOR stored in one column precisely so a path with spaces (or a
    /// quote, or a JSON metacharacter) comes back as ONE element. A
    /// delimiter-joined encoding would pass a naive test and silently
    /// fragment exactly the invocation PLAN_M3.md item 7 calls out.
    #[tokio::test]
    async fn an_integration_snapshot_survives_the_round_trip_element_by_element() {
        let (_dir, store) = fresh_store().await;
        let template = vec![
            "/opt/my agents/claude".to_string(),
            "--resume".to_string(),
            "{conversation}".to_string(),
            r#"weird "quoted", value"#.to_string(),
        ];
        store
            .insert_session(
                StoredSession {
                    id: "s1".to_string(),
                    title: "t".to_string(),
                    cwd: "/tmp/work".to_string(),
                    invocation: "agent".to_string(),
                    tmux_name: "fh-s1".to_string(),
                    pane: String::new(),
                    outcome: LastOutcome::Launching,
                    agent_kind: farhelm_proto::AgentKind::Claude,
                    resume_template: Some(template.clone()),
                    canonical_cwd: Some("/tmp/work".to_string()),
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: None,
                    generation: 0,
                    launch_scoped: false,
                },
                None,
            )
            .await
            .expect("insert");
        let row = store.session("s1").await.expect("read").expect("present");
        assert_eq!(row.agent_kind, farhelm_proto::AgentKind::Claude);
        assert_eq!(row.resume_template.as_deref(), Some(template.as_slice()));
        // And through the bulk loader, which uses the same decoder but a
        // different statement — the one a supervisor restart runs.
        let loaded = store.load_all().await.expect("load");
        assert_eq!(
            loaded[0].resume_template.as_deref(),
            Some(template.as_slice())
        );
    }

    /// Both mutable capture columns are WRITE-ONCE, and each protects a
    /// different failure.
    ///
    /// Moving `first_input_at` forward would slide the capture window past
    /// the very record it exists to match, so a later observation must
    /// lose to the first. Overwriting `captured_conversation` would let an
    /// explicit fork — which writes a NEW id — silently move a session onto
    /// a conversation it never ran, which is the wrong-conversation resume
    /// SPEC.md forbids. Both are enforced by SQL predicate rather than by
    /// caller discipline, so this asserts the predicate.
    #[tokio::test]
    async fn the_capture_columns_are_write_once_and_report_what_is_committed() {
        let (_dir, store) = fresh_store().await;
        insert_running(&store, "s1").await;

        store
            .record_first_input("s1", 0, 1_000)
            .await
            .expect("first");
        store
            .record_first_input("s1", 0, 2_000)
            .await
            .expect("second");
        assert_eq!(
            store
                .session("s1")
                .await
                .expect("read")
                .unwrap()
                .first_input_at,
            Some(1_000),
            "the FIRST input is the correlator; a later one must not move it"
        );

        assert_eq!(
            store
                .record_captured_conversation("s1", 0, "conv-a", Path::new("/records/a.jsonl"))
                .await
                .expect("capture"),
            Some("conv-a".to_string())
        );
        assert_eq!(
            store
                .record_captured_conversation("s1", 0, "conv-b", Path::new("/records/b.jsonl"))
                .await
                .expect("second capture"),
            Some("conv-a".to_string()),
            "the committed value is reported, not the one this caller intended"
        );
        assert_eq!(
            store
                .session("s1")
                .await
                .expect("read")
                .unwrap()
                .captured_conversation,
            Some("conv-a".to_string())
        );

        // A deleted session is not an error for either writer: a capture
        // pass racing a delete is ordinary, and there is nothing to repair.
        store.delete_session("s1", None).await.expect("delete");
        assert_eq!(
            store
                .record_captured_conversation("s1", 0, "conv-c", Path::new("/records/c.jsonl"))
                .await
                .expect("a vanished row is not a failure"),
            None
        );
        store
            .record_first_input("s1", 0, 3_000)
            .await
            .expect("a vanished row is not a failure");
    }

    /// The title is the one metadata column a caller may overwrite, and
    /// the write reports whether there was a row to overwrite.
    ///
    /// Both halves are contract. Unconditional overwriting is what makes
    /// concurrent renames last-write-wins (PLAN_M5.md item 3) instead of
    /// silently write-once like the capture columns beside it — an easy
    /// thing to "fix" by pattern-matching the neighbouring predicates. And
    /// the `false` answer is the only way the handler can tell a rename
    /// that raced a delete from one that worked, which no integration test
    /// can reach: the in-memory entry is removed by the same delete, so
    /// the request never gets this far. Without it a caller could be told
    /// its rename succeeded against a session that no longer exists.
    #[tokio::test]
    async fn a_title_write_overwrites_and_reports_a_missing_row() {
        let (_dir, store) = fresh_store().await;
        insert_running(&store, "s1").await;

        assert!(
            store
                .set_session_title("s1", "first")
                .await
                .expect("rename"),
            "renaming a session that exists must report the row it updated"
        );
        assert!(
            store
                .set_session_title("s1", "second")
                .await
                .expect("rename again"),
            "a second rename must not be refused the way the write-once columns are"
        );
        assert_eq!(
            store.session("s1").await.expect("read").unwrap().title,
            "second",
            "the later write wins"
        );

        store.delete_session("s1", None).await.expect("delete");
        assert!(
            !store
                .set_session_title("s1", "too late")
                .await
                .expect("a vanished row is not a failure"),
            "renaming a deleted session must report that nothing was updated"
        );
    }

    /// A database written before item 7 has no snapshot at all, and the
    /// migration must NOT invent one: re-deriving a kind from the stored
    /// invocation is exactly the later re-guessing item 7 forbids, and a
    /// pre-M3 session additionally has no first-input time, so capture
    /// could never run for it even if it looked integrated. `generic` with
    /// no template is the honest reading — restart can only offer it a
    /// fresh launch.
    #[tokio::test]
    async fn migrating_a_pre_snapshot_database_claims_no_integration() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("supervisor.db");
        {
            let conn = Connection::open(&path).expect("create raw");
            conn.execute_batch(V1_SCHEMA).expect("v1 schema");
            conn.execute(
                "INSERT INTO sessions (id, title, cwd, invocation, tmux_name, pane, created_at) \
                 VALUES ('old', 't', '/tmp', 'claude --resume-me', 'fh-old', '%0', 0)",
                [],
            )
            .expect("seed a pre-M3 row");
            conn.pragma_update(None, "user_version", 1).expect("stamp");
        }
        let store = SessionStore::open(&path, true).await.expect("migrate");
        let row = store.session("old").await.expect("read").expect("present");
        assert_eq!(
            row.agent_kind,
            farhelm_proto::AgentKind::Generic,
            "a session that predates the snapshot must not acquire an integration"
        );
        assert_eq!(row.resume_template, None);
        assert_eq!(row.captured_conversation, None);
        assert_eq!(row.first_input_at, None);
    }

    /// The database is a trust boundary: a row is whatever the last writer
    /// left plus whatever a crash, a downgrade, or a hand-edit did to it.
    /// Every shape that would produce a session this build cannot honor is
    /// refused at LOAD rather than discovered later — most sharply the
    /// integrated-kind-without-placeholder row, which describes a session
    /// that could capture an identity and then be unable to resume with
    /// it: SPEC.md's exact-conversation promise silently false, discovered
    /// only at the restart that needed it.
    ///
    /// A matrix rather than one case, because these are independent
    /// decoders and a fix to one says nothing about the others.
    #[tokio::test]
    async fn a_semantically_impossible_session_row_is_refused_at_load() {
        let cases = [
            (
                "UPDATE sessions SET agent_kind = 'claude', resume_template = \
                 '[\"claude\",\"--continue\"]'",
                "{conversation}",
            ),
            (
                "UPDATE sessions SET agent_kind = 'claude', resume_template = NULL",
                "{conversation}",
            ),
            (
                "UPDATE sessions SET resume_template = 'not json at all'",
                "resume template",
            ),
            (
                "UPDATE sessions SET resume_template = '{\"not\":\"an array\"}'",
                "resume template",
            ),
            (
                "UPDATE sessions SET resume_template = '[1,2,3]'",
                "resume template",
            ),
        ];
        for (corruption, expected) in cases {
            let (dir, store) = fresh_store().await;
            insert_running(&store, "s1").await;
            {
                let conn = Connection::open(dir.path().join("supervisor.db")).expect("open raw");
                conn.execute(corruption, []).expect("plant corruption");
            }
            let rendered = format!(
                "{:#}",
                store
                    .session("s1")
                    .await
                    .expect_err("an unusable row must not be handed out")
            );
            assert!(
                rendered.contains(expected) && rendered.contains("s1"),
                "the refusal must name the problem and the row: {rendered}"
            );
            // The bulk loader shares the decoder, but through a different
            // statement — the one a supervisor restart runs, where a
            // silently-dropped refusal would be worst.
            assert!(store.load_all().await.is_err());
        }
    }

    /// An agent-kind column outside this build's vocabulary is refused
    /// rather than downgraded to `generic`, for the same no-guessing
    /// reason every other decoder here refuses: quietly dropping the
    /// integration would ALSO orphan whatever conversation identity sits
    /// in the very next column, turning a resumable session into an
    /// unresumable one with no error anywhere.
    #[tokio::test]
    async fn a_corrupt_agent_kind_is_refused_rather_than_defaulted() {
        let (dir, store) = fresh_store().await;
        insert_running(&store, "s1").await;
        {
            let conn = Connection::open(dir.path().join("supervisor.db")).expect("open raw");
            conn.execute("UPDATE sessions SET agent_kind = 'gemini'", [])
                .expect("plant corruption");
        }
        let rendered = format!(
            "{:#}",
            store
                .session("s1")
                .await
                .expect_err("an unrecognized kind must not be guessed at")
        );
        assert!(
            rendered.contains("gemini") && rendered.contains("s1"),
            "the refusal must name the problem and the row: {rendered}"
        );
    }

    /// A basis that matches a session with nothing captured — what
    /// `begin_relaunch`'s offer condition is checked against for the
    /// ordinary fresh relaunch.
    fn uncaptured_basis() -> OfferBasis {
        OfferBasis {
            captured_conversation: None,
            capture_ambiguous: false,
        }
    }

    fn claimed(decision: RelaunchDecision) -> RelaunchClaim {
        match decision {
            RelaunchDecision::Claimed(claim) => claim,
            other => panic!("expected a claimed generation, got {other:?}"),
        }
    }

    /// PLAN_M3.md items 4 and 9: a restart's new generation reopens a
    /// terminal outcome — the one write in this module allowed to — and
    /// takes the previous run's whole description with it, while handing
    /// that description back so a failed relaunch can put it right back.
    ///
    /// The stop annotation is the clause that matters most: SPEC.md makes
    /// it durable session metadata, and item 4 says a SUCCESSFUL restart is
    /// what clears it. Clearing it here and RETURNING it is what makes
    /// "only a successful one" enforceable rather than merely intended (see
    /// `abort_relaunch`'s own test below).
    ///
    /// The pane is emptied for a reason a later reload depends on: a crash
    /// between this commit and the launch's confirmation must leave the
    /// exact shape reload already reconciles (a `launching` row with no
    /// pane), not one claiming a pane the new run may never get.
    #[tokio::test]
    async fn a_relaunch_generation_reopens_a_stopped_session_and_clears_its_annotation() {
        let (_dir, store) = fresh_store().await;
        insert_running(&store, "s1").await;
        store
            .transition("s1", 0, Transition::StopRequested)
            .await
            .expect("stop intent");
        store
            .transition("s1", 0, Transition::StopCompleted { exit_code: Some(0) })
            .await
            .expect("stop outcome");
        let stopped = store.session("s1").await.expect("read").expect("present");
        let annotated = LastOutcome::Exited {
            exit_code: Some(0),
            annotation: Some(farhelm_proto::STOP_ANNOTATION.to_string()),
        };
        assert_eq!(stopped.outcome, annotated);

        let claim = claimed(
            store
                .begin_relaunch("s1", uncaptured_basis(), true, false)
                .await
                .expect("begin relaunch"),
        );
        assert_eq!(claim.generation, 1, "generations count launches from zero");
        assert_eq!(
            claim.prior.outcome, annotated,
            "the prior run is handed back verbatim, which is what makes it restorable"
        );
        assert_eq!(claim.prior.pane, stopped.pane);

        let relaunching = store.session("s1").await.expect("read").expect("present");
        assert_eq!(relaunching.outcome, LastOutcome::Launching);
        assert_eq!(relaunching.pane, "");
        assert_eq!(relaunching.generation, 1);
        // Everything describing the SESSION rather than the run is
        // untouched — a relaunched session is still the same session.
        assert_eq!(relaunching.tmux_name, stopped.tmux_name);
        assert_eq!(relaunching.invocation, stopped.invocation);
        assert_eq!(relaunching.agent_kind, stopped.agent_kind);
    }

    /// Item 4's other half, and the contract this PR itself documented: a
    /// relaunch that fails before touching anything external puts the
    /// previous run's outcome back, annotation included. Without the
    /// restore, opening the generation up front — which the crash-ordering
    /// rule requires — would silently destroy the very metadata SPEC.md
    /// calls durable.
    #[tokio::test]
    async fn aborting_a_relaunch_restores_the_outcome_it_replaced() {
        let (_dir, store) = fresh_store().await;
        insert_running(&store, "s1").await;
        store
            .transition("s1", 0, Transition::StopRequested)
            .await
            .expect("stop intent");
        store
            .transition("s1", 0, Transition::StopCompleted { exit_code: Some(3) })
            .await
            .expect("stop outcome");

        let claim = claimed(
            store
                .begin_relaunch("s1", uncaptured_basis(), true, false)
                .await
                .expect("begin relaunch"),
        );
        assert!(
            store
                .abort_relaunch("s1", claim.generation, &claim.prior)
                .await
                .expect("abort"),
        );
        let restored = store.session("s1").await.expect("read").expect("present");
        assert_eq!(restored.outcome, claim.prior.outcome);
        assert_eq!(restored.pane, claim.prior.pane);
        assert_eq!(
            restored.generation, claim.generation,
            "the generation is monotonic and never rolled back: a reused number is exactly \
             what would let stale evidence land on a future launch"
        );
    }

    /// An abort must never step on a LATER restart that has already claimed
    /// the session: by then the newer generation's outcome is the truth,
    /// and this one's prior run is ancient history.
    #[tokio::test]
    async fn aborting_a_superseded_relaunch_changes_nothing() {
        let (_dir, store) = fresh_store().await;
        insert_running(&store, "s1").await;
        let first = claimed(
            store
                .begin_relaunch("s1", uncaptured_basis(), true, false)
                .await
                .expect("first"),
        );
        let second = claimed(
            store
                .begin_relaunch("s1", uncaptured_basis(), true, false)
                .await
                .expect("second"),
        );
        assert_eq!(second.generation, first.generation + 1);

        assert!(
            !store
                .abort_relaunch("s1", first.generation, &first.prior)
                .await
                .expect("abort"),
            "the older restart no longer owns this session"
        );
        let row = store.session("s1").await.expect("read").expect("present");
        assert_eq!(row.outcome, LastOutcome::Launching);
        assert_eq!(row.generation, second.generation);
    }

    /// PLAN_M3.md item 10's `launch_scoped` column, on the side no other
    /// test in this module reaches: every other fixture here inserts with
    /// `launch_scoped: false` (systemd-scope availability is host-
    /// dependent, and this module's tests must pass with no user manager
    /// present), so the `true` side of the column has never round-tripped
    /// through an insert-and-read-back at the store level. Pins that a row
    /// created scoped actually reports scoped, both from the immediate
    /// `session` lookup and from `load_all`'s independent read path.
    /// Neither read checks whether any unit currently exists — the column
    /// records which mechanism the launch SELECTED, and preserving that
    /// recorded selection is what lets a later stop, delete, or restart
    /// reap through the scope the launch actually ran under.
    #[tokio::test]
    async fn launch_scoped_true_round_trips_through_insert_and_reload() {
        let (_dir, store) = fresh_store().await;
        insert_running_with_scope(&store, "s1", true).await;

        let read_back = store.session("s1").await.expect("read").expect("present");
        assert!(
            read_back.launch_scoped,
            "a session inserted with launch_scoped = true must report true immediately"
        );

        let rows = store.load_all().await.expect("load");
        assert!(
            rows.iter()
                .find(|r| r.id == "s1")
                .expect("present")
                .launch_scoped,
            "and must still report true through load_all's independent read path"
        );
    }

    /// The re-decide half of PLAN_M3.md item 10, in both directions: a
    /// relaunch's `scope_available` argument is the CURRENT probe result,
    /// not a carry-over of the previous launch's own selection, because a
    /// host can gain or lose its user manager between two launches of the
    /// same session (see `begin_relaunch`'s docs).
    ///
    /// The scoped-to-unscoped direction is the one that matters most and
    /// was, before this test, exercised by nothing host-independent: every
    /// other relaunch test in this module passes `scope_available: false`
    /// throughout, so a relaunch dropping a TRUE prior claim had only ever
    /// been reachable through the systemd-gated e2e suite, which skips
    /// wherever no user manager exists (including CI).
    #[tokio::test]
    async fn a_relaunch_re_decides_launch_scoped_from_the_current_probe() {
        let (_dir, store) = fresh_store().await;

        // One case per direction the host can drift between two launches:
        // it lost its user manager (scoped -> unscoped), or gained one
        // (unscoped -> scoped).
        for (id, prior_scoped, probe_scoped) in
            [("was-scoped", true, false), ("was-unscoped", false, true)]
        {
            insert_running_with_scope(&store, id, prior_scoped).await;
            let claim = claimed(
                store
                    .begin_relaunch(id, uncaptured_basis(), true, probe_scoped)
                    .await
                    .expect("begin relaunch"),
            );
            assert_eq!(
                claim.prior.scoped, prior_scoped,
                "{id}: the prior run's own record must keep its original selection"
            );
            assert_eq!(
                claim.scoped, probe_scoped,
                "{id}: the new generation must take the current probe's answer, not inherit \
                 the prior claim"
            );
            let relaunched = store.session(id).await.expect("read").expect("present");
            assert_eq!(
                relaunched.launch_scoped, probe_scoped,
                "{id}: the stored row must reflect the re-decided selection"
            );
        }
    }

    /// `abort_relaunch` restores everything item 4 promises the prior run
    /// gets back — [`PriorRun::scoped`]'s own docs call the scope
    /// selection out by name, alongside the pane and the outcome, as
    /// something a failed restart must not leave describing an abandoned
    /// generation's claim. Every OTHER abort test in this module begins
    /// and ends at `launch_scoped = false`, so the restore had never been
    /// asserted for a `true` prior value, nor in either direction of a
    /// flip.
    #[tokio::test]
    async fn aborting_a_relaunch_restores_the_prior_scope_selection() {
        let (_dir, store) = fresh_store().await;

        // Both flip directions: an aborted relaunch must hand back exactly
        // the selection the prior run recorded, whichever way the attempt
        // had re-decided it.
        for (id, prior_scoped, attempted_scoped) in
            [("was-scoped", true, false), ("was-unscoped", false, true)]
        {
            insert_running_with_scope(&store, id, prior_scoped).await;
            let claim = claimed(
                store
                    .begin_relaunch(id, uncaptured_basis(), true, attempted_scoped)
                    .await
                    .expect("begin relaunch"),
            );
            assert_eq!(
                claim.scoped, attempted_scoped,
                "{id}: sanity — the relaunch must have flipped the selection before the abort"
            );
            assert!(
                store
                    .abort_relaunch(id, claim.generation, &claim.prior)
                    .await
                    .expect("abort"),
            );
            let restored = store.session(id).await.expect("read").expect("present");
            assert_eq!(
                restored.launch_scoped, prior_scoped,
                "{id}: abort must restore the pre-relaunch scope selection"
            );
        }
    }

    /// The generation fence, from the side that matters most: an observer
    /// holding a pre-restart view of a session must not be able to record
    /// the OLD run's exit against the new one. That is not hypothetical —
    /// it is precisely what a `ListSessions` pass does when it clones an
    /// entry, goes to tmux, and comes back after a restart replaced the
    /// pane it was asking about.
    #[tokio::test]
    async fn a_stale_generations_observation_is_dropped() {
        let (_dir, store) = fresh_store().await;
        insert_running(&store, "s1").await;
        let claim = claimed(
            store
                .begin_relaunch("s1", uncaptured_basis(), true, false)
                .await
                .expect("begin relaunch"),
        );
        store
            .transition(
                "s1",
                claim.generation,
                Transition::ConfirmRunning {
                    pane: "%7".to_string(),
                },
            )
            .await
            .expect("confirm the new launch");

        let committed = store
            .transition("s1", 0, Transition::ObservedExit { exit_code: Some(9) })
            .await
            .expect("stale observation")
            .expect("the row exists");
        assert_eq!(
            committed,
            LastOutcome::Running,
            "the stale observation is dropped, and the caller is told what is actually true"
        );
        let row = store.session("s1").await.expect("read").expect("present");
        assert_eq!(row.outcome, LastOutcome::Running);
        assert_eq!(row.pane, "%7");
    }

    /// Capture writes carry the same fence, for the sharper version of the
    /// same risk: an in-flight correlation pass committing the PREVIOUS
    /// run's conversation identity onto a fresh relaunch is how a session
    /// would come to offer a resume for a conversation the new run never
    /// had — the silently-wrong-conversation resume SPEC.md forbids.
    #[tokio::test]
    async fn a_stale_generations_capture_is_dropped() {
        let (_dir, store) = fresh_store().await;
        insert_running(&store, "s1").await;
        let claim = claimed(
            store
                .begin_relaunch("s1", uncaptured_basis(), true, false)
                .await
                .expect("begin relaunch"),
        );

        assert_eq!(
            store
                .record_captured_conversation("s1", 0, "conv-old", Path::new("/records/old"))
                .await
                .expect("stale capture"),
            None,
            "nothing was claimed, and the read-back says so"
        );
        store
            .record_first_input("s1", 0, 1_000)
            .await
            .expect("stale first input");
        store
            .record_capture_ambiguous("s1", 0)
            .await
            .expect("stale ambiguity");
        let row = store.session("s1").await.expect("read").expect("present");
        assert_eq!(row.captured_conversation, None);
        assert_eq!(row.first_input_at, None);
        assert!(!row.capture_ambiguous);

        // The CURRENT generation's own capture still works.
        assert_eq!(
            store
                .record_captured_conversation(
                    "s1",
                    claim.generation,
                    "conv-new",
                    Path::new("/records/new")
                )
                .await
                .expect("current capture")
                .as_deref(),
            Some("conv-new")
        );
    }

    /// A relaunch that is not resuming a captured identity opens a FRESH
    /// capture window: the first-input anchor and the correlation verdict
    /// are per-LAUNCH state, and carrying them forward would either point
    /// the correlator at a window that closed long ago or deny the new run
    /// any capture at all.
    #[tokio::test]
    async fn a_fresh_relaunch_clears_the_previous_runs_capture_state() {
        let (_dir, store) = fresh_store().await;
        insert_running(&store, "s1").await;
        store
            .record_first_input("s1", 0, 1_000)
            .await
            .expect("first input");
        store
            .record_capture_ambiguous("s1", 0)
            .await
            .expect("ambiguity");

        let claim = claimed(
            store
                .begin_relaunch(
                    "s1",
                    OfferBasis {
                        captured_conversation: None,
                        capture_ambiguous: true,
                    },
                    true,
                    false,
                )
                .await
                .expect("begin relaunch"),
        );
        let row = store.session("s1").await.expect("read").expect("present");
        assert_eq!(row.first_input_at, None);
        assert!(!row.capture_ambiguous);
        assert_eq!(row.captured_conversation, None);

        // And the new window can be captured into, which an inherited
        // ambiguity would have denied forever.
        assert_eq!(
            store
                .record_captured_conversation(
                    "s1",
                    claim.generation,
                    "conv-1",
                    Path::new("/records/1")
                )
                .await
                .expect("capture")
                .as_deref(),
            Some("conv-1")
        );
    }

    /// A `Resume` relaunch keeps every scrap of capture state, because the
    /// identity it is resuming is exactly what the capture pass must go on
    /// reverifying across the restart.
    #[tokio::test]
    async fn a_resuming_relaunch_keeps_the_captured_identity() {
        let (_dir, store) = fresh_store().await;
        insert_running(&store, "s1").await;
        store
            .record_first_input("s1", 0, 1_000)
            .await
            .expect("first input");
        store
            .record_captured_conversation("s1", 0, "conv-1", Path::new("/records/1"))
            .await
            .expect("capture");

        claimed(
            store
                .begin_relaunch(
                    "s1",
                    OfferBasis {
                        captured_conversation: Some("conv-1".to_string()),
                        capture_ambiguous: false,
                    },
                    false,
                    false,
                )
                .await
                .expect("begin relaunch"),
        );
        let row = store.session("s1").await.expect("read").expect("present");
        assert_eq!(row.captured_conversation.as_deref(), Some("conv-1"));
        assert_eq!(row.first_input_at, Some(1_000));
    }

    /// The offer condition, which is what makes "validate the mode, then
    /// relaunch" atomic rather than merely sequential: a capture that
    /// commits between the two turns the claim into a refusal instead of
    /// launching the fresh agent the user chose against a session that has
    /// meanwhile become resumable.
    #[tokio::test]
    async fn a_capture_landing_mid_restart_refuses_the_generation() {
        let (_dir, store) = fresh_store().await;
        insert_running(&store, "s1").await;
        // What the restart handler validated against.
        let basis = uncaptured_basis();
        // ...and what capture committed in between.
        store
            .record_captured_conversation("s1", 0, "conv-1", Path::new("/records/1"))
            .await
            .expect("capture");

        assert_eq!(
            store
                .begin_relaunch("s1", basis, true, false)
                .await
                .expect("begin relaunch"),
            RelaunchDecision::OfferChanged
        );
        let row = store.session("s1").await.expect("read").expect("present");
        assert_eq!(
            row.generation, 0,
            "a refused claim writes nothing at all, generation included"
        );
        assert_eq!(row.outcome, LastOutcome::Running);
    }

    /// An `Error` row — a launch that never execed — reopens too, and its
    /// detail goes with the generation that replaced it. Without this, a
    /// session whose first launch was a typo would keep reporting that
    /// typo's errno forever, even after a restart fixed it (M3 acceptance
    /// 4: "after a successful restart the error is gone").
    #[tokio::test]
    async fn a_relaunch_generation_clears_a_previous_launch_error() {
        let (_dir, store) = fresh_store().await;
        insert_running(&store, "s1").await;
        force_outcome(
            &store,
            "s1",
            &LastOutcome::Error {
                detail: "exec_failed argv0=/nope errno=2".to_string(),
            },
        );

        claimed(
            store
                .begin_relaunch("s1", uncaptured_basis(), true, false)
                .await
                .expect("begin relaunch"),
        );
        let row = store.session("s1").await.expect("read").expect("present");
        assert_eq!(row.outcome, LastOutcome::Launching);
    }

    /// A restart racing a delete must not resurrect the session: with no
    /// row left, the generation cannot be opened at all, and the caller is
    /// expected to abandon the relaunch rather than recreate what the user
    /// threw away.
    #[tokio::test]
    async fn a_relaunch_generation_refuses_a_deleted_session() {
        let (_dir, store) = fresh_store().await;
        assert_eq!(
            store
                .begin_relaunch("gone", uncaptured_basis(), true, false)
                .await
                .expect("begin relaunch"),
            RelaunchDecision::Gone
        );
        assert!(store.session("gone").await.expect("read").is_none());
    }
}

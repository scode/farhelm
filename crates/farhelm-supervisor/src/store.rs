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
//! The profiles catalog (PLAN_M6_75.md item 4) is the one table here whose
//! rows are not about a session at all: it holds the named, user-editable
//! agent definitions SPEC.md's "a fresh supervisor is not empty" promises,
//! and it is the ONLY copy of the truth about which profiles currently
//! exist. Sessions never join against it to learn what they were launched
//! with — they carry their own immutable snapshot — and the two
//! `source_profile_*` columns beside that snapshot record only the
//! profile's IDENTITY as it was chosen ([`ProfileSnapshot`]). Whether that
//! profile still exists, and under which name, is derived at reply-build
//! time from this catalog and never stored (see `service::status`'s
//! derivation and `farhelm_proto::SourceProfile` for the contract).
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
use base64::Engine as _;
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use subtle::ConstantTimeEq;

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
const SCHEMA_VERSION: i64 = 12;

/// Random payload size behind one URL-safe session bearer.
const SESSION_TOKEN_BYTES: usize = 32;

/// Mint the recoverable bearer value injected into one session.
///
/// The token has the session row's lifetime. It is intentionally plaintext:
/// a later launch of the same session must be able to inject the same value,
/// so a one-way digest could not satisfy the restart contract.
fn mint_session_token() -> anyhow::Result<String> {
    let mut bytes = [0u8; SESSION_TOKEN_BYTES];
    getrandom::fill(&mut bytes)
        .map_err(|error| anyhow::anyhow!("reading operating-system randomness: {error}"))?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}

/// The sole row id of [`supervisor_meta`](apply_schema) — the table is
/// single-row by construction (a `CHECK` on this value), so every read and
/// write names it explicitly rather than scanning.
const META_ROW_ID: i64 = 0;

/// The profiles catalog's DDL, shared verbatim by the fresh-database path
/// and by the version-7-to-8 migration so the two cannot drift.
///
/// `agent_kind` carries [`agent_kind_column`]'s stable vocabulary, the same
/// spelling `sessions.agent_kind` uses — one vocabulary, so a profile and
/// the session it creates can never disagree about what "claude" means.
/// `resume_template` is [`resume_template_column`]'s JSON array, NULL for
/// the two meanings `farhelm_proto::Profile::resume_template` gives it (an
/// integrated kind deriving its own default, or a generic profile with no
/// resume at all).
///
/// No `NOT NULL` on `resume_template` and no uniqueness on `name`: names
/// are deliberately non-unique (the wire docs' reasoning — `id` is what
/// anything references, and refusing a duplicate name turns a cosmetic
/// collision into a dead end).
const PROFILES_SCHEMA: &str = "CREATE TABLE profiles (
                 id              TEXT PRIMARY KEY,
                 name            TEXT NOT NULL,
                 invocation      TEXT NOT NULL,
                 agent_kind      TEXT NOT NULL,
                 resume_template TEXT
             ) STRICT;";

/// The starter catalog SPEC.md promises every supervisor ships with —
/// Claude Code and Codex — inserted ONCE, in the same transaction that
/// creates the table (PLAN_M6_75.md item 4).
///
/// ## Why seeding rides the schema ladder rather than a seeded-flag
///
/// The requirement is that a starter a user DELETES stays deleted forever,
/// across every later restart. That is a statement about seeding happening
/// exactly once per database, and the ladder gives it for free: a migration
/// step runs on the transition between two `user_version` values and can
/// never run again, so "already seeded" is not a fact anything has to
/// record, consult, or keep in step — it is implied by the version stamp
/// the seeding transaction itself committed. A `supervisor_meta.profiles_
/// seeded` flag would instead introduce a SECOND durable fact that can
/// disagree with the table it describes (a flag lost to a partial write, a
/// startup path that reads it wrong, a future migration that rebuilds the
/// table), and every one of those disagreements re-seeds profiles the user
/// deliberately removed — the exact failure this must not have. It would
/// also have to be consulted at every startup, where the ladder is
/// consulted only when the version actually moves.
///
/// The starters are ordinary rows with no marking of any kind: nothing
/// downstream may treat them as special, because SPEC.md makes them
/// editable and deletable like any other profile.
///
/// ## Why the ids are fixed strings rather than minted UUIDs
///
/// A profile id is opaque and only ever meaningful to the supervisor that
/// minted it (`farhelm_proto::Profile::id`), so there is nothing to be
/// gained from making a starter's id unpredictable — and a fixed id makes a
/// freshly created database and a migrated one byte-identical in this
/// table, which is what lets `migrated_and_fresh_schemas_agree` compare the
/// seeded DATA and not merely the columns. Collision with a later
/// user-created profile is impossible: creates mint UUIDs, which these are
/// deliberately not shaped like.
///
/// ## Why the resume templates are NULL
///
/// NULL is `Profile::resume_template`'s "let the kind supply its default"
/// (see that field's docs), which is precisely what a starter for an
/// integrated kind wants. Writing the derived argv out here instead would
/// fork each integration's default template into a second copy that a SQL
/// literal can never keep in step — and would freeze it against the
/// invocation, so a user editing `claude` to `/opt/bin/claude` would keep
/// resuming through the old path. Deriving at create time
/// (`IntegrationSnapshot::resolve`) follows the edit.
const STARTER_PROFILES: &str = "INSERT INTO profiles \
                 (id, name, invocation, agent_kind, resume_template) VALUES \
                 ('starter-claude', 'Claude Code', 'claude', 'claude', NULL), \
                 ('starter-codex', 'Codex', 'codex', 'codex', NULL);";

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
/// `Permanent` rows are TOMBSTONES: they outlive the session they created (see
/// [`SessionStore::delete_session_settling_reservations`]), because the
/// question a replay asks — "did this intent already happen?" — still has
/// an answer after the session is gone, and the honest answer is "yes, and
/// it was deleted", never a fresh duplicate.
///
/// PLAN_M7.md adds `SessionLifetime` for spawn. Those rows are pruned in the
/// same transaction that deletes their child, so a retry key becomes usable
/// again exactly when the session it identified is gone.
///
/// Each row holds a
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
    /// How long this row deduplicates its key. Restricted spawn creates use
    /// `SessionLifetime`; full-authority interactive creates use `Permanent`.
    pub dedup_scope: DedupScope,
    pub outcome: ReservationOutcome,
}

/// The lifetime policy attached to one create reservation (PLAN_M7.md
/// item 2).
///
/// This is derived from the creating connection and never appears on the
/// wire, so a caller cannot widen its own deduplication window. One
/// reservation mechanism therefore serves both interactive creates and
/// bounded spawn retries without trusting the request to select policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DedupScope {
    /// M3's existing tombstone behavior: the key remains spent forever.
    Permanent,
    /// The key remains spent only while the child session exists.
    SessionLifetime,
}

impl DedupScope {
    /// Stable SQLite spelling, independent of Rust variant names.
    fn column(self) -> &'static str {
        match self {
            DedupScope::Permanent => "permanent",
            DedupScope::SessionLifetime => "session_lifetime",
        }
    }

    /// Decode the stored policy without guessing at corrupt or future data.
    fn from_column(text: &str) -> anyhow::Result<Self> {
        Ok(match text {
            "permanent" => DedupScope::Permanent,
            "session_lifetime" => DedupScope::SessionLifetime,
            other => anyhow::bail!("reservation row has unrecognized dedup scope {other:?}"),
        })
    }
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
    /// Derived from the creating connection, never from request fields.
    pub dedup_scope: DedupScope,
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
    Ours {
        session_token: String,
        creation_seq: u64,
    },
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
    ///
    /// `created_at` is the PREVIOUS attempt's value, not the caller's
    /// freshly minted one — see `restart_pending_launch`'s own docs for
    /// why the crashed attempt's timestamp must be preserved rather than
    /// re-minted, and `StoredSession::created_at`'s docs for the
    /// reload-then-list window that makes it client-observable. The
    /// caller threads this value into the `SessionInfo` it replies with,
    /// so a retried create's reply always matches whatever a concurrent
    /// `ListSessions` could already have shown for the row.
    ///
    /// `title` is handed back for the SAME reason and it is the sharper of
    /// the two, because a title is the one field a user can change after
    /// creation. The takeover already preserves a rename that landed
    /// between the crash and the retry (see `restart_pending_launch`), but
    /// preserving it only in SQLite is half a fix: the caller builds both
    /// its reply and the replacement in-memory entry from the snapshot it
    /// resolved before the race, so an unreported preserved title means
    /// every list served by this process shows the pre-rename label until
    /// the next reload — the user's rename accepted, acknowledged, and then
    /// apparently reverted. Returning it here is what lets the caller adopt
    /// the committed value instead of its own stale one.
    Acquired {
        created_at: i64,
        creation_seq: u64,
        title: String,
        session_token: String,
    },
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
    /// The session-level archive flag a successful restart clears.
    /// Restored when a restart fails before touching anything external, so
    /// a failed attempt cannot make an archived session appear active.
    pub archived: bool,
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
type RelaunchBasisColumns = (OutcomeColumns, String, i64, Option<String>, i64, i64, i64);

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
        K::Unauthorized => "unauthorized",
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
        "unauthorized" => K::Unauthorized,
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
///
/// Shared by the session and profile decoders, which is why the message
/// names neither: both columns hold the same vocabulary on purpose (see
/// [`PROFILES_SCHEMA`]), and each caller adds its own row identity as
/// context.
fn agent_kind_from_column(text: &str) -> anyhow::Result<farhelm_proto::AgentKind> {
    use farhelm_proto::AgentKind as K;
    Ok(match text {
        "claude" => K::Claude,
        "codex" => K::Codex,
        "generic" => K::Generic,
        other => anyhow::bail!("row has unrecognized agent kind {other:?}"),
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
        serde_json::from_str::<Vec<String>>(&text).context("decoding a stored resume template")
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
/// Not the whole `sessions` row — `outcome_state`/`exit_code`/etc. fold
/// into [`LastOutcome`] below rather than appearing as their own fields.
/// `created_at` DOES appear here as of PLAN_M6.md item 1: it was write-only
/// from this type's perspective before that (a human-inspection column and
/// a future migration's foothold, per the note that used to live here),
/// but M6's `SessionInfo::created_at` and its pagination cursor's ordering
/// key both need the value a live session was actually inserted with, so
/// it is load-bearing now. The caller decides the value (`insert_session`
/// no longer mints it internally) and `insert_session_row` simply persists
/// what it is given — see that function's own docs for why the decision
/// moved to the call site rather than staying inside the store.
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
    /// Direct parent metadata, or `None` for an ordinary root session.
    pub parent: Option<String>,
    /// Whether the session is hidden from the default list and has no
    /// terminal until restart opens a new launch generation.
    ///
    /// This is metadata rather than an outcome state. The outcome records
    /// the deliberate teardown as an annotated exit; this flag explains
    /// why the row remains while its terminal does not.
    pub archived: bool,
    pub title: String,
    /// Seconds since the Unix epoch when this row was inserted (`now_unix`,
    /// called once by the caller so the exact instant matches what
    /// `insert_session_row` persists — see that function's docs).
    ///
    /// A crash-interrupted create's RETRY (`SessionStore::
    /// restart_pending_launch`, taken only when the earlier attempt left no
    /// evidence of ever reaching tmux — see that method's own docs)
    /// PRESERVES the dead attempt's value rather than minting a fresh one.
    /// An earlier version of this doc argued mint-fresh was fine because
    /// "the reply that would have carried it never went out" — true, but
    /// beside the point: the dead attempt's row was still committed and
    /// durable before the crash, and `service::Supervisor::reload_sessions`
    /// loads and lists EVERY stored row unconditionally, `Launching`
    /// included, with no filter on whether a reply for it ever shipped
    /// (see that method's own docs). A supervisor restart between the
    /// crash and the retry therefore CAN serve this row through
    /// `ListSessions` before the retry ever runs, making the original
    /// timestamp client-observable — an observation a re-minted value on
    /// retry would silently move within PLAN_M6.md's creation-time-
    /// descending pagination order, changing where the caller's own
    /// earlier reload placed it. Preserving avoids exactly that. Once a
    /// `SessionCreated`/`SessionRestarted` reply has actually shipped a
    /// `created_at`, this field is immutable either way — nothing later
    /// in this session's life re-derives or overwrites it.
    pub created_at: i64,
    /// Strict creation order within this supervisor installation.
    ///
    /// Unlike `created_at`, this cannot tie. Retries preserve the original
    /// value, so one logical create keeps one place in the sequence.
    pub creation_seq: u64,
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
    /// Which profile this session was CREATED from, or `None` for a
    /// raw-created session (PLAN_M6_75.md item 4).
    ///
    /// Immutable, like the integration snapshot beside it and for the same
    /// reason SPEC.md gives: editing or deleting a profile must not disturb
    /// the sessions already created from it, so nothing ever rewrites this
    /// — not a rename, not a delete, not a restart. See [`ProfileSnapshot`]
    /// for what is (and deliberately is not) in it.
    pub source_profile: Option<ProfileSnapshot>,
}

/// The identity of the profile a session was created from, exactly as it
/// was at that moment (PLAN_M6_75.md item 4).
///
/// The durable half of `farhelm_proto::SourceProfile`, whose third field —
/// the profile's CURRENT existence — is deliberately absent here: existence
/// is a statement about the catalog at reply-build time, and a column
/// holding it would be wrong the moment anyone edited or deleted a profile.
/// See that wire type's docs for the whole snapshot-plus-derived-existence
/// argument; this struct is the "nothing mutable lives in the snapshot"
/// half of it.
///
/// The two fields are stored as two nullable columns that are written and
/// read only as a PAIR. SQLite is not asked to enforce that (a table-level
/// `CHECK` cannot be added by `ALTER TABLE ADD COLUMN`, so adding one would
/// make a migrated database differ from a fresh one — the exact divergence
/// `migrated_and_fresh_schemas_agree` exists to prevent), so the invariant
/// lives in code: one writer ([`insert_session_row`]) sets both or neither,
/// and [`decode_session_row`] refuses a half-written row rather than
/// guessing at the missing side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileSnapshot {
    /// The profile's immutable id — the key existence is derived by, and
    /// the key a client filters on. Never re-resolved to a name.
    pub id: String,
    /// The profile's name AS IT WAS when this session was created. Not
    /// refreshed when the profile is renamed; that is the whole point of
    /// snapshotting it (the session keeps saying what it was created from,
    /// and the derived existence is what reports the rename).
    pub name: String,
}

/// The catalog projection reply-building needs: profile id to its CURRENT
/// name, for every profile that currently exists.
///
/// Read once per reply rather than once per session — a page of sessions
/// costs one catalog read, not one lookup per row (`farhelm_proto::
/// SourceProfile`'s note on per-snapshot lookup cost). Absence of a key IS
/// the deleted case, which is why this is a whole-catalog map rather than a
/// per-id query returning `Option`: a page mixing present, renamed, and
/// deleted profiles resolves out of one map with no further I/O.
pub type ProfileNames = HashMap<String, String>;

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
    /// How many times [`SessionStore::profile_names`] has been asked for
    /// the catalog, for the tests that pin reply-build COST rather than
    /// reply-build correctness (PLAN_M6_75.md item 5's "one read per
    /// reply, not one per session").
    ///
    /// Test-only, and it has to be here rather than in the tests: the
    /// contract is about how many times production code calls the store,
    /// which nothing outside the store can observe. Cloned with the handle
    /// (an `Arc`), so a counter read through any clone sees every clone's
    /// reads — which is what a test holding one handle while the supervisor
    /// holds another needs.
    #[cfg(test)]
    profile_name_reads: Arc<std::sync::atomic::AtomicU64>,
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
/// - 5: PLAN_M3.md item 9 — `sessions.generation`, the launch counter a
///   restart bumps.
/// - 6: PLAN_M3.md item 10 — `sessions.launch_scoped`, whether that launch
///   ran inside its own transient cgroup scope.
/// - 7: PLAN_M6.md item 2 — `supervisor_meta.host_identity`, the UUID
///   [`SessionStore::ensure_host_identity`] mints on the row's first read
///   and never touches again. Alongside `boot_id` on the SAME row rather
///   than a table of its own, exactly as version 2's own entry above
///   anticipated: both are process-independent facts about this HOST's
///   install, not about any one session.
/// - 8: PLAN_M6_75.md item 4 — the `profiles` catalog together with its
///   starter rows ([`STARTER_PROFILES`], which also argues why seeding is a
///   migration rather than a startup check), and `sessions.source_profile_
///   id`/`source_profile_name`, the immutable identity of the profile a
///   session was created from. One step for both, and nothing backfilled;
///   the step's own comment carries the reasoning for each.
/// - 9: PLAN_M7.md item 2 — `create_reservations.dedup_scope`, defaulting
///   existing interactive reservations to their historical `permanent`
///   policy. Spawn begins writing `session_lifetime` in item 4.
/// - 10: PLAN_M7.md item 4 — plaintext per-session spawn credentials and
///   direct-parent metadata. Every existing row receives a credential in
///   the migration transaction; parent is absent for pre-spawn rows.
/// - 11: PR #118's spawn review — a supervisor-monotonic creation sequence.
///   The counter lives in `supervisor_meta`, so deleting the newest session
///   cannot make its value reusable the way a bare SQLite rowid could.
/// - 12: PLAN_M7.md item 5 — `sessions.archived`, durable metadata kept
///   separate from the recorded exit outcome it accompanies.
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
        conn.execute_batch(&format!(
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
                 launch_scoped         INTEGER NOT NULL DEFAULT 0,
                 source_profile_id     TEXT,
                 source_profile_name   TEXT,
                 parent                TEXT,
                 session_token         TEXT,
                 creation_seq          INTEGER,
                 archived              INTEGER NOT NULL DEFAULT 0
             ) STRICT;
             CREATE TABLE supervisor_meta (
                 id            INTEGER PRIMARY KEY CHECK (id = 0),
                 boot_id       TEXT,
                 host_identity TEXT,
                 last_creation_seq INTEGER NOT NULL DEFAULT 0
             ) STRICT;
             CREATE TABLE create_reservations (
                 intent_key   TEXT PRIMARY KEY,
                 fingerprint  TEXT NOT NULL,
                 state        TEXT NOT NULL,
                 session_id   TEXT NOT NULL,
                 tmux_name    TEXT NOT NULL,
                 error_kind   TEXT,
                 error_detail TEXT,
                 created_at   INTEGER NOT NULL,
                 dedup_scope  TEXT NOT NULL DEFAULT 'permanent'
             ) STRICT;
             CREATE INDEX create_reservations_pending
                 ON create_reservations (session_id) WHERE state = 'pending';
             {PROFILES_SCHEMA}
             {STARTER_PROFILES}
             PRAGMA user_version = 12;
             COMMIT;"
        ))
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
    if version == 6 {
        // PLAN_M6.md item 2's host identity: SPEC.md's promise that a
        // supervisor is "generated by its supervisor at install time" and
        // immutable thereafter. Nullable, and deliberately backfilled with
        // NOTHING (unlike `launch_scoped`'s `DEFAULT 0` above) — there is no
        // honest value to invent for a pre-existing row, only a real one to
        // MINT, and minting is `ensure_host_identity`'s job, not this
        // migration's: this step only makes room for the column. A migrated
        // database therefore opens with `host_identity` still NULL, exactly
        // like a freshly created one before its own first `ensure_host_
        // identity` call, so the two paths converge on identical behavior
        // (`migrated_and_fresh_schemas_agree`) rather than one silently
        // skipping the mint that gives every supervisor its identity.
        conn.execute_batch(
            "BEGIN;
             ALTER TABLE supervisor_meta ADD COLUMN host_identity TEXT;
             PRAGMA user_version = 7;
             COMMIT;",
        )
        .context("migrating schema from version 6 to 7")?;
        version = 7;
    }
    if version == 7 {
        // PLAN_M6_75.md item 4: the profiles catalog, its starter rows, and
        // the two columns a session uses to remember which profile it came
        // from.
        //
        // ONE step for both halves rather than two, because they are not
        // independently useful: a catalog with no session columns cannot
        // record a profile-backed create, and session columns with no
        // catalog name profiles that could not exist. Splitting them would
        // buy a database state — either half without the other — that no
        // code path in this build is written for, and every future reader
        // would have to reason about it anyway.
        //
        // The columns are NULLABLE with no default and are backfilled with
        // NOTHING, which is the honest reading rather than merely the easy
        // one: every session predating this migration was created from an
        // invocation, by a build that had no catalog at all, so there is no
        // profile it "really" came from and none to invent. NULL is exactly
        // what `farhelm_proto::SessionInfo::source_profile`'s absent case
        // already means — raw-created — so migrated rows and rows this
        // build writes for raw creates are indistinguishable by design.
        //
        // The starters land HERE, in the same transaction, and so an
        // upgrading host gets the catalog SPEC.md promises rather than an
        // empty one. See `STARTER_PROFILES` for why this transaction — not
        // a flag consulted at startup — is what makes seeding happen
        // exactly once and keeps a deleted starter deleted.
        conn.execute_batch(&format!(
            "BEGIN;
             ALTER TABLE sessions ADD COLUMN source_profile_id   TEXT;
             ALTER TABLE sessions ADD COLUMN source_profile_name TEXT;
             {PROFILES_SCHEMA}
             {STARTER_PROFILES}
             PRAGMA user_version = 8;
             COMMIT;"
        ))
        .context("migrating schema from version 7 to 8")?;
        version = 8;
    }
    if version == 8 {
        // PLAN_M7.md item 2's per-reservation deduplication window. Every
        // existing row came from an interactive create, so `permanent` is
        // both the compatibility default and its actual historical policy.
        // Spawn starts writing `session_lifetime` in item 4, when the
        // authenticated creating connection exists to derive it from.
        conn.execute_batch(
            "BEGIN;
             ALTER TABLE create_reservations
                 ADD COLUMN dedup_scope TEXT NOT NULL DEFAULT 'permanent';
             PRAGMA user_version = 9;
             COMMIT;",
        )
        .context("migrating schema from version 8 to 9")?;
        version = 9;
    }
    if version == 9 {
        // A credential is recoverable launch state, not an authentication
        // digest: restart has to inject the SAME value into the replacement
        // process. The nullable add is only an intermediate shape inside
        // this transaction. Every existing row is filled before the final
        // table shape is claimed by user_version 10, and all later readers
        // refuse NULL through their ordinary String decode.
        let migration = conn
            .unchecked_transaction()
            .context("starting schema migration from version 9 to 10")?;
        migration
            .execute_batch(
                "ALTER TABLE sessions ADD COLUMN parent TEXT;
                 ALTER TABLE sessions ADD COLUMN session_token TEXT;",
            )
            .context("adding spawn columns during schema migration from version 9 to 10")?;
        {
            let ids = {
                let mut stmt = migration
                    .prepare("SELECT id FROM sessions")
                    .context("preparing migrated-session credential scan")?;
                stmt.query_map([], |row| row.get::<_, String>(0))
                    .context("querying sessions that need credentials")?
                    .collect::<Result<Vec<_>, _>>()
                    .context("decoding sessions that need credentials")?
            };
            for id in ids {
                migration
                    .execute(
                        "UPDATE sessions SET session_token = ?2 WHERE id = ?1",
                        rusqlite::params![id, mint_session_token()?],
                    )
                    .context("minting a migrated session credential")?;
            }
        }
        migration
            .pragma_update(None, "user_version", 10)
            .context("recording schema version 10")?;
        migration
            .commit()
            .context("committing schema migration from version 9 to 10")?;
        version = 10;
    }
    if version == 10 {
        // Seconds are display data, not chronology: concurrent creates can
        // share one timestamp. The explicit counter remains monotonic even
        // after the newest session is deleted, unlike a sessions-table
        // rowid whose maximum value SQLite is allowed to reuse.
        //
        // Existing rows receive the old stable ordering: `created_at`
        // ascending, then id descending, so the previous newest ordering
        // (`created_at DESC, id ASC`) maps to the greatest sequence.
        let migration = conn
            .unchecked_transaction()
            .context("starting schema migration from version 10 to 11")?;
        migration
            .execute_batch(
                "ALTER TABLE sessions ADD COLUMN creation_seq INTEGER;
                 ALTER TABLE supervisor_meta
                     ADD COLUMN last_creation_seq INTEGER NOT NULL DEFAULT 0;",
            )
            .context("adding creation-sequence storage")?;
        let ids = {
            let mut stmt = migration
                .prepare("SELECT id FROM sessions ORDER BY created_at ASC, id DESC")
                .context("preparing migrated-session sequence scan")?;
            stmt.query_map([], |row| row.get::<_, String>(0))
                .context("querying sessions that need creation sequences")?
                .collect::<Result<Vec<_>, _>>()
                .context("decoding sessions that need creation sequences")?
        };
        for (index, id) in ids.iter().enumerate() {
            let sequence = i64::try_from(index + 1).context("too many migrated sessions")?;
            migration
                .execute(
                    "UPDATE sessions SET creation_seq = ?2 WHERE id = ?1",
                    rusqlite::params![id, sequence],
                )
                .context("assigning a migrated session creation sequence")?;
        }
        let last = i64::try_from(ids.len()).context("too many migrated sessions")?;
        migration
            .execute(
                "INSERT INTO supervisor_meta (id, last_creation_seq) VALUES (0, ?1)
                 ON CONFLICT(id) DO UPDATE SET last_creation_seq = excluded.last_creation_seq",
                rusqlite::params![last],
            )
            .context("initializing the session creation counter")?;
        migration
            .pragma_update(None, "user_version", 11)
            .context("recording schema version 11")?;
        migration
            .commit()
            .context("committing schema migration from version 10 to 11")?;
        version = 11;
    }
    if version == 11 {
        conn.execute_batch(
            "BEGIN;
             ALTER TABLE sessions ADD COLUMN archived INTEGER NOT NULL DEFAULT 0;
             PRAGMA user_version = 12;
             COMMIT;",
        )
        .context("migrating schema from version 11 to 12")?;
        version = 12;
    }
    if version == SCHEMA_VERSION {
        return Ok(());
    }
    anyhow::bail!(
        "supervisor.db has schema version {version}, but this build only understands \
         version {SCHEMA_VERSION}; refusing to open it rather than risk misreading it"
    )
}

/// Seconds since the Unix epoch, for [`StoredSession::created_at`] and the
/// `create_reservations` table's own informational timestamp. `pub(crate)`
/// so a `StoredSession` builder in `service` can mint the SAME value it
/// hands to `insert_session`/`restart_pending_launch`, which is what keeps
/// a freshly created `SessionInfo`'s `created_at` (PLAN_M6.md item 1)
/// consistent with the row that actually lands in SQLite rather than a
/// second, independently-timed reading. Never fails the caller over a
/// clock reading: a pre-epoch system clock — the only way `duration_since`
/// errors — degrades to `0` instead of rejecting an otherwise-successful
/// session creation.
///
/// DECISION (accepted, not built around): a pre-epoch clock's `0` is the
/// same bit pattern `SessionInfo::created_at`'s wire doc assigns "sender
/// predates the field" — a real pre-epoch host and an old sender that
/// never sent this column are indistinguishable to any reader. Accepted
/// as-is rather than given a fallible signature or a sentinel floor: a
/// pre-epoch system clock is not a configuration this system supports
/// running under at all, the worst case of the collision is one session's
/// row sorting as if it were legacy-shaped (last, in the pagination
/// order's descending-by-`created_at` walk), and total order survives
/// regardless because the `id` tiebreak never depends on `created_at`
/// being distinct.
pub(crate) fn now_unix() -> i64 {
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
///
/// `row.created_at` is persisted VERBATIM rather than minted here with
/// `now_unix()` (as this function did before PLAN_M6.md item 1): the
/// caller — `Supervisor::launch_reserved` — reads `now_unix()` itself once
/// and reuses that same value for the `SessionInfo` it hands back in its
/// create reply, so the timestamp the client sees always matches the one
/// this row actually gets. Computing it twice (once here, once at the
/// call site) would let the two drift by however long launch took.
/// Values minted with a session row and needed by its first launch.
struct InsertedSession {
    session_token: String,
    creation_seq: u64,
}

/// Advance the durable supervisor-wide creation counter.
///
/// The caller owns a transaction. Keeping allocation inside that same
/// transaction means a failed insert consumes no sequence and a committed
/// row can never exist without one.
fn next_creation_seq(conn: &Connection) -> anyhow::Result<u64> {
    conn.execute(
        "INSERT INTO supervisor_meta (id, last_creation_seq) VALUES (0, 0)
         ON CONFLICT(id) DO NOTHING",
        [],
    )
    .context("ensuring the session creation counter exists")?;
    conn.execute(
        "UPDATE supervisor_meta SET last_creation_seq = last_creation_seq + 1 WHERE id = 0",
        [],
    )
    .context("advancing the session creation counter")?;
    let value: i64 = conn
        .query_row(
            "SELECT last_creation_seq FROM supervisor_meta WHERE id = 0",
            [],
            |row| row.get(0),
        )
        .context("reading the advanced session creation counter")?;
    u64::try_from(value).context("session creation counter became negative")
}

fn insert_session_row(
    conn: &Connection,
    row: &StoredSession,
    preserved: Option<(&str, u64)>,
) -> anyhow::Result<InsertedSession> {
    let (state, exit_code, annotation, error_detail) = row.outcome.columns();
    let (session_token, creation_seq) = match preserved {
        Some((token, sequence)) => (token.to_string(), sequence),
        None => (mint_session_token()?, next_creation_seq(conn)?),
    };
    let stored_creation_seq = i64::try_from(creation_seq).context("creation sequence overflow")?;
    conn.execute(
        "INSERT INTO sessions \
         (id, title, cwd, invocation, tmux_name, pane, created_at, creation_seq, \
          outcome_state, exit_code, annotation, error_detail, \
          agent_kind, resume_template, canonical_cwd, captured_conversation, \
          captured_record, capture_ambiguous, first_input_at, generation, launch_scoped, \
          source_profile_id, source_profile_name, parent, session_token, archived) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, \
                 ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)",
        rusqlite::params![
            row.id,
            row.title,
            row.cwd,
            row.invocation,
            row.tmux_name,
            row.pane,
            row.created_at,
            stored_creation_seq,
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
            // Both or neither, from one `Option` — see `ProfileSnapshot`
            // for why this pairing is a code invariant rather than a
            // `CHECK` constraint.
            row.source_profile.as_ref().map(|profile| &profile.id),
            row.source_profile.as_ref().map(|profile| &profile.name),
            row.parent,
            session_token,
            i64::from(row.archived),
        ],
    )
    .context("inserting session row")?;
    Ok(InsertedSession {
        session_token,
        creation_seq,
    })
}

/// The column list every session read shares, in the order
/// [`decode_session_row`] expects. Named so the two readers cannot drift
/// apart by one column and start decoding each other's fields.
///
/// `created_at` is appended at the END here even though the schema itself
/// puts the column right after `pane` (see the `CREATE TABLE` above) —
/// this list's order does NOT need to match the table's. What it must
/// match is [`read_session_columns`]'s positional `r.get(N)` calls: every
/// position before a given column is load-bearing for every index after
/// it, so inserting a new column in the MIDDLE of this projection would
/// silently shift every existing index by one. Appending is the one
/// change that cannot do that, regardless of where the column actually
/// sits in the table.
const SESSION_COLUMNS: &str = "id, title, cwd, invocation, tmux_name, pane, \
                               outcome_state, exit_code, annotation, error_detail, \
                               agent_kind, resume_template, canonical_cwd, \
                               captured_conversation, captured_record, capture_ambiguous, \
                               first_input_at, generation, launch_scoped, created_at, \
                               source_profile_id, source_profile_name, parent, creation_seq, \
                               archived";

/// The raw columns of one session row, before the fallible decoding that
/// cannot happen inside a rusqlite row mapper (whose error type is
/// rusqlite's own — see `load_all`'s two-stage comment).
///
/// The trailing members are the raw agent-kind text, the raw
/// resume-template JSON, and the raw source-profile id/name pair; every
/// other column is already in place on the partially-built
/// `StoredSession`, because only these (with the outcome) can be REFUSED.
type SessionColumns = (
    StoredSession,
    OutcomeColumns,
    String,
    Option<String>,
    (Option<String>, Option<String>),
    i64,
);

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
            parent: r.get(22)?,
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
            created_at: r.get(19)?,
            creation_seq: 0,
            source_profile: None,
            archived: r.get::<_, i64>(24)? != 0,
        },
        (r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?),
        r.get(10)?,
        r.get(11)?,
        (r.get(20)?, r.get(21)?),
        r.get::<_, i64>(23)?,
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
///
/// A HALF-WRITTEN source-profile pair is refused for the same class of
/// reason (see [`ProfileSnapshot`] for why SQLite is not asked to enforce
/// the pairing): an id with no name would render as a session created from
/// a nameless profile, and a name with no id could never have its existence
/// derived at all, since the id is the only key the catalog is looked up
/// by. Neither is something to invent a value for.
fn decode_session_row(columns: SessionColumns) -> anyhow::Result<StoredSession> {
    let (
        mut row,
        (state, exit_code, annotation, error_detail),
        kind,
        template,
        source_profile,
        creation_seq,
    ) = columns;
    row.creation_seq = u64::try_from(creation_seq)
        .with_context(|| format!("session {} has a negative creation sequence", row.id))?;
    row.source_profile = match source_profile {
        (Some(id), Some(name)) => Some(ProfileSnapshot { id, name }),
        (None, None) => None,
        (id, name) => anyhow::bail!(
            "session {} has only half of a source-profile snapshot recorded (id {:?}, name {:?}); \
             the two columns are written together or not at all",
            row.id,
            id,
            name
        ),
    };
    row.outcome = LastOutcome::from_columns(&state, exit_code, annotation, error_detail)
        .with_context(|| format!("session {}", row.id))?;
    row.agent_kind =
        agent_kind_from_column(&kind).with_context(|| format!("session {}", row.id))?;
    row.resume_template =
        resume_template_from_column(template).with_context(|| format!("session {}", row.id))?;
    // The stored template is an argv a RESTART will hand to `execvp`, so
    // the shapes that could never be one are refused at the trust boundary
    // rather than at the restart that needed them. A session's own
    // create-time validation is not enough on its own: this row may have
    // been written by a build with looser rules, or edited by hand, and by
    // restart time the honest options are a garbled command line or
    // silently declining to resume a conversation the session captured.
    if let Some(template) = row.resume_template.as_deref()
        && let Err(message) = crate::agent_kind::ensure_resume_template(template)
    {
        anyhow::bail!("session {}: {message}", row.id);
    }
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

/// The column list every profile read shares, in the order
/// [`read_profile_columns`] expects — the same drift hazard
/// [`SESSION_COLUMNS`] documents, on a much smaller table.
const PROFILE_COLUMNS: &str = "id, name, invocation, agent_kind, resume_template";

/// One profile row exactly as SQLite hands it over, before the two
/// fallible decodings (`agent_kind`, `resume_template`) that cannot run
/// inside a rusqlite row mapper. Same two-stage shape, same reason, as
/// [`SessionColumns`].
type ProfileColumns = (String, String, String, String, Option<String>);

fn read_profile_columns(r: &rusqlite::Row<'_>) -> rusqlite::Result<ProfileColumns> {
    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
}

/// Finish decoding a profile row, refusing rather than guessing on a value
/// outside this build's vocabulary — the trust-boundary rule
/// [`decode_session_row`] states in full.
///
/// The wire [`farhelm_proto::Profile`] IS the stored shape, so there is no
/// store-side mirror type here (unlike [`StoredSession`], which carries
/// `tmux_name`/`pane` that no reply has any business seeing). A profile has
/// no supervisor-private field to hide and no derived field to compute: the
/// catalog holds exactly what a client sent and exactly what a client is
/// told back, and a second struct would be a copy with nothing to add.
fn decode_profile_row(columns: ProfileColumns) -> anyhow::Result<farhelm_proto::Profile> {
    let (id, name, invocation, kind, template) = columns;
    let agent_kind = agent_kind_from_column(&kind).with_context(|| format!("profile {id}"))?;
    let resume_template =
        resume_template_from_column(template).with_context(|| format!("profile {id}"))?;
    // The SEMANTIC rules too, not merely the syntactic ones. A row that
    // decodes into a well-formed struct can still describe a profile no
    // create could use — a blank name that renders as an unpickable row, an
    // invocation naming no program, an oversized record that makes
    // `ProfileList` undeliverable — and until this ran below the handler,
    // reaching that state took nothing more exotic than a database restored
    // from a build with looser rules, or one hand-edited.
    //
    // REFUSED rather than skipped, which is the same stance every other
    // decode in this module takes and the right one here for a sharper
    // reason: quietly dropping the row from the listing would leave a
    // profile the user can see in no picker and therefore cannot delete or
    // repair, while every create that names its id keeps working.
    validate_profile_fields(&name, &invocation, agent_kind, resume_template.as_deref())
        .map_err(|message| anyhow::anyhow!("profile {id}: {message}"))?;
    Ok(farhelm_proto::Profile {
        id,
        name,
        invocation,
        agent_kind,
        resume_template,
    })
}

/// Cap on how many argv elements a resume template may carry, whether it
/// arrives as a profile definition or as a `CreateSession` override
/// (PLAN_M3.md items 6 and 7; PLAN_M6_75.md item 4).
///
/// Independent of the byte caps it is enforced alongside, because the two
/// bound different things: a template of ten thousand EMPTY elements costs
/// almost nothing in bytes while still being nothing a resume invocation
/// could legitimately be, and on the create path it lands in a never-pruned
/// reservation row. 64 elements is far beyond every real resume invocation
/// (`claude --resume {conversation}` is three).
///
/// It lives HERE rather than beside the request caps in `service::handlers`
/// only because [`validate_profile_fields`] had to move below the handler
/// to be reachable from the store's own writes; `handlers` imports it back
/// for the create-override check, which is the other half of the same rule.
pub(crate) const RESUME_TEMPLATE_ELEMENT_CAP: usize = 64;

/// Everything a profile record must satisfy before it is stored, or after
/// it is read back (PLAN_M6_75.md item 4).
///
/// One function for every one of those moments, because the alternative is
/// rules that hold only where someone remembered to apply them. It began as
/// a handler-side check on `CreateProfile`/`UpdateProfile`, which left
/// [`SessionStore::create_profile`] and [`SessionStore::update_profile`]
/// admitting anything a direct caller handed them, and left
/// [`decode_profile_row`] returning whatever an older build (or a hand-edit)
/// had committed. Both gaps end in the same two places: a catalog too large
/// to LIST, or a snapshotted restart with an argv nothing can run.
///
/// The `Err` is the user-facing message verbatim (SPEC.md's concrete,
/// actionable errors), naming the limit it hit — the same shape
/// `create_mode` and the create caps use. Callers wrap it in whichever
/// `ErrorKind` or context their boundary wants.
///
/// The rules, and why each:
///
/// - **The field cap.** [`farhelm_proto::PROFILE_FIELD_CAP`] bounds this
///   record's own caller-supplied text. Together with
///   [`farhelm_proto::MAX_PROFILES_PER_HOST`] (enforced in the insert's own
///   transaction, since only a transaction can read a COUNT truthfully) it
///   is what keeps `ProfileList` sendable — and a catalog that could outgrow
///   one reply could never be listed, and therefore never be trimmed back
///   down.
/// - **The element cap.** [`RESUME_TEMPLATE_ELEMENT_CAP`], for the reason
///   that constant gives.
/// - **A printable, non-blank name.** A profile name is a one-line label
///   rendered in pickers, logs, and terminals exactly as a session title is,
///   so it gets the title's rule for control characters: refused, never
///   sanitized, because it is caller data nothing legitimate spells this
///   way. It additionally may not be EMPTY or all whitespace, which is where
///   the two part company — a session title may be blank (the server derives
///   one), but a profile is a NAMED definition whose whole purpose is being
///   picked out of a list, and a blank row in that list is not something a
///   user can act on or tell apart from its neighbours.
/// - **A usable invocation.** Parsed as a command line and checked as an
///   executable argv (`agent_kind::ensure_executable_argv`), then resolved
///   through the very same `IntegrationSnapshot::resolve` a create will run
///   it through — so a profile that could never launch anything is refused
///   when it is WRITTEN rather than at every create that names it
///   afterwards. Catching it here is what keeps "pick a profile" from being
///   a request that can fail for reasons the picker could not have shown.
/// - **An executable resume template**
///   (`agent_kind::ensure_resume_template`). Same argument, one step further
///   out: the moment a bad template would otherwise be discovered is a
///   restart that has a captured conversation to resume and no way left to
///   do it.
pub(crate) fn validate_profile_fields(
    name: &str,
    invocation: &str,
    agent_kind: farhelm_proto::AgentKind,
    resume_template: Option<&[String]>,
) -> Result<(), String> {
    let template_bytes: usize = resume_template
        .iter()
        .flat_map(|template| template.iter())
        .map(String::len)
        .sum();
    let field_len = name.len() + invocation.len() + template_bytes;
    if field_len > farhelm_proto::PROFILE_FIELD_CAP {
        return Err(format!(
            "profile name, invocation, and resume template together are {field_len} bytes, \
             exceeding the {}-byte limit",
            farhelm_proto::PROFILE_FIELD_CAP
        ));
    }
    if resume_template.is_some_and(|template| template.len() > RESUME_TEMPLATE_ELEMENT_CAP) {
        return Err(format!(
            "resume template has {} elements, exceeding the \
             {RESUME_TEMPLATE_ELEMENT_CAP}-element limit",
            resume_template.map_or(0, <[String]>::len)
        ));
    }
    if name.chars().any(char::is_control) {
        return Err("profile name must not contain control characters".to_string());
    }
    if name.trim().is_empty() {
        return Err(
            "profile name must not be empty or only whitespace; a profile is a NAMED definition \
             and a blank label cannot be picked out of a list"
                .to_string(),
        );
    }
    if let Some(template) = resume_template {
        crate::agent_kind::ensure_resume_template(template)?;
    }
    // Checked on the STRING before the split, because `shell_words` would
    // happily carry a NUL through into an argv element and the message
    // should name the field the caller sent rather than a token of it.
    if invocation.contains('\0') {
        return Err(
            "profile invocation contains a NUL byte, which cannot survive being passed to a \
             program"
                .to_string(),
        );
    }
    let argv = shell_words::split(invocation)
        .map_err(|e| format!("profile invocation does not parse as a command line: {e}"))?;
    crate::agent_kind::ensure_executable_argv("profile invocation", &argv)?;
    crate::agent_kind::IntegrationSnapshot::resolve(
        &argv[0],
        Some(agent_kind),
        resume_template.map(<[String]>::to_vec),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// What [`SessionStore::create_profile`] did, since one of the two answers
/// is a refusal rather than a failure.
///
/// A dedicated outcome instead of an `Err`: the catalog being full is a
/// bounded, expected answer to a legitimate request (the caller must be
/// told which limit it hit and what to do about it), not an error the store
/// failed at. It is spelled here rather than checked by the caller because
/// the check and the insert must be ONE transaction — a count read outside
/// it could be stale by the time the insert lands, which is precisely how a
/// bound gets exceeded by concurrent creates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileCreation {
    /// Stored, with the id the store minted for it.
    Created(farhelm_proto::Profile),
    /// Refused: the catalog already holds
    /// [`farhelm_proto::MAX_PROFILES_PER_HOST`] profiles and NOTHING was
    /// written.
    CatalogFull,
}

fn read_reservation(conn: &Connection, intent_key: &str) -> anyhow::Result<Option<Reservation>> {
    let row = conn
        .query_row(
            "SELECT fingerprint, state, session_id, tmux_name, error_kind, error_detail, \
                    dedup_scope \
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
                    r.get::<_, String>(6)?,
                ))
            },
        )
        .optional()
        .context("reading a create reservation")?;
    let Some((fingerprint, state, session_id, tmux_name, error_kind, error_detail, dedup_scope)) =
        row
    else {
        return Ok(None);
    };
    let outcome = ReservationOutcome::from_columns(&state, error_kind, error_detail)
        .with_context(|| format!("create reservation {intent_key}"))?;
    Ok(Some(Reservation {
        intent_key: intent_key.to_string(),
        fingerprint,
        session_id,
        tmux_name,
        dedup_scope: DedupScope::from_column(&dedup_scope)
            .with_context(|| format!("create reservation {intent_key}"))?,
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
            #[cfg(test)]
            profile_name_reads: Arc::new(std::sync::atomic::AtomicU64::new(0)),
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
                          error_kind, error_detail, created_at, dedup_scope) \
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
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
                            claim.dedup_scope.column(),
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
            let inserted = insert_session_row(&tx, &row, None)?;
            tx.commit().context("committing the session insert")?;
            Ok(Claimed::Ours {
                session_token: inserted.session_token,
                creation_seq: inserted.creation_seq,
            })
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
        // A bounded key has no child to bound it when validation fails.
        // Recording that refusal would therefore turn a session-lifetime
        // reservation into an immortal row. Its retry revalidates instead.
        if claim.dedup_scope == DedupScope::SessionLifetime {
            return Ok(());
        }
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
                  error_kind, error_detail, created_at, dedup_scope) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
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
                    claim.dedup_scope.column(),
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
    /// the same id and tmux name the reservation already carries. `row`'s
    /// OWN `created_at` is discarded in favor of the row it replaces: see
    /// [`StoredSession::created_at`]'s docs for why the crashed attempt's
    /// timestamp already durable a moment ago must survive the takeover
    /// rather than being re-minted, and the returned [`RetryClaim::Acquired`]
    /// carries that preserved value back to the caller for its reply.
    ///
    /// The same applies to `title`, and there it protects a USER action
    /// rather than an ordering property: a rename committed between the two
    /// attempts would otherwise be undone by this reinsert. It is handed
    /// back on `Acquired` for the same reason `created_at` is — the caller's
    /// reply and its replacement in-memory entry are built from a snapshot
    /// resolved before the race, so preserving the rename in SQLite alone
    /// would leave every list this process serves showing the old label
    /// until the next reload. Those two are
    /// the whole of what survives — every other column on the replaced row
    /// describes a launch that provably never happened.
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
            let current: Option<(String, String, i64, String, String, i64)> = tx
                .query_row(
                    "SELECT outcome_state, pane, created_at, title, session_token, creation_seq \
                     FROM sessions WHERE id = ?1",
                    rusqlite::params![row.id],
                    |r| {
                        Ok((
                            r.get(0)?,
                            r.get(1)?,
                            r.get(2)?,
                            r.get(3)?,
                            r.get(4)?,
                            r.get(5)?,
                        ))
                    },
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
            //
            // `created_at` rides along in the same read as a matched pair
            // with `outcome_state`/`pane`: whichever row this transaction
            // is about to act on (refuse, or replace) is also the row whose
            // timestamp the reply must honor, so reading all three off one
            // committed snapshot rules out a second query ever disagreeing
            // with the first about which row it saw.
            let (preserved_created_at, preserved_title, preserved_token, preserved_sequence) =
                match current {
                    Some((state, pane, created_at, title, token, sequence)) => {
                        if !pane.is_empty()
                            || !matches!(state.as_str(), "launching" | "interrupted")
                        {
                            return Ok(RetryClaim::Launched);
                        }
                        (
                            created_at,
                            title,
                            Some(token),
                            Some(u64::try_from(sequence).context("negative creation sequence")?),
                        )
                    }
                    // Contradicts `SessionStore::insert_session`'s own
                    // invariant — a Pending reservation's row is committed in
                    // the SAME transaction as the reservation itself, so one
                    // can never durably exist without the other. Handled
                    // rather than asserted for the same reason the relaunch
                    // takeover as a whole re-checks its conditions instead of
                    // trusting the caller's evidence: "cannot happen" is a
                    // poor thing to stake a duplicate agent on, and here that
                    // would extend to a lost timestamp too. Falls back to the
                    // caller's own freshly minted `row.created_at` — the
                    // least-wrong answer when the row this takeover was
                    // supposed to preserve cannot be found at all. (The
                    // original `current.is_some_and(..)` check this replaces
                    // took the same "nothing found, proceed anyway" branch for
                    // `None`, so this preserves that behavior rather than
                    // introducing a new refusal path.)
                    None => (row.created_at, row.title.clone(), None, None),
                };
            tx.execute(
                "DELETE FROM sessions WHERE id = ?1",
                rusqlite::params![row.id],
            )
            .context("clearing the interrupted attempt's launching row")?;
            // The title comes from the ROW being replaced, not from the
            // caller's snapshot, and for a sharper reason than `created_at`
            // above: `title` is the one field a USER can change after
            // creation (`set_session_title`), and this takeover is a
            // delete-and-reinsert. A rename that landed between the crashed
            // attempt and this retry would otherwise be silently undone —
            // the user's rename accepted, acknowledged, and then reverted
            // by a relaunch that had no idea it happened. Taking it from
            // the same committed snapshot the other two conditions are
            // read from is what makes that atomic rather than a second
            // read racing the first.
            let mut row = StoredSession {
                created_at: preserved_created_at,
                creation_seq: 0,
                title: preserved_title,
                ..row
            };
            let inserted = insert_session_row(
                &tx,
                &row,
                preserved_token.as_deref().zip(preserved_sequence),
            )
            .context("re-inserting the launching row for a relaunch")?;
            tx.commit().context("committing the relaunch takeover")?;
            let preserved_title = std::mem::take(&mut row.title);
            Ok(RetryClaim::Acquired {
                created_at: preserved_created_at,
                creation_seq: inserted.creation_seq,
                title: preserved_title,
                session_token: inserted.session_token,
            })
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
    /// - `archived` is cleared because restart is the only unarchive path.
    ///   The prior value rides in [`PriorRun`] so a definitive launch
    ///   failure can restore the archived row rather than exposing it as an
    ///   active session with no terminal.
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
                     generation, captured_conversation, capture_ambiguous, launch_scoped, \
                     archived \
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
                            r.get(9)?,
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
                archived,
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
                archived: archived != 0,
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
                 archived = 0, \
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
        let archived = i64::from(prior.archived);
        tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
            let conn = conn.lock().expect("session db mutex poisoned");
            let restored = conn
                .execute(
                    "UPDATE sessions SET outcome_state = ?2, exit_code = ?3, annotation = ?4, \
                     error_detail = ?5, pane = ?6, launch_scoped = ?8, archived = ?9 \
                     WHERE id = ?1 AND generation = ?7",
                    rusqlite::params![
                        id,
                        state,
                        exit_code,
                        annotation,
                        error_detail,
                        pane,
                        generation,
                        scoped,
                        archived,
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

    /// Recover the credential one session's current and future launches use.
    ///
    /// This is intentionally a separate projection from [`Self::session`]:
    /// callers describing sessions never need the bearer value, so keeping
    /// it out of `StoredSession` prevents an innocent `Debug` or log of that
    /// metadata type from exposing it.
    pub async fn session_token(&self, id: &str) -> anyhow::Result<Option<String>> {
        let conn = Arc::clone(&self.conn);
        let id = id.to_string();
        tokio::task::spawn_blocking(move || -> anyhow::Result<Option<String>> {
            let conn = conn.lock().expect("session db mutex poisoned");
            conn.query_row(
                "SELECT session_token FROM sessions WHERE id = ?1",
                rusqlite::params![id],
                |row| row.get(0),
            )
            .optional()
            .context("reading a session credential")
        })
        .await
        .context("session credential read task panicked")?
    }

    /// Validate a hello's session attribution without exposing the stored
    /// bearer value to the connection layer.
    pub async fn authenticates_session(&self, id: &str, token: &str) -> anyhow::Result<bool> {
        let Some(expected) = self.session_token(id).await? else {
            return Ok(false);
        };
        Ok(expected.as_bytes().ct_eq(token.as_bytes()).into())
    }

    /// The newest profile-backed session's immutable source snapshot.
    ///
    /// No catalog join appears here on purpose. Spawn defaults to the last
    /// profile that was actually used, even when that profile has since
    /// been deleted; the caller must then refuse and name `--agent`, never
    /// walk backward to an older surviving profile. The monotonic creation
    /// sequence is the chronology authority; unlike wall-clock seconds and
    /// random ids, it cannot tie or reorder same-second creates.
    pub async fn latest_source_profile(&self) -> anyhow::Result<Option<ProfileSnapshot>> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<Option<ProfileSnapshot>> {
            let conn = conn.lock().expect("session db mutex poisoned");
            conn.query_row(
                "SELECT source_profile_id, source_profile_name FROM sessions \
                 WHERE source_profile_id IS NOT NULL \
                 ORDER BY creation_seq DESC LIMIT 1",
                [],
                |row| {
                    Ok(ProfileSnapshot {
                        id: row.get(0)?,
                        name: row.get(1)?,
                    })
                },
            )
            .optional()
            .context("reading the most recently used source profile")
        })
        .await
        .context("last-used profile read task panicked")?
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
                    "SELECT intent_key, fingerprint, session_id, tmux_name, dedup_scope \
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
                        dedup_scope: DedupScope::from_column(&r.get::<_, String>(4)?).map_err(
                            |error| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    4,
                                    rusqlite::types::Type::Text,
                                    error.into(),
                                )
                            },
                        )?,
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

    /// [`SessionStore::delete_session`] for the user-visible DELETE path.
    ///
    /// Permanent interactive reservations are settled as `Created` in the
    /// same transaction as row removal: their keys remain tombstones. A
    /// session-lifetime spawn reservation is deleted instead, because its
    /// key is promised only while this child exists and becomes reusable at
    /// the same commit that removes the child.
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
    /// The permanent reservation row is deliberately kept; see
    /// [`Reservation`]'s docs on tombstones. This is a separate method from plain
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
                "DELETE FROM create_reservations \
                 WHERE session_id = ?1 AND dedup_scope = 'session_lifetime'",
                rusqlite::params![id],
            )
            .context("pruning the deleted spawn's bounded reservation")?;
            tx.execute(
                "UPDATE create_reservations SET state = 'created' \
                 WHERE session_id = ?1 AND state = 'pending' \
                 AND dedup_scope = 'permanent'",
                rusqlite::params![id],
            )
            .context("settling the deleted interactive session's reservations")?;
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
                let current: Option<(OutcomeColumns, i64, bool)> = tx
                    .query_row(
                        "SELECT outcome_state, exit_code, annotation, error_detail, generation, \
                             archived \
                             FROM sessions WHERE id = ?1",
                        rusqlite::params![id],
                        |r| {
                            Ok((
                                (r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?),
                                r.get(4)?,
                                r.get::<_, i64>(5)? != 0,
                            ))
                        },
                    )
                    .optional()
                    .context("reading the current outcome")?;
                let Some((
                    (state, exit_code, annotation, error_detail),
                    current_generation,
                    archived,
                )) = current
                else {
                    continue;
                };
                let current =
                    LastOutcome::from_columns(&state, exit_code, annotation, error_detail)
                        .with_context(|| format!("session {id}"))?;
                // Archive is an outcome fence of its own. The observation
                // may have cloned this launch before archive reached
                // SQLite; once the flag lands, no delayed pane or exit
                // observation may restore terminal state to the row.
                // Checked in the same transaction as the outcome write, so
                // either commit order converges on archive's deliberate
                // terminal-less exit.
                if archived {
                    committed.insert(id, current);
                    continue;
                }
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

    /// Mark a session archived after its process tree and terminals are
    /// gone, preserving every other piece of session metadata.
    ///
    /// The flag and the deliberate annotated exit land in one transaction,
    /// so no reader can observe an archived row that still claims a live or
    /// interrupted outcome. `Ok(None)` means the row vanished. `Some(false)`
    /// is the idempotent already-archived case; `Some(true)` means this call
    /// performed the transition.
    pub async fn archive_session(&self, id: &str) -> anyhow::Result<Option<bool>> {
        let conn = Arc::clone(&self.conn);
        let id = id.to_string();
        tokio::task::spawn_blocking(move || -> anyhow::Result<Option<bool>> {
            let mut conn = conn.lock().expect("session db mutex poisoned");
            let tx = conn
                .transaction()
                .context("beginning the session archive transaction")?;
            let archived = tx
                .query_row(
                    "SELECT archived FROM sessions WHERE id = ?1",
                    rusqlite::params![id],
                    |row| row.get::<_, i64>(0),
                )
                .optional()
                .context("reading the session archive flag")?;
            let Some(archived) = archived else {
                return Ok(None);
            };
            if archived != 0 {
                return Ok(Some(false));
            }
            tx.execute(
                "UPDATE sessions SET archived = 1, pane = '', outcome_state = 'exited', \
                 exit_code = NULL, annotation = ?2, error_detail = NULL WHERE id = ?1",
                rusqlite::params![id, farhelm_proto::STOP_ANNOTATION],
            )
            .context("archiving the session row")?;
            tx.commit().context("committing the session archive")?;
            Ok(Some(true))
        })
        .await
        .context("session archive task panicked")?
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

    /// Read this host's identity, minting and persisting a fresh UUIDv4 on
    /// the row's first read — SPEC.md's promise that a supervisor's
    /// identity is "generated by its supervisor at install time" and, per
    /// the same passage, immutable thereafter. Idempotent across any number
    /// of calls, in this process or any other: a supervisor calls this once
    /// at construction (`Supervisor::new`) and every later hello just
    /// reports the field that call already resolved, but a second caller —
    /// a test asserting stability, or two supervisors racing to open a
    /// genuinely fresh database — reads back the SAME identity rather than
    /// minting a second one.
    ///
    /// Callers must hold the state directory's exclusivity
    /// (`service::StateDirOwnership`) before calling this — minting is a
    /// durable write, and a process without the claim has no standing to
    /// perform one (see `Supervisor::host_identity`'s own docs). A
    /// claimless process calls [`Self::read_host_identity`] instead, which
    /// never mints.
    ///
    /// The database, not this process, is the source of truth for whether
    /// minting has happened: wiping the state directory (and therefore this
    /// database) is indistinguishable from a genuine first run, which is
    /// exactly SPEC.md's reinstall semantics — a fresh install gets a fresh
    /// identity because nothing durable says otherwise.
    ///
    /// The identity is an OPAQUE token: every consumer compares it for
    /// equality only, and nothing anywhere parses it as a UUID even though
    /// minting happens to use one. A row hand-edited to some other non-NULL
    /// string is therefore served as-is, not rejected — the panel that
    /// settled this deliberately rejected read-back validation (checking
    /// the stored value still parses as a UUID before trusting it): a
    /// human editing this row already has full filesystem access to the
    /// database, so validation would only ever catch accidental corruption,
    /// and this build has no story for repairing a corrupted identity
    /// short of a fresh install anyway — a startup failure over a value
    /// this function never needed to understand in the first place would
    /// be a worse outcome than simply serving what is there.
    ///
    /// The write is a conditional upsert (`... WHERE host_identity IS
    /// NULL`), not an unconditional one, which is the mechanism behind
    /// "never regenerated while the row exists": if some other writer won
    /// the race between this call's read and its write, the condition
    /// simply matches nothing and this call's own candidate UUID is
    /// discarded — the trailing `SELECT` below reports whatever actually
    /// landed, never the value this call merely proposed. There is no
    /// transaction spanning the leading read and the write below it — they
    /// are two separate statements, and both racers' leading reads can
    /// equally well observe `NULL`. What actually decides the race is
    /// SQLite's own single-writer serialization of the CONDITIONAL WRITE
    /// itself: only one of the two `INSERT ... ON CONFLICT ... WHERE`
    /// statements can be the one whose `WHERE` clause still matches, so
    /// exactly one candidate lands, and the trailing re-`SELECT` — run
    /// fresh by BOTH callers, after their own write attempt — is what lets
    /// each one observe the actual winner rather than trust its own
    /// (possibly losing) candidate.
    pub async fn ensure_host_identity(&self) -> anyhow::Result<String> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            let conn = conn.lock().expect("session db mutex poisoned");
            let existing: Option<Option<String>> = conn
                .query_row(
                    "SELECT host_identity FROM supervisor_meta WHERE id = ?1",
                    rusqlite::params![META_ROW_ID],
                    |r| r.get(0),
                )
                .optional()
                .context("reading stored host identity")?;
            if let Some(Some(id)) = existing {
                return Ok(id);
            }
            let candidate = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO supervisor_meta (id, host_identity) VALUES (?1, ?2) \
                 ON CONFLICT(id) DO UPDATE SET host_identity = excluded.host_identity \
                 WHERE supervisor_meta.host_identity IS NULL",
                rusqlite::params![META_ROW_ID, candidate],
            )
            .context("minting host identity")?;
            // Re-read rather than trust `candidate`: see the doc comment
            // above on why a concurrent winner's value, not this call's own
            // proposal, must be what gets returned.
            conn.query_row(
                "SELECT host_identity FROM supervisor_meta WHERE id = ?1",
                rusqlite::params![META_ROW_ID],
                |r| r.get(0),
            )
            .context("reading back host identity after minting")
        })
        .await
        .context("host identity mint task panicked")?
    }

    /// Read this host's identity WITHOUT minting one — `Ok(None)` when the
    /// row has never been written, rather than the fresh UUID
    /// `ensure_host_identity` would conjure and persist.
    ///
    /// For the one caller with no standing to mint: a claimless supervisor
    /// (`Supervisor::new_with_seams`, when `StateDirOwnership::claim`
    /// returned `None`) must not perform ANY durable write — see
    /// `Supervisor::host_identity`'s own docs for why minting here would be
    /// exactly that write, smuggled in as a side effect of merely reading a
    /// value for its own hello. A claimless process still reports whatever
    /// identity is already there (so a losing racer's clients see the same
    /// value the eventual owner will), but leaves the row alone — including
    /// leaving it `NULL` — when nothing has been minted yet.
    pub async fn read_host_identity(&self) -> anyhow::Result<Option<String>> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<Option<String>> {
            let conn = conn.lock().expect("session db mutex poisoned");
            let stored: Option<Option<String>> = conn
                .query_row(
                    "SELECT host_identity FROM supervisor_meta WHERE id = ?1",
                    rusqlite::params![META_ROW_ID],
                    |r| r.get(0),
                )
                .optional()
                .context("reading stored host identity")?;
            Ok(stored.flatten())
        })
        .await
        .context("host identity read task panicked")?
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
    /// This is the CREATE-ROLLBACK delete, not the user-facing one. A
    /// permanent reservation is settled `Failed`, preserving M3's replay
    /// contract. A session-lifetime reservation is deleted instead: once
    /// the child row is gone there is no lifetime left to bound it, and a
    /// failed tombstone would otherwise be immortal.
    ///
    /// The reservation change rides the SAME transaction as the removal.
    /// `None` is an unkeyed rollback and touches no reservation.
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
                let pruned = tx
                    .execute(
                        "DELETE FROM create_reservations
                         WHERE intent_key = ?1 AND session_id = ?2
                           AND dedup_scope = 'session_lifetime'",
                        rusqlite::params![settlement.intent_key, settlement.session_id],
                    )
                    .context("pruning a rolled-back bounded reservation")?;
                if pruned == 0 {
                    settle_within(&tx, settlement)?;
                }
            }
            tx.commit().context("committing the launch rollback")?;
            Ok(())
        })
        .await
        .context("session delete task panicked")?
    }

    /// Remove a bounded reservation after its child has disappeared.
    ///
    /// The absence check and deletion are one SQLite statement, closing
    /// the replay-versus-delete race: either the child still exists and
    /// this does nothing, or the key is free before a fresh claim begins.
    pub async fn prune_orphaned_bounded_reservation(
        &self,
        intent_key: &str,
        session_id: &str,
    ) -> anyhow::Result<bool> {
        let conn = Arc::clone(&self.conn);
        let intent_key = intent_key.to_string();
        let session_id = session_id.to_string();
        tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
            let conn = conn.lock().expect("session db mutex poisoned");
            let removed = conn
                .execute(
                    "DELETE FROM create_reservations
                     WHERE intent_key = ?1 AND session_id = ?2
                       AND dedup_scope = 'session_lifetime'
                       AND NOT EXISTS (SELECT 1 FROM sessions WHERE id = ?2)",
                    rusqlite::params![intent_key, session_id],
                )
                .context("pruning an orphaned bounded reservation")?;
            Ok(removed != 0)
        })
        .await
        .context("bounded-reservation prune task panicked")?
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

    /// The whole profiles catalog, ordered by [`farhelm_proto::Profile::id`]
    /// ASCENDING — the order `ControlMsg::ProfileList` promises on the wire
    /// (PLAN_M6_75.md item 4).
    ///
    /// Unpaginated because the catalog is BOUNDED
    /// ([`farhelm_proto::MAX_PROFILES_PER_HOST`], enforced by
    /// [`SessionStore::create_profile`]); see that constant's docs for why a
    /// catalog that could outgrow one reply would be a catalog nobody could
    /// trim back.
    pub async fn profiles(&self) -> anyhow::Result<Vec<farhelm_proto::Profile>> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<farhelm_proto::Profile>> {
            let conn = conn.lock().expect("session db mutex poisoned");
            let mut stmt = conn
                .prepare(&format!(
                    "SELECT {PROFILE_COLUMNS} FROM profiles ORDER BY id ASC"
                ))
                .context("preparing profile list query")?;
            // Two stages for `load_all`'s reason: the fallible decoding
            // cannot happen inside a rusqlite row mapper.
            let raw = stmt
                .query_map([], read_profile_columns)
                .context("querying profiles")?
                .collect::<Result<Vec<_>, rusqlite::Error>>()
                .context("decoding profile rows")?;
            raw.into_iter().map(decode_profile_row).collect()
        })
        .await
        .context("profile list task panicked")?
    }

    /// One profile by id, or `None` when the catalog does not hold it.
    ///
    /// `None` is the load-bearing answer on the create path: it is the
    /// unknown-profile precondition (PLAN_M6_75.md item 4), which fails a
    /// create visibly with no session rather than falling back to some other
    /// profile. A profile can vanish between a client reading the picker and
    /// submitting, so this really is a race and not merely a malformed
    /// request.
    pub async fn profile(&self, id: &str) -> anyhow::Result<Option<farhelm_proto::Profile>> {
        let conn = Arc::clone(&self.conn);
        let id = id.to_string();
        tokio::task::spawn_blocking(move || -> anyhow::Result<Option<farhelm_proto::Profile>> {
            let conn = conn.lock().expect("session db mutex poisoned");
            let raw = conn
                .query_row(
                    &format!("SELECT {PROFILE_COLUMNS} FROM profiles WHERE id = ?1"),
                    rusqlite::params![id],
                    read_profile_columns,
                )
                .optional()
                .context("reading one profile")?;
            raw.map(decode_profile_row).transpose()
        })
        .await
        .context("profile read task panicked")?
    }

    /// Every profile's CURRENT name, keyed by id — the projection
    /// source-profile existence is derived from ([`ProfileNames`]).
    ///
    /// Deliberately not `profiles()` filtered down by the caller: existence
    /// derivation needs only the two columns, runs on every reply that
    /// carries a profile-created session, and has no business paying for the
    /// invocations and templates of a catalog it is not showing.
    pub async fn profile_names(&self) -> anyhow::Result<ProfileNames> {
        #[cfg(test)]
        self.profile_name_reads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<ProfileNames> {
            let conn = conn.lock().expect("session db mutex poisoned");
            let mut stmt = conn
                .prepare("SELECT id, name FROM profiles")
                .context("preparing profile name query")?;
            let names = stmt
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
                .context("querying profile names")?
                .collect::<Result<ProfileNames, rusqlite::Error>>()
                .context("decoding profile names")?;
            Ok(names)
        })
        .await
        .context("profile name read task panicked")?
    }

    /// Store a new profile under a freshly minted id, unless the catalog is
    /// already full (PLAN_M6_75.md item 4).
    ///
    /// The COUNT bound is enforced here rather than by the handler because
    /// only a transaction can enforce it truthfully: a caller that counted
    /// first and inserted after would be reading a number that another
    /// create can invalidate in between, which is exactly how a bound gets
    /// exceeded. The per-record FIELD bound is the handler's — it is a
    /// property of the request alone, needs no view of the catalog, and
    /// belongs where the rest of that request's caller-supplied text is
    /// measured.
    ///
    /// The id is a UUID minted here rather than accepted from the caller,
    /// for `insert_session`'s reason: an id is a reference, and letting a
    /// client choose one lets it collide with (or overwrite) another
    /// profile. Starter profiles are the one exception and they are not
    /// created through this path at all — they are seeded by the schema
    /// ladder with deliberately non-UUID ids (see [`STARTER_PROFILES`]).
    ///
    /// The SEMANTIC rules ([`validate_profile_fields`]) run here rather than
    /// only at the request boundary. The handler checks first so a client
    /// gets an `InvalidRequest` with the exact message, but that check is
    /// not what makes the rule true: every direct caller — a test, a future
    /// import path, a repair tool — would otherwise be able to commit a row
    /// that `ProfileList` cannot render and no create can launch.
    pub async fn create_profile(
        &self,
        name: String,
        invocation: String,
        agent_kind: farhelm_proto::AgentKind,
        resume_template: Option<Vec<String>>,
    ) -> anyhow::Result<ProfileCreation> {
        validate_profile_fields(&name, &invocation, agent_kind, resume_template.as_deref())
            .map_err(|message| anyhow::anyhow!("refusing to store this profile: {message}"))?;
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<ProfileCreation> {
            let mut conn = conn.lock().expect("session db mutex poisoned");
            let tx = conn
                .transaction()
                .context("beginning the profile create transaction")?;
            let held: i64 = tx
                .query_row("SELECT COUNT(*) FROM profiles", [], |r| r.get(0))
                .context("counting profiles")?;
            if held as usize >= farhelm_proto::MAX_PROFILES_PER_HOST {
                // Rolled back by the drop, having written nothing.
                return Ok(ProfileCreation::CatalogFull);
            }
            let profile = farhelm_proto::Profile {
                id: uuid::Uuid::new_v4().to_string(),
                name,
                invocation,
                agent_kind,
                resume_template,
            };
            tx.execute(
                "INSERT INTO profiles (id, name, invocation, agent_kind, resume_template) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    profile.id,
                    profile.name,
                    profile.invocation,
                    agent_kind_column(profile.agent_kind),
                    resume_template_column(profile.resume_template.as_deref()),
                ],
            )
            .context("inserting profile row")?;
            tx.commit().context("committing the profile create")?;
            Ok(ProfileCreation::Created(profile))
        })
        .await
        .context("profile create task panicked")?
    }

    /// Replace a profile's definition wholesale, keyed by its id, and give
    /// back what is now stored — or `None` when no profile with that id
    /// exists.
    ///
    /// A full replacement rather than a patch, matching the wire contract
    /// (`ControlMsg::UpdateProfile`): per-field optionality would make
    /// "clear the resume template" and "leave it alone" the same request.
    /// Last-write-wins with no version token, exactly like a session rename.
    ///
    /// Touches NOTHING else — no session row is read or written here. That
    /// is SPEC.md's snapshot rule holding structurally rather than by
    /// discipline: sessions carry their own launch and resume snapshot and
    /// their own copy of the name they were created under, so an edit
    /// literally has nothing of theirs to disturb.
    ///
    /// Validated exactly as a create is ([`SessionStore::create_profile`]
    /// carries the argument): an update that accepted what a create refuses
    /// would let a bounded catalog be grown past its bound one edit at a
    /// time.
    pub async fn update_profile(
        &self,
        profile: farhelm_proto::Profile,
    ) -> anyhow::Result<Option<farhelm_proto::Profile>> {
        validate_profile_fields(
            &profile.name,
            &profile.invocation,
            profile.agent_kind,
            profile.resume_template.as_deref(),
        )
        .map_err(|message| anyhow::anyhow!("refusing to store this profile: {message}"))?;
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<Option<farhelm_proto::Profile>> {
            let conn = conn.lock().expect("session db mutex poisoned");
            let changed = conn
                .execute(
                    "UPDATE profiles SET name = ?2, invocation = ?3, agent_kind = ?4, \
                     resume_template = ?5 WHERE id = ?1",
                    rusqlite::params![
                        profile.id,
                        profile.name,
                        profile.invocation,
                        agent_kind_column(profile.agent_kind),
                        resume_template_column(profile.resume_template.as_deref()),
                    ],
                )
                .context("updating profile row")?;
            Ok((changed > 0).then_some(profile))
        })
        .await
        .context("profile update task panicked")?
    }

    /// How many catalog reads [`SessionStore::profile_names`] has served —
    /// see that counter's own docs.
    #[cfg(test)]
    pub(crate) fn profile_name_reads(&self) -> u64 {
        self.profile_name_reads
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Make every catalog read FAIL, by removing the table they read.
    ///
    /// Test-only, and the only way to exercise the reply paths' behaviour
    /// when the catalog cannot be read at all. Those paths deliberately
    /// refuse rather than degrade — an empty catalog is indistinguishable
    /// from "every profile was deleted", so a degrading reply would render
    /// a transient database error as a page of sessions whose profiles are
    /// all gone — and a refusal nothing exercises is a refusal nobody knows
    /// still works.
    #[cfg(test)]
    pub(crate) async fn drop_profile_catalog_for_test(&self) {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || {
            conn.lock()
                .expect("session db mutex poisoned")
                .execute_batch("DROP TABLE profiles;")
                .expect("drop the catalog");
        })
        .await
        .expect("catalog drop task panicked");
    }

    /// Store a profile under an id the CALLER chooses, for the one test
    /// that needs an id the catalog once held to exist again.
    ///
    /// Test-only, and deliberately not a variant of
    /// [`SessionStore::create_profile`]: minting the id is what stops a
    /// client from colliding with (or overwriting) another profile, so
    /// production has no business choosing one. The test that needs this is
    /// pinning that a create refused for an unknown profile REPLAYS its
    /// refusal even after a profile with that exact id exists again — a
    /// state that cannot be reached through the real API at all, which is
    /// precisely why the replay rule needs to be pinned rather than assumed.
    #[cfg(test)]
    pub(crate) async fn insert_profile_with_id(
        &self,
        profile: farhelm_proto::Profile,
    ) -> anyhow::Result<()> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let conn = conn.lock().expect("session db mutex poisoned");
            conn.execute(
                "INSERT INTO profiles (id, name, invocation, agent_kind, resume_template) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    profile.id,
                    profile.name,
                    profile.invocation,
                    agent_kind_column(profile.agent_kind),
                    resume_template_column(profile.resume_template.as_deref()),
                ],
            )
            .context("inserting a profile under a chosen id")?;
            Ok(())
        })
        .await
        .context("profile insert task panicked")?
    }

    /// Remove a profile from the catalog. `false` means no profile with that
    /// id was there and nothing was deleted — which the handler reports as
    /// `NotFound` rather than as a silent success, per
    /// `ControlMsg::DeleteProfile`'s own docs.
    ///
    /// Like [`SessionStore::update_profile`], this touches no session row.
    /// Sessions created from the deleted profile keep running and keep their
    /// snapshot; the only thing that changes for them is what a catalog
    /// lookup finds the next time a reply derives their source profile's
    /// existence.
    pub async fn delete_profile(&self, id: &str) -> anyhow::Result<bool> {
        let conn = Arc::clone(&self.conn);
        let id = id.to_string();
        tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
            let conn = conn.lock().expect("session db mutex poisoned");
            let deleted = conn
                .execute("DELETE FROM profiles WHERE id = ?1", rusqlite::params![id])
                .context("deleting profile row")?;
            Ok(deleted > 0)
        })
        .await
        .context("profile delete task panicked")?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Credential strength is measured on the decoded random payload, not
    /// the encoded string length that base64 formatting can inflate.
    #[test]
    fn minted_session_credentials_carry_a_32_byte_random_payload() {
        let token = mint_session_token().expect("mint session credential");
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(token)
            .expect("minted credential is URL-safe base64");
        assert_eq!(payload.len(), 32);
    }

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

    /// The v6 schema, verbatim as `apply_schema`'s version-1-through-6
    /// migrations leave it — the shape immediately BEFORE PLAN_M6.md item
    /// 2's `supervisor_meta.host_identity` column exists. Written out
    /// directly rather than replayed migration-by-migration from
    /// `V1_SCHEMA`: `sessions` and `create_reservations` are already in
    /// their FINAL (v7) shape here because neither table changes between
    /// v6 and v7 — only `supervisor_meta` does, and this constant's whole
    /// job is to omit exactly the one column that migration adds. Kept
    /// independent of `apply_schema`'s own DDL for the same reason
    /// `V1_SCHEMA` is: a fixture built from today's migration code would
    /// silently stop testing that code the moment it changed.
    const V6_SCHEMA: &str = "CREATE TABLE sessions (
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
             ON create_reservations (session_id) WHERE state = 'pending';";

    /// Plant a schema-6 database with `boot_id` already populated — the
    /// shape a real upgrading host has AFTER PLAN_M3.md item 2's boot-id
    /// tracking has run at least once, but before PLAN_M6.md item 2's
    /// identity column exists. `host_identity_v6_to_v7_migration_*` below
    /// needs exactly this POPULATED starting point: every other migration
    /// test in this module starts `supervisor_meta` empty, which cannot
    /// tell "the migration preserves an existing row" apart from "the
    /// migration merely tolerates a table with no rows at all".
    fn plant_v6_database(path: &Path, boot_id: &str) {
        let conn = Connection::open(path).expect("create raw db");
        conn.execute_batch(V6_SCHEMA).expect("v6 schema");
        conn.execute(
            "INSERT INTO supervisor_meta (id, boot_id) VALUES (0, ?1)",
            rusqlite::params![boot_id],
        )
        .expect("insert v6 supervisor_meta row");
        conn.pragma_update(None, "user_version", 6).expect("stamp");
    }

    /// Plant a schema-7 database holding one session — the shape a host
    /// running the build immediately BEFORE PLAN_M6_75.md item 4 has.
    ///
    /// Built from [`V6_SCHEMA`] plus exactly the one column version 7 adds,
    /// rather than by replaying `apply_schema`: a fixture built from
    /// today's migration code stops testing that code the moment it
    /// changes, which is the same reason `V1_SCHEMA` and `V6_SCHEMA` are
    /// written out by hand.
    ///
    /// The session row is what makes the migration's DATA promise testable:
    /// version 8 adds two nullable columns to a table that already has
    /// rows, and a rebuild-and-copy that dropped or transposed a column
    /// would still pass a schema comparison.
    fn plant_v7_database(path: &Path, session_id: &str) {
        let conn = Connection::open(path).expect("create raw db");
        conn.execute_batch(V6_SCHEMA).expect("v6 schema");
        conn.execute_batch("ALTER TABLE supervisor_meta ADD COLUMN host_identity TEXT;")
            .expect("v7 column");
        conn.execute(
            "INSERT INTO sessions (id, title, cwd, invocation, tmux_name, pane, created_at, \
             outcome_state, agent_kind, resume_template) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'running', 'claude', ?8)",
            rusqlite::params![
                session_id,
                format!("title-{session_id}"),
                "/work",
                "claude",
                format!("fh-{session_id}"),
                "%0",
                1_700_000_000_i64,
                // An integrated kind must carry a placeholder-bearing
                // template or the row is refused at load — the same
                // invariant a create enforces, so the fixture has to
                // satisfy it to be a realistic pre-migration row at all.
                r#"["claude","--resume","{conversation}"]"#,
            ],
        )
        .expect("insert v7 session row");
        conn.pragma_update(None, "user_version", 7).expect("stamp");
    }

    /// Plant the last pre-spawn schema with two credential-less sessions.
    ///
    /// Written from the older schema fixtures and the historical DDL, not
    /// from today's migration, so a credential-minting regression cannot
    /// accidentally update both the implementation and its starting point.
    fn plant_v9_database(path: &Path) {
        let conn = Connection::open(path).expect("create raw db");
        conn.execute_batch(V6_SCHEMA).expect("v6 schema");
        conn.execute_batch(
            "ALTER TABLE supervisor_meta ADD COLUMN host_identity TEXT;
             ALTER TABLE sessions ADD COLUMN source_profile_id TEXT;
             ALTER TABLE sessions ADD COLUMN source_profile_name TEXT;
             ALTER TABLE create_reservations
                 ADD COLUMN dedup_scope TEXT NOT NULL DEFAULT 'permanent';",
        )
        .expect("v7 through v9 columns");
        conn.execute_batch(PROFILES_SCHEMA)
            .expect("v8 profile catalog");
        for (index, id) in ["old-a", "old-b"].into_iter().enumerate() {
            conn.execute(
                "INSERT INTO sessions (id, title, cwd, invocation, tmux_name, pane, created_at, \
                 outcome_state, agent_kind) \
                 VALUES (?1, ?1, '/work', 'agent', ?2, ?3, ?4, 'running', 'generic')",
                rusqlite::params![
                    id,
                    format!("fh-{id}"),
                    format!("%{index}"),
                    1_700_000_000_i64 + index as i64,
                ],
            )
            .expect("insert v9 session");
        }
        conn.pragma_update(None, "user_version", 9).expect("stamp");
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
                    parent: None,
                    archived: false,
                    title: id.to_string(),
                    created_at: now_unix(),
                    creation_seq: 0,
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
                    source_profile: None,
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

    /// Upgrading a running pre-spawn installation gives every existing
    /// session a distinct, durable credential inside the migration itself.
    #[tokio::test]
    async fn schema_9_migration_mints_stable_credentials_for_every_existing_session() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("supervisor.db");
        plant_v9_database(&db_path);

        let store = SessionStore::open(&db_path, true).await.expect("migrate");
        let a = store
            .session_token("old-a")
            .await
            .expect("read token")
            .expect("old-a received a token");
        let b = store
            .session_token("old-b")
            .await
            .expect("read token")
            .expect("old-b received a token");
        assert!(!a.is_empty() && !b.is_empty());
        assert_ne!(
            a, b,
            "credentials are per session, not one migration secret"
        );
        assert!(store.authenticates_session("old-a", &a).await.unwrap());
        drop(store);

        let reopened = SessionStore::open(&db_path, true).await.expect("reopen");
        let mut migrated = reopened.load_all().await.unwrap();
        migrated.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(
            migrated
                .iter()
                .map(|row| row.creation_seq)
                .collect::<Vec<_>>(),
            vec![1, 2],
            "migration preserves the former timestamp/id chronology"
        );
        assert_eq!(
            reopened.session_token("old-a").await.unwrap().as_deref(),
            Some(a.as_str()),
            "a relaunch reuses the durable credential"
        );
        assert!(
            migrated.iter().all(|row| row.parent.is_none()),
            "pre-spawn rows gain no invented parent"
        );
    }

    /// Every pre-v11 reservation was interactive and therefore permanent.
    /// The 8-to-9 migration must record that fact rather than inventing a
    /// bounded lifetime for a key that was originally promised forever.
    #[tokio::test]
    async fn schema_8_reservations_migrate_to_permanent_dedup_scope() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("supervisor.db");
        plant_v7_database(&db_path, "s1");
        let conn = Connection::open(&db_path).expect("open v7 fixture");
        conn.execute_batch(&format!(
            "ALTER TABLE sessions ADD COLUMN source_profile_id TEXT;
             ALTER TABLE sessions ADD COLUMN source_profile_name TEXT;
             {PROFILES_SCHEMA}
             {STARTER_PROFILES}
             PRAGMA user_version = 8;"
        ))
        .expect("v8 schema");
        conn.execute(
            "INSERT INTO create_reservations
             (intent_key, fingerprint, state, session_id, tmux_name, created_at)
             VALUES ('old-key', 'fp', 'created', 's1', 'fh-s1', 1700000000)",
            [],
        )
        .expect("insert v8 reservation");
        drop(conn);

        let store = SessionStore::open(&db_path, true)
            .await
            .expect("migrate v8 reservation");
        let reservation = store
            .reservation("old-key")
            .await
            .expect("read migrated reservation")
            .expect("the migration must preserve the row");
        assert_eq!(reservation.dedup_scope, DedupScope::Permanent);
    }

    /// The v6-to-v7 migration (PLAN_M6.md item 2's `host_identity` column)
    /// against a database that ALREADY HAS `supervisor_meta` data — every
    /// other migration test in this module starts that table empty
    /// (`plant_v1_database` never rows it), which cannot distinguish
    /// "preserves an existing row" from "merely tolerates zero rows".
    /// `plant_v6_database` seeds a boot id, matching a real host that has
    /// been through at least one PLAN_M3.md item 2 reboot check before this
    /// build's identity column ever existed.
    ///
    /// Four things must all hold once the migrating `open` and a following
    /// mint have run: the schema actually reached version 7 (not silently
    /// stuck partway); the pre-existing `boot_id` survived untouched (the
    /// migration only ADDS a column — see `apply_schema`'s own version-6
    /// entry for why it deliberately backfills nothing); `ensure_host_
    /// identity` can mint into the now-existing column on a migrated row,
    /// not just a freshly created one; and that minted value is DURABLE —
    /// a reopen must read the same identity back, not remint (mirroring
    /// `a_migrated_database_reopens_at_the_new_version`'s own durability
    /// check, extended to the identity this migration's column exists
    /// for).
    #[tokio::test]
    async fn host_identity_v6_to_v7_migration_preserves_boot_id_and_mints_cleanly() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("supervisor.db");
        plant_v6_database(&db_path, "boot-abc-123");

        let migrated = SessionStore::open(&db_path, true)
            .await
            .expect("migrating open");
        let version: i64 = {
            let conn = migrated.conn.lock().expect("db mutex");
            conn.query_row("PRAGMA user_version", [], |r| r.get(0))
                .expect("read version")
        };
        assert_eq!(
            version, SCHEMA_VERSION,
            "the ladder must run all the way to the current version, not stall at v6"
        );
        assert_eq!(
            migrated.boot_id().await.expect("boot id"),
            Some("boot-abc-123".to_string()),
            "the pre-existing boot id must survive a migration that only ADDS a column"
        );

        let minted = migrated
            .ensure_host_identity()
            .await
            .expect("mint into the migrated row");
        uuid::Uuid::parse_str(&minted).expect("minted identity must be a real UUID");
        drop(migrated);

        let reopened = SessionStore::open(&db_path, true).await.expect("reopen");
        assert_eq!(
            reopened.boot_id().await.expect("boot id after reopen"),
            Some("boot-abc-123".to_string()),
            "the boot id must still be there after a reopen, not just immediately post-migration"
        );
        assert_eq!(
            reopened
                .read_host_identity()
                .await
                .expect("read back after reopen"),
            Some(minted),
            "the minted identity must be durable, not reminted on the next open"
        );
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
    ///
    /// One piece of DATA is compared too, and only one: the starter
    /// profiles (PLAN_M6_75.md item 4). They are seeded by the same schema
    /// step on both paths, so a fresh install and an upgraded host must
    /// both come up with the same catalog — an upgrade that skipped the
    /// seed would leave a long-running host as the only one where SPEC.md's
    /// "a fresh supervisor is not empty" quietly does not hold, and no
    /// schema comparison would notice. This is also why the starter ids are
    /// FIXED rather than minted (see `STARTER_PROFILES`): the two databases
    /// can be compared row for row.
    #[tokio::test]
    async fn migrated_and_fresh_schemas_agree() {
        let dir = tempfile::tempdir().expect("tempdir");
        let migrated = dir.path().join("migrated.db");
        plant_v1_database(&migrated, &[]);
        let migrated_store = SessionStore::open(&migrated, true).await.expect("migrate");
        let fresh = dir.path().join("fresh.db");
        let fresh_store = SessionStore::open(&fresh, true).await.expect("create");

        assert_eq!(columns_of(&migrated), columns_of(&fresh));
        assert_eq!(
            migrated_store.profiles().await.expect("migrated catalog"),
            fresh_store.profiles().await.expect("fresh catalog"),
            "both paths seed the same starter catalog, or an upgraded host is the only one \
             without one"
        );
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
                    parent: None,
                    archived: false,
                    title: "demo".to_string(),
                    created_at: now_unix(),
                    creation_seq: 0,
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
                    source_profile: None,
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
                    parent: None,
                    archived: false,
                    created_at: now_unix(),
                    creation_seq: 0,
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
                    source_profile: None,
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
                    parent: Some("parent-7".to_string()),
                    archived: false,
                    title: "demo".to_string(),
                    created_at: now_unix(),
                    creation_seq: 0,
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
                    source_profile: None,
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
        assert_eq!(rows[0].parent.as_deref(), Some("parent-7"));
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

    /// A fresh database has no identity until asked for one — pinning that
    /// `ensure_host_identity` is what actually mints it, not `open` or
    /// `apply_schema` on their own (a reader who only skimmed the schema
    /// migration could otherwise assume the column is populated the moment
    /// the table exists).
    #[tokio::test]
    async fn host_identity_is_null_until_ensure_host_identity_is_called() {
        let (_dir, store) = fresh_store().await;
        let conn = Arc::clone(&store.conn);
        let stored: Option<Option<String>> = tokio::task::spawn_blocking(move || {
            conn.lock()
                .unwrap()
                .query_row(
                    "SELECT host_identity FROM supervisor_meta WHERE id = ?1",
                    rusqlite::params![META_ROW_ID],
                    |r| r.get(0),
                )
                .optional()
                .unwrap()
        })
        .await
        .unwrap();
        assert_eq!(
            stored.flatten(),
            None,
            "a database nobody has minted an identity for must report no row, or a NULL \
             column — never a value this test never asked for"
        );
    }

    /// SPEC.md's core promise for this identity: minted once, and stable
    /// for the life of the install — pinned across a full close-and-reopen
    /// of the database file, `ensure_host_identity`'s durable-persistence
    /// path. (An EARLIER version of this test also repeated the call
    /// against the same still-open store before reopening; dropped as
    /// redundant — that repeat exercises the identical read-then-
    /// conditional-write query this reopened call already runs, just
    /// without the intervening file close, so it added no coverage this
    /// reopen check does not already provide. In-process concurrent
    /// minting is `concurrent_first_mint_converges_on_one_identity`'s job,
    /// below — a genuine second code path, unlike a second sequential
    /// call.)
    #[tokio::test]
    async fn host_identity_is_minted_once_and_stable_across_reopen() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("supervisor.db");

        let store = SessionStore::open(&db_path, true).await.expect("open");
        let first = store
            .ensure_host_identity()
            .await
            .expect("mint on first read");
        uuid::Uuid::parse_str(&first).expect("minted identity must be a real UUID");
        drop(store);

        let reopened = SessionStore::open(&db_path, true).await.expect("reopen");
        let after_reopen = reopened
            .ensure_host_identity()
            .await
            .expect("read the durable identity back");
        assert_eq!(
            first, after_reopen,
            "the identity must survive a full close and reopen of the database file — the \
             boot_id-migration precedent this column follows persists exactly the same way"
        );
    }

    /// The race the sequential tests above cannot reach: TWO independent
    /// `SessionStore`s (separate `rusqlite::Connection`s, so a genuine
    /// second real writer — not one connection called twice) opened
    /// against the SAME fresh database, both calling `ensure_host_identity`
    /// for the first time. An unconditional upsert (dropping the `WHERE
    /// host_identity IS NULL` guard `ensure_host_identity`'s own docs
    /// describe) would let each racer's INSERT blindly clobber whatever
    /// the other had just written, with no guarantee either racer's own
    /// return value still matches what ends up durably persisted.
    ///
    /// Deterministic despite racing: this does NOT depend on which
    /// `spawn_blocking` task actually reaches SQLite first (there is no
    /// barrier forcing that, and none is needed) — the invariant under
    /// test is that BOTH racers converge on ONE identity and the durable
    /// row agrees, and the conditional `WHERE` guard plus the trailing
    /// re-`SELECT` (`ensure_host_identity`'s own docs) make that hold
    /// under EVERY possible interleaving, not just one particular
    /// ordering. That is what makes this safe to run un-seeded rather than
    /// needing a timing-dependent construction — nothing here would ever
    /// become flaky from scheduling variance.
    #[tokio::test]
    async fn concurrent_first_mint_converges_on_one_identity() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("supervisor.db");
        let store_a = SessionStore::open(&db_path, true).await.expect("open a");
        let store_b = SessionStore::open(&db_path, true).await.expect("open b");

        let (id_a, id_b) = tokio::join!(
            store_a.ensure_host_identity(),
            store_b.ensure_host_identity()
        );
        let id_a = id_a.expect("racer a mints or reads back the winner");
        let id_b = id_b.expect("racer b mints or reads back the winner");
        assert_eq!(
            id_a, id_b,
            "two racing first-mint calls must converge on ONE identity, never two different ones"
        );
        uuid::Uuid::parse_str(&id_a).expect("the converged identity must be a real UUID");

        let persisted = store_a
            .read_host_identity()
            .await
            .expect("reading back the persisted identity")
            .expect("a row must exist once either racer's write has landed");
        assert_eq!(
            persisted, id_a,
            "the durable row must hold the SAME identity both racers converged on, \
             not a value only one of them ever returned"
        );
    }

    /// The other half of SPEC.md's reinstall semantics: wiping the state
    /// directory must NOT be distinguishable from "reuse the old
    /// identity" — a fresh database mints its own, independent of what a
    /// prior install at the SAME path ever minted.
    ///
    /// The wipe reuses one path rather than comparing two independently
    /// `tempfile::tempdir()`-ed stores: two different paths would pass
    /// this assertion even if minting secretly derived identity from the
    /// state dir path (a bug this test exists to rule out), since
    /// different paths trivially mint different identities either way.
    /// Deleting and recreating the SAME directory is what actually
    /// exercises "wipe", forcing the durable row itself — not anything
    /// path-keyed — to be the only source of truth for whether minting
    /// already happened (`ensure_host_identity`'s own docs).
    #[tokio::test]
    async fn a_wiped_state_dir_mints_a_different_host_identity() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().to_path_buf();
        let db_path = path.join("supervisor.db");

        let store_a = SessionStore::open(&db_path, true).await.expect("open a");
        let id_a = store_a.ensure_host_identity().await.expect("mint a");
        drop(store_a);

        std::fs::remove_dir_all(&path).expect("wipe the state dir");
        std::fs::create_dir(&path).expect("recreate the state dir at the same path");

        let store_b = SessionStore::open(&db_path, true).await.expect("open b");
        let id_b = store_b.ensure_host_identity().await.expect("mint b");

        assert_ne!(
            id_a, id_b,
            "a real wipe-and-reinstall at the SAME path must mint a fresh identity, \
             never resurrect the one the wiped database held"
        );
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

    /// `created_at` (PLAN_M6.md item 1: load-bearing as of this PR, no
    /// longer the write-only column `StoredSession`'s docs used to
    /// describe) is written VERBATIM from what the caller put on the row,
    /// never re-derived from SQLite's own clock. A fixed sentinel — not
    /// `now_unix()`, unlike every other fixture in this module — is what
    /// proves that: bracketing the insert with `before`/`after` reads of
    /// the wall clock (as an earlier version of this test did) can only
    /// ever show the store's OWN minting agrees with itself, since both
    /// the fixture and the column would be reading the same clock at
    /// roughly the same instant either way. Pinning a value nowhere near
    /// "now" and asserting it comes back unchanged is the only way to
    /// rule out the store silently substituting its own `now_unix()` for
    /// whatever the caller passed. Two read paths are checked against the
    /// sentinel: the raw SQLite column, and a fresh `load_all()` of the
    /// same row — the latter catches a future `read_session_columns`
    /// index drift (see [`SESSION_COLUMNS`]'s own docs on why that would
    /// otherwise fail silently) that the raw-column check alone could not.
    #[tokio::test]
    async fn insert_session_persists_created_at_verbatim() {
        let (_dir, store) = fresh_store().await;

        // Deliberately far from "now": a value this test's own clock
        // could never produce by accident is what makes a passing
        // assertion mean the store passed the caller's value through
        // unchanged, not merely that two clock reads landed close together.
        const SENTINEL_CREATED_AT: i64 = 1_000_000_000;
        store
            .insert_session(
                StoredSession {
                    id: "s1".to_string(),
                    parent: None,
                    archived: false,
                    title: "s1".to_string(),
                    created_at: SENTINEL_CREATED_AT,
                    creation_seq: 0,
                    cwd: "/tmp/work".to_string(),
                    invocation: "agent".to_string(),
                    tmux_name: "fh-s1".to_string(),
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
                    launch_scoped: false,
                    source_profile: None,
                },
                None,
            )
            .await
            .expect("insert");

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
        assert_eq!(
            created_at, SENTINEL_CREATED_AT,
            "the raw column must carry the caller's value verbatim, not a re-minted one"
        );

        let rows = store.load_all().await.expect("load");
        assert_eq!(
            rows[0].created_at, SENTINEL_CREATED_AT,
            "StoredSession::created_at must agree with the raw column it was read from"
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
            parent: None,
            archived: false,
            title: id.to_string(),
            created_at: now_unix(),
            creation_seq: 0,
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
            source_profile: None,
        }
    }

    /// Seed one reserved launch: a launching session row plus the pending
    /// claim that reserved it, exactly as `create_session`'s first step
    /// commits them.
    async fn insert_reserved(store: &SessionStore, id: &str, key: &str, fingerprint: &str) {
        insert_reserved_with_scope(store, id, key, fingerprint, DedupScope::Permanent).await;
    }

    /// [`insert_reserved`] with an explicit scope for PLAN_M7.md item 2's
    /// storage tests. Production callers derive this value from their
    /// connection rather than accepting it from the wire.
    async fn insert_reserved_with_scope(
        store: &SessionStore,
        id: &str,
        key: &str,
        fingerprint: &str,
        dedup_scope: DedupScope,
    ) {
        let claimed = store
            .insert_session(
                launching_row(id),
                Some(IntentClaim {
                    intent_key: key.to_string(),
                    fingerprint: fingerprint.to_string(),
                    dedup_scope,
                }),
            )
            .await
            .expect("insert with claim");
        assert!(
            matches!(claimed, Claimed::Ours { .. }),
            "the key {key} must have been free"
        );
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
                farhelm_proto::ErrorKind::Unauthorized,
            ]
            .into_iter()
            .zip(["failed-0", "failed-1", "failed-2", "failed-3", "failed-4"])
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
            assert_eq!(read.dedup_scope, DedupScope::Permanent);
        }
        assert_eq!(
            store.reservation("never-claimed").await.expect("read"),
            None,
            "a key this state directory has never seen has no reservation"
        );
    }

    /// Both deduplication windows survive SQLite unchanged.
    #[tokio::test]
    async fn reservation_dedup_scopes_round_trip() {
        let (_dir, store) = fresh_store().await;
        insert_reserved_with_scope(&store, "s1", "permanent", "fp", DedupScope::Permanent).await;
        insert_reserved_with_scope(&store, "s2", "session", "fp", DedupScope::SessionLifetime)
            .await;

        assert_eq!(
            reservation_of(&store, "permanent").await.dedup_scope,
            DedupScope::Permanent
        );
        assert_eq!(
            reservation_of(&store, "session").await.dedup_scope,
            DedupScope::SessionLifetime
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
                    dedup_scope: DedupScope::Permanent,
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
    /// The non-default scope is part of that work item: losing it here would
    /// turn a bounded spawn key into a permanent tombstone during recovery.
    #[tokio::test]
    async fn pending_reservations_lists_only_the_unsettled_ones() {
        let (_dir, store) = fresh_store().await;
        insert_reserved_with_scope(
            &store,
            "s1",
            "still-pending",
            "fp",
            DedupScope::SessionLifetime,
        )
        .await;
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
        assert_eq!(pending[0].dedup_scope, DedupScope::SessionLifetime);
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

    /// A spawned child's key and credential have exactly the child's row's
    /// lifetime, while an interactive key remains a durable tombstone.
    #[tokio::test]
    async fn deleting_a_spawned_child_prunes_its_bounded_key_and_credential() {
        let (_dir, store) = fresh_store().await;
        insert_reserved_with_scope(
            &store,
            "spawn-child",
            "spawn-key",
            "fp",
            DedupScope::SessionLifetime,
        )
        .await;
        insert_reserved(&store, "interactive", "interactive-key", "fp").await;
        let token = store
            .session_token("spawn-child")
            .await
            .unwrap()
            .expect("every inserted session receives a credential");
        assert!(
            store
                .authenticates_session("spawn-child", &token)
                .await
                .unwrap()
        );

        store
            .delete_session_settling_reservations("spawn-child")
            .await
            .expect("delete child");
        assert_eq!(store.reservation("spawn-key").await.unwrap(), None);
        assert_eq!(store.session_token("spawn-child").await.unwrap(), None);
        assert!(
            !store
                .authenticates_session("spawn-child", &token)
                .await
                .unwrap(),
            "a deleted session can no longer authenticate"
        );
        assert!(
            store
                .reservation("interactive-key")
                .await
                .unwrap()
                .is_some(),
            "another session's permanent key is untouched"
        );
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
        insert_reserved_with_scope(&store, "s3", "bounded", "fp", DedupScope::SessionLifetime)
            .await;

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
        store
            .delete_session(
                "s3",
                Some(settlement(
                    "bounded",
                    "s3",
                    ReservationOutcome::Failed {
                        kind: farhelm_proto::ErrorKind::Internal,
                        message: "launch failed".to_string(),
                    },
                )),
            )
            .await
            .expect("rollback bounded launch");

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
        assert_eq!(store.reservation("bounded").await.unwrap(), None);
        assert!(store.load_all().await.expect("load").is_empty());
    }

    /// A refused intent is recorded with no session row at all — the shape
    /// a create rejected by validation leaves behind. A bounded spawn has
    /// no child lifetime to bind this reservation to, so it must leave no
    /// claim; permanent interactive refusals retain their durable replay.
    #[tokio::test]
    async fn a_refused_intent_records_without_a_session_row() {
        let (_dir, store) = fresh_store().await;
        store
            .record_failed_intent(
                IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint: "fp".to_string(),
                    dedup_scope: DedupScope::SessionLifetime,
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
        assert_eq!(store.reservation("key").await.unwrap(), None);

        store
            .record_failed_intent(
                IntentClaim {
                    intent_key: "permanent-failure".to_string(),
                    fingerprint: "fp".to_string(),
                    dedup_scope: DedupScope::Permanent,
                },
                "never-created",
                "fh-never-created",
                farhelm_proto::ErrorKind::InvalidRequest,
                "working directory does not exist: /nope",
            )
            .await
            .expect("record permanent failure");
        assert!(matches!(
            reservation_of(&store, "permanent-failure").await.outcome,
            ReservationOutcome::Failed { .. }
        ));

        // A key someone else claimed in the meantime is left alone: a
        // refusal must never overwrite a live claim.
        insert_reserved(&store, "s2", "live", "fp").await;
        store
            .record_failed_intent(
                IntentClaim {
                    intent_key: "live".to_string(),
                    fingerprint: "fp".to_string(),
                    dedup_scope: DedupScope::Permanent,
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

        // Acquired: pending reservation, launching row. `created_at` is not
        // pinned here — `restart_pending_launch_preserves_created_at_across_
        // a_retry` below is where that value earns its own scrutiny; this
        // test only needs to know a takeover happened at all.
        insert_reserved(&store, "s1", "acquire", "fp").await;
        let original_token = store
            .session_token("s1")
            .await
            .unwrap()
            .expect("insert minted a token");
        let RetryClaim::Acquired { session_token, .. } = store
            .restart_pending_launch(launching_row("s1"), "acquire")
            .await
            .expect("takeover")
        else {
            panic!("the pending launch should be acquired");
        };
        assert_eq!(session_token.as_bytes(), original_token.as_bytes());
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
        assert!(matches!(
            store
                .restart_pending_launch(launching_row("s4"), "rebooted")
                .await
                .expect("takeover"),
            RetryClaim::Acquired { .. }
        ));
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

    /// PLAN_M6.md's pagination promises a total order over STABLE columns
    /// (`PROTOCOL_VERSION`'s version-8 history), and `created_at` is the
    /// primary one — a retry that re-minted it would silently move a
    /// session within that order. This matters even though the crashed
    /// attempt's own reply never shipped: `service::Supervisor::
    /// reload_sessions` loads and lists every stored row unconditionally,
    /// `Launching` included, so a supervisor restart between the crash and
    /// the retry can already have served the ORIGINAL timestamp through a
    /// real `ListSessions` reply before this takeover ever runs (see
    /// `StoredSession::created_at`'s docs for the full argument). The
    /// retry's own `row` here is built with a DELIBERATELY different
    /// `created_at` from the original, so a test that silently re-minted —
    /// rather than preserved — would fail loudly instead of by
    /// coincidentally matching.
    #[tokio::test]
    async fn restart_pending_launch_preserves_created_at_across_a_retry() {
        let (_dir, store) = fresh_store().await;

        const ORIGINAL_CREATED_AT: i64 = 1_000_000_000;
        const RETRY_CREATED_AT: i64 = 2_000_000_000;

        store
            .insert_session(
                StoredSession {
                    created_at: ORIGINAL_CREATED_AT,
                    creation_seq: 0,
                    ..launching_row("s1")
                },
                Some(IntentClaim {
                    intent_key: "retry".to_string(),
                    fingerprint: "fp".to_string(),
                    dedup_scope: DedupScope::Permanent,
                }),
            )
            .await
            .expect("insert with claim");

        let claim = store
            .restart_pending_launch(
                StoredSession {
                    created_at: RETRY_CREATED_AT,
                    creation_seq: 0,
                    ..launching_row("s1")
                },
                "retry",
            )
            .await
            .expect("takeover");
        let RetryClaim::Acquired { created_at, .. } = claim else {
            panic!("expected the takeover to acquire: {claim:?}");
        };
        assert_eq!(
            created_at, ORIGINAL_CREATED_AT,
            "the reply must carry the crashed attempt's ORIGINAL timestamp, not the retry's own"
        );

        let row = store
            .session("s1")
            .await
            .expect("read")
            .expect("the takeover leaves a row under the same id");
        assert_eq!(
            row.created_at, ORIGINAL_CREATED_AT,
            "the durable row after the takeover must still carry the original timestamp"
        );
    }

    /// A reservation row this build cannot honestly decode is refused, not
    /// guessed at — same stance as `load_all`'s, and for a sharper reason:
    /// a guessed outcome here either replays a success that never happened
    /// or launches a duplicate.
    ///
    /// The matrix covers every way a `failed` row can be incomplete and an
    /// unknown deduplication scope. Each has a tempting default (`Internal`,
    /// an empty message, or `Permanent`), and every one would fabricate a
    /// policy the row did not state.
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
            (
                "UPDATE create_reservations SET dedup_scope = 'ephemeral'",
                "ephemeral",
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
                    parent: None,
                    archived: false,
                    title: "t".to_string(),
                    created_at: now_unix(),
                    creation_seq: 0,
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
                    source_profile: None,
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

    /// Archive keeps the row, records the deliberate stopped outcome, and
    /// is idempotent. Restart clears the flag as part of opening the next
    /// generation, while an aborted restart restores it with the prior run.
    #[tokio::test]
    async fn archive_is_durable_idempotent_and_restored_by_an_aborted_restart() {
        let (_dir, store) = fresh_store().await;
        insert_running(&store, "s1").await;

        assert_eq!(store.archive_session("s1").await.unwrap(), Some(true));
        assert_eq!(store.archive_session("s1").await.unwrap(), Some(false));
        let archived = store.session("s1").await.unwrap().unwrap();
        assert!(archived.archived);
        assert_eq!(archived.title, "s1", "session metadata survives archive");
        assert_eq!(archived.pane, "", "archive removes the terminal handle");
        assert_eq!(
            archived.outcome,
            LastOutcome::Exited {
                exit_code: None,
                annotation: Some(farhelm_proto::STOP_ANNOTATION.to_string()),
            }
        );

        let claim = claimed(
            store
                .begin_relaunch("s1", uncaptured_basis(), true, false)
                .await
                .unwrap(),
        );
        assert!(claim.prior.archived);
        assert!(!store.session("s1").await.unwrap().unwrap().archived);

        assert!(
            store
                .abort_relaunch("s1", claim.generation, &claim.prior)
                .await
                .unwrap()
        );
        let restored = store.session("s1").await.unwrap().unwrap();
        assert!(restored.archived);
        assert_eq!(restored.outcome, archived.outcome);
        assert_eq!(
            crate::service::recovered_archive_flag(true, claim.prior.archived),
            restored.archived,
            "definitive recovery must agree in memory and SQLite"
        );

        let ambiguous = claimed(
            store
                .begin_relaunch("s1", uncaptured_basis(), true, false)
                .await
                .unwrap(),
        );
        let durable = store.session("s1").await.unwrap().unwrap();
        assert!(!durable.archived);
        assert_eq!(
            crate::service::recovered_archive_flag(false, ambiguous.prior.archived),
            durable.archived,
            "ambiguous recovery must keep both representations visible"
        );
    }

    /// Once archive commits, a delayed observation from the retired pane
    /// cannot repopulate either its terminal handle or its prior outcome.
    #[tokio::test]
    async fn archive_fences_a_stale_outcome_observation() {
        let (_dir, store) = fresh_store().await;
        insert_running(&store, "s1").await;
        store.archive_session("s1").await.unwrap();

        let committed = store
            .transition(
                "s1",
                0,
                Transition::RediscoveredExit {
                    pane: "%late".to_string(),
                    exit_code: Some(17),
                },
            )
            .await
            .unwrap()
            .unwrap();
        let row = store.session("s1").await.unwrap().unwrap();
        assert!(row.archived);
        assert_eq!(row.pane, "");
        assert_eq!(committed, row.outcome);
        assert_eq!(
            row.outcome,
            LastOutcome::Exited {
                exit_code: None,
                annotation: Some(farhelm_proto::STOP_ANNOTATION.to_string()),
            }
        );
    }

    /// The v11-to-v12 migration preserves every preexisting row and gives
    /// each one the only truthful historical value: it was not archived.
    #[tokio::test]
    async fn schema_11_rows_migrate_as_unarchived() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("supervisor.db");
        let store = SessionStore::open(&db_path, true)
            .await
            .expect("create current db");
        insert_running(&store, "s1").await;
        drop(store);

        let conn = Connection::open(&db_path).expect("open fixture");
        conn.execute_batch(
            "ALTER TABLE sessions DROP COLUMN archived;
             PRAGMA user_version = 11;",
        )
        .expect("downgrade the fixture to the pre-archive schema");
        drop(conn);

        let migrated = SessionStore::open(&db_path, true)
            .await
            .expect("migrate v11");
        let row = migrated.session("s1").await.unwrap().unwrap();
        assert!(!row.archived);
        assert_eq!(row.title, "s1");
        assert_eq!(row.pane, "%0");
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

    /// SPEC.md's "a fresh supervisor is not empty", spelled out: the exact
    /// starter catalog a brand-new database comes up with (PLAN_M6_75.md
    /// item 4).
    ///
    /// Pinned field by field rather than counted, because each field is a
    /// product decision somebody could plausibly "tidy" into something
    /// wrong: the KINDS are what select conversation capture and per-kind
    /// status sharpening at all (a starter that landed as `Generic` would
    /// silently ship two profiles with no integration), and the NULL resume
    /// templates are the deliberate "let the kind supply its default"
    /// spelling rather than an omission — see `STARTER_PROFILES` for why
    /// materializing them here would fork each integration's default.
    #[tokio::test]
    async fn a_fresh_database_ships_the_two_starter_profiles() {
        let (_dir, store) = fresh_store().await;
        let profiles = store.profiles().await.expect("catalog");
        assert_eq!(
            profiles,
            vec![
                farhelm_proto::Profile {
                    id: "starter-claude".to_string(),
                    name: "Claude Code".to_string(),
                    invocation: "claude".to_string(),
                    agent_kind: farhelm_proto::AgentKind::Claude,
                    resume_template: None,
                },
                farhelm_proto::Profile {
                    id: "starter-codex".to_string(),
                    name: "Codex".to_string(),
                    invocation: "codex".to_string(),
                    agent_kind: farhelm_proto::AgentKind::Codex,
                    resume_template: None,
                },
            ],
            "a fresh supervisor ships Claude Code and Codex, in id order"
        );
    }

    /// The seeding contract that is easiest to break and worst to break:
    /// starters are seeded ONCE, so a starter the user DELETED stays
    /// deleted across every later start of the supervisor (PLAN_M6_75.md
    /// item 4).
    ///
    /// A re-seed would be maximally annoying rather than merely wrong — the
    /// profile the user removed would come back on every restart, forever,
    /// with no way for them to make it stop. This is exactly what a
    /// startup-time "is the catalog seeded?" check gets wrong when its flag
    /// and its table disagree, and why the seed rides the schema ladder
    /// instead (see `STARTER_PROFILES`).
    ///
    /// Also asserts an EDITED starter survives, which is the same rule seen
    /// from the other side: an idempotent re-seed that "restored" the
    /// original definition would quietly discard the user's edit.
    #[tokio::test]
    async fn a_deleted_starter_stays_deleted_and_an_edited_one_stays_edited() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("supervisor.db");
        let store = SessionStore::open(&path, true).await.expect("open");
        assert!(
            store
                .delete_profile("starter-claude")
                .await
                .expect("delete")
        );
        store
            .update_profile(farhelm_proto::Profile {
                id: "starter-codex".to_string(),
                name: "Codex (mine)".to_string(),
                invocation: "codex --search".to_string(),
                agent_kind: farhelm_proto::AgentKind::Codex,
                resume_template: None,
            })
            .await
            .expect("update")
            .expect("the starter is there to edit");
        drop(store);

        let reopened = SessionStore::open(&path, true).await.expect("reopen");
        let profiles = reopened.profiles().await.expect("catalog");
        assert_eq!(
            profiles.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
            vec!["starter-codex"],
            "a deleted starter must not be re-seeded by a later start"
        );
        assert_eq!(profiles[0].name, "Codex (mine)");
        assert_eq!(profiles[0].invocation, "codex --search");
    }

    /// The catalog's CRUD contract end to end, through the on-disk round
    /// trip: what a create stores is what a read gives back, an update
    /// replaces wholesale, a delete removes, and both mutating verbs report
    /// honestly when the id is not there.
    ///
    /// The id-ascending list order is asserted rather than assumed because
    /// it is a WIRE promise (`ControlMsg::ProfileList`), not an artifact of
    /// however SQLite happens to return rows.
    #[tokio::test]
    async fn profiles_round_trip_through_create_update_and_delete() {
        let (_dir, store) = fresh_store().await;
        let created = match store
            .create_profile(
                "Local Claude".to_string(),
                "/opt/bin/claude --verbose".to_string(),
                farhelm_proto::AgentKind::Claude,
                Some(vec![
                    "/opt/bin/claude".to_string(),
                    "--resume".to_string(),
                    crate::agent_kind::CONVERSATION_PLACEHOLDER.to_string(),
                ]),
            )
            .await
            .expect("create")
        {
            ProfileCreation::Created(profile) => profile,
            ProfileCreation::CatalogFull => panic!("a catalog of two starters is not full"),
        };
        assert_ne!(created.id, "", "the store mints the id");
        assert_eq!(
            store.profile(&created.id).await.expect("read"),
            Some(created.clone()),
            "what a create reports must be exactly what the row holds"
        );

        let listed = store.profiles().await.expect("list");
        let ids: Vec<&str> = listed.iter().map(|profile| profile.id.as_str()).collect();
        assert!(
            ids.is_sorted(),
            "the listing is id-ascending, as the wire contract promises: {ids:?}"
        );
        assert!(ids.contains(&created.id.as_str()));

        // A full replacement, including CLEARING the resume template — the
        // case a patch-shaped update could not express at all.
        let edited = farhelm_proto::Profile {
            id: created.id.clone(),
            name: "Renamed".to_string(),
            invocation: "bash".to_string(),
            agent_kind: farhelm_proto::AgentKind::Generic,
            resume_template: None,
        };
        assert_eq!(
            store
                .update_profile(edited.clone())
                .await
                .expect("update")
                .as_ref(),
            Some(&edited)
        );
        assert_eq!(
            store.profile(&created.id).await.expect("read"),
            Some(edited)
        );

        assert!(store.delete_profile(&created.id).await.expect("delete"));
        assert_eq!(store.profile(&created.id).await.expect("read"), None);
        assert!(
            !store.delete_profile(&created.id).await.expect("delete"),
            "deleting what is already gone reports so rather than claiming success"
        );
        assert_eq!(
            store
                .update_profile(farhelm_proto::Profile {
                    id: created.id.clone(),
                    name: "ghost".to_string(),
                    invocation: "bash".to_string(),
                    agent_kind: farhelm_proto::AgentKind::Generic,
                    resume_template: None,
                })
                .await
                .expect("update"),
            None,
            "an update is an edit of something that exists, never a create in disguise"
        );
    }

    /// The catalog bound at its exact boundary (PLAN_M6_75.md item 4): the
    /// create that brings the catalog TO `MAX_PROFILES_PER_HOST` succeeds
    /// and the next one is refused with nothing written.
    ///
    /// Boundary rather than "some large number" because an off-by-one here
    /// is invisible in production until the day someone hits it, and both
    /// directions are real failures: refusing one early costs a profile
    /// nobody could explain, and allowing one extra is the first step of
    /// the unbounded catalog `MAX_PROFILES_PER_HOST` exists to prevent —
    /// one too large to LIST, and therefore too large to trim back.
    ///
    /// The starters count toward the bound, deliberately: they are ordinary
    /// rows, and a bound that excused them would be a bound on a number
    /// nobody can observe.
    #[tokio::test]
    async fn the_catalog_bound_refuses_the_create_that_would_exceed_it() {
        let (_dir, store) = fresh_store().await;
        let seeded = store.profiles().await.expect("catalog").len();
        for i in seeded..farhelm_proto::MAX_PROFILES_PER_HOST {
            assert!(
                matches!(
                    store
                        .create_profile(
                            format!("p{i}"),
                            "bash".to_string(),
                            farhelm_proto::AgentKind::Generic,
                            None,
                        )
                        .await
                        .expect("create"),
                    ProfileCreation::Created(_)
                ),
                "profile {i} is still within the bound"
            );
        }
        assert_eq!(
            store.profiles().await.expect("catalog").len(),
            farhelm_proto::MAX_PROFILES_PER_HOST
        );
        assert_eq!(
            store
                .create_profile(
                    "one too many".to_string(),
                    "bash".to_string(),
                    farhelm_proto::AgentKind::Generic,
                    None,
                )
                .await
                .expect("create"),
            ProfileCreation::CatalogFull
        );
        assert_eq!(
            store.profiles().await.expect("catalog").len(),
            farhelm_proto::MAX_PROFILES_PER_HOST,
            "a refused create must write nothing at all"
        );
    }

    /// A session's source-profile snapshot must survive the on-disk round
    /// trip intact, and a HALF-written pair must be refused at load
    /// (PLAN_M6_75.md item 4).
    ///
    /// The refusal half is the part worth a test: SQLite is deliberately
    /// not asked to enforce the pairing (a table-level `CHECK` cannot be
    /// added by `ALTER TABLE`, so adding one would make migrated and fresh
    /// databases differ), which means the invariant lives entirely in code
    /// and would rot silently without this. A row with an id and no name
    /// would render as a session created from a nameless profile; one with
    /// a name and no id could never have its existence derived at all.
    #[tokio::test]
    async fn a_sessions_source_profile_round_trips_and_a_half_row_is_refused() {
        let (_dir, store) = fresh_store().await;
        store
            .insert_session(
                StoredSession {
                    id: "s1".to_string(),
                    parent: None,
                    archived: false,
                    title: "s1".to_string(),
                    created_at: now_unix(),
                    creation_seq: 0,
                    cwd: "/tmp/work".to_string(),
                    invocation: "claude".to_string(),
                    tmux_name: "fh-s1".to_string(),
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
                    launch_scoped: false,
                    source_profile: Some(ProfileSnapshot {
                        id: "starter-claude".to_string(),
                        name: "Claude Code".to_string(),
                    }),
                },
                None,
            )
            .await
            .expect("insert");
        assert_eq!(
            store
                .session("s1")
                .await
                .expect("read")
                .expect("present")
                .source_profile,
            Some(ProfileSnapshot {
                id: "starter-claude".to_string(),
                name: "Claude Code".to_string(),
            })
        );

        {
            let conn = store.conn.lock().expect("db mutex");
            conn.execute(
                "UPDATE sessions SET source_profile_name = NULL WHERE id = ?1",
                rusqlite::params!["s1"],
            )
            .expect("hand-edit half the pair away");
        }
        let refusal = store
            .session("s1")
            .await
            .expect_err("half a snapshot is not something to guess the other half of");
        assert!(
            format!("{refusal:#}").contains("half of a source-profile snapshot"),
            "the refusal must say what is wrong with the row: {refusal:#}"
        );

        // The OTHER orientation, which is not symmetric and would be easy
        // to miss with a check written as "if the id is missing": a name
        // with no id is worse, because the id is the only key existence can
        // be derived by, so the row could never be described at all.
        {
            let conn = store.conn.lock().expect("db mutex");
            conn.execute(
                "UPDATE sessions SET source_profile_id = NULL, source_profile_name = 'orphan' \
                 WHERE id = ?1",
                rusqlite::params!["s1"],
            )
            .expect("hand-edit the other half away");
        }
        let refusal = store
            .session("s1")
            .await
            .expect_err("a name with no id names nothing this build can look up");
        assert!(
            format!("{refusal:#}").contains("half of a source-profile snapshot"),
            "the refusal must say what is wrong with the row: {refusal:#}"
        );
    }

    /// The version-7-to-8 migration against a database that ALREADY HAS
    /// session rows (PLAN_M6_75.md item 4).
    ///
    /// `migrated_and_fresh_schemas_agree` compares an EMPTY migrated
    /// database against a fresh one, which cannot distinguish "preserves
    /// the rows it never mentions" from "tolerates a table with no rows".
    /// This is the shape a real upgrading host has: sessions already
    /// running, created by a build that had no catalog at all.
    ///
    /// Three promises, and each fails differently. The pre-existing session
    /// survives with every column intact — a migration that rebuilt the
    /// table would be the classic way to lose one. It comes back
    /// raw-created rather than acquiring some invented profile, which is
    /// the honest reading of a session that predates the whole feature.
    /// And the starter catalog appears, so an upgraded host is not the one
    /// place where SPEC.md's "a fresh supervisor is not empty" quietly does
    /// not hold.
    #[tokio::test]
    async fn the_v7_to_v8_migration_preserves_sessions_and_seeds_the_catalog() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("supervisor.db");
        plant_v7_database(&path, "s1");

        let store = SessionStore::open(&path, true).await.expect("migrate");
        let rows = store.load_all().await.expect("load");
        assert_eq!(rows.len(), 1, "the migration must preserve every session");
        assert_eq!(rows[0].id, "s1");
        assert_eq!(rows[0].title, "title-s1");
        assert_eq!(rows[0].invocation, "claude");
        assert_eq!(rows[0].agent_kind, farhelm_proto::AgentKind::Claude);
        assert_eq!(rows[0].outcome, LastOutcome::Running);
        assert_eq!(
            rows[0].source_profile, None,
            "a session that predates the catalog is raw-created, not a session whose profile \
             the migration had to invent"
        );
        assert_eq!(
            store
                .profiles()
                .await
                .expect("catalog")
                .iter()
                .map(|profile| profile.id.as_str())
                .collect::<Vec<_>>(),
            vec!["starter-claude", "starter-codex"],
            "an upgrading host gets the starter catalog too"
        );
    }

    /// A version-7-to-8 migration that fails PARTWAY must leave the
    /// database exactly as it was: no new columns, no seeded profiles, and
    /// the version still 7.
    ///
    /// The step does three things — two `ALTER TABLE`s, a `CREATE TABLE`,
    /// and an `INSERT` — and the ALTERs run FIRST, so a failure at the
    /// create is the case that actually exercises the rollback rather than
    /// a failure that had nothing to undo. A half-applied ladder step is
    /// the worst outcome available here: the version would say 7 while the
    /// columns said 8, and the next open would try the ALTERs again and
    /// fail forever on the duplicate column.
    ///
    /// Provoked the way the version-2 test provokes its own equivalent: a
    /// table already occupying the name the migration is about to create.
    /// That is not a contrived situation — it is precisely what a database
    /// touched by a NEWER build and then opened by an older one looks like.
    #[tokio::test]
    async fn a_failed_v7_to_v8_migration_rolls_back_columns_seed_and_version_together() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("supervisor.db");
        plant_v7_database(&path, "s1");
        {
            let conn = Connection::open(&path).expect("open raw");
            conn.execute_batch("CREATE TABLE profiles (nonsense TEXT);")
                .expect("occupy the name the migration wants");
        }

        SessionStore::open(&path, true)
            .await
            .expect_err("the migration cannot create a table that already exists");

        let conn = Connection::open(&path).expect("open raw");
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .expect("read version");
        assert_eq!(version, 7, "a failed step must not claim its version");
        let columns: Vec<String> = conn
            .prepare("SELECT name FROM pragma_table_info('sessions')")
            .expect("prepare")
            .query_map([], |r| r.get(0))
            .expect("query")
            .collect::<Result<_, _>>()
            .expect("collect");
        assert!(
            !columns
                .iter()
                .any(|name| name.starts_with("source_profile")),
            "the ALTERs ran before the failing statement and must have been rolled back with \
             it: {columns:?}"
        );
        let seeded: i64 = conn
            .query_row("SELECT COUNT(*) FROM profiles", [], |r| r.get(0))
            .expect("count the decoy table's rows");
        assert_eq!(
            seeded, 0,
            "nothing may have been seeded into the table that blocked the migration"
        );
    }

    /// A rename that lands between a crashed create and its retry SURVIVES
    /// the retry's takeover.
    ///
    /// The takeover is a delete-and-reinsert, and the row it reinserts is
    /// built from the caller's snapshot — which was resolved before the
    /// crash and therefore carries the OLD title. Every other column on
    /// that row describes a launch that provably never happened, so
    /// overwriting them is right; `title` is the exception, because it is
    /// the one field a user can change after creation, and a rename the
    /// supervisor accepted and acknowledged being silently reverted by an
    /// unrelated retry is a lost write the user has no way to explain.
    ///
    /// The rename is applied through the ordinary writer rather than by
    /// hand, so this exercises the real interleaving of the two paths.
    #[tokio::test]
    async fn a_relaunch_takeover_preserves_a_rename_that_landed_between_the_attempts() {
        let (_dir, store) = fresh_store().await;
        let stranded = |title: &str| StoredSession {
            id: "s1".to_string(),
            parent: None,
            archived: false,
            title: title.to_string(),
            created_at: 1_700_000_000,
            creation_seq: 0,
            cwd: "/tmp/work".to_string(),
            invocation: "agent".to_string(),
            tmux_name: "fh-s1".to_string(),
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
            source_profile: None,
        };
        store
            .insert_session(
                stranded("as created"),
                Some(IntentClaim {
                    intent_key: "key".to_string(),
                    fingerprint: "fp".to_string(),
                    dedup_scope: DedupScope::Permanent,
                }),
            )
            .await
            .expect("seed the crashed attempt");

        // The user renames it while the create is still unresolved — the
        // session is listed (a launching row lists like any other), so this
        // is an ordinary thing to do rather than a contrived race.
        store
            .set_session_title("s1", "as renamed")
            .await
            .expect("rename");

        let claim = store
            .restart_pending_launch(stranded("as created"), "key")
            .await
            .expect("takeover");
        let RetryClaim::Acquired { title, .. } = &claim else {
            panic!("premise: the retry takes the reservation over, got {claim:?}");
        };
        assert_eq!(
            title, "as renamed",
            "the preserved title has to come BACK to the caller, not merely stay in SQLite: the \
             caller builds its reply and the replacement in-memory entry from the snapshot it \
             resolved before the rename, so a title kept only in the row would leave every list \
             this process serves showing the old label until the next reload"
        );
        let row = store.session("s1").await.expect("read").expect("present");
        assert_eq!(
            row.title, "as renamed",
            "the retry must not revert a rename it knew nothing about"
        );
        assert_eq!(
            row.created_at, 1_700_000_000,
            "and the crashed attempt's timestamp is still preserved alongside it"
        );
    }

    /// A profile row this build cannot honestly decode is REFUSED, with
    /// context naming the row — never defaulted, and never quietly dropped
    /// from the listing.
    ///
    /// The database is a trust boundary like any other input: a row is
    /// whatever the last writer, a crash, a downgrade, or a hand-edit left
    /// behind. Both failure modes here would be silent in the worst way.
    /// Defaulting an unrecognized `agent_kind` to `Generic` would turn a
    /// Claude profile into one with no integration — no capture, no
    /// sharpening — with nothing anywhere saying so. Dropping a row with a
    /// malformed template from the listing would make a profile the user
    /// can see in no picker and therefore cannot delete or repair, while
    /// the create that names it keeps working.
    ///
    /// Asserted through BOTH readers, because they are separate queries
    /// with separate decode paths and a fix applied to one would leave the
    /// other guessing.
    ///
    /// The table covers SEMANTIC corruption as well as syntactic, and that
    /// half is the newer and less obvious one. A row can decode into a
    /// perfectly well-formed `Profile` and still describe something no
    /// create could use or no listing could render — a blank name that is a
    /// row a user cannot tell from its neighbours, an invocation naming no
    /// program, a record large enough to make `ProfileList` undeliverable, a
    /// template whose program slot is the substitution placeholder. Reaching
    /// that state needs nothing exotic: a database written by a build with
    /// looser rules, restored from a backup, or hand-edited.
    #[tokio::test]
    async fn a_corrupt_profile_row_is_refused_by_both_readers() {
        for (column, value, expected) in [
            ("agent_kind", "'sonnet'", "unrecognized agent kind"),
            ("resume_template", "'not json'", "resume template"),
            // Semantic corruption: each of these decodes cleanly and is
            // still a profile the rest of the system cannot honor.
            ("name", "''", "must not be empty"),
            ("name", "'   '", "must not be empty"),
            ("name", "'tab\theld'", "control characters"),
            ("invocation", "''", "is empty"),
            // A SQL literal holding the two characters `''` — a command
            // line that parses to one empty token, which exists and names
            // nothing.
            ("invocation", "''''''", "names no program"),
            (
                "resume_template",
                "'[\"\", \"--resume\", \"{conversation}\"]'",
                "names no program",
            ),
            (
                "resume_template",
                "'[\"{conversation}\", \"--resume\"]'",
                "the PROGRAM",
            ),
            (
                "name",
                &format!("'{}'", "x".repeat(farhelm_proto::PROFILE_FIELD_CAP + 1)),
                "exceeding",
            ),
        ] {
            let (_dir, store) = fresh_store().await;
            {
                let conn = store.conn.lock().expect("db mutex");
                conn.execute_batch(&format!(
                    "UPDATE profiles SET {column} = {value} WHERE id = 'starter-claude';"
                ))
                .expect("hand-edit the row into a shape this build cannot read");
            }

            let listed = store
                .profiles()
                .await
                .expect_err("a listing that silently omitted the row would hide it forever");
            let read = store
                .profile("starter-claude")
                .await
                .expect_err("and the single read must agree with the listing");
            for (what, refusal) in [("listing", &listed), ("read", &read)] {
                let rendered = format!("{refusal:#}");
                assert!(
                    rendered.contains(expected) && rendered.contains("starter-claude"),
                    "the {what}'s refusal must name both the fault and the row it is in: \
                     {rendered}"
                );
            }
        }
    }

    /// The store's OWN writes refuse a profile no create could use, without
    /// depending on a handler having checked first.
    ///
    /// The handler check is the one that produces a good client error, and
    /// it is not going anywhere — but it is not what makes the rule true.
    /// Anything else reaching these methods (a test, a repair path, a future
    /// import) could otherwise commit a row that `ProfileList` cannot render
    /// and no create can launch, which then has to be caught on the way back
    /// OUT by the decode. Refusing at both ends is what keeps the catalog
    /// from ever holding such a row in the first place.
    ///
    /// Create and update are asserted together because an update that
    /// accepted what a create refuses would let a bounded catalog be grown
    /// past its bound one edit at a time.
    #[tokio::test]
    async fn the_store_refuses_a_profile_no_create_could_use() {
        let (_dir, store) = fresh_store().await;
        let seeded = store.profiles().await.expect("catalog").len();

        for (what, name, invocation, kind, template) in [
            (
                "a blank name",
                "   ",
                "bash",
                farhelm_proto::AgentKind::Generic,
                None,
            ),
            (
                "an invocation naming no program",
                "empty program",
                "''",
                farhelm_proto::AgentKind::Generic,
                None,
            ),
            (
                "a template whose program slot is the placeholder",
                "placeholder program",
                "claude",
                farhelm_proto::AgentKind::Claude,
                Some(vec![
                    crate::agent_kind::CONVERSATION_PLACEHOLDER.to_string(),
                    "--resume".to_string(),
                ]),
            ),
        ] {
            store
                .create_profile(
                    name.to_string(),
                    invocation.to_string(),
                    kind,
                    template.clone(),
                )
                .await
                .expect_err(&format!(
                    "a create with {what} must be refused by the store"
                ));
            store
                .update_profile(farhelm_proto::Profile {
                    id: "starter-claude".to_string(),
                    name: name.to_string(),
                    invocation: invocation.to_string(),
                    agent_kind: kind,
                    resume_template: template,
                })
                .await
                .expect_err(&format!("and so must an update with {what}"));
        }

        assert_eq!(
            store.profiles().await.expect("catalog").len(),
            seeded,
            "not one refused write may have landed"
        );
        assert_eq!(
            store
                .profile("starter-claude")
                .await
                .expect("read")
                .expect("the starter is still there")
                .name,
            "Claude Code",
            "and the refused updates must have left the row they targeted alone"
        );
    }

    /// Two creates racing at the catalog's last free slot: exactly one wins
    /// (PLAN_M6_75.md item 4).
    ///
    /// This is the whole reason the count lives inside the insert's
    /// transaction rather than in the handler. A caller that counted first
    /// and inserted after would read `MAX_PROFILES_PER_HOST - 1` in BOTH
    /// racers, and both would insert — a catalog one profile past a bound
    /// whose entire job is to keep `ProfileList` sendable. Reading the
    /// count in the same transaction as the insert is what makes the second
    /// racer see the first one's row.
    ///
    /// Run through `tokio::join!` on one store handle, which is what the
    /// supervisor's own concurrent requests look like: the store serializes
    /// on its connection mutex, so the two transactions genuinely order
    /// against each other rather than merely appearing to.
    #[tokio::test]
    async fn two_creates_racing_the_last_catalog_slot_produce_exactly_one_profile() {
        let (_dir, store) = fresh_store().await;
        let seeded = store.profiles().await.expect("catalog").len();
        for i in seeded..farhelm_proto::MAX_PROFILES_PER_HOST - 1 {
            store
                .create_profile(
                    format!("p{i}"),
                    "bash".to_string(),
                    farhelm_proto::AgentKind::Generic,
                    None,
                )
                .await
                .expect("create");
        }
        assert_eq!(
            store.profiles().await.expect("catalog").len(),
            farhelm_proto::MAX_PROFILES_PER_HOST - 1,
            "premise: exactly one slot left"
        );

        let create = |name: &str| {
            let store = store.clone();
            let name = name.to_string();
            async move {
                store
                    .create_profile(
                        name,
                        "bash".to_string(),
                        farhelm_proto::AgentKind::Generic,
                        None,
                    )
                    .await
                    .expect("create")
            }
        };
        let (first, second) = tokio::join!(create("racer-a"), create("racer-b"));
        let created = [&first, &second]
            .iter()
            .filter(|outcome| matches!(outcome, ProfileCreation::Created(_)))
            .count();
        assert_eq!(
            created, 1,
            "exactly one racer may take the last slot: {first:?} and {second:?}"
        );
        assert_eq!(
            store.profiles().await.expect("catalog").len(),
            farhelm_proto::MAX_PROFILES_PER_HOST,
            "and the catalog must land exactly ON the bound, never past it"
        );
    }
}

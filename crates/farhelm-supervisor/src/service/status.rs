//! Reading a session's current state back out: what its status is, what a
//! restart would offer it, and the `SessionInfo` every reply is built
//! from.
//!
//! These are the read-out half of the state model the `service` module doc
//! describes — the half that answers "what happened to this session?" from
//! the entry's own durable record and sample cell, and its terminal's
//! evidence, which is the host runtime's state read from memory
//! ([`crate::hosts::HostStatus::evidence`]). Nothing here goes looking for
//! either one.
//!
//! The guarantee that shape buys holds for the SYNCHRONOUS
//! classification core — `session_status`, `session_restart_offer`, and
//! `entry_info`, the three a reply is actually built from. None of those
//! consults the session map, and none issues an I/O round trip of its own;
//! each is a pure read of the entry. That is what lets a list pass
//! classify every entry under its own locks, and what makes the
//! classification testable against hand-built entries with no store and
//! no supervisor at all.
//!
//! PLAN_M6_75.md item 2's activity classification did not weaken the shape
//! either: the sampler's
//! [`ActivitySample`](crate::service::ticker::ActivitySample) rides the
//! ENTRY, so "which live status" is answered from what is already in hand —
//! no clock, no supervisor, no lookup. The absent clock is a deliberate
//! property rather than an accident of shape;
//! `agent_kind::screen_reader::QUIET_SAMPLES_BEFORE_IDLE` carries that
//! argument.
//!
//! Three claims a reader might over-generalize from that, all FALSE, and
//! the difference matters to anyone reasoning about what may run where:
//!
//! - "Nothing here does I/O" — no. `observe_entry` reads launch sentinels
//!   off disk; it is `async` and does not belong inside a lock hold. It
//!   sits here because it answers the same question, not because it shares
//!   the core's constraints.
//! - "Lock-free" — no. `session_status` and `session_restart_offer` take
//!   the per-ENTRY `outcome`, `activity` and `capture` mutexes (and
//!   `entry_info` takes all three, through them). Those are leaf mutexes
//!   held across no await, so they are safe to call from anywhere, but
//!   they are locks.
//! - "Uniform signature" — no. `observe_entry` takes the supervisor. The
//!   bare `&SessionEntry` shape describes the core, not every symbol in the
//!   file.
//!
//! The precedence rules these encode are the contract PLAN_M3.md items 2
//! and 3 define, and the full order is: a durably recorded **error**
//! outranks everything, including a terminal still reported alive (a
//! failed exec is a fact no probe can discover); then a launch sentinel
//! found THIS pass, for the same reason, while the recorded outcome still
//! permits launch evidence (a stop-annotated exit fences it out); then a
//! live terminal, which no non-error stored outcome may override; then,
//! with no terminal to ask, the stored outcome, which beats the blanket
//! exited-unknown fallback. A host that cannot be read
//! ([`TerminalEvidence::Unattributed`]) sits beside that ladder rather than
//! on it: nothing is inferred or recorded from it, and an unsettled outcome
//! reads as unknown. `session_status` and `observe_entry` carry the two
//! halves of that order and must agree.
//! Keeping them in one module is what makes "a `SessionRenamed` reply
//! describes a session exactly as `ListSessions` would" a property of the
//! code rather than a promise maintained by hand.

use super::core::{SessionEntry, Supervisor};
use super::launch_artifacts::{
    append_launch_excerpt, read_launch_sentinel, sentinel_could_still_apply, stopped_before_start,
    wrapper_failure_detail,
};
use crate::agent_kind::screen_reader::ScreenState;
use crate::hosts::HostTerminal;
use crate::store::{LastOutcome, SessionStore, Transition};
use anyhow::Context;
use farhelm_proto::{RestartOffer, SessionInfo, SessionStatus};
use std::path::Path;
use std::sync::Arc;

/// What status reads from a terminal its session owns: whether its program
/// has exited, and how, from the host runtime's exit record
/// (`crate::hosts::HostEvidence::Owned`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OwnedTerminal {
    /// The program has exited, and the terminal still holds what it left:
    /// a host whose exit record has been processed.
    pub(crate) dead: bool,
    /// The exit status, when the program is dead and exited with one.
    pub(crate) exit_code: Option<i32>,
}

/// What can be said about one recorded terminal, as far as it can be
/// attributed to the session that recorded it
/// (`crate::hosts::HostStatus::evidence`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TerminalEvidence {
    /// The terminal answers for this session's program: the host's runtime
    /// reads it, and its dead flag and exit status are this session's.
    Owned(OwnedTerminal),
    /// The terminal is gone: the host has ended, or there is none. The tick
    /// and reload can record the exit; screen sampling still requires
    /// positive ownership.
    Absent,
    /// The terminal may run this session's program, and nothing can be
    /// concluded about it: a host that may run but cannot be read (an
    /// unknown protocol or snapshot format, a connection that keeps ending:
    /// the plan's D-16). Neither liveness nor an exit can be concluded from
    /// it, so observers draw no inference and report an unresolved outcome
    /// as `Unknown`, the same way the stop, restart and delete verbs refuse
    /// to act on it, and nothing is recorded, a launch sentinel included,
    /// since the host may belong to a supervisor that can read it and will
    /// classify the launch itself. The ticker skips screen sampling.
    Unattributed,
}

impl From<crate::hosts::HostEvidence> for TerminalEvidence {
    fn from(evidence: crate::hosts::HostEvidence) -> Self {
        match evidence {
            crate::hosts::HostEvidence::Owned { dead, exit_code } => {
                TerminalEvidence::Owned(OwnedTerminal { dead, exit_code })
            }
            crate::hosts::HostEvidence::Absent => TerminalEvidence::Absent,
            crate::hosts::HostEvidence::Unattributed => TerminalEvidence::Unattributed,
        }
    }
}

/// The evidence for an entry's recorded terminal; no terminal at all is
/// [`TerminalEvidence::Absent`].
fn terminal_evidence(terminal: Option<&HostTerminal>) -> TerminalEvidence {
    terminal.map_or(TerminalEvidence::Absent, |host| host.evidence().into())
}

/// Compute one session's liveness for a `ListSessions` reply. This
/// function only ever reports what it can actually observe, never a guess:
/// a session with no terminal, or whose host has ended, has nothing live to
/// report, and only a host whose runtime reads it decides live-versus-
/// `Exited` from its own exit record. A host that cannot be read
/// ([`TerminalEvidence::Unattributed`]) may still run our agent, so a
/// session whose outcome is not already settled reports `Unknown` rather
/// than an exit that may never have happened.
///
/// ## Classification precedence (PLAN_M3.md items 2 and 3)
///
/// As of M3 the live probe is no longer the only input: the session's
/// durable last-known outcome answers the questions a vanished terminal
/// cannot. The order below is the precedence, and it is deliberate:
///
/// 1. A recorded **error** — the launch shim's exec-failure sentinel —
///    outranks every inference, because "the agent never started" is a
///    fact about THIS launch that no amount of probing can discover (an
///    unexec'd command leaves an ordinary dead terminal behind, exactly
///    like a command that ran and exited). PLAN_M3.md item 3 owns the
///    READER that ever writes this state; this PR only makes sure it
///    already sits above the inference so item 3 has nothing to
///    restructure. **The sentinel is deliberately not read here.**
/// 2. A live terminal decides live-versus-`Exited` exactly as M2 did — a
///    stored outcome never overrides something still observable. What the
///    record still contributes to a DEAD terminal is what the terminal
///    cannot hold: the stop annotation, and an exit code it does not have
///    (`known code wins`, matching the store's own monotonic enrichment
///    rule, so the live reading can never be the poorer of the two).
/// 3. With no terminal to ask, the recorded outcome speaks: `Interrupted`
///    (the reboot conversion) and `Exited` (a previously witnessed exit,
///    with the code and annotation it was witnessed with) are RETAINED
///    KNOWLEDGE, not guesses, and outrank M2's blanket exited-unknown.
/// 4. A `Launching` row with no terminal is `Unknown`, not `Exited`: SPEC.md's
///    exited means the agent RAN, and a launch whose side effects were
///    never found has not established that. It stays pending for item 3's
///    sentinel (error) or item 6's reservation (retry) to resolve.
///    A dead terminal during an ambiguous restart is the same unknown
///    evidence: its exit code may belong to the previous generation, so it
///    must not be attributed to this launch.
/// 5. Anything else with no terminal — a stored `LastOutcome::Running` (the
///    durable record's own vocabulary, not the wire status that now
///    shares its name), or a stop whose sweep is in flight — falls back
///    to M2's honest `Exited { exit_code: None }`.
///
/// ## Which live status (PLAN_M6_75.md item 2)
///
/// Rule 2 above says a live terminal wins; [`live_status`] says which of
/// the three live statuses it wins WITH. That split is deliberate and is
/// where the milestone's whole heuristic lives: liveness is observed (the
/// host's child has not exited — a fact), while running-versus-waiting-versus-idle is
/// inferred from sampled screens (a guess, cosmetic by SPEC.md's own
/// status rule). Nothing in the precedence above depends on which one it
/// is, so a wrong guess can never promote or demote a session across the
/// boundary that actually matters.
///
/// The annotation returned alongside the status is SPEC.md's user-legible
/// qualifier ("stopped by user"), which lives with the recorded outcome
/// and therefore survives restarts and reboots. It is returned only for a
/// status that ends up `Exited`: a session that has since been relaunched
/// into a live terminal must not still be labelled with how its PREVIOUS
/// run ended.
pub(crate) fn session_status(entry: &SessionEntry) -> (SessionStatus, Option<String>) {
    // The guard is held across the whole match rather than cloned out of:
    // this function is synchronous (no await can intervene) and every arm
    // only reads, so the clone would have bought nothing but an allocation
    // on the hottest path the list reply has.
    let recorded = entry.run.outcome.lock().expect("outcome mutex poisoned");
    let live = match terminal_evidence(entry.terminal.as_ref()) {
        TerminalEvidence::Owned(state) => Some(state),
        TerminalEvidence::Absent => None,
        // A settled outcome is retained knowledge and still speaks below; an
        // unsettled one has nothing observable to decide it.
        TerminalEvidence::Unattributed if !recorded.is_terminal() => {
            return (SessionStatus::Unknown, None);
        }
        TerminalEvidence::Unattributed => None,
    };
    match (&*recorded, live) {
        (LastOutcome::Error { detail }, _) => (
            SessionStatus::Error {
                detail: detail.clone(),
            },
            None,
        ),
        // A live terminal, and no annotation with it: an annotation describes
        // how a run ENDED, and this one has not.
        (_, Some(state)) if !state.dead => (live_status(entry), None),
        (LastOutcome::Launching, Some(state)) if state.dead => (SessionStatus::Unknown, None),
        (recorded, Some(state)) => {
            let (recorded_code, annotation) = match recorded {
                LastOutcome::Exited {
                    exit_code,
                    annotation,
                } => (*exit_code, annotation.clone()),
                _ => (None, None),
            };
            (
                SessionStatus::Exited {
                    exit_code: state.exit_code.or(recorded_code),
                },
                annotation,
            )
        }
        (LastOutcome::Interrupted, None) => (SessionStatus::Interrupted, None),
        (
            LastOutcome::Exited {
                exit_code,
                annotation,
            },
            None,
        ) => (
            SessionStatus::Exited {
                exit_code: *exit_code,
            },
            annotation.clone(),
        ),
        (LastOutcome::Launching, None) => (SessionStatus::Unknown, None),
        (LastOutcome::Running | LastOutcome::StopRequested, None) => {
            (SessionStatus::Exited { exit_code: None }, None)
        }
    }
}

/// Which of the three live statuses a session with a living terminal gets
/// (PLAN_M6_75.md item 2): what the session's screen reader concluded from
/// its most recent successful sample.
///
/// All per-harness knowledge lives in the readers
/// (`agent_kind::screen_reader`); this function only maps a stored
/// [`ScreenState`] onto the wire vocabulary, with one rule of its own: a
/// reloaded run reports `Unknown` until its first sample that is fresh
/// evidence, so the helm keeps its cached answer through the gap — except
/// that a recognized waiting prompt is reported at once, since it names
/// the interaction the user is asked for.
///
/// ## The pre-first-sample state
///
/// A new launch starts `Running` before sampling: its live terminal is
/// evidence of a newly started agent, and the default reading says working.
/// A reloaded terminal has no such fresh launch evidence, so it reports
/// `Unknown` while the helm retains the preceding cached status.
///
/// ## Bounds this is deliberately allowed to violate cosmetically
///
/// A newly launched session that never gets sampled — because its screen
/// cannot be read on any tick — stays `Running`. A reloaded one stays
/// provisional instead. Both answers can remain stale while capture fails;
/// neither changes whether the terminal can be used.
/// Agent-requested prompt delivery is the narrow admission exception: after
/// probing its own live terminal, it refuses a Waiting or Unknown reading to avoid
/// knowingly typing into a dialog. The user's own terminal input stays ungated.
///
/// Only called for a live terminal ([`session_status`] decides liveness
/// first), which is what keeps a stale prompt on a dead terminal from
/// reading waiting.
/// No clock is read here, and none should be (see
/// `screen_reader::QUIET_SAMPLES_BEFORE_IDLE`). Runs under the entry's
/// `activity` mutex, a leaf lock held across no await and alongside no
/// other lock, for a field read.
pub(crate) fn live_status(entry: &SessionEntry) -> SessionStatus {
    let activity = entry.run.activity.lock().expect("activity mutex poisoned");
    match activity.reading.state {
        ScreenState::Waiting => SessionStatus::Waiting,
        _ if activity.startup_provisional => SessionStatus::Unknown,
        ScreenState::Working => SessionStatus::Running,
        ScreenState::Idle => SessionStatus::Idle,
        // Never stored: the sampler keeps the previous reading when a
        // screen says nothing about the agent. Mapped anyway so the match
        // stays total without a panic on a cosmetic path.
        ScreenState::Unknown => SessionStatus::Unknown,
    }
}

/// What restarting this session would do to its conversation, computed
/// fresh from the snapshot and whatever identity is DURABLY claimed right
/// now.
///
/// Recomputed on every reply for the same reason `status` is: a capture
/// pass can upgrade a session from `NotCaptured` to `Resume` at any moment,
/// so the value stored in `SessionEntry::info` at create or reload is a
/// starting point rather than an answer. Reads only the COMMITTED identity
/// ([`super::capture::CaptureState::committed_conversation`]), which keeps
/// the offer from promising a resume that no stored value could fill.
pub(super) fn session_restart_offer(entry: &SessionEntry) -> RestartOffer {
    let capture = entry.run.capture.lock().expect("capture mutex poisoned");
    entry.snapshot.restart_offer(
        capture.committed_conversation(),
        capture.committed_ownership_version().unwrap_or(0),
    )
}

/// One entry as a reply must describe it: the stored metadata plus the
/// three fields that are NEVER stored as answers and are therefore
/// recomputed on every reply — live-probed `status` (with its annotation),
/// the session's current `tabs`, and a freshly derived `restart_offer`.
///
/// `last_activity_at` is refreshed here too, and is deliberately not one
/// of those three: it IS stored, and this is a plain read of the entry's
/// live cell rather than a recomputation. It needs refreshing for a
/// mechanical reason only — the entry is immutable behind its `Arc`, so
/// the ticker advances a cell beside `info` rather than `info` itself.
///
/// The single place that shape is defined, shared by `ListSessions` and by
/// the single-session replies that must match it (`SessionRenamed`, whose
/// own protocol docs promise a `SessionInfo` "built the same way
/// `ListSessions` builds one"). Two copies would drift, and the drift
/// would be invisible: both would still be `SessionInfo`s, differing only
/// in which fields told the truth.
///
/// A cached launch failure outranks terminal-derived status even when recording
/// was unavailable, but only while the mirrored outcome could still accept
/// launch evidence. A completed stop cannot be overwritten by an observer's
/// now-uncommittable fallback. Replies never touch launch artifacts themselves.
pub(crate) fn entry_info(entry: &SessionEntry) -> SessionInfo {
    let mut info = entry.info.clone();
    // List and rename replies read the same run cell; the immutable entry was
    // built before any footer was sampled and must not freeze that first value.
    let activity = entry.run.activity.lock().expect("activity mutex poisoned");
    info.screen_yolo = activity.screen_yolo;
    info.claude_trust_prompt = activity.claude_trust_prompt;
    info.workspace_trust_prompt = activity.workspace_trust_prompt;
    drop(activity);
    info.restart_offer = session_restart_offer(entry);
    // The entry's `info` froze this at build time (creation, reload, or
    // restart); the sampler has been advancing the cell beside it ever
    // since. Overwritten here rather than kept in sync on the entry
    // because a `SessionEntry` is immutable behind its `Arc` — the same
    // reason `status` and `restart_offer` are recomputed above. Unlike
    // those, this is a READ of a value the ticker decided, not a fresh
    // computation: nothing on the reply path may mint an activity time.
    info.last_activity_at = entry
        .session
        .last_activity_at
        .load(std::sync::atomic::Ordering::Relaxed);
    info.last_work_started_at = entry
        .session
        .last_work_started_at
        .load(std::sync::atomic::Ordering::Relaxed);
    // Notifications are recorded long after the entry was built, so the
    // clone carries whatever the entry started with; the session's cell is
    // the current list (`SessionCells::notifications`).
    info.notifications = super::notifications::notification_wire(
        &entry
            .session
            .notifications
            .lock()
            .expect("notification cell poisoned"),
    );
    // Tabs are not stored anywhere at all (`SessionInfo::tabs`): the list is
    // the session's own cell, in the order they were opened. An exited one
    // is hidden: SPEC.md reaps a tab whose process exited, and the ticker's
    // reap may lag this reply by a tick — hiding the corpse here is what
    // keeps the listing honest in that window.
    info.tabs = super::core::listed_host_tabs(&entry.session.host_tabs, false);
    // From the terminal the entry holds, not the row's stored kind: a
    // terminal-less entry (a launch not yet confirmed, a terminal a reboot
    // took) reports no kind.
    info.agent_terminal = entry.terminal.as_ref().map(HostTerminal::wire_kind);
    let (mut status, mut annotation) = session_status(entry);
    let launch_error = entry
        .run
        .launch_error
        .lock()
        .expect("launch-error mutex poisoned")
        .clone();
    let launch_error = launch_error.filter(|error| {
        // An observer can publish its cache after Stop has already committed.
        // Recheck the outcome on the read side instead of relying on cache
        // cleanup or on the observer's earlier eligibility decision. The
        // cache mutex has been released before taking this outcome lock.
        let applicable =
            error.applies_to(&entry.run.outcome.lock().expect("outcome mutex poisoned"));
        if !applicable {
            // That fence may have committed after the first status read.
            // Refresh its status too, so rejecting a stale error cannot lose
            // the new durable failure or stop annotation. No guard is held.
            (status, annotation) = session_status(entry);
        }
        applicable
    });
    // A concurrent observer can retain a fallback after another writer has
    // committed Error. Durable failure still wins, including its stored detail.
    if matches!(status, SessionStatus::Error { .. }) {
        info.status = status;
        info.annotation = annotation;
    } else if let Some(detail) = launch_error {
        info.status = SessionStatus::Error {
            detail: detail.detail,
        };
        info.annotation = None;
    } else {
        info.status = status;
        info.annotation = annotation;
    }
    // A dead terminal can retain its last rendered question. It is no longer a
    // live opportunity to answer, so do not advertise the checkout setting.
    info.claude_trust_prompt &= info.status.is_live();
    info.workspace_trust_prompt &= info.status.is_live();
    info
}

/// Report on a reply that this supervisor creates new terminals in
/// terminal hosts (`SessionInfo::supervisor_creates_host_terminals`), which
/// is always true: every terminal a supervisor creates is a host.
///
/// Still reported because clients read it: the client shows its migration
/// band only for a session whose supervisor reports this, and a supervisor
/// from before terminal hosts, which never sets it, creates tmux terminals.
/// Applied at the two reply boundaries every `SessionInfo` leaves through,
/// the listing (`listing::list_all`) and the single-session replies
/// (`Supervisor::with_checkout_metadata`), rather than stored on entries:
/// it describes the supervisor sending the reply, while a stored copy would
/// describe whichever supervisor built the entry.
pub(crate) fn report_new_terminals(info: &mut SessionInfo) {
    info.supervisor_creates_host_terminals = true;
}

/// What this observation should offer the durable record, or `None` when
/// there is nothing worth telling the store.
///
/// Only the OBSERVATION is decided here; whether it changes anything is
/// [`Transition::apply`]'s call, inside the transaction. Two cases produce
/// nothing at all: a session whose outcome is already terminal (no probe
/// can add to `Interrupted`, `Error`, or an exit that already has its
/// code), and a `Launching` row with no terminal — see `session_status`'s
/// point 4 for why absence of side effects is not evidence of an exit.
pub(crate) fn observation(
    recorded: &LastOutcome,
    live: Option<OwnedTerminal>,
) -> Option<Transition> {
    match live {
        Some(state) if !state.dead => None,
        Some(state) => {
            let exit_code = state.exit_code;
            match recorded {
                // A dead terminal during an ambiguous restart may belong to
                // the previous generation. It cannot establish that this launch
                // ran and exited, so do not offer an exit transition.
                LastOutcome::Launching => None,
                // An already-recorded exit still accepts a code that only
                // now arrived (monotonic enrichment).
                LastOutcome::Exited {
                    exit_code: recorded_code,
                    ..
                } if recorded_code.is_none() && exit_code.is_some() => {
                    Some(Transition::ObservedExit {
                        stopped_before_start: false,
                        exit_code,
                    })
                }
                _ if recorded.is_terminal() => None,
                _ => Some(Transition::ObservedExit {
                    stopped_before_start: false,
                    exit_code,
                }),
            }
        }
        None => matches!(recorded, LastOutcome::Running | LastOutcome::StopRequested).then_some(
            Transition::ObservedExit {
                stopped_before_start: false,
                exit_code: None,
            },
        ),
    }
}

/// Launch evidence and the generation-fenced transition one observer found.
/// The tick, liveness wake and deterministic seam collect these before
/// committing a batch; replies only read the resulting durable and memory state.
pub(crate) struct EntryObservation {
    /// Every applicable launch read succeeded and found no failure after an
    /// owned terminal died, or after its boot ended (including already-exited runs). The caller may
    /// settle future reads only once this entry's terminal outcome is durable.
    pub(crate) settle_launch_reads: bool,
    /// A launch-sentinel, wrapper-failure or interrupted-preparation detail found for this
    /// entry NOW. Outranks ordinary status while its provenance still applies
    /// to the current outcome, whether or not the matching transition commits — see
    /// the generation-local launch-error mirror read by [`entry_info`].
    pub(crate) sentinel: Option<String>,
    /// Whether the cached detail came from terminal-based inference. A Stop may
    /// commit during its reads, so this provenance must survive publication.
    pub(crate) inferred: bool,
    /// The transition this observation wants committed, or `None`
    /// when nothing changed or this supervisor may not record.
    pub(crate) transition: Option<Transition>,
    /// This entry is ALREADY durably `Error`: there is nothing left to
    /// witness, and its launch artifacts are due for the idempotent
    /// cleanup a crash between an earlier commit and its cleanup can
    /// leave behind. The caller skips cleanup once its success latch is set.
    pub(crate) settled_error: bool,
}

/// Explain incomplete setup only after an accepted terminal has stopped.
/// Callers establish dead-or-absent liveness first; an in-progress state is
/// normal while Git or the hook still runs. Durable terminal provenance keeps
/// a preterminal pending create from becoming launch evidence merely because
/// its preparation has not run yet. Membership alone gives no setup duty:
/// borrowers deliberately use the checkout's current contents.
/// Generation matching also prevents a stale polling entry from borrowing a
/// newer restart's accepted terminal as evidence about the old launch. The store
/// reads acceptance and provenance together, so concurrent Delete cannot leave
/// this observer comparing a removed session against its retained origin row.
pub(crate) async fn interrupted_preparation_detail(
    state_dir: &Path,
    store: &SessionStore,
    session_id: &str,
    generation: i64,
) -> anyhow::Result<Option<String>> {
    let Some(origin) = store.preparation_origin(session_id, generation).await? else {
        return Ok(None);
    };
    let path = crate::launch::preparation_state_path(state_dir, &origin.id);
    let detail = match crate::launch::read_preparation_state(&path, &origin.id) {
        Ok(Some(record)) if record.state == crate::launch::PreparationState::Ready => {
            return Ok(None);
        }
        Ok(Some(_)) => "checkout preparation did not finish before its terminal stopped",
        Ok(None) => "checkout preparation evidence is missing after its terminal stopped",
        Err(_) => "checkout preparation evidence is unreadable after its terminal stopped",
    };
    Ok(Some(format!(
        "{detail}; clone and post-clone hook will not be repeated automatically"
    )))
}

/// Classify terminal and launch evidence without committing it.
///
/// The timer and wake share this precedence: durable Error is settled;
/// a shim failure, wrapper failure or incomplete accepted preparation outranks
/// an inferred exit. A pending Stop excludes wrapper and preparation inference
/// because its own kill leaves that same evidence; a direct shim report still
/// wins until Stop's annotated exit commits. A host that cannot be read
/// records nothing at all.
/// Callers batch transitions, mirror committed outcomes and retain uncommitted
/// error details for replies. An unreadable sentinel returns an error so the
/// observer can log and retry without recording a possibly false exit.
/// Successful negative reads can become final only after owned terminal death
/// or proof that this launch's boot ended and durable terminal classification.
/// The caller finishes that contract after committing any proposed outcome.
pub(crate) async fn observe_entry(
    sup: &Supervisor,
    entry: &Arc<SessionEntry>,
) -> anyhow::Result<EntryObservation> {
    let recorded = entry
        .run
        .outcome
        .lock()
        .expect("outcome mutex poisoned")
        .clone();
    let evidence = terminal_evidence(entry.terminal.as_ref());
    if matches!(recorded, LastOutcome::Error { .. }) {
        return Ok(EntryObservation {
            sentinel: None,
            inferred: false,
            transition: None,
            settled_error: true,
            settle_launch_reads: false,
        });
    }
    // An unreadable host records nothing durable, not even a sentinel: the
    // plan's D-16 leaves it to whoever can read it.
    let live = match evidence {
        TerminalEvidence::Owned(state) => Some(state),
        TerminalEvidence::Absent => None,
        TerminalEvidence::Unattributed => {
            return Ok(EntryObservation {
                sentinel: None,
                inferred: false,
                transition: None,
                settled_error: false,
                settle_launch_reads: false,
            });
        }
    };

    // Two different questions, deliberately not one: "no live
    // process" (which a sentinel check needs) and "a terminal that
    // EXISTS and is dead" (which the wrapper-failure classifier
    // needs — see its docs for why an absent one must not qualify).
    let dead_or_absent = live.is_none_or(|state| state.dead);
    let terminal_dead = live.is_some_and(|state| state.dead);

    // A sentinel is READ regardless of whether this supervisor
    // `may_record()` (item 2 of the review-swarm fix batch): a
    // degraded supervisor still has standing to REPORT what it can
    // read, even though it must not WRITE a conclusion it has no
    // standing to store — which is why `sentinel` and `transition`
    // below are set independently.
    let mut settle_launch_reads = false;
    // A failed spec or waypoint read proves nothing either way. The observation
    // still goes ahead, as it did before reads could settle, so the exit is
    // recorded from the terminal; only settlement waits for successful evidence reads.
    let mut launch_evidence_inconclusive = false;
    if sentinel_could_still_apply(&recorded)
        && dead_or_absent
        && !entry
            .run
            .launch_reads_settled
            .load(std::sync::atomic::Ordering::Relaxed)
    {
        let found = read_launch_sentinel(&sup.state_dir, &entry.info.id, entry.generation)
            .await
            .with_context(|| {
                format!("could not read session {}'s launch sentinel", entry.info.id)
            })?;
        let inferred = found.is_none();
        let mut detail = match found {
            Some(detail) => Some(detail),
            None if recorded == LastOutcome::StopRequested => None,
            // The wrapper-failure shape: no sentinel, a terminal that is
            // present and dead, and a launch spec nothing consumed.
            None => match wrapper_failure_detail(
                &sup.state_dir,
                &entry.info.id,
                entry.generation,
                entry.scope.is_some(),
                terminal_dead,
                live.and_then(|state| state.exit_code),
            )
            .await
            {
                Ok(Some(detail)) => Some(append_launch_excerpt(entry, detail)),
                Ok(None) => None,
                Err(_) => {
                    launch_evidence_inconclusive = true;
                    None
                }
            },
        };
        if detail.is_none() && recorded != LastOutcome::StopRequested {
            detail = interrupted_preparation_detail(
                &sup.state_dir,
                &sup.store,
                &entry.info.id,
                entry.generation,
            )
            .await?;
        }
        if let Some(detail) = detail {
            // An in-memory Terminal can precede durable confirmation when
            // create's confirmation write fails. Preserve its accepted terminal
            // address with the error: wrapper failures have no sentinel to
            // keep that provenance alive after the physical terminal
            // disappears.
            let transition = sup.may_record().then(|| Transition::SentinelError {
                inferred,
                detail: detail.clone(),
                address: entry
                    .terminal
                    .as_ref()
                    .map(|terminal| terminal.recorded_address().to_owned()),
            });
            return Ok(EntryObservation {
                sentinel: Some(detail),
                inferred,
                transition,
                settled_error: false,
                settle_launch_reads: false,
            });
        }
        // The shim finishes its evidence writes before its terminal becomes
        // dead. An absent terminal in the same boot is not that proof: the
        // shim may still run. A recorded reboot ends
        // the writers of already-exited runs too; preserve Interrupted as
        // equivalent proof for rows migrated without the new boot-finality bit.
        settle_launch_reads = sup.may_record()
            && !launch_evidence_inconclusive
            && (terminal_dead
                || entry.run.launch_boot_ended
                || matches!(recorded, LastOutcome::Interrupted));
    }

    let transition = if sup.may_record() {
        let mut transition = observation(&recorded, live);
        // Read even when the cached outcome still says Running: a Stop may
        // have committed its intent while this observer awaited evidence.
        // The store chooses whether this exit needs an annotation.
        if let Some(Transition::ObservedExit {
            stopped_before_start: choice,
            ..
        }) = &mut transition
        {
            *choice = stopped_before_start(&sup.state_dir, &entry.info.id, entry.generation).await;
        }
        transition
    } else {
        None
    };
    Ok(EntryObservation {
        sentinel: None,
        inferred: false,
        transition,
        settled_error: false,
        settle_launch_reads,
    })
}

#[cfg(test)]
mod tests {
    use super::super::core::tests::{a_terminal, entry_with};
    use super::*;
    use crate::agent_kind::IntegrationSnapshot;
    use crate::agent_kind::screen_reader::QUIET_SAMPLES_BEFORE_IDLE;
    use crate::service::ticker::ActivitySample;
    use farhelm_proto::AgentKind;

    /// A completed Stop fences out preparation errors published late by the
    /// observer, while unsettled outcomes must still expose a cached failure.
    /// Exercise the reply builder with both cells already populated: relying
    /// on observer cleanup would miss the cache-after-commit publication order.
    #[farhelm_testtrace::test]
    fn cached_launch_error_respects_committed_outcome_fences() {
        let cached = "checkout preparation failed";
        let pending_error = SessionStatus::Error {
            detail: cached.into(),
        };
        let cases = [
            (LastOutcome::Launching, pending_error.clone(), None),
            (LastOutcome::Running, pending_error.clone(), None),
            (LastOutcome::StopRequested, pending_error.clone(), None),
            (
                LastOutcome::Exited {
                    exit_code: Some(1),
                    annotation: None,
                },
                pending_error.clone(),
                None,
            ),
            (LastOutcome::Interrupted, pending_error, None),
            (
                LastOutcome::Exited {
                    exit_code: Some(0),
                    annotation: Some("stopped by user".into()),
                },
                SessionStatus::Exited { exit_code: Some(0) },
                Some("stopped by user"),
            ),
            (
                LastOutcome::Error {
                    detail: "durable failure".into(),
                },
                SessionStatus::Error {
                    detail: "durable failure".into(),
                },
                None,
            ),
        ];
        for (outcome, expected, annotation) in cases {
            let entry = entry_with(None, outcome.clone());
            *entry.run.launch_error.lock().expect("launch-error mutex") =
                Some(super::super::launch_artifacts::LaunchError {
                    detail: cached.into(),
                    inferred: false,
                });
            let reply = entry_info(&entry);
            assert_eq!(reply.status, expected, "recorded outcome: {outcome:?}");
            assert_eq!(
                reply.annotation.as_deref(),
                annotation,
                "recorded outcome: {outcome:?}"
            );
        }
    }

    /// A cached inference can arrive after Stop committed during evidence I/O.
    /// Reply construction must reject it from the current pending state, while
    /// preserving a direct shim report and ordinary degraded-mode inference.
    #[farhelm_testtrace::test]
    fn pending_stop_filters_only_inferred_cached_launch_errors() {
        for outcome in [LastOutcome::Running, LastOutcome::StopRequested] {
            for inferred in [false, true] {
                let entry = entry_with(None, outcome.clone());
                let baseline = entry_info(&entry);
                *entry.run.launch_error.lock().unwrap() =
                    Some(super::super::launch_artifacts::LaunchError {
                        detail: "launch evidence".into(),
                        inferred,
                    });
                let reply = entry_info(&entry);
                if outcome == LastOutcome::StopRequested && inferred {
                    assert_eq!(reply.status, baseline.status);
                    assert_eq!(reply.annotation, baseline.annotation);
                } else {
                    assert_eq!(
                        reply.status,
                        SessionStatus::Error {
                            detail: "launch evidence".into()
                        }
                    );
                }
            }
        }
    }

    /// An entry with a live terminal whose sample cell has been filled in
    /// by hand, standing in for whatever the ticker would have written.
    ///
    /// The counts are set directly rather than by replaying
    /// `ActivitySample::observe`, because these tests are about how the
    /// CLASSIFIER reads a cell; driving them through the sampler would make
    /// every case depend on its change detection as well, which has its own
    /// tests next to it.
    fn entry_sampled(
        kind: AgentKind,
        samples: u64,
        unchanged_streak: u64,
        tail: Option<&str>,
    ) -> SessionEntry {
        let entry = entry_with(Some(a_terminal()), LastOutcome::Running);
        {
            let mut activity = entry.run.activity.lock().expect("activity mutex");
            activity.set_for_test(kind, samples, unchanged_streak, tail);
        }
        SessionEntry {
            snapshot: IntegrationSnapshot {
                kind,
                wrapper_prefix_words: 0,
                resume_template: None,
            },
            ..entry
        }
    }

    /// A Claude Code approval dialog as it renders at the bottom of a
    /// pane: the question, then the numbered answers. Shares one fixture
    /// between the classifier tests here and nothing else — the per-kind
    /// recognition itself is pinned in `agent_kind`, and what this file
    /// tests is that the wiring reaches it at all.
    const CLAUDE_APPROVAL_TAIL: &str = "\
 Bash command
   rm -rf build
 Do you want to proceed?
 ❯ 1. Yes
   2. No
 Esc to cancel · Tab to amend";

    /// A terminal in the given state with no host behind it
    /// (`HostTerminal::fixed_for_test`): running, or dead with `exit_code`.
    /// The classification tests below are about liveness and the durable
    /// record, so they put the same entry on terminals in different states
    /// ([`on`]). Made within a tokio runtime, as such a handle must be.
    fn terminal_in(dead: bool, exit_code: Option<i32>) -> HostTerminal {
        use terminal_host::protocol::ChildExit;
        HostTerminal::fixed_for_test(if dead {
            crate::hosts::Liveness::Exited {
                exit: exit_code.map_or(ChildExit::Signal(9), ChildExit::Code),
            }
        } else {
            crate::hosts::Liveness::Running { child: None }
        })
    }

    /// A terminal whose host has ended, which status reads as no terminal.
    fn gone_terminal() -> HostTerminal {
        HostTerminal::fixed_for_test(crate::hosts::Liveness::Gone)
    }

    /// `entry` on `terminal`: the same cells (outcome, samples), with
    /// `terminal` as its agent terminal.
    fn on(entry: &SessionEntry, terminal: &HostTerminal) -> SessionEntry {
        SessionEntry {
            info: entry.info.clone(),
            terminal: Some(terminal.clone()),
            run: entry.run.clone(),
            session: entry.session.clone(),
            snapshot: entry.snapshot.clone(),
            canonical_cwd: entry.canonical_cwd.clone(),
            generation: entry.generation,
            scope: entry.scope.clone(),
        }
    }

    /// A current Codex interrupt hint is stronger evidence than three quiet
    /// comparisons, but an approval question is stronger still: it tells the
    /// user which interaction is presently required.
    #[farhelm_testtrace::test]
    async fn codex_working_hint_prevents_idle_but_waiting_still_wins() {
        let live = terminal_in(false, None);
        let entry = entry_sampled(
            AgentKind::Codex,
            8,
            QUIET_SAMPLES_BEFORE_IDLE,
            Some("Working (4s • esc to interrupt)\n\n› draft\n\nfooter"),
        );
        let show = |screen: &str| {
            entry
                .run
                .activity
                .lock()
                .expect("activity mutex")
                .set_for_test(AgentKind::Codex, 8, QUIET_SAMPLES_BEFORE_IDLE, Some(screen));
        };
        assert_eq!(session_status(&on(&entry, &live)).0, SessionStatus::Running);

        show(
            "  $ sleep 20\n› 1. Yes, proceed (y)\n  2. No (esc)\n  Press enter to confirm or esc to cancel",
        );
        assert_eq!(session_status(&on(&entry, &live)).0, SessionStatus::Waiting);

        show("Working (4s • esc to interrupt)\n\n› draft\n\nfooter");
        assert_eq!(
            session_status(&on(&entry, &live)).0,
            SessionStatus::Running,
            "once the pending question is gone, the retained work hint resumes winning over idle"
        );
    }

    /// The classification precedence PLAN_M3.md items 2 and 3 define, in
    /// one place: what a live probe says, what the durable record says,
    /// and which wins where. Table-driven because the RELATIONSHIPS are
    /// the contract — each case in isolation looks obvious, and only side
    /// by side do the two inversions stand out (a live pane beating a
    /// recorded outcome, and a recorded outcome beating "no pane found").
    #[farhelm_testtrace::test]
    async fn classification_precedence_between_live_probing_and_the_recorded_outcome() {
        let live = terminal_in(false, None);
        let dead = terminal_in(true, Some(3));
        let empty = gone_terminal();

        // A live pane outranks a stale record: what can still be observed
        // is never overridden by what was once written down. (The entry
        // has never been sampled, so the live answer is the unwatched
        // default — which of the three live statuses it is has its own
        // tests below.)
        assert_eq!(
            session_status(&on(
                &entry_with(Some(a_terminal()), LastOutcome::Launching),
                &live
            )),
            (SessionStatus::Running, None)
        );

        // A dead pane's own code is the answer, and the record supplies
        // the annotation the pane cannot know.
        assert_eq!(
            session_status(&on(
                &entry_with(
                    Some(a_terminal()),
                    LastOutcome::Exited {
                        exit_code: None,
                        annotation: Some("stopped by user".to_string()),
                    }
                ),
                &dead
            )),
            (
                SessionStatus::Exited { exit_code: Some(3) },
                Some("stopped by user".to_string())
            )
        );

        // A dead pane left over from the previous run does not establish
        // that an ambiguous restart ever started; live status must agree
        // with reload instead of attributing the old exit.
        assert_eq!(
            session_status(&on(
                &entry_with(Some(a_terminal()), LastOutcome::Launching),
                &dead
            )),
            (SessionStatus::Unknown, None)
        );

        // No pane to ask: the record answers, and interrupted is NOT
        // flattened into exited-unknown — the whole point of the state.
        assert_eq!(
            session_status(&on(&entry_with(None, LastOutcome::Interrupted), &empty)),
            (SessionStatus::Interrupted, None)
        );
        assert_eq!(
            session_status(&on(
                &entry_with(
                    None,
                    LastOutcome::Exited {
                        exit_code: Some(7),
                        annotation: Some("stopped by user".to_string()),
                    }
                ),
                &empty
            )),
            (
                SessionStatus::Exited { exit_code: Some(7) },
                Some("stopped by user".to_string())
            ),
            "a code and annotation witnessed before the terminal vanished are retained \
             knowledge, not a guess"
        );

        // Nothing observed and nothing recorded: M2's honest fallback.
        assert_eq!(
            session_status(&on(&entry_with(None, LastOutcome::Running), &empty)),
            (SessionStatus::Exited { exit_code: None }, None)
        );

        // The seam PLAN_M3.md item 3 slots into: a recorded error outranks
        // every inference, including a terminal its host calls alive.
        assert_eq!(
            session_status(&on(
                &entry_with(
                    Some(a_terminal()),
                    LastOutcome::Error {
                        detail: "Permission denied".to_string()
                    }
                ),
                &live
            )),
            (
                SessionStatus::Error {
                    detail: "Permission denied".to_string()
                },
                None
            )
        );
    }

    /// What each observation OFFERS the store, which is the half
    /// `session_status` does not decide. Two silences matter more than the
    /// writes: a terminal outcome is not re-observed at all (nothing a
    /// probe can see adds to it), and a `Launching` row with no pane
    /// offers nothing — "no side effects found" is not evidence the agent
    /// ran, and recording an exit for it would claim exactly that
    /// (PLAN_M3.md item 2 sends that row to item 3/6 instead).
    ///
    /// The enrichment case is the one a naive "already terminal, skip it"
    /// rule gets wrong: a death can be seen before its code is (a host
    /// terminal's probe says dead before its exit record arrives), so the
    /// observation that first sees the death can have no code while the
    /// next one does.
    #[farhelm_testtrace::test]
    fn observations_offered_to_the_store_cover_silence_and_enrichment() {
        let dead_with_code = OwnedTerminal {
            dead: true,
            exit_code: Some(3),
        };
        let dead_without_code = OwnedTerminal {
            dead: true,
            exit_code: None,
        };
        let alive = OwnedTerminal {
            dead: false,
            exit_code: None,
        };

        assert_eq!(observation(&LastOutcome::Running, Some(alive)), None);
        assert_eq!(
            observation(&LastOutcome::Running, Some(dead_with_code)),
            Some(Transition::ObservedExit {
                stopped_before_start: false,
                exit_code: Some(3)
            })
        );
        assert_eq!(
            observation(&LastOutcome::Launching, Some(dead_with_code)),
            None,
            "a dead pane during an ambiguous launch cannot establish that the new run exited"
        );
        assert_eq!(
            observation(&LastOutcome::Running, None),
            Some(Transition::ObservedExit {
                stopped_before_start: false,
                exit_code: None
            })
        );
        assert_eq!(
            observation(&LastOutcome::StopRequested, None),
            Some(Transition::ObservedExit {
                stopped_before_start: false,
                exit_code: None
            }),
            "a stop whose terminal vanished still ended; the store decides it was the stop"
        );
        assert_eq!(
            observation(&LastOutcome::Launching, None),
            None,
            "a launch with no side effects has not been shown to have run"
        );
        assert_eq!(
            observation(&LastOutcome::Interrupted, None),
            None,
            "a reboot is never re-observed into something poorer"
        );
        assert_eq!(
            observation(
                &LastOutcome::Exited {
                    exit_code: None,
                    annotation: None
                },
                Some(dead_with_code)
            ),
            Some(Transition::ObservedExit {
                stopped_before_start: false,
                exit_code: Some(3)
            }),
            "a code that only now arrived must still reach the record"
        );
        assert_eq!(
            observation(
                &LastOutcome::Exited {
                    exit_code: Some(3),
                    annotation: None
                },
                Some(dead_without_code)
            ),
            None,
            "a known code is never re-offered to be replaced by a missing one"
        );
    }

    /// The decay rule at both sides of its boundary, counted in the
    /// session's own samples.
    ///
    /// Table-driven for the same reason the precedence test above is: each
    /// case alone looks arbitrary, and it is the RELATIONSHIPS that are the
    /// contract — that the threshold is inclusive, that one silent look
    /// short of it is still `Running`, and that "not watched yet" and
    /// "watched and still" are opposite answers.
    #[farhelm_testtrace::test]
    async fn the_quiet_sample_threshold_decides_running_from_idle_at_both_edges() {
        let live = terminal_in(false, None);

        let one_short = entry_sampled(
            AgentKind::Generic,
            40,
            QUIET_SAMPLES_BEFORE_IDLE - 1,
            Some("working"),
        );
        assert_eq!(
            session_status(&on(&one_short, &live)).0,
            SessionStatus::Running,
            "a screen can repeat for a sample or two while an agent works"
        );

        let exactly_at = entry_sampled(
            AgentKind::Generic,
            40,
            QUIET_SAMPLES_BEFORE_IDLE,
            Some("working"),
        );
        assert_eq!(
            session_status(&on(&exactly_at, &live)).0,
            SessionStatus::Idle,
            "the threshold is inclusive: the Nth silent look is the one that decides"
        );

        let long_quiet = entry_sampled(AgentKind::Generic, 4_000, 3_999, Some("a shell prompt"));
        assert_eq!(
            session_status(&on(&long_quiet, &live)).0,
            SessionStatus::Idle
        );
    }

    /// The bug this classifier's shape exists to prevent: a busy session on
    /// a crowded host must not decay just because the sampler gets around
    /// to it less often.
    ///
    /// With more live sessions than `SAMPLE_TAIL_BUDGET`, a session is
    /// sampled once every `ceil(live / budget)` ticks — at 49 live sessions
    /// that is every 8 seconds at the production cadence, which is past any
    /// wall-clock window a reviewer would call reasonable. A clock-based
    /// classifier therefore reports a continuously-changing pane as `Idle`
    /// purely because the HOST is busy, and the user watching that column
    /// sees their fleet go idle as it grows.
    ///
    /// Expressed as a pure classifier case because that is where the
    /// property lives: no elapsed time appears anywhere below, which is
    /// exactly the point — the same cell means the same thing at any
    /// cadence, so there is nothing for a population to change. The
    /// matching end-to-end case lives beside the sampler in `ticker`.
    #[farhelm_testtrace::test]
    async fn a_session_that_changes_at_every_sample_stays_running_at_any_cadence() {
        let live = terminal_in(false, None);
        // Sampled hundreds of times over an arbitrarily long life, and
        // never once found unchanged: the shape of a pane that is printing
        // whenever anybody looks at it, however rarely that is.
        let rarely_sampled = entry_sampled(AgentKind::Generic, 500, 0, Some("tick 500"));
        assert_eq!(
            session_status(&on(&rarely_sampled, &live)).0,
            SessionStatus::Running,
            "a pane that changed at every one of its own samples is working, whatever the \
             interval between them was"
        );
    }

    /// A session nothing has sampled twice yet is `Running`, not `Idle`.
    ///
    /// This is the documented pre-first-sample state for a NEW launch. A
    /// reloaded entry carries a separate provisional marker and keeps the
    /// helm's last answer until this process has evidence. Painting every
    /// fresh launch idle before its first comparison would still make the
    /// column systematically wrong.
    ///
    /// Both sub-two counts are pinned, because they are different facts:
    /// zero means the ticker has not reached this session, one means it
    /// has but has nothing to compare against yet.
    #[farhelm_testtrace::test]
    async fn a_session_that_has_not_been_watched_twice_is_running_rather_than_idle() {
        let live = terminal_in(false, None);
        for samples in [0, 1] {
            let entry = entry_sampled(AgentKind::Generic, samples, 0, None);
            assert_eq!(
                session_status(&on(&entry, &live)).0,
                SessionStatus::Running,
                "an unwatched live session is one that just launched, not one at rest \
                 ({samples} samples)"
            );
        }
    }

    /// A reloaded pane must not erase the helm's previous idle or waiting
    /// answer with `Running` before this supervisor has seen its screen.
    /// A changed screen and a settled quiet screen each end that gap, while
    /// a dead pane uses the lifecycle result immediately.
    #[farhelm_testtrace::test]
    async fn a_reloaded_live_pane_waits_for_status_evidence() {
        let live = terminal_in(false, None);
        let mut entry = entry_with(Some(a_terminal()), LastOutcome::Running);
        entry.run.activity = ActivitySample::reloaded();
        assert_eq!(session_status(&on(&entry, &live)).0, SessionStatus::Unknown);

        {
            let mut activity = entry.run.activity.lock().expect("activity mutex");
            activity.observe("unchanged".to_string());
            activity.observe("unchanged".to_string());
        }
        assert_eq!(session_status(&on(&entry, &live)).0, SessionStatus::Unknown);

        entry
            .run
            .activity
            .lock()
            .expect("activity mutex")
            .observe("changed".to_string());
        assert_eq!(session_status(&on(&entry, &live)).0, SessionStatus::Running);

        let mut quiet = entry_with(Some(a_terminal()), LastOutcome::Running);
        quiet.run.activity = ActivitySample::reloaded();
        for _ in 0..=QUIET_SAMPLES_BEFORE_IDLE {
            quiet
                .run
                .activity
                .lock()
                .expect("activity mutex")
                .observe("still".to_string());
        }
        assert_eq!(session_status(&on(&quiet, &live)).0, SessionStatus::Idle);

        let waiting = entry_sampled(AgentKind::Claude, 1, 0, Some(CLAUDE_APPROVAL_TAIL));
        waiting
            .run
            .activity
            .lock()
            .expect("activity mutex")
            .startup_provisional = true;
        assert_eq!(
            session_status(&on(&waiting, &live)).0,
            SessionStatus::Waiting
        );

        let dead = terminal_in(true, Some(7));
        assert_eq!(
            session_status(&on(&entry, &dead)).0,
            SessionStatus::Exited { exit_code: Some(7) }
        );
    }

    /// Screen reading is actually WIRED: an integrated session whose sampled
    /// tail carries its agent's approval prompt classifies `Waiting`, and
    /// the same tail on a session with no integration does not.
    ///
    /// The negative half is what makes this a wiring test rather than a
    /// duplicate of `agent_kind`'s own recognition tests: it pins that the
    /// per-kind knowledge is reached THROUGH the snapshot, so a generic
    /// session cannot accidentally inherit another agent's heuristics.
    #[farhelm_testtrace::test]
    async fn an_integrated_sessions_prompt_screen_reads_waiting() {
        let live = terminal_in(false, None);
        // Quiet by the baseline rule — a pending approval is exactly the
        // case where nothing is being printed — so the `Waiting` below can
        // only have come from the screen reader.
        let claude = entry_sampled(AgentKind::Claude, 9, 5, Some(CLAUDE_APPROVAL_TAIL));
        assert_eq!(
            session_status(&on(&claude, &live)).0,
            SessionStatus::Waiting
        );

        let generic = entry_sampled(AgentKind::Generic, 9, 5, Some(CLAUDE_APPROVAL_TAIL));
        assert_eq!(
            session_status(&on(&generic, &live)).0,
            SessionStatus::Idle,
            "a session with no integration keeps the generic baseline, whatever its screen says"
        );

        // And an integrated session that is merely working is left alone.
        let busy = entry_sampled(
            AgentKind::Claude,
            9,
            0,
            Some("⏺ Reading src/main.rs (120 lines)"),
        );
        assert_eq!(session_status(&on(&busy, &live)).0, SessionStatus::Running);
    }

    /// A stored waiting reading never outlives the terminal it was read
    /// from: a dead terminal reports its exit whatever the last screen
    /// showed.
    ///
    /// Why it matters: the last capture of an agent that exited mid-question
    /// still shows the question, and reporting a finished session as blocked
    /// on a human is the one wrong status that sends the user to answer
    /// nothing. Liveness is decided before any reading is consulted.
    #[farhelm_testtrace::test]
    async fn a_dead_terminal_is_never_reported_waiting_on_its_last_screen() {
        let entry = entry_sampled(AgentKind::Claude, 9, 5, Some(CLAUDE_APPROVAL_TAIL));
        assert_eq!(
            session_status(&on(&entry, &terminal_in(false, None))).0,
            SessionStatus::Waiting,
            "premise: the screen reads waiting while the pane lives"
        );
        assert_eq!(
            session_status(&on(&entry, &terminal_in(true, Some(0)))).0,
            SessionStatus::Exited { exit_code: Some(0) }
        );
    }
}

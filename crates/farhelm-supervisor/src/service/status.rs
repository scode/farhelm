//! Reading a session's current state back out: what its status is, what a
//! restart would offer it, and the `SessionInfo` every reply is built
//! from.
//!
//! These are the read-out half of the state model the `service` module doc
//! describes — the half that answers "what happened to this session?" from
//! two inputs: the entry's own durable record and a pane-state map the
//! CALLER's liveness probe returned.
//!
//! The guarantee that shape buys holds for the SYNCHRONOUS
//! classification core — `session_status`, `session_restart_offer`, and
//! `entry_info`, the three a reply is actually built from. None of those
//! consults the session map, and none issues an I/O round trip of its own;
//! each is a pure read of the entry plus the map it was handed. That is
//! what lets a list pass probe tmux ONCE for a whole page and then
//! classify every entry from the result, and what makes the
//! classification testable against hand-built entries with no tmux, no
//! store, and no supervisor at all.
//!
//! PLAN_M6_75.md item 2's activity classification did not weaken that: the
//! sampler's [`ActivitySample`](crate::service::ticker::ActivitySample)
//! rides the ENTRY, so "which live status" is answered from the same two
//! inputs everything else here is. What the classifier does add is a
//! reading of the monotonic clock, which is why `session_status` is a thin
//! shell over `session_status_at`.
//!
//! Three claims a reader might over-generalize from that, all FALSE, and
//! the difference matters to anyone reasoning about what may run where:
//!
//! - "Nothing here does I/O" — no. `dead_pane_exit_code` takes a
//!   `&Supervisor` precisely because it MUST ask tmux, and `observe_entry`
//!   reads launch sentinels off disk. Both are `async` and neither belongs
//!   inside a lock hold; they sit here because they answer the same
//!   question, not because they share the core's constraints.
//! - "Lock-free" — no. `session_status` and `session_restart_offer` take
//!   the per-ENTRY `outcome`, `activity` and `capture` mutexes (and
//!   `entry_info` takes all three, through them). Those are leaf mutexes
//!   held across no await, so they are safe to call from anywhere, but
//!   they are locks.
//! - "Uniform signature" — no. `session_restart_offer` needs no pane map
//!   at all, and the two functions above take the supervisor. The
//!   `&SessionEntry` + pane-map shape describes the core, not every symbol
//!   in the file.
//!
//! The precedence rules these encode are the contract PLAN_M3.md items 2
//! and 3 define, and the full order is: a durably recorded **error**
//! outranks everything, including a pane tmux still calls alive (a failed
//! exec is a fact no probe can discover); then a launch sentinel found
//! THIS pass, for the same reason, whether or not it could also be
//! committed; then a live pane, which no non-error stored outcome may
//! override; then, with no pane to ask, the stored outcome, which beats
//! the blanket exited-unknown fallback. `session_status` and
//! `observe_entry` carry the two halves of that order and must agree.
//! Keeping them in one module is what makes "a `SessionRenamed` reply
//! describes a session exactly as `ListSessions` would" a property of the
//! code rather than a promise maintained by hand.

use super::core::{SessionEntry, Supervisor};
use super::launch_artifacts::{
    read_launch_sentinel, sentinel_could_still_apply, wrapper_failure_detail,
};
use super::terminals::{Terminal, tabs_from_pane_states};
use super::ticker::TICKER_INTERVAL;
use crate::store::{LastOutcome, Transition};
use crate::tmux::PaneState;
use anyhow::Context;
use farhelm_proto::{RestartOffer, SessionInfo, SessionStatus, TabInfo};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::warn;

/// How long a live session's screen may sit unchanged before it stops
/// being called `Running`.
///
/// Stated as a multiple of the sampler's own cadence
/// ([`TICKER_INTERVAL`]) rather than as an independent number, because
/// there is no such thing as recency finer than the cadence that measures
/// it. Three intervals is the smallest value that survives the ways a
/// genuinely working agent goes momentarily still:
///
/// - Change is only ever established by COMPARING two consecutive samples
///   (`ActivitySample::observe`), so the freshest `last_change` a busy
///   session can possibly show is already up to one interval old. A
///   one-interval window would therefore flip sessions to `Idle` purely on
///   sampling phase.
/// - An agent between tool calls, or one whose spinner happens to render
///   identically twice, produces a tick or two of no visible change while
///   very much running.
/// - The tail budget (`SAMPLE_TAIL_BUDGET`) stretches the effective
///   cadence once a host has many live sessions, which pushes in the same
///   direction.
///
/// Nothing wants it much larger: the helm drains every ~3 seconds, so a
/// window of tens of seconds would make `Idle` an announcement that the
/// agent stopped a while ago rather than that it is at rest, and `Idle` is
/// half of what the status column is for.
///
/// The coupling is one-directional and worth naming: `SupervisorSeams`
/// lets tests SHORTEN the ticker interval, which only makes classification
/// fresher inside this same window. A supervisor configured with an
/// interval LONGER than this window would report every session `Idle`, so
/// that combination is not a supported one.
const OUTPUT_RECENCY_WINDOW: Duration = Duration::from_secs(3 * TICKER_INTERVAL.as_secs());

/// Compute one session's liveness for a `ListSessions` reply. tmux is the
/// truth (module docs); this function only ever reports what it can
/// actually observe, never a guess.
///
/// Three cases all collapse into the same honest `Exited { exit_code:
/// None }` rather than assuming alive:
/// - no terminal at all (the restart-gap entry);
/// - this pane id is entirely absent from `pane_states` (removed mid-
///   query, or never existed on this server at all);
/// - this pane id IS present, but for a DIFFERENT session name than the
///   one this entry remembers creating it under. Pane ids reset to `%0`
///   on a fresh tmux server (`PaneState::session_name`'s own docs), so a
///   stale, never-reloaded entry's pane id can be silently recycled by an
///   unrelated NEW session after a server restart; matching pane id alone
///   would let that entry inherit the new session's liveness. Requiring
///   BOTH identifiers to agree is also what a tmux-side rename of the
///   session name (a rare, deliberately-provoked edge case, not a normal
///   product flow) trips: this function has no positive way to confirm
///   the renamed pane is still "the same session" rather than tmux having
///   handed that pane to something else entirely, so it reports the same
///   honest `Exited` rather than guessing either way.
///
/// Only a pane found under BOTH its remembered pane id and its remembered
/// tmux session name gets to decide live-versus-`Exited` from tmux's own
/// dead flag and status.
///
/// ## Classification precedence (PLAN_M3.md items 2 and 3)
///
/// As of M3 the live probe is no longer the only input: the session's
/// durable last-known outcome answers the questions a vanished tmux
/// cannot. The order below is the precedence, and it is deliberate:
///
/// 1. A recorded **error** — the launch shim's exec-failure sentinel —
///    outranks every inference, because "the agent never started" is a
///    fact about THIS launch that no amount of pane probing can discover
///    (an unexec'd command leaves an ordinary dead pane behind, exactly
///    like a command that ran and exited). PLAN_M3.md item 3 owns the
///    READER that ever writes this state; this PR only makes sure it
///    already sits above the inference so item 3 has nothing to
///    restructure. **The sentinel is deliberately not read here.**
/// 2. A live pane decides live-versus-`Exited` exactly as M2 did — a
///    stored outcome never overrides something still observable. What the
///    record still contributes to a DEAD pane is what the pane cannot
///    hold: the stop annotation, and an exit code the pane has already
///    forgotten (`known code wins`, matching the store's own monotonic
///    enrichment rule — tmux publishes `pane_dead` before
///    `pane_dead_status` is readable, so the live reading can be the
///    poorer of the two).
/// 3. With no pane to ask, the recorded outcome speaks: `Interrupted`
///    (the reboot conversion) and `Exited` (a previously witnessed exit,
///    with the code and annotation it was witnessed with) are RETAINED
///    KNOWLEDGE, not guesses, and outrank M2's blanket exited-unknown.
/// 4. A `Launching` row with no pane is `Unknown`, not `Exited`: SPEC.md's
///    exited means the agent RAN, and a launch whose side effects were
///    never found has not established that. It stays pending for item 3's
///    sentinel (error) or item 6's reservation (retry) to resolve.
/// 5. Anything else with no pane — a stored `LastOutcome::Running` (the
///    durable record's own vocabulary, not the wire status that now
///    shares its name), or a stop whose sweep is in flight — falls back
///    to M2's honest `Exited { exit_code: None }`.
///
/// ## Which live status (PLAN_M6_75.md item 2)
///
/// Rule 2 above says a live pane wins; [`live_status`] says which of the
/// three live statuses it wins WITH. That split is deliberate and is where
/// the milestone's whole heuristic lives: liveness is observed (tmux's
/// pane is not dead — a fact), while running-versus-waiting-versus-idle is
/// inferred from sampled screens (a guess, cosmetic by SPEC.md's own
/// status rule). Nothing in the precedence above depends on which one it
/// is, so a wrong guess can never promote or demote a session across the
/// boundary that actually matters.
///
/// The annotation returned alongside the status is SPEC.md's user-legible
/// qualifier ("stopped by user"), which lives with the recorded outcome
/// and therefore survives restarts and reboots. It is returned only for a
/// status that ends up `Exited`: a session that has since been relaunched
/// into a live pane must not still be labelled with how its PREVIOUS run
/// ended.
pub(crate) fn session_status(
    entry: &SessionEntry,
    pane_states: &HashMap<String, PaneState>,
) -> (SessionStatus, Option<String>) {
    session_status_at(entry, pane_states, Instant::now())
}

/// [`session_status`] with the clock supplied.
///
/// The only impurity classification has is "how long ago was that", so it
/// is the only thing lifted into a parameter — the same seam
/// `ActivitySample::observe` takes for the same reason. A test can then
/// place a sample either side of [`OUTPUT_RECENCY_WINDOW`] exactly, rather
/// than sleeping out a real window and hoping the runner cooperated.
///
/// `now` must come from the same monotonic clock the sampler wrote with
/// (`Instant::now`), which is why this is private: a caller free to pass
/// an arbitrary instant is a caller free to make recency meaningless.
fn session_status_at(
    entry: &SessionEntry,
    pane_states: &HashMap<String, PaneState>,
    now: Instant,
) -> (SessionStatus, Option<String>) {
    // The guard is held across the whole match rather than cloned out of:
    // this function is synchronous (no await can intervene) and every arm
    // only reads, so the clone would have bought nothing but an allocation
    // on the hottest path the list reply has.
    let recorded = entry.outcome.lock().expect("outcome mutex poisoned");
    let live = entry.terminal.as_ref().and_then(|terminal| {
        pane_states
            .get(&terminal.pane)
            .filter(|state| state.session_name == terminal.tmux_name)
    });
    match (&*recorded, live) {
        (LastOutcome::Error { detail }, _) => (
            SessionStatus::Error {
                detail: detail.clone(),
            },
            None,
        ),
        // A live pane, and no annotation with it: an annotation describes
        // how a run ENDED, and this one has not.
        (_, Some(state)) if !state.dead => (live_status(entry, now), None),
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

/// Which of the three live statuses a session with a living pane gets
/// (PLAN_M6_75.md item 2).
///
/// Two stages, and the order matters:
///
/// 1. **The generic baseline** is output recency and nothing else. A
///    screen that changed within [`OUTPUT_RECENCY_WINDOW`] is `Running`; a
///    screen that has sat still longer is `Idle`. This works for every
///    agent, including one this build has never heard of, because "the
///    terminal is producing output" needs no vendor knowledge.
/// 2. **Per-kind sharpening** may then promote that to `Waiting` by
///    recognizing this agent's own question or approval shape in the
///    sampled tail (`AgentIntegration::sharpen`). Waiting is not derivable
///    from recency — an agent blocked on an approval and an agent that has
///    finished look identical to a change detector, which is precisely why
///    the two stages exist.
///
/// ## The pre-first-sample state is `Running`, on purpose
///
/// Fewer than two samples means no comparison has been made yet, so there
/// is no recency to read: `last_change` is `None` because nothing has been
/// WATCHED, not because the pane is still. `Running` is the honest reading
/// of that — a session with a live pane and no history is one that just
/// launched, and an agent that just launched is working — and it is also
/// the reading that fails safely, since the alternative would paint every
/// session `Idle` for the first seconds of its life and again after every
/// supervisor restart. The distinction between "unwatched" and "watched
/// and quiet" is exactly what `ActivitySample::samples` exists to keep
/// (see its own docs).
///
/// ## Bounds this is deliberately allowed to violate cosmetically
///
/// A session that never gets sampled at all — because tmux is unreachable
/// on every tick — stays `Running` forever. That is the same honest
/// "nothing has been observed" answer as the launch case, and it costs a
/// wrong badge on a supervisor that already cannot talk to its terminals.
///
/// Runs under the entry's `activity` mutex, which is a leaf lock held
/// across no await. The sharpener is called INSIDE that hold rather than
/// after cloning the tail out: a tail is up to `SAMPLE_TAIL_BYTES` and
/// this runs once per session per reply, so cloning it would add a
/// kilobytes-per-row allocation to the list path to avoid holding an
/// uncontended lock for a substring search.
fn live_status(entry: &SessionEntry, now: Instant) -> SessionStatus {
    let activity = entry.activity.lock().expect("activity mutex poisoned");
    let baseline = match activity.last_change {
        _ if activity.samples < 2 => SessionStatus::Running,
        Some(at) if now.saturating_duration_since(at) <= OUTPUT_RECENCY_WINDOW => {
            SessionStatus::Running
        }
        _ => SessionStatus::Idle,
    };
    let (Some(integration), Some(tail)) = (entry.snapshot.integration(), activity.tail.as_deref())
    else {
        return baseline;
    };
    let sharpened = integration.sharpen(baseline.clone(), tail);
    live_only(baseline, sharpened)
}

/// The guard that makes "a sharpener may not invent liveness" structural
/// rather than a promise: `sharpened` is taken only when it is itself a
/// live status, and otherwise the baseline stands.
///
/// A sharpener looks at a SCREEN, and a screen is not evidence about a
/// process — a pane can render "Process exited" while the agent is very
/// much alive, and tmux is the only thing entitled to say otherwise. So no
/// amount of vendor-shape matching can move a session across the one
/// distinction in this enum consumers may branch on
/// ([`SessionStatus::is_live`]); the worst a wrong sharpener can do is
/// pick the wrong one of three cosmetic labels.
///
/// Enforced here rather than trusted to each implementation because the
/// cost of the two mistakes is wildly asymmetric, and because this is the
/// one place every implementation's answer passes through.
fn live_only(baseline: SessionStatus, sharpened: SessionStatus) -> SessionStatus {
    if sharpened.is_live() {
        sharpened
    } else {
        baseline
    }
}

/// What restarting this session would do to its conversation, computed
/// fresh from the snapshot and whatever identity is DURABLY claimed right
/// now.
///
/// Recomputed on every reply for the same reason `status` is: a capture
/// pass can upgrade a session from `FreshOnly` to `Resume` at any moment,
/// so the value stored in `SessionEntry::info` at create or reload is a
/// starting point rather than an answer. Reads only the COMMITTED identity
/// (`CaptureState::committed_conversation`), which is what keeps the offer
/// from promising a resume that no stored value could fill.
fn session_restart_offer(entry: &SessionEntry) -> RestartOffer {
    let capture = entry.capture.lock().expect("capture mutex poisoned");
    entry
        .snapshot
        .restart_offer(capture.committed_conversation())
}

/// One entry as a reply must describe it: the stored metadata plus the
/// three fields that are NEVER stored and are therefore recomputed on
/// every reply — live-probed `status` (with its annotation), rediscovered
/// `tabs`, and a freshly derived `restart_offer`.
///
/// The single place that shape is defined, shared by `ListSessions` and by
/// the single-session replies that must match it (`SessionRenamed`, whose
/// own protocol docs promise a `SessionInfo` "built the same way
/// `ListSessions` builds one"). Two copies would drift, and the drift
/// would be invisible: both would still be `SessionInfo`s, differing only
/// in which fields told the truth.
///
/// `sentinel` is a launch-sentinel (or wrapper-failure) detail the CALLER
/// found for this entry in the pass it is replying from, and it OUTRANKS
/// `session_status` — that is the whole point of PLAN_M3.md item 3's
/// write-inability note: a failed exec is not something a pane can show,
/// so a reply must surface it whether or not the transition could also be
/// committed durably this pass. Callers that have not looked pass `None`.
///
/// `pane_states` must be the map the caller's own liveness probe returned;
/// an empty map is correct only for an entry with no terminal (the restart
/// gap), whose status comes entirely from its recorded outcome.
pub(crate) fn entry_info(
    entry: &SessionEntry,
    pane_states: &HashMap<String, PaneState>,
    sentinel: Option<&str>,
) -> SessionInfo {
    let mut info = entry.info.clone();
    info.restart_offer = session_restart_offer(entry);
    // Tabs are not stored anywhere at all (`SessionInfo::tabs`), so this
    // rediscovery IS the tab list. A terminal-less entry has no tmux
    // session and therefore no tabs, which the empty default states
    // honestly.
    info.tabs = entry
        .terminal
        .as_ref()
        .map(|terminal| {
            tabs_from_pane_states(pane_states, &terminal.tmux_name)
                .into_iter()
                .map(|tab| TabInfo { id: tab.id })
                .collect()
        })
        .unwrap_or_default();
    match sentinel {
        Some(detail) => {
            info.status = SessionStatus::Error {
                detail: detail.to_string(),
            };
            info.annotation = None;
        }
        None => {
            let (status, annotation) = session_status(entry, pane_states);
            info.status = status;
            info.annotation = annotation;
        }
    }
    info
}

/// The exit code tmux still holds for `terminal`'s pane, if the pane is
/// dead and tmux could reduce its death to one.
///
/// `pane_states`, not `pane_process`: the latter answers "is it dead, and
/// what pid did it have", and only the former carries
/// `#{pane_dead_status}` at all. Used by `StopSession` at both of its
/// exit-recording moments — a stop that found the agent already gone, and
/// a stop whose kill sweep just finished — because in both the code is
/// worth keeping for exactly as long as the pane survives to hold it, and
/// nothing else will look again.
///
/// A failed query is logged and degrades to `None` rather than failing the
/// stop: it costs the exit code, never the annotation, and the store's
/// monotonic enrichment lets a later list fill the code in.
pub(crate) async fn dead_pane_exit_code(
    sup: &Supervisor,
    terminal: Option<&Terminal>,
    session_id: &str,
) -> Option<i32> {
    let terminal = terminal?;
    match sup.tmux.pane_states().await {
        Ok(states) => states
            .get(&terminal.pane)
            .filter(|state| state.session_name == terminal.tmux_name && state.dead)
            .and_then(|state| state.exit_code),
        Err(e) => {
            warn!(
                session = %session_id, error = %format!("{e:#}"),
                "could not read the pane's exit code; recording the outcome without one"
            );
            None
        }
    }
}

/// What this observation should offer the durable record, or `None` when
/// there is nothing worth telling the store.
///
/// Only the OBSERVATION is decided here; whether it changes anything is
/// [`Transition::apply`]'s call, inside the transaction. Two cases produce
/// nothing at all: a session whose outcome is already terminal (no probe
/// can add to `Interrupted`, `Error`, or an exit that already has its
/// code), and a `Launching` row with no pane — see `session_status`'s
/// point 4 for why absence of side effects is not evidence of an exit.
pub(crate) fn observation(recorded: &LastOutcome, live: Option<&PaneState>) -> Option<Transition> {
    match live {
        Some(state) if !state.dead => None,
        Some(state) => {
            let exit_code = state.exit_code;
            match recorded {
                // An already-recorded exit still accepts the code tmux may
                // only now be able to report (monotonic enrichment).
                LastOutcome::Exited {
                    exit_code: recorded_code,
                    ..
                } if recorded_code.is_none() && exit_code.is_some() => {
                    Some(Transition::ObservedExit { exit_code })
                }
                _ if recorded.is_terminal() => None,
                _ => Some(Transition::ObservedExit { exit_code }),
            }
        }
        None => matches!(recorded, LastOutcome::Running | LastOutcome::StopRequested)
            .then_some(Transition::ObservedExit { exit_code: None }),
    }
}

/// What one pre-reply observation of a session concluded — the
/// per-entry half of a `ListSessions` pass, hoisted out so a
/// single-session reply can reach the same conclusions (PLAN_M5.md
/// item 3's `SessionRenamed`, whose `SessionInfo` must be built the
/// way a list builds one).
pub(crate) struct EntryObservation {
    /// A launch-sentinel or wrapper-failure detail found for this
    /// entry NOW. Outranks whatever `session_status` would compute,
    /// whether or not the matching transition also commits — see
    /// [`entry_info`]'s `sentinel` parameter.
    pub(crate) sentinel: Option<String>,
    /// The transition this observation wants committed, or `None`
    /// when nothing changed or this supervisor may not record.
    pub(crate) transition: Option<Transition>,
    /// This entry is ALREADY durably `Error`: there is nothing left to
    /// witness, and its launch artifacts are due for the idempotent
    /// cleanup a crash between an earlier commit and its cleanup can
    /// leave behind.
    pub(crate) settled_error: bool,
}

/// Look at one entry the way a `ListSessions` pass looks at it:
/// classify what its pane and launch artifacts say, without
/// committing anything.
///
/// Extracted from that pass rather than reimplemented, and shared
/// with it, because the precedence here is subtle and duplicating it
/// would eventually mean two different answers to "what happened to
/// this session" depending on which request asked. The order is
/// itself the contract (PLAN_M3.md items 2, 3 and 4): an entry
/// already durably `Error` is settled; otherwise a launch sentinel —
/// or the wrapper-failure shape that stands in for one — outranks
/// every inference, because a failed exec leaves an ordinary dead
/// pane that no probe can tell from a command that ran and finished;
/// only then does the plain pane observation apply.
///
/// Deliberately does NOT commit: the list pass batches every entry's
/// transition into ONE transaction, and taking that apart per entry
/// would turn one poll into a write per session. Callers commit what
/// they collect (`SessionStore::transition_many`) and then mirror
/// what it reports.
///
/// An unreadable sentinel is an `Err`, never a fall-through: basing a
/// reply on an inference the unreadable file might contradict is
/// exactly the silent-wrong-answer this refuses to give. The error
/// already names the session, so callers report it verbatim.
pub(crate) async fn observe_entry(
    sup: &Supervisor,
    entry: &Arc<SessionEntry>,
    pane_states: &HashMap<String, PaneState>,
) -> anyhow::Result<EntryObservation> {
    let recorded = entry
        .outcome
        .lock()
        .expect("outcome mutex poisoned")
        .clone();
    // Borrowed out of the caller's map rather than cloned: this runs once
    // per entry on the polling path, and the pane state is only ever read.
    let live: Option<&PaneState> = entry.terminal.as_ref().and_then(|terminal| {
        pane_states
            .get(&terminal.pane)
            .filter(|state| state.session_name == terminal.tmux_name)
    });
    // Two different questions, deliberately not one: "no live
    // process" (which a sentinel check needs) and "a pane that
    // EXISTS and is dead" (which the wrapper-failure classifier
    // needs — see its docs for why an absent pane must not qualify).
    let dead_or_absent = live.is_none_or(|state| state.dead);
    let pane_dead = live.is_some_and(|state| state.dead);

    if matches!(recorded, LastOutcome::Error { .. }) {
        return Ok(EntryObservation {
            sentinel: None,
            transition: None,
            settled_error: true,
        });
    }

    // A sentinel is READ regardless of whether this supervisor
    // `may_record()` (item 2 of the review-swarm fix batch): a
    // degraded supervisor still has standing to REPORT what it can
    // read, even though it must not WRITE a conclusion it has no
    // standing to store — which is why `sentinel` and `transition`
    // below are set independently.
    if sentinel_could_still_apply(&recorded) && dead_or_absent {
        let found = read_launch_sentinel(&sup.state_dir, &entry.info.id, entry.generation)
            .await
            .with_context(|| {
                format!("could not read session {}'s launch sentinel", entry.info.id)
            })?;
        let detail = match found {
            Some(detail) => Some(detail),
            // The wrapper-failure shape: no sentinel, a pane that is
            // present and dead, and a launch spec nothing consumed.
            None => {
                wrapper_failure_detail(
                    &sup.state_dir,
                    &entry.info.id,
                    entry.generation,
                    entry.scope.is_some(),
                    pane_dead,
                )
                .await
            }
        };
        if let Some(detail) = detail {
            // No pane to rediscover here (unlike `reload_sessions`'s
            // by-name search): callers only visit sessions this
            // process already tracks a `Terminal` for or explicitly
            // does not, so there is nothing new for this transition
            // to record beyond the outcome itself.
            let transition = sup.may_record().then(|| Transition::SentinelError {
                detail: detail.clone(),
                pane: None,
            });
            return Ok(EntryObservation {
                sentinel: Some(detail),
                transition,
                settled_error: false,
            });
        }
    }

    let transition = if sup.may_record() {
        observation(&recorded, live)
    } else {
        None
    };
    Ok(EntryObservation {
        sentinel: None,
        transition,
        settled_error: false,
    })
}

#[cfg(test)]
mod tests {
    use super::super::core::tests::{a_terminal, entry_with};
    use super::*;
    use crate::agent_kind::IntegrationSnapshot;
    use farhelm_proto::AgentKind;

    /// An entry with a live terminal whose sample cell has been filled in
    /// by hand, standing in for whatever the ticker would have written.
    ///
    /// `samples` and `last_change` are set directly rather than by
    /// replaying `ActivitySample::observe`, because these tests are about
    /// how the CLASSIFIER reads a cell, and driving it through the sampler
    /// would make every case depend on the sampler's own change detection
    /// as well.
    fn entry_sampled(
        kind: AgentKind,
        samples: u64,
        last_change: Option<Instant>,
        tail: Option<&str>,
    ) -> SessionEntry {
        let entry = entry_with(Some(a_terminal()), LastOutcome::Running);
        {
            let mut activity = entry.activity.lock().expect("activity mutex");
            activity.samples = samples;
            activity.last_change = last_change;
            activity.tail = tail.map(str::to_string);
        }
        SessionEntry {
            snapshot: IntegrationSnapshot {
                kind,
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
⏺ Bash(rm -rf build)
╭───────────────────────────────────────────────╮
│ Do you want to run this command?              │
│                                               │
│ ❯ 1. Yes                                      │
│   2. Yes, and don't ask again this session    │
│   3. No, and tell Claude what to do instead   │
╰───────────────────────────────────────────────╯";

    /// A `pane_states` map containing exactly [`a_terminal`]'s pane in the
    /// given state.
    ///
    /// Unmarked and at window index 0, which is what an ordinary agent
    /// window looked like before markers existed — the classification
    /// tests below are about liveness and the durable record, and read
    /// neither the tab nor the agent marker.
    fn pane_map(dead: bool, exit_code: Option<i32>) -> HashMap<String, PaneState> {
        let state = PaneState::for_test("fh-1", "%0", "@0");
        let state = if dead {
            state.dead_with(exit_code)
        } else {
            state
        };
        HashMap::from([("%0".to_string(), state)])
    }

    /// The classification precedence PLAN_M3.md items 2 and 3 define, in
    /// one place: what a live probe says, what the durable record says,
    /// and which wins where. Table-driven because the RELATIONSHIPS are
    /// the contract — each case in isolation looks obvious, and only side
    /// by side do the two inversions stand out (a live pane beating a
    /// recorded outcome, and a recorded outcome beating "no pane found").
    #[test]
    fn classification_precedence_between_live_probing_and_the_recorded_outcome() {
        let live = pane_map(false, None);
        let dead = pane_map(true, Some(3));
        let empty = HashMap::new();

        // A live pane outranks a stale record: what can still be observed
        // is never overridden by what was once written down. (The entry
        // has never been sampled, so the live answer is the unwatched
        // default — which of the three live statuses it is has its own
        // tests below.)
        assert_eq!(
            session_status(
                &entry_with(Some(a_terminal()), LastOutcome::Launching),
                &live
            ),
            (SessionStatus::Running, None)
        );

        // A dead pane's own code is the answer, and the record supplies
        // the annotation the pane cannot know.
        assert_eq!(
            session_status(
                &entry_with(
                    Some(a_terminal()),
                    LastOutcome::Exited {
                        exit_code: None,
                        annotation: Some("stopped by user".to_string()),
                    }
                ),
                &dead
            ),
            (
                SessionStatus::Exited { exit_code: Some(3) },
                Some("stopped by user".to_string())
            )
        );

        // No pane to ask: the record answers, and interrupted is NOT
        // flattened into exited-unknown — the whole point of the state.
        assert_eq!(
            session_status(&entry_with(None, LastOutcome::Interrupted), &empty),
            (SessionStatus::Interrupted, None)
        );
        assert_eq!(
            session_status(
                &entry_with(
                    None,
                    LastOutcome::Exited {
                        exit_code: Some(7),
                        annotation: Some("stopped by user".to_string()),
                    }
                ),
                &empty
            ),
            (
                SessionStatus::Exited { exit_code: Some(7) },
                Some("stopped by user".to_string())
            ),
            "a code and annotation witnessed before the terminal vanished are retained \
             knowledge, not a guess"
        );

        // Nothing observed and nothing recorded: M2's honest fallback.
        assert_eq!(
            session_status(&entry_with(None, LastOutcome::Running), &empty),
            (SessionStatus::Exited { exit_code: None }, None)
        );

        // The seam PLAN_M3.md item 3 slots into: a recorded error outranks
        // every inference, including a pane tmux would call alive.
        assert_eq!(
            session_status(
                &entry_with(
                    Some(a_terminal()),
                    LastOutcome::Error {
                        detail: "Permission denied".to_string()
                    }
                ),
                &live
            ),
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
    /// rule gets wrong: tmux publishes `pane_dead` before
    /// `pane_dead_status` is readable, so the poll that first sees the
    /// death routinely has no code while the next one does.
    #[test]
    fn observations_offered_to_the_store_cover_silence_and_enrichment() {
        let dead_with_code = PaneState::for_test("fh-1", "%0", "@0").dead_with(Some(3));
        let dead_without_code = PaneState::for_test("fh-1", "%0", "@0").dead_with(None);
        let alive = PaneState::for_test("fh-1", "%0", "@0");

        assert_eq!(observation(&LastOutcome::Running, Some(&alive)), None);
        assert_eq!(
            observation(&LastOutcome::Running, Some(&dead_with_code)),
            Some(Transition::ObservedExit { exit_code: Some(3) })
        );
        assert_eq!(
            observation(&LastOutcome::Running, None),
            Some(Transition::ObservedExit { exit_code: None })
        );
        assert_eq!(
            observation(&LastOutcome::StopRequested, None),
            Some(Transition::ObservedExit { exit_code: None }),
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
                Some(&dead_with_code)
            ),
            Some(Transition::ObservedExit { exit_code: Some(3) }),
            "a code tmux can only now report must still reach the record"
        );
        assert_eq!(
            observation(
                &LastOutcome::Exited {
                    exit_code: Some(3),
                    annotation: None
                },
                Some(&dead_without_code)
            ),
            None,
            "a known code is never re-offered to be replaced by a missing one"
        );
    }

    /// The recency rule, at both sides of its boundary and at the two
    /// states that have no recency at all.
    ///
    /// Table-driven for the same reason the precedence test above is: each
    /// case alone looks arbitrary, and it is the RELATIONSHIPS that are the
    /// contract — that the boundary is inclusive, that "not watched yet"
    /// and "watched and still" are opposite answers, and that a session
    /// with no pane never reaches this code at all.
    ///
    /// Driven through `session_status_at` rather than by sleeping: the
    /// window is measured in production seconds, and a test that waited it
    /// out would be both slow and, on a loaded runner, a flake.
    #[test]
    fn the_output_recency_window_decides_running_from_idle_at_both_edges() {
        let live = pane_map(false, None);
        let now = Instant::now();
        // Far enough into the fake timeline that subtracting a window
        // cannot underflow the monotonic clock's origin.
        let now = now + OUTPUT_RECENCY_WINDOW * 4;

        let just_inside = entry_sampled(
            AgentKind::Generic,
            9,
            Some(now - OUTPUT_RECENCY_WINDOW),
            Some("working"),
        );
        assert_eq!(
            session_status_at(&just_inside, &live, now).0,
            SessionStatus::Running,
            "the window is inclusive: output exactly one window old still counts"
        );

        let just_outside = entry_sampled(
            AgentKind::Generic,
            9,
            Some(now - OUTPUT_RECENCY_WINDOW - Duration::from_millis(1)),
            Some("working"),
        );
        assert_eq!(
            session_status_at(&just_outside, &live, now).0,
            SessionStatus::Idle,
            "a millisecond past the window is quiet; nothing rounds in the session's favour"
        );

        // Sampled repeatedly and never seen to change: the genuinely idle
        // shape, and the one a `last_change`-only classifier would have to
        // guess at.
        let never_changed = entry_sampled(AgentKind::Generic, 9, None, Some("a shell prompt"));
        assert_eq!(
            session_status_at(&never_changed, &live, now).0,
            SessionStatus::Idle
        );
    }

    /// A session nothing has sampled twice yet is `Running`, not `Idle`.
    ///
    /// This is the documented pre-first-sample state, and it is worth its
    /// own test because it is the state EVERY session passes through — at
    /// create, and again for every session after a supervisor restart. The
    /// wrong answer here would paint a whole fleet idle for the first
    /// seconds of its life, which is exactly the kind of systematically
    /// wrong status that teaches users to ignore the column.
    ///
    /// Both sub-two counts are pinned, because they are different facts:
    /// zero means the ticker has not reached this session, one means it
    /// has but has nothing to compare against yet.
    #[test]
    fn a_session_that_has_not_been_watched_twice_is_running_rather_than_idle() {
        let live = pane_map(false, None);
        let now = Instant::now();
        for samples in [0, 1] {
            let entry = entry_sampled(AgentKind::Generic, samples, None, None);
            assert_eq!(
                session_status_at(&entry, &live, now).0,
                SessionStatus::Running,
                "an unwatched live session is one that just launched, not one at rest \
                 ({samples} samples)"
            );
        }
    }

    /// Sharpening is actually WIRED: an integrated session whose sampled
    /// tail carries its agent's approval prompt classifies `Waiting`, and
    /// the same tail on a session with no integration does not.
    ///
    /// The negative half is what makes this a wiring test rather than a
    /// duplicate of `agent_kind`'s own recognition tests: it pins that the
    /// per-kind knowledge is reached THROUGH the snapshot, so a generic
    /// session cannot accidentally inherit another agent's heuristics.
    #[test]
    fn an_integrated_sessions_prompt_tail_is_sharpened_to_waiting() {
        let live = pane_map(false, None);
        let now = Instant::now();
        // Quiet by the baseline rule — a pending approval is exactly the
        // case where nothing is being printed — so the `Waiting` below can
        // only have come from the sharpener.
        let claude = entry_sampled(AgentKind::Claude, 9, None, Some(CLAUDE_APPROVAL_TAIL));
        assert_eq!(
            session_status_at(&claude, &live, now).0,
            SessionStatus::Waiting
        );

        let generic = entry_sampled(AgentKind::Generic, 9, None, Some(CLAUDE_APPROVAL_TAIL));
        assert_eq!(
            session_status_at(&generic, &live, now).0,
            SessionStatus::Idle,
            "a session with no integration keeps the generic baseline, whatever its screen says"
        );

        // And an integrated session that is merely working is left alone.
        let busy = entry_sampled(
            AgentKind::Claude,
            9,
            Some(now),
            Some("⏺ Reading src/main.rs (120 lines)"),
        );
        assert_eq!(
            session_status_at(&busy, &live, now).0,
            SessionStatus::Running
        );
    }

    /// A sharpener may only ever choose between LIVE statuses; anything
    /// else it returns is discarded and the baseline stands.
    ///
    /// Tested against [`live_only`] directly because that is the whole
    /// mechanism — no implementation in the tree returns a dead status
    /// today, and the point of the guard is that a future one (or a
    /// mistyped match arm) cannot make tmux's liveness verdict wrong by
    /// looking at a screen. The screen is not evidence about the process:
    /// a pane can print "process exited" from a log file while the agent
    /// that printed it runs on.
    #[test]
    fn a_sharpener_can_never_move_a_session_across_the_liveness_boundary() {
        for dead in [
            SessionStatus::Exited { exit_code: Some(0) },
            SessionStatus::Error {
                detail: "nope".to_string(),
            },
            SessionStatus::Interrupted,
            SessionStatus::Unknown,
        ] {
            assert_eq!(
                live_only(SessionStatus::Running, dead.clone()),
                SessionStatus::Running,
                "{dead:?} from a sharpener must not survive"
            );
        }
        // The live answers it IS allowed to give pass through untouched,
        // including the promotion the whole method exists for.
        assert_eq!(
            live_only(SessionStatus::Idle, SessionStatus::Waiting),
            SessionStatus::Waiting
        );
        assert_eq!(
            live_only(SessionStatus::Running, SessionStatus::Running),
            SessionStatus::Running
        );
    }
}

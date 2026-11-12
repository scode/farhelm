//! Terminal identity and the attachment bookkeeping built around it.
//!
//! [`HostTerminal`] is what backs one of a session's terminals (the agent or
//! one of its tabs): always a terminal host, through the supervisor's
//! runtime for it. [`TerminalId`] names which of them a request means, and
//! `resolve_terminal` finds it: the agent's on the session entry, a tab's
//! in the session's cells (`service::core::host_tabs`).
//! `AttachmentKey`/`ActiveAttach`/`InputRoute` are the bookkeeping an
//! `Attach` installs and every later `Resize`/`Detach`/input frame looks
//! up by.

use super::core::{RequestError, SessionEntry, Supervisor, truncate_for_error};
use crate::hosts::HostTerminal;
use crate::store::{LastOutcome, TerminalKind};
use farhelm_proto::{ErrorKind, Frame, RestartOffer, TerminalSelector};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch};

/// How long an `OpenTab` watches a new tab's shell before accepting that it
/// started (`core::host_tabs`, the dead-at-reply refusal).
///
/// A shell that cannot start is gone in microseconds; what the wait covers
/// is the host's runtime learning of it (the child's exit reaches the
/// supervisor as a record on the host's connection). A quarter second is
/// the WORST case a healthy open pays, on an operation a human performs by
/// hand. Shortening it would make the refusal flaky in exactly the
/// direction that matters (a failed launch reported as a live tab);
/// lengthening it buys nothing but latency.
pub(crate) const TAB_LAUNCH_SETTLE: Duration = Duration::from_millis(250);

/// Poll interval within [`TAB_LAUNCH_SETTLE`]: the settle ends early, on
/// the failing path, at the first step that sees the shell gone.
pub(crate) const TAB_LAUNCH_SETTLE_STEP: Duration = Duration::from_millis(25);

/// Which of a session's terminals something addresses — the second half
/// of every [`AttachmentKey`], and the supervisor-side resolution of
/// [`TerminalSelector`].
///
/// Separate from the wire type on purpose: a `TerminalSelector` is what a
/// CLIENT asked for, while a `TerminalId` is a terminal this supervisor
/// has resolved against the session (`resolve_terminal`), so nothing past
/// that resolution can accidentally key state off an unvalidated request.
/// The two shapes are otherwise deliberately parallel, and that paid off
/// exactly as intended: teaching this supervisor to serve tabs (PLAN_M4.md
/// item 2) taught `resolve_terminal` how to find a tab's terminal and
/// changed no key, map, or handler shape at all.
///
/// Both variants resolve without a round trip: `Agent` is answered from
/// the session entry, and a `Tab` from the session's own cell
/// (`SessionCells::host_tabs`), which holds its runtime. Tabs are not
/// durable metadata in the store (`SessionInfo::tabs`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum TerminalId {
    /// The session's agent terminal: the one terminal every session has
    /// had since M1, and the one every pre-M4 `Attach` meant implicitly.
    Agent,
    /// A terminal tab, by the supervisor-minted id `TabInfo::id` carries.
    Tab(String),
}

/// Resolve a client's wire-level selector to the terminal identity the
/// attachment machinery keys on. Total and side-effect-free: whether the
/// named terminal actually EXISTS is a separate question, answered by
/// `resolve_terminal` against the session entry.
///
/// By value so a tab id MOVES out of the decoded request rather than
/// being copied — the selector has no other consumer once its terminal is
/// resolved.
impl From<TerminalSelector> for TerminalId {
    fn from(selector: TerminalSelector) -> TerminalId {
        match selector {
            TerminalSelector::Agent => TerminalId::Agent,
            TerminalSelector::Tab { id } => TerminalId::Tab(id),
        }
    }
}

/// What one live attachment is FOR: a session's terminal, not merely a
/// session.
///
/// SPEC.md caps a session's attached CLIENTS at one, never its attached
/// terminals — a client owning a session owns all of its terminals at
/// once, each on its own channel with its own output task and input side
/// (PLAN_M4.md item 3). So the attachment map is keyed per (session,
/// terminal) and the one-attachment rule is enforced above it, by lease
/// (see the `Attach` handler). Keying by session id alone — what this was
/// before M4 — would make a second terminal of the same session look like
/// a takeover of the first.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct AttachmentKey {
    pub(crate) session: String,
    pub(crate) terminal: TerminalId,
}

impl AttachmentKey {
    pub(crate) fn new(session: &str, terminal: TerminalId) -> AttachmentKey {
        AttachmentKey {
            session: session.to_string(),
            terminal,
        }
    }
}

/// Find one of a session's terminals, or say why there is none.
///
/// The ONE resolution funnel for [`TerminalId`], so no path has to learn
/// what a tab is on its own — attach, resize, and input use this LIVE-ONLY
/// spelling, while close goes through [`resolve_terminal_for_close`],
/// whose whole difference is that an exited tab still resolves (it has
/// to be findable to be destroyed).
///
/// The agent's terminal is the session entry's; a tab's is its runtime in
/// the session's cells (`SessionCells::host_tabs`), looked up at the moment
/// of use because a session's tabs are opened and closed while the entry
/// stands still, possibly by another client.
///
/// A terminal that does not exist is `NotFound` — the shape `Attach`
/// promises for a stale selector (`TerminalSelector::Tab`'s own contract)
/// — and there is deliberately no fallback to another terminal, because
/// attaching the WRONG one quietly would be worse than failing.
pub(crate) async fn resolve_terminal(
    sup: &Supervisor,
    entry: &SessionEntry,
    terminal: &TerminalId,
) -> Result<HostTerminal, RequestError> {
    resolve_terminal_inner(sup, entry, terminal, TabResolution::LiveOnly).await
}

/// [`resolve_terminal`] for the CLOSE path, which must be able to find a
/// tab whose shell has exited: an exited tab is exactly what the ticker's
/// reap (and a user's close racing it) has to locate and destroy, while
/// every interactive resolver treats it as already gone (SPEC.md reaps it;
/// offering its "discarded" scrollback through attach would contradict the
/// listing that no longer shows it).
pub(crate) async fn resolve_terminal_for_close(
    sup: &Supervisor,
    entry: &SessionEntry,
    terminal: &TerminalId,
) -> Result<HostTerminal, RequestError> {
    resolve_terminal_inner(sup, entry, terminal, TabResolution::DeadIncluded).await
}

/// Whether a tab whose shell has exited resolves or reads as gone.
/// Two variants rather than a bool so call sites say what they mean.
enum TabResolution {
    LiveOnly,
    DeadIncluded,
}

/// The refusal for a session whose terminal no longer exists on this host,
/// worded from what the supervisor actually knows.
///
/// This string travels a long way: the helm relays it verbatim as the
/// terminal socket's `detached` reason, and the browser paints it under a
/// banner, so it is the sentence a user reads when they open such a
/// session. SPEC.md's reboot contract forbids inferring whether the agent
/// exited before the machine went down or was killed by it, and
/// "interrupted" exists to say exactly that, so no wording orders the
/// agent's end against the event that ended its terminal. The wording
/// follows the durable outcome first, in the same precedence the UI's own
/// terminal-absence decision uses (`terminal_absence` in the session view),
/// so a refusal never contradicts the band on screen, and then the row's
/// terminal kind:
///
/// - An entry marked [`LastOutcome::Interrupted`] (a host reboot, or an
///   update that ended an old terminal) is told it ended while no
///   supervisor was watching.
/// - A host row's terminal host has ended (killed, or gone with the
///   machine), never started (a launch kept as a launching record), or may
///   run but is not held (an earlier host the claim-window fallback could
///   not read at this supervisor's start, or a replacement that could not
///   be confirmed ended). Where the host may run, a Restart may be refused
///   until the host is settled, or start without its history.
/// - A row of the old kind ran in a tmux terminal an earlier release
///   created, which this release ends at startup (`core::old_terminals`).
///
/// Each ends with the way out, per the session's own [`RestartOffer`]: a
/// Restart of a session with no terminal starts the agent in a new
/// terminal (`core::host_restart`), but only a session whose conversation
/// was captured can be promised a resume, and SPEC.md makes Restart
/// unavailable otherwise, so such a session is pointed at Replace instead.
fn missing_terminal_message(
    id: &str,
    outcome: &LastOutcome,
    offer: RestartOffer,
    kind: TerminalKind,
) -> String {
    let id = truncate_for_error(id);
    let way_out = if offer.can_restart() {
        "restart offers to resume the conversation"
    } else {
        "its conversation cannot be resumed, so replace starts it over"
    };
    match (outcome, kind) {
        (LastOutcome::Interrupted, _) => format!(
            "session {id} has no terminal: it ended while no supervisor was watching (a host \
             reboot, or an update that ended an old terminal); {way_out}, in a new terminal"
        ),
        (_, TerminalKind::Host) => format!(
            "session {id} has no terminal on this host: its terminal host has ended, never \
             started, or could not be reached; {way_out}, in a new terminal"
        ),
        (_, TerminalKind::Tmux) => format!(
            "session {id} has no terminal on this host: it ran in an old terminal, which an \
             update of Farhelm ended; {way_out}, in a new terminal"
        ),
    }
}

/// The refusal for a tab selector that names no tab of `entry`'s session.
fn no_such_tab(entry: &SessionEntry, id: &str) -> RequestError {
    RequestError::new(
        ErrorKind::NotFound,
        format!(
            "session {} has no terminal tab {}: it was closed, or a reboot erased it",
            truncate_for_error(&entry.info.id),
            truncate_for_error(id),
        ),
    )
}

/// Resolve only an active session's terminal. A retired container row remains
/// listable for Delete but cannot grant access to its outside-host terminal.
async fn resolve_terminal_inner(
    sup: &Supervisor,
    entry: &SessionEntry,
    terminal: &TerminalId,
    resolution: TabResolution,
) -> Result<HostTerminal, RequestError> {
    super::core::require_active_launch(&entry.info.launch)?;
    let id = match terminal {
        TerminalId::Agent => return agent_terminal(sup, entry).await,
        TerminalId::Tab(id) => id,
    };
    // A tab is the session's own runtime, found whatever the agent's
    // terminal is: a tab outlives its agent's terminal. An exited one reads
    // as gone on interactive paths until the reap (or a close) ends its
    // host: the listing already dropped it, so a client holding its id must
    // not be able to attach into the corpse and read scrollback the product
    // has declared discarded.
    match super::core::find_host_tab(&entry.session.host_tabs, id) {
        Some(tab) => match resolution {
            TabResolution::LiveOnly if tab.finished() => Err(no_such_tab(entry, id)),
            TabResolution::LiveOnly | TabResolution::DeadIncluded => Ok(tab.host),
        },
        None => Err(no_such_tab(entry, id)),
    }
}

/// The session's agent terminal, or the refusal for a session that has
/// none ([`missing_terminal_message`]).
///
/// An entry without a terminal was reloaded from the store at supervisor
/// startup with no terminal it could use, or lost it since. Reporting
/// `NotFound` — rather than fabricating a dead terminal to attach to — is
/// the same "do not guess" discipline SPEC.md applies elsewhere; the session
/// stays visible in the list either way.
async fn agent_terminal(
    sup: &Supervisor,
    entry: &SessionEntry,
) -> Result<HostTerminal, RequestError> {
    if let Some(terminal) = &entry.terminal {
        return Ok(terminal.clone());
    }
    // The row's terminal kind words the refusal, taken as a host's when the
    // row cannot be read (the wording that names no old terminal).
    let kind = sup
        .store
        .session(&entry.info.id)
        .await
        .ok()
        .flatten()
        .map_or(TerminalKind::Host, |row| row.terminal_kind);
    // Two per-entry mutexes, taken one after the other and never nested:
    // each hold is a read of one small value, which is the rule that makes
    // blocking mutexes safe inside async code here (see `RunCells::outcome`).
    let outcome = entry
        .run
        .outcome
        .lock()
        .expect("outcome mutex poisoned")
        .clone();
    // The same derivation the restart path uses for the offer it reports on
    // the wire, so the refusal never promises a resume the restart would not
    // perform.
    let offer = {
        let capture = entry.run.capture.lock().expect("capture mutex poisoned");
        entry.snapshot.restart_offer(
            capture.committed_conversation(),
            capture.committed_ownership_version().unwrap_or(0),
        )
    };
    Err(RequestError::new(
        ErrorKind::NotFound,
        missing_terminal_message(&entry.info.id, &outcome, offer, kind),
    ))
}

/// What one connection remembers about a data channel it has attached:
/// enough to find that channel's attachment again, and to charge input
/// against the right session.
///
/// Connection-local, and deliberately holding the session ENTRY rather
/// than just its id: the input path needs the entry anyway (first-input
/// accounting), and holding the same `Arc` the attach resolved is what
/// keeps a channel typing into the session it was attached to even if
/// the map has since been rebuilt around it.
///
/// The `key` is built ONCE, at attach time, and every later hop (input,
/// resize, detach) borrows it. Rebuilding it per frame would allocate a
/// session id and a terminal id on every keystroke, and — worse — would
/// leave two places that must agree on how a channel maps to an
/// attachment.
pub(crate) struct InputRoute {
    pub(crate) entry: Arc<SessionEntry>,
    pub(crate) key: AttachmentKey,
}

/// Whether an existing attachment is displaced by an incoming attach —
/// the session-scoped half of SPEC.md's one-attached-client rule
/// (PLAN_M4.md item 3).
///
/// A pure function rather than an inline closure so the rule can be
/// tested against key shapes the integration tests cannot yet produce (a
/// session holding both an agent and a tab attachment): it decides how
/// many of a session's terminals a takeover tears down, which is exactly
/// the part that only becomes observable once tabs exist.
///
/// Two independent conditions: same SESSION (a lease is never
/// cross-session — one client may hold terminals in many sessions, and
/// attaching one of them must not disturb the others), and a lease that
/// does not group with the requester's (see [`same_lease_client`]).
pub(crate) fn displaced_by_attach(
    existing: &AttachmentKey,
    existing_lease: &str,
    session: &str,
    lease: &str,
) -> bool {
    existing.session == session && !same_lease_client(existing_lease, lease)
}

/// Whether two attachments belong to the same client, and so whether the
/// newcomer takes the incumbent over (PLAN_M4.md item 3, and the `lease`
/// field's own contract in `farhelm-proto`).
///
/// Non-empty equality, with the empty lease matching NOTHING — not even
/// another empty lease. That asymmetry is the whole of the pre-M4
/// compatibility story: an un-leased attach is its own singleton client,
/// so it takes over everything on the session and is taken over by
/// anything, which is exactly what a single-terminal client expected
/// before leases existed. Treating empty as a shared identity instead
/// would silently FUSE every legacy client on a session into one lease
/// and suppress the takeover they depend on.
fn same_lease_client(incumbent: &str, requester: &str) -> bool {
    !incumbent.is_empty() && incumbent == requester
}

/// The reason a client is told it lost its attachment to a DIFFERENT
/// client (SPEC.md's one-attached-client rule, made visible).
///
/// One constant because a session-scoped takeover emits it once per
/// terminal channel the loser held: the identical string across all of
/// them is what lets a client coalesce those `Detached`s into a single
/// banner, which is why the protocol needs no session-scoped takeover
/// message of its own (see `ControlMsg::Attach`'s docs).
///
/// DEFINED from the proto constant a non-displacing attach is refused with
/// rather than repeating its text: a client told it lost the session and a
/// client told it cannot have the session back are learning the same fact,
/// and the only difference is whether it had a socket open at the time.
/// Two literals would be two chances for the two banners to read
/// differently. What the browser decides from is the accompanying
/// `DetachCode::TakenOver`, never this text.
pub(crate) const DETACH_REASON_TAKEOVER: &str = farhelm_proto::ATTACH_REFUSED_TAKEN_OVER;

/// The reason the SAME client is told its previous attachment to a
/// terminal was replaced by its own newer one.
///
/// Distinct from [`DETACH_REASON_TAKEOVER`] because the wire contract
/// makes them different events: equal non-empty leases are one client
/// reconnecting (a reload, a re-mount), and telling that client "another
/// client attached" is simply false — it would surface a takeover banner
/// accusing a second user who does not exist, in the one case SPEC.md
/// treats as an ordinary reconnect. Both reasons are still rendered
/// generically by clients (`ControlMsg::Detached` takes an open-ended
/// string), so the split costs nothing but honesty gains everything.
///
/// Phrased as a bare cause, one line, no leading "detached:" — clients
/// paste it into their own detach surface, exactly like
/// [`DETACH_REASON_STALLED`].
pub(crate) const DETACH_REASON_REPLACED: &str = "replaced by a newer attachment to this terminal";

/// Byte cap on `ControlMsg::Attach`'s `lease`, enforced before the attach
/// touches anything.
///
/// The lease is a client-minted identifier the supervisor RETAINS for the
/// life of every attachment made under it, so its size is per-attachment
/// retained memory, not a one-off parse cost — and control frames are
/// capped at megabytes (`MAX_FRAME_LEN`), so an unbounded lease turns one
/// oversized frame into memory held until that client detaches. 128 bytes
/// is generous for the only legitimate use the wire contract describes:
/// one high-entropy random id per session-view instance (a UUID is 36).
/// BYTES rather than characters, because bytes are what is retained and
/// what a frame carries.
pub(crate) const MAX_LEASE_BYTES: usize = 128;

/// The refusal an attach gets once the supervisor has begun stopping
/// ([`Supervisor::is_stopping`]). Retryable: the supervisor is about to
/// exit, and a restarted one accepts attaches again.
pub(crate) const SUPERVISOR_STOPPING: &str =
    "the supervisor is stopping; attach again once it is running";

/// One live attachment: a session's terminal, streamed to one channel of
/// one connection. `notify` reaches the owning connection's writer so a
/// takeover can tell the old client it was detached.
pub(crate) struct ActiveAttach {
    pub(crate) channel: u32,
    /// The client identity this attachment was made under
    /// (`ControlMsg::Attach::lease`), stored verbatim — including the
    /// empty legacy lease, whose meaning is entirely in
    /// [`same_lease_client`].
    ///
    /// Held per ATTACHMENT rather than in a session-level "current owner"
    /// slot deliberately: the takeover decision is made by scanning the
    /// session's live attachments, so there is no owner record to leave
    /// stale when the last of a lease's channels detaches, and no window
    /// in which a session claims an owner that holds nothing.
    pub(crate) lease: String,
    pub(crate) notify: mpsc::Sender<Frame>,
    /// The output task (`super::grid_stream::GridSender`). Teardown asks it
    /// to stop through `forwarder_shutdown`, then joins it; it holds no
    /// external process, so it stops at once.
    pub(crate) forwarder: tokio::task::JoinHandle<()>,
    /// Asks the output task to stop and report its end as requested,
    /// rather than aborting it, so that the detach reason a teardown chose
    /// wins over whatever the stream would have reported.
    pub(crate) forwarder_shutdown: watch::Sender<bool>,
    /// When this attachment's client asked for its output to stop
    /// (`ControlMsg::PauseOutput`), or `None` while output may flow. Read
    /// by the output task.
    ///
    /// A `watch` rather than a flag plus a `Notify`: the task needs both
    /// "what is the state right now" and "wake me when it changes" (to
    /// resume promptly), and a watch channel is exactly those two together
    /// with no window where a notification can be missed between checking
    /// and parking.
    ///
    /// Carrying the pause's START INSTANT rather than a bare bool is what
    /// makes [`super::core::STALL_DETACH_TIMEOUT`] a hard maximum: one
    /// continuous pause has exactly ONE deadline no matter how many wakeups
    /// happen under it, so a client that drains just fast enough to keep
    /// the task moving cannot reset the clock forever. A repeated
    /// `PauseOutput` must NOT overwrite the stored start (see
    /// `set_attachment_paused`), for the same reason.
    pub(crate) pause: watch::Sender<Option<tokio::time::Instant>>,
    /// The attachment's input side (`super::grid_stream`, "Input"): it
    /// encodes each grid input message and owns the pump that writes the
    /// bytes to the host. Dropped with the attachment on every teardown
    /// path, which stops the pump.
    pub(crate) input: super::grid_stream::HostAttach,
}

impl ActiveAttach {
    /// Ask the output task to stop at its next cancellation-safe await.
    pub(crate) fn request_forwarder_shutdown(&self) {
        self.forwarder_shutdown.send_replace(true);
    }
}

/// Wait for an attachment's output task to stop once it was asked to
/// ([`ActiveAttach::request_forwarder_shutdown`]), logging one that
/// panicked: it held nothing outside this process, so there is nothing
/// else to clean up after it, and the attachment is gone either way.
///
/// Callers that end several attachments ask every task first and join them
/// after, so that none waits behind another.
pub(crate) async fn join_forwarder(key: &AttachmentKey, forwarder: tokio::task::JoinHandle<()>) {
    if let Err(error) = forwarder.await {
        tracing::warn!(session = %key.session, terminal = ?key.terminal, %error,
            "a terminal attachment's output task panicked or was cancelled");
    }
}

impl Supervisor {
    /// Whether this supervisor's orderly stop has begun draining its
    /// attachments ([`Supervisor::shutdown_output_clients`]): from then on
    /// an attach is refused with [`SUPERVISOR_STOPPING`], so that nothing new
    /// starts streaming after the stop drained what was attached.
    pub(crate) fn is_stopping(&self) -> bool {
        self.attachments_closed
            .load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The missing-terminal refusal tells a reboot apart from a restart
    /// gap, and never orders the agent's end against either.
    ///
    /// This is the sentence the browser shows when an interrupted session
    /// is opened, so its two claims are pinned: an `Interrupted` outcome
    /// names its two causes (a host reboot, or an update that ended an old
    /// terminal) and the restart-with-resume way forward, while every other
    /// outcome says only that this host has no terminal, an old terminal's
    /// with the same way forward. The
    /// negative assertion guards the regression this replaced — wording
    /// that stated the restart came "after the agent ended", an ordering
    /// SPEC.md says the supervisor cannot know after a reboot.
    #[farhelm_testtrace::test]
    fn missing_terminal_wording_follows_the_durable_outcome() {
        let resumable = missing_terminal_message(
            "s-1",
            &LastOutcome::Interrupted,
            RestartOffer::Resume,
            TerminalKind::Tmux,
        );
        assert!(
            resumable.contains("host reboot") && resumable.contains("ended an old terminal"),
            "{resumable}"
        );
        assert!(
            resumable.contains("restart offers to resume"),
            "{resumable}"
        );
        // Only a captured conversation may be promised a resume: every
        // unavailable offer points at replace instead, and never at a
        // restart that would start fresh.
        for offer in [
            RestartOffer::NotCaptured,
            RestartOffer::NoConversationReporting,
        ] {
            let text = missing_terminal_message(
                "s-1",
                &LastOutcome::Interrupted,
                offer,
                TerminalKind::Tmux,
            );
            assert!(text.contains("host reboot"), "{text}");
            assert!(
                text.contains("replace starts it over") && !text.contains("restart offers"),
                "{offer:?}: {text}"
            );
        }

        for other in [
            LastOutcome::Launching,
            LastOutcome::Running,
            LastOutcome::StopRequested,
            LastOutcome::Exited {
                exit_code: Some(0),
                annotation: None,
            },
            LastOutcome::Error {
                detail: "exec failed".to_string(),
            },
        ] {
            let text = missing_terminal_message(
                "s-1",
                &other,
                RestartOffer::NotCaptured,
                TerminalKind::Tmux,
            );
            assert!(
                text.contains("has no terminal on this host"),
                "{other:?}: {text}"
            );
            assert!(
                !text.contains("reboot"),
                "{other:?} must not claim a reboot: {text}"
            );
        }
        let old = missing_terminal_message(
            "s-1",
            &LastOutcome::Exited {
                exit_code: Some(0),
                annotation: None,
            },
            RestartOffer::Resume,
            TerminalKind::Tmux,
        );
        assert!(
            old.contains("old terminal") && old.contains("restart offers to resume"),
            "an ended old session is told why and how to go on: {old}"
        );
        for text in [
            resumable,
            missing_terminal_message(
                "s-1",
                &LastOutcome::Running,
                RestartOffer::NotCaptured,
                TerminalKind::Tmux,
            ),
        ] {
            assert!(
                !text.contains("after the agent ended"),
                "no wording may order the agent's end against the restart: {text}"
            );
        }
    }

    /// A host session with no terminal is told its terminal host ended,
    /// never started, or could not be reached, and is given the way out a
    /// Restart now takes.
    ///
    /// Why: a host session never ran in tmux, and its Restart with no
    /// terminal starts a new host (`core::host_restart`), so the refusal
    /// must neither name tmux nor send the user to Replace when Restart
    /// would resume the conversation; and a host row is terminal-less also
    /// while its host may run unreadable, so the text must not say it
    /// ended. Spec: for a host row's outcomes other than `Interrupted`, the
    /// text says its terminal host has ended, never started, or could not
    /// be reached, names no tmux, and offers to resume the conversation in
    /// a new terminal when the conversation was captured and points at
    /// replace otherwise; an `Interrupted` host session is told of a host
    /// reboot as an old-kind one is.
    #[farhelm_testtrace::test]
    fn a_host_session_without_a_terminal_is_told_its_host_ended() {
        for outcome in [
            LastOutcome::Launching,
            LastOutcome::Running,
            LastOutcome::Exited {
                exit_code: Some(0),
                annotation: None,
            },
        ] {
            let resumable =
                missing_terminal_message("s-1", &outcome, RestartOffer::Resume, TerminalKind::Host);
            assert!(
                resumable
                    .contains("terminal host has ended, never started, or could not be reached")
                    && resumable.contains("restart offers to resume the conversation")
                    && !resumable.contains("tmux"),
                "{outcome:?}: {resumable}"
            );
            let fresh = missing_terminal_message(
                "s-1",
                &outcome,
                RestartOffer::NotCaptured,
                TerminalKind::Host,
            );
            assert!(
                fresh.contains("replace starts it over") && !fresh.contains("restart offers"),
                "{outcome:?}: {fresh}"
            );
        }
        let rebooted = missing_terminal_message(
            "s-1",
            &LastOutcome::Interrupted,
            RestartOffer::Resume,
            TerminalKind::Host,
        );
        assert!(rebooted.contains("host reboot"), "{rebooted}");
    }

    /// The lease grouping rule, as a truth table.
    ///
    /// This one predicate decides every takeover the supervisor performs,
    /// and two of its four cases are unreachable from the integration
    /// tests today: "same lease, different terminal is NOT a takeover"
    /// needs a session with two terminals (the tabs PR), and "two empty
    /// leases are still two clients" is invisible wherever a takeover
    /// happens to be expected anyway. Pinning the table here is what
    /// keeps the empty-lease asymmetry — the entire pre-M4 compatibility
    /// story — from being "simplified" into plain string equality, which
    /// would fuse every un-leased client on a session into one client and
    /// silently delete the takeover they depend on.
    #[farhelm_testtrace::test]
    fn the_empty_lease_groups_with_nothing_while_equal_leases_group() {
        // (incumbent, requester, same client?)
        let cases = [
            ("client-a", "client-a", true),
            ("client-a", "client-b", false),
            ("", "client-a", false),
            ("client-a", "", false),
            // The case that reads as equality but must not be: two
            // un-leased clients are two clients.
            ("", "", false),
        ];
        for (incumbent, requester, expected) in cases {
            assert_eq!(
                same_lease_client(incumbent, requester),
                expected,
                "lease grouping of incumbent {incumbent:?} against requester {requester:?}"
            );
        }
    }

    /// Which attachments an incoming attach displaces, over key shapes
    /// the integration tests cannot yet build.
    ///
    /// The property that matters is quantified over a session's
    /// TERMINALS — a different lease takes over every one of them, a
    /// matching lease takes over none — and today a session has exactly
    /// one terminal, so an end-to-end test can only ever observe the
    /// single-key case. Asserting it here with an agent key AND a tab key
    /// in the same session is what keeps the rule from silently
    /// degrading into "displace the one terminal I know about" before the
    /// tabs PR can notice.
    ///
    /// The unrelated-session case is the other half: a lease is not
    /// cross-session, so a client attaching one session must never
    /// disturb the terminals it holds in another.
    #[farhelm_testtrace::test]
    fn a_different_lease_displaces_every_terminal_of_that_session_alone() {
        let agent = AttachmentKey::new("session-1", TerminalId::Agent);
        let tab = AttachmentKey::new("session-1", TerminalId::Tab("tab-1".to_string()));
        let elsewhere = AttachmentKey::new("session-2", TerminalId::Agent);

        for held in [&agent, &tab] {
            assert!(
                displaced_by_attach(held, "lease-a", "session-1", "lease-b"),
                "a different lease must displace {held:?}"
            );
            assert!(
                !displaced_by_attach(held, "lease-a", "session-1", "lease-a"),
                "the same lease must displace none of its own terminals ({held:?})"
            );
            // The empty lease is its own singleton client in both
            // directions — see `same_lease_client`.
            assert!(
                displaced_by_attach(held, "", "session-1", "lease-a"),
                "a leased attach must displace an un-leased holder ({held:?})"
            );
            assert!(
                displaced_by_attach(held, "lease-a", "session-1", ""),
                "an un-leased attach must displace a leased holder ({held:?})"
            );
            assert!(
                displaced_by_attach(held, "", "session-1", ""),
                "two un-leased attachments are two clients ({held:?})"
            );
        }

        for (held_lease, incoming_lease) in [("lease-a", "lease-b"), ("", ""), ("lease-a", "")] {
            assert!(
                !displaced_by_attach(&elsewhere, held_lease, "session-1", incoming_lease),
                "attaching session-1 must never displace an attachment of session-2 \
                 (held {held_lease:?}, incoming {incoming_lease:?})"
            );
        }
    }
}

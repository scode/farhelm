//! Terminal identity and the attach/lease/sink types built on it.
//!
//! `TerminalId`/`Terminal` name which of a session's terminals (the agent
//! or one of its tabs) a request means and which tmux handles back it;
//! `AttachmentKey`/`ActiveAttach`/`InputRoute` are the bookkeeping an
//! `Attach` installs and every later `Resize`/`Detach`/input frame looks
//! up by. `SessionSinkHandle`/`run_session_sink` are the per-tmux-session
//! control client every attachment shares, supervised independently of
//! any one attachment's lifetime.

use super::core::{RequestError, SessionEntry, Supervisor, error_kind, truncate_for_error};
use crate::tmux::{InputClient, SessionSink};
use farhelm_proto::{ErrorKind, Frame, TerminalSelector};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tracing::warn;

/// How long an `OpenTab` watches a new tab's pane before accepting that
/// its shell started (PLAN_M4.md item 2's dead-at-reply refusal).
///
/// Sized against tmux's own latency in marking a pane dead, not against
/// the shell's: a shell that cannot start is gone in microseconds, but
/// tmux publishes `#{pane_dead}` only after reaping the child and
/// draining its pty — measured at 4–18 ms on an idle host (tmux 3.7b).
/// A quarter second is an order of magnitude of headroom over that for a
/// loaded CI runner, and it is the WORST case a healthy open pays, on an
/// operation a human performs by hand. Shortening it would make the
/// refusal flaky in exactly the direction that matters (a failed launch
/// reported as a live tab); lengthening it buys nothing but latency.
pub(crate) const TAB_LAUNCH_SETTLE: Duration = Duration::from_millis(250);

/// Poll interval within [`TAB_LAUNCH_SETTLE`]. Each step is one tmux
/// subprocess, so this trades a handful of spawns for the settle's own
/// early exit on the failing path.
pub(crate) const TAB_LAUNCH_SETTLE_STEP: Duration = Duration::from_millis(25);

/// The two tmux handles that address one of a session's terminals.
///
/// Both are needed and neither substitutes for the other: session name is
/// the target for the control-mode attach (control clients attach to a
/// session, never to a window — see [`crate::tmux::OutputStream`]), pane
/// id (`%N`) for anything pane-scoped (`send-keys`, `capture-pane`, format
/// queries).
///
/// There is deliberately no window handle, even though a tab IS a window
/// (PLAN_M4.md item 2): tmux resolves a window target from any pane inside
/// it, so `crate::tmux`'s window-scoped calls take the pane and pair it
/// with the session — which a bare window id cannot be made to do safely
/// (see `tmux::pane_in_session`'s audit). One less handle to keep in sync,
/// and the one that remains is already the session-validated one.
///
/// The same struct describes the AGENT's terminal and a TAB's, because
/// nothing below this level distinguishes them; which terminal a given
/// value describes is the caller's [`TerminalId`], not a field here.
#[derive(Clone)]
pub(crate) struct Terminal {
    pub(crate) tmux_name: String,
    pub(crate) pane: String,
}

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
/// item 2) taught `resolve_terminal` how to find a tab's pane and changed
/// no key, map, or handler shape at all.
///
/// The two variants differ in how much they cost to resolve, which is
/// worth knowing before adding a caller: `Agent` is answered from the
/// session entry, while `Tab` costs a tmux round trip, because a tab is
/// rediscovered from window markers rather than stored (`SessionInfo::tabs`
/// — tabs are not durable metadata, so there is nothing to cache that a
/// second client could not invalidate).
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
/// once, each on its own channel with its own control-mode client
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

/// One of a session's tab windows as rediscovery found it.
///
/// Ordered by [`Ord`] on `window_ordinal` alone, which is what makes
/// `SessionInfo::tabs`'s "creation order" promise mechanical rather than
/// bookkept: tmux hands out window ids from a monotonically increasing
/// per-server counter, so their numeric order IS creation order — within
/// ONE tmux-server lifetime, which is the only lifetime a tab can survive
/// anyway (a reboot erases tabs by contract).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiscoveredTab {
    pub(crate) id: String,
    pub(crate) pane: String,
    /// The numeric part of `pane` (`%9` → 9) — how a split window's panes
    /// are compared, since `%10` sorts before `%9` as a string.
    pub(crate) pane_ordinal: u64,
    /// The numeric part of the tmux window id (`@7` → 7). Parsed rather
    /// than compared as a string because `@10` sorts before `@9`
    /// lexically, which would reorder a session's tab strip the moment a
    /// user opened a tenth window.
    pub(crate) window_ordinal: u64,
}

/// Rediscover one session's tabs from a pane-state map, in creation order.
///
/// Tabs are not durable metadata — SPEC.md says a reboot or archive erases
/// them and nothing recreates them — so tmux's own window markers are the
/// record, and this is the one function that turns them back into tabs.
/// Taking an already-fetched map rather than querying keeps the
/// `ListSessions` path from paying a second lookup per session.
///
/// Every filter closes a different way a window could be misreported as a
/// tab:
///
/// - **Session name must match.** The map is server-wide.
/// - **The window must carry a tab marker at all.** A window a pane
///   process conjured on our private server carries none — pane processes
///   inherit `TMUX`, which is why a positional "windows 1 and up" scan was
///   never an option.
/// - **The marker must be complete and minted-shaped**, validated where it
///   is read (`tmux::join_pane_markers`) rather than here.
/// - **An AGENT-marked window is never a tab**, whatever else is written
///   on it. Nothing stops a pane from adding a tab marker to the agent's
///   own window, and adopting it would offer a "tab" whose close would
///   reap the agent.
/// - **A tab id claimed by two WINDOWS is ambiguous and drops BOTH.** Ids
///   are minted unique, so a duplicate means one of them was written by
///   something else — and there is no basis for preferring either. Picking
///   one would let a forged marker capture an existing tab's identity and
///   redirect close and attach onto a window of the forger's choosing.
///
/// A single window holding several panes (someone split one of ours) is
/// still ONE tab, keyed on its lowest pane NUMERICALLY — `%9` before
/// `%10`, which a string comparison gets backwards — so repeated
/// rediscovery answers the same way rather than depending on hash-map
/// iteration order.
pub(crate) fn tabs_from_pane_states(
    states: &HashMap<String, crate::tmux::PaneState>,
    tmux_name: &str,
) -> Vec<DiscoveredTab> {
    let mut found: HashMap<String, DiscoveredTab> = HashMap::new();
    let mut ambiguous: HashSet<String> = HashSet::new();
    for state in states.values() {
        if state.session_name != tmux_name || state.agent.is_some() {
            continue;
        }
        let Some(tab_id) = state.tab.as_deref() else {
            continue;
        };
        let candidate = DiscoveredTab {
            id: tab_id.to_string(),
            pane: format!("%{}", state.pane_ordinal),
            pane_ordinal: state.pane_ordinal,
            window_ordinal: state.window_ordinal,
        };
        match found.entry(tab_id.to_string()) {
            std::collections::hash_map::Entry::Occupied(mut existing) => {
                if existing.get().window_ordinal == candidate.window_ordinal {
                    // The same window, seen through a second pane: a
                    // split, not a second claimant.
                    if candidate.pane_ordinal < existing.get().pane_ordinal {
                        existing.insert(candidate);
                    }
                } else {
                    ambiguous.insert(tab_id.to_string());
                }
            }
            std::collections::hash_map::Entry::Vacant(slot) => {
                slot.insert(candidate);
            }
        }
    }
    let mut tabs: Vec<DiscoveredTab> = found
        .into_iter()
        .filter(|(id, _)| !ambiguous.contains(id))
        .map(|(_, tab)| tab)
        .collect();
    tabs.sort_by_key(|tab| tab.window_ordinal);
    tabs
}

/// The pane backing one session's AGENT terminal when the durable record
/// has none — the pane-less recovery `reload_sessions` performs for a row
/// whose `pane` column is empty (a launch that crashed before the pane id
/// could be written, or a row that predates the column).
///
/// Preference order, and the reason for each rung:
///
/// 1. **The window marked for THIS session.** Identity, not position; it
///    is why the agent window is marked at all.
/// 2. **Nothing marked for another session, and nothing marked as a tab.**
///    Adopting a tab's window would make the agent terminal and a tab the
///    same pane — stop would reap the tab, restart would respawn into it.
///    Adopting a foreign window is the "conjured window" case in the other
///    direction.
/// 3. **Otherwise the lowest window INDEX.** The legacy rung, for a
///    session created before markers existed: window 0 is the agent's in
///    every layout this system has ever produced, so position is the only
///    evidence left and it is right in exactly the case it applies to.
///
/// Ties within a rung break on the lowest pane ordinal, so repeated
/// reloads answer identically.
pub(crate) fn agent_pane_from_states(
    states: &HashMap<String, crate::tmux::PaneState>,
    tmux_name: &str,
    session_id: &str,
) -> Option<(String, crate::tmux::PaneState)> {
    let mine = || {
        states
            .values()
            .filter(move |state| state.session_name == tmux_name)
    };
    let lowest = |mut candidates: Vec<&crate::tmux::PaneState>| {
        candidates.sort_by_key(|state| (state.window_index, state.pane_ordinal));
        candidates
            .first()
            .map(|state| (format!("%{}", state.pane_ordinal), (*state).clone()))
    };
    let marked: Vec<&crate::tmux::PaneState> = mine()
        .filter(|state| state.agent.as_deref() == Some(session_id))
        .collect();
    if !marked.is_empty() {
        return lowest(marked);
    }
    lowest(
        mine()
            .filter(|state| state.agent.is_none() && state.tab.is_none())
            .collect(),
    )
}

/// Find the tmux handles behind one of a session's terminals, or say why
/// there are none.
///
/// The ONE place a resolved [`TerminalId`] becomes something attachable,
/// which is why the attach, resize, input, and close paths all funnel
/// through it rather than each learning what a tab is.
///
/// Async and OWNING, unlike the pre-tab version which borrowed from the
/// entry. Both changes follow from the same fact: a `SessionEntry` is
/// immutable once published, and a session's tabs are not — they are
/// opened and closed while the entry stands still, possibly by another
/// client. So a tab cannot be a field to borrow; it has to be rediscovered
/// from tmux at the moment of use, which costs one round trip. The AGENT
/// terminal keeps its old cost exactly: it is answered from the entry with
/// no tmux call at all.
///
/// A terminal that does not exist is `NotFound` — the shape `Attach`
/// promises for a stale selector (`TerminalSelector::Tab`'s own contract)
/// — and there is deliberately no fallback to another terminal, because
/// attaching the WRONG one quietly would be worse than failing. A tmux
/// query that FAILS keeps its own error kind instead: "we could not ask"
/// and "it is not there" are different answers, and only one of them
/// means the client should stop retrying.
pub(crate) async fn resolve_terminal(
    sup: &Supervisor,
    entry: &SessionEntry,
    terminal: &TerminalId,
) -> Result<Terminal, RequestError> {
    // The restart-gap case (PLAN_M2.md): this entry was reloaded from
    // SQLite at startup and its tmux session was gone by then. Reporting
    // `NotFound` — rather than fabricating a dead terminal to attach to —
    // is the same "do not guess" discipline SPEC.md applies elsewhere; the
    // session stays visible in the list either way. It is also the gate
    // every TAB lookup passes through first: with no tmux session there is
    // no window to carry a marker, so a tab selector on such a session is
    // not-found for the same underlying reason.
    let agent = entry.terminal.as_ref().ok_or_else(|| {
        RequestError::new(
            ErrorKind::NotFound,
            format!(
                "session {} has no terminal: the supervisor (or its tmux server) restarted \
                 after the agent ended",
                truncate_for_error(&entry.info.id)
            ),
        )
    })?;
    match terminal {
        TerminalId::Agent => Ok(agent.clone()),
        TerminalId::Tab(id) => {
            let states = sup.tmux.pane_states().await.map_err(|e| {
                RequestError::new(
                    error_kind(&e),
                    format!("could not ask tmux which terminal tabs this session has: {e:#}"),
                )
            })?;
            tabs_from_pane_states(&states, &agent.tmux_name)
                .into_iter()
                .find(|tab| tab.id == *id)
                .map(|tab| Terminal {
                    tmux_name: agent.tmux_name.clone(),
                    pane: tab.pane,
                })
                .ok_or_else(|| {
                    RequestError::new(
                        ErrorKind::NotFound,
                        format!(
                            "session {} has no terminal tab {}: it was closed, or a reboot \
                             erased it",
                            truncate_for_error(&entry.info.id),
                            truncate_for_error(id),
                        ),
                    )
                })
        }
    }
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
pub(crate) const DETACH_REASON_TAKEOVER: &str = "another client attached";

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

/// How long a sink client must survive before its death is treated as an
/// isolated incident rather than one of a run of them.
///
/// A sink dies for two very different reasons and the supervisor cannot
/// ask which: something killed the client (a one-off, retry immediately),
/// or the tmux session it attaches to is unavailable (retrying will fail
/// again for a while). This threshold is what tells them apart in
/// practice — a client that attached and then lived a while was healthy,
/// while one that exited at once never really attached — and all it
/// decides is how fast to retry, never whether to.
const SINK_HEALTHY_RUN: Duration = Duration::from_secs(2);

/// First delay before replacing a dead sink, and the unit the backoff
/// doubles from.
///
/// Short enough that an ordinary `kill -9` is invisible to the panes the
/// sink protects: the guarantee lapses only for this long, and only if a
/// per-terminal client happens to be stalled at that exact moment.
const SINK_RETRY_BASE: Duration = Duration::from_millis(200);

/// Ceiling on the respawn backoff.
///
/// This is what bounds the window in which a session has no sink, and so
/// what makes the honest qualification on the isolation guarantee a
/// BOUNDED one rather than an open-ended "eventually" (see
/// [`crate::tmux::SessionSink`], and SPEC_impl.md's respawn note). Chosen
/// low for that reason rather than to be gentle on the machine: the thing
/// being retried is one process spawn against a local socket, so even the
/// pathological case — a session whose tmux server is wedged — costs one
/// short-lived process every five seconds, against a supervisor that is
/// already failing every attach on that session.
const SINK_RETRY_MAX: Duration = Duration::from_secs(5);

/// How long [`Supervisor::ensure_session_sink`] waits for a sink that is
/// between incarnations before failing the attach.
///
/// An attach must not proceed on a handle whose client is mid-respawn: the
/// pane filters it is about to install are safe only while a sink is
/// actually attached, so "there is a handle" is the wrong question and
/// "there is an attached client right now" is the right one. Generous
/// against the backoff above (several retries fit inside it) and still far
/// short of leaving a user's attach hanging with no explanation.
pub(crate) const SINK_READY_TIMEOUT: Duration = Duration::from_secs(15);

/// The delay before the `n`th consecutive replacement attempt: exponential
/// from [`SINK_RETRY_BASE`], capped at [`SINK_RETRY_MAX`].
///
/// A free function so the policy is testable without a tmux server, and
/// because "how long until the next try" is exactly the kind of arithmetic
/// that is easy to get subtly wrong (an off-by-one in the shift, an
/// overflow at large `n`) and impossible to notice in production, where
/// the difference between correct backoff and a tight loop shows up only
/// as machine load during an outage.
fn sink_retry_delay(consecutive_failures: u32) -> Duration {
    // Saturating rather than wrapping: a session that has been failing for
    // hours must land on the cap, not wrap around to no delay at all.
    let factor = 1u32.checked_shl(consecutive_failures.min(31)).unwrap_or(0);
    if factor == 0 {
        return SINK_RETRY_MAX;
    }
    SINK_RETRY_BASE.saturating_mul(factor).min(SINK_RETRY_MAX)
}

/// A session's live sink client, owned jointly by every attachment on that
/// session (PLAN_M4.md order-of-work step 5).
///
/// # Lifetime by refcount, deliberately
///
/// Every [`ActiveAttach`] holds an `Arc` of this, and [`Supervisor::sinks`]
/// holds only a `Weak`. That is what implements "the sink starts when a
/// session gains its first attachment and stops when the last one goes"
/// without a single explicit teardown call: attachments are removed on a
/// dozen paths — takeover, replace, detach, stall, connection loss,
/// restart, tab close, delete — and a scheme that had to be invoked on
/// each of them would eventually be forgotten on the next one added. The
/// last `Arc` to drop aborts the supervising task, whose cancellation
/// drops the client and kills the process through `kill_on_drop`.
///
/// # Self-healing, without an exit
///
/// The task outlives any individual client process: if the sink dies while
/// attachments are still live — a stray `kill -9`, an OOM kill, a tmux
/// server restarting under it — it reattaches, backing off exponentially
/// to [`SINK_RETRY_MAX`] and NEVER giving up. That last part is the whole
/// policy: as long as some attachment holds this handle, a terminal is on
/// screen whose isolation guarantee depends on a sink existing, and a
/// supervisor that had quietly stopped trying would leave that terminal
/// looking perfectly healthy while the guarantee was gone for good.
/// Stopping is the owner's decision, expressed by dropping the last `Arc`,
/// and it needs no cooperation from this task.
///
/// Nothing persists across a supervisor restart: sinks are a property of
/// live attachments, and the next attach builds one.
pub(crate) struct SessionSinkHandle {
    /// The task that keeps a client attached, drains it, and replaces it
    /// when it dies. Aborted by [`Drop`], which is the whole teardown.
    pub(crate) task: tokio::task::JoinHandle<()>,
    /// The live client's process id, or `None` while there is none —
    /// which doubles as this handle's READINESS signal.
    ///
    /// One channel for both because they are one fact: a sink is ready
    /// exactly when it has an attached client, and that client's pid is
    /// what identifies it. A `watch` rather than a plain atomic because
    /// [`Supervisor::ensure_session_sink`] must be able to WAIT for the
    /// transition (an attach that installed pane filters against a handle
    /// whose client was mid-respawn would be filtering with no sink
    /// attached, the one combination that stops tmux reading a pane), and
    /// polling an atomic for that would be a busy-wait with a made-up
    /// interval.
    pub(crate) state: watch::Sender<Option<u32>>,
}

impl Drop for SessionSinkHandle {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Keep `session` sinked until this task is cancelled.
///
/// Takes an already-attached `client` rather than opening its own, because
/// the FIRST attach must be synchronous with the attaching request: a
/// terminal may not turn foreign panes off until tmux already has a client
/// that keeps them readable (see [`crate::tmux::SessionSink`]). Every
/// LATER attach is this task's own business, announced through `state` for
/// whoever is waiting on readiness.
///
/// Runs until cancelled, with no exit of its own — see
/// [`SessionSinkHandle`] for why "give up" is not a state this is allowed
/// to reach.
///
/// `open` is how a replacement is obtained, injected rather than called
/// through the driver directly so the RETRY POLICY is testable without a
/// tmux server: what this loop does is decided entirely by how long a
/// client's `drain` takes to return and whether the next open succeeds,
/// and reproducing "the tmux server is unreachable for ten seconds" with a
/// real server means breaking one and hoping.
pub(crate) async fn run_session_sink<O, F>(
    session: String,
    first: SessionSink,
    state: watch::Sender<Option<u32>>,
    open: O,
) where
    O: Fn(String) -> F,
    F: Future<Output = anyhow::Result<SessionSink>>,
{
    let mut client = first;
    // Counts CONSECUTIVE unhealthy outcomes — a client that died young, or
    // a spawn that failed — and drives nothing but the delay. A client
    // that lived a healthy while resets it, so a session that loses its
    // sink once an hour retries instantly every time, while one whose tmux
    // server is unreachable settles onto the cap.
    let mut consecutive_failures = 0u32;
    loop {
        let started = tokio::time::Instant::now();
        client.drain().await;
        // Announce the gap BEFORE anything else: from here until a
        // replacement is attached, this session has no sink, and an attach
        // that arrives meanwhile must wait rather than install filters.
        //
        // `send_replace`, never `send`: a `watch` sender with no live
        // receivers refuses `send` AND leaves the stored value untouched,
        // and this channel spends most of its life with none — readers
        // subscribe on demand. An earlier draft used `send` here and the
        // published pid simply never changed, so a killed sink looked
        // healthy forever to anything that asked.
        state.send_replace(None);
        // Kill the corpse before sleeping. `drain` returning means the
        // stream ended, which is very nearly always the process exiting —
        // but "very nearly" is not good enough here: a client whose stdout
        // closed while the process lived on would be an ATTACHED tmux
        // client that nothing is reading, i.e. precisely the flow-control
        // victim this whole mechanism exists to guarantee does not exist.
        drop(client);
        if started.elapsed() >= SINK_HEALTHY_RUN {
            consecutive_failures = 0;
        } else {
            consecutive_failures = consecutive_failures.saturating_add(1);
        }
        client = loop {
            tokio::time::sleep(sink_retry_delay(consecutive_failures)).await;
            match open(session.clone()).await {
                Ok(next) => break next,
                Err(e) => {
                    consecutive_failures = consecutive_failures.saturating_add(1);
                    warn!(session = %session, error = %format!("{e:#}"),
                          "could not reattach the session sink; retrying");
                }
            }
        };
        state.send_replace(client.pid());
        warn!(session = %session, "the session sink died and was reattached");
    }
}

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
    /// The forwarder task. A `JoinHandle` rather than an `AbortHandle`
    /// because a takeover must be able to *wait* for the old forwarder to
    /// finish: `abort()` only schedules cancellation, so the old
    /// control-mode client's process would otherwise still be alive when
    /// the new one starts.
    pub(crate) forwarder: tokio::task::JoinHandle<()>,
    /// A second control-mode client, dedicated to this attachment's input,
    /// opened alongside the replay stream in the attach handler. `send`
    /// on it only returns once tmux has actually executed the command —
    /// see [`InputClient`] — which is what lets a failed send here mean
    /// "this attachment's input is broken" rather than "the bytes went
    /// somewhere unconfirmed". Dropped (and so killed, via
    /// `kill_on_drop`) whenever this `ActiveAttach` is removed from the
    /// map, on every teardown path: takeover, detach, connection loss, and
    /// the input-failure branch below.
    pub(crate) input: InputClient,
    /// When this attachment's client asked for its output to stop
    /// (`ControlMsg::PauseOutput`), or `None` while output may flow. Read
    /// by the forwarder task.
    ///
    /// A `watch` rather than a flag plus a `Notify`: the forwarder needs
    /// both "what is the state right now" (to decide whether to pull from
    /// tmux at all) and "wake me when it changes" (to resume promptly),
    /// and a watch channel is exactly those two together with no window
    /// where a notification can be missed between checking and parking.
    ///
    /// Carrying the pause's START INSTANT rather than a bare bool is what
    /// makes [`STALL_DETACH_TIMEOUT`] a hard maximum. Every place the
    /// forwarder can block computes its deadline as `start + timeout`, so
    /// one continuous pause has exactly ONE deadline no matter how many
    /// chunks, phases, or wakeups happen under it. With a bool and a
    /// per-await timer, a client that drains just fast enough to keep the
    /// forwarder moving between chunks would reset the clock forever and
    /// never be detached.
    ///
    /// A repeated `PauseOutput` must NOT overwrite the stored start (see
    /// `set_attachment_paused`), for the same reason: pause spam would
    /// otherwise restart the hard maximum indefinitely.
    pub(crate) pause: watch::Sender<Option<tokio::time::Instant>>,
    /// This attachment's share of its SESSION's sink client.
    ///
    /// Never read. Holding it IS the effect: the sink lives exactly as long
    /// as some attachment on the session holds one of these, which is how
    /// the "first attach starts it, last detach stops it" lifecycle needs
    /// no code on any of the many teardown paths (see
    /// [`SessionSinkHandle`]). Placed here rather than beside the session
    /// entry deliberately — a session with no attachment needs no sink, and
    /// a sink outliving its last viewer would be a control client attached
    /// to a session nobody is watching.
    #[allow(dead_code)]
    pub(crate) sink: Arc<SessionSinkHandle>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tmux::PaneState;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// A pane-state map is a whole tmux SERVER's worth of panes, and this
    /// is what turns it back into ONE session's tab strip. Every filter
    /// here closes a way a window could be misreported as a tab, and the
    /// ORDER is the `SessionInfo::tabs` contract clients derive their
    /// positional labels from.
    ///
    /// The `@10`-before-`@9` case is the one a plain string sort gets
    /// wrong, and it would not show up until a user opened a tenth window
    /// — at which point the whole strip would silently relabel itself.
    #[test]
    fn tabs_are_rediscovered_in_window_creation_order_and_nothing_else_qualifies() {
        const AGENT_SESSION: &str = "2b1f0e4c-0000-4000-8000-000000000001";
        let tab = |n: u8| format!("9c3d5a71-0000-4000-8000-0000000000{n:02x}");
        let state = |session: &str, pane: &str, window: &str, tab: Option<String>| {
            let base = PaneState::for_test(session, pane, window);
            match tab {
                Some(tab) => base.with_tab(&tab),
                None => base,
            }
        };
        let states = HashMap::from([
            // The agent's own window: marked as the agent, never as a tab.
            (
                "%0".to_string(),
                state("fh-mine", "%0", "@0", None).with_agent(AGENT_SESSION),
            ),
            ("%1".to_string(), state("fh-mine", "%1", "@9", Some(tab(9)))),
            (
                "%2".to_string(),
                state("fh-mine", "%2", "@10", Some(tab(10))),
            ),
            // A window a pane process conjured on the private server: no
            // marker, so not a tab (the reason discovery is not positional).
            ("%3".to_string(), state("fh-mine", "%3", "@11", None)),
            // Another session's tab, on the same server.
            (
                "%4".to_string(),
                state("fh-other", "%4", "@12", Some(tab(12))),
            ),
            // A window carrying BOTH markers: the agent's window is never
            // adopted as a tab, whatever else was written on it.
            (
                "%5".to_string(),
                state("fh-mine", "%5", "@13", Some(tab(13))).with_agent(AGENT_SESSION),
            ),
        ]);

        let tabs = tabs_from_pane_states(&states, "fh-mine");
        assert_eq!(
            tabs.iter().map(|t| t.id.clone()).collect::<Vec<_>>(),
            vec![tab(9), tab(10)],
            "creation order comes from the window id's NUMERIC part, so @9 precedes @10"
        );
        assert_eq!(tabs[0].pane, "%1");
        assert_eq!(tabs[1].pane, "%2");
        assert!(
            tabs_from_pane_states(&states, "fh-nobody").is_empty(),
            "a session with no windows on this server has no tabs"
        );
    }

    /// A tab window someone split into two panes must still be ONE tab.
    ///
    /// Splitting is not something farhelm does, but nothing stops a pane's
    /// own processes from doing it on the private server — and reporting
    /// the same tab twice would hand a client two strip entries claiming
    /// one id, both of which it would then try to attach. The lowest pane
    /// is chosen so the answer is stable across repeated rediscovery
    /// rather than dependent on hash-map iteration order.
    #[test]
    fn a_tab_window_holding_several_panes_is_reported_once_by_its_lowest_pane() {
        let tab_id = "9c3d5a71-0000-4000-8000-0000000000ff".to_string();
        let state = |pane: &str| {
            (
                pane.to_string(),
                PaneState::for_test("fh-mine", pane, "@3").with_tab(&tab_id),
            )
        };
        // `%9` is deliberately present alongside `%10`: string ordering
        // would pick `%10`, and the pane the tab resolves to must not
        // depend on how many panes happen to have been created first.
        let states = HashMap::from([state("%9"), state("%10"), state("%11")]);
        let tabs = tabs_from_pane_states(&states, "fh-mine");
        assert_eq!(tabs.len(), 1, "one window is one tab: {tabs:?}");
        assert_eq!(
            tabs[0].pane, "%9",
            "the lowest pane is chosen NUMERICALLY: %9 precedes %10"
        );
    }

    /// The sink respawn backoff grows and then stops growing.
    ///
    /// Both halves matter and neither is visible in production. Without
    /// growth, a session whose tmux server is unreachable is retried in a
    /// tight loop, spawning processes as fast as they can fail; without a
    /// cap, the delay doubles past any useful bound and the "the gap is
    /// bounded" qualification this design puts on its own isolation
    /// guarantee (see `crate::tmux::SessionSink`) stops being true.
    #[test]
    fn the_sink_backoff_grows_to_a_cap_and_stays_there() {
        assert_eq!(sink_retry_delay(0), SINK_RETRY_BASE);
        assert_eq!(sink_retry_delay(1), SINK_RETRY_BASE * 2);
        assert_eq!(sink_retry_delay(2), SINK_RETRY_BASE * 4);
        assert_eq!(sink_retry_delay(1000), SINK_RETRY_MAX);
        // Monotonic and never past the cap, at every step in between — a
        // shift that overflowed would show up here as a delay collapsing
        // back to nothing, which is precisely the tight loop the backoff
        // exists to prevent.
        let mut previous = Duration::ZERO;
        for failures in 0..64 {
            let delay = sink_retry_delay(failures);
            assert!(delay >= previous, "backoff went backwards at {failures}");
            assert!(delay <= SINK_RETRY_MAX, "backoff exceeded its cap");
            assert!(!delay.is_zero(), "backoff reached zero at {failures}");
            previous = delay;
        }
    }

    /// A stand-in sink client that exits at once, so the supervising loop
    /// sees a client that "died young".
    ///
    /// `true` rather than a killed `cat`: a process that has already
    /// exited by the time anyone reads its stdout is the cleanest way to
    /// reach the loop's death branch with no timing to arrange.
    fn dying_fake_sink() -> SessionSink {
        let child = tokio::process::Command::new("true")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawning a fake sink client");
        SessionSink::from_child_for_tests(child)
    }

    /// A stand-in sink client that lives until something kills it.
    ///
    /// `cat` blocking on a stdin the sink holds and never writes to: its
    /// stdout stays open, so `drain` blocks exactly as it does on a
    /// healthy tmux client with nothing to say.
    fn living_fake_sink() -> SessionSink {
        let child = tokio::process::Command::new("cat")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawning a fake sink client");
        SessionSink::from_child_for_tests(child)
    }

    /// Let the supervising task make progress while the virtual clock
    /// advances, until `done` holds or a REAL deadline passes.
    ///
    /// Virtual time is what makes these tests instant, but the fake
    /// clients are real processes whose exits happen in real time, so
    /// neither clock alone is enough: the loop advances only when both
    /// have moved. Hence the interleave, and hence the real-time bound —
    /// a virtual-time bound would spin forever if the process side never
    /// progressed.
    async fn advance_until(mut done: impl FnMut() -> bool, what: &str) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while !done() {
            assert!(std::time::Instant::now() < deadline, "timed out {what}");
            tokio::time::advance(Duration::from_millis(500)).await;
            tokio::task::yield_now().await;
        }
    }

    /// The supervising loop never stops retrying, however long the
    /// failures run.
    ///
    /// This is the headline property of the whole mechanism and the one an
    /// earlier draft got wrong: it had a bounded attempt count, so a
    /// session whose tmux server was briefly unreachable would come back
    /// with its sink permanently gone — every terminal on it still
    /// attached, still streaming, and silently without the pane-read
    /// guarantee its own documentation promises. Nothing in the product
    /// would ever have reported that. So this asserts the loop is still
    /// trying well past any plausible bound, rather than asserting some
    /// particular number of attempts.
    #[tokio::test(start_paused = true)]
    async fn the_sink_supervisor_keeps_retrying_indefinitely() {
        let attempts = Arc::new(AtomicU64::new(0));
        let (state, _rx) = watch::channel(Some(1));
        let counter = Arc::clone(&attempts);
        let task = tokio::spawn(run_session_sink(
            "fh-test".to_string(),
            dying_fake_sink(),
            state.clone(),
            move |_| {
                let counter = Arc::clone(&counter);
                async move {
                    counter.fetch_add(1, Ordering::Relaxed);
                    anyhow::bail!("the tmux server is unreachable")
                }
            },
        ));
        // Far past the five-attempt bound the earlier draft stopped at.
        advance_until(
            || attempts.load(Ordering::Relaxed) > 20,
            "waiting for the sink supervisor to keep retrying",
        )
        .await;
        assert!(
            !task.is_finished(),
            "the supervising task exited on its own; only its owner may end it"
        );
        assert_eq!(
            *state.borrow(),
            None,
            "a session with no attached sink must not report one as ready"
        );

        // ...and the owner's abort is what ends it, which is the only exit
        // this task has.
        task.abort();
        assert!(
            task.await.is_err_and(|e| e.is_cancelled()),
            "the task must end by cancellation, not by returning"
        );
    }

    /// Dropping the last handle is what stops the supervising task — the
    /// other half of "never gives up".
    ///
    /// The retry loop has no exit of its own by design, so the ONLY thing
    /// standing between it and a task that outlives its session is this
    /// `Drop`. A handle whose `Drop` stopped aborting (a field reorder, a
    /// `mem::forget`, a clone held somewhere unnoticed) would leave a
    /// supervisor respawning sink clients for sessions nobody is attached
    /// to, forever, and nothing else in the system would object.
    #[tokio::test(start_paused = true)]
    async fn dropping_the_last_sink_handle_stops_its_supervisor() {
        let (state, _rx) = watch::channel(Some(1));
        let handle = Arc::new(SessionSinkHandle {
            task: tokio::spawn(run_session_sink(
                "fh-test".to_string(),
                living_fake_sink(),
                state,
                |_| async { anyhow::bail!("never called") },
            )),
            state: watch::channel(None).0,
        });
        let second = Arc::clone(&handle);
        drop(handle);
        tokio::task::yield_now().await;
        assert!(
            !second.task.is_finished(),
            "a handle still held by another owner must not stop the sink"
        );
        // The task handle is inside the Arc, so its state has to be
        // sampled through a clone taken before the last drop.
        let task = second.task.abort_handle();
        drop(second);
        tokio::task::yield_now().await;
        assert!(
            task.is_finished(),
            "dropping the last handle must stop the supervising task"
        );
    }

    /// A sink that lived a healthy while resets the backoff, so the next
    /// death is retried promptly rather than at whatever delay the last
    /// outage had climbed to.
    ///
    /// Without the reset, a session that hit one bad patch would carry a
    /// five-second replacement delay for the rest of its life — every
    /// later sink death, however isolated, leaving the pane-read guarantee
    /// down for the full cap instead of a fifth of a second. That is
    /// invisible in production and invisible to every other test here,
    /// because both look like "the sink came back".
    ///
    /// Discriminating on the DELAY rather than on any internal counter:
    /// after a run of failures the backoff is seconds, so an attempt
    /// landing inside a 250 ms window can only mean it was reset.
    #[tokio::test(start_paused = true)]
    async fn a_healthy_sink_run_resets_the_respawn_backoff() {
        let attempts = Arc::new(AtomicU64::new(0));
        // Published by the opener so the test can kill the one client it
        // hands out, which is how the "healthy run" is ended on cue.
        let live_pid = Arc::new(AtomicU64::new(0));
        let (state, _rx) = watch::channel(Some(1));
        let counter = Arc::clone(&attempts);
        let pid_slot = Arc::clone(&live_pid);
        let task = tokio::spawn(run_session_sink(
            "fh-test".to_string(),
            dying_fake_sink(),
            state.clone(),
            move |_| {
                let counter = Arc::clone(&counter);
                let pid_slot = Arc::clone(&pid_slot);
                async move {
                    // Fail three times (so the backoff climbs), then hand
                    // out one long-lived client, then fail forever.
                    let attempt = counter.fetch_add(1, Ordering::Relaxed);
                    if attempt == 3 {
                        let sink = living_fake_sink();
                        pid_slot.store(u64::from(sink.pid().unwrap_or(0)), Ordering::Relaxed);
                        return Ok(sink);
                    }
                    anyhow::bail!("the tmux server is unreachable")
                }
            },
        ));

        advance_until(
            || live_pid.load(Ordering::Relaxed) != 0,
            "waiting for the supervisor to accept a healthy sink",
        )
        .await;
        assert_eq!(
            attempts.load(Ordering::Relaxed),
            4,
            "test premise: three failures must precede the healthy client"
        );

        // Let it be healthy for longer than the threshold, then kill it.
        tokio::time::advance(SINK_HEALTHY_RUN + Duration::from_secs(1)).await;
        tokio::task::yield_now().await;
        let pid = live_pid.load(Ordering::Relaxed);
        // The kill is SYNCHRONOUS deliberately, and it must stay that way.
        // With an AWAITED kill the sequence under `start_paused` is: the
        // test task parks on the child's exit; the supervisor task may
        // observe the sink's EOF first and run its respawn loop up to the
        // backoff sleep, registering the timer; if no task is then ready,
        // tokio's time driver auto-advances the frozen clock straight to
        // that timer — the very delay this test measures — and can do so
        // repeatedly, burning backoff iterations in virtual time before
        // the test task ever wakes. One stolen iteration doubles the
        // delay past the 250ms window below and fails the assertion.
        // Which side won came down to scheduling order (the sink's EOF
        // versus the kill child's exit), which is why this one-offed only
        // on loaded CI runners. The blocking libc call occupies the
        // runtime thread without yielding, so the driver never reaches
        // its park-and-auto-advance logic while the kill runs.
        //
        // SAFETY: the call has no Rust memory-safety preconditions — any
        // pid value yields a normal errno-style result, never a memory
        // access. Pid VALIDITY is a separate behavioral concern (a stale
        // pid could in principle signal an unrelated recycled process),
        // not a safety one.
        let killed = unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
        assert_eq!(killed, 0, "test setup: killing the fake sink");

        // The death must be NOTICED before the delay is measured;
        // otherwise this would time the kill, not the backoff.
        advance_until(
            || state.borrow().is_none(),
            "waiting for the supervisor to notice the healthy sink died",
        )
        .await;
        let before = attempts.load(Ordering::Relaxed);
        tokio::time::advance(SINK_RETRY_BASE + Duration::from_millis(50)).await;
        tokio::task::yield_now().await;
        assert!(
            attempts.load(Ordering::Relaxed) > before,
            "no replacement was attempted within {:?} of a healthy sink dying; the backoff was \
             not reset and is still at the previous outage's delay",
            SINK_RETRY_BASE
        );

        task.abort();
        let _ = task.await;
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
    #[test]
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
    #[test]
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

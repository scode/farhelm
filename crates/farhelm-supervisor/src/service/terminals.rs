//! Terminal identity and the output-reap and sink lifecycles built around it.
//!
//! `TerminalId`/`Terminal` name which of a session's terminals (the agent
//! or one of its tabs) a request means and which tmux handles back it;
//! `AttachmentKey`/`ActiveAttach`/`InputRoute` are the bookkeeping an
//! `Attach` installs and every later `Resize`/`Detach`/input frame looks
//! up by. `SessionSinkHandle`/`run_session_sink` are the per-tmux-session
//! control client every attachment shares, supervised independently of
//! any one attachment's lifetime. The `Supervisor` extension methods below
//! own the lifecycle transitions around those types: output-client cleanup
//! barriers and per-session sink opening, handoff, reaping, and readiness.

use super::core::{RequestError, SessionEntry, Supervisor, error_kind, truncate_for_error};
use crate::store::LastOutcome;
use crate::tmux::{InputClient, ReplayStreamCandidate, SessionSink};
use anyhow::Context;
use farhelm_proto::{ErrorKind, Frame, RestartOffer, TerminalSelector};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex as StdMutex, Weak};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, watch};
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
    /// Whether EVERY pane of this tab's window is dead — the condition
    /// under which the tab is finished as far as the product is concerned
    /// (SPEC.md: a tab whose process exits is reaped automatically).
    /// All-panes rather than any-pane because a split someone made by
    /// hand is still one tab, and one exited half must not condemn the
    /// half that is still running. Consumers split on this: listing paths
    /// hide dead tabs, and the ticker reaps them.
    pub(crate) dead: bool,
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
///   is read (`tmux/control_codec.rs`) rather than here.
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
/// still ONE tab, keyed on its lowest LIVE pane — the handle attach and
/// input flow through must not be an exited half of a split while a live
/// shell sits beside it — falling back to the lowest pane overall only
/// when the whole window is dead (the reap still needs a handle). See the
/// selection comment in the body for the full argument.
///
/// Dead tabs are REPORTED, not filtered: `DiscoveredTab::dead` carries
/// the verdict and the caller decides. Filtering here would blind the
/// consumers that must see corpses — the ticker's reaper (which turns a
/// dead tab into a closed one), the close-capable resolver, and
/// teardown's scope enumeration (`session_tabs_including_dead`) — while
/// the reply-facing listings apply their own hide.
pub(crate) fn tabs_from_pane_states<'a>(
    states: impl IntoIterator<Item = &'a crate::tmux::PaneState>,
    tmux_name: &str,
) -> Vec<DiscoveredTab> {
    /// One tab id's accumulated evidence: the window claiming it and every
    /// pane seen in that window, deferred so the HANDLE pane can be chosen
    /// once all panes are in (see below) rather than by fold order.
    struct Claim {
        window_ordinal: u64,
        /// `(pane_ordinal, dead)` for every pane of the claiming window.
        panes: Vec<(u64, bool)>,
    }
    let mut found: HashMap<String, Claim> = HashMap::new();
    let mut ambiguous: HashSet<String> = HashSet::new();
    for state in states {
        if state.session_name != tmux_name || state.agent.is_some() {
            continue;
        }
        let Some(tab_id) = state.tab.as_deref() else {
            continue;
        };
        match found.entry(tab_id.to_string()) {
            std::collections::hash_map::Entry::Occupied(mut existing) => {
                if existing.get().window_ordinal == state.window_ordinal {
                    // The same window, seen through a second pane: a
                    // split, not a second claimant.
                    existing
                        .get_mut()
                        .panes
                        .push((state.pane_ordinal, state.dead));
                } else {
                    ambiguous.insert(tab_id.to_string());
                }
            }
            std::collections::hash_map::Entry::Vacant(slot) => {
                slot.insert(Claim {
                    window_ordinal: state.window_ordinal,
                    panes: vec![(state.pane_ordinal, state.dead)],
                });
            }
        }
    }
    let mut tabs: Vec<DiscoveredTab> = found
        .into_iter()
        .filter(|(id, _)| !ambiguous.contains(id))
        .map(|(id, claim)| {
            // The handle pane is the lowest LIVE pane when one exists —
            // attach, input, replay, and resize all flow through it, and
            // aiming them at an exited half of a hand-split window while a
            // live shell sits beside it would make the tab look wedged.
            // Only a fully dead window falls back to the lowest pane
            // overall (the reap path needs SOME handle to address the
            // window through). Lowest NUMERICALLY in both cases — `%9`
            // before `%10`, which a string comparison gets backwards — so
            // repeated rediscovery answers the same way.
            let dead = claim.panes.iter().all(|(_, dead)| *dead);
            let pane_ordinal = claim
                .panes
                .iter()
                .filter(|(_, pane_dead)| dead || !pane_dead)
                .map(|(ordinal, _)| *ordinal)
                .min()
                .expect("a claim always holds at least one pane");
            DiscoveredTab {
                id,
                pane: format!("%{pane_ordinal}"),
                pane_ordinal,
                window_ordinal: claim.window_ordinal,
                dead,
            }
        })
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
/// The ONE resolution funnel for [`TerminalId`], so no path has to learn
/// what a tab is on its own — attach, resize, and input use this LIVE-ONLY
/// spelling, while close goes through [`resolve_terminal_for_close`],
/// whose whole difference is that a fully dead tab still resolves (it has
/// to be findable to be destroyed).
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
    resolve_terminal_inner(sup, entry, terminal, TabResolution::LiveOnly).await
}

/// [`resolve_terminal`] for the CLOSE path, which must be able to find a
/// tab whose panes are all dead: an exited tab is exactly what the
/// ticker's reap (and a user's close racing it) has to locate and
/// destroy, while every interactive resolver treats it as already gone
/// (SPEC.md reaps it; offering its "discarded" scrollback through attach
/// would contradict the listing that no longer shows it).
pub(crate) async fn resolve_terminal_for_close(
    sup: &Supervisor,
    entry: &SessionEntry,
    terminal: &TerminalId,
) -> Result<Terminal, RequestError> {
    resolve_terminal_inner(sup, entry, terminal, TabResolution::DeadIncluded).await
}

/// Whether a tab whose window is entirely dead resolves or reads as gone.
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
/// banner, so it is the sentence a user reads when they open a session the
/// reboot took. The wording it replaced claimed the restart happened
/// "after the agent ended", which SPEC.md's reboot contract explicitly
/// forbids inferring: after a reboot the supervisor cannot know whether the
/// agent exited moments before or was killed by the boot, and "interrupted"
/// exists to say exactly that. So the two cases are told apart by the
/// durable outcome, and neither orders the agent's end against the restart.
///
/// An entry loses its terminal on exactly two paths, and the wording
/// follows which one it was, in the same precedence the UI's own
/// terminal-absence decision uses (`terminal_absence` in the session view),
/// so a refusal never contradicts the band on screen:
///
/// - An archived entry (`archive_session` publishes it terminal-less)
///   names the archive: its terminal was removed on purpose, and restart
///   is how a fresh one comes back. Archive outranks the reboot below
///   because it is a deliberate act that stands whatever the boot did.
/// - An entry the boot-id classifier marked [`LastOutcome::Interrupted`]
///   names the reboot and points at the one way forward, restart, which
///   SPEC.md says offers to resume the conversation.
/// - Everything else is the restart gap: the entry was reloaded from the
///   store at supervisor startup and its tmux session was already gone,
///   whether the agent had exited before, the launch's pane never came
///   back, or the tmux server itself died. The restart is a fact there;
///   when the agent ended relative to it is not, so the sentence names the
///   restart and nothing about the agent.
///
/// The reboot case ends with what restart would actually do, per the
/// session's own [`RestartOffer`]: only a session whose conversation was
/// captured can be promised a resume, and SPEC.md forbids implying one
/// where a fresh launch is what the user would get.
fn missing_terminal_message(
    id: &str,
    archived: bool,
    outcome: &LastOutcome,
    offer: RestartOffer,
) -> String {
    let id = truncate_for_error(id);
    if archived {
        format!(
            "session {id} has no terminal: it is archived, which removed its terminal; restart \
             creates a fresh one"
        )
    } else if matches!(outcome, LastOutcome::Interrupted) {
        let restart = match offer {
            RestartOffer::Resume => "restart offers to resume the conversation",
            RestartOffer::FallbackTemplate => "restart runs its configured resume command",
            RestartOffer::FreshOnly => "restart launches a fresh agent in the same directory",
        };
        format!(
            "session {id} has no terminal: its host rebooted and the terminal did not survive; \
             {restart}"
        )
    } else {
        format!(
            "session {id} has no terminal on this host: the supervisor (or its tmux server) \
             restarted and the terminal did not survive"
        )
    }
}

async fn resolve_terminal_inner(
    sup: &Supervisor,
    entry: &SessionEntry,
    terminal: &TerminalId,
    resolution: TabResolution,
) -> Result<Terminal, RequestError> {
    // The restart-gap case (PLAN_M2.md): this entry was reloaded from
    // SQLite at startup and its tmux session was gone by then. Reporting
    // `NotFound` — rather than fabricating a dead terminal to attach to —
    // is the same "do not guess" discipline SPEC.md applies elsewhere; the
    // session stays visible in the list either way. It is also the gate
    // every TAB lookup passes through first: with no tmux session there is
    // no window to carry a marker, so a tab selector on such a session is
    // not-found for the same underlying reason. The wording comes from the
    // durable outcome (`missing_terminal_message`), because a reboot's
    // interruption and an ordinary restart gap are different facts and the
    // user is about to act on which one it was.
    let agent = entry.terminal.as_ref().ok_or_else(|| {
        // Two per-entry mutexes, taken one after the other and never
        // nested: each hold is a read of one small value, which is the
        // rule that makes blocking mutexes safe inside async code here
        // (see `SessionEntry::outcome`).
        let outcome = entry
            .outcome
            .lock()
            .expect("outcome mutex poisoned")
            .clone();
        // The same derivation the restart path uses for the offer it
        // reports on the wire, so the refusal never promises a resume the
        // restart would not perform.
        let offer = entry.snapshot.restart_offer(
            entry
                .capture
                .lock()
                .expect("capture mutex poisoned")
                .committed_conversation(),
        );
        RequestError::new(
            ErrorKind::NotFound,
            missing_terminal_message(&entry.info.id, entry.info.archived, &outcome, offer),
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
            tabs_from_pane_states(states.values(), &agent.tmux_name)
                .into_iter()
                .find(|tab| tab.id == *id)
                // An exited-but-unreaped tab answers exactly like a reaped
                // one on interactive paths: the listing already dropped it,
                // and the reap is normally a tick away (its budget can
                // defer a mass exit a few ticks longer), so a client holding
                // its id must not be able to attach into the corpse and
                // read scrollback the product has declared discarded. The
                // close path keeps seeing it (`TabResolution::DeadIncluded`)
                // because destroying it is how it stops existing at all.
                .filter(|tab| match resolution {
                    TabResolution::LiveOnly => !tab.dead,
                    TabResolution::DeadIncluded => true,
                })
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
///
/// DEFINED from the proto constant a non-displacing attach is refused with
/// rather than repeating its text: a client told it lost the session and a
/// client told it cannot have the session back are learning the same fact,
/// and the only difference is whether it had a socket open at the time.
/// Two literals would be two chances for that identity to lapse — and the
/// browser matches ONE string to decide it lost the session, so a lapse
/// would show up as a refused reconnect falling through to a generic
/// banner and climbing the ladder against a session it can never have.
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
///
/// This is the PRODUCTION value; `ensure_session_sink` actually consults
/// [`crate::service::core::SupervisorTimeouts::sink_ready`], which
/// defaults to this constant. Injectable for the same reason the tmux
/// control-mode budgets are (`SupervisorTimeouts::tmux_exchange` and
/// `tmux_pane_list`): a sink respawn opens a fresh control-mode client
/// through THAT budget, so a test harness widening it must be able to
/// widen this one to match, or a sink genuinely mid-respawn on a loaded
/// runner could still fail this wait despite the retry itself being
/// perfectly healthy.
pub(crate) const SINK_READY_TIMEOUT: Duration = Duration::from_secs(15);

/// The delay after `n` consecutive failures: exponential from
/// [`SINK_RETRY_BASE`], capped at [`SINK_RETRY_MAX`].
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
    // One failure gets the base delay; the exponent grows only after another
    // consecutive failure. Zero is also the base for the initial retry path.
    let exponent = consecutive_failures.saturating_sub(1).min(31);
    let factor = 1u32.checked_shl(exponent).unwrap_or(0);
    if factor == 0 {
        return SINK_RETRY_MAX;
    }
    SINK_RETRY_BASE.saturating_mul(factor).min(SINK_RETRY_MAX)
}

/// Retain a sink client until its output-off boundary and process exit are proven.
///
/// Owner shutdown has no safe fallback: returning an error would drop the
/// `kill_on_drop` child, which can invalidate a pane block still queued inside
/// tmux. This retry tail therefore outlives the request that triggered it and
/// keeps the same client handle until orderly shutdown succeeds.
async fn shutdown_session_sink_until_safe(
    session: &str,
    client: &mut SessionSink,
    reason: &'static str,
) {
    let mut failures = 0u32;
    loop {
        match client.shutdown().await {
            Ok(()) => return,
            Err(error) => {
                failures = failures.saturating_add(1);
                if failures.is_power_of_two() {
                    warn!(
                        session,
                        error = %format!("{error:#}"),
                        failures,
                        reason,
                        "session-sink shutdown is not yet safe; retrying"
                    );
                }
            }
        }
        tokio::time::sleep(sink_retry_delay(failures)).await;
    }
}

/// A session's live sink client, owned jointly by every attachment on that
/// session (PLAN_M4.md order-of-work step 5).
///
/// # Lifetime by refcount, deliberately
///
/// Every [`ActiveAttach`] holds an `Arc` of this. [`Supervisor::sinks`]
/// normally holds the registered live sink's `Weak`; final-owner release
/// replaces that slot with `Reaping`, while ANY unconfirmed reap — including
/// an unregistered client that lost a concurrent first-attach race — replaces
/// it with fail-closed `Failed`. That is what implements "the sink starts
/// when a session gains its first attachment and stops when the last one
/// goes": attachments are removed on a dozen paths — takeover, replace,
/// detach, stall, connection loss, restart, tab close, delete — and the last
/// `Arc` still defines the end of the sink's lifetime. Each path that can
/// release the last owner passes its [`SessionSinkLease`] through its drop
/// path. Takeover and replacement already hold the incoming attachment's
/// reference, so the old attachment merely releases its share and the same
/// sink stays live. Cancellation and unwinding retain the abort-on-drop
/// fallback so cleanup cannot leak the task when no async teardown can run.
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
/// Stopping is the owner's decision: orderly last-owner release asks this
/// task to reap its client, while cancellation aborts the task and relies on
/// `kill_on_drop`. Sink death or reopen failure alone never makes it stop.
///
/// Nothing persists across a supervisor restart: sinks are a property of
/// live attachments, and the next attach builds one.
pub(crate) struct SessionSinkHandle {
    /// Registry key for the client this handle supervises. Teardown needs
    /// it after the attachment has been removed, when no `Terminal` remains
    /// available to name the per-session handoff state.
    pub(crate) tmux_name: String,
    /// The task that keeps a client attached, drains it, and replaces it
    /// when it dies. `Option` lets [`Self::shutdown`] await it despite this
    /// type's abort-on-drop fallback.
    pub(crate) task: Option<tokio::task::JoinHandle<anyhow::Result<()>>>,
    /// Requests orderly teardown when an attachment-ending path releases
    /// the last owner. Cancellation still relies on [`Drop`], because
    /// async work cannot be performed from an arbitrary destructor.
    pub(crate) shutdown: Option<oneshot::Sender<()>>,
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
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}

impl SessionSinkHandle {
    /// Stop the sink client and wait until its process has exited.
    ///
    /// The runtime-owned lease reaper awaits this method and publishes its
    /// result through the per-session registry slot. Successful completion
    /// is therefore the boundary every same-session replacement waits for;
    /// task or process errors remain a fail-closed registry state.
    pub(crate) async fn shutdown(mut self) -> anyhow::Result<()> {
        drop(self.shutdown.take());
        if let Some(task) = self.task.take() {
            return task.await.context("joining the session-sink task")?;
        }
        Ok(())
    }
}

type SinkReapOutcome = Option<Result<(), Arc<str>>>;
type SinkReapSender = watch::Sender<SinkReapOutcome>;
type SinkReapReceiver = watch::Receiver<SinkReapOutcome>;

/// Registry state for one tmux session's sink handoff.
pub(crate) enum SinkRegistryEntry {
    Live(Weak<SessionSinkHandle>),
    Reaping(SinkReapReceiver),
    Failed(Arc<str>),
}

/// The sink registry is synchronously locked because a lease destructor
/// must publish `Reaping` before its last strong reference disappears.
/// Every critical section is memory-only; process work runs after release.
#[derive(Default)]
pub(crate) struct SinkRegistryState {
    entries: HashMap<String, SinkRegistryEntry>,
    /// Candidate opens and reaps that every same-session ensure must wait for.
    pub(crate) candidates: HashMap<String, Vec<SinkReapReceiver>>,
}

impl std::ops::Deref for SinkRegistryState {
    type Target = HashMap<String, SinkRegistryEntry>;

    fn deref(&self) -> &Self::Target {
        &self.entries
    }
}

impl std::ops::DerefMut for SinkRegistryState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.entries
    }
}

pub(crate) type SinkRegistry = Arc<StdMutex<SinkRegistryState>>;

/// A cleanup result that is absent until the runtime-owned reaper settles.
pub(crate) type OutputReapOutcome = Option<Result<(), Arc<str>>>;

/// The stable receiver published before an output client begins shutdown.
pub(crate) type OutputReapReceiver = watch::Receiver<OutputReapOutcome>;

/// The handoff state left by a terminal-output client that could not finish
/// cleanup or whose provisional open has not resolved yet.
pub(crate) enum OutputReapEntry {
    /// A runtime-owned task still holds and drains the old client.
    Reaping(OutputReapReceiver),
    /// Cleanup lost its task or otherwise became unconfirmable.
    Failed(Arc<str>),
}

/// Per-terminal ownership barriers for provisional opens and deferred reaps.
pub(crate) type OutputReapRegistry = Arc<StdMutex<HashMap<AttachmentKey, OutputReapEntry>>>;

/// A published candidate operation whose abandonment is orderly.
///
/// The guard begins before opening and may later own an opened, unregistered
/// client. Cancellation before the opener starts settles it through this
/// guard's own drop path. Once opening begins, the runtime-owned task sends the
/// guard across a oneshot channel; cancellation before or after that send still
/// leaves value drop responsible for settling the barrier and reaping any
/// client rather than falling through the handle's abort-only fallback.
pub(crate) struct SessionSinkCandidate {
    handle: Option<SessionSinkHandle>,
    completion: Option<SinkReapSender>,
    tmux_name: String,
    opening_started: bool,
    registry: SinkRegistry,
    runtime: tokio::runtime::Handle,
}

impl SessionSinkCandidate {
    /// Publish a barrier before a runtime-owned candidate open begins.
    #[cfg(test)]
    pub(crate) fn begin(tmux_name: String, registry: SinkRegistry) -> Self {
        let registry_for_candidate = Arc::clone(&registry);
        let mut state = registry.lock().expect("sink registry poisoned");
        Self::begin_locked(tmux_name, registry_for_candidate, &mut state)
    }

    /// Publish a barrier while the caller's registry decision is locked.
    pub(crate) fn begin_locked(
        tmux_name: String,
        registry: SinkRegistry,
        state: &mut SinkRegistryState,
    ) -> Self {
        let (completion, completion_rx) = watch::channel(None);
        state
            .candidates
            .entry(tmux_name.clone())
            .or_default()
            .push(completion_rx);
        Self {
            handle: None,
            completion: Some(completion),
            tmux_name,
            opening_started: false,
            registry,
            runtime: tokio::runtime::Handle::current(),
        }
    }

    /// Mark that cancellation can no longer prove no process was spawned.
    pub(crate) fn mark_opening_started(&mut self) {
        self.opening_started = true;
    }

    /// Add the client produced by the guarded open operation.
    pub(crate) fn set_handle(&mut self, handle: SessionSinkHandle) {
        assert!(self.handle.replace(handle).is_none(), "candidate set twice");
    }

    /// Resolve an open that returned no installable client handle.
    ///
    /// `outcome` distinguishes confirmed cleanup from an attach that spawned
    /// a process whose exit could not be confirmed.
    pub(crate) fn finish_without_handle(mut self, outcome: Result<(), Arc<str>>) {
        self.finish(outcome);
    }

    /// Transfer an installed client and release its opening barrier.
    pub(crate) fn install(mut self) -> SessionSinkHandle {
        let handle = self.handle.take().expect("the candidate finished opening");
        self.finish(Ok(()));
        handle
    }

    /// Orderly-reap an unregistered client before releasing its barrier.
    pub(crate) async fn shutdown(mut self) -> Result<(), Arc<str>> {
        let outcome = match self.handle.take() {
            Some(handle) => handle
                .shutdown()
                .await
                .map_err(|error| Arc::<str>::from(format!("{error:#}"))),
            None => Ok(()),
        };
        self.finish(outcome.clone());
        outcome
    }

    /// Publish one candidate operation's terminal outcome exactly once.
    fn finish(&mut self, outcome: Result<(), Arc<str>>) {
        if let Some(completion) = self.completion.take() {
            completion.send_replace(Some(outcome));
        }
    }
}

impl Drop for SessionSinkCandidate {
    fn drop(&mut self) {
        let Some(completion) = self.completion.take() else {
            return;
        };
        let handle = self.handle.take();
        let tmux_name = self.tmux_name.clone();
        let opening_started = self.opening_started;
        let registry = Arc::clone(&self.registry);
        self.runtime.spawn(async move {
            let outcome = match handle {
                Some(handle) => handle
                    .shutdown()
                    .await
                    .map_err(|error| Arc::<str>::from(format!("{error:#}"))),
                None if opening_started => Err(Arc::<str>::from(
                    "the runtime-owned session-sink opener stopped after process creation began",
                )),
                None => Ok(()),
            };
            if let Err(error) = &outcome {
                registry
                    .lock()
                    .expect("sink registry poisoned")
                    .insert(tmux_name, SinkRegistryEntry::Failed(Arc::clone(error)));
            }
            completion.send_replace(Some(outcome));
        });
    }
}

/// One provisional or committed attachment's ownership of a session sink.
///
/// Dropping the final lease of the currently registered `Live` handle
/// atomically replaces its weak entry with a reaping barrier, then hands
/// shutdown to a runtime-owned task. A final lease made stale by an existing
/// `Failed` barrier reaps independently and leaves that fail-closed evidence
/// intact. Both paths make failure, cancellation, and unwinding obey the same
/// orderly teardown contract without requiring async destructor code.
pub(crate) struct SessionSinkLease {
    handle: Option<Arc<SessionSinkHandle>>,
    registry: SinkRegistry,
    runtime: tokio::runtime::Handle,
}

impl SessionSinkLease {
    /// Wrap one strong handle as ownership that publishes its final reap.
    pub(crate) fn new(handle: Arc<SessionSinkHandle>, registry: SinkRegistry) -> Self {
        Self {
            handle: Some(handle),
            registry,
            runtime: tokio::runtime::Handle::current(),
        }
    }
}

impl std::ops::Deref for SessionSinkLease {
    type Target = SessionSinkHandle;

    fn deref(&self) -> &Self::Target {
        self.handle
            .as_deref()
            .expect("a live lease owns its handle")
    }
}

impl Drop for SessionSinkLease {
    fn drop(&mut self) {
        let Some(handle) = self.handle.take() else {
            return;
        };
        let tmux_name = handle.tmux_name.clone();
        let (owned, tracked_reaper) = {
            let mut registry = self.registry.lock().expect("sink registry poisoned");
            let registered = registry.get(&tmux_name).is_some_and(|entry| match entry {
                SinkRegistryEntry::Live(registered) => {
                    Weak::ptr_eq(registered, &Arc::downgrade(&handle))
                }
                SinkRegistryEntry::Reaping(_) | SinkRegistryEntry::Failed(_) => false,
            });
            if Arc::strong_count(&handle) != 1 {
                // Release this share while the registry is still locked.
                // Otherwise two simultaneous final drops can both observe
                // two owners, release the lock, and only then decrement the
                // count, leaving nobody to publish the reaping barrier.
                drop(handle);
                return;
            }
            let tracked_reaper = if registered {
                let (done, done_rx) = watch::channel(None);
                let done_identity = done_rx.clone();
                registry.insert(tmux_name.clone(), SinkRegistryEntry::Reaping(done_rx));
                Some((done, done_identity))
            } else {
                None
            };
            (
                Arc::try_unwrap(handle).unwrap_or_else(|_| {
                    unreachable!("the registry lock prevents a new lease after the count check")
                }),
                tracked_reaper,
            )
        };
        let registry = Arc::clone(&self.registry);
        self.runtime.spawn(async move {
            let outcome = owned
                .shutdown()
                .await
                .map_err(|error| Arc::<str>::from(format!("{error:#}")));
            match tracked_reaper {
                Some((done, done_identity)) => {
                    {
                        let mut registry = registry.lock().expect("sink registry poisoned");
                        let same_reaper =
                            registry.get(&tmux_name).is_some_and(|entry| match entry {
                                SinkRegistryEntry::Reaping(current) => {
                                    current.same_channel(&done_identity)
                                }
                                SinkRegistryEntry::Live(_) | SinkRegistryEntry::Failed(_) => false,
                            });
                        if same_reaper {
                            match &outcome {
                                Ok(()) => {
                                    registry.remove(&tmux_name);
                                }
                                Err(error) => {
                                    registry.insert(
                                        tmux_name.clone(),
                                        SinkRegistryEntry::Failed(Arc::clone(error)),
                                    );
                                }
                            }
                        }
                    }
                    done.send_replace(Some(outcome));
                }
                None => {
                    // A stale handle can become final after another sink has
                    // replaced its registry slot. It still needs orderly
                    // reaping; failure poisons the current slot because an
                    // unconfirmed client makes every same-session replacement
                    // unsafe, regardless of which incarnation owned it.
                    if let Err(error) = outcome {
                        registry
                            .lock()
                            .expect("sink registry poisoned")
                            .insert(tmux_name, SinkRegistryEntry::Failed(error));
                    }
                }
            }
        });
    }
}

/// Keep `session` sinked until its owner requests shutdown or cancels it.
///
/// Takes an already-attached `client` rather than opening its own, because
/// the FIRST attach must be synchronous with the attaching request: a
/// terminal may not turn foreign panes off until tmux already has a client
/// that keeps them readable (see [`crate::tmux::SessionSink`]). Every
/// LATER attach is this task's own business, announced through `state` for
/// whoever is waiting on readiness.
///
/// Orderly shutdown is its only normal exit. Sink death and reopen failure
/// never make it give up — see [`SessionSinkHandle`] for why retrying remains
/// mandatory while an attachment owns the handle.
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
    mut shutdown: oneshot::Receiver<()>,
    #[cfg(test)] retrying: Option<watch::Sender<u64>>,
    open: O,
) -> anyhow::Result<()>
where
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
        let ended = tokio::select! {
            _ = client.drain() => tokio::time::Instant::now(),
            _ = &mut shutdown => {
                state.send_replace(None);
                shutdown_session_sink_until_safe(
                    &session,
                    &mut client,
                    "the final attachment released its sink",
                )
                .await;
                return Ok(());
            }
        };
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
        // Reap the corpse before sleeping. `drain` returning means the
        // stream ended, which is very nearly always the process exiting —
        // but "very nearly" is not good enough here: a client whose stdout
        // closed while the process lived on would be an ATTACHED tmux
        // client that nothing is reading, i.e. precisely the flow-control
        // victim this whole mechanism exists to guarantee does not exist.
        //
        // A transient process-control error cannot safely advance to a new
        // client and must not end supervision while attachments remain.
        // Keep the old child handle and retry its reap until exit is proven;
        // an owner shutdown that arrives meanwhile changes only what happens
        // AFTER that proof, never whether the proof is required.
        let mut reap_failures = 0u32;
        let mut stop_after_reap = false;
        loop {
            match client.shutdown().await {
                Ok(()) => break,
                Err(error) => {
                    reap_failures = reap_failures.saturating_add(1);
                    if reap_failures.is_power_of_two() {
                        warn!(session = %session, error = %format!("{error:#}"),
                              failures = reap_failures,
                              "could not reap the dead session sink; retrying");
                    }
                }
            }
            let delay = tokio::time::sleep(sink_retry_delay(reap_failures));
            if stop_after_reap {
                delay.await;
            } else {
                tokio::select! {
                    _ = delay => {}
                    _ = &mut shutdown => stop_after_reap = true,
                }
            }
        }
        if stop_after_reap {
            return Ok(());
        }
        // Cleanup can retry for arbitrarily long. Health is the time the sink
        // actually carried output, frozen when `drain` ended, not how long it
        // took afterward to prove the client was gone.
        if ended.duration_since(started) >= SINK_HEALTHY_RUN {
            consecutive_failures = 0;
        } else {
            consecutive_failures = consecutive_failures.saturating_add(1);
        }
        client = loop {
            #[cfg(test)]
            if let Some(retrying) = &retrying {
                retrying.send_modify(|sequence| *sequence = sequence.wrapping_add(1));
            }
            tokio::select! {
                _ = tokio::time::sleep(sink_retry_delay(consecutive_failures)) => {}
                _ = &mut shutdown => return Ok(()),
            }
            // Finish a started attach exchange even if shutdown arrives.
            // Cancelling `open_session_sink` can drop a spawned tmux child
            // without awaiting its exit, recreating the overlap this
            // orderly path exists to prevent. Its exchange is bounded; once
            // it returns, reap any client it produced before stopping.
            let opened = open(session.clone()).await;
            if matches!(
                shutdown.try_recv(),
                Ok(()) | Err(oneshot::error::TryRecvError::Closed)
            ) {
                if let Ok(mut client) = opened {
                    shutdown_session_sink_until_safe(
                        &session,
                        &mut client,
                        "shutdown arrived during sink replacement",
                    )
                    .await;
                }
                return Ok(());
            }
            match opened {
                Ok(next) => break next,
                Err(e) => {
                    consecutive_failures = consecutive_failures.saturating_add(1);
                    if consecutive_failures.is_power_of_two() {
                        warn!(session = %session, error = %format!("{e:#}"),
                              failures = consecutive_failures,
                              "could not reattach the session sink; retrying");
                    }
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
    /// The forwarder task. Teardown requests cooperative shutdown through
    /// `forwarder_shutdown`, then waits until this task has either reaped the
    /// old control client or transferred it to `forwarder_cleanup`. A
    /// replacement remains behind that receiver's registry barrier.
    pub(crate) forwarder: tokio::task::JoinHandle<()>,
    /// Wakes a blocked forwarder without cancelling its cleanup tail.
    ///
    /// Aborting the task drops and kills its output-bearing tmux client at an
    /// arbitrary point. tmux 3.7b can abort the whole server when that races a
    /// queued pane-output block, so every ordinary teardown signals this watch
    /// instead and lets `Forwarder::run` close the client gracefully.
    pub(crate) forwarder_shutdown: watch::Sender<bool>,
    /// Completion of the output client's safe boundary and process reap.
    ///
    /// Teardown publishes this receiver before signalling the task. The task
    /// may hand work to a longer-lived reaper, but the receiver stays the same,
    /// so request cancellation cannot erase the per-terminal handoff barrier.
    pub(crate) forwarder_cleanup: OutputReapReceiver,
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
    /// Holding it keeps the shared sink alive. Orderly teardown consumes
    /// this share through [`SessionSinkLease`]'s drop path, which reaps the
    /// client only when no other attachment still owns it;
    /// takeover and replacement keep their incoming share, so the sink
    /// spans that handoff unchanged. Placed here rather than beside the
    /// session entry deliberately — a session with no attachment needs no
    /// sink, and a sink outliving its last viewer would be a control client
    /// attached to a session nobody is watching.
    pub(crate) sink: SessionSinkLease,
}

impl ActiveAttach {
    /// Ask the output forwarder to stop at its next cancellation-safe await.
    pub(crate) fn request_forwarder_shutdown(&self) {
        self.forwarder_shutdown.send_replace(true);
    }
}

// Output-reap barriers serialize replacement against terminal-client cleanup.
impl Supervisor {
    /// Publish a terminal's cleanup barrier before requesting shutdown.
    ///
    /// Callers hold `attachments` while removing the corresponding live entry.
    /// Installing a deferred barrier before that lock is released closes the
    /// only gap in which a replacement could see neither the old attachment nor
    /// its still-running cleanup. Join failure is permanent for this supervisor
    /// process: the task lost the only proof that its output client reached the
    /// safe boundary, so retrying an attach would be a guess.
    pub(crate) fn begin_forwarder_shutdown(&self, key: AttachmentKey, attachment: &ActiveAttach) {
        self.track_output_reap(key, attachment.forwarder_cleanup.clone());
        attachment.request_forwarder_shutdown();
    }

    /// Open a provisional replay client without tying its cleanup to a request.
    ///
    /// The barrier is published before the runtime-owned open begins and stays
    /// until the caller installs the returned guard or abandoning it proves the
    /// client safely reaped. The request-facing wait is bounded; timing out does
    /// not cancel the opener or erase the per-terminal barrier.
    pub(crate) async fn open_replay_stream_candidate(
        &self,
        key: AttachmentKey,
        tmux_name: &str,
        pane: &str,
    ) -> anyhow::Result<ReplayStreamCandidate> {
        let (completion, completion_rx) = watch::channel(None);
        self.track_output_reap(key, completion_rx);
        let (send, receive) = tokio::sync::oneshot::channel();
        let tmux = self.tmux.clone();
        let tmux_name = tmux_name.to_string();
        let pane = pane.to_string();
        tokio::spawn(async move {
            match tmux.open_replay_stream_candidate(&tmux_name, &pane).await {
                Ok(mut candidate) => {
                    candidate.set_completion(completion);
                    let _ = send.send(Ok(candidate));
                }
                Err(error) => {
                    completion.send_replace(Some(Ok(())));
                    let _ = send.send(Err(error));
                }
            }
        });
        tokio::time::timeout(self.timeouts.sink_ready, receive)
            .await
            .with_context(|| {
                format!(
                    "the terminal-output client did not finish opening within {:?}",
                    self.timeouts.sink_ready
                )
            })?
            .context("the runtime-owned terminal-output opener stopped")?
    }

    /// Install one reap receiver and own its eventual registry transition.
    fn track_output_reap(&self, key: AttachmentKey, done: OutputReapReceiver) {
        self.output_reaps
            .lock()
            .expect("output-reap registry poisoned")
            .insert(key.clone(), OutputReapEntry::Reaping(done.clone()));

        let registry = Arc::clone(&self.output_reaps);
        tokio::spawn(async move {
            let identity = done.clone();
            let mut done = done;
            let outcome = loop {
                if let Some(outcome) = done.borrow().clone() {
                    break outcome;
                }
                if done.changed().await.is_err() {
                    break Err(Arc::<str>::from(
                        "terminal-output cleanup task ended without a result",
                    ));
                }
            };
            let mut registry = registry.lock().expect("output-reap registry poisoned");
            let same = registry.get(&key).is_some_and(|entry| match entry {
                OutputReapEntry::Reaping(current) => current.same_channel(&identity),
                OutputReapEntry::Failed(_) => false,
            });
            if same {
                match outcome {
                    Ok(()) => {
                        registry.remove(&key);
                    }
                    Err(error) => {
                        registry.insert(key, OutputReapEntry::Failed(error));
                    }
                }
            }
        });
    }

    /// Preserve a join failure as fail-closed evidence for this terminal.
    pub(crate) fn record_forwarder_join(
        &self,
        key: AttachmentKey,
        joined: Result<(), tokio::task::JoinError>,
    ) -> Result<(), Arc<str>> {
        match joined {
            Ok(()) => Ok(()),
            Err(join) => {
                let error = Arc::<str>::from(format!(
                    "terminal {:?} output forwarder panicked or was cancelled: {join}",
                    key.terminal
                ));
                self.output_reaps
                    .lock()
                    .expect("output-reap registry poisoned")
                    .insert(key, OutputReapEntry::Failed(Arc::clone(&error)));
                Err(error)
            }
        }
    }

    /// Whether any terminal in `session` still has an unresolved output client.
    ///
    /// The caller uses this while holding `attachments`; teardown publishes
    /// under that same async lock, making the check atomic with new attachment
    /// installation even though the registry itself uses a synchronous lock.
    pub(crate) fn has_output_reap_for_session(&self, session: &str) -> bool {
        let mut registry = self
            .output_reaps
            .lock()
            .expect("output-reap registry poisoned");
        let settled: Vec<(AttachmentKey, Result<(), Arc<str>>)> = registry
            .iter()
            .filter_map(|(key, entry)| match entry {
                OutputReapEntry::Reaping(done) => {
                    done.borrow().clone().map(|outcome| (key.clone(), outcome))
                }
                OutputReapEntry::Failed(_) => None,
            })
            .collect();
        for (key, outcome) in settled {
            match outcome {
                Ok(()) => {
                    registry.remove(&key);
                }
                Err(error) => {
                    registry.insert(key, OutputReapEntry::Failed(error));
                }
            }
        }
        registry.keys().any(|key| key.session == session)
    }

    /// Whether `key` still has an unresolved output client.
    ///
    /// Output-client overlap is a per-terminal hazard. Restart, tab close, and
    /// attach therefore use this narrower check; archive and delete retain the
    /// session-wide check because they tear down every terminal together.
    pub(crate) fn has_output_reap_for_key(&self, key: &AttachmentKey) -> bool {
        let mut registry = self
            .output_reaps
            .lock()
            .expect("output-reap registry poisoned");
        let settled = registry.get(key).and_then(|entry| match entry {
            OutputReapEntry::Reaping(done) => done.borrow().clone(),
            OutputReapEntry::Failed(_) => None,
        });
        if let Some(outcome) = settled {
            match outcome {
                Ok(()) => {
                    registry.remove(key);
                }
                Err(error) => {
                    registry.insert(key.clone(), OutputReapEntry::Failed(error));
                }
            }
        }
        registry.contains_key(key)
    }

    /// Wait for one terminal's old output client, without pinning a request.
    ///
    /// The runtime-owned reaper keeps running after this budget expires. A
    /// caller gets a retryable failure instead of wedging its connection's
    /// dispatcher behind cleanup that may legitimately retry forever.
    pub(crate) async fn wait_for_output_reap(&self, key: &AttachmentKey) -> Result<(), Arc<str>> {
        let key = key.clone();
        let budget = self.timeouts.sink_ready;
        tokio::time::timeout(budget, async {
            loop {
                let mut done = {
                    let registry = self
                        .output_reaps
                        .lock()
                        .expect("output-reap registry poisoned");
                    match registry.get(&key) {
                        Some(OutputReapEntry::Reaping(done)) => done.clone(),
                        Some(OutputReapEntry::Failed(error)) => {
                            return Err(Arc::clone(error));
                        }
                        None => return Ok(()),
                    }
                };
                loop {
                    if let Some(outcome) = done.borrow().clone() {
                        outcome?;
                        break;
                    }
                    done.changed().await.map_err(|_| {
                        Arc::<str>::from("terminal-output cleanup task ended without a result")
                    })?;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .map_err(|_| {
            Arc::<str>::from(format!(
                "terminal-output cleanup did not finish within {budget:?}"
            ))
        })?
    }

    /// Wait outside `attachments` for every old output client on one session.
    ///
    /// A test seam for the whole-session registry contract. Production archive
    /// and delete fail fast when this state exists; they do not pin a request
    /// while runtime-owned reapers keep retrying.
    #[cfg(test)]
    pub(crate) async fn wait_for_output_reaps(&self, session: &str) -> Result<(), Arc<str>> {
        let budget = self.timeouts.sink_ready;
        tokio::time::timeout(budget, async {
            loop {
                let pending: Vec<OutputReapReceiver> = {
                    let registry = self
                        .output_reaps
                        .lock()
                        .expect("output-reap registry poisoned");
                    let mut pending = Vec::new();
                    for (key, entry) in registry.iter().filter(|(key, _)| key.session == session) {
                        match entry {
                            OutputReapEntry::Reaping(done) => pending.push(done.clone()),
                            OutputReapEntry::Failed(error) => {
                                return Err(Arc::<str>::from(format!(
                                    "terminal {:?} cleanup is unconfirmed: {error}",
                                    key.terminal
                                )));
                            }
                        }
                    }
                    pending
                };
                if pending.is_empty() {
                    return Ok(());
                }

                let mut waits = tokio::task::JoinSet::new();
                for mut done in pending {
                    waits.spawn(async move {
                        loop {
                            if let Some(outcome) = done.borrow().clone() {
                                return outcome;
                            }
                            if done.changed().await.is_err() {
                                return Err(Arc::<str>::from(
                                    "terminal-output cleanup task ended without a result",
                                ));
                            }
                        }
                    });
                }
                while let Some(joined) = waits.join_next().await {
                    match joined {
                        Ok(Ok(())) => {}
                        Ok(Err(error)) => return Err(error),
                        Err(join) => {
                            return Err(Arc::<str>::from(format!(
                                "waiting for terminal-output cleanup failed: {join}"
                            )));
                        }
                    }
                }
                // The watcher that owns registry cleanup may be one scheduling turn
                // behind the completion we just observed.
                tokio::task::yield_now().await;
            }
        })
        .await
        .map_err(|_| {
            Arc::<str>::from(format!(
                "terminal-output cleanup for session {session} did not finish within {budget:?}"
            ))
        })?
    }
}

// The session-sink lifecycle keeps pane filters backed by a live reader.
impl Supervisor {
    /// Open a candidate in a task that survives cancellation of its caller.
    ///
    /// Opening a tmux control client spawns a process before its bounded
    /// attach exchange completes. If the request awaiting that exchange is
    /// cancelled, the runtime-owned task finishes it. `open_session_sink`
    /// retains and retries cleanup for any child whose attach fails, so this
    /// task cannot release the candidate barrier until returning an attached
    /// client or a failure whose child is confirmed gone.
    async fn open_sink_candidate(
        &self,
        tmux_name: &str,
        candidate: SessionSinkCandidate,
    ) -> anyhow::Result<SessionSinkCandidate> {
        let (send, receive) = tokio::sync::oneshot::channel();
        let tmux = self.tmux.clone();
        let tmux_name = tmux_name.to_string();
        let waiting_for = tmux_name.clone();
        tokio::spawn(async move {
            let mut candidate = candidate;
            candidate.mark_opening_started();
            let outcome: anyhow::Result<SessionSinkHandle> = async {
                let client = tmux.open_session_sink(&tmux_name).await?;
                let (state, _) = watch::channel(client.pid());
                let (shutdown, shutdown_rx) = tokio::sync::oneshot::channel();
                let replacement_tmux = tmux.clone();
                Ok(SessionSinkHandle {
                    tmux_name: tmux_name.clone(),
                    task: Some(tokio::spawn(run_session_sink(
                        tmux_name.clone(),
                        client,
                        state.clone(),
                        shutdown_rx,
                        #[cfg(test)]
                        None,
                        move |name| {
                            let tmux = replacement_tmux.clone();
                            async move { tmux.open_session_sink(&name).await }
                        },
                    ))),
                    shutdown: Some(shutdown),
                    state,
                })
            }
            .await;
            match outcome {
                Ok(handle) => {
                    candidate.set_handle(handle);
                    // If the receiver vanished before OR after this send,
                    // dropping the guard reaps the client before completing
                    // the already-published candidate barrier.
                    let _ = send.send(Ok(candidate));
                }
                Err(error) => {
                    candidate.finish_without_handle(Ok(()));
                    let _ = send.send(Err(error));
                }
            }
        });
        tokio::time::timeout(self.timeouts.sink_ready, receive)
            .await
            .with_context(|| {
                format!(
                    "the session-sink opener for {waiting_for} did not finish within {:?}",
                    self.timeouts.sink_ready
                )
            })?
            .context("the runtime-owned session-sink opener stopped")?
    }

    /// Reap an unregistered candidate, poisoning the session on failure.
    ///
    /// Concurrent first attaches may each open a control client before one
    /// wins the registry race. A losing client is outside the registered
    /// lease protocol, but its unconfirmed exit is just as dangerous as a
    /// registered reap failure: either one can overlap every later client
    /// for this tmux session. Recording `Failed` makes that uncertainty a
    /// durable in-process barrier instead of an error seen by only one caller.
    async fn reap_competing_sink(
        &self,
        tmux_name: &str,
        candidate: SessionSinkCandidate,
    ) -> anyhow::Result<()> {
        let sinks = Arc::clone(&self.sinks);
        let tmux_name = tmux_name.to_string();
        tokio::spawn(async move {
            if let Err(message) = candidate.shutdown().await {
                sinks.lock().expect("sink registry poisoned").insert(
                    tmux_name.clone(),
                    SinkRegistryEntry::Failed(Arc::clone(&message)),
                );
                anyhow::bail!(
                    "a competing session sink for {tmux_name} could not be reaped: {message}"
                );
            }
            Ok(())
        })
        .await
        .context("joining the competing session-sink reaper")?
    }

    /// Wait for every candidate open or reap already published for a session.
    ///
    /// Candidate operations live beside, not instead of, the registered sink
    /// state: a live winner and an abandoned loser can coexist briefly. Every
    /// ensure passes this barrier before opening and again before returning,
    /// so neither case exposes a client while that loser is still unresolved.
    async fn await_sink_candidates(&self, tmux_name: &str) -> anyhow::Result<()> {
        let sink_ready = self.timeouts.sink_ready;
        let deadline = tokio::time::Instant::now() + sink_ready;
        loop {
            let mut pending = {
                let mut sinks = self.sinks.lock().expect("sink registry poisoned");
                if let Some(candidates) = sinks.candidates.get_mut(tmux_name) {
                    candidates.retain(|done| !matches!(&*done.borrow(), Some(Ok(()))));
                    if candidates.is_empty() {
                        sinks.candidates.remove(tmux_name);
                        Vec::new()
                    } else {
                        candidates.clone()
                    }
                } else {
                    Vec::new()
                }
            };
            if pending.is_empty() {
                return Ok(());
            }
            let outcome = tokio::time::timeout_at(deadline, async {
                for done in &mut pending {
                    loop {
                        if let Some(outcome) = done.borrow().clone() {
                            outcome?;
                            break;
                        }
                        done.changed().await.map_err(|_| {
                            Arc::<str>::from(
                                "a candidate sink lost its reaper before process exit was confirmed",
                            )
                        })?;
                    }
                }
                Ok::<(), Arc<str>>(())
            })
            .await;
            match outcome {
                Ok(Ok(())) => {}
                Ok(Err(message)) => anyhow::bail!(
                    "a candidate session sink for {tmux_name} could not be reaped: {message}"
                ),
                Err(_) => anyhow::bail!(
                    "a candidate session sink for {tmux_name} did not settle within \
                     {sink_ready:?}"
                ),
            }
        }
    }

    /// The sink client for `tmux_name`, attaching one if this is the
    /// session's first attachment (see [`SessionSinkHandle`]).
    ///
    /// The caller must hold the returned lease for as long as its attachment
    /// lives, and must call this BEFORE opening any per-terminal client
    /// for the session: the pane filter those clients install is only safe
    /// while a sink is already attached (see [`crate::tmux::SessionSink`]).
    ///
    /// # What "ensure" promises
    ///
    /// Not merely that a handle exists — that a client is ATTACHED RIGHT
    /// NOW. A handle whose sink is mid-respawn is the one state in which
    /// installing pane filters is actively harmful (filters on, sink off,
    /// and tmux stops reading a pane nobody is watching), so this waits
    /// out that window rather than handing back a handle that is only
    /// nominally healthy. The wait is bounded by
    /// [`super::core::SupervisorTimeouts::sink_ready`] (production:
    /// [`SINK_READY_TIMEOUT`]) and its expiry
    /// fails the attach loudly; it is not a state a healthy host reaches,
    /// and pretending otherwise would mean attaching into exactly the
    /// configuration the wait exists to avoid.
    ///
    /// # Locking
    ///
    /// Takes no lock but its own and must not be called while `attachments`
    /// is held (see the attach handler's cost note). The synchronous registry
    /// lock covers memory-only state transitions and is never held across a
    /// spawn, reply exchange, readiness wait, or reap. Candidate inspection,
    /// registered-sink lookup, and a missing-sink reservation share one such
    /// critical section, so only one first attach opens a client; later callers
    /// wait on its per-session barrier without delaying unrelated sessions.
    /// Reaping an old registered sink uses the same per-session waiting rule.
    ///
    /// # Failure
    ///
    /// Failing here fails the attach, which is a deliberate choice over
    /// degrading silently. Ordinary open failures — a vanished tmux session,
    /// an unresponsive server, exhausted process limits — are early news of
    /// conditions that would fail the per-terminal clients opened next.
    /// Lifecycle failures are different: an earlier, competing, or abandoned
    /// sink whose process exit was not confirmed leaves a fail-closed barrier,
    /// even when tmux is otherwise healthy, because opening another client
    /// would risk the overlap this handoff exists to prevent. A runtime-owned
    /// opener or reaper stopping unexpectedly is surfaced for the same reason.
    pub(crate) async fn ensure_session_sink(
        &self,
        tmux_name: &str,
    ) -> anyhow::Result<SessionSinkLease> {
        enum Lookup {
            Candidate,
            Live(Arc<SessionSinkHandle>),
            Reaping(watch::Receiver<Option<Result<(), Arc<str>>>>),
            Failed(Arc<str>),
            Missing(SessionSinkCandidate),
        }

        let lease = loop {
            self.await_sink_candidates(tmux_name).await?;
            if let Some(gate) = &self.seams.sink_lookup_gate {
                gate().await;
            }
            let lookup = {
                let registry = Arc::clone(&self.sinks);
                let mut sinks = registry.lock().expect("sink registry poisoned");
                Self::prune_dead_sinks(&mut sinks);
                let candidate_pending = sinks.candidates.get(tmux_name).is_some_and(|candidates| {
                    candidates
                        .iter()
                        .any(|done| !matches!(&*done.borrow(), Some(Ok(()))))
                });
                if candidate_pending {
                    Lookup::Candidate
                } else {
                    sinks.candidates.remove(tmux_name);
                    match sinks.get(tmux_name) {
                        Some(SinkRegistryEntry::Live(handle)) => {
                            handle.upgrade().map(Lookup::Live).unwrap_or_else(|| {
                                Lookup::Missing(SessionSinkCandidate::begin_locked(
                                    tmux_name.to_string(),
                                    Arc::clone(&registry),
                                    &mut sinks,
                                ))
                            })
                        }
                        Some(SinkRegistryEntry::Reaping(done)) => Lookup::Reaping(done.clone()),
                        Some(SinkRegistryEntry::Failed(error)) => Lookup::Failed(Arc::clone(error)),
                        None => Lookup::Missing(SessionSinkCandidate::begin_locked(
                            tmux_name.to_string(),
                            Arc::clone(&registry),
                            &mut sinks,
                        )),
                    }
                }
            };
            let candidate = match lookup {
                Lookup::Candidate => {
                    if let Some(gate) = &self.seams.sink_candidate_wait_gate {
                        gate().await;
                    }
                    continue;
                }
                Lookup::Live(handle) => {
                    break SessionSinkLease::new(handle, Arc::clone(&self.sinks));
                }
                Lookup::Reaping(mut done) => {
                    let sink_ready = self.timeouts.sink_ready;
                    let outcome = tokio::time::timeout(sink_ready, async {
                        loop {
                            if let Some(outcome) = done.borrow().clone() {
                                return Some(outcome);
                            }
                            if done.changed().await.is_err() {
                                return None;
                            }
                        }
                    })
                    .await;
                    match outcome {
                        Ok(Some(Ok(()))) => continue,
                        Ok(Some(Err(message))) => anyhow::bail!(
                            "the previous session sink for {tmux_name} could not be reaped: \
                             {message}"
                        ),
                        Ok(None) => anyhow::bail!(
                            "the previous session sink for {tmux_name} lost its reaper before \
                             process exit was confirmed"
                        ),
                        Err(_) => anyhow::bail!(
                            "the previous session sink for {tmux_name} did not finish shutting \
                             down within {sink_ready:?}"
                        ),
                    }
                }
                Lookup::Failed(message) => anyhow::bail!(
                    "the previous session sink for {tmux_name} could not be reaped: {message}"
                ),
                Lookup::Missing(candidate) => candidate,
            };
            if let Some(gate) = &self.seams.sink_reservation_gate {
                gate().await;
            }

            {
                let candidate = self.open_sink_candidate(tmux_name, candidate).await?;
                enum Install {
                    Winner(Arc<SessionSinkHandle>),
                    Reaping,
                    Failed(Arc<str>),
                    Installed(Arc<SessionSinkHandle>),
                }
                let mut candidate = Some(candidate);
                // Re-check under the lock: another first-attach may have
                // finished while this one was spawning. Whoever is already
                // registered wins. Reap the losing client before returning;
                // abort-on-drop requests a kill but would let both first
                // attaches overlap until that process actually exited.
                let install = {
                    let mut sinks = self.sinks.lock().expect("sink registry poisoned");
                    Self::prune_dead_sinks(&mut sinks);
                    match sinks.get(tmux_name) {
                        Some(SinkRegistryEntry::Live(winner)) if winner.strong_count() > 0 => {
                            Install::Winner(
                                winner
                                    .upgrade()
                                    .expect("a positive strong count must upgrade"),
                            )
                        }
                        Some(SinkRegistryEntry::Reaping(_)) => Install::Reaping,
                        Some(SinkRegistryEntry::Failed(error)) => {
                            Install::Failed(Arc::clone(error))
                        }
                        _ => {
                            let handle =
                                Arc::new(candidate.take().expect("candidate available").install());
                            sinks.insert(
                                tmux_name.to_string(),
                                SinkRegistryEntry::Live(Arc::downgrade(&handle)),
                            );
                            Install::Installed(handle)
                        }
                    }
                };
                match install {
                    Install::Winner(winner) => {
                        let winner = SessionSinkLease::new(winner, Arc::clone(&self.sinks));
                        self.reap_competing_sink(
                            tmux_name,
                            candidate
                                .take()
                                .expect("a losing candidate remains available"),
                        )
                        .await?;
                        break winner;
                    }
                    Install::Reaping => {
                        self.reap_competing_sink(
                            tmux_name,
                            candidate
                                .take()
                                .expect("a losing candidate remains available"),
                        )
                        .await?;
                        continue;
                    }
                    Install::Failed(error) => {
                        self.reap_competing_sink(
                            tmux_name,
                            candidate
                                .take()
                                .expect("a losing candidate remains available"),
                        )
                        .await?;
                        anyhow::bail!(
                            "the previous session sink for {tmux_name} could not be reaped: \
                             {error}"
                        );
                    }
                    Install::Installed(handle) => {
                        break SessionSinkLease::new(handle, Arc::clone(&self.sinks));
                    }
                }
            }
        };
        self.await_sink_candidates(tmux_name).await?;
        // Readiness, whether the handle is new (already attached, so this
        // returns at once) or adopted (possibly mid-respawn).
        let mut state = lease.state.subscribe();
        if state.borrow().is_none() {
            let sink_ready = self.timeouts.sink_ready;
            let ready = tokio::time::timeout(sink_ready, async {
                while state.changed().await.is_ok() {
                    if state.borrow().is_some() {
                        return true;
                    }
                }
                false
            })
            .await;
            match ready {
                Ok(true) => {}
                Ok(false) => anyhow::bail!(
                    "the session sink for {tmux_name} stopped being supervised while this \
                     attach waited for it; attach again"
                ),
                Err(_) => anyhow::bail!(
                    "the session sink for {tmux_name} did not come back within \
                     {sink_ready:?}; the tmux server is not answering"
                ),
            }
        }
        Ok(lease)
    }

    /// Drop registry entries whose handle is gone.
    ///
    /// Dead `Live` entries carry no owner and completed successful reaps no
    /// longer carry a handoff, so both may be discarded. An in-progress
    /// reap and a failed reap are deliberately retained: the former blocks
    /// overlap until completion, while the latter is the fail-closed proof
    /// that process exit was never confirmed. Opportunistic rather than
    /// scheduled because every path that can grow the map passes here.
    fn prune_dead_sinks(sinks: &mut HashMap<String, SinkRegistryEntry>) {
        sinks.retain(|_, entry| match entry {
            SinkRegistryEntry::Live(handle) => handle.strong_count() > 0,
            SinkRegistryEntry::Reaping(done) => !matches!(&*done.borrow(), Some(Ok(()))),
            SinkRegistryEntry::Failed(_) => true,
        });
    }

    /// The process id of `tmux_name`'s live sink client, or `None` when
    /// that session has no sink (no attachments) or its sink is between
    /// incarnations.
    ///
    /// A test seam with no production caller: the sink's whole contract is
    /// about a PROCESS being attached, and neither its presence, its
    /// absence, nor its replacement after a `kill -9` is observable from
    /// the wire protocol. Reads the registry rather than a live tmux query
    /// so a test can tell "the supervisor believes it has a sink" apart
    /// from "some client happens to be attached".
    pub fn session_sink_pid(&self, tmux_name: &str) -> Option<u32> {
        let sinks = self.sinks.lock().expect("sink registry poisoned");
        let Some(SinkRegistryEntry::Live(handle)) = sinks.get(tmux_name) else {
            return None;
        };
        let handle = handle.upgrade()?;
        *handle.state.borrow()
    }

    /// How many registered sink-state entries the registry is holding.
    ///
    /// A test seam for the churn test that pins [`Self::prune_dead_sinks`]
    /// doing its job: "the map stays bounded" is not observable in any
    /// other way, and a leak here would be invisible until a long-lived
    /// supervisor's memory made it obvious. Transient candidate-operation
    /// barriers are deliberately excluded; their own tests inspect their
    /// completion boundary rather than treating in-flight work as a leak.
    pub fn session_sink_registry_len(&self) -> usize {
        self.sinks.lock().expect("sink registry poisoned").len()
    }
}

#[cfg(test)]
mod tests {
    use super::super::core::tests::{StateDir, dummy_exe};
    use super::super::core::{SupervisorSeams, SupervisorTimeouts};
    use super::*;
    use crate::tmux::PaneState;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    /// The missing-terminal refusal tells a reboot apart from a restart
    /// gap, and never orders the agent's end against either.
    ///
    /// This is the sentence the browser shows when an interrupted session
    /// is opened, so its two claims are pinned: an `Interrupted` outcome
    /// names the reboot and the restart-with-resume way forward, while
    /// every other outcome says only that this host has no terminal. The
    /// negative assertion guards the regression this replaced — wording
    /// that stated the restart came "after the agent ended", an ordering
    /// SPEC.md says the supervisor cannot know after a reboot.
    #[farhelm_testtrace::test]
    fn missing_terminal_wording_follows_the_durable_outcome() {
        let resumable = missing_terminal_message(
            "s-1",
            false,
            &LastOutcome::Interrupted,
            RestartOffer::Resume,
        );
        assert!(resumable.contains("host rebooted"), "{resumable}");
        assert!(
            resumable.contains("restart offers to resume"),
            "{resumable}"
        );
        // Only a captured conversation may be promised a resume: the other
        // two offers name what restart would really do instead.
        let fresh = missing_terminal_message(
            "s-1",
            false,
            &LastOutcome::Interrupted,
            RestartOffer::FreshOnly,
        );
        assert!(fresh.contains("host rebooted"), "{fresh}");
        assert!(
            fresh.contains("fresh agent") && !fresh.contains("resume"),
            "{fresh}"
        );
        let template = missing_terminal_message(
            "s-1",
            false,
            &LastOutcome::Interrupted,
            RestartOffer::FallbackTemplate,
        );
        assert!(template.contains("configured resume command"), "{template}");

        // An archived entry lost its terminal to the archive, not to any
        // restart, and says so; the reboot still wins when both apply.
        let archived = missing_terminal_message(
            "s-1",
            true,
            &LastOutcome::Exited {
                exit_code: None,
                annotation: Some("stopped by user".to_string()),
            },
            RestartOffer::FreshOnly,
        );
        assert!(archived.contains("archived"), "{archived}");
        assert!(!archived.contains("restarted"), "{archived}");
        // Archive outranks the reboot, matching the UI's `terminal_absence`
        // precedence: the band on screen says "archived", so must this.
        let archived_after_reboot =
            missing_terminal_message("s-1", true, &LastOutcome::Interrupted, RestartOffer::Resume);
        assert!(
            archived_after_reboot.contains("archived")
                && !archived_after_reboot.contains("rebooted"),
            "{archived_after_reboot}"
        );

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
            let text = missing_terminal_message("s-1", false, &other, RestartOffer::FreshOnly);
            assert!(
                text.contains("has no terminal on this host"),
                "{other:?}: {text}"
            );
            assert!(
                !text.contains("rebooted") && !text.contains("archived"),
                "{other:?} must claim neither a reboot nor an archive: {text}"
            );
        }
        for text in [
            resumable,
            archived,
            missing_terminal_message("s-1", false, &LastOutcome::Running, RestartOffer::FreshOnly),
        ] {
            assert!(
                !text.contains("after the agent ended"),
                "no wording may order the agent's end against the restart: {text}"
            );
        }
    }

    /// Records whether a gated test future was cancelled before release.
    struct DropFlag(Arc<AtomicBool>);

    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Relaxed);
        }
    }

    /// A replacement cannot pass a published output-reap barrier early.
    ///
    /// The forwarder may return after handing its client to a runtime-owned
    /// reaper. This test pins the registry half of that handoff: waiting stays
    /// blocked while the result is absent, then the successful result removes
    /// the barrier instead of leaving a terminal permanently unattached.
    #[farhelm_testtrace::test]
    async fn an_output_reap_barrier_blocks_until_cleanup_succeeds() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let key = AttachmentKey::new("session", TerminalId::Agent);
        let (done, done_rx) = watch::channel(None);
        sup.track_output_reap(key, done_rx);

        let waiting_sup = Arc::clone(&sup);
        let waiting =
            tokio::spawn(async move { waiting_sup.wait_for_output_reaps("session").await });
        tokio::task::yield_now().await;
        assert!(
            !waiting.is_finished(),
            "an unresolved client cleanup must hold replacement attach behind its barrier"
        );

        done.send_replace(Some(Ok(())));
        tokio::time::timeout(Duration::from_secs(2), waiting)
            .await
            .expect("the successful cleanup must release its waiter")
            .expect("joining the output-reap waiter")
            .expect("successful cleanup must permit replacement");
        assert!(
            !sup.has_output_reap_for_session("session"),
            "a successful cleanup must remove its registry entry"
        );
    }

    /// Lost cleanup proof remains a durable fail-closed attachment barrier.
    ///
    /// Treating a failed reaper like success would recreate the overlapping
    /// control-client race on the next attach. The error must therefore reach
    /// the waiter and remain in the registry for every later attempt.
    #[farhelm_testtrace::test]
    async fn a_failed_output_reap_remains_fail_closed() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let key = AttachmentKey::new("session", TerminalId::Agent);
        let (done, done_rx) = watch::channel(None);
        sup.track_output_reap(key, done_rx);
        done.send_replace(Some(Err(Arc::<str>::from("injected cleanup loss"))));

        let error = sup
            .wait_for_output_reaps("session")
            .await
            .expect_err("unconfirmed cleanup must refuse replacement");
        assert!(
            error.contains("injected cleanup loss"),
            "the refusal must preserve the cause: {error}"
        );
        assert!(
            sup.has_output_reap_for_session("session"),
            "a cleanup failure must remain visible after the first waiter returns"
        );
        assert!(
            sup.wait_for_output_reaps("session").await.is_err(),
            "later attaches must see the same fail-closed state"
        );
    }

    /// An unresolved tab client does not block the agent terminal's attach.
    ///
    /// Output-client overlap is per pane, even though both clients attach to
    /// the same tmux session. A session-wide check here would let one stuck tab
    /// make every otherwise-independent terminal unavailable.
    #[farhelm_testtrace::test]
    async fn output_reap_barriers_are_scoped_to_one_terminal() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let tab = AttachmentKey::new("session", TerminalId::Tab("tab-1".to_string()));
        let agent = AttachmentKey::new("session", TerminalId::Agent);
        let (_done, done_rx) = watch::channel(None);
        sup.track_output_reap(tab.clone(), done_rx);

        assert!(sup.has_output_reap_for_key(&tab));
        assert!(
            !sup.has_output_reap_for_key(&agent),
            "a tab cleanup barrier must not cover the agent terminal"
        );
        tokio::time::timeout(Duration::from_secs(1), sup.wait_for_output_reap(&agent))
            .await
            .expect("the unrelated terminal returns immediately")
            .expect("the unrelated terminal has no cleanup failure");
    }

    /// A request stops waiting while its runtime-owned output reaper remains.
    ///
    /// Reapers intentionally retry forever when tmux cannot acknowledge the
    /// safe boundary. The connection dispatcher must still regain control, and
    /// the retained registry entry must keep later replacement attempts closed.
    #[farhelm_testtrace::test]
    async fn output_reap_waits_are_bounded_without_erasing_the_barrier() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe_and_timeouts(
            state.path(),
            dummy_exe(),
            SupervisorTimeouts {
                sink_ready: Duration::from_millis(25),
                ..SupervisorTimeouts::default()
            },
        )
        .await
        .expect("supervisor");
        let key = AttachmentKey::new("session", TerminalId::Agent);
        let (_done, done_rx) = watch::channel(None);
        sup.track_output_reap(key.clone(), done_rx);

        let error = sup
            .wait_for_output_reap(&key)
            .await
            .expect_err("an unresolved reaper must exhaust the request budget");
        assert!(
            error.contains("did not finish within"),
            "the timeout must describe the retained cleanup boundary: {error}"
        );
        assert!(
            sup.has_output_reap_for_key(&key),
            "timing out the request must not erase its fail-closed barrier"
        );
    }

    /// A competing first-attach caller does not return before its client is
    /// confirmed shut down.
    ///
    /// The candidate is unregistered, so the per-session lease barrier cannot
    /// protect this boundary. The runtime-owned competing reaper must still
    /// keep the losing request pending until its task acknowledges teardown.
    #[farhelm_testtrace::test]
    async fn a_competing_sink_reaper_waits_for_shutdown_before_returning() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let (shutdown, shutdown_rx) = tokio::sync::oneshot::channel();
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let task_entered = Arc::clone(&entered);
        let task_release = Arc::clone(&release);
        let candidate = SessionSinkHandle {
            tmux_name: "fh-test".to_string(),
            task: Some(tokio::spawn(async move {
                let _ = shutdown_rx.await;
                task_entered.notify_one();
                task_release.notified().await;
                Ok(())
            })),
            shutdown: Some(shutdown),
            state: watch::channel(Some(1)).0,
        };
        let mut guarded =
            SessionSinkCandidate::begin("fh-test".to_string(), Arc::clone(&sup.sinks));
        guarded.set_handle(candidate);
        let future = sup.reap_competing_sink("fh-test", guarded);
        tokio::pin!(future);

        tokio::select! {
            result = future.as_mut() => panic!("competing reap returned early: {result:?}"),
            _ = entered.notified() => {}
        }
        let remained_pending = std::future::poll_fn(|cx| {
            std::task::Poll::Ready(future.as_mut().poll(cx).is_pending())
        })
        .await;
        assert!(
            remained_pending,
            "the losing attach returned before its candidate finished shutting down"
        );

        release.notify_one();
        future.await.expect("the competing sink is reaped");
    }

    /// A competing client whose exit is unconfirmed blocks every later open.
    ///
    /// The failure must survive the losing request and be consulted by the
    /// next ensure call; otherwise one caller would see an error while another
    /// immediately created the overlapping client that error warned about.
    #[farhelm_testtrace::test]
    async fn a_failed_competing_reap_poison_is_refused_by_the_next_ensure() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let (shutdown, shutdown_rx) = tokio::sync::oneshot::channel();
        let candidate = SessionSinkHandle {
            tmux_name: "fh-test".to_string(),
            task: Some(tokio::spawn(async move {
                let _ = shutdown_rx.await;
                anyhow::bail!("the competing process exit was not confirmed")
            })),
            shutdown: Some(shutdown),
            state: watch::channel(Some(1)).0,
        };
        let mut guarded =
            SessionSinkCandidate::begin("fh-test".to_string(), Arc::clone(&sup.sinks));
        guarded.set_handle(candidate);

        assert!(
            sup.reap_competing_sink("fh-test", guarded).await.is_err(),
            "the losing request must receive the reap failure"
        );
        assert!(matches!(
            sup.sinks.lock().expect("sink registry").get("fh-test"),
            Some(SinkRegistryEntry::Failed(_))
        ));
        let error = match sup.ensure_session_sink("fh-test").await {
            Ok(_) => panic!("a failed reap must prevent another same-session client"),
            Err(error) => error,
        };
        assert!(
            format!("{error:#}").contains("could not be reaped"),
            "the next attach must report the retained reap failure: {error:#}"
        );
    }

    /// Cancelling a provisional readiness wait publishes final-owner reaping.
    ///
    /// A mid-respawn handle is a real owner even though its attach has not
    /// committed yet. If the last committed lease disappears and that wait is
    /// then cancelled, the provisional lease must install `Reaping`
    /// synchronously instead of dropping the handle through abort-only cleanup.
    #[farhelm_testtrace::test]
    async fn cancelling_provisional_sink_readiness_publishes_a_reaping_barrier() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let (shutdown, shutdown_rx) = tokio::sync::oneshot::channel();
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let task_entered = Arc::clone(&entered);
        let task_release = Arc::clone(&release);
        let handle = Arc::new(SessionSinkHandle {
            tmux_name: "fh-test".to_string(),
            task: Some(tokio::spawn(async move {
                let _ = shutdown_rx.await;
                task_entered.notify_one();
                task_release.notified().await;
                Ok(())
            })),
            shutdown: Some(shutdown),
            state: watch::channel(None).0,
        });
        sup.sinks.lock().expect("sink registry").insert(
            "fh-test".to_string(),
            SinkRegistryEntry::Live(Arc::downgrade(&handle)),
        );
        let committed = SessionSinkLease::new(handle, Arc::clone(&sup.sinks));
        let mut provisional = Box::pin(sup.ensure_session_sink("fh-test"));
        let waiting = std::future::poll_fn(|cx| {
            std::task::Poll::Ready(provisional.as_mut().poll(cx).is_pending())
        })
        .await;
        assert!(waiting, "the provisional attach must wait for readiness");

        drop(committed);
        assert!(matches!(
            sup.sinks.lock().expect("sink registry").get("fh-test"),
            Some(SinkRegistryEntry::Live(_))
        ));
        drop(provisional);
        assert!(matches!(
            sup.sinks.lock().expect("sink registry").get("fh-test"),
            Some(SinkRegistryEntry::Reaping(_))
        ));

        entered.notified().await;
        release.notify_one();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        loop {
            if sup
                .sinks
                .lock()
                .expect("sink registry")
                .get("fh-test")
                .is_none()
            {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "the provisional lease reaper did not clear its barrier"
            );
            tokio::task::yield_now().await;
        }
    }

    /// A same-session ensure cannot overtake a controlled reaper.
    ///
    /// This is the request boundary behind an immediate detach/reattach: the
    /// old sink's shutdown is deliberately held open, and the replacement
    /// ensure must remain pending until that reaper publishes an outcome.
    /// Returning while the gate is closed would let the wire attach reply
    /// overtake process exit even if cleanup finished moments afterward.
    #[farhelm_testtrace::test]
    async fn a_same_session_ensure_waits_for_the_reaper_outcome() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let (shutdown, shutdown_rx) = tokio::sync::oneshot::channel();
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let task_entered = Arc::clone(&entered);
        let task_release = Arc::clone(&release);
        let handle = Arc::new(SessionSinkHandle {
            tmux_name: "fh-test".to_string(),
            task: Some(tokio::spawn(async move {
                let _ = shutdown_rx.await;
                task_entered.notify_one();
                task_release.notified().await;
                anyhow::bail!("controlled reap outcome")
            })),
            shutdown: Some(shutdown),
            state: watch::channel(Some(1)).0,
        });
        sup.sinks.lock().expect("sink registry").insert(
            "fh-test".to_string(),
            SinkRegistryEntry::Live(Arc::downgrade(&handle)),
        );
        drop(SessionSinkLease::new(handle, Arc::clone(&sup.sinks)));
        entered.notified().await;

        let mut ensure = Box::pin(sup.ensure_session_sink("fh-test"));
        let remained_pending = std::future::poll_fn(|cx| {
            std::task::Poll::Ready(ensure.as_mut().poll(cx).is_pending())
        })
        .await;
        assert!(
            remained_pending,
            "same-session ensure returned while the old sink reaper was gated"
        );

        release.notify_one();
        let error = match ensure.await {
            Ok(_) => panic!("the controlled reap failure must fail the waiting ensure"),
            Err(error) => error,
        };
        assert!(
            format!("{error:#}").contains("controlled reap outcome"),
            "the waiting ensure must receive the reaper's outcome: {error:#}"
        );
    }

    /// An abandoned candidate blocks adoption of an otherwise-live winner.
    ///
    /// Candidate cleanup is tracked separately from the registered sink, so a
    /// successful ensure cannot return the winner while an abandoned control
    /// client for the same tmux session is still being reaped.
    #[farhelm_testtrace::test]
    async fn an_abandoned_candidate_blocks_ensure_until_its_reap_finishes() {
        let state = StateDir::new();
        let sup = Supervisor::new_with_exe(state.path(), dummy_exe())
            .await
            .expect("supervisor");
        let (winner_shutdown, winner_shutdown_rx) = tokio::sync::oneshot::channel();
        let winner = Arc::new(SessionSinkHandle {
            tmux_name: "fh-test".to_string(),
            task: Some(tokio::spawn(async move {
                let _ = winner_shutdown_rx.await;
                Ok(())
            })),
            shutdown: Some(winner_shutdown),
            state: watch::channel(Some(1)).0,
        });
        sup.sinks.lock().expect("sink registry").insert(
            "fh-test".to_string(),
            SinkRegistryEntry::Live(Arc::downgrade(&winner)),
        );
        let winner_owner = SessionSinkLease::new(winner, Arc::clone(&sup.sinks));

        let (candidate_shutdown, candidate_shutdown_rx) = tokio::sync::oneshot::channel();
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let task_entered = Arc::clone(&entered);
        let task_release = Arc::clone(&release);
        let candidate_handle = SessionSinkHandle {
            tmux_name: "fh-test".to_string(),
            task: Some(tokio::spawn(async move {
                let _ = candidate_shutdown_rx.await;
                task_entered.notify_one();
                task_release.notified().await;
                Ok(())
            })),
            shutdown: Some(candidate_shutdown),
            state: watch::channel(Some(2)).0,
        };
        let mut candidate =
            SessionSinkCandidate::begin("fh-test".to_string(), Arc::clone(&sup.sinks));
        candidate.set_handle(candidate_handle);
        drop(candidate);
        entered.notified().await;

        let mut ensure = Box::pin(sup.ensure_session_sink("fh-test"));
        let remained_pending = std::future::poll_fn(|cx| {
            std::task::Poll::Ready(ensure.as_mut().poll(cx).is_pending())
        })
        .await;
        assert!(
            remained_pending,
            "ensure returned the winner while an abandoned candidate was still reaping"
        );

        release.notify_one();
        let adopted = ensure
            .await
            .expect("ensure adopts the winner after candidate cleanup");
        assert_eq!(*adopted.state.borrow(), Some(1));
        drop(adopted);
        drop(winner_owner);
    }

    /// Reserving a missing sink publishes its barrier before opening can pause.
    ///
    /// This pins the lock boundary itself: the first ensure is stopped after
    /// its missing decision but before tmux is touched. A second ensure must
    /// already wait on that reservation instead of making another missing
    /// decision and exposing a competing client.
    #[farhelm_testtrace::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_missing_sink_reservation_is_visible_before_opening_begins() {
        let state = StateDir::new();
        let lookup_barrier = Arc::new(tokio::sync::Barrier::new(3));
        let lookup_calls = Arc::new(AtomicU64::new(0));
        let (candidate_observed, mut candidate_observed_rx) = mpsc::unbounded_channel();
        let reservation_entered = Arc::new(tokio::sync::Notify::new());
        let reservation_release = Arc::new(tokio::sync::Notify::new());
        let reservation_calls = Arc::new(AtomicU64::new(0));
        let gate_lookup_barrier = Arc::clone(&lookup_barrier);
        let gate_lookup_calls = Arc::clone(&lookup_calls);
        let gate_reservation_entered = Arc::clone(&reservation_entered);
        let gate_reservation_release = Arc::clone(&reservation_release);
        let gate_reservation_calls = Arc::clone(&reservation_calls);
        let sup = Supervisor::new_with_seams(
            state.path(),
            dummy_exe(),
            SupervisorTimeouts::default(),
            SupervisorSeams {
                sink_lookup_gate: Some(Arc::new(move || {
                    let barrier = Arc::clone(&gate_lookup_barrier);
                    let call = gate_lookup_calls.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    Box::pin(async move {
                        if call < 2 {
                            barrier.wait().await;
                        }
                    })
                })),
                sink_candidate_wait_gate: Some(Arc::new(move || {
                    let observed = candidate_observed.clone();
                    Box::pin(async move {
                        let _ = observed.send(());
                    })
                })),
                sink_reservation_gate: Some(Arc::new(move || {
                    let entered = Arc::clone(&gate_reservation_entered);
                    let release = Arc::clone(&gate_reservation_release);
                    let call =
                        gate_reservation_calls.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    Box::pin(async move {
                        if call == 0 {
                            entered.notify_one();
                            release.notified().await;
                        }
                    })
                })),
                ..SupervisorSeams::default()
            },
        )
        .await
        .expect("supervisor");

        let first_sup = Arc::clone(&sup);
        let first =
            tokio::spawn(
                async move { first_sup.ensure_session_sink("fh-missing").await.map(drop) },
            );
        let second_sup = Arc::clone(&sup);
        let second =
            tokio::spawn(
                async move { second_sup.ensure_session_sink("fh-missing").await.map(drop) },
            );
        lookup_barrier.wait().await;
        reservation_entered.notified().await;
        candidate_observed_rx
            .recv()
            .await
            .expect("the losing lookup observes the winner's candidate barrier");

        assert_eq!(
            reservation_calls.load(std::sync::atomic::Ordering::Relaxed),
            1,
            "only one simultaneous missing decision may reserve an opener"
        );
        assert_eq!(
            sup.sinks
                .lock()
                .expect("sink registry")
                .candidates
                .get("fh-missing")
                .map(Vec::len),
            Some(1),
            "the missing decision must publish its candidate barrier atomically"
        );
        assert!(
            !first.is_finished() && !second.is_finished(),
            "an ensure escaped while the sole reserved opener was gated"
        );

        reservation_release.notify_one();
        let (first_result, second_result) = tokio::join!(first, second);
        assert!(
            first_result.expect("first ensure task joins").is_err()
                && second_result.expect("second ensure task joins").is_err(),
            "the nonexistent fixture session must reject both opens"
        );
    }

    /// A pane-state map is a whole tmux SERVER's worth of panes, and this
    /// is what turns it back into ONE session's tab strip. Every filter
    /// here closes a way a window could be misreported as a tab, and the
    /// ORDER is the `SessionInfo::tabs` contract clients derive their
    /// positional labels from.
    ///
    /// The `@10`-before-`@9` case is the one a plain string sort gets
    /// wrong, and it would not show up until a user opened a tenth window
    /// — at which point the whole strip would silently relabel itself.
    #[farhelm_testtrace::test]
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

        let tabs = tabs_from_pane_states(states.values(), "fh-mine");
        assert_eq!(
            tabs.iter().map(|t| t.id.clone()).collect::<Vec<_>>(),
            vec![tab(9), tab(10)],
            "creation order comes from the window id's NUMERIC part, so @9 precedes @10"
        );
        assert_eq!(tabs[0].pane, "%1");
        assert_eq!(tabs[1].pane, "%2");
        assert!(
            tabs_from_pane_states(states.values(), "fh-nobody").is_empty(),
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
    #[farhelm_testtrace::test]
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
        let tabs = tabs_from_pane_states(states.values(), "fh-mine");
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
    #[farhelm_testtrace::test]
    fn the_sink_backoff_grows_to_a_cap_and_stays_there() {
        assert_eq!(sink_retry_delay(0), SINK_RETRY_BASE);
        assert_eq!(sink_retry_delay(1), SINK_RETRY_BASE);
        assert_eq!(sink_retry_delay(2), SINK_RETRY_BASE * 2);
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

    /// A client whose output closes while its process stays alive.
    ///
    /// This is the exceptional EOF shape the supervisor must kill before
    /// opening a replacement; ordinary fake clients exit and cannot
    /// distinguish an awaited reap from a drop-only teardown.
    fn output_closed_fake_sink() -> SessionSink {
        let child = tokio::process::Command::new("sh")
            .args(["-c", "exec 1>&-; exec sleep 30"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawning a fake sink with closed output");
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
    #[farhelm_testtrace::test(start_paused = true)]
    async fn the_sink_supervisor_keeps_retrying_indefinitely() {
        let attempts = Arc::new(AtomicU64::new(0));
        let (state, _rx) = watch::channel(Some(1));
        let (_shutdown, shutdown_rx) = oneshot::channel();
        let counter = Arc::clone(&attempts);
        let task = tokio::spawn(run_session_sink(
            "fh-test".to_string(),
            dying_fake_sink(),
            state.clone(),
            shutdown_rx,
            None,
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
    #[farhelm_testtrace::test(start_paused = true)]
    async fn dropping_the_last_sink_handle_stops_its_supervisor() {
        let (state, _rx) = watch::channel(Some(1));
        let (shutdown, shutdown_rx) = oneshot::channel();
        let handle = Arc::new(SessionSinkHandle {
            tmux_name: "fh-test".to_string(),
            task: Some(tokio::spawn(run_session_sink(
                "fh-test".to_string(),
                living_fake_sink(),
                state,
                shutdown_rx,
                None,
                |_| async { anyhow::bail!("never called") },
            ))),
            shutdown: Some(shutdown),
            state: watch::channel(None).0,
        });
        let second = Arc::clone(&handle);
        drop(handle);
        tokio::task::yield_now().await;
        assert!(
            !second
                .task
                .as_ref()
                .expect("the task is still owned")
                .is_finished(),
            "a handle still held by another owner must not stop the sink"
        );
        // The task handle is inside the Arc, so its state has to be
        // sampled through a clone taken before the last drop.
        let task = second
            .task
            .as_ref()
            .expect("the task is still owned")
            .abort_handle();
        drop(second);
        tokio::task::yield_now().await;
        assert!(
            task.is_finished(),
            "dropping the last handle must stop the supervising task"
        );
    }

    /// Orderly last-owner shutdown does not return while the sink process
    /// can still overlap its replacement.
    ///
    /// Abort-on-drop is sufficient for leak prevention but does not wait
    /// for process death. A browser reload sends detach and attach back to
    /// back on one connection, so this stronger boundary is what prevents
    /// the new control client from racing the old one's tmux teardown.
    #[farhelm_testtrace::test]
    async fn orderly_sink_shutdown_reaps_the_client_before_returning() {
        let sink = living_fake_sink();
        let pid = sink.pid().expect("the fake sink has a process id");
        let (state, _rx) = watch::channel(Some(pid));
        let (shutdown, shutdown_rx) = oneshot::channel();
        let handle = SessionSinkHandle {
            tmux_name: "fh-test".to_string(),
            task: Some(tokio::spawn(run_session_sink(
                "fh-test".to_string(),
                sink,
                state.clone(),
                shutdown_rx,
                None,
                |_| async { anyhow::bail!("never called") },
            ))),
            shutdown: Some(shutdown),
            state,
        };

        handle
            .shutdown()
            .await
            .expect("orderly sink shutdown succeeds");

        assert!(
            !std::path::Path::new(&format!("/proc/{pid}")).exists(),
            "orderly shutdown returned while the sink process still existed"
        );
    }

    /// An EOF from a still-live client is reaped before replacement begins.
    ///
    /// A control client whose output pipe closes without process exit would
    /// otherwise remain attached and unread while its replacement starts,
    /// reproducing the overlap this lifecycle is designed to exclude.
    #[farhelm_testtrace::test]
    async fn a_live_sink_after_eof_is_reaped_before_replacement_opens() {
        let sink = output_closed_fake_sink();
        let pid = sink.pid().expect("the fake sink has a process id");
        let process = Arc::new(format!("/proc/{pid}"));
        let (state, _rx) = watch::channel(Some(pid));
        let (_shutdown, shutdown_rx) = oneshot::channel();
        let (observed, mut observed_rx) = tokio::sync::mpsc::unbounded_channel();
        let process_for_task = Arc::clone(&process);
        let task = tokio::spawn(run_session_sink(
            "fh-test".to_string(),
            sink,
            state,
            shutdown_rx,
            None,
            move |_| {
                let observed = observed.clone();
                let process = Arc::clone(&process_for_task);
                async move {
                    let gone = !std::path::Path::new(process.as_str()).exists();
                    let _ = observed.send(gone);
                    anyhow::bail!("stop after observing the replacement boundary")
                }
            },
        ));

        let gone = observed_rx
            .recv()
            .await
            .expect("the replacement opener reports the process boundary");
        assert!(
            gone,
            "replacement opening began before the EOF client was reaped"
        );
        task.abort();
        let _ = task.await;
    }

    /// Owner shutdown interrupts retry backoff without opening a replacement.
    ///
    /// Last-attachment teardown commonly lands while a dead sink is waiting
    /// out its retry delay. Letting the delay win would keep the handoff
    /// barrier occupied and could create a client after the session no longer
    /// has an owner, so shutdown must end that wait immediately.
    #[farhelm_testtrace::test(start_paused = true)]
    async fn shutdown_during_retry_backoff_does_not_open_a_replacement() {
        let attempts = Arc::new(AtomicU64::new(0));
        let (state, _rx) = watch::channel(Some(1));
        let (shutdown, shutdown_rx) = oneshot::channel();
        let (retrying, mut retrying_rx) = watch::channel(0u64);
        let counter = Arc::clone(&attempts);
        let task = tokio::spawn(run_session_sink(
            "fh-test".to_string(),
            dying_fake_sink(),
            state,
            shutdown_rx,
            Some(retrying),
            move |_| {
                let counter = Arc::clone(&counter);
                async move {
                    counter.fetch_add(1, Ordering::Relaxed);
                    anyhow::bail!("the replacement must not open")
                }
            },
        ));

        retrying_rx
            .changed()
            .await
            .expect("the sink supervisor announces its retry timer");
        shutdown
            .send(())
            .expect("the sink supervisor still receives shutdown");
        task.await
            .expect("the sink supervisor task joins")
            .expect("shutdown during backoff succeeds");

        assert_eq!(
            attempts.load(Ordering::Relaxed),
            0,
            "shutdown during backoff must not open a replacement client"
        );
    }

    /// Shutdown waits out an in-flight open and reaps its returned client.
    ///
    /// Cancelling a tmux attach exchange can detach the future before the
    /// spawned process has exited. The exchange is bounded, so orderly
    /// teardown finishes it and then proves the resulting client is gone
    /// before releasing the same-session handoff barrier.
    #[farhelm_testtrace::test(start_paused = true)]
    async fn shutdown_during_replacement_open_reaps_the_returned_client() {
        let (state, _rx) = watch::channel(Some(1));
        let (shutdown, shutdown_rx) = oneshot::channel();
        let (retrying, mut retrying_rx) = watch::channel(0u64);
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let returned_pid = Arc::new(AtomicU64::new(0));
        let opener_dropped = Arc::new(AtomicBool::new(false));
        let opener_entered = Arc::clone(&entered);
        let opener_release = Arc::clone(&release);
        let opener_pid = Arc::clone(&returned_pid);
        let opener_drop_flag = Arc::clone(&opener_dropped);
        let future = run_session_sink(
            "fh-test".to_string(),
            dying_fake_sink(),
            state,
            shutdown_rx,
            Some(retrying),
            move |_| {
                let entered = Arc::clone(&opener_entered);
                let release = Arc::clone(&opener_release);
                let returned_pid = Arc::clone(&opener_pid);
                let opener_dropped = Arc::clone(&opener_drop_flag);
                async move {
                    let _drop_flag = DropFlag(opener_dropped);
                    entered.notify_one();
                    release.notified().await;
                    let sink = living_fake_sink();
                    returned_pid.store(u64::from(sink.pid().unwrap_or(0)), Ordering::Relaxed);
                    Ok(sink)
                }
            },
        );
        tokio::pin!(future);

        tokio::select! {
            result = future.as_mut() => panic!("sink supervisor exited before replacement: {result:?}"),
            changed = retrying_rx.changed() => {
                changed.expect("the sink supervisor announces its retry timer");
            }
        }
        tokio::time::advance(SINK_RETRY_MAX).await;
        tokio::select! {
            result = future.as_mut() => panic!("sink supervisor exited before opening: {result:?}"),
            _ = entered.notified() => {}
        }
        shutdown
            .send(())
            .expect("the sink supervisor still receives shutdown");
        let remained_pending = std::future::poll_fn(|cx| {
            std::task::Poll::Ready(future.as_mut().poll(cx).is_pending())
        })
        .await;
        assert!(
            remained_pending,
            "shutdown must not cancel an in-flight replacement open"
        );
        assert!(
            !opener_dropped.load(Ordering::Relaxed),
            "shutdown dropped the in-flight replacement opener"
        );

        release.notify_one();
        future
            .await
            .expect("shutdown after the replacement opens succeeds");
        let pid = returned_pid.load(Ordering::Relaxed);
        assert_ne!(pid, 0, "the opener must return a real client");
        assert!(
            !std::path::Path::new(&format!("/proc/{pid}")).exists(),
            "shutdown returned while the replacement process still existed"
        );
    }

    /// Shutdown also ends an in-flight replacement that eventually fails.
    ///
    /// The error branch is separate from the returned-client branch above. It
    /// must observe the pending owner shutdown and exit without scheduling a
    /// second open, or a session with no owners can retain its handoff barrier
    /// and retry forever.
    #[farhelm_testtrace::test(start_paused = true)]
    async fn shutdown_during_failed_replacement_open_does_not_retry() {
        let (state, _rx) = watch::channel(Some(1));
        let (shutdown, shutdown_rx) = oneshot::channel();
        let (retrying, mut retrying_rx) = watch::channel(0u64);
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let attempts = Arc::new(AtomicU64::new(0));
        let opener_entered = Arc::clone(&entered);
        let opener_release = Arc::clone(&release);
        let opener_attempts = Arc::clone(&attempts);
        let future = run_session_sink(
            "fh-test".to_string(),
            dying_fake_sink(),
            state,
            shutdown_rx,
            Some(retrying),
            move |_| {
                let entered = Arc::clone(&opener_entered);
                let release = Arc::clone(&opener_release);
                let attempts = Arc::clone(&opener_attempts);
                async move {
                    attempts.fetch_add(1, Ordering::Relaxed);
                    entered.notify_one();
                    release.notified().await;
                    anyhow::bail!("injected replacement failure")
                }
            },
        );
        tokio::pin!(future);

        tokio::select! {
            result = future.as_mut() => panic!("sink supervisor exited before retrying: {result:?}"),
            changed = retrying_rx.changed() => {
                changed.expect("the sink supervisor announces its retry timer");
            }
        }
        tokio::time::advance(SINK_RETRY_MAX).await;
        tokio::select! {
            result = future.as_mut() => panic!("sink supervisor exited before opening: {result:?}"),
            _ = entered.notified() => {}
        }
        shutdown
            .send(())
            .expect("the sink supervisor still receives shutdown");

        release.notify_one();
        future
            .await
            .expect("a failed replacement observes shutdown as success");
        assert_eq!(
            attempts.load(Ordering::Relaxed),
            1,
            "shutdown after a failed open must not start another replacement"
        );
    }

    /// The last lease publishes its handoff barrier synchronously, while
    /// the runtime-owned reaper completes independently afterward.
    ///
    /// This is the cancellation-safe core of detach/reattach ordering: a
    /// caller may disappear as soon as it drops the lease, but a same-session
    /// attach must already see `Reaping` before it can attempt a replacement.
    #[farhelm_testtrace::test]
    async fn the_last_sink_lease_publishes_reaping_before_shutdown_finishes() {
        let registry: SinkRegistry = Arc::new(StdMutex::new(Default::default()));
        let (shutdown, shutdown_rx) = oneshot::channel();
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let task_entered = Arc::clone(&entered);
        let task_release = Arc::clone(&release);
        let handle = Arc::new(SessionSinkHandle {
            tmux_name: "fh-test".to_string(),
            task: Some(tokio::spawn(async move {
                let _ = shutdown_rx.await;
                task_entered.notify_one();
                task_release.notified().await;
                Ok(())
            })),
            shutdown: Some(shutdown),
            state: watch::channel(Some(1)).0,
        });
        registry.lock().expect("sink registry").insert(
            "fh-test".to_string(),
            SinkRegistryEntry::Live(Arc::downgrade(&handle)),
        );
        let first = SessionSinkLease::new(Arc::clone(&handle), Arc::clone(&registry));
        let last = SessionSinkLease::new(handle, Arc::clone(&registry));

        drop(first);
        assert!(matches!(
            registry.lock().expect("sink registry").get("fh-test"),
            Some(SinkRegistryEntry::Live(_))
        ));
        drop(last);
        assert!(matches!(
            registry.lock().expect("sink registry").get("fh-test"),
            Some(SinkRegistryEntry::Reaping(_))
        ));

        entered.notified().await;
        assert!(matches!(
            registry.lock().expect("sink registry").get("fh-test"),
            Some(SinkRegistryEntry::Reaping(_))
        ));
        release.notify_one();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        loop {
            if registry
                .lock()
                .expect("sink registry")
                .get("fh-test")
                .is_none()
            {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "a successful reaper did not clear its handoff barrier"
            );
            tokio::task::yield_now().await;
        }
    }

    /// An old successful reaper cannot erase a newer registry barrier.
    ///
    /// Reaper completion is asynchronous, so its original slot may have been
    /// replaced by stronger fail-closed evidence before it finishes. Clearing
    /// by key alone would lose that evidence and let a new sink overlap the
    /// client whose newer cleanup remains unconfirmed.
    #[farhelm_testtrace::test]
    async fn a_stale_successful_reaper_does_not_clear_a_newer_barrier() {
        let registry: SinkRegistry = Arc::new(StdMutex::new(Default::default()));
        let (shutdown, shutdown_rx) = oneshot::channel();
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let task_entered = Arc::clone(&entered);
        let task_release = Arc::clone(&release);
        let handle = Arc::new(SessionSinkHandle {
            tmux_name: "fh-test".to_string(),
            task: Some(tokio::spawn(async move {
                let _ = shutdown_rx.await;
                task_entered.notify_one();
                task_release.notified().await;
                Ok(())
            })),
            shutdown: Some(shutdown),
            state: watch::channel(Some(1)).0,
        });
        registry.lock().expect("sink registry").insert(
            "fh-test".to_string(),
            SinkRegistryEntry::Live(Arc::downgrade(&handle)),
        );

        drop(SessionSinkLease::new(handle, Arc::clone(&registry)));
        entered.notified().await;
        let mut old_done = {
            let state = registry.lock().expect("sink registry");
            match state.get("fh-test") {
                Some(SinkRegistryEntry::Reaping(done)) => done.clone(),
                _ => panic!("the old reaper did not publish its barrier"),
            }
        };
        registry.lock().expect("sink registry").insert(
            "fh-test".to_string(),
            SinkRegistryEntry::Failed(Arc::<str>::from("newer cleanup is unconfirmed")),
        );
        release.notify_one();
        while old_done.borrow().is_none() {
            old_done
                .changed()
                .await
                .expect("the old reaper publishes its outcome");
        }

        let state = registry.lock().expect("sink registry");
        assert!(matches!(
            state.get("fh-test"),
            Some(SinkRegistryEntry::Failed(error)) if error.as_ref() == "newer cleanup is unconfirmed"
        ));
    }

    /// A stale lease failure poisons the replacement registry slot.
    ///
    /// The client belongs to an older incarnation, but an unconfirmed tmux
    /// process is unsafe for every newer incarnation of the same session. A
    /// replacement `Live` entry must therefore become `Failed` rather than
    /// silently discarding the stale lease's shutdown error.
    #[farhelm_testtrace::test]
    async fn a_stale_lease_failure_poisons_a_newer_live_slot() {
        let registry: SinkRegistry = Arc::new(StdMutex::new(Default::default()));
        let (shutdown, shutdown_rx) = oneshot::channel();
        let stale = Arc::new(SessionSinkHandle {
            tmux_name: "fh-test".to_string(),
            task: Some(tokio::spawn(async move {
                let _ = shutdown_rx.await;
                anyhow::bail!("the stale sink could not be reaped")
            })),
            shutdown: Some(shutdown),
            state: watch::channel(Some(1)).0,
        });
        let stale = SessionSinkLease::new(stale, Arc::clone(&registry));
        let replacement = Arc::new(SessionSinkHandle {
            tmux_name: "fh-test".to_string(),
            task: None,
            shutdown: None,
            state: watch::channel(Some(2)).0,
        });
        registry.lock().expect("sink registry").insert(
            "fh-test".to_string(),
            SinkRegistryEntry::Live(Arc::downgrade(&replacement)),
        );

        drop(stale);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        loop {
            let failure = {
                let state = registry.lock().expect("sink registry");
                match state.get("fh-test") {
                    Some(SinkRegistryEntry::Failed(error)) => Some(Arc::clone(error)),
                    _ => None,
                }
            };
            if let Some(error) = failure {
                assert!(
                    error.contains("the stale sink could not be reaped"),
                    "the stale failure must replace the newer live slot: {error}"
                );
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "the stale lease failure did not poison the newer slot"
            );
            tokio::task::yield_now().await;
        }
        drop(replacement);
    }

    /// A failed reap remains visible instead of permitting another client.
    ///
    /// Once process exit cannot be confirmed, reopening the same session
    /// could overlap the lost client. Retaining the failure in the registry
    /// makes subsequent attaches fail closed until the supervisor restarts.
    #[farhelm_testtrace::test]
    async fn a_failed_sink_reap_remains_a_fail_closed_registry_entry() {
        let registry: SinkRegistry = Arc::new(StdMutex::new(Default::default()));
        let (shutdown, shutdown_rx) = oneshot::channel();
        let handle = Arc::new(SessionSinkHandle {
            tmux_name: "fh-test".to_string(),
            task: Some(tokio::spawn(async move {
                let _ = shutdown_rx.await;
                anyhow::bail!("the sink process could not be reaped")
            })),
            shutdown: Some(shutdown),
            state: watch::channel(Some(1)).0,
        });
        registry.lock().expect("sink registry").insert(
            "fh-test".to_string(),
            SinkRegistryEntry::Live(Arc::downgrade(&handle)),
        );

        drop(SessionSinkLease::new(handle, Arc::clone(&registry)));
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        loop {
            let failed = {
                let registry = registry.lock().expect("sink registry");
                match registry.get("fh-test") {
                    Some(SinkRegistryEntry::Failed(error)) => Some(Arc::clone(error)),
                    _ => None,
                }
            };
            if let Some(error) = failed {
                assert!(
                    error.contains("the sink process could not be reaped"),
                    "the registry must preserve the reap failure: {error}"
                );
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "the failed reaper did not publish its fail-closed state"
            );
            tokio::task::yield_now().await;
        }
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
    #[farhelm_testtrace::test(start_paused = true)]
    async fn a_healthy_sink_run_resets_the_respawn_backoff() {
        let attempts = Arc::new(AtomicU64::new(0));
        // Published by the opener so the test can kill the one client it
        // hands out, which is how the "healthy run" is ended on cue.
        let live_pid = Arc::new(AtomicU64::new(0));
        let (state, _rx) = watch::channel(Some(1));
        let (_shutdown, shutdown_rx) = oneshot::channel();
        let (retrying, mut retrying_rx) = watch::channel(0u64);
        let counter = Arc::clone(&attempts);
        let pid_slot = Arc::clone(&live_pid);
        let task = tokio::spawn(run_session_sink(
            "fh-test".to_string(),
            dying_fake_sink(),
            state.clone(),
            shutdown_rx,
            Some(retrying),
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
        retrying_rx.borrow_and_update();
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
        // The supervisor now reaps the dead client before scheduling its
        // replacement. Wait on that real-process boundary without advancing
        // virtual time; otherwise Tokio can spend the measurement window
        // while the task is still awaiting the reap, and this test would be
        // timing process cleanup rather than the backoff it names.
        let process = format!("/proc/{pid}");
        let reap_deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while std::path::Path::new(&process).exists() {
            assert!(
                std::time::Instant::now() < reap_deadline,
                "the dead fake sink was not reaped before its replacement"
            );
            tokio::task::yield_now().await;
        }
        retrying_rx
            .changed()
            .await
            .expect("the sink supervisor announces its retry timer");
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

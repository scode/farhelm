//! The session list: `ListView` (the flat, polled listing plus its
//! stop/delete/create/rename actions), `SessionRow` (one row, including
//! the inline delete-confirmation prompt and the inline rename field), and
//! `CreateSessionForm` (the "new session" inline form). All three are
//! `ListView`'s own concern — none of them is meaningful mounted outside
//! it — so only `ListView` itself is `pub(crate)`; `SessionRow` and
//! `CreateSessionForm` stay private to this module. The rename FIELD is
//! the one exception: `rename::RenameForm` is shared with the session
//! view, since SPEC.md puts the same operation on both surfaces.
//!
//! ## The list is multi-host (PLAN_M6.md item 6)
//!
//! Every row names the host it lives on, and a row whose host is not
//! connected is marked stale rather than hidden — SPEC.md: sessions on an
//! unreachable host "stay in the list from the helm's last-known
//! knowledge, clearly marked". Their lifecycle controls stay live too, and
//! deliberately: the helm refuses such an operation with the host's state
//! in the message, which is a far more useful answer than a disabled button
//! that explains nothing.
//!
//! This view also owns the hosts READ (`hosts::HostsPanel` renders it),
//! because two consumers need one poll: the panel, and the create dialog's
//! host selector.
//!
//! ## The list is the WHOLE list
//!
//! `api::fetch_sessions` follows the helm's cursor to exhaustion and this
//! view renders what comes back in the helm's own order, unsorted. Multi-host
//! aggregation is what makes lists long enough for one page to be a lie —
//! showing 500 of a fleet's sessions while `total` reports the real count is
//! a partial list wearing a complete one's clothes — and re-sorting the
//! pages here would scramble them into an order no cursor agrees with.

use std::collections::{HashMap, HashSet};

use dioxus::prelude::*;

use crate::api::{
    POLL_INTERVAL_MS, SessionListing, create_session, delete_session, fetch_hosts, fetch_sessions,
    mint_intent_key, rename_session, stop_session,
};
use crate::hosts::{HostsPanel, HostsRead, host_incarnation, is_connected, phase_label};
use crate::ops::{OpLock, ReadGate, use_op_lock};
use crate::peer::{DetailPart, PeerLine, display_peer};
use crate::rename::RenameForm;
use crate::rows::{apply_optimistic_renames, count_banner, prune_optimistic_renames};
use crate::status::{confirm_consequence, status_badge};
use crate::{ApiBase, HostId, HostKind, Session, SessionStatus};

/// The subset of `Session` `on_delete` actually needs, in `ListView` below:
/// the id the API call targets, plus `status` to decide whether this click
/// deletes immediately or opens the inline confirm prompt. No `title` —
/// unlike before this file's eval-based `window.confirm()` was replaced
/// with `SessionRow`'s in-page one, `on_delete` itself never builds any
/// confirm wording anymore; `confirm_message` (near `SessionRow`) computes
/// that straight from the row's own live `session.title`/`session.status`
/// on every render instead, so a title never needs to travel through this
/// type at all. Deliberately narrower than the whole `Session` — `on_delete`
/// has no legitimate reason to depend on `cwd` or `invocation` either, and a
/// dedicated type is what makes that impossible rather than merely
/// unlikely, unlike `on_open` (which keeps taking the whole `Session`: it
/// needs every field to populate `SessionView`).
#[derive(Debug, Clone)]
struct DeleteTarget {
    id: String,
    status: SessionStatus,
}

/// One entry in the create dialog's host selector: everything that decides
/// how a host is offered, and nothing more.
///
/// Narrower than `Host` on purpose — the dialog has no business re-deriving
/// anything from a `HostPhase`, which is the duplication `hosts`'s helpers
/// exist to prevent. `ListView` reduces each host to these facts once.
#[derive(Debug, Clone, PartialEq, Eq)]
struct HostOption {
    id: HostId,
    /// The helm's own display name, so the selector and the session rows
    /// call a host the same thing.
    name: String,
    /// Whether this is the helm's own machine — the reserved local row,
    /// which is SPEC.md's fallback default.
    local: bool,
    /// The phase to show beside the name, for a host that is NOT connected;
    /// `None` for one that is.
    ///
    /// Every host is offered as a create target regardless of phase (see
    /// `ListView`), so the label is what keeps that from being a trap: a
    /// user picking a host the helm will refuse can see why before they
    /// submit, and the refusal that follows repeats the same word.
    phase: Option<String>,
    /// This host's current incarnation (`hosts::host_incarnation`) — what an
    /// idempotency key is bound to, so that an id pointed at a different
    /// machine since the key was minted is a different intent.
    incarnation: String,
}

impl HostOption {
    /// What the `<option>` reads: the host's name, with its phase appended
    /// when there is one to warn about.
    ///
    /// The name is [`display_peer`]'d for the reason every other rendering
    /// of a destination is — an option label is exactly the place where a
    /// directional override could make one host's row read as another's, and
    /// the choice made there is which machine a command runs on.
    fn label(&self) -> String {
        let name = display_peer(&self.name);
        match &self.phase {
            Some(phase) => format!("{name} ({phase})"),
            None => name,
        }
    }
}

/// How many times one submit will mint a key before giving up.
///
/// The retry exists for a queued keystroke landing during the mint, which
/// resolves on the second attempt; anything beyond that is a form whose
/// values keep changing faster than a UUID can be generated, which is not a
/// create anybody is waiting on. Bounded rather than a bare loop because
/// spinning is a worse answer than saying so.
const MINT_ATTEMPTS: usize = 3;

/// Everything one intended create IS — the exact thing an idempotency key
/// stands for.
///
/// The helm treats a key as "this create, retried"; this type is what makes
/// that claim true on the client's side. Two parts, and both were learned
/// the hard way:
///
/// - **The host, as an INCARNATION rather than an id.** A `HostId` is a
///   registry row, and the row outlives every edit made to it: retargeting
///   points it at another address, adopting binds it to another install.
///   Keyed on the id alone, a retry after an ambiguous failure carries the
///   first attempt's key to a machine that has never seen it — where it is
///   not idempotent at all, and the "retry" is a second real agent. See
///   `hosts::host_incarnation`.
/// - **The form's values, snapshotted.** They already start a new intent
///   when edited (each field's `oninput` is what clears the key), but that
///   rule has a gap the size of one await: minting is asynchronous and the
///   inputs are only disabled once a rerender lands, so a keystroke queued
///   at submit time can change a field while the key is being made. The
///   binding is re-read after minting and compared against this, which
///   turns that gap into another mint rather than a key that describes
///   something the user did not submit.
#[derive(Debug, Clone, PartialEq, Eq)]
struct IntentBinding {
    host: HostId,
    /// The target host's incarnation at submit time — see the type docs.
    incarnation: String,
    cwd: String,
    invocation: String,
    title: String,
}

impl IntentBinding {
    /// The binding for a submit, or `None` when there is no host to create
    /// on — the one case a submit is refused locally rather than sent.
    fn of(
        selected: Option<HostId>,
        hosts: &[HostOption],
        cwd: String,
        invocation: String,
        title: String,
    ) -> Option<IntentBinding> {
        let host = hosts.iter().find(|host| Some(host.id) == selected)?;
        Some(IntentBinding {
            host: host.id,
            incarnation: host.incarnation.clone(),
            cwd,
            invocation,
            title,
        })
    }
}

/// Wait one poll interval.
///
/// Split into a helper because two loops and one early-continue all need it
/// and the body is a per-target `cfg` pair rather than a call:
/// `tokio::time::sleep` is unavailable on wasm32 (no reactor in the browser)
/// while `gloo-timers`' `TimeoutFuture` only works on wasm32 (a
/// `wasm-bindgen` binding to `setTimeout`), so each target gets the idiom
/// that already fits it. The desktop build runs inside the tokio
/// multi-thread runtime `dioxus-desktop` itself constructs (see its
/// `launch.rs`), so `tokio::time::sleep` needs no extra setup there.
async fn sleep_one_interval() {
    #[cfg(target_arch = "wasm32")]
    gloo_timers::future::TimeoutFuture::new(POLL_INTERVAL_MS as u32).await;
    #[cfg(not(target_arch = "wasm32"))]
    tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)).await;
}

/// Which host a fresh create dialog should target: the LOCAL row, whatever
/// phase it is in.
///
/// ## Why SPEC.md's first clause is not implemented here
///
/// SPEC.md's default is "the host of the currently open session, else the
/// helm's own host". In this UI those are mutually exclusive views — `App`
/// renders EITHER the list or a session, never both — so while this dialog
/// exists there is no currently open session, and the first clause selects
/// nothing every single time. It was plumbed through anyway for a while:
/// `App` derived the open session's host, passed it down, and this function
/// compared it against the option list. Every step of that was unreachable
/// code with a live-looking test around it, which is worse than an absent
/// feature — it reads as coverage for a rule nothing exercises.
///
/// The clause becomes reachable the moment a create surface can coexist with
/// an open session (a split view, a modal over the terminal, a command
/// palette — some later milestone's UI shape). At that point the parameter
/// comes back, and the rule to restore is the one SPEC.md states: the host
/// of the session open RIGHT NOW, never a remembered last-viewed one,
/// because a session the user backed out of is not open.
///
/// ## Two things the fallback deliberately does not do
///
/// - It does not skip a local row that is not connected. SPEC.md names the
///   helm's own host unconditionally; a create against it is a precondition
///   failure the helm explains in its own words, and a default that quietly
///   moved to another machine because the local supervisor was down would be
///   a create on a host the user never chose.
/// - It does not fall back to "the first host in the list" when there is no
///   local row. The first host is whichever the registry happens to order
///   first, and creating on it is a guess. `None` — a selector with nothing
///   chosen, which the form refuses to submit — is the honest answer, and it
///   only arises before the first hosts read lands, since a live helm always
///   has its local row.
fn default_create_host(hosts: &[HostOption]) -> Option<HostId> {
    hosts.iter().find(|host| host.local).map(|host| host.id)
}

/// The flat session list: host, title, cwd, invocation, and a truthful
/// status badge per row, refetched on a timer; the hosts panel, the "new
/// session" form and the per-row stop/delete actions (PLAN_M2.md step 8)
/// live here too, since all of them need to reach into the same poll loops
/// — a create or a stop should be reflected as soon as the next poll runs,
/// not held behind an optimistic local edit.
///
/// The poll loops live in `use_future`s scoped to this component, so they
/// are cancelled for free when `App` switches to `SessionView` and this
/// component unmounts — "polling stops while a terminal is open"
/// (PLAN_M2.md) falls out of Dioxus's own task lifecycle rather than
/// needing an explicit stop signal.
///
/// Two polls, one cadence. The hosts read is its own loop rather than a
/// second leg of the listing's, so a slow or failing `/api/hosts` cannot
/// delay the session list (or vice versa) — and it is deliberately not new
/// push machinery: M6.75 owns that, and until then the hosts panel gets the
/// same three-second freshness every other surface has.
///
/// ## One operation at a time
///
/// The create, the five host mutations and the add-host form are mutually
/// exclusive, and the exclusion is `ops::OpLock` — a token each handler
/// claims synchronously at entry — rather than a set of render-time
/// booleans. See that module for why the booleans could not work: they are
/// values captured at the last render, so two clicks inside one frame both
/// see an idle page.
///
/// The session-OPEN click is gated by the same token even though it starts
/// nothing, because it ENDS everything: opening a session unmounts this
/// component and every task it owns, so a mid-flight mutation's result is
/// simply lost. Per-session stop/rename/delete stay outside the token, on
/// their own per-row set — they cannot invalidate one another's premises,
/// and two rows acting at once is behavior the browser suite pins.
#[component]
pub(crate) fn ListView(on_open: EventHandler<Session>) -> Element {
    let base = use_context::<ApiBase>().0;
    // The page's single live-operation token (see this component's docs).
    let ops = use_op_lock();
    let mut listing = use_signal(|| None::<Result<SessionListing, String>>);
    // The same generation discipline the hosts read has, for the same
    // reason and against a slower race: a listing walk is several round
    // trips, so a poll that started before a delete can easily still be
    // walking when the delete's own refresh has already landed — and
    // committing it would put the deleted row back until the next tick.
    let mut listing_reads = use_signal(ReadGate::default);
    // The host registry as this client currently knows it — four states, not
    // three, so a failed poll cannot blank the panel (see `hosts::HostsRead`).
    // Shared by the hosts panel and the create dialog's selector.
    let mut hosts = use_signal(HostsRead::default);
    // Per-REQUEST, not per-poll: the periodic loop and every
    // mutation-triggered refetch draw from the same gate, so an older
    // completion cannot resurrect what a newer one removed — see
    // `ops::ReadGate` for why successes and failures are gated differently.
    let mut hosts_reads = use_signal(ReadGate::default);
    // Which host row is BUSY, for drawing only: the exclusion is the token.
    // Lifted out of `HostsPanel` because navigating away unmounts the tasks
    // running its mutations, so this component has to see them.
    let busy_host = use_signal(|| None::<HostId>);
    // Per-session, not one shared slot: a stop failing on session A must
    // not blank out session B's still-fresh success (or vice versa), and
    // a LATER success on any session must not silently erase an EARLIER
    // failure on a different one — which a single `Option<String>` would
    // do on every write regardless of which session it was about. Keyed
    // by session id so each row renders only its own entry.
    let mut errors = use_signal(HashMap::<String, String>::new);
    // Which sessions have a stop/delete in flight right now (also keyed
    // by id): both disables that row's buttons (so a second click can't
    // race the first) and is the re-entry guard the click handlers check
    // before doing anything — belt-and-suspenders, since a disabled
    // button should already stop the click from firing, but the DOM
    // update disabling it is not synchronous with the click handler
    // itself.
    let mut pending = use_signal(HashSet::<String>::new);
    // Which sessions are showing the inline "confirm delete?" prompt in
    // place of their normal stop/delete buttons — see `on_delete` below.
    // Deliberately a plain client-side set with no timeout and no
    // poll-driven reset: a listing refresh must leave an in-progress
    // confirmation alone (the user is mid-decision, not mid-poll), so
    // this is intentionally NOT derived from `listing` on every render.
    // The one reconciliation that does happen is in the poll loop below,
    // which drops an entry once its session is no longer in the listing
    // at all (deleted from elsewhere, say) — there is no row left for a
    // dangling entry to ever affect, so this is tidiness, not correctness.
    let mut confirming = use_signal(HashSet::<String>::new);
    // Which row, if any, has its rename field open (PLAN_M5.md item 6),
    // and the text being typed into it.
    //
    // One at a time, unlike `confirming`'s set, and that is the whole
    // interaction rather than a limitation: renaming is a focused edit the
    // user finishes or abandons, and a second open field would be an
    // invitation to type into two and lose track of which one Enter
    // submits. The draft lives HERE rather than in `RenameForm` for a
    // reason that has nothing to do with how many can be open: this
    // component re-renders for reasons the user did not cause, and one of
    // them (a failed listing poll swapping the rows for an error line)
    // unmounts the form entirely — a draft owned by the form would be
    // silently discarded with it. Seeded from the row's current title when
    // the field opens, which is also what keeps a poll carrying someone
    // else's rename from overwriting an edit in progress.
    let mut renaming = use_signal(|| None::<String>);
    let mut rename_draft = use_signal(String::new);
    // The optimistic rename corrections `apply_optimistic_renames` paints
    // over the server's listing, keyed by session id and carrying the poll
    // sequence number that bounds when the server could first have told
    // this view about it (`prune_optimistic_renames`). The tab strip's
    // scheme, applied to a title: without the number, a listing reply that
    // was already in flight when the rename landed would be
    // indistinguishable from the server disagreeing, and the row would flip
    // back to the old title for up to a full poll interval — a visible
    // wobble on the one operation whose entire point is that the new name
    // shows up at once.
    let mut renamed = use_signal(HashMap::<String, (String, u64)>::new);
    // How many listing polls this view has STARTED. A poll's own index is
    // the value it reads before incrementing, so an optimistic rename
    // recording the current value names the first poll GUARANTEED to have
    // started after the rename's response completed. That is a
    // conservative bound rather than a statement about the server: a poll
    // launched earlier can perfectly well observe the committed title,
    // since the write lands before the response is read. Conservative is
    // the safe direction — it can only keep a correction slightly longer
    // than strictly necessary, never retire one on a reply that could not
    // have seen it.
    let mut poll_sequence = use_signal(|| 0_u64);
    let mut show_create = use_signal(|| false);

    // Everything that happens to a listing reply once it is BACK, in one
    // place: decide whether this read still speaks for the view, reconcile
    // the view-local state a fresh listing settles, and paint it. A reply
    // the gate rejects leaves every one of those untouched.
    //
    // Hoisted rather than inlined because there are two readers of the
    // session listing — the poll below and `on_stop`'s immediate refetch —
    // and a second hand-rolled copy of the gate decision is exactly the kind
    // of divergence that shows up as a stale row nobody can reproduce.
    //
    // What it deliberately does NOT do is claim the generation. That claim
    // has to happen synchronously at the point the request is ISSUED (see
    // `ops::ReadGate::start`), so that the order reads are gated in is the
    // order they were asked for rather than the order their tasks happened
    // to be polled. Taking an already-claimed `generation` keeps that
    // property with the caller, where the `await` is.
    //
    // `poll_index` is `Some` only for a poll tick, carrying that tick's own
    // position in the view's poll order. The reconciliation below is gated
    // on it in full: `on_stop`'s refetch exists to show ONE session's new
    // status at once and has no standing in the poll order, so it can
    // neither date an optimistic rename nor be read as evidence that some
    // other session has left the listing.
    let mut commit_listing = move |generation: u64,
                                   fetched: Result<SessionListing, String>,
                                   poll_index: Option<u64>| {
        // Superseded reads are dropped before they can touch
        // anything — including the optimistic-correction pruning
        // below, which would otherwise retire a rename on the
        // authority of a walk that predates it.
        let accepted = match &fetched {
            Ok(_) => listing_reads.write().accept_success(generation),
            Err(_) => listing_reads.peek().accept_failure(generation),
        };
        if !accepted {
            return;
        }
        // Drop any `confirming` entry whose session is gone from
        // this fetch entirely — the counterpart to the "a poll
        // refresh must not clear an in-progress confirmation"
        // rule just above: that rule protects a row that is
        // still LISTED, not one that has vanished (deleted from
        // another client while this one sat mid-confirmation, an
        // externally-imposed departure the `retain` below cannot
        // distinguish from the id simply never having existed).
        // Left off a failed fetch on purpose: an error reply
        // carries no session ids at all, and a transient fetch
        // failure is not evidence any session actually left.
        if let (Ok(listing), Some(index)) = (&fetched, poll_index) {
            let live_ids: HashSet<&str> = listing.sessions.iter().map(|s| s.id.as_str()).collect();
            confirming
                .write()
                .retain(|id| live_ids.contains(id.as_str()));
            // An open rename field for a session that has left the
            // listing entirely goes with it, the same tidiness the
            // `confirming` retain above performs — there is no row
            // left for it to sit in.
            let renaming_vanished = renaming
                .read()
                .as_ref()
                .is_some_and(|id| !live_ids.contains(id.as_str()));
            if renaming_vanished {
                renaming.set(None);
            }
            // Same "only a successful fetch is evidence" rule as
            // the two above, for the same reason: an error carries
            // no titles at all, so it can neither confirm nor
            // contradict an optimistic rename.
            prune_optimistic_renames(&mut renamed.write(), &listing.sessions, index);
        }
        listing.set(Some(fetched));
    };

    // Cloned once up front rather than moved into the poll loop below: a
    // `move ||` closure takes ownership of everything it captures, and
    // `on_stop`/`on_delete` need their own copy of `base` afterward.
    let poll_base = base.clone();
    use_future(move || {
        let base = poll_base.clone();
        async move {
            loop {
                // Read before incrementing, so `index` is this poll's own
                // position in the view's poll order — what tells an
                // optimistic rename whether this reply is late enough to
                // be evidence about it.
                let index = poll_sequence.peek().to_owned();
                poll_sequence += 1;
                let generation = listing_reads.write().start();
                let fetched = fetch_sessions(&base).await;
                commit_listing(generation, fetched, Some(index));
                sleep_one_interval().await;
            }
        }
    });

    // One hosts read, generation-guarded, shared by every caller — the
    // periodic poll and every mutation-triggered refetch. Going through one
    // place is what makes the generation a total order over READS rather
    // than a per-loop counter two callers could each satisfy independently.
    //
    // The number is claimed synchronously, at the CALL, so ordering is
    // decided by when a read was asked for rather than by when its task
    // happens to be scheduled.
    let read_hosts_base = base.clone();
    let start_hosts_read = move || {
        let base = read_hosts_base.clone();
        let generation = hosts_reads.write().start();
        async move {
            let outcome = fetch_hosts(&base).await;
            // Successes and failures are gated differently — see
            // `ops::ReadGate`. An older success is dropped entirely (it
            // describes a registry that has since been changed by something
            // this client did, so committing it would resurrect exactly what
            // a removal removed), while a failure newer than what is on
            // screen is reported even though a later read has already
            // started, because otherwise a helm that is down looks merely
            // quiet.
            let accepted = match &outcome {
                Ok(_) => hosts_reads.write().accept_success(generation),
                Err(_) => hosts_reads.peek().accept_failure(generation),
            };
            if accepted {
                hosts.write().record(outcome);
            }
        }
    };

    // The hosts poll, at the listing's cadence and independent of it (see
    // this component's docs). A failed read keeps the last snapshot and adds
    // a refresh-failure line rather than blanking the rows: SPEC.md's
    // promise is that connection state is VISIBLE, and one dropped request
    // is not evidence that anything changed.
    let poll_hosts = start_hosts_read.clone();
    use_future(move || {
        let mut read = poll_hosts.clone();
        async move {
            loop {
                read().await;
                sleep_one_interval().await;
            }
        }
    });

    // An immediate re-read after a host mutation, instead of waiting out the
    // poll. Every host verb changes state this side cannot predict — an
    // add's chip is whatever the connection finds, a retarget's is a fresh
    // active-retry window, an adopt's is a reconnect — so there is nothing
    // honest to paint optimistically, and the fastest truthful answer is the
    // server's.
    let mut refresh_read = start_hosts_read;
    let refresh_hosts = move |_| {
        spawn(refresh_read());
    };

    let stop_base = base.clone();
    // Takes the id directly, not the whole `Session`: nothing past the
    // insert-into-`pending` check below reads any other field, so a
    // `Session` clone (and a second, redundant id clone off of it) would
    // only be dead weight — see `SessionRow`'s call site for the mirrored
    // simplification on the caller's side.
    let on_stop = move |id: String| {
        // Cross-guard against `confirming`, not just `pending`: the
        // stop/delete buttons are only ABSENT from the DOM once a
        // rerender following `confirming.insert` has actually landed
        // (see `SessionRow`'s doc), so a stop click queued just ahead of
        // that rerender — a rapid synthetic double-click, say — could
        // otherwise still reach this handler for a row that is, or is
        // about to be, showing the confirm prompt. Refusing here keeps
        // the row's two lifecycle handlers from ever racing each other
        // for the same id: without it, a stop could slip `id` into
        // `pending` WHILE a delete confirmation is open, and the eventual
        // "confirm delete" click would then silently no-op — NOT because
        // of anything in `confirm_delete` itself, but because `do_delete`
        // (which it calls after removing `confirming`) has its OWN
        // `pending`-insert re-entry guard, which would find the id
        // already occupied by that stop and bail with no error at all.
        //
        // The same argument covers an open RENAME field, which replaces
        // the same buttons for the same reason.
        if confirming.read().contains(&id) || renaming.read().as_deref() == Some(id.as_str()) {
            return;
        }
        // Re-entry guard for the per-session in-flight set: a disabled
        // button should already stop this, but the click and the
        // re-render that disables it are not synchronous, so the handler
        // checks for itself too. `insert` returning `false` means an op
        // for this id was already running.
        if !pending.write().insert(id.clone()) {
            return;
        }
        let base = stop_base.clone();
        spawn(async move {
            // No optimistic flip (PLAN_M2.md design note): the row's
            // badge only ever reflects what the NEXT poll observes, so a
            // stop that silently failed can never leave the UI claiming a
            // session is exited when tmux still disagrees.
            let outcome = stop_session(&base, &id).await;
            match outcome.err() {
                Some(e) => {
                    errors.write().insert(id.clone(), format!("stop: {e}"));
                    pending.write().remove(&id);
                }
                None => {
                    errors.write().remove(&id);
                    // `pending` stays set across this extra fetch — not
                    // released until it completes: `on_delete`'s confirm
                    // wording is decided from the `status` the LATEST
                    // listing carries, and without this, an instant
                    // delete right after this stop would still see the
                    // stale pre-stop live status (up to `POLL_INTERVAL_MS`)
                    // and confirm with the wrong "is still running"
                    // wording for a session that just got stopped.
                    //
                    // Through the SAME gate the poll uses, which is the
                    // whole reason the gate is per-request rather than
                    // per-loop: this read exists to show the stop at once,
                    // and a poll that started before the stop — a walk is
                    // several round trips, so one easily spans it —
                    // completing afterwards would put the pre-stop status
                    // back and undo exactly what this call is for.
                    let generation = listing_reads.write().start();
                    let fetched = fetch_sessions(&base).await;
                    // `None`: this read is not a poll tick, so it settles
                    // nothing about optimistic renames or about which
                    // sessions still exist — see `commit_listing`.
                    commit_listing(generation, fetched, None);
                    pending.write().remove(&id);
                }
            }
        });
    };

    let delete_base = base.clone();
    // The actual DELETE call, shared by both ways a delete can be
    // decided on: immediately for an Exited session, or after the user
    // hits "confirm delete" on the inline prompt for a live or `Unknown`
    // one (see `on_delete` and `confirm_delete` below, both of which
    // clone this closure rather than each reimplementing the request/
    // pending/error bookkeeping). Mirrors `on_stop`'s shape exactly,
    // `delete_session` and `errors`'/`pending`'s "delete:"-prefixed entry
    // in place of `on_stop`'s "stop:" one.
    let mut do_delete = move |id: String| {
        if !pending.write().insert(id.clone()) {
            return;
        }
        let base = delete_base.clone();
        spawn(async move {
            let outcome = delete_session(&base, &id).await;
            match outcome.err() {
                Some(e) => {
                    errors.write().insert(id.clone(), format!("delete: {e}"));
                    pending.write().remove(&id);
                }
                None => {
                    errors.write().remove(&id);
                    // Removed from the LOCAL listing immediately, before
                    // releasing `pending` — the one deliberate optimistic
                    // exception in this file (PLAN_M2.md's "no optimistic
                    // status flips" is about STATUS badges specifically;
                    // this is acting on a delete response already in
                    // hand, not guessing). Waiting for the next poll
                    // instead would leave the stale row's delete button
                    // re-enabled with nothing left server-side to delete
                    // — a second click in that window would 404 against
                    // an id that no longer exists, a confusing failure
                    // for an action that had already succeeded.
                    if let Some(Ok(current)) = listing.write().as_mut() {
                        current.sessions.retain(|s| s.id != id);
                    }
                    pending.write().remove(&id);
                }
            }
        });
    };

    // The delete button's initial click: decides whether this id needs
    // confirming at all, never itself calling the API.
    //
    // SPEC.md's "Lifecycle operations": delete confirms first only when
    // the agent might still be alive — an Exited session deletes
    // immediately (see its own arm below for the residual risk that
    // accepts). The live statuses and Unknown all confirm, entering the per-session
    // `confirming` state that `SessionRow` reads to swap its action area
    // (see that component's doc) — this closure itself does nothing more
    // than flip that flag; `confirm_delete` below is what a confirmed
    // click actually acts on.
    //
    // Refuses an id already in `pending` (the cross-guard mirroring
    // `on_stop`'s `confirming` check above): the delete button is only
    // disabled once a rerender following a `pending` insert lands, so a
    // rapid click queued just ahead of that rerender could otherwise
    // still reach this handler while, say, a stop is already in flight
    // for the same session. Refusing here is what keeps `confirming` from
    // ever being entered in that window — closing the door on the
    // opposite race `on_stop`'s guard closes. `do_delete`'s OWN
    // `pending`-insert guard (not `confirm_delete`'s `confirming`-removal
    // check, which exists for a different race entirely — see its own
    // doc below) would eventually refuse this same id too, but only AFTER
    // a confirm prompt had already opened for nothing; catching it here
    // means the prompt never opens in the first place.
    let mut do_delete_on_confirm = do_delete.clone();
    let on_delete = move |target: DeleteTarget| {
        if pending.read().contains(&target.id)
            || renaming.read().as_deref() == Some(target.id.as_str())
        {
            return;
        }
        // The split is exactly `has_ended()` vs. everything else, which is
        // to say: a status the UI knows to be finished deletes straight
        // away, and the rest — the three live statuses and `Unknown` —
        // go through a confirmation. Asking the status rather than
        // listing the variants is what kept this correct THROUGH M6.75's
        // liveness split, which added two live variants and needed no edit
        // here (see `SessionStatus::has_ended`).
        if target.status.has_ended() {
            // Deliberately unconfirmed, a known residual: the AGENT
            // process has exited, but process-tree descendants it
            // spawned (a stray MCP server, a dev server) can outlive it,
            // and delete's process-tree sweep will kill whatever it
            // still finds. The UI has no way to know whether any such
            // descendant exists — only the supervisor's sweep does,
            // after the fact — so there is nothing concrete to report
            // here, and always confirming "just in case" would make
            // deleting routine, already-finished sessions needlessly
            // noisy. M6.75's status work sharpened what a LIVE session is
            // doing, not what an ended one left behind, so this residual
            // stands unchanged.
            // `Interrupted` joins `Exited` here for a stronger version of
            // the same argument: a host reboot is what produced this
            // status, and a reboot leaves no descendants at all — there
            // is not even the stray-MCP-server residual to accept. The
            // session's agent is definitively not running, so confirming
            // would be asking about a danger that cannot exist.
            // `Error` joins them for the strongest version yet: the login
            // shell and the launch shim DID run briefly (the shim is what
            // WRITES this very sentinel, from inside a real process), but
            // the AGENT'S OWN exec is what failed (PLAN_M3.md item 3) —
            // before it, before anything the agent itself might have
            // spawned. There is no lingering process tree to worry about,
            // not because nothing ever ran, but because the one thing
            // that could have left descendants never got the chance to.
            do_delete_on_confirm(target.id);
        } else {
            // Unknown must not borrow a live status's "is still running" claim
            // it has no basis for — SPEC.md's no-guessing rule means an
            // unresolved status is presented as exactly that, uncertain,
            // never rounded up to a known-alive claim just because both
            // wordings end up confirming the same way. The DIFFERENT
            // wording itself lives in `SessionRow`, computed from
            // whatever `status` the row's own next render carries — not
            // captured here, since a status that changes while a
            // confirmation sits open (a session stopped from another
            // client, say) should be reflected in the prompt too.
            confirming.write().insert(target.id);
        }
    };

    // The confirm-delete button's click, inside the inline prompt: the
    // exact same DELETE call an accepted `window.confirm()` used to
    // trigger before this rewrite, just reached from a different UI
    // widget. Clears `confirming` first so the row falls back to its
    // normal (busy/disabled) button layout the instant `do_delete`'s own
    // `pending` insert takes effect, rather than momentarily showing
    // both the prompt and a busy state.
    //
    // Proceeds ONLY when `remove` reports the id was actually present:
    // `HashSet::remove` returns `false` for an id already gone, which
    // happens whenever this confirmation was already resolved by
    // something else — `cancel_delete` running first (a queued confirm
    // click landing just after a cancel click, both fired in the same
    // burst), or a second confirm click racing the first's own removal.
    // Without this check, that second call would fall through to
    // `do_delete` regardless, which for the cancel-then-confirm race
    // would delete a session the user just told the UI to leave alone.
    let confirm_delete = move |id: String| {
        if !confirming.write().remove(&id) {
            return;
        }
        do_delete(id);
    };

    // The inline prompt's cancel button: just drops the flag. No API
    // call, no `pending` involvement — cancelling was never in flight to
    // begin with.
    let cancel_delete = move |id: String| {
        confirming.write().remove(&id);
    };

    // The rename button's click: opens this row's field, seeds the draft
    // from the title the row is showing right now, and never calls the API
    // — exactly as `on_delete` opens the confirm prompt. Refuses a row
    // with an operation already in flight or a confirmation already open,
    // the same cross-guard those two keep against each other and for the
    // same reason (the controls only disappear once a rerender lands, so a
    // click queued just ahead of one can still arrive here).
    //
    // Seeding HERE rather than in the form is what makes reopening start
    // from the current title while an edit already in progress is never
    // overwritten by a poll (see `renaming`/`rename_draft`).
    let on_rename_start = move |(id, title): (String, String)| {
        if pending.read().contains(&id) || confirming.read().contains(&id) {
            return;
        }
        rename_draft.set(title);
        renaming.set(Some(id));
    };

    // The rename field's submit. The title goes to the supervisor exactly
    // as typed (`api::rename_session`); everything decided here is what to
    // do with its answer.
    //
    // On success the reply — the session as the supervisor now describes
    // it, status re-probed and tabs rediscovered — is recorded as this
    // view's optimistic correction, so the new title paints without
    // waiting for a poll, and the field closes. On failure the field stays
    // open with what the user typed still in it (the same courtesy
    // `CreateSessionForm` extends to a failed create — a refused title is
    // usually one keystroke away from an accepted one) and the
    // supervisor's own words land in this row's error line, while the old
    // title stays everywhere it was.
    let rename_base = base.clone();
    let on_rename_submit = move |(id, title): (String, String)| {
        if !pending.write().insert(id.clone()) {
            return;
        }
        // This row's own previous failure, cleared by the retry that
        // supersedes it and by nothing else (see `errors`).
        errors.write().remove(&id);
        let base = rename_base.clone();
        spawn(async move {
            match rename_session(&base, &id, &title).await {
                Ok(session) => {
                    // The sequence number is read AFTER the reply, never
                    // before the request: it names the first poll
                    // GUARANTEED to have started after this response
                    // completed. A poll launched while the POST was still
                    // in flight MAY also observe the new title — the write
                    // lands before the reply is read — so this is a
                    // conservative bound, and conservative in the only
                    // safe direction (it can keep a correction a little
                    // longer, never retire it on a reply that could not
                    // have seen the rename).
                    let observed_from = poll_sequence.peek().to_owned();
                    renamed
                        .write()
                        .insert(id.clone(), (session.title.clone(), observed_from));
                    // Closed only if this row's field is still the open
                    // one. The form disables its own cancel while a
                    // request is in flight, so the user has to beat a
                    // rerender to get here — but if they do (cancel, then
                    // open another row's field), a blind `set(None)` would
                    // close a field they are typing in and throw the draft
                    // away.
                    if renaming.peek().as_deref() == Some(id.as_str()) {
                        renaming.set(None);
                    }
                }
                Err(e) => {
                    errors.write().insert(id.clone(), format!("rename: {e}"));
                }
            }
            pending.write().remove(&id);
        });
    };

    // Opening a row navigates `App` away from `ListView` entirely, which
    // unmounts this component and every task it owns: a create or a host
    // mutation still in flight has its eventual result silently discarded
    // instead of ever being acted on. So the open click consults the page
    // token AND this view's own per-session set, and it does so INSIDE the
    // handler — the `nav_locked` value below is what the button renders
    // with, and a render-time value is exactly what a click landing in the
    // same frame as the operation it should have seen would read as idle.
    let guarded_open = move |session: Session| {
        if ops.busy_now() || !pending.peek().is_empty() {
            return;
        }
        on_open.call(session);
    };
    // Cosmetic reflection of the same conditions, for the disabled
    // attributes. Not the guard — see `ops`.
    let busy = ops.busy();
    let nav_locked = busy || !pending.read().is_empty();
    // EVERY host is offered as a create target, whatever phase it is in.
    // Filtering to connected hosts — which this used to do — quietly
    // rewrites SPEC.md's default: the local row is the fallback
    // unconditionally, and a filter that removed it whenever the local
    // supervisor was down would silently move the default to another
    // machine. A create against a non-connected host is a precondition
    // failure the helm explains in its own words, which is a better answer
    // than an option the user cannot even select to find out.
    let host_options: Vec<HostOption> = hosts
        .read()
        .hosts()
        .unwrap_or_default()
        .iter()
        .map(|host| HostOption {
            id: host.id,
            name: host.name.clone(),
            local: host.kind == HostKind::Local,
            // Non-connected hosts are labelled with their phase, so choosing
            // one is an informed choice rather than a surprise refusal.
            phase: (!is_connected(&host.state)).then(|| phase_label(&host.state).to_string()),
            incarnation: host_incarnation(host),
        })
        .collect();

    rsx! {
        HostsPanel { hosts, ops, busy_host, on_changed: refresh_hosts }
        div { class: "list-toolbar",
            button {
                r#type: "button",
                class: "btn new-session-button",
                // This control UNMOUNTS the create form, so it must not act
                // while anything is in flight: dropping the component drops
                // its `spawn`ed task's ability to ever act on the response,
                // silently losing track of whether the create happened.
                disabled: busy,
                onclick: move |_| {
                    // The token, read synchronously here rather than through
                    // the attribute above: a rerender's DOM update is not
                    // synchronous with a click, so a second click landing in
                    // that gap still reaches this handler.
                    if ops.busy_now() {
                        return;
                    }
                    show_create.set(!show_create());
                },
                "new session"
            }
        }
        if show_create() {
            CreateSessionForm {
                hosts: host_options,
                hosts_loaded: hosts.read().hosts().is_some(),
                ops,
                on_created: move |session| {
                    show_create.set(false);
                    on_open.call(session);
                },
            }
        }
        match &*listing.read() {
            None => rsx! { div { class: "status", "loading sessions…" } },
            Some(Err(e)) => rsx! {
                div { class: "status error", "failed to load sessions: {e}" }
            },
            Some(Ok(listing)) => rsx! {
                if listing.sessions.is_empty() && listing.total == 0 {
                    div { class: "status", "no sessions" }
                } else {
                    // The count ALWAYS renders, and which of the two
                    // wordings it carries is `rows::count_banner`'s
                    // decision — see there for why an absent banner would
                    // itself be a claim nobody can read.
                    {
                        let banner = count_banner(listing);
                        rsx! {
                            div { class: "{banner.class}",
                                "{banner.text}"
                                if let Some(note) = banner.incoherence {
                                    "{note}"
                                }
                            }
                        }
                    }
                    div { class: "session-list",
                        // The rows are the server's listing with this
                        // view's own just-landed renames painted over it,
                        // so a renamed session reads correctly EVERYWHERE
                        // the row shows its title — the row itself, the
                        // delete prompt that quotes it, the rename field
                        // if it is reopened, and the `Session` that
                        // `on_open` carries into the session view.
                        for session in apply_optimistic_renames(&listing.sessions, &renamed.read()) {
                            SessionRow {
                                key: "{session.id}",
                                error: errors.read().get(&session.id).cloned(),
                                busy: pending.read().contains(&session.id),
                                confirming: confirming.read().contains(&session.id),
                                renaming: renaming.read().as_deref() == Some(session.id.as_str()),
                                nav_disabled: nav_locked,
                                session,
                                on_open: guarded_open,
                                on_stop: on_stop.clone(),
                                on_delete: on_delete.clone(),
                                on_confirm_delete: confirm_delete.clone(),
                                on_cancel_delete: cancel_delete,
                                rename_draft,
                                on_rename_start,
                                on_rename_submit: on_rename_submit.clone(),
                                // Inlined rather than a named closure: it
                                // is one assignment, and the only rename
                                // state a cancel touches is which row is
                                // open — the draft is deliberately LEFT
                                // alone, since the next open reseeds it.
                                on_rename_cancel: move |_| renaming.set(None),
                            }
                        }
                    }
                }
            },
        }
    }
}

/// Inline create form (PLAN_M2.md step 8's "not a modal library" design
/// choice): working directory and agent command are required, title is
/// optional. Lives entirely inside `ListView` — there is no route or
/// signal for it beyond the `show_create` toggle that mounts/unmounts it.
///
/// `submitting` is owned by the CALLER (`ListView`), not this component:
/// `ListView`'s own "new session" toggle button needs to see it too, so it
/// can refuse to unmount this form while a create is still in flight —
/// dropping this component mid-`spawn` would strand the POST's eventual
/// response with nothing left to act on it. Lifting the flag up is
/// simpler than trying to keep a detached task meaningful after the fact.
///
/// `on_created` fires only on a successful POST, with the newly created
/// `Session` from the response body; `ListView` uses that to both close
/// the form and navigate straight into the new session's terminal
/// (SPEC.md: "creation launches the agent; you type your first prompt
/// into its terminal"). On failure the form stays mounted with its values
/// untouched and the error text rendered next to it — the fields are
/// plain `use_signal<String>`s rather than being reset or lifted into
/// `ListView`, so "form contents preserved" falls out of simply not
/// clearing them rather than needing a restore step. On success the
/// fields are left as-is too: `on_created` drives `ListView` to unmount
/// this whole component immediately (closing the form and navigating
/// away), so there is no one left to observe a reset — only the failure
/// path needs to leave the control usable again.
///
/// ## The intent key (PLAN_M3.md item 6), and what it is bound to
///
/// One key per INTENDED create, reused across every retry of it. The
/// lifecycle is deliberately tied to the form's values rather than to its
/// mount: minted at first submit, kept across a failed submit (the retry
/// case the key exists for), and dropped the moment any field changes
/// (which makes the next submit a different intent). Both edges matter —
/// keeping it across an edit would send a request the server refuses as a
/// key reuse once the first attempt has a durable outcome, and dropping it
/// on failure would make a retry able to create a second session for the
/// same intent, which is the exact gap this closes.
///
/// The key is bound to its TARGET HOST as well as to the fields, and that
/// binding closes a hole the field-only version left open. An intent is "run
/// this command in this directory ON THIS MACHINE" — the same key against a
/// different host is a different intended create, and the helm scopes keys
/// per host, so replaying one at a second host is not idempotent there at
/// all. The dangerous shape is not the user changing the selector (that
/// clears the key inline, like any other edit) but the target changing
/// UNDER them: a host removed from the registry moves the effective default,
/// and a retry after an ambiguous failure would then carry the first
/// attempt's key to a machine that has never seen it — a second real agent,
/// which is precisely the outcome the key exists to prevent. So the key is
/// stored WITH the host it was minted for and re-minted whenever the
/// effective target no longer matches.
///
/// The inputs are DISABLED while a create is in flight, which is what makes
/// that lifecycle a rule rather than a race: key generation is itself
/// asynchronous (`mint_intent_key` is an `await` on both renderers, even
/// though only the wasm build's half of it actually yields), so without it
/// a keystroke could land between minting a key and sending it, publishing
/// a key that belongs to values the user has already changed. Disabling was
/// chosen over reconciling generations afterwards because the form is inert
/// for that window anyway — the submit button and both navigation controls
/// are already disabled by the same flag.
///
/// A create from this form ALWAYS carries a key. If the key cannot be
/// generated the create is refused locally, with the failure shown like any
/// other: falling back to an unkeyed create would silently drop the one
/// protection this whole feature exists to provide, at exactly the moment
/// something is already wrong with the environment, and a user who retries
/// after a dropped reply would get a duplicate agent with nothing to
/// indicate why.
///
/// The server's key-reuse and already-deleted refusals need no handling of
/// their own here: they arrive as ordinary create failures (a 409 with the
/// supervisor's own message) and render in the same `.create-session-error`
/// line as every other one, which is what SPEC.md's "concrete, actionable
/// errors" asks for — the message names the key and what happened to it.
///
/// ## The host selector (PLAN_M6.md item 6)
///
/// `hosts` carries EVERY registered host, whatever phase it is in (see
/// `ListView` for why filtering to connected ones would quietly rewrite
/// SPEC.md's default), with non-connected ones labelled by their phase. The
/// initial selection follows SPEC.md's default through
/// `default_create_host`. The selection is unset-until-touched rather than
/// seeded into a signal at mount: the poll underneath can change which hosts
/// exist, and a seeded value would pin the dialog to a host that has since
/// been removed with nothing to say about it.
///
/// When a chosen host DOES disappear, the reconciliation is visible rather
/// than silent: the selector moves to the default, a line says so, and the
/// intent key is re-minted for the new target. The failure that rules out is
/// the quiet one — a selector still displaying host A while the body carries
/// host B.
///
/// A submit before the first hosts read lands is refused locally with a
/// reason, rather than sent without a host. The helm would happily default a
/// hostless create to its local row, which is usually right and is not
/// something this form may decide by omission: the user is looking at a
/// selector that has not filled in yet.
///
/// A refused create — an unreachable host, a nonexistent directory —
/// surfaces the helm's words in the same error line and leaves the form
/// exactly as filled, host selection included.
#[component]
fn CreateSessionForm(
    hosts: Vec<HostOption>,
    /// Whether the hosts read has EVER succeeded. Distinguishes "there are
    /// no hosts" (impossible for a live helm, which always has its local
    /// row) from "nothing has come back yet", which is what a submit has to
    /// be refused for.
    hosts_loaded: bool,
    /// The page's live-operation token. Claimed at submit, released when the
    /// request completes — the exclusion against every host mutation, and
    /// against a second submit of this form (see `ops`).
    mut ops: OpLock,
    on_created: EventHandler<Session>,
) -> Element {
    let base = use_context::<ApiBase>().0;
    let mut cwd = use_signal(String::new);
    let mut invocation = use_signal(String::new);
    let mut title = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    // The user's explicit choice, if they have made one. `None` means "no
    // choice yet", not "no host" — the effective target is
    // `default_create_host`'s answer, recomputed per render against the
    // hosts that exist right now.
    let mut chosen_host = use_signal(|| None::<HostId>);
    // Whether an explicit choice has been overtaken by reality. Derived per
    // render rather than written back into `chosen_host`, so it cannot
    // outlive the condition that produced it — and so a host that comes back
    // (a re-added destination) silently reinstates the user's choice.
    let choice_vanished =
        chosen_host().is_some_and(|chosen| !hosts.iter().any(|host| host.id == chosen));
    let selected = chosen_host()
        .filter(|_| !choice_vanished)
        .or_else(|| default_create_host(&hosts));
    // This form's current intended create, if one has been submitted yet
    // (PLAN_M3.md item 6), together with the BINDING it was minted for.
    // Minted at first submit, reused by every later submit of the same
    // intent, and superseded the moment any part of that binding changes.
    let mut intent_key = use_signal(|| None::<(String, IntentBinding)>);
    let busy = ops.busy();

    rsx! {
        form {
            class: "create-session-form",
            onsubmit: move |evt| {
                evt.prevent_default();
                // The claim is the guard, and it is synchronous: it covers a
                // second submit of THIS form (a double-click, a stray repeat
                // event) and every host mutation at once, with no render in
                // between for a stale boolean to be read from.
                //
                // The OTHER half of double-submission — a retry after an
                // ambiguous transport failure (request sent, response lost)
                // reaching the supervisor a second time — is what
                // `intent_key` closes, and it cannot be closed here: only
                // the server knows whether the lost reply belonged to a
                // session that actually exists. This handler's job is merely
                // to send the SAME key for every retry of one intent.
                if !ops.claim() {
                    return;
                }
                // No host, no create. The helm would default a hostless body
                // to its local row — usually the right answer, and not one
                // this form may reach by omission while its own selector is
                // still blank. Saying so beats creating on a machine the
                // user was never shown.
                let Some(binding) = IntentBinding::of(selected, &hosts, cwd(), invocation(), title())
                else {
                    error.set(Some(
                        if hosts_loaded {
                            "this helm reported no hosts at all, so there is nothing to create on"
                                .to_string()
                        } else {
                            "the host list has not loaded yet, so this create was not sent — it \
                             would have gone to whichever host the helm picked rather than one \
                             you chose"
                                .to_string()
                        },
                    ));
                    ops.release();
                    return;
                };
                let base = base.clone();
                error.set(None);
                spawn(async move {
                    // Mint until the key and the binding agree.
                    //
                    // Minting is an `await` (the wasm renderer asks the
                    // browser for a UUID), and this form's inputs are only
                    // DISABLED once a rerender lands — so a keystroke
                    // already queued when submit fired can still change a
                    // field while the key is being made. Publishing that key
                    // would bind it to values the user has since edited,
                    // which is the same wrong-intent failure a changed host
                    // causes, arriving through a narrower window. Re-reading
                    // the binding after the await and minting again on a
                    // mismatch is what closes it.
                    //
                    // Bounded rather than a `loop`: a form whose values keep
                    // changing every time a key is minted is not a create
                    // anybody is waiting on, and spinning would be worse
                    // than saying so.
                    let mut binding = binding;
                    let mut attempts = 0;
                    let (key, bound) = loop {
                        let held = intent_key.peek().clone();
                        if let Some((key, held_binding)) = held
                            && held_binding == binding
                        {
                            break (key, held_binding);
                        }
                        if attempts >= MINT_ATTEMPTS {
                            error.set(Some(
                                "the form kept changing while an idempotency key was being \
                                 generated, so this create was not sent; try again"
                                    .to_string(),
                            ));
                            ops.release();
                            return;
                        }
                        attempts += 1;
                        match mint_intent_key().await {
                            Ok(key) => intent_key.set(Some((key, binding.clone()))),
                            Err(reason) => {
                                // No key, no create: see this component's
                                // docs on why an unkeyed create is not an
                                // acceptable degradation. The message says
                                // what failed rather than blaming the
                                // request, since nothing the user typed
                                // caused it.
                                error.set(Some(format!(
                                    "could not generate an idempotency key for this create, so \
                                     it was not sent (a retry could otherwise create a second \
                                     session): {reason}"
                                )));
                                ops.release();
                                return;
                            }
                        }
                        // What the form says NOW. Identical on the ordinary
                        // path; different exactly when a queued edit landed
                        // during the mint.
                        binding = IntentBinding {
                            cwd: cwd.peek().clone(),
                            invocation: invocation.peek().clone(),
                            title: title.peek().clone(),
                            ..binding
                        };
                    };
                    // Key, fields and host all travel from ONE value, so
                    // there is no arrangement of edits or polls in which the
                    // body describes a different intent than the key claims.
                    match create_session(
                            &base,
                            &bound.cwd,
                            &bound.invocation,
                            &bound.title,
                            &key,
                            Some(bound.host),
                        )
                        .await
                    {
                        // The reply is the bare `SessionInfo` — the helm
                        // deliberately puts no host fields on it, since the
                        // caller already knows which host it asked for. This
                        // is that caller: filling the target in here is what
                        // lets the session view name the right host before
                        // the first detail poll answers.
                        Ok(session) => {
                            // Released BEFORE navigating: `on_created`
                            // unmounts this component, and a token released
                            // afterwards would be released by a task nobody
                            // is left to run.
                            ops.release();
                            on_created.call(Session {
                                host: session.host.or(Some(bound.host)),
                                ..session
                            });
                        }
                        Err(e) => {
                            // The key deliberately SURVIVES a failure:
                            // this is exactly the case it exists for. A
                            // failure whose cause was an ambiguous
                            // transport error may have created a session
                            // the user cannot see, and resubmitting
                            // unchanged must reach that same session
                            // rather than launch a second agent. A user
                            // who instead fixes the form gets a new key,
                            // because the binding no longer matches.
                            error.set(Some(e));
                            ops.release();
                        }
                    }
                });
            },
            // Working directory and agent command are literal text that
            // gets EXECUTED, never prose — OS-level text mangling has no
            // way to tell the difference and "corrects" them anyway
            // (observed directly: WKWebView's autocorrect silently
            // substituting "claude" with "Claude" in place, with no
            // visible suggestion popup to catch and reject). A
            // capitalized command or a suggestion-popup keystroke
            // swallowed mid-path corrupts what actually runs. Title IS
            // ordinary prose, but the same opt-out applies to it too, for
            // a narrower reason: whatever the user types is what should
            // come back out verbatim (SPEC.md's "auto-generated when
            // omitted" is the only substitution this field ever gets, and
            // it happens server-side, deliberately, not as a silent
            // client-side "helpful" rewrite) — so every input here opts
            // out of every form of text mangling a browser might apply on
            // its own, for whichever of these two reasons applies to it.
            // First, because it decides what everything below it means: a
            // working directory and an agent command are only meaningful
            // relative to the machine they will run on.
            label {
                "host"
                select {
                    class: "create-session-host",
                    // Disabled for the whole round trip, exactly like the
                    // text fields: the key is bound to the target, and a
                    // selection changing between minting and sending would
                    // publish a key that belongs to a different machine.
                    disabled: busy,
                    // Empty only before the first hosts read lands — a live
                    // helm always has its local row. The submit handler
                    // refuses in that window rather than sending a hostless
                    // create.
                    value: selected.map(|id| id.to_string()).unwrap_or_default(),
                    onchange: move |evt| {
                        chosen_host.set(evt.value().parse::<HostId>().ok());
                        // A different host is a different intended create,
                        // exactly as a different directory is — so the key
                        // the last submit used stops applying (see this
                        // component's docs for both edges of that rule).
                        intent_key.set(None);
                    },
                    for host in hosts.iter() {
                        option {
                            key: "{host.id}",
                            value: "{host.id}",
                            "{host.label()}"
                        }
                    }
                }
            }
            // The reconciliation, said out loud. A chosen host leaving the
            // registry moves the effective target, and the one thing that
            // must not happen is that move being invisible — a selector
            // showing host A while the body carries host B is a create on a
            // machine nobody picked. The key is re-minted for the new target
            // by the submit path's own binding check.
            if choice_vanished {
                div { class: "create-session-host-note",
                    "the host you picked is no longer registered, so this create would go to the \
                     one selected now"
                }
            }
            label {
                "working directory"
                input {
                    r#type: "text",
                    required: true,
                    autocomplete: "off",
                    autocorrect: "off",
                    autocapitalize: "none",
                    spellcheck: "false",
                    value: "{cwd}",
                    disabled: busy,
                    oninput: move |evt| {
                        cwd.set(evt.value());
                        // An edit makes the next submit a DIFFERENT
                        // intent, so the key the last one used stops
                        // applying here (this component's docs carry the
                        // full argument for both edges of that rule).
                        intent_key.set(None);
                    },
                }
            }
            label {
                "agent command"
                input {
                    r#type: "text",
                    required: true,
                    autocomplete: "off",
                    autocorrect: "off",
                    autocapitalize: "none",
                    spellcheck: "false",
                    value: "{invocation}",
                    disabled: busy,
                    oninput: move |evt| {
                        invocation.set(evt.value());
                        // An edit makes the next submit a DIFFERENT
                        // intent, so the key the last one used stops
                        // applying here (this component's docs carry the
                        // full argument for both edges of that rule).
                        intent_key.set(None);
                    },
                }
            }
            label {
                "title (optional)"
                input {
                    r#type: "text",
                    autocomplete: "off",
                    autocorrect: "off",
                    autocapitalize: "none",
                    spellcheck: "false",
                    value: "{title}",
                    disabled: busy,
                    oninput: move |evt| {
                        title.set(evt.value());
                        // An edit makes the next submit a DIFFERENT
                        // intent, so the key the last one used stops
                        // applying here (this component's docs carry the
                        // full argument for both edges of that rule).
                        intent_key.set(None);
                    },
                }
            }
            button {
                r#type: "submit",
                class: "btn create-session-submit",
                // `blocked` as well as this form's own flag: a create must
                // not overlap a host mutation (see `ListView`'s operation
                // gate), and a control that is inert for that window says so
                // rather than silently dropping the click.
                disabled: busy,
                "create"
            }
            if let Some(err) = error.read().clone() {
                // The helm's own words, which for a create refused by a
                // host's state quote that host and its identities — so the
                // same escaping and isolation every other peer string gets
                // (see `peer::PeerLine`).
                PeerLine {
                    class: "create-session-error".to_string(),
                    parts: vec![DetailPart::Peer(err)],
                }
            }
        }
    }
}

/// One row: a plain `<div>` wrapper around three real `<button>`s (open,
/// stop, delete) rather than a `div` with `role`/`tabindex`/a hand-rolled
/// `onkeydown` — every action gets Enter- and Space-activation, focus
/// styling, and screen-reader semantics for free from being a real
/// button, and none of it needs reimplementing here (a hand-rolled div
/// also had a latent bug: Space on a focused element scrolls the page
/// unless the handler prevents default, which native button activation
/// never triggers in the first place).
///
/// The wrapper is a `div`, not a `button`, because M1's whole-row button
/// cannot host the stop/delete actions PLAN_M2.md step 8 adds: HTML
/// forbids interactive content nested inside a `<button>`, so a `<button
/// class="session-row">` containing further `<button>`s would be invalid
/// markup with undefined browser behavior. Splitting the open action into
/// its own sibling button (`.session-row-open`) keeps every action a real,
/// individually focusable button while satisfying that constraint — tab
/// order simply walks open → stop → delete across the row in the NORMAL
/// (not-confirming) state; see "Inline delete confirmation" below for how
/// that changes while a delete prompt is open.
///
/// ## Host and staleness (PLAN_M6.md item 6)
///
/// The row leads with the host it lives on, and a row the helm marked stale
/// — its host is in some non-connected state — is dimmed and badged rather
/// than hidden, per SPEC.md's "stay in the list … clearly marked". Its
/// stop/rename/delete controls stay ENABLED, which looks like an oversight
/// and is not: the helm refuses those operations with the host's own state
/// named in the message ("host 3 is unreachable-reprobing, so this operation
/// is refused and nothing was queued: …"), and that message is strictly more
/// useful than a greyed-out button. Nothing queues either way — SPEC.md v1
/// refuses rather than deferring — so there is no risk of a click being
/// quietly banked.
///
/// `data-session-id` stays on the outer wrapper for Playwright to key off
/// of, joined by `data-session-stale`. No `stop_propagation` on the
/// stop/delete buttons: the wrapper
/// `<div>` has no click handler of its own for a click to bubble into
/// (the open action lives on its own sibling button), so there is nothing
/// here for propagation to trigger by accident.
///
/// `error` is this row's own entry from `ListView`'s per-session error map
/// (`None` when its last action succeeded or none has run yet); `busy` is
/// whether a stop/delete for THIS session is currently in flight, which
/// disables both action buttons — both are `ListView`'s per-session state
/// (see its own docs for why a single shared error/in-flight slot would be
/// wrong), just narrowed to this one row by the caller before it gets here.
///
/// `nav_disabled`, unlike `error`/`busy`, is NOT this row's own state — it
/// is `ListView`'s single global "something is in flight somewhere" flag
/// (see `nav_locked` in `ListView`), applied identically to every row's
/// open button. Opening ANY row unmounts `ListView` (navigation replaces
/// it with `SessionView`), which would silently cancel whatever this row
/// OR any other row's in-flight create/stop/delete was doing — the whole
/// point of disabling it is to keep that unmount from happening at all
/// while anything still needs `ListView` to stay alive to finish.
///
/// ## Inline delete confirmation
///
/// `confirming` (also `ListView`'s per-session state, same discipline as
/// `error`/`busy`) swaps the stop/delete pair out for a prompt plus
/// **confirm delete**/**cancel** in place — there used to be a
/// `window.confirm()` call here instead; wry ships no native JS dialogs on
/// macOS's WKWebView (observed directly running the desktop build), which
/// made delete-on-a-live-session silently do nothing on that target. An
/// in-page prompt has no such platform dependency, so it replaces the
/// eval-based one everywhere, not just on desktop. While it is open, tab
/// order walks confirm delete → cancel; initial FOCUS lands directly on
/// cancel regardless of tab order (see below).
///
/// The `.session-row-open` button (title/cwd/invocation/badge) is given
/// the extra `confirming` class and hidden outright (`display: none` in
/// app.css) rather than merely staying `disabled`, which is what it did
/// before this fix (MT-8): `.session-row-main` lays out its children in
/// one non-wrapping flex row, and that button's own children each carry a
/// `min-width` floor (see `.session-title`/`.session-cwd`/
/// `.session-invocation`) that does not shrink to nothing just because
/// the OUTER flex algorithm hands the button a narrower slot to make room
/// for the confirm prompt's own elements. Past that floor the button's
/// content overflows its shrunk box — CSS flexbox does not clip
/// overflow by default — and renders on top of the confirm prompt sitting
/// immediately after it in the row, rather than being replaced by it.
/// Removing the button from layout entirely while confirming is open
/// sidesteps that interaction completely instead of trying to out-shrink
/// it: the confirm prompt already repeats the title (`.confirm-title`
/// below), so nothing the hidden button showed is lost information while
/// it is gone.
///
/// The prompt itself is TWO separate elements, not one combined sentence:
/// `.confirm-consequence` (from `confirm_consequence`, fixed wording with
/// no title in it at all) and `.confirm-title` (the title alone, quoted).
/// Both are ordinary Dioxus text interpolation, never `document::eval` —
/// a title containing quotes or JS-source-looking text (plausible, since a
/// supervisor over `--ssh` may be a different, possibly untrusted host)
/// renders as inert DOM text, never something parsed as script, which is
/// what makes the injection concern the old eval path needed
/// `serde_json`-encoding to guard against moot on this path. The SPLIT
/// exists for a different reason: a legal title can be tens of KB with no
/// whitespace at all, and app.css lets `.confirm-title` (only) shrink and
/// ellipsize under space pressure while `.confirm-consequence` never
/// shrinks — so the safety-critical "will be killed" half can never be
/// the one that gets clipped, which a single combined, single-ellipsized
/// string could not promise once the title ran long enough. Rendering the
/// consequence element FIRST is what makes "read the risk before the
/// title" the actual reading order, not just an incidental visual
/// side effect that a later DOM-order change could quietly undo.
///
/// ## Inline rename (PLAN_M5.md item 6)
///
/// `renaming` swaps the rename/stop/delete trio for the session's own
/// title plus `rename::RenameForm`, and hides the open button for the same
/// layout reason the confirm prompt does (see `open_class`). The two
/// states are mutually exclusive by construction — `ListView` refuses to
/// open either while the other is showing — so the branches below can be a
/// plain if/else chain rather than a composition of overlays.
///
/// Repeating the title is load-bearing, not decoration: the element that
/// normally shows it is out of layout while the field is open, so a
/// REFUSED rename would otherwise leave the rejected draft as the only
/// name on screen, when SPEC.md requires the old title to stay while the
/// supervisor's refusal is shown.
///
/// The draft itself is `ListView`'s (`rename_draft`), seeded when the
/// field opens; everything the submitted string then goes through is
/// `ListView`'s too. This component neither validates it nor decides what
/// a refusal means.
///
/// Focus-on-open uses the plain HTML `autofocus` attribute on the cancel
/// button (below), not Dioxus's `onmounted`/`set_focus` API: `set_focus`
/// returns a `Result` future that can fail (`MountedError`, e.g. on a
/// renderer that does not support it), and since focus-on-cancel is a
/// safety default — landing keyboard focus on the SAFE action before a
/// stray Enter/Space can reach anything — silently discarding that
/// `Result` would let the safety behavior vanish with nothing to show for
/// it. `autofocus` cannot fail the same way: it is applied by the browser
/// itself at parse/insert time as a plain attribute, with no fallible
/// async call in the UI's own control to get wrong or ignore. It reliably
/// fires exactly once per entry into `confirming` for the same reason
/// `onmounted` would have: the button is only ever created fresh inside
/// the `if confirming` branch below.
#[component]
fn SessionRow(
    session: Session,
    error: Option<String>,
    busy: bool,
    confirming: bool,
    renaming: bool,
    nav_disabled: bool,
    on_open: EventHandler<Session>,
    on_stop: EventHandler<String>,
    on_delete: EventHandler<DeleteTarget>,
    on_confirm_delete: EventHandler<String>,
    on_cancel_delete: EventHandler<String>,
    rename_draft: Signal<String>,
    on_rename_start: EventHandler<(String, String)>,
    on_rename_submit: EventHandler<(String, String)>,
    on_rename_cancel: EventHandler<()>,
) -> Element {
    // `None` for a status nothing has classified yet, and the row then
    // renders no badge ELEMENT at all rather than an empty one — see
    // `status_badge`'s own docs for why an empty badge box would be the
    // same mistake in CSS.
    let badge = status_badge(&session.status, session.annotation.as_deref());
    let open_session = session.clone();
    let stop_id = session.id.clone();
    let delete_target = DeleteTarget {
        id: session.id.clone(),
        status: session.status.clone(),
    };
    let confirm_id = session.id.clone();
    let cancel_id = session.id.clone();
    let rename_start = (session.id.clone(), session.title.clone());
    let rename_submit_id = session.id.clone();
    // The open button is removed from layout by EITHER prompt — one
    // modifier class for both, since what the stylesheet needs to know is
    // "a prompt occupies this row", not which one. The reason is the one
    // MT-8 recorded: `.session-row-main` is a non-wrapping flex row whose
    // children have `min-width` floors, so anything that takes space
    // beside this button — the confirm prompt's elements, or the rename
    // field — is painted over by the button's own overflowing content
    // rather than laid out next to it.
    let open_class = if confirming || renaming {
        "session-row-open prompting"
    } else {
        "session-row-open"
    };
    // A stale row is DIMMED and badged, never hidden or disabled: SPEC.md
    // requires such sessions to stay listed and be clearly marked, and their
    // lifecycle controls stay live because the helm's refusal (which names
    // the host's state) is a better answer than a dead button.
    let row_class = if session.stale {
        "session-row stale"
    } else {
        "session-row"
    };

    rsx! {
        div {
            class: row_class,
            "data-session-id": "{session.id}",
            "data-session-stale": "{session.stale}",
            // Two stacked rows, not one: the buttons need a plain flex
            // ROW (see `.session-row-main` in app.css), but a per-session
            // error line needs its own full-width row underneath rather
            // than squeezing in as a fourth flex item next to the
            // buttons — hence the extra wrapper rather than putting
            // everything directly under `.session-row`.
            div { class: "session-row-main",
                button {
                    r#type: "button",
                    // The modifier is what app.css hides (MT-8, see the
                    // "Inline delete confirmation" section of this
                    // component's doc above and `open_class` itself) —
                    // without it, this button's own title/cwd/invocation
                    // content overflows its flex-shrunk box and paints
                    // over whichever prompt is rendered right after it.
                    class: open_class,
                    // Disabled by ANY of the three locks: the global nav
                    // lock (any in-flight op anywhere), or this row's own
                    // confirmation or rename field being open — the
                    // simplest way to satisfy "cancel is the only way back
                    // to normal" (see the component doc above) is to make
                    // the open button inert for the whole time a prompt is
                    // showing, rather than giving it a second, competing
                    // meaning as an implicit cancel.
                    disabled: nav_disabled || confirming || renaming,
                    onclick: move |_| on_open.call(open_session.clone()),
                    // The host leads the row: with more than one machine in
                    // play it is the first thing that disambiguates two
                    // otherwise identical sessions. The name is the helm's
                    // own rendering (`host_name`), denormalized onto the row
                    // so the list needs no second request — a row from a
                    // helm that sends none simply shows nothing here rather
                    // than inventing a label.
                    // Escaped and direction-isolated like every other
                    // rendering of a destination: this one names the machine
                    // a row's stop and delete will reach, so a name able to
                    // reorder the row around it could make one host's
                    // session read as another's.
                    if let Some(host_name) = &session.host_name {
                        span { class: "session-host peer-value", dir: "ltr",
                            "{display_peer(host_name)}"
                        }
                    }
                    span { class: "session-title", "{session.title}" }
                    span { class: "session-cwd", "{session.cwd}" }
                    span { class: "session-invocation", "{session.invocation}" }
                    // Beside the status badge rather than replacing it: the
                    // last-known status is still what the helm knows, and
                    // this says how old that knowledge is — two facts, not
                    // one, exactly as the stop annotation qualifies rather
                    // than replaces `exited`.
                    if session.stale {
                        span { class: "stale-badge", "stale" }
                    }
                    if let Some((badge_class, badge_text)) = badge {
                        span { class: "status-badge {badge_class}", "{badge_text}" }
                    }
                }
                if confirming {
                    // Called inline, not hoisted into a `let` above this
                    // `if`: this is the ONLY place either half of the
                    // prompt is ever shown, so computing them
                    // unconditionally on every render regardless of
                    // `confirming` would be wasted work on the common
                    // (not-confirming) case, and — since `confirm_consequence`
                    // is documented as never being CALLED outside this
                    // state (see its own doc) — computing it only here is
                    // what actually keeps that contract true rather than
                    // just asserted.
                    //
                    // Two elements, consequence first: see the component
                    // doc above for why an untruncatable consequence and
                    // a separately truncatable title, in THIS order, is
                    // what keeps a long title from ever clipping the
                    // safety-critical half.
                    span {
                        class: "confirm-consequence",
                        "{confirm_consequence(&session.status)}"
                    }
                    span { class: "confirm-title", "\"{session.title}\"" }
                    button {
                        r#type: "button",
                        class: "btn confirm-delete",
                        onclick: move |_| on_confirm_delete.call(confirm_id.clone()),
                        "confirm delete"
                    }
                    button {
                        r#type: "button",
                        class: "btn confirm-cancel",
                        // Safe default: land keyboard focus on cancel, not
                        // confirm, the instant this prompt appears — a
                        // stray Enter/Space right after the row's delete
                        // click (residual focus, a fast typist) then backs
                        // OUT of the destructive action instead of into
                        // it. Declarative `autofocus`, not `onmounted` +
                        // `set_focus`: see the component doc above for why
                        // the fallible, discardable-`Result` async API was
                        // rejected in favor of a plain HTML attribute that
                        // cannot silently fail to apply.
                        autofocus: true,
                        onclick: move |_| on_cancel_delete.call(cancel_id.clone()),
                        "cancel"
                    }
                } else if renaming {
                    // The field takes the action area over exactly as the
                    // confirm prompt does, rather than sitting beside the
                    // buttons: a text field needs real width, and the row
                    // has none to spare (see `open_class`).
                    //
                    // The AUTHORITATIVE title is repeated alongside it,
                    // and that is a requirement rather than a nicety: the
                    // open button that normally shows it is out of layout
                    // here, so without this element a refused rename would
                    // leave the rejected DRAFT as the only name on screen
                    // — the opposite of SPEC.md's "the old title stays"
                    // while the supervisor's refusal is shown. It shrinks
                    // and ellipsizes (app.css) so a legal multi-KB title
                    // cannot push the field or its buttons off the row.
                    span { class: "rename-current-title", "{session.title}" }
                    RenameForm {
                        draft: rename_draft,
                        busy,
                        on_submit: move |title| {
                            on_rename_submit.call((rename_submit_id.clone(), title))
                        },
                        on_cancel: move |_| on_rename_cancel.call(()),
                    }
                } else {
                    button {
                        r#type: "button",
                        class: "btn session-row-rename",
                        disabled: busy,
                        onclick: move |_| on_rename_start.call(rename_start.clone()),
                        "rename"
                    }
                    button {
                        r#type: "button",
                        class: "btn session-row-stop",
                        disabled: busy,
                        onclick: move |_| on_stop.call(stop_id.clone()),
                        "stop"
                    }
                    button {
                        r#type: "button",
                        class: "btn session-row-delete",
                        disabled: busy,
                        onclick: move |_| on_delete.call(delete_target.clone()),
                        "delete"
                    }
                }
            }
            // The helm's own refusal, and on a stale row it names the host's
            // state and can quote peer-supplied text — so it renders through
            // the same escaping and isolation as every other peer string.
            if let Some(err) = error {
                PeerLine {
                    class: "action-error".to_string(),
                    parts: vec![DetailPart::Peer(err)],
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A CONNECTED host as the create dialog would be offered it —
    /// `phase: None` is what "connected" means to an option.
    fn option(id: HostId, name: &str, local: bool) -> HostOption {
        HostOption {
            id,
            name: name.into(),
            local,
            phase: None,
            incarnation: format!("incarnation-{id}"),
        }
    }

    /// The same, for a host in some non-connected phase: still offered, and
    /// labelled with the phase so the choice is informed.
    fn option_in(id: HostId, name: &str, local: bool, phase: &str) -> HostOption {
        HostOption {
            phase: Some(phase.into()),
            ..option(id, name, local)
        }
    }

    /// The default is the LOCAL row, and SPEC.md's open-session clause is
    /// deliberately not implemented — see `default_create_host` for why
    /// plumbing an unreachable branch is worse than an absent one.
    #[test]
    fn the_create_default_is_the_local_row() {
        let hosts = vec![
            option(1, "this machine", true),
            option(2, "user@box", false),
        ];
        assert_eq!(default_create_host(&hosts), Some(1));
    }

    /// The local row is the fallback WHATEVER phase it is in — SPEC.md names
    /// the helm's own host unconditionally.
    ///
    /// This is the rule an earlier shape got wrong by filtering the options
    /// to connected hosts: with the local supervisor down, the local row
    /// vanished from the list and the default silently moved to a remote
    /// machine, so a create the user thought was landing here would launch
    /// an agent somewhere else. Offering it and letting the helm refuse in
    /// its own words is the honest behavior.
    #[test]
    fn the_local_row_is_the_default_even_when_it_is_not_connected() {
        let hosts = vec![
            option_in(1, "this machine", true, "unreachable-reprobing"),
            option(2, "user@box", false),
        ];
        assert_eq!(
            default_create_host(&hosts),
            Some(1),
            "a down local host is still the helm's own host"
        );
    }

    /// With no local row to fall back on, the answer is NOTHING rather than
    /// an arbitrary host.
    ///
    /// "The first host in the list" was the earlier fallback and had no rule
    /// behind it: first is whatever the registry happens to order first, and
    /// creating an agent there is a guess about which machine the user
    /// meant. `None` leaves the selector empty, which the form refuses to
    /// submit with a reason. In practice this state exists only before the
    /// first hosts read lands, since a live helm always has its local row.
    #[test]
    fn a_fleet_with_no_local_row_defaults_to_nothing_rather_than_guessing() {
        assert_eq!(default_create_host(&[]), None);

        let remote_only = vec![option(2, "user@box", false), option(3, "user@other", false)];
        assert_eq!(
            default_create_host(&remote_only),
            None,
            "picking whichever host sorted first would be a create on a machine nobody chose"
        );
    }

    /// An intent is a command, in a directory, on one INCARNATION of a host
    /// — so a binding must differ whenever any of those does, and the
    /// incarnation is the part an id alone cannot express.
    ///
    /// The failure this pins is the expensive one: a retarget or an adopt
    /// leaves the id untouched, so a key bound to the id alone survives into
    /// a retry aimed at a machine that has never seen it, where it dedups
    /// nothing and launches a second real agent.
    #[test]
    fn an_intent_binding_changes_with_the_host_incarnation_and_with_the_fields() {
        let hosts = vec![option(1, "this machine", true)];
        let base = IntentBinding::of(
            Some(1),
            &hosts,
            "/tmp".to_string(),
            "agent".to_string(),
            "title".to_string(),
        )
        .expect("the selected host is in the list");

        // Same id, different incarnation: a retarget or an adopt.
        let moved = vec![HostOption {
            incarnation: "incarnation-after-the-retarget".to_string(),
            ..hosts[0].clone()
        }];
        let after = IntentBinding::of(
            Some(1),
            &moved,
            "/tmp".to_string(),
            "agent".to_string(),
            "title".to_string(),
        )
        .expect("still selectable");
        assert_ne!(
            base, after,
            "the row is the same row; the machine behind it is not"
        );

        // Every form field is part of the intent too — this is what the
        // post-mint re-read compares against.
        for edited in [
            IntentBinding {
                cwd: "/other".to_string(),
                ..base.clone()
            },
            IntentBinding {
                invocation: "other-agent".to_string(),
                ..base.clone()
            },
            IntentBinding {
                title: "other title".to_string(),
                ..base.clone()
            },
        ] {
            assert_ne!(base, edited);
        }

        // And an unchanged submit is the SAME intent, which is the whole
        // point of the key surviving a failure.
        assert_eq!(
            base,
            IntentBinding::of(
                Some(1),
                &hosts,
                "/tmp".to_string(),
                "agent".to_string(),
                "title".to_string()
            )
            .expect("still selectable")
        );
    }

    /// A submit with no host selected has no binding at all — the one case
    /// the form refuses locally instead of sending, because a hostless body
    /// would be silently defaulted by the helm to a machine the user was
    /// never shown.
    #[test]
    fn no_selected_host_yields_no_binding() {
        let hosts = vec![option(1, "this machine", true)];
        assert!(
            IntentBinding::of(None, &hosts, String::new(), String::new(), String::new()).is_none()
        );
        assert!(
            IntentBinding::of(
                Some(99),
                &hosts,
                String::new(),
                String::new(),
                String::new()
            )
            .is_none(),
            "a selection the option list no longer contains is not a target either"
        );
    }

    /// A non-connected option must SAY so in its label, and a connected one
    /// must not be decorated.
    ///
    /// Every host is selectable now, so the label is what keeps that from
    /// being a trap: the phase a user sees before choosing is the same word
    /// the helm's refusal will use if they choose it anyway.
    #[test]
    fn an_option_label_names_the_phase_only_when_there_is_one_to_warn_about() {
        assert_eq!(option(1, "this machine", true).label(), "this machine");
        assert_eq!(
            option_in(2, "user@box", false, "unreachable-reprobing").label(),
            "user@box (unreachable-reprobing)"
        );
        // The name is peer/user-supplied and is escaped like every other
        // rendering of it: an option label is exactly where a directional
        // override could make one host read as another, and what is chosen
        // there decides which machine a command runs on.
        assert_eq!(
            option(3, "user@\u{202E}box", false).label(),
            "user@<U+202E>box"
        );
    }
}

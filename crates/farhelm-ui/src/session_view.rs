//! One open session: `SessionView`, the stateful component that owns the
//! terminal tabs feature's signals, futures, and effects (mount/unmount,
//! the reads the invalidation feed drives, the optimistic open/close
//! corrections, the lease). The tab
//! domain it calls into — which tabs to show, their labels and DOM ids,
//! the close-confirmation wording (renderer-free, unit-tested), plus the
//! strip's presentational `TabStripItem` — lives in `tabs`; see that
//! module's own docs for why the split is drawn there. The restart affordance's own pure wording
//! helpers (`restart_offer_text`, `restart_button_label`) stay local, since
//! nothing outside this component calls them.

use std::collections::{HashMap, HashSet};

use dioxus::prelude::*;

use crate::api::{
    close_tab, fetch_hosts, fetch_session, mint_lease, open_tab, rename_session, restart_mode_for,
    restart_session,
};
use crate::attachments::{attachment_policy, attachment_status_element_id};
use crate::feed::{fallback_polls_now, fallback_sleep, use_feed_reader};
use crate::hosts::{HostLookup, HostsRead, is_connected, stale_session_notice};
use crate::ops::ReadGate;
use crate::peer::PeerLine;
use crate::reader::{SurfaceReader, Trigger, request_read};
use crate::reconnect::reconnect_policy;
use crate::rename::RenameForm;
use crate::status::status_badge;
use crate::tabs::{
    AGENT_BANNER_ELEMENT_ID, AGENT_CONNECTING_ELEMENT_ID, AGENT_TERMINAL_ELEMENT_ID,
    CLOSE_TAB_CONSEQUENCE, MAX_MOUNTED_TAB_ISLANDS, TAB_OPEN_ERROR_KEY, TabStripItem,
    sorted_tab_errors, tab_banner_element_id, tab_connecting_element_id, tab_label,
    tab_terminal_element_id, terminal_ws_path, terminal_ws_unowned_path, visible_tabs,
};
use crate::{ApiBase, RestartOffer, Session, SessionStatus};

/// One session: a tab strip over one terminal per open terminal, with a
/// back control above them. Each terminal div is handed to the JS island
/// on mount; Dioxus never touches its children again — that boundary is
/// the whole design.
///
/// ## Tabs (PLAN_M4.md item 6)
///
/// The agent terminal comes first and cannot be closed; every open tab
/// follows it, labeled positionally. Every one of them is ATTACHED at the
/// same time, and selecting a tab is a CSS visibility change, never an
/// attach cutover — attach-on-select was considered and rejected, since
/// each switch would pay a full replay and exercise the takeover machinery
/// for no gain. What that costs is one xterm instance, one WebSocket, and
/// one supervisor-side control client per tab; what it buys is that a
/// background tab keeps consuming its own stream, which per-terminal flow
/// control (PLAN_M4.md item 3) makes safe.
///
/// Hidden panes are hidden with `visibility`, not `display: none`, and
/// that is load-bearing rather than stylistic: a `display: none` element
/// has no layout box, so `FitAddon.fit()` would size every unselected
/// terminal to zero columns at mount time — and every tab in a session
/// with more than one mounts while hidden. See app.css's `.terminal-pane`.
///
/// A tab whose WINDOW is gone (closed from another client, erased by a
/// reboot) needs no special case here: its attach fails and the helm
/// relays the reason as an ordinary detach notice, which is the same
/// no-terminal explanation the session view has always rendered. A tab
/// whose SHELL merely exited is a live, viewable dead pane, exactly as the
/// agent terminal is.
///
/// ## Paste and drop interception (PLAN_M4.md item 7)
///
/// Every terminal this view mounts also intercepts pastes and drops, and
/// this component's whole part in that is declarative: each pane renders
/// an empty status line whose id comes from `attachments`, and each
/// terminal spec carries that id alongside the shared policy the same
/// module builds. The interception itself — classifying the payload,
/// uploading, inserting the host path at the cursor — is terminal.js's,
/// registered per island inside `mount()`, because a `File` on a DOM event
/// is not something either renderer can hand to Rust (see the
/// `attachments` module header).
///
/// ## Catch-up presentation (PLAN_M5.md item 5)
///
/// Every pane renders one more empty, terminal.js-owned element: the
/// connecting placeholder that stands in front of a terminal while its
/// attach replays what the user missed. This component's whole part in
/// that is declarative — render the element, name it in the terminal's
/// spec — because the reveal happens inside a `term.write()` completion
/// callback, a moment the reactive layer has no way to observe.
///
/// ## Auto-reconnect (PLAN_M6.md item 7)
///
/// A terminal that loses its TRANSPORT gets itself back, and this
/// component's part in that is one more policy object handed to
/// `farhelmTerm.sync()` — the ladder and the wording from `reconnect`,
/// applied by terminal.js. Nothing here changes: the recovery reuses this
/// view's existing spec, lease included, so it is an ordinary reattach of a
/// channel this view already owns rather than anything the reactive layer
/// has to arrange. The surface it paints is the same connecting element
/// declared below, wearing its recovery face.
///
/// ## Rename (PLAN_M5.md item 6)
///
/// The header carries the rename surface: clicking `rename` opens
/// `rename::RenameForm` NEXT TO the title (never in place of it — a
/// refusal has to leave the old name on screen), and what is typed is sent
/// verbatim. Only the reply's TITLE is taken; the poll owns every dynamic
/// field, so a slow rename can never roll status or tabs backwards. A
/// refusal is shown under the header while the title stays what it was.
/// The list has the same affordance on every row (`list::SessionRow`), and
/// both are optimistic-then-poll-corrected in the same way — see `renamed`
/// below.
///
/// ## The attachment lease (PLAN_M4.md item 3)
///
/// One high-entropy lease per VIEW INSTANCE, minted here (see
/// `api::mint_lease`) and carried on every terminal WebSocket this view
/// opens, agent included. The supervisor groups a session's channels by it
/// to enforce SPEC.md's one-attached-client rule across all of a session's
/// terminals at once.
///
/// This deliberately sharpens what a takeover means: a second view of the
/// same session now detaches ALL of the first view's terminals, not just
/// the one they have in common. That is the rule working as specified —
/// the attached client owns the session's terminals — and each losing
/// terminal still banners its own detach.
///
/// No terminal mounts until the lease exists, and a lease that cannot be
/// minted is fatal to this view's terminals rather than something to
/// degrade around. That is not caution copied from `list::CreateSessionForm`:
/// an un-leased attach is read as its own one-terminal client, so N
/// un-leased attachments from ONE view would take each other over in turn
/// and the session would end up with exactly one live terminal and no
/// explanation. Refusing loudly beats shipping that.
///
/// ## A stale session shows metadata, not a terminal (PLAN_M6.md item 6)
///
/// When the helm marks this session stale — its host is in any
/// non-connected state — this view renders its metadata behind a notice and
/// mounts NO terminal at all. SPEC.md: "opening such a session shows its
/// metadata — title, directory, last-known status — behind a clear
/// host-unreachable notice; there is no terminal to show." Attaching anyway
/// would produce a socket the helm refuses, and a blank pane under a failed
/// attach explains nothing.
///
/// The notice names the host's ACTUAL state rather than saying "unreachable"
/// for every cause (`hosts::stale_session_notice`), because a skewed host
/// described as unreachable hides the upgrade that fixes it and a
/// mismatched one hides that a decision is being waited on. That costs one
/// extra read of `/api/hosts`, taken only while the session is stale — see
/// the host-state read below.
///
/// The restart affordance deliberately stays: a refused restart surfaces the
/// helm's own words, which is the same bargain the list's stale rows make
/// with their lifecycle controls.
///
/// ## Mount/unmount lifecycle (PLAN_M2.md step 7)
///
/// M1 never unmounted this component (it was the only view), so
/// `terminal.js` only ever needed a mount-time guard against a
/// re-render calling `mount()` twice. Now that `App` can navigate away
/// and back, this component must clean up on drop — otherwise that
/// guard (terminal.js's `islands` map, now the ONLY mount guard — see its
/// own docs) would permanently wedge shut, and reopening ANY session
/// after the first would silently no-op `mount()` instead of attaching.
/// `use_drop` fires `farhelmTerm.unmountAll()` for exactly that reason: it
/// is the regression this lifecycle work exists to prevent, not a
/// hypothetical.
///
/// Individual mounts are NOT driven from here. This component computes the
/// full set of terminals it wants and hands it to `farhelmTerm.sync()`,
/// which reconciles (see terminal.js's header for why the diff belongs on
/// that side). The effect below is therefore idempotent by construction,
/// which is what makes it safe for every feed notification to re-run it
/// forever.
///
/// ## The sync-generation token
///
/// `window.__farhelmSyncGeneration` guards only the OUTER wait here —
/// for `window.farhelmTerm` to exist at all, since terminal.js's
/// `document::Script` is injected asynchronously and this component can
/// render before it has executed (a real, if rare, race). Bumping the
/// counter on every sync attempt AND on drop means backing out before
/// that wait resolves reliably cancels it. Once `window.farhelmTerm`
/// exists, `mountWhenReady`'s OWN per-island wait for xterm's globals is a
/// separate concern guarded entirely inside terminal.js (its `pendings`
/// replacement-and-clear scheme — see that function's docs); this token
/// does not reach that far.
/// What a detail reply may do to this view.
///
/// Three outcomes rather than a boolean, because "do not apply this" has two
/// meanings that differ in what happens NEXT — and collapsing them is what
/// left a restart showing the run that had just ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Admission {
    /// Commit it: this is the newest word about the session as it is now.
    Apply,
    /// A newer read has already committed. Drop this reply, and count the
    /// surface as ANSWERED — someone else did the work.
    Superseded,
    /// A restart happened after this read started, so the reply describes a
    /// run that no longer exists. Drop it, and do NOT count the surface as
    /// answered: nothing was applied, and the demand that produced this read
    /// is still owed.
    Discarded,
}

/// Whether a detail reply may touch this view at all.
///
/// Split out of `SessionView`'s commit path so the rule can be exercised
/// without a Dioxus runtime, and because the shape of it is the whole point:
/// ONE decision covering EVERY answer the helm can give, rather than a gate
/// on the happy path and a separate ungated route for the rest. It is
/// deliberately blind to the PAYLOAD — a session and a 404 are both
/// statements about the same moment, and the bug this shape fixes was
/// exactly a 404 that skipped the epoch check because it was not an
/// `Ok(Some(..))`.
///
/// - **Ordering** is `ops::ReadGate`'s: an older reply landing last must
///   lose, or a stale session overwrites newer status, title and tabs, and a
///   stale 404 raises the staleness notice over a view that was just
///   refreshed. Neither self-corrects — a healthy feed keeps the fallback
///   poll off, and the next notification only comes if the fleet changes.
/// - **The restart epoch** is a different question: not "is this the newest
///   reply" but "does this reply describe the run this view is showing".
///   A read held across a restart can be both newest and obsolete.
///
/// A transport failure never reaches here: it commits nothing, and admitting
/// it would let a dropped request advance the ordering and then refuse the
/// perfectly good reply that follows it.
fn admit_detail(
    reads: &mut ReadGate,
    generation: u64,
    started_at: u32,
    epoch_now: u32,
) -> Admission {
    if !reads.accept_success(generation) {
        return Admission::Superseded;
    }
    if started_at != epoch_now {
        return Admission::Discarded;
    }
    Admission::Apply
}

#[component]
pub(crate) fn SessionView(session: Session, on_back: EventHandler<()>) -> Element {
    let base = use_context::<ApiBase>().0;
    // The session as this view currently understands it. Seeded from the
    // prop and then owned here, because a restart changes it: the reply
    // carries the session's recomputed status and offer, and a REFUSED
    // restart is exactly the case where re-reading the server's current
    // answer matters (a stale offer is what caused the refusal).
    let mut current = use_signal(|| session.clone());
    // Bumped on every successful restart to force the AGENT terminal's
    // island to remount — and only that one, since restart touches the
    // agent terminal alone (the supervisor's `detach_for_restart`) and a
    // tab's attachment survives it untouched. Both reuse cases need it: a
    // reused pane's attachment was deliberately torn down by the restart,
    // and a fresh terminal is a different pane entirely. Remounting also
    // replays the pane's scrollback, which for a reused terminal is what
    // puts the PRIOR run's output back on screen above the new one.
    let mut mount_generation = use_signal(|| 0_u32);
    // Whether a restart is in flight (disables the control and guards
    // re-entry, mirroring `ListView`'s `pending`), and whether the inline
    // confirm prompt is open for a still-running agent (the same in-page
    // pattern delete uses — never a browser dialog, which wry's macOS
    // webview does not have at all).
    let mut restarting = use_signal(|| false);
    let mut confirming = use_signal(|| false);
    let mut restart_error = use_signal(|| None::<String>);
    // The rename affordance (PLAN_M5.md item 6): whether the field is
    // open, whether its request is in flight, the supervisor's own words
    // if it refused, and this view's optimistic correction over `current`.
    //
    // The correction is the tab strip's `(value, observed_from)` scheme
    // applied to a title, and it earns its keep for the same reason: the
    // detail read below can have a fetch in the air from before the rename
    // landed, and committing that reply's older title would flip the
    // header back until the next read landed. Pairing the new title with
    // the index of the first read GUARANTEED to start after the rename's
    // response is what tells "this reply cannot have carried it yet" apart
    // from "the server disagrees".
    //
    // The draft is a signal here rather than inside `RenameForm` for the
    // reason that component's docs give: a re-render can unmount the form
    // for reasons the user did not cause, and a draft that died with it
    // would silently discard what they had typed.
    let mut renaming = use_signal(|| false);
    let mut renaming_pending = use_signal(|| false);
    let mut rename_error = use_signal(|| None::<String>);
    let mut renamed = use_signal(|| None::<(String, u64)>);
    let mut rename_draft = use_signal(String::new);
    // Counts restart ATTEMPTS, and versions every read of this session
    // against them. The detail read below is a round trip that can outlive
    // a restart the user starts while one of its fetches is in flight;
    // without this, that stale answer would land on top of the restart's
    // own fresher one and the view would go back to describing the run
    // that no longer exists.
    let mut restart_epoch = use_signal(|| 0_u32);
    // Which of several in-flight reads may be BELIEVED, per surface
    // (`ops::ReadGate`). Two gates rather than one because they order two
    // unrelated resources: a slow `/api/hosts` must not make a fresh session
    // detail look superseded, or vice versa.
    //
    // Distinct from `restart_epoch`, which answers a different question —
    // that one invalidates reads that span a restart, this one orders reads
    // against each other. Both are needed: a read can be perfectly current
    // with respect to restarts and still be the older of two replies.
    let mut detail_reads = use_signal(ReadGate::default);
    let mut host_reads = use_signal(ReadGate::default);
    // How many times this session has crossed the stale/fresh line. The
    // host-state read's own epoch, mirroring what `restart_epoch` does for
    // the detail read: ordering says which of two replies is newer, and this
    // says whether a reply is still about the situation that asked for it.
    // A host reading taken while the session was stale must not explain a
    // session that has since gone fresh, and must not be the thing that
    // fires the reconnect follow-up.
    let mut stale_epoch = use_signal(|| 0_u32);

    // This view's attachment lease, `None` until minting finishes (it is
    // an `await` on both renderers) and `lease_error` instead if it never
    // does. See this component's own docs for why nothing attaches without
    // it rather than falling back to an un-leased attach.
    let mut lease = use_signal(|| None::<String>);
    let mut lease_error = use_signal(|| None::<String>);
    // Which terminal is on screen: `None` is the agent terminal, `Some(id)`
    // a tab. Only the SELECTION changes here — every terminal stays
    // attached regardless of what this holds.
    let mut selected = use_signal(|| None::<String>);
    // The optimistic corrections `visible_tabs` applies over the server's
    // list, so the user sees their own open/close before the next detail
    // read confirms it. Pruned by that read (below) once the server agrees.
    //
    // `opened_tabs` carries a SEQUENCE NUMBER per entry, not just an id,
    // and that is what keeps a phantom tab from living forever. A reply
    // that omits an optimistic tab is only evidence of absence if the
    // request behind it was sent AFTER the open completed; a read already
    // in flight when the tab was created legitimately predates it. Without
    // the number, "absent" could never be distinguished from "too early",
    // so an entry the server would never list — one another client closed
    // before this view ever saw it listed — had no way to be retired, and
    // the strip would show a tab that attaches to nothing until the view
    // was closed. `poll_sequence` below is the counter both sides read.
    let mut opened_tabs = use_signal(Vec::<(String, u64)>::new);
    let mut closed_tabs = use_signal(HashSet::<String>::new);
    // How many detail reads have been STARTED by this view. Incremented at
    // each fetch's launch, so a read's own index is the value it took
    // before incrementing, and an optimistic open recording the current
    // value names the first read that could possibly know about it.
    //
    // Named for the polls it used to count, deliberately: `ListView` keeps
    // the same counter under the same name, and renaming one of them would
    // leave two vocabularies for one scheme.
    let mut poll_sequence = use_signal(|| 0_u64);
    // In-flight guards, mirroring `ListView`'s `pending`: one flag for the
    // single add control, a set for closes (which are per tab and can
    // legitimately overlap).
    let mut opening_tab = use_signal(|| false);
    let mut closing_tabs = use_signal(HashSet::<String>::new);
    // Which tab, if any, is showing the inline close confirmation.
    let mut confirming_close = use_signal(|| None::<String>);
    // Set when a detail read gets a 404 for this session: what is on
    // screen is the last state that DID arrive, and this says so. See
    // `fetch_session` for why a 404 cannot be read as "deleted" — the
    // helm's detail route is listing-backed and inherits its cap — and
    // therefore why this is a staleness notice rather than an obituary.
    let mut refresh_stale = use_signal(|| false);
    // Tab-operation failures, keyed by OPERATION rather than pooled into
    // one slot — the same discipline `ListView` keeps for its per-session
    // errors, and for the same reason: with a single slot, closing tab B
    // successfully would wipe the still-unread failure from closing tab A,
    // and two concurrent failures would clobber each other so the user
    // only ever learned about whichever lost the race. The key is
    // `TAB_OPEN_ERROR_KEY` for the add control and the tab's own id for a
    // close, so an operation only ever clears its own message.
    let mut tab_errors = use_signal(HashMap::<String, String>::new);
    // The hosts read behind a stale session's notice, refreshed ONLY while
    // the session is stale — that is the one time this view needs it, and
    // reading `/api/hosts` for every open terminal would be a second request
    // per notification to answer a question nobody is asking.
    //
    // The four-state model rather than an `Option<Host>`, because this
    // surface has to tell three failures apart: not read yet, could not be
    // refreshed, and confirmed gone from the registry. An `Option` collapses
    // all three into "no host", and the worst of the three — a failed
    // refresh — would then be rendered as the phase this view happened to
    // see last, which is a claim about the past dressed as the present.
    let mut host_read = use_signal(HostsRead::default);

    // Minted once, on mount, and never re-minted: the lease identifies
    // THIS view instance for as long as it lives, so a remount of one
    // terminal (a restart) must keep it — reusing the lease is exactly
    // what makes that remount an ordinary reconnect of one channel rather
    // than a takeover of the view's own sibling terminals.
    use_future(move || async move {
        match mint_lease().await {
            Ok(minted) => lease.set(Some(minted)),
            Err(reason) => lease_error.set(Some(format!(
                "could not generate this view's terminal lease, so no terminal was attached \
                 (without one, this session's terminals would take each other over): {reason}"
            ))),
        }
    });

    // Keep the session DETAIL current for as long as this view is open
    // (PLAN_M4.md item 6, rebuilt on the feed by PLAN_M6_75.md item 6).
    //
    // Through M3 this was ONE refresh on open, and its doc said polling
    // was deliberately not wanted: the `Session` this view is handed can
    // be a create-time placeholder (`SessionCreated` reports `Unknown`
    // deliberately — see the supervisor's create docs) and the restart
    // affordance reads `status`, but that status is only ever a UI HINT —
    // the supervisor rechecks real liveness and refuses a restart that
    // would kill an agent without consent — so one fetch was enough. Tabs
    // change that, and only that: the tab list has no server-side recheck
    // standing behind it, and SPEC.md's changes-appear-automatically rule
    // means a tab opened or closed from another client has to show up
    // without a reload. M2's poll was the interim mechanism; the
    // invalidation feed is what actually delivers that promise, and the
    // status refresh comes along with it for free.
    //
    // Every trigger is scoped to this component, so all of them stop when
    // the view unmounts — the same lifecycle the poll relied on, now
    // carrying the stronger property that only the MOUNTED page re-reads.
    // Each of the two surfaces (this session's detail, and the host registry
    // behind a stale notice) runs its reads through one `reader`, so a burst
    // of notifications coalesces into one follow-up read and a read that
    // never answered is retried rather than forgotten.
    //
    // Everything that happens to a detail reply once it is BACK: decide
    // whether it still speaks for this view, retire the optimistic
    // corrections it settles, and paint it.
    //
    // Hoisted out of the read so the read body reads as what it is — a
    // fetch and a commit — rather than as forty lines in which the two are
    // interleaved. The counters it needs are PARAMETERS rather than reads
    // of their own, and that is the load-bearing part: all three must be
    // sampled synchronously at the point the request is issued, BEFORE the
    // `await`, or they stop describing the request they are supposed to
    // version. `restart_epoch` is read a second time INSIDE, after the
    // await, and that one has to stay here: it is the current value that
    // `started_at` is compared against.
    //
    // ## Two orderings, and why the epoch is not enough on its own
    //
    // `started_at` versions a reply against RESTARTS, which is a different
    // question from whether a reply is the newest one. Three triggers read
    // this session — the mount read, the feed's re-read, the host-state
    // follow-up — and they overlap freely, so without a generation gate an
    // older reply committing last would silently overwrite newer status,
    // title and tabs. Nothing would correct it either: with the feed healthy
    // the fallback poll is off, and the next notification only arrives if
    // the fleet changes again.
    //
    // ## The epoch has TWO boundaries, and both are bumps
    //
    // A restart invalidates reads on either side of itself, and they are
    // different sets:
    //
    // - **In flight when the user clicked.** The bump before the request
    //   goes out invalidates every read already walking. Those replies
    //   describe the run that is being replaced, and the newest of them can
    //   easily land after the restart has finished — putting the old run's
    //   status, offer and tabs back on screen.
    // - **STARTED DURING the restart.** The bump after it finishes
    //   invalidates reads issued while the request was in the air. Those
    //   share the first bump's epoch, so they pass that guard, and they saw
    //   a session mid-relaunch — a supervisor that has torn the old pane
    //   down and not yet execed the new agent. Without the second bump the
    //   view would settle on that intermediate state and stay there.
    //
    // The refresh a restart owes is therefore issued AFTER the second bump
    // (see `restart`), which is what makes it the only read in the set that
    // can commit.
    //
    // Every ANSWER goes through both guards, 404s included, and the return
    // value says which of the three things happened (`Admission`). A caller
    // needs that distinction: a reply the epoch DISCARDED applied nothing, so
    // the demand behind it is still owed and reporting the read as answered
    // would spend a notification on nothing.
    //
    // Takes the reply as an `Option<Session>` rather than the fetch's own
    // `Result`: an error commits nothing at all, so the caller drops it
    // before this closure is ever reached and the failure arm here would be
    // one nobody could take.
    let mut commit_detail =
        move |generation: u64, fetched: Option<Session>, started_at: u32, index: u64| {
            match admit_detail(
                &mut detail_reads.write(),
                generation,
                started_at,
                *restart_epoch.peek(),
            ) {
                // A newer read already spoke for this view; this one is spent
                // rather than lost.
                Admission::Superseded => true,
                // Held across a restart: it describes a run that no longer
                // exists. Nothing is applied and nothing is satisfied.
                Admission::Discarded => false,
                Admission::Apply => {
                    // A 404 is surfaced rather than absorbed, and deliberately
                    // NOT acted on: the view keeps everything it has —
                    // metadata, tabs, live terminals — and merely stops
                    // claiming to be current. `fetch_session`'s docs carry the
                    // reason this is not "the session was deleted".
                    let Some(fresh) = fetched else {
                        refresh_stale.set(true);
                        return true;
                    };
                    refresh_stale.set(false);
                    // Prune the optimistic corrections this reply settles.
                    // Deliberately NOT done on a failed or 404 fetch — an
                    // error carries no tab list at all, and a transient
                    // failure is not evidence about what exists
                    // (`ListView`'s commit path makes the same call for the
                    // same reason).
                    let live: HashSet<&str> =
                        fresh.tabs.iter().map(|tab| tab.id.as_str()).collect();
                    // An optimistic open retires two ways: the server now
                    // lists it (it graduated to the real list), or this
                    // read STARTED after the open and still does not
                    // mention it (it is genuinely gone — closed from
                    // another client between creation and this view's
                    // first sight of it). A read that predates the open
                    // says nothing either way and leaves it alone.
                    opened_tabs.write().retain(|(id, observed_from)| {
                        !live.contains(id.as_str()) && index < *observed_from
                    });
                    closed_tabs.write().retain(|id| live.contains(id.as_str()));
                    // The optimistic rename retires on exactly the two
                    // reads that settle it — the server now reports this
                    // title (it graduated), or a reply GUARANTEED to have
                    // started after the rename's response completed
                    // reports a different one (another client renamed it
                    // afterwards, and the server wins). A reply that may
                    // predate it says nothing either way and leaves it
                    // alone; that bound is conservative rather than exact
                    // (see `on_rename_submit`), which can only hold a
                    // correction a little longer than needed.
                    let settled = renamed
                        .peek()
                        .as_ref()
                        .is_some_and(|(title, observed_from)| {
                            &fresh.title == title || index >= *observed_from
                        });
                    if settled {
                        renamed.set(None);
                    }
                    current.set(fresh);
                    true
                }
            }
        };

    // One detail read, shared by every trigger — the mount read, the feed's
    // re-read, the fallback poll, and the host-state read's follow-up. Going
    // through one door is what made replacing the poll with the feed a
    // change of TRIGGER only: the commit closure above, and every
    // reconciliation rule in it, is reached the same way from all of them.
    //
    // All three versioning values are sampled synchronously, BEFORE the
    // await, and that is load-bearing rather than tidy. `generation` orders
    // this read against every other one; `started_at` versions it against
    // restarts, so a restart landing mid-flight invalidates only the request
    // that was in the air when it happened; `index` is this read's position
    // in the view's own read order, which is what tells an optimistic tab
    // whether this reply is late enough to be evidence about it. Sampled
    // after the await, all three would describe a moment the reply has
    // already outlived.
    //
    // Reports whether the helm ANSWERED, which is what the surface reader
    // needs (see `reader`). A 404 counts: the helm replied about this
    // session, the view said so, and retrying the same question in a loop
    // would not change the answer.
    let detail_base = base.clone();
    let detail_id = session.id.clone();
    let read_detail = move || {
        let base = detail_base.clone();
        let id = detail_id.clone();
        let generation = detail_reads.write().start();
        let started_at = restart_epoch.peek().to_owned();
        let index = poll_sequence.peek().to_owned();
        poll_sequence += 1;
        async move {
            // A read that never reached the helm is not evidence about the
            // session and never reaches the commit path — the surface reader
            // retries it (see `reader`).
            let Ok(fetched) = fetch_session(&base, &id).await else {
                return false;
            };
            // The commit's own answer, because a reply can arrive and still
            // leave the surface unread: one the restart epoch discards
            // applied nothing, so the demand stands and the reader asks
            // again.
            commit_detail(generation, fetched, started_at, index)
        }
    };

    // The detail surface's reader: one read at a time, notices arriving
    // mid-read coalesced into one follow-up, and a read that never answered
    // retried until one does. The retry is what keeps a failed read from
    // being permanent — the notification that prompted it is spent, and with
    // a healthy feed nothing else is coming (see `reader`). A notice landing
    // during one of those retry waits is dispatched at once rather than
    // waiting the rung out: the backoff belongs to the reader's own blind
    // retries, and a notice is neither blind nor late.
    let detail_surface = use_signal(SurfaceReader::default);
    let request_detail =
        move |trigger: Trigger| request_read(detail_surface, trigger, read_detail.clone());

    // The host-state read behind a stale session's notice (PLAN_M6.md item
    // 6), likewise through one door and under its own generation gate. Only
    // ever called while the session READS stale — that is the one time this
    // view needs the registry at all, and reading `/api/hosts` for every
    // open terminal would be a second request to answer a question nobody is
    // asking.
    //
    // Gated for the same reason the detail read is: the staleness transition
    // fires it, the feed fires it, and the fallback fires it, so two reads
    // can be in the air at once and an older registry landing last would
    // explain this session's staleness with a phase that has since changed.
    let host_base = base.clone();
    let follow_up_detail = request_detail.clone();
    let read_host_state = move || {
        let base = host_base.clone();
        let host = current.peek().host;
        // Re-checked at every invocation, not only at the trigger, because
        // the reader calls this again on its own: a retry ladder started
        // while the session was stale would otherwise go on probing
        // `/api/hosts` long after the host came back, asking a question
        // nobody has any use for. Reported as ANSWERED so the reader retires
        // rather than treating the skip as another failure.
        let wanted = current.peek().stale;
        // The staleness EPOCH this read belongs to. Two transitions can
        // happen while one read is in flight (stale, back, stale again), and
        // a reply that crosses one is not merely old — it describes a
        // different situation. Committing it would explain a fresh session
        // with a phase from before the host recovered, and would fire the
        // reconnect follow-up below on the strength of it.
        let epoch = stale_epoch.peek().to_owned();
        let generation = host_reads.write().start();
        let follow_up = follow_up_detail.clone();
        async move {
            if !wanted {
                return true;
            }
            // Recorded through the four-state model, so a dropped request
            // becomes a REPORTED refresh failure rather than either a silent
            // hold — which would keep presenting a phase that may have
            // changed since — or a blank, which would lose the only
            // explanation on screen.
            let outcome = fetch_hosts(&base).await;
            let answered = outcome.is_ok();
            // Successes and failures are gated differently — see
            // `ops::ReadGate`: an older success describes a registry that
            // has since moved on, while a failure newer than what is on
            // screen is worth reporting even though a later read has already
            // started.
            let accepted = match &outcome {
                Ok(_) => host_reads.write().accept_success(generation),
                Err(_) => host_reads.peek().accept_failure(generation),
            };
            // The epoch is checked at COMMIT as well as at dispatch: a read
            // launched while the session was stale can land after it has gone
            // fresh, and the effect that cleared `host_read` on that
            // transition would then be undone by a reply describing the
            // outage that just ended. Still counted as answered — the helm
            // did reply, it simply answers a question this view has stopped
            // asking.
            if !accepted || *stale_epoch.peek() != epoch {
                return answered;
            }
            host_read.write().record(outcome);
            // The host is back while this session still reads stale: two
            // reads disagreeing, not a state to explain. The notice says
            // exactly that (`stale_session_notice`), and this closes the gap
            // rather than waiting for the detail read the feed will bring —
            // the terminal cannot mount until the session row itself stops
            // saying stale, so any wait here is a wait with a usable session
            // showing no terminal.
            //
            // Asked for through the detail door rather than fetched here and
            // written straight into `current`, which is what this used to do.
            // A direct write skips the restart epoch AND the generation gate,
            // so a follow-up racing a restart could put the previous run back
            // on screen, and one racing an ordinary re-read could roll the
            // view backwards with nothing left to correct it.
            let reconnected = matches!(
                host_read.peek().lookup(host),
                HostLookup::Known(host) if is_connected(&host.state)
            );
            if reconnected {
                // News: the registry just said the host is back, and the
                // session row has to catch up before a terminal can mount.
                follow_up(Trigger::Notice);
            }
            answered
        }
    };
    let host_surface = use_signal(SurfaceReader::default);
    let request_host_state =
        move |trigger: Trigger| request_read(host_surface, trigger, read_host_state.clone());

    // The page's first look, for the same reason `ListView` keeps one: a
    // view has to draw something before anything has changed. The
    // host-state read has no equivalent here because its trigger below
    // already fires at mount for a session that arrives stale.
    // Explicit for `ListView`'s reason: opening a session is a person
    // navigating, and a view that read nothing under a latched build
    // mismatch would show an empty shell rather than the metadata the skew
    // notice sits above.
    let mount_detail = request_detail.clone();
    use_hook(move || mount_detail(Trigger::Explicit));

    // Whether the session currently READS stale, as a memo rather than a
    // plain read, and that distinction is what keeps the effect below from
    // running on every detail commit: a memo marks dirty only when its VALUE
    // changes, so the host-state read is triggered by the staleness
    // TRANSITION rather than by each reply that reports the same flag again.
    let stale = use_memo(move || current.read().stale);

    // Staleness is discovered by the detail read, so the host-state read
    // follows the flag rather than waiting for the next revision: a session
    // that has just gone stale must name its host's state on the frames that
    // follow, not whenever the fleet next happens to change. At mount this
    // is also what gives a session opened from a stale row its notice
    // immediately.
    let stale_host_read = request_host_state.clone();
    use_effect(move || {
        // Bumped on EVERY transition, in both directions, before anything
        // else happens: it is what tells a host read in flight that the
        // situation it was launched for is over (see `read_host_state`).
        stale_epoch += 1;
        if stale() {
            // Notice, not explicit: the staleness flag changed because a
            // READ said so, not because anyone clicked. It is news — the one
            // moment this view needs the registry at all — but nobody is
            // waiting on it, so a page that has stood down under skew leaves
            // the notice's generic wording rather than fetching a registry
            // it has been told to stop reading.
            stale_host_read(Trigger::Notice);
        } else if *host_read.peek() != HostsRead::default() {
            // The host came back. Dropping the record is what keeps a LATER
            // staleness from being explained by a reading taken before the
            // host recovered.
            host_read.set(HostsRead::default());
        }
    });

    // The feed's consumer (PLAN_M6_75.md item 6). A revision notification
    // says only that something changed, so this view re-reads what it shows:
    // its own session always, and the host registry only while the session
    // is stale — which is exactly when a host state change is the thing the
    // user is waiting on.
    //
    // Marked, not awaited, and marked through the readers: a notification
    // that lands while a read is still walking becomes that read's single
    // follow-up rather than a second concurrent walk.
    let feed_detail = request_detail.clone();
    let feed_host_read = request_host_state.clone();
    use_feed_reader(move || {
        feed_detail(Trigger::Notice);
        if current.peek().stale {
            feed_host_read(Trigger::Notice);
        }
    });

    // The documented fallback (PLAN_M6_75.md item 6): the timer runs
    // unconditionally and the READS are what is gated, so the mechanism that
    // covers a failed feed is never itself being started at the moment the
    // feed fails. It reads only while `feed::fallback_polls` says to — never
    // on a healthy feed, and never under build skew, where this view stands
    // down with the rest of the page rather than polling a helm whose
    // vocabulary it does not share.
    let fallback_detail = request_detail.clone();
    let fallback_host_read = request_host_state;
    use_future(move || {
        let detail = fallback_detail.clone();
        let host = fallback_host_read.clone();
        async move {
            loop {
                fallback_sleep().await;
                if fallback_polls_now() {
                    detail(Trigger::Scheduled);
                    if current.peek().stale {
                        host(Trigger::Scheduled);
                    }
                }
            }
        }
    });

    let restart_base = base.clone();
    // The detail door, for the refresh a restart owes (see below).
    let refresh_after_restart = request_detail.clone();
    let restart = move |stop_if_running: bool| {
        if restarting() {
            return;
        }
        restarting.set(true);
        restart_error.set(None);
        restart_epoch += 1;
        let base = restart_base.clone();
        let id = current.read().id.clone();
        let mode = restart_mode_for(current.read().restart_offer);
        // Cloned per click: the spawned task owns what it captures, and this
        // closure runs again for the next restart.
        let refresh_after_restart = refresh_after_restart.clone();
        spawn(async move {
            let outcome = restart_session(&base, &id, mode, stop_if_running).await;
            if let Err(e) = &outcome {
                restart_error.set(Some(e.clone()));
            }
            // Remounted on both paths. A success obviously needs it (new
            // pane, or a respawned one, and the server tore the old
            // attachment down). A FAILURE needs it just as much: the
            // server detaches before anything can fail, so a view that
            // only remounted on success would leave the user staring at a
            // permanently detached terminal for a session that is running
            // perfectly well. Remounting when nothing changed is merely a
            // reattach — the same thing a reload does.
            mount_generation += 1;
            // A SECOND bump, closing the other half of the staleness this
            // counter exists for. The first bump (before the request)
            // invalidates reads that were already in flight; without this
            // one, a read that STARTED during the restart shares the new
            // epoch, passes its own guard, and can land its mid-restart
            // answer on top of the refresh below — putting the view back to
            // describing the run that just ended. Bumping again means only
            // reads launched after the restart finished are allowed to
            // commit, which is exactly the set that can have seen the
            // result.
            restart_epoch += 1;
            // Released before the refresh rather than after it, deliberately:
            // the refresh is now a reader's business and a failing helm can
            // keep it retrying for minutes, so holding the control until it
            // lands would freeze the affordance on exactly the helm where a
            // user most wants to try again. What that costs is a second
            // restart acting on a `current` that predates the refresh — and
            // that is the safe direction, because the status it still holds
            // is the pre-restart one, which confirms.
            restarting.set(false);
            // The authoritative refresh, asked for through the SAME door
            // every other read uses and issued after the final bump so it
            // carries the epoch it will be judged against.
            //
            // It is needed on both paths. After a success the reply's
            // `status` is a deliberate `Unknown` (the supervisor cannot
            // claim the agent execed yet); after a failure the refusal is
            // most often a STALE OFFER, whose prescribed handling is to
            // re-present the offer the session has NOW — and a failure is
            // not even proof the restart did not happen, since the reply can
            // be lost after the relaunch succeeded.
            //
            // Direct, this used to be: fetch here and write `current`
            // straight through. That bypassed both guards at once. A slow
            // direct reply could land on top of newer feed-driven state with
            // nothing left to correct it, and a FAILED one left the view
            // describing the run that just ended forever — the notice that
            // would have re-read was already consumed by a read the epoch
            // bump discarded, which reported itself answered. Through the
            // door, the reader retries until an answer lands and every reply
            // is ordered against every other.
            //
            // What is deliberately NOT carried over is the old 404 branch's
            // "session no longer exists" claim: `fetch_session`'s docs are
            // explicit that a 404 on the listing-backed detail route cannot
            // be read as deletion, and the guarded commit says the honest
            // thing instead (the staleness notice).
            refresh_after_restart(Trigger::Explicit);
        });
    };
    // One closure, two call sites (the confirm button and the direct
    // restart), cloned rather than duplicated so both send exactly the
    // same request shape and share the same in-flight guard.
    let mut confirm_restart = restart.clone();
    let mut fresh_restart = restart;

    // The rename field's submit (PLAN_M5.md item 6). What the user typed
    // goes to the supervisor verbatim (`api::rename_session`); what is
    // decided here is only what to do with the answer.
    //
    // A success takes the reply's TITLE and nothing else, as the
    // optimistic correction the next detail read then settles. Taking the
    // whole `SessionInfo` would be the obvious move and is wrong: the reply
    // is a snapshot from when the rename was handled, so a slow rename
    // landing after a fresher read would roll `status` and `tabs` BACKWARDS
    // until the next one — a rename silently reverting a status change is
    // exactly the staleness `restart_epoch` exists to prevent elsewhere.
    // The dynamic fields belong to the detail read; a rename only ever
    // knows about a label.
    //
    // A refusal leaves the field open with what the user typed still in it
    // and puts the supervisor's own words on screen, while the title
    // everywhere stays what it was; a control-character refusal is the
    // case SPEC.md names, and its message is the contract.
    let rename_base = base.clone();
    let on_rename_submit = move |title: String| {
        if renaming_pending() {
            return;
        }
        renaming_pending.set(true);
        rename_error.set(None);
        let base = rename_base.clone();
        let id = current.read().id.clone();
        spawn(async move {
            match rename_session(&base, &id, &title).await {
                Ok(session) => {
                    // Read AFTER the reply, never before the request: the
                    // index names the first read GUARANTEED to have started
                    // after this response completed. A read launched while
                    // the POST was still in flight MAY also carry the new
                    // title — the write lands before the reply is read — so
                    // this is a conservative bound, and conservative in the
                    // only safe direction (it can hold the correction a
                    // little longer, never retire it on a reply that could
                    // not have seen the rename).
                    let observed_from = poll_sequence.peek().to_owned();
                    renamed.set(Some((session.title, observed_from)));
                    renaming.set(false);
                }
                Err(e) => rename_error.set(Some(e)),
            }
            renaming_pending.set(false);
        });
    };

    // The add-tab control. Unlike `ListView`'s create, navigating away
    // while this is in flight is deliberately NOT locked out: a stranded
    // create can cost the user a duplicate AGENT they never see, whereas a
    // stranded open just means a tab that exists and is listed the next
    // time this session is opened — nothing is lost and nothing is
    // duplicated. The in-flight flag here only guards re-entry and
    // disables the button.
    let add_base = base.clone();
    let add_session_id = session.id.clone();
    let on_add_tab = move |_| {
        // Signal-level re-entry check, not just the `disabled` attribute:
        // the attribute's DOM update from a rerender is not synchronous
        // with a click event, so a second click landing in that gap would
        // still reach this handler.
        if opening_tab() {
            return;
        }
        opening_tab.set(true);
        // Cleared at the START of this operation and keyed to it alone, so
        // a retry drops its own stale message without touching a close
        // failure the user has not read yet.
        tab_errors.write().remove(TAB_OPEN_ERROR_KEY);
        let base = add_base.clone();
        let session_id = add_session_id.clone();
        spawn(async move {
            match open_tab(&base, &session_id).await {
                Ok(tab) => {
                    // Rendered immediately at the END of the strip and
                    // selected, so the user lands in the terminal they
                    // just asked for; the next detail read gives it its
                    // real position (see `tabs::visible_tabs`).
                    //
                    // The sequence number is read AFTER the reply, not
                    // before the request: it names the first read that
                    // could possibly have seen this tab, and a read
                    // launched while the POST was still in flight could
                    // not have.
                    let observed_from = poll_sequence.peek().to_owned();
                    opened_tabs.write().push((tab.id.clone(), observed_from));
                    selected.set(Some(tab.id));
                }
                // The supervisor's own words, verbatim: a vanished working
                // directory and a session needing a restart first are both
                // refusals SPEC.md requires to name what is wrong.
                Err(e) => {
                    tab_errors
                        .write()
                        .insert(TAB_OPEN_ERROR_KEY.to_string(), format!("open tab: {e}"));
                }
            }
            opening_tab.set(false);
        });
    };

    // The DELETE itself, reached only through the confirmation below.
    let close_base = base.clone();
    let close_session_id = session.id.clone();
    let mut do_close_tab = move |tab_id: String| {
        // Re-entry guard on the per-tab in-flight set, mirroring
        // `ListView`'s `pending`: `insert` returning `false` means a close
        // for this tab is already running.
        if !closing_tabs.write().insert(tab_id.clone()) {
            return;
        }
        // This tab's own previous failure, cleared at the start of the
        // retry that supersedes it — never anyone else's (see
        // `tab_errors`).
        tab_errors.write().remove(&tab_id);
        let base = close_base.clone();
        let session_id = close_session_id.clone();
        spawn(async move {
            match close_tab(&base, &session_id, &tab_id).await {
                Ok(()) => {
                    // Acting on a response already in hand, not guessing:
                    // the same deliberate optimism `ListView` applies to a
                    // deleted row, and for the same reason — leaving the
                    // tab on screen until the next read would leave a
                    // clickable control that attaches a terminal the
                    // supervisor has already destroyed.
                    closed_tabs.write().insert(tab_id.clone());
                    // Retired from the OPTIMISTIC list too, not just
                    // suppressed by `closed_tabs`. A tab opened and closed
                    // before any read observed it would otherwise sit in
                    // `opened_tabs` forever — `closed_tabs` is itself
                    // pruned once the server stops listing the id, and for
                    // a tab the server never listed that is immediately,
                    // at which point the stale optimistic entry would
                    // resurrect a tab that does not exist.
                    opened_tabs.write().retain(|(id, _)| id != &tab_id);
                    if selected.read().as_deref() == Some(tab_id.as_str()) {
                        selected.set(None);
                    }
                }
                Err(e) => {
                    tab_errors
                        .write()
                        .insert(tab_id.clone(), format!("close tab: {e}"));
                }
            }
            closing_tabs.write().remove(&tab_id);
        });
    };

    // The close affordance's first click: opens the in-page confirmation
    // and never calls the API. In-page for the reason every confirmation
    // in this UI is (wry ships no native JS dialogs on macOS's WKWebView,
    // where a `window.confirm()` silently does nothing), and confirmed
    // unconditionally rather than only for a busy shell: the UI cannot see
    // what a tab's shell is running, and close kills it either way.
    //
    // Refuses a tab whose close is already in flight, the cross-guard
    // mirroring `ListView`'s: the button is only disabled once a rerender
    // following the `closing_tabs` insert lands, so a click queued just
    // ahead of that could otherwise open a prompt for an operation already
    // under way.
    let on_close_tab = move |tab_id: String| {
        if closing_tabs.read().contains(&tab_id) {
            return;
        }
        confirming_close.set(Some(tab_id));
    };

    // The confirm button. Proceeds ONLY when this tab is still the one
    // being confirmed — `Some(id)` matching — which is what stops a
    // confirm click queued behind a cancel (both fired in the same burst)
    // from closing a tab the user just told the UI to leave alone.
    let mut confirm_close_tab = move |tab_id: String| {
        if confirming_close.read().as_deref() != Some(tab_id.as_str()) {
            return;
        }
        confirming_close.set(None);
        do_close_tab(tab_id);
    };

    // What this view currently wants mounted, as the JSON array
    // `farhelmTerm.sync()` takes — `None` until the lease exists, which is
    // what keeps any terminal from attaching un-leased.
    //
    // A memo rather than computing this inside the effect: the detail read
    // above writes `current` on every reply whether or not the session
    // changed,
    // and each write marks its subscribers dirty. A memo re-COMPUTES just
    // as often, but only notifies when the resulting value differs, so the
    // effect — and with it an eval round trip into the page — runs on real
    // changes only. Correctness does not depend on that (`sync()` is
    // idempotent), but a `document::eval` every 3 seconds forever would be
    // pure waste.
    let spec_session_id = session.id.clone();
    let terminal_specs = use_memo(move || {
        let lease = lease.read().clone()?;
        let session = current.read();
        // A stale session has no terminal to attach (see this component's
        // docs), so the desired set is EMPTY rather than absent — an empty
        // sync is what tears down islands belonging to a host that went away
        // while this view was open, which returning `None` (the
        // lease-not-yet-minted case) would leave mounted and silently dead.
        if session.stale {
            return Some("[]".to_string());
        }
        let tabs = visible_tabs(&session.tabs, &opened_tabs.read(), &closed_tabs.read());
        let active = selected.read().clone().filter(|id| tabs.contains(id));
        // Only the tabs within the island cap are ever handed to
        // terminal.js — see `tabs::MAX_MOUNTED_TAB_ISLANDS`. The strip
        // still lists the rest; their panes explain themselves instead.
        let mountable = tabs.iter().take(MAX_MOUNTED_TAB_ISLANDS);
        // Every value here goes through serde_json rather than string
        // interpolation: the session and tab ids come from a supervisor,
        // which with --ssh is a different machine. A hostile or compromised
        // host returning an id containing a quote would otherwise get
        // arbitrary JavaScript running on the helm's origin — turning a
        // remote-host compromise into control of the local helm API.
        let mut specs = vec![serde_json::json!({
            "el": AGENT_TERMINAL_ELEMENT_ID,
            "banner": AGENT_BANNER_ELEMENT_ID,
            // Where this terminal's connecting placeholder is written
            // while its attach catches up (PLAN_M5.md item 5). Per island
            // like the two above, because each terminal has its own socket
            // and therefore its own catch-up phase.
            "connecting": AGENT_CONNECTING_ELEMENT_ID,
            // Where this terminal's own upload indicator and upload
            // failures are written (PLAN_M4.md item 7). Per island, like
            // the banner and for the same reason: a transfer belongs to
            // the terminal that received the paste, and reporting it
            // anywhere else would leave the user reading about a file
            // they dropped somewhere they cannot see.
            "status": attachment_status_element_id(AGENT_TERMINAL_ELEMENT_ID),
            "path": terminal_ws_path(&spec_session_id, None, &lease),
            // Where an UNATTENDED reconnect attaches instead (PLAN_M6.md
            // item 7): a route only a helm that honors the non-displacing
            // contract serves, so the request cannot be silently
            // reinterpreted by one that does not.
            "pathUnowned": terminal_ws_unowned_path(&spec_session_id, None, &lease),
            // Only the agent terminal carries the restart generation: a
            // restart detaches the agent's attachment alone (the
            // supervisor's `detach_for_restart`), so bumping a tab's would
            // tear down and replay a terminal the restart never touched.
            "gen": mount_generation(),
            // The agent terminal owns terminal.js's legacy singleton test
            // globals — see that file's `mount()`.
            "primary": true,
            "focus": active.is_none(),
        })];
        for id in mountable {
            specs.push(serde_json::json!({
                "el": tab_terminal_element_id(id),
                "banner": tab_banner_element_id(id),
                "connecting": tab_connecting_element_id(id),
                "status": attachment_status_element_id(&tab_terminal_element_id(id)),
                "path": terminal_ws_path(&spec_session_id, Some(id), &lease),
                "pathUnowned": terminal_ws_unowned_path(&spec_session_id, Some(id), &lease),
                "gen": 0,
                "primary": false,
                "focus": active.as_deref() == Some(id.as_str()),
            }));
        }
        Some(serde_json::Value::Array(specs).to_string())
    });

    // The paste/drop rules this view's terminals run on (PLAN_M4.md item
    // 7), as one object for the whole view rather than one per island —
    // see `attachment_policy`.
    //
    // Memoized rather than rebuilt in the component body: it depends on
    // the session id alone, so no read can ever change it, and the body
    // re-runs on every reply the feed brings. Without the memo this would
    // serialize the same ~1KB of JSON forever, for nothing.
    let policy_session_id = session.id.clone();
    let attach_policy = use_memo(move || attachment_policy(&policy_session_id).to_string());
    // The auto-reconnect policy (PLAN_M6.md item 7), handed across the same
    // boundary and for the same reason as the one above: the ladder, the
    // phase boundary, and every sentence a recovering terminal shows are
    // decisions, reviewable in Rust and unit-tested there, while the socket
    // handling that applies them can only live in terminal.js.
    //
    // Depends on ONE thing: whether the helm answering this page is the
    // build this bundle was made for (`skew::helm_is_current`), which is
    // what decides whether either UNATTENDED behavior — the heartbeat and
    // automatic reconnect attempts — may run at all. See
    // `reconnect_policy`.
    //
    // Reactive rather than sampled, and here that is load-bearing rather
    // than tidy: a helm upgraded (or rolled back) under an open tab latches
    // a mismatch on the very next reply, and this memo is what carries that
    // revocation into terminals that are ALREADY open — terminal.js reads
    // the synced policy at every decision, so a pending attempt is
    // downgraded to the manual-only surface rather than firing against a
    // helm that cannot honor it.
    let reconnect_policy =
        use_memo(move || reconnect_policy(crate::skew::helm_is_current()).to_string());
    use_effect(move || {
        // Nothing attaches until the lease is minted; `lease_error` is
        // what the user sees if it never is.
        let Some(specs) = terminal_specs() else {
            return;
        };
        let attach_policy = attach_policy();
        let reconnect_policy = reconnect_policy();
        let base_js = serde_json::to_string(&base).expect("string is serializable");
        let js = format!(
            r#"(function() {{
                var gen = (window.__farhelmSyncGeneration || 0) + 1;
                window.__farhelmSyncGeneration = gen;
                (function waitForIsland() {{
                    if (window.__farhelmSyncGeneration !== gen) return;
                    if (window.farhelmTerm) {{
                        farhelmTerm.sync(
                            {base_js}, {specs}, {attach_policy}, {reconnect_policy}
                        );
                    }} else {{
                        setTimeout(waitForIsland, 50);
                    }}
                }})();
            }})();"#
        );
        document::eval(&js);
    });

    use_drop(|| {
        // Bump the generation FIRST so an in-flight outer wait (see the
        // component docs above) aborts before `unmountAll()` below can
        // race it; `unmountAll()` itself is what cancels each island's own
        // pending `mountWhenReady` wait, once the outer one has already
        // handed off to it. Fire-and-forget: this runs on the way out
        // (navigating back to the list, or the whole app tearing down), so
        // there is no reactive state left to update with a result, and the
        // JS side is already written to be a no-op if nothing is mounted.
        document::eval(
            "window.__farhelmSyncGeneration = (window.__farhelmSyncGeneration || 0) + 1; \
             if (window.farhelmTerm) { farhelmTerm.unmountAll(); }",
        );
    });

    let shown = current.read().clone();
    let alive = shown.status.is_live();
    let tabs = visible_tabs(&shown.tabs, &opened_tabs.read(), &closed_tabs.read());
    // Both of these are DERIVED rather than written back to their signals
    // when they go stale, and that is safe precisely because tab ids are
    // never reused (farhelm-proto's `TabInfo::id`): an id that has left the
    // list can never come back, so a signal still holding it can only ever
    // resolve to "no such tab" and will be overwritten by the user's next
    // click. Writing during render would be the alternative, and Dioxus
    // makes that a re-render loop rather than a fix.
    let active_tab = selected.read().clone().filter(|id| tabs.contains(id));
    let confirming_tab = confirming_close
        .read()
        .clone()
        .filter(|id| tabs.contains(id));
    // The prompt names the tab by the SAME positional label the strip
    // shows, recomputed from the list as it is right now — so a tab that
    // shifts position under an open prompt (a lower-numbered sibling
    // closed from another client) is still identified correctly.
    let confirming_label = confirming_tab
        .as_ref()
        .and_then(|id| tabs.iter().position(|candidate| candidate == id))
        .map(tab_label);
    // The title as this view should show it: the server's, unless a rename
    // this view performed has not been settled by a detail read yet (see
    // `renamed`). Derived per render rather than written back into
    // `current`, so the correction can only ever be as long-lived as the
    // next read takes to confirm it.
    let shown_title = renamed
        .read()
        .as_ref()
        .map(|(title, _)| title.clone())
        .unwrap_or_else(|| shown.title.clone());
    // Built from the row's own `host_name` rather than from the host record,
    // so the notice can name the host even before the hosts read lands —
    // and so it still names it if the host has since been removed from the
    // registry entirely.
    let host_read_now = host_read.read();
    let stale_notice = shown.stale.then(|| {
        stale_session_notice(
            shown.host_name.as_deref().unwrap_or_default(),
            host_read_now.lookup(shown.host),
        )
    });
    // SPEC.md's metadata triple for a stale session is "title, directory,
    // last-known status", and the status is the one this view has nowhere
    // else to put: the titlebar carries the title and the directory, while
    // the restart offer describes what a relaunch would do rather than what
    // the session last WAS. Rendered through the list's own badge so the two
    // surfaces cannot describe one session differently.
    // Flattened, not nested: a stale session whose last-known status was
    // never classified has no badge to show (see `status_badge`'s
    // no-badge-for-`Unknown` rule), which is the same absence as "not
    // stale" as far as this render is concerned.
    let stale_badge = shown
        .stale
        .then(|| status_badge(&shown.status, shown.annotation.as_deref()))
        .flatten();
    rsx! {
        div { class: "layout",
            header { class: "titlebar",
                button {
                    class: "btn back-button",
                    // Refused while a rename is in flight, the same
                    // ownership rule `ListView`'s `nav_locked` enforces for
                    // its own in-flight operations: leaving unmounts this
                    // component, and with it the task that would have shown
                    // the supervisor's refusal — so a rename could fail
                    // with nothing said about it anywhere, which SPEC.md's
                    // visible-failure rule does not allow.
                    disabled: renaming_pending(),
                    onclick: move |_| {
                        if renaming_pending() {
                            return;
                        }
                        on_back.call(());
                    },
                    "← back",
                }
                // SPEC.md puts rename on both surfaces, and here it sits
                // where the title already is. The title STAYS while the
                // field is open rather than being replaced by it, and that
                // is a requirement rather than a layout preference: a
                // refused rename must leave the old title on screen (the
                // draft in the field is precisely what the supervisor
                // rejected), so replacing it would make the only visible
                // name the one that does not exist.
                span { class: "title", "{shown_title}" }
                if renaming() {
                    RenameForm {
                        draft: rename_draft,
                        busy: renaming_pending(),
                        on_submit: on_rename_submit,
                        on_cancel: move |_| {
                            renaming.set(false);
                            // The refusal goes with the field that
                            // produced it: leaving it on screen after the
                            // user backed out would describe an edit that
                            // no longer exists.
                            rename_error.set(None);
                        },
                    }
                } else {
                    button {
                        r#type: "button",
                        class: "btn session-rename",
                        onclick: move |_| {
                            // Seeded from the title on screen right now, so
                            // reopening the field starts from what the
                            // session is currently called; the draft
                            // deliberately outlives a cancel, since the
                            // next open reseeds it anyway.
                            rename_draft.set(shown_title.clone());
                            // Cleared on OPEN as well as on cancel: a
                            // refusal survives until something supersedes
                            // it, and reopening the field to try again is
                            // exactly that — leaving the old message over
                            // a fresh attempt would make it look like the
                            // new one had already failed.
                            rename_error.set(None);
                            renaming.set(true);
                        },
                        "rename"
                    }
                }
                span { class: "meta", "{shown.cwd} — {shown.invocation}" }
            }
            // The supervisor's own words on its own row under the header,
            // for the reason `.restart-error` sits below its controls: a
            // refusal names what is wrong with the title, and squeezing it
            // into the titlebar's one non-wrapping line would truncate the
            // sentence that says what to fix.
            if let Some(err) = rename_error.read().clone() {
                div { class: "rename-error", "{err}" }
            }
            // SPEC.md: "Opening an interrupted session offers
            // restart-with-resume" — which is why this leads the view
            // rather than hiding behind a menu, and why its wording states
            // what restarting would do to the CONVERSATION rather than
            // just naming the action. Declining is simply not clicking it:
            // there is no dismiss, and nothing about the session changes.
            // The same affordance serves every other state too, since
            // restart is the one relaunch mechanism there is.
            div { class: "restart-offer",
                span { class: "restart-offer-text",
                    "{restart_offer_text(&shown.status, shown.restart_offer)}"
                }
                if confirming() {
                    // The inline confirm delete already uses, for the same
                    // reason (SPEC.md: restart on a running agent confirms,
                    // stops, then relaunches) and with the same safety
                    // defaults — consequence text first, focus on cancel.
                    span { class: "confirm-consequence",
                        "still running — restarting stops the agent and its whole process tree first:"
                    }
                    button {
                        r#type: "button",
                        class: "btn restart-confirm",
                        disabled: restarting(),
                        onclick: move |_| {
                            if !confirming() {
                                return;
                            }
                            confirming.set(false);
                            // The only place `stop_if_running` is ever
                            // true: it carries THIS click's consent onto
                            // the wire, which the supervisor then checks
                            // against liveness it rechecks itself.
                            confirm_restart(true);
                        },
                        "confirm restart"
                    }
                    button {
                        r#type: "button",
                        class: "btn restart-cancel",
                        autofocus: true,
                        onclick: move |_| confirming.set(false),
                        "cancel"
                    }
                } else {
                    button {
                        r#type: "button",
                        class: "btn restart-primary",
                        // Whether this click opens the confirmation rather
                        // than restarting outright — the status-derived
                        // decision, exposed so it is inspectable (the
                        // browser suite waits on it) instead of only
                        // observable after the fact by clicking.
                        "data-confirms": "{alive}",
                        disabled: restarting(),
                        onclick: move |_| {
                            if restarting() {
                                return;
                            }
                            if alive {
                                // Never a direct request for a live agent:
                                // restarting one kills it, so the click
                                // only opens the confirmation.
                                confirming.set(true);
                            } else {
                                fresh_restart(false);
                            }
                        },
                        "{restart_button_label(shown.restart_offer)}"
                    }
                }
                if let Some(err) = restart_error.read().clone() {
                    div { class: "restart-error", "{err}" }
                }
            }
            // Worded to state the FACT (the helm stopped listing this
            // session) and the CONSEQUENCE (what is shown may be stale),
            // then BOTH readings — never a pick between them, because this
            // view genuinely cannot tell them apart (see `fetch_session`).
            //
            // An earlier wording ended "its terminals are unaffected",
            // which is a claim this line has no standing to make: under
            // the cap reading it is true, but a session deleted from
            // another client has had its terminals killed, and they will
            // say so in their own banners a moment later. Naming the two
            // possibilities and stopping is the most this can honestly do.
            //
            // Distinct from the host-staleness notice below, and both can
            // legitimately show at once: this one is about the LIST no
            // longer carrying this session, that one about its HOST being
            // gone. Collapsing them would attribute one cause to the other.
            if refresh_stale() {
                div { class: "refresh-stale",
                    "the helm stopped listing this session, so what is shown here may be out of \
                     date — it was deleted from another client, or there are more sessions than \
                     the helm lists at once"
                }
            }
            // The whole terminal surface is either present or replaced by
            // the host-state notice — never both. SPEC.md gives a stale
            // session metadata behind a notice and NO terminal, and an
            // empty strip beside a dead pane would be exactly the
            // unexplained blankness that rule exists to prevent.
            if let Some(notice) = stale_notice {
                // The notice is rendered as PARTS, not as one interpolated
                // string: every peer-supplied value in it (the host's name,
                // its identities, the transport's own words) gets its own
                // direction-isolated element, so none of them can rearrange
                // the sentence explaining why this session has no terminal.
                PeerLine { class: "host-stale-notice", parts: notice }
                if let Some((badge_class, badge_text)) = stale_badge {
                    div { class: "stale-metadata",
                        span { class: "status-badge {badge_class}", "{badge_text}" }
                    }
                }
            } else {
                // The tab strip: the agent terminal first and unclosable
                // (SPEC.md gives a session one agent terminal, and closing it
                // is not one of the operations that exist), then every open
                // tab in the server's creation order, then the add control.
                div { class: "tab-strip",
                    button {
                        r#type: "button",
                        class: if active_tab.is_none() { "btn tab tab-agent selected" } else { "btn tab tab-agent" },
                        "data-terminal": "agent",
                        onclick: move |_| selected.set(None),
                        "agent"
                    }
                    // `key` is the tab id, not the loop index, and that is
                    // load-bearing rather than a lint: closing a tab shifts
                    // every later sibling's position, and an index key would
                    // make Dioxus reuse each item's DOM for a DIFFERENT tab —
                    // carrying the wrong id into the close handler of a
                    // still-open prompt. The id keys the PANES below for the
                    // same reason, with more at stake: reused pane DOM would
                    // hand one tab's mounted island to another's element.
                    for (index , tab_id) in tabs.iter().enumerate() {
                        TabStripItem {
                            key: "{tab_id}",
                            tab_id: tab_id.clone(),
                            label: tab_label(index),
                            selected: active_tab.as_deref() == Some(tab_id.as_str()),
                            busy: closing_tabs.read().contains(tab_id),
                            on_select: move |id| selected.set(Some(id)),
                            on_close: on_close_tab,
                        }
                    }
                    button {
                        r#type: "button",
                        class: "btn tab-add",
                        disabled: opening_tab(),
                        onclick: on_add_tab,
                        "+ terminal"
                    }
                }
                // The close confirmation, on its own row under the strip
                // rather than in place of the tab's own controls: the strip is
                // a single-line flex row that a prompt would push tabs out of,
                // and unlike a session row there is no per-row action area for
                // it to take over. Consequence first, then the label, then the
                // buttons — the same reading order (and the same untruncatable
                // consequence / truncatable name split) the delete prompt uses;
                // see `status::confirm_consequence`'s docs for why that order is
                // the safety-critical part.
                if let Some(tab_id) = confirming_tab {
                    div { class: "tab-confirm",
                        span { class: "confirm-consequence", "{CLOSE_TAB_CONSEQUENCE}" }
                        span { class: "confirm-title",
                            "{confirming_label.clone().unwrap_or_default()}"
                        }
                        button {
                            r#type: "button",
                            class: "btn confirm-delete confirm-close-tab",
                            onclick: move |_| confirm_close_tab(tab_id.clone()),
                            "confirm close"
                        }
                        button {
                            r#type: "button",
                            class: "btn confirm-cancel",
                            // Safe default, exactly as the delete prompt does
                            // it: keyboard focus lands on the way OUT of the
                            // destructive action, via the plain HTML attribute
                            // rather than a fallible `set_focus` whose
                            // discarded `Result` could silently drop the
                            // safety behavior.
                            autofocus: true,
                            onclick: move |_| confirming_close.set(None),
                            "cancel"
                        }
                    }
                }
                // One line per failed operation, each keyed to the operation
                // that produced it (see `tab_errors`), so an unrelated success
                // never clears a message the user has not read. Sorted so the
                // lines do not reshuffle on every render — a `HashMap` has no
                // order of its own, and rows jumping around under the strip
                // would make a second failure look like a replaced one.
                for (key , err) in sorted_tab_errors(&tab_errors.read()) {
                    div { key: "{key}", class: "tab-error", "data-tab-error": "{key}", "{err}" }
                }
                // Fatal to the terminals, not to the view: the metadata, the
                // restart affordance, and the tab strip all still work, and
                // saying so beats rendering empty panes with no explanation.
                if let Some(err) = lease_error.read().clone() {
                    div { class: "lease-error", "{err}" }
                }
                // Every terminal is mounted at once and stacked; only the
                // selected one is visible. The agent's pane is a fixed node
                // rather than part of the loop below so that Dioxus never
                // recreates its DOM (and with it `#term-banner`, which
                // long-lived observers in the browser suite hold a reference
                // to) just because the tab list changed around it.
                div { class: "terminal-panes",
                    div {
                        class: if active_tab.is_none() { "terminal-pane selected" } else { "terminal-pane" },
                        "data-terminal": "agent",
                        div { id: "{AGENT_BANNER_ELEMENT_ID}", class: "banner" }
                        div { id: "{AGENT_TERMINAL_ELEMENT_ID}", class: "terminal" }
                        // Empty, hidden, and owned by terminal.js exactly like
                        // the banner beside it: the reveal it performs happens
                        // inside a `term.write()` completion callback
                        // (PLAN_M5.md item 5), which is not a moment the
                        // reactive layer can be driven from — and routing it
                        // through the eval channel would put the one surface a
                        // catching-up terminal shows on the channel that is
                        // dead on wry's WKWebView (MT-5).
                        //
                        // Overlaid rather than in flow, for the reason the
                        // attachment status line below documents: an in-flow
                        // element would take height from the terminal, and the
                        // per-island ResizeObserver would answer with a real
                        // pty resize — so every single attach would reflow the
                        // pane twice.
                        div {
                            id: "{AGENT_CONNECTING_ELEMENT_ID}",
                            class: "terminal-connecting",
                        }
                        // Empty, hidden, and never touched by Dioxus again —
                        // terminal.js owns its content for the same reason it
                        // owns the banner's: an upload's progress and failures
                        // are events on the island's own DOM path, not
                        // reactive state, and routing them back through the
                        // eval channel would put the one message a failed
                        // attachment gets on the channel that is dead on wry's
                        // WKWebView (MT-5).
                        //
                        // Last in the pane, because it OVERLAYS the terminal
                        // (app.css explains why it must not take layout space)
                        // and paint order is what puts it on top.
                        div {
                            id: "{attachment_status_element_id(AGENT_TERMINAL_ELEMENT_ID)}",
                            class: "attach-status",
                        }
                    }
                    for (index , tab_id) in tabs.iter().enumerate() {
                        div {
                            key: "{tab_id}",
                            class: if active_tab.as_deref() == Some(tab_id.as_str()) { "terminal-pane selected" } else { "terminal-pane" },
                            "data-terminal": "{tab_id}",
                            // Past the island cap this view renders an
                            // explanation instead of a mount point, and
                            // deliberately renders no terminal DIV at all: an
                            // element with the island's id but no island
                            // behind it is exactly what a later `sync()` would
                            // mount into.
                            if index < MAX_MOUNTED_TAB_ISLANDS {
                                div { id: "{tab_banner_element_id(tab_id)}", class: "banner" }
                                div { id: "{tab_terminal_element_id(tab_id)}", class: "terminal" }
                                // This tab's own catch-up surface — see the
                                // agent pane's for why it is overlaid and why
                                // terminal.js owns its content.
                                div {
                                    id: "{tab_connecting_element_id(tab_id)}",
                                    class: "terminal-connecting",
                                }
                                // Last, and overlaid — see the agent pane's
                                // own status element above.
                                div {
                                    id: "{attachment_status_element_id(&tab_terminal_element_id(tab_id))}",
                                    class: "attach-status",
                                }
                            } else {
                                div { class: "terminal-not-mounted",
                                    "this session reports more than {MAX_MOUNTED_TAB_ISLANDS} terminal tabs; \
                                     this one is listed but not attached (close some to attach it)"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// What restarting this session would do to its conversation, in the
/// user's own terms — SPEC.md's "restart says so and offers the fallback
/// or a fresh launch — it must never silently resume the wrong
/// conversation", which is a promise about what the user is TOLD, not only
/// about what runs.
///
/// The status leads for `Interrupted` because that is the one state where
/// the user needs to know why their terminal is gone before they are asked
/// to act (SPEC.md: opening an interrupted session offers
/// restart-with-resume). Everything else states the offer alone.
fn restart_offer_text(status: &SessionStatus, offer: RestartOffer) -> String {
    let offered = match offer {
        RestartOffer::Resume => "restarting resumes this session's own conversation",
        RestartOffer::FallbackTemplate => {
            "no conversation was captured, so restarting runs this session's configured resume \
             command"
        }
        RestartOffer::FreshOnly => {
            "no conversation was captured for this session, so restarting launches a fresh agent \
             in the same directory"
        }
    };
    match status {
        SessionStatus::Interrupted => {
            format!("interrupted by a host reboot — {offered}.")
        }
        SessionStatus::Error { .. } => format!("the agent never started — {offered}."),
        _ => format!("{offered}."),
    }
}

/// The restart control's label, which names the OFFER rather than the
/// action: "restart" alone would leave a user guessing whether their
/// conversation survives, which is the exact question SPEC.md requires an
/// honest answer to before they click.
fn restart_button_label(offer: RestartOffer) -> &'static str {
    match offer {
        RestartOffer::Resume => "resume conversation",
        RestartOffer::FallbackTemplate => "restart with the configured resume command",
        RestartOffer::FreshOnly => "restart (fresh launch)",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An older detail reply must never overwrite a newer one, and the case
    /// that matters is the one that happens: the NEWER read completing
    /// first.
    ///
    /// Three triggers read this session (mount, feed notice, host-state
    /// follow-up) and they overlap freely, so this is an ordinary
    /// interleaving rather than an exotic one. Nothing would correct it
    /// afterwards either — with the feed healthy the fallback poll is off,
    /// and the next notification only arrives if the fleet changes again —
    /// so an accepted stale reply is a view that describes the past until
    /// the user navigates away.
    #[test]
    fn a_detail_reply_older_than_what_is_shown_is_refused() {
        let mut reads = ReadGate::default();
        let older = reads.start();
        let newer = reads.start();

        assert_eq!(
            admit_detail(&mut reads, newer, 0, 0),
            Admission::Apply,
            "the newer read lands first and is committed"
        );
        assert_eq!(
            admit_detail(&mut reads, older, 0, 0),
            Admission::Superseded,
            "the older read, completing afterwards, describes a moment already superseded"
        );
    }

    /// A 404 goes through the same guards as a session, in both directions.
    ///
    /// The failure this forbids is specific: a 404 is what raises the
    /// staleness notice, and only a later successful commit clears it — so
    /// an older 404 accepted after a newer success would put a permanent
    /// "this may be out of date" over a view that is not. The reverse
    /// ordering has to keep working too, since a 404 that IS the newest word
    /// must still be able to raise the notice.
    ///
    /// That the admission is PAYLOAD-BLIND is the point rather than an
    /// accident of the signature: the bug it fixes was a 404 taking a
    /// different route through the commit path from a session, and any
    /// future outcome the helm learns to send inherits both guards for free.
    #[test]
    fn a_stale_404_cannot_reopen_the_staleness_notice() {
        let mut reads = ReadGate::default();
        let older_404 = reads.start();
        let newer = reads.start();
        assert_eq!(admit_detail(&mut reads, newer, 0, 0), Admission::Apply);
        assert_eq!(
            admit_detail(&mut reads, older_404, 0, 0),
            Admission::Superseded,
            "an older 404 must not be believed over a newer answer"
        );

        let latest_404 = reads.start();
        assert_eq!(
            admit_detail(&mut reads, latest_404, 0, 0),
            Admission::Apply,
            "but the newest word, 404 or not, is the one the view reflects"
        );
    }

    /// A 404 held across a RESTART is discarded, and discarding it leaves the
    /// read unsatisfied.
    ///
    /// The shape is ordinary: a read goes out, the user restarts, the reply
    /// arrives afterwards. It is the newest reply by generation — nothing
    /// else has completed — so ordering alone admits it, and it would raise
    /// the staleness notice over the run that was just launched. The epoch is
    /// what catches it, and it has to be consulted for EVERY answered
    /// outcome: applying it only to `Ok(Some(..))` is exactly how a 404 got
    /// through.
    ///
    /// `Discarded` rather than `Superseded` is the other half. Nothing was
    /// applied and no newer read is guaranteed to be coming, so the caller
    /// must report the read as unanswered — otherwise the notification that
    /// prompted it is spent on nothing and the view keeps the restart's
    /// placeholder until the fleet next changes.
    #[test]
    fn a_404_held_across_a_restart_is_discarded_rather_than_applied() {
        let mut reads = ReadGate::default();
        let held = reads.start();
        assert_eq!(
            admit_detail(&mut reads, held, 0, 1),
            Admission::Discarded,
            "the reply describes the run that the restart replaced"
        );

        // And the read that the restart itself asks for afterwards — same
        // epoch, newer generation — applies normally.
        let after = reads.start();
        assert_eq!(admit_detail(&mut reads, after, 1, 1), Admission::Apply);
    }

    /// The generation is claimed even by a reply the epoch discards, so an
    /// older read cannot slip in behind it.
    ///
    /// Subtle but load-bearing: the discarded reply is still EVIDENCE about
    /// ordering — it came from a request issued later than everything before
    /// it — and letting an older one through afterwards would put a
    /// pre-restart view on screen through the very path the epoch exists to
    /// close.
    #[test]
    fn a_discarded_reply_still_settles_the_ordering() {
        let mut reads = ReadGate::default();
        let older = reads.start();
        let held = reads.start();
        assert_eq!(admit_detail(&mut reads, held, 0, 1), Admission::Discarded);
        assert_eq!(
            admit_detail(&mut reads, older, 0, 1),
            Admission::Superseded,
            "an older reply is refused on ordering, before the epoch is even asked"
        );
    }

    /// SPEC.md requires restart to SAY what it would do to the
    /// conversation — "it must never silently resume the wrong
    /// conversation" is a promise about what the user is told before they
    /// click, not only about what runs. So the three offers must read
    /// differently, and a fresh launch must say so rather than borrowing
    /// resume's wording.
    ///
    /// The interrupted case leads with WHY the terminal is gone, since
    /// that is the state where the user is being asked to act on something
    /// they did not do (SPEC.md: opening an interrupted session offers
    /// restart-with-resume).
    #[test]
    fn the_offer_text_states_what_would_happen_to_the_conversation() {
        let resumable = restart_offer_text(&SessionStatus::Interrupted, RestartOffer::Resume);
        assert!(
            resumable.contains("reboot") && resumable.contains("resumes"),
            "an interrupted, resumable session must say both: {resumable}"
        );

        let fresh = restart_offer_text(&SessionStatus::Interrupted, RestartOffer::FreshOnly);
        assert!(
            fresh.contains("no conversation was captured") && fresh.contains("fresh agent"),
            "a fresh-only restart must say plainly that nothing is resumed: {fresh}"
        );

        let fallback =
            restart_offer_text(&SessionStatus::Interrupted, RestartOffer::FallbackTemplate);
        assert!(
            fallback.contains("configured resume command"),
            "a configured fallback is labeled honestly, not as a plain fresh launch: {fallback}"
        );

        let error = restart_offer_text(
            &SessionStatus::Error {
                detail: "exec_failed".to_string(),
            },
            RestartOffer::FreshOnly,
        );
        assert!(
            error.contains("never started"),
            "an errored session's own reason leads instead: {error}"
        );
    }

    /// The button names the OFFER, not the action: "restart" alone leaves
    /// the user guessing whether their conversation survives, which is the
    /// exact question SPEC.md requires answered before they click.
    #[test]
    fn the_restart_button_label_names_the_offer() {
        assert_eq!(
            restart_button_label(RestartOffer::Resume),
            "resume conversation"
        );
        assert!(
            restart_button_label(RestartOffer::FreshOnly).contains("fresh"),
            "a fresh launch must not be labeled as a resume"
        );
        assert!(
            restart_button_label(RestartOffer::FallbackTemplate).contains("resume command"),
            "a configured fallback is its own thing, distinct from both"
        );
    }
}

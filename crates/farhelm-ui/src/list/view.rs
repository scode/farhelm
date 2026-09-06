//! The session listing and the reads that keep it current.
//!
//! `ListView` owns the fleet-wide state; row rendering and creation stay in
//! child modules so their narrower contracts remain visible.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use dioxus::prelude::*;

use crate::activity::{ACTIVITY_NOW, ActivityStamp};
use crate::api::{
    self, ListSort, Preferences, SessionFilter, SessionListing, archive_session, delete_session,
    fetch_hosts, fetch_session, fetch_sessions, queue_seen_write, rename_session, replace_session,
    stop_session,
};
use crate::app_bar::AppBar;
use crate::archive::confirmation as archive_confirmation;
use crate::feed::{fallback_polls_now, fallback_sleep, use_feed_reader};
use crate::hosts::{HostsPanel, HostsRead};
use crate::menu_panel::{PanelPlacement, measurement_outcome};
use crate::ops::{OpLock, ReadGate};
use crate::profiles::use_catalog_surface;
use crate::provisioning::ProvisioningTraceShape;
use crate::reader::{SurfaceReader, Trigger, request_read, sleep_ms};
use crate::rows::{
    self, absence_is_evidence, apply_optimistic_renames, count_banner, listing_is_complete,
    menu_row_reordered, retire_vanished_renames, settle_optimistic_renames,
};
use crate::{ApiBase, HostId, Session};

use super::create_form::{CreatePrefill, CreateSessionForm, CreateTarget, prefill_from};
use super::row::SessionRow;
use super::shared::{
    DeleteTarget, HostOption, OpenHost, RowState, effective_create_host, host_options,
    session_locality,
};

/// The shared preference (SPEC.md, Session list) as this page holds it:
/// the chosen list order, last user-selected session, and compact-row choice,
/// seeded once from the helm by `PreferencesGate` and provided to this view
/// as context.
///
/// The helm remembers ONE preference for every client — no client keeps
/// its own copy, and per-client persistence (browser storage, a desktop
/// state file) is not wanted. This signal is the page's in-memory view of
/// that row: reads seed the sort control and the auto-select effect on
/// their first run (the gate holds this component off the screen until the
/// row has been read, so there is no frame that shows a fallback and then
/// corrects it), and every user-initiated change writes the signal first
/// and then a one-field patch to the helm. Writing the signal first is what
/// keeps the choice in force in this client when the write fails: SPEC.md
/// makes this persistence best-effort, and a failed write costs the next
/// launch, never the current choice.
///
/// Only USER-initiated selections are remembered (row clicks, creation).
/// The auto-select fallback deliberately never writes: a remembered row
/// that happens to sit beyond a TRUNCATED listing's bound must survive
/// that listing, not be overwritten by whichever row the fallback picked.
///
/// Because the row is the helm's, a "second client" attaches to whatever
/// was most recently selected ANYWHERE, taking it over per the
/// one-attached-client rule — the consequence SPEC.md's Terminal
/// experience section names.
///
/// One signal for all three fields, so a write to one dirties every reader
/// of the struct: a sort change with an empty right pane re-runs the
/// auto-select effect. That effect is idempotent (its `resolving_*`
/// guards), so this is a wasted run rather than a wrong one, and one
/// signal keeps the seed and the three writers trivially consistent.
#[derive(Clone, Copy)]
pub(crate) struct SharedPreferences(pub(crate) Signal<Preferences>);

/// The remembered selection, if any: the bare session id.
///
/// A bare id rather than the `{helm, id}` record the browser once kept in
/// localStorage. That record was keyed by helm identity because origin-
/// scoped storage could outlive a state-directory swap and hand one helm
/// another's session id; a row in this helm's own database cannot describe
/// any other helm's fleet, so the key would guard against nothing.
fn stored_selection(preferences: SharedPreferences) -> Option<String> {
    preferences.0.read().last_selected.clone()
}

/// Record a USER-initiated selection (a row click, a creation): the signal
/// first, so the choice is in force in this client whatever the helm says,
/// then a one-field patch through `api::store_preference`'s serialized
/// queue. The auto-select fallback never calls this (see
/// `SharedPreferences`).
fn remember_selection(base: &str, mut preferences: SharedPreferences, id: &str) {
    preferences.0.write().last_selected = Some(id.to_string());
    api::store_preference(base, api::PreferenceValue::Selected(id.to_string()));
}

/// A stored preference as an order: anything this build does not recognize —
/// absent, junk, a word a later build wrote — is the default.
///
/// Split out from the signal reads so the decision is testable without a
/// component. Falling back rather than failing is the whole contract: a page
/// that refused to list sessions because of a word the helm remembers would
/// be broken by a value the user cannot see and did not knowingly write —
/// the helm refuses to STORE an unknown word, but the row outlives the
/// build that validated it.
fn decoded_sort(raw: Option<&str>) -> ListSort {
    raw.and_then(ListSort::from_key).unwrap_or_default()
}

/// The remembered order as this build understands it (see `decoded_sort`).
fn stored_sort(preferences: SharedPreferences) -> ListSort {
    decoded_sort(preferences.0.read().list_sort.as_deref())
}

/// Whether this client starts with the helm's shared compact-row preference.
///
/// An unset value is the old preference row and therefore means expanded.
/// The route's typed boolean decoding rejects malformed stored API values
/// before they could reach this fallback.
fn stored_compact(preferences: SharedPreferences) -> bool {
    preferences.0.read().compact.unwrap_or(false)
}

/// Record a chosen order: written on CHANGE only (see `apply_sort`), so a
/// client that never touches the control never writes at all.
///
/// The bare wire word goes to the helm, which validates it against the
/// same vocabulary its `?sort=` accepts.
fn remember_sort(base: &str, mut preferences: SharedPreferences, sort: ListSort) {
    preferences.0.write().list_sort = Some(sort.key().to_string());
    api::store_preference(base, api::PreferenceValue::Sort(sort.key().to_string()));
}

/// Keep a compact-mode choice locally while asking the helm to remember it.
///
/// The client changes first because preference persistence is best effort:
/// a failed request must not make the checkbox reverse under the user's
/// cursor. The shared row supplies the value to a later seed; it is not a
/// live broadcast to other already-open clients.
fn remember_compact(base: &str, mut preferences: SharedPreferences, compact: bool) {
    preferences.0.write().compact = Some(compact);
    api::store_preference(base, api::PreferenceValue::Compact(compact));
}

/// The orders the sidebar offers, in the order it offers them, each with the
/// words the user reads.
///
/// The labels are this surface's alone; the helm never sees them. What must
/// not drift is the MEANING — "recently active" is the wire's `activity`,
/// whose stamp is the supervisor's coarse activity observation rather than a
/// liveness signal — so the label deliberately says "active" rather than
/// anything implying "running now" (which is what the status badge answers).
///
/// The default leads the list as PRESENTATION, not as mechanism. Which
/// option a user sees selected is decided by the select's own `value` and by
/// each option's `selected` — the control is fully controlled from `sort`,
/// so nothing would visibly break if the default sat third. What ordering it
/// first buys is that the list reads the way the sidebar behaves out of the
/// box, and that a stray uncontrolled render (an engine ignoring the
/// attribute on mount, a framework losing the binding) degrades to showing
/// the order the rows are actually in rather than to a control that lies.
const SORT_OPTIONS: [(ListSort, &str); 3] = [
    (ListSort::Activity, "recently active"),
    (ListSort::Created, "newest created"),
    (ListSort::Title, "title A–Z"),
];

/// SPEC.md's auto-select fallback — "the newest-created non-archived
/// session" — chosen from the rows one listing actually carries.
///
/// A function of its own because the choice is subtler than it looks and
/// because it is the half of the fallback that can be tested without a
/// browser. It answers only who wins among the rows it is given; whether a
/// cut listing is a sound basis for the question is the auto-select
/// effect's call (see its own comment on why it picks from them anyway).
///
/// Three rules, and each exists because a plainer one is wrong:
///
/// - **By `created_at`, never by position.** The rows arrive in whichever
///   order the client asked for, so the first row is the newest-created one
///   only under `ListSort::Created`. That assumption was true for the whole
///   life of this list until the sort control landed, and it is the reason
///   the UI decodes `created_at` at all.
/// - **Zero means UNKNOWN, not 1970.** A helm that predates the field leaves
///   it defaulted (`Session::created_at`), and treating that as a real
///   timestamp would make every such row lose to any row that has one, and
///   would make an all-old fleet's winner arbitrary. Rows with no stamp are
///   therefore not candidates; if none of them has one, the answer is the
///   first non-archived row in the listing's own order, which is what this
///   fallback did before `created_at` existed here.
/// - **Equal stamps keep the listing's order.** `created_at` has one-second
///   granularity, so ties are ordinary rather than exotic. `min_by` over a
///   REVERSED comparator is what implements "first of the maxima":
///   `Iterator::max_by` returns the LAST maximal element, which would make
///   the winner depend on the listing's length rather than on its head.
fn newest_created_fallback(sessions: &[Session]) -> Option<&Session> {
    sessions
        .iter()
        .filter(|session| !session.archived && session.created_at > 0)
        .min_by(|a, b| b.created_at.cmp(&a.created_at))
        .or_else(|| sessions.iter().find(|session| !session.archived))
}

/// Every status this list offers as a filter, in the spelling the wire uses
/// and the helm parses.
///
/// Offered as a CHOICE rather than typed, and that is the helm's constraint
/// showing through rather than a UI preference: it refuses an unrecognized
/// status with a 400 rather than answering "no sessions", precisely because
/// a typo that answers "no sessions" is a lie the user will believe. A
/// select cannot produce a typo.
///
/// `unknown` is deliberately NOT among them, even though the wire has the
/// word and this UI still decodes it. PLAN_M6_75.md item 3 makes Unknown
/// internal/compat vocabulary that must never RENDER — a session nothing has
/// classified yet shows no badge at all, precisely so the UI never says
/// something it does not know. Offering it here would put that word back on
/// screen in the one control whose options ARE its vocabulary, and would
/// invite a user to search for a state the rest of the interface refuses to
/// name. The compat side is untouched: a helm still reporting it decodes as
/// before, and those rows still appear under every unfiltered listing.
const FILTERABLE_STATUSES: [&str; 6] = [
    "running",
    "waiting",
    "idle",
    "exited",
    "interrupted",
    "error",
];

/// Keep the filter popover visibly tethered without letting it touch a
/// viewport edge that would make a focused control difficult to reach.
const FILTER_POPOVER_VIEWPORT_MARGIN_PX: f64 = 8.0;

/// The filter's preferred reading width. Placement owns the responsive cap so
/// the same ceiling governs a measured and an unmeasurable renderer.
const FILTER_POPOVER_MAX_WIDTH_PX: f64 = 288.0;

/// Separate the filter from its toggle while retaining the requested
/// below-left anchor when the viewport has room.
const FILTER_POPOVER_TOGGLE_GAP_PX: f64 = 2.0;

/// Render the filter popover from its toggle's viewport rect.
///
/// Unlike a row action menu, this surface is left-aligned with the toggle: it
/// does not need to keep a neighboring destructive-action column uncovered.
/// The CSS expressions preserve that ordinary anchor but clamp both horizontal
/// edges and the bottom edge when a narrow or short viewport cannot fit it.
/// `Unmeasured` stays hidden until the async rect read resolves, while a
/// measurement failure gets the visible fallback rather than a dead toggle.
fn filter_popover_placement_style(placement: PanelPlacement) -> String {
    match placement {
        PanelPlacement::Unmeasured => "opacity: 0; pointer-events: none;".to_string(),
        PanelPlacement::Measured(rect) => {
            let top = rect.max_y() + FILTER_POPOVER_TOGGLE_GAP_PX;
            let left = rect.min_x();
            format!(
                "opacity: 1; pointer-events: auto; right: auto; \
                 --filter-popover-top: max({FILTER_POPOVER_VIEWPORT_MARGIN_PX}px, \
                 min({top}px, calc(100vh - {FILTER_POPOVER_VIEWPORT_MARGIN_PX}px))); \
                 --filter-popover-left: max({FILTER_POPOVER_VIEWPORT_MARGIN_PX}px, \
                 min({left}px, calc(100vw - {FILTER_POPOVER_VIEWPORT_MARGIN_PX}px))); \
                 top: var(--filter-popover-top); left: var(--filter-popover-left); \
                 max-width: min({FILTER_POPOVER_MAX_WIDTH_PX}px, \
                 calc(100vw - {FILTER_POPOVER_VIEWPORT_MARGIN_PX}px - var(--filter-popover-left)), \
                 calc(100vw - {}px)); \
                 max-height: calc(100vh - {FILTER_POPOVER_VIEWPORT_MARGIN_PX}px - var(--filter-popover-top));",
                FILTER_POPOVER_VIEWPORT_MARGIN_PX * 2.0,
            )
        }
        PanelPlacement::Fallback => format!(
            "opacity: 1; pointer-events: auto; right: auto; \
             top: {FILTER_POPOVER_VIEWPORT_MARGIN_PX}px; left: {FILTER_POPOVER_VIEWPORT_MARGIN_PX}px; \
             max-width: min({FILTER_POPOVER_MAX_WIDTH_PX}px, \
             calc(100vw - {}px)); \
             max-height: calc(100vh - {}px);",
            FILTER_POPOVER_VIEWPORT_MARGIN_PX * 2.0,
            FILTER_POPOVER_VIEWPORT_MARGIN_PX * 2.0,
        ),
    }
}

/// Whether a listing reply may touch this view at all.
///
/// Two independent admissions, and a reply needs both. Split out of
/// `ListView`'s commit path so the rule can be exercised without a Dioxus
/// runtime — and so the two questions stay visibly separate, because
/// conflating them is exactly how one of them goes missing.
///
/// - **Does it still answer the question on screen?** `ops::ReadGate` orders
///   requests but knows nothing about what they ASKED. A read issued under
///   filter A completing after filter B was applied is the newest reply
///   available and still wrong: its rows describe A while every control
///   describes B. Nothing corrects that either — with a healthy feed the
///   fallback poll is off, and if B's own read failed there is no later
///   reply coming. Refusing it costs nothing, since applying a filter marks
///   a demand on the surface and the reader will ask again under B. The
///   ORDER is half of that question too, and in exactly the same shape: a
///   reply walked under the previous sort is a correctly-ordered list of the
///   wrong sequence, arriving under a control that now names another one.
/// - **Is it the newest word?** The generation gate's ordinary split, with
///   successes and failures gated differently for the reasons `ops` gives.
fn accepts_listing(
    reads: &mut ReadGate,
    generation: u64,
    succeeded: bool,
    answers_applied_query: bool,
) -> bool {
    if !answers_applied_query {
        return false;
    }
    if succeeded {
        reads.accept_success(generation)
    } else {
        reads.accept_failure(generation)
    }
}

/// Whether a click that only SNAPSHOTS this row's `Session` — clone (opens
/// the create form pre-filled) or replace (opens `confirming_replace`) —
/// must be refused outright, before `ListView` touches a single signal for
/// it.
///
/// Pulled out as a pure predicate over the same primitives `on_rename_start`
/// and `on_delete` already guard on, so the cross-guard is checkable without
/// mounting a component: the controls a busy or mid-decision row disables
/// lag one render behind the click that triggered them (`disabled`/
/// `aria-disabled` are attributes, not synchronous vetoes), so a click
/// queued just ahead of that render can still reach the handler. `busy` is
/// the page-wide operation lock (`OpLock::busy_now`); the rest are this
/// SPECIFIC row's own state — any of them true means this row's `Session`
/// is about to change, or is mid-decision, and is not a stable thing to
/// snapshot right now. Shared by both callers rather than duplicated,
/// because clone and replace need the identical answer to the identical
/// question — a row mid-archive-confirmation, say, is exactly as unstable
/// a clone source as it is a replace source.
fn clone_is_refused(
    busy: bool,
    session_id: &str,
    pending: &HashSet<String>,
    confirming: &HashSet<String>,
    confirming_archive: &HashSet<String>,
    confirming_replace: &HashSet<String>,
    renaming: Option<&str>,
) -> bool {
    busy || pending.contains(session_id)
        || confirming.contains(session_id)
        || confirming_archive.contains(session_id)
        || confirming_replace.contains(session_id)
        || renaming == Some(session_id)
}

/// The flat session list: host, title, cwd, invocation, and a truthful
/// status badge per row; the filter and search surface above them, the hosts
/// panel, the "new session" form and the per-row stop/delete actions
/// (PLAN_M2.md step 8) live here too, since all of them need to reach into
/// the same reads — a create or a stop should be reflected as soon as the
/// next read lands, not held behind an optimistic local edit.
///
/// ## Two reads, and what drives them (PLAN_M6_75.md item 6)
///
/// The listing and the hosts registry are separate reads, deliberately, so a
/// slow or failing `/api/hosts` cannot delay the session list or vice versa.
/// Each is reachable from four places and goes through ONE door
/// (`listing_read` / `hosts_read`), which is what keeps the generation gate a
/// total order over reads rather than a per-caller counter:
///
/// - once on mount, because a page has to draw something;
/// - on every feed notification, which is what replaced the periodic loops —
///   the mounted page re-reads through the very same commit path, so every
///   reconciliation rule those closures carry survives the change of trigger
///   untouched;
/// - from the fallback poll, which runs only while the feed is unhealthy and
///   no build mismatch has been latched (`feed::fallback_polls`);
/// - from the filter surface's own submit, since nothing else is coming.
///
/// None of those STARTS a read directly. All four ask the surface's reader
/// (`reader::request_read`), which runs one read at a time, coalesces
/// everything that arrives mid-read into a single follow-up, and retries a
/// read that never answered. Both properties are load-bearing rather than
/// tidy: a notification is spent the moment a read is dispatched, so without
/// the retry a read that failed against a HEALTHY feed leaves this page
/// stale until the fleet happens to change again — the fallback poll is off,
/// and nothing else is owed. Without the coalescing, a helm that has stopped
/// answering accumulates one read per notification and per fallback tick for
/// as long as the page is open.
///
/// Everything is scoped to this component. Under the sidebar layout
/// (BUGS_BURNDOWN.md issue 5) that no longer means much unmounting: the
/// list is permanently on screen beside the selected session, so its
/// readers stay live while a terminal is open — the coalescing above is
/// what keeps that affordable, and PLAN_M2.md's "polling stops while a
/// terminal is open" is deliberately retired along with the page swap
/// that implied it.
///
/// ## One more read, shared (PLAN_M6_75.md item 8)
///
/// Profiles add one always-active reader under exactly the same discipline
/// (`profiles::use_catalog_surface`). The app-bar popup and create picker
/// consume its answer together because profiles belong to the helm, not the
/// selected host. Keeping the reader mounted with this page also means feed
/// invalidations advance the answer while both consumers are closed.
///
/// ## Filtering is a query, not a render pass (PLAN_M6_75.md item 7)
///
/// The filter surface builds `api::SessionFilter` and the helm answers with
/// the matching rows plus their count; nothing here narrows a list it was
/// handed. That is the only arrangement coherent with the helm's cap — a
/// client-side filter over a cut list hides matches beyond the cut while
/// the banner reports a count that includes them.
///
/// The popover writes the applied filter as it is edited. Discrete choices
/// request a read immediately; the four text fields share a 150 ms generation
/// debounce, which coalesces a phrase or a multi-field edit without letting an
/// obsolete delay request a filter the user can no longer see.
///
/// ## Ordering is a query too, and a separate one
///
/// The sort control is the same arrangement with a different dimension: it
/// changes `api::ListSort`, the next walk asks the helm for that order, and
/// nothing here rearranges rows it was handed (which pagination forbids for
/// the same reason it forbids a client-side filter — the rows past a cut are
/// the ones a local sort never sees). Applied on CHANGE rather than on
/// submit, because unlike a filter there is no second field to fill in first.
///
/// It is deliberately NOT part of the filter's state, and the consequence
/// worth carrying in mind while reading the rest of this file is that every
/// reconciliation below keys off the FILTER: what a reply is evidence about,
/// whether the banner may call the list filtered, whether an absent session
/// left the fleet. A re-sorted listing covers exactly what the same filter's
/// listing covered, so none of those answers moves with it. The one thing the
/// order does gate is whether a reply is still on-topic at all (see
/// `accepts_listing`), since a walk under the previous order is a correct
/// list of the wrong sequence.
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
/// The session-OPEN click is gated by the same token even though the
/// sidebar layout no longer unmounts this component on open. What the
/// gate still buys is coherence, not survival: selecting a session swaps
/// the MAIN pane (remounting `SessionView` and its attachments), and
/// doing that mid-mutation would let the mutation's completion race a
/// right-pane world that changed under it — the same
/// two-actions-one-frame class the token exists for. Per-session
/// stop/rename/delete stay outside the token, on their own per-row set —
/// they cannot invalidate one another's premises, and two rows acting at
/// once is behavior the browser suite pins.
#[component]
pub(crate) fn ListView(
    on_open: EventHandler<Session>,
    /// The currently selected session's host — SPEC.md's first create-
    /// default clause, supplied by `App` because only it knows the
    /// selection. `None` when nothing is selected (or the selected
    /// session predates per-host metadata), which falls back to the
    /// helm's own host. Carries the install identity the session row
    /// reported beside the row id, so the default can notice a retarget
    /// or adopt landing between selection and create — see [`OpenHost`].
    open_host: Option<OpenHost>,
    /// The SHARED live-operation token (see `ops`'s module docs): owned by
    /// `AppBody` rather than created here, because the selected session's
    /// view claims the same token for its own restart/archive — a private
    /// token per pane would let the two panes mutate the fleet under each
    /// other.
    ops: OpLock,
    /// Live count of this list's per-row operations, maintained here and
    /// read by the session view's `PaneGate`: row operations never hold
    /// the token (rows must stay concurrent with each other), so this
    /// count is the only way the other pane can refuse to start a write
    /// while one is in flight.
    row_ops: Signal<u32>,
    /// A session this list REMOVED — a successful delete, or an archive
    /// (which the default filter drops). `AppBody` reconciles the
    /// selection: without this, deleting the selected row would leave the
    /// right pane showing a session this client knows is gone.
    on_removed: EventHandler<String>,
    /// A session this list successfully RENAMED, with the title the
    /// supervisor confirmed. `AppBody` patches the selected session's
    /// title in place (no remount): the view otherwise learns of renames
    /// through the feed-driven detail re-read, and under a LATCHED build
    /// mismatch that channel is withdrawn — leaving the sidebar showing
    /// the new name and the titlebar the old one for the rest of the
    /// page's life.
    on_renamed: EventHandler<(String, String)>,
    /// The currently selected session's id, or `None` when the right pane
    /// is empty — which is the state auto-select exists to end (see
    /// `stored_selection`): whenever a committed listing has rows and
    /// nothing is selected, this list opens one. A signal rather than a
    /// plain prop because `commit_listing` consults it at REPLY time (a
    /// captured prop value would be as old as the render that made the
    /// closure) and the auto-select effect tracks it.
    selected: ReadSignal<Option<String>>,
    /// The fleet's emptiness as the last committed listing proved it —
    /// `None` until a listing lands. `AppBody` renders the right pane's
    /// placeholder from this: "no active sessions" may only be claimed on
    /// a committed empty result, never during loading or failure (the
    /// sidebar's own status lines carry those).
    fleet_empty: Signal<Option<bool>>,
    /// A counter `AppBody` bumps on every EXTERNAL layout event that can
    /// move a session row out from under an open actions panel's already-
    /// measured `position: fixed` coordinates: an `onscroll` from EITHER
    /// of the sidebar's two real scrolling ancestors — `.app-sidebar`
    /// itself (`overflow: hidden auto`, the vertical scroller) and
    /// `.app-shell` (which scrolls horizontally when the window is
    /// narrower than the sidebar plus the main pane's floor) — plus an
    /// `onresize` on `.app-sidebar` where the renderer supports it (a
    /// window resize narrow enough to trigger `.app-shell`'s scroll moves
    /// rows without ever firing `onscroll` on its own). None of those
    /// listeners live here: this component only renders `.session-list`,
    /// which is not itself a scroll container (see the comment beside
    /// it) and would never see any of those events — none of them
    /// bubble. A counter rather than a plain signal-of-unit because
    /// `use_effect` reruns on ANY write, so what this component actually
    /// needs is just something to subscribe to that changes once per
    /// event; the count itself is never read, only watched.
    ///
    /// This is deliberately not the WHOLE story — see the `use_effect`
    /// that reads it (near `show_create`'s declaration, once every signal
    /// it also watches is in scope) for the INTERNAL layout causes (this
    /// component's own on-demand panels) it also watches, which never
    /// touch this counter at all.
    layout_epoch: ReadSignal<u64>,
) -> Element {
    let base = use_context::<ApiBase>().0;
    // The helm's shared preference, already read by `PreferencesGate` —
    // this component never mounts before it has (see `SharedPreferences`).
    let preferences = use_context::<SharedPreferences>();
    let mut row_ops = row_ops;
    let mut listing = use_signal(|| None::<Result<SessionListing, String>>);
    // The same generation discipline the hosts read has, for the same
    // reason and against a slower race: a listing read that started before
    // a delete can easily still be in flight when the delete's own refresh
    // has already landed — and committing it would put the deleted row
    // back until the next one.
    let mut listing_reads = use_signal(ReadGate::default);
    // The host registry as this client currently knows it — four states, not
    // three, so a failed read cannot blank the panel (see `hosts::HostsRead`).
    // Shared by the hosts panel and the create dialog's selector.
    let mut hosts = use_signal(HostsRead::default);
    // Per-REQUEST, not per-caller: the mount read, the feed's re-read, the
    // fallback poll and every mutation-triggered refetch draw from the same
    // gate, so an older completion cannot resurrect what a newer one removed
    // — see `ops::ReadGate` for why successes and failures are gated
    // differently.
    let mut hosts_reads = use_signal(ReadGate::default);
    // Ordinary host mutations and accepted provisioning runs are independent
    // owners. They stay in separate sets so one completion cannot erase the
    // other's busy state; rows render their union. Both live above the hosts
    // panel because navigating away can unmount the task that reported them.
    let mutation_busy_hosts = use_signal(HashSet::<HostId>::new);
    let provisioning_busy_hosts = use_signal(HashSet::<HostId>::new);
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
    // The on-demand sidebar popovers default closed and stay unpersisted.
    // Their state lives together here so every floating surface can enforce
    // the page-wide one-popover-at-a-time contract. The host list itself is
    // always mounted; its global details disclosure belongs to HostsPanel.
    let mut filter_open = use_signal(|| false);
    let mut profiles_open = use_signal(|| false);
    // The fixed popover needs the same measured-rect race handling as a row
    // menu. Its toggle lives in this component, so unlike a row-local menu
    // the geometry state belongs here beside the open state.
    let mut filter_toggle_handle = use_signal(|| None::<Rc<MountedData>>);
    let filter_placement = use_signal(|| PanelPlacement::Unmeasured);
    let filter_open_generation = use_signal(|| 0_u64);
    // Text inputs share one generation because their combined values form one
    // server-side query. A later edit to any text field makes every earlier
    // delayed request obsolete before it can add an intermediate read.
    let filter_text_generation = use_signal(|| 0_u64);
    // The count row is part of the filter toggle's anchor geometry, but live
    // filtering changes it on every accepted listing. Preserve the popover
    // through those ordinary updates and remeasure instead of treating each
    // changed count as a disruptive surface transition.
    let listing_header_shape = use_memo(move || match &*listing.read() {
        Some(Ok(listing)) => Some(count_banner(listing).text),
        None | Some(Err(_)) => None,
    });
    // `pending`'s entry and exit, with the cross-pane bookkeeping attached:
    // every row operation must (a) refuse to start while the SHARED token
    // is held — the session view or a page operation is mid-write, and a
    // row op landing under it is exactly the cross-pane race the shared
    // gate exists to close — and (b) keep `row_ops` equal to `pending`'s
    // size, because that count is what the session view's `PaneGate`
    // consults to refuse ITS claims while a row op runs. Paired helpers
    // rather than open-coded at each site so no exit path can forget the
    // decrement (a stuck count would leave the session view inert).
    let mut begin_row_op = move |id: &String| -> bool {
        if ops.busy_now() {
            return false;
        }
        if !pending.write().insert(id.clone()) {
            return false;
        }
        row_ops += 1;
        true
    };
    let mut end_row_op = move |id: &String| {
        if pending.write().remove(id) {
            row_ops -= 1;
        }
    };
    // Which sessions are showing the inline "confirm delete?" prompt in
    // place of their normal stop/delete buttons — see `on_delete` below.
    // Deliberately a plain client-side set with no timeout and no
    // refresh-driven reset: a listing refresh must leave an in-progress
    // confirmation alone (the user is mid-decision, not mid-refresh), so
    // this is intentionally NOT derived from `listing` on every render.
    // The one reconciliation that does happen is in `commit_listing`,
    // which drops an entry once its session is no longer in the listing
    // at all (deleted from elsewhere, say) — there is no row left for a
    // dangling entry to ever affect, so this is tidiness, not correctness.
    let mut confirming = use_signal(HashSet::<String>::new);
    // Archive has a distinct prompt because its consequence and mutation
    // differ from delete's. Keeping the sets separate also makes the row's
    // mutual exclusion explicit instead of overloading one flag with an
    // action kind that every handler would then have to decode.
    let mut confirming_archive = use_signal(HashSet::<String>::new);
    // Replace's own prompt, on the same footing as the two above. Unlike
    // `confirming_archive` — which `commit_listing` retires once a row
    // ARCHIVES, since an archived row's `archive_confirmation` prompt no
    // longer applies to it — this one reconciles the same way `confirming`
    // does: only a row that leaves the listing ENTIRELY drops its pending
    // replace confirmation, because replace stays a legitimate action on an
    // archived row (`row::session_menu_order` offers it unconditionally,
    // same as clone).
    let mut confirming_replace = use_signal(HashSet::<String>::new);
    // At most one row's actions menu is open, and this parent owns which.
    // A per-row boolean would let two menus fight, and the parent is the
    // only place "opening yours closes mine" can live. Defined up here with
    // the other row-scoped UI state because
    // `commit_listing` reconciles it (a menu whose row left the listing
    // must not reappear, already open, when the row comes back).
    let mut menu_open = use_signal(|| None::<String>);
    // The host row's counterpart — see `HostsPanel`'s own "one row menu
    // open, across BOTH panels" doc for why this is a SECOND signal (kept
    // in step with `menu_open` above by each side's own toggle callback)
    // rather than one signal shared between two very differently-shaped
    // row kinds. Owned here, next to `menu_open`, for the same reason that
    // one is: this is the only component both the session list and the
    // hosts panel are mounted underneath.
    let mut host_menu_open = use_signal(|| None::<HostId>);
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
    // them (a failed listing read swapping the rows for an error line)
    // unmounts the form entirely — a draft owned by the form would be
    // silently discarded with it. Seeded from the row's current title when
    // the field opens, which is also what keeps a read carrying someone
    // else's rename from overwriting an edit in progress.
    let mut renaming = use_signal(|| None::<String>);
    let mut rename_draft = use_signal(String::new);
    // The optimistic rename corrections `apply_optimistic_renames` paints
    // over the server's listing, keyed by session id and carrying the read
    // sequence number that bounds when the server could first have told
    // this view about it (`rows::settle_optimistic_renames`). The tab strip's
    // scheme, applied to a title: without the number, a listing reply that
    // was already in flight when the rename landed would be
    // indistinguishable from the server disagreeing, and the row would flip
    // back to the old title until the next read landed — a visible wobble on
    // the one operation whose entire point is that the new name shows up at
    // once.
    let mut renamed = use_signal(HashMap::<String, (String, u64)>::new);
    // How many listing reads this view has STARTED. A read's own index is
    // the value it takes before incrementing, so an optimistic rename
    // recording the current value names the first read GUARANTEED to have
    // started after the rename's response completed. That is a
    // conservative bound rather than a statement about the server: a read
    // launched earlier can perfectly well observe the committed title,
    // since the write lands before the response is read. Conservative is
    // the safe direction — it can only keep a correction slightly longer
    // than strictly necessary, never retire one on a reply that could not
    // have seen it.
    //
    // Named for the polls it used to count, and kept under that name
    // deliberately: `rows`' two pruning halves and the session view's twin
    // all speak of a "poll sequence", and renaming the counter without them
    // would leave two vocabularies for one number.
    let mut poll_sequence = use_signal(|| 0_u64);
    let mut show_create = use_signal(|| false);
    let provisioning_trace_shapes = use_signal(HashMap::<HostId, ProvisioningTraceShape>::new);
    // A shape signature for the always-mounted host list. A memo prevents
    // ordinary phase-only polls and progress-step churn from closing fixed
    // surfaces while still catching every rendered trace transition and the
    // count or read-state changes that move rows below the host list.
    let hosts_list_shape = use_memo(move || {
        let read = hosts.read();
        let mut traces = provisioning_trace_shapes
            .read()
            .iter()
            .map(|(host, shape)| (*host, shape.operation, shape.status))
            .collect::<Vec<_>>();
        traces.sort_by_key(|(host, _, _)| *host);
        (
            read.hosts().map(<[_]>::len).unwrap_or(0),
            read.is_loading(),
            read.refresh_error().is_some(),
            traces,
        )
    });
    // Closes any open actions-menu panel — a SESSION row's or a HOST row's,
    // whichever is open — the instant something above `.session-list` (or
    // above `.host-list`, for the host row's own panel) could have moved
    // the row it was measured against: a `position: fixed` panel's
    // coordinates are a one-time snapshot (see `menu_panel_style`), so
    // anything that shifts the sidebar's layout above the rows leaves it
    // visibly detached from its toggle unless something notices and closes
    // it. Two different KINDS of cause feed this one effect:
    //
    // - EXTERNAL layout shifts — a scroll on `.app-sidebar`/`.app-shell`,
    //   or (where the renderer supports it) a window resize — arrive as
    //   `layout_epoch`, a counter `AppBody` owns and bumps from listeners
    //   this component cannot host itself (see that prop's own doc for
    //   why).
    // - INTERNAL layout shifts this component causes directly: the create
    //   form and host-list read state can change the content above the
    //   session rows. None of these go
    //   through `layout_epoch` — they are this component's own renders, not
    //   an ancestor's scroll or resize — so each is read directly here
    //   instead. The fixed filter popover does not shift a row, so opening it
    //   is handled separately as mutual exclusion rather than geometry.
    //
    // The three dependencies are deliberate: `layout_epoch` aggregates
    // ancestor scroll and resize events, `show_create` covers the create
    // form, and `hosts_list_shape` covers host count/read-state and collapsed
    // provisioning-trace transitions. The initial run is a no-op because
    // both row-menu signals start empty.
    //
    // NOT exhaustive — a same-INDEX height change on a row already above
    // the open one (a per-row error line appearing, say) moves the open
    // row without tripping any of these signals or the index-based
    // reflow check in `commit_listing`. See that check's own doc for the
    // residual this leaves and why it is accepted rather than chased
    // further here. `commit_listing`'s own reconciliation is SESSION-only,
    // too: the hosts list carries no analogous reorder guard, on the
    // judgment that a small, largely id-ordered registry reordering under
    // an open host menu is enough rarer than a session listing reordering
    // to accept as a residual rather than duplicate that machinery for a
    // second row kind.
    //
    // The host row accepts the identical same-index-height-change residual
    // as the session row above, for the identical reason: it is covered by
    // NEITHER a reorder guard nor the three consolidated dependencies above,
    // and a host row's OWN detail/remedy/warning/error text growing or
    // shrinking is exactly the shape of change that can move an open host
    // menu without tripping any of them. Chasing it would mean
    // watching every open row's own measured height, which is the same
    // trade `commit_listing`'s doc already declines for the rarer
    // reordering case, made again here for a residual judged rarer still.
    use_effect(move || {
        layout_epoch();
        show_create();
        hosts_list_shape();
        if menu_open.peek().is_some() {
            menu_open.set(None);
        }
        if host_menu_open.peek().is_some() {
            host_menu_open.set(None);
        }
    });
    // Scroll and resize are ancestor-owned movement signals. Keep this effect
    // narrow so the filter closes only when that anchor can actually move,
    // not merely because an unrelated menu signal reran the broad effect.
    use_effect(move || {
        layout_epoch();
        if *filter_open.peek() {
            filter_open.set(false);
        }
    });
    // Host-list and create-form shape changes move the header itself. Unlike
    // a listing count update, these are layout transitions, so a fixed
    // snapshot is no longer a trustworthy attachment to its toggle.
    use_effect(move || {
        show_create();
        hosts_list_shape();
        if *filter_open.peek() {
            filter_open.set(false);
        }
    });
    // There is one floating surface at a time. Opening the fixed filter is
    // mutual exclusion, not a row-geometry event; closing it must not race a
    // row menu the user opens immediately afterward.
    use_effect(move || {
        if filter_open() {
            menu_open.set(None);
            host_menu_open.set(None);
            if *profiles_open.peek() && ops.busy_now() {
                // A busy profile popup cannot be dismissed: the response
                // needs its mounted form. Refuse the newer surface instead
                // of briefly allowing two fixed panels to overlap.
                filter_open.set(false);
            } else {
                profiles_open.set(false);
            }
        }
    });
    // Opening the profile popup takes every other floating surface down. Its
    // own busy guard is enforced by `AppBar`, so a mutation cannot strand the
    // form by letting another surface replace it mid-request.
    use_effect(move || {
        if profiles_open() {
            filter_open.set(false);
            menu_open.set(None);
            host_menu_open.set(None);
        }
    });
    // Row menus are the remaining entry points into the same mutual-
    // exclusion set. They can be opened from child components, so an effect
    // is the single place that also covers keyboard and pointer activation.
    use_effect(move || {
        if menu_open().is_some() || host_menu_open().is_some() {
            filter_open.set(false);
            if *profiles_open.peek() && ops.busy_now() {
                // Row menus remain ordinary transient surfaces while the
                // profile mutation owns the page. Closing the attempted menu
                // preserves both the one-popover rule and the busy form.
                menu_open.set(None);
                host_menu_open.set(None);
            } else {
                profiles_open.set(false);
            }
        }
    });

    // The create dialog's explicit host choice is separate from the catalog:
    // it names the installation used for creation idempotency, while every
    // host now sees the same helm-owned profiles.
    let mut chosen_host = use_signal(|| None::<HostId>);
    // A "clone" click's seed for the create form (`create_form::
    // CreatePrefill`), or `None` for the ordinary blank-form open. Lives
    // HERE rather than inside the form for the same reason `chosen_host`
    // does: the form itself unmounts on close and remounts fresh on the
    // next open, so anything the NEXT open has to remember belongs to
    // whatever survives that — this component's own state. Cleared
    // alongside `chosen_host` at both of that signal's own clearing points
    // below, so an ordinary "new session" open never inherits a stale
    // clone's fields.
    let mut clone_prefill = use_signal(|| None::<CreatePrefill>);
    // The filter the reads carry and the popover displays. One signal is now
    // the honest model: edits are live, and `accepts_listing` rejects replies
    // that were walking under an earlier value while a debounce was pending.
    let mut filter = use_signal(SessionFilter::default);
    // The order the reads are carrying, seeded from the helm's shared
    // remembered order (`stored_sort` over the `SharedPreferences` context).
    //
    // ONE signal, unlike the filter's applied/draft pair, because a sort has
    // no draft state to speak of: picking an option IS the decision, there is
    // nothing else on the control to fill in first, and applying it costs the
    // same one re-read a submit would. It is deliberately not a field of
    // `SessionFilter` either — see `api::ListSort` for why the two dimensions
    // stay apart, and note that every reconciliation predicate on this page
    // reads the FILTER: a re-sorted listing covers exactly what the same
    // filter's listing covered, so nothing about evidence changes with it.
    let mut sort = use_signal(move || stored_sort(preferences));
    // Render the optimistic shared preference directly: compactness has no
    // separate draft or listing-read state to keep synchronized.
    let compact = stored_compact(preferences);

    // The two surfaces' readers (`reader::SurfaceReader`): one reader each,
    // coalescing every trigger into a single live read and retrying one that
    // failed to answer. Separate, because the two reads are independent —
    // a hanging `/api/hosts` must not hold the session list off the screen,
    // which is the same independence the mount reads keep below.
    let listing_surface = use_signal(SurfaceReader::default);
    let hosts_surface = use_signal(SurfaceReader::default);

    // One always-active reader feeds both the management popup and the create
    // picker. Mounting it with the list preserves a last-known catalog across
    // either surface closing and gives feed/fallback refreshes one door.
    let profiles = use_catalog_surface();

    // Which installation the create request is about. This follows
    // `effective_create_host`, but changing it deliberately leaves the
    // helm-wide profile choice untouched.
    let mut create_target = use_signal(|| None::<CreateTarget>);
    // `use_reactive` because `open_host` is a plain prop, not a signal:
    // without it the effect would capture the value it saw on first run
    // and an open form would keep offering the OLD session's host and
    // catalog after the user selected a session on another machine —
    // rerenders alone never rerun a `use_effect`.
    use_effect(use_reactive((&open_host,), move |(open_host,)| {
        let wanted = show_create()
            .then(|| {
                let read = hosts.read();
                let options = host_options(read.hosts().unwrap_or_default());
                let effective = effective_create_host(&options, chosen_host(), open_host.as_ref());
                options
                    .into_iter()
                    .find(|host| Some(host.id) == effective)
                    // The fingerprint comes off the option rather than being
                    // re-derived, so request validation and the selected
                    // machine share one registry snapshot.
                    .map(|host| CreateTarget::new(host.id, host.incarnation))
            })
            .flatten();
        // Compared before writing so an unrelated hosts refresh does not
        // rotate the create request's idempotency key.
        if *create_target.peek() != wanted {
            create_target.set(wanted);
        }
    }));

    // Everything that happens to a listing reply once it is BACK, in one
    // place: decide whether this read still speaks for the view, reconcile
    // the view-local state a fresh listing settles, and paint it. A reply
    // the gate rejects leaves every one of those untouched.
    //
    // Hoisted rather than inlined because there are several readers of the
    // session listing — the mount read, the feed's re-read, the fallback
    // poll, and `on_stop`'s immediate refetch — and a second hand-rolled
    // copy of the gate decision is exactly the kind of divergence that shows
    // up as a stale row nobody can reproduce. M6.75 made that pay off
    // directly: the feed became one more caller of this closure rather than
    // a second commit path with its own approximation of these rules.
    //
    // What it deliberately does NOT do is claim the generation. That claim
    // has to happen synchronously at the point the request is ISSUED (see
    // `ops::ReadGate::start`), so that the order reads are gated in is the
    // order they were asked for rather than the order their tasks happened
    // to be polled. Taking an already-claimed `generation` keeps that
    // property with the caller, where the `await` is.
    //
    // `requested` and `ordered_by` are the filter and the order this reply
    // ANSWERS, both sampled where the request was issued. Ordering alone is
    // not enough to make a reply usable: the gate knows that read A started
    // before read B, but not that B asked a different question. A read for
    // filter A completing after filter B was applied would paint A's rows
    // under controls describing B — indefinitely, if B's own read failed —
    // so a reply whose filter is no longer the applied one is refused
    // outright, and its order is checked on the same argument (a walk under
    // the old sort would land as a list in a sequence the control no longer
    // names). The user's next move (submitting, clearing, re-sorting) is a
    // read of its own, and the surface reader has already recorded that
    // demand, so nothing is lost by dropping this one.
    //
    // `authoritative` says whether this READ speaks for the whole fleet —
    // `on_stop`'s refetch exists to show ONE session's new status and does
    // not. It is necessary and not sufficient: the REPLY has to speak for
    // the whole fleet too, which is `rows::absence_is_evidence`'s question
    // (a filtered listing omits what did not match; a truncated one omits
    // whatever lay past its ceiling, and neither omission means "gone").
    //
    // What every successful read does regardless is settle the corrections
    // its own ROWS speak to (`rows::settle_optimistic_renames`): a title the
    // server now agrees with graduates, and a title it contradicts on a late
    // enough read loses. That half needs no authority, and withholding it
    // from filtered reads is what used to leave a rename painted over the
    // server's own rows for as long as any filter was applied.
    let mut commit_listing = move |generation: u64,
                                   requested: SessionFilter,
                                   ordered_by: ListSort,
                                   fetched: Result<SessionListing, String>,
                                   index: u64,
                                   authoritative: bool| {
        // Superseded reads, and reads answering a question nobody is asking
        // anymore, are dropped before they can touch anything — including
        // the optimistic-correction pruning below, which would otherwise
        // retire a rename on the authority of a walk that predates it.
        let accepted = accepts_listing(
            &mut listing_reads.write(),
            generation,
            fetched.is_ok(),
            *filter.peek() == requested && *sort.peek() == ordered_by,
        );
        if !accepted {
            return;
        }
        // A second handle on the SAME `listing` signal, under a name that
        // survives the shadowing two lines down (`if let Ok(listing) =
        // &fetched` rebinds `listing` to the reply's own rows for the rest
        // of this block) — needed below to read the OUTGOING listing
        // (still the signal's current value at that point) as the
        // baseline `rows::menu_row_reordered` diffs against. `Signal` is
        // `Copy`, so this is not a second signal, just a second name for
        // the one `ListView` owns.
        let listing_signal = listing;
        if let Ok(listing) = &fetched {
            // Only a successful fetch is evidence about titles: an error
            // carries none at all, so it can neither confirm nor contradict
            // an optimistic rename.
            settle_optimistic_renames(&mut renamed.write(), &listing.sessions, index);
            // An open actions menu closes the moment its row leaves the
            // RENDERED list — any committed reply counts, filtered or
            // not, unlike the fleet-absence retains below (a title filter
            // is not evidence a session left the fleet, but it absolutely
            // removes the row this transient popup was anchored to).
            // Left set, the panel would reappear already open if the row
            // later returned — a popup nobody re-requested, exposing
            // controls for a session whose state changed while it was
            // gone. Confirmation and rename state deliberately DO
            // survive this (they are answers-in-progress, and the suite
            // pins that a refresh cannot revert them); the menu is just
            // a lens.
            let menu_vanished = menu_open
                .read()
                .as_ref()
                .is_some_and(|id| !listing.sessions.iter().any(|s| s.id == *id));
            if menu_vanished {
                menu_open.set(None);
            }
            // The row STAYING listed does not mean it stayed PUT: an
            // insert or removal above it shifts its index without ever
            // making `menu_vanished` true (`rows::menu_row_reordered`'s
            // own doc has the full contract, including why a first load
            // or a recovery from a failed read is deliberately never
            // read as a reorder here). Only evaluated once the row is
            // confirmed still present — a row `menu_vanished` already
            // closed above has nothing left here to reconcile.
            //
            // `still_open_id` is bound to an OWNED value, not left as a
            // live `peek()` borrow: the mutation below needs `menu_open`
            // free to write again, and a borrow held through the `if let`
            // chain (Rust extends its temporary past the whole condition)
            // would still be outstanding at that point. The PREVIOUS
            // listing's `Ref` guard is scoped even tighter, to this one
            // block: it must drop well before this closure's own
            // `listing.set(...)` at the end, or that write would panic
            // against a borrow still outstanding on the same signal.
            let still_open_id = menu_open.peek().clone();
            if !menu_vanished && let Some(open_id) = still_open_id {
                let reordered = {
                    let previous = listing_signal.peek();
                    let previous_sessions = match previous.as_ref() {
                        Some(Ok(prev)) => Some(prev.sessions.as_slice()),
                        _ => None,
                    };
                    menu_row_reordered(previous_sessions, &listing.sessions, &open_id)
                };
                if reordered {
                    menu_open.set(None);
                }
            }
            // The right pane's placeholder may only claim "no active
            // sessions" on a committed result that actually proves one:
            // the DEFAULT view's own reply, uncut, with no rows. Not `rows::is_empty_fleet` — that helper answers a
            // different question (is the WHOLE fleet empty), and the
            // ordinary view withholds archived rows, so it can never
            // support that claim no matter what its own count says. An
            // archived-only fleet lists nothing in the default view, and
            // "no ACTIVE sessions" is exactly right for it. A user filter
            // proves nothing about the pane and leaves the verdict as it
            // was.
            if requested == SessionFilter::default() && !listing.truncated {
                fleet_empty.set(Some(listing.sessions.is_empty()));
            }
        }
        // Everything below reads ABSENCE, so it needs a read with standing
        // AND a reply that covers the fleet — and a successful one, since an
        // error reply carries no session ids and a transient failure is not
        // evidence that any session left.
        if let Ok(listing) = &fetched
            && authoritative
            && absence_is_evidence(listing)
        {
            // Drop any `confirming` entry whose session is gone from
            // this fetch entirely — the counterpart to the "a poll
            // refresh must not clear an in-progress confirmation"
            // rule just above: that rule protects a row that is
            // still LISTED, not one that has vanished (deleted from
            // another client while this one sat mid-confirmation, an
            // externally-imposed departure the `retain` below cannot
            // distinguish from the id simply never having existed).
            let live_ids: HashSet<&str> = listing.sessions.iter().map(|s| s.id.as_str()).collect();
            confirming
                .write()
                .retain(|id| live_ids.contains(id.as_str()));
            let active_ids: HashSet<&str> = listing
                .sessions
                .iter()
                .filter(|session| !session.archived)
                .map(|session| session.id.as_str())
                .collect();
            confirming_archive
                .write()
                .retain(|id| active_ids.contains(id.as_str()));
            // Replace reconciles against `live_ids`, not `active_ids`: an
            // archived row is still a legitimate replace target (see the
            // signal's own doc), so archiving a row elsewhere must not
            // silently dismiss a replace confirmation already open on it —
            // only the row leaving the listing entirely does that.
            confirming_replace
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
            // The SELECTED session reconciles on the same evidence: a
            // session deleted from another client must not keep a detail
            // view (and an attachment) mounted for an object this client
            // now knows is gone. Deliberately ONLY here — a filtered
            // listing hides rows without unselecting them (a filter
            // narrows the list, not the open session), and a truncated
            // walk proves nothing about what it did not reach; the
            // absence gate above is precisely "this reply speaks for the
            // fleet".
            let selected_vanished = selected
                .peek()
                .as_ref()
                .is_some_and(|id| !live_ids.contains(id.as_str()));
            if selected_vanished && let Some(id) = selected.peek().clone() {
                on_removed.call(id);
            }
            retire_vanished_renames(&mut renamed.write(), &listing.sessions, index);
        }
        listing.set(Some(fetched));
    };

    // One listing read, shared by every caller — the mount read, the feed's
    // re-read, the fallback poll, and the filter surface's apply. Going
    // through one place is what makes the generation a total order over
    // READS rather than a per-caller counter each could satisfy
    // independently, and it is why swapping the poll for the feed changed
    // the TRIGGER and nothing else.
    //
    // Everything that has to describe THIS request is sampled synchronously,
    // before the `await`: the generation, the read's position in the read
    // order, and the filter itself. Sampling the filter after the await
    // would let a submit landing mid-flight relabel a reply as answering a
    // query it never asked. Sampled per CALL rather than per reader, because
    // the reader calls this again for a retry or a coalesced notice, and
    // that later read is a new request with a new filter to answer for.
    //
    // The `bool` it reports is what `reader` needs: whether the helm
    // answered at all. Anything narrower (whether the reply was painted)
    // would make a superseded read look like a failed one and retry against
    // a surface that is already current.
    //
    // Cloned bases rather than one moved in: a `move ||` closure takes
    // ownership of everything it captures, and `on_stop`/`on_delete` need
    // their own copy of `base` afterward.
    let read_listing_base = base.clone();
    let listing_read = move || {
        let base = read_listing_base.clone();
        // Cloned OUT of the signal rather than read through it, and the
        // annotation is what enforces that: a borrow guard moved into the
        // async block below would be held across the walk's every round
        // trip, and the filter surface's own submit writes that signal.
        let requested: SessionFilter = filter.peek().clone();
        // Sampled with the filter and for the same reason: the reply has to
        // be able to say which SEQUENCE it walked, or a re-sort landing
        // mid-flight would relabel it as answering the order now on screen.
        let ordered_by: ListSort = *sort.peek();
        // Read before incrementing, so `index` is this read's own position
        // in the view's read order — what tells an optimistic rename whether
        // this reply is late enough to be evidence about it. Claimed even
        // for a filtered read, so the order stays a single sequence; what a
        // filtered read does not get is AUTHORITY over absence (see
        // `commit_listing`).
        let index = poll_sequence.peek().to_owned();
        poll_sequence += 1;
        // The EVIDENCE predicate, never the banner's: the default view reads
        // as unfiltered on screen while still hiding archived rows, so
        // authorizing its reads to treat absence as departure would retire
        // work on any session archived from another client.
        let authoritative = !requested.omits_fleet_members();
        let generation = listing_reads.write().start();
        async move {
            let fetched = fetch_sessions(&base, &requested, ordered_by).await;
            let answered = fetched.is_ok();
            commit_listing(
                generation,
                requested,
                ordered_by,
                fetched,
                index,
                authoritative,
            );
            answered
        }
    };

    // One hosts read, generation-guarded, shared by every caller — the mount
    // read, the feed, the fallback poll and every mutation-triggered
    // refetch, on the same one-door reasoning as the listing above.
    //
    // The number is claimed synchronously, at the CALL, so ordering is
    // decided by when a read was asked for rather than by when its task
    // happens to be scheduled. The `bool` it reports means what the listing
    // read's does: the helm answered, whatever the gate then did with it.
    let read_hosts_base = base.clone();
    let hosts_read = move || {
        let base = read_hosts_base.clone();
        let generation = hosts_reads.write().start();
        async move {
            let outcome = fetch_hosts(&base).await;
            let answered = outcome.is_ok();
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
            answered
        }
    };

    // The two doors every trigger below actually knocks on. Asking for a
    // read is not the same as starting one: `reader::request_read` starts a
    // reader only if this surface has none, and otherwise records the demand
    // so the running read is followed by exactly one more (see `reader`).
    // That is what keeps a burst of notifications, or a helm that has
    // stopped answering, from accumulating walks for as long as the page is
    // open — and what makes a read that FAILED get retried at all, since the
    // notification that prompted it is spent the moment it is dispatched.
    //
    // Every caller names WHY it wants a read (`reader::Trigger`), and the
    // three answers are treated differently in two ways that matter here: a
    // fallback tick may only start an idle reader (it carries no news, so
    // cancelling a backoff with it would flatten the retry ladder into a
    // three-second poll), and under a latched build mismatch only ATTENDED
    // reads still happen — the feed and the fallback stand down, while a
    // live filter edit or a mutation's refresh is answered, because the page
    // must keep working for the person using it (SPEC_impl.md's withdrawal
    // rule is about unattended behavior).
    //
    // Cloned per call site rather than made `Copy`: the read closures own
    // their copy of the API base, which is a `String`.
    let request_listing =
        move |trigger: Trigger| request_read(listing_surface, trigger, listing_read.clone());
    let request_hosts =
        move |trigger: Trigger| request_read(hosts_surface, trigger, hosts_read.clone());

    // The page's first look at both surfaces. A `use_hook` rather than a
    // future, because the work of starting a read now belongs to the reader:
    // this is one call, made once, on mount.
    //
    // Both reads are asked for together and neither waits on the other,
    // which preserves the independence the two loops had: a hosts read that
    // hangs must not hold the session list off the screen.
    //
    // Explicit, and that classification is a decision rather than a default:
    // a mount is a person navigating here, it happens once rather than on a
    // cadence, and under a latched build mismatch it is the difference
    // between a page that shows what it can beside the reload prompt and one
    // that shows nothing at all. The withdrawal rule revokes unattended
    // BEHAVIOR, not the user's ability to look at their own fleet.
    let mount_listing = request_listing.clone();
    let mount_hosts = request_hosts.clone();
    use_hook(move || {
        mount_listing(Trigger::Explicit);
        mount_hosts(Trigger::Explicit);
    });

    // The feed's consumer (PLAN_M6_75.md item 6): every revision
    // notification re-reads BOTH surfaces through the same doors, because
    // the notification says only that something changed — a status flip, a
    // rename, a host going down and a registry edit are indistinguishable on
    // that channel by design, so the honest answer is to re-read whatever
    // this page is showing.
    //
    // Marked, not awaited: an effect is not a place to hold a round trip
    // open. A notice landing while a read is already in flight is not
    // dropped — it becomes the follow-up read the reader runs next.
    let feed_listing = request_listing.clone();
    let feed_hosts = request_hosts.clone();
    use_feed_reader(move || {
        feed_listing(Trigger::Notice);
        feed_hosts(Trigger::Notice);
    });

    // The documented fallback (PLAN_M6_75.md item 6). The timer runs
    // unconditionally and the READ is what is gated, which is deliberate:
    // the alternative — starting and stopping a task as the feed's health
    // changes — makes the handover a lifecycle problem, and a fallback whose
    // job is to cover the moment the feed fails is the worst possible thing
    // to have to spin up at that moment.
    //
    // So it ticks forever and asks only while `feed::fallback_polls` says
    // to: never on a healthy feed, and never under build skew, where the
    // page stands down entirely rather than polling a helm whose vocabulary
    // it does not share.
    //
    // Both surfaces on one timer, unlike the mount reads: a fallback is not
    // where the independence argument pays, and one loop is one thing to
    // reason about when the interesting question is whether it runs at all.
    // A tick that lands while the previous one's read is still walking adds
    // nothing at all — a tick may only start an IDLE reader (see
    // `reader::Trigger::Scheduled`), which is what keeps a slow helm from
    // turning a three-second cadence into a queue and what keeps a tick from
    // cancelling a backoff it knows nothing about.
    let fallback_listing = request_listing.clone();
    let fallback_hosts = request_hosts.clone();
    use_future(move || {
        let listing = fallback_listing.clone();
        let hosts = fallback_hosts.clone();
        async move {
            loop {
                fallback_sleep().await;
                if fallback_polls_now() {
                    listing(Trigger::Scheduled);
                    hosts(Trigger::Scheduled);
                }
            }
        }
    });

    // An immediate re-read after a host mutation, instead of waiting for the
    // helm's own notification. Every host verb changes state this side
    // cannot predict — an add's status is whatever the connection finds, a
    // retarget's is a fresh active-retry window, an adopt's is a reconnect —
    // so there is nothing honest to paint optimistically, and the fastest
    // truthful answer is the server's. The feed will say so too, a moment
    // later, and the generation gate is what makes the two arriving in
    // either order harmless.
    //
    // BOTH surfaces, because every host verb moves the session list too: the
    // merged view is per-host, so a removal takes that host's sessions out of
    // it, an adopt rebinds them, and a retarget changes which machine they
    // are read from. Refreshing only the panel leaves rows for a host that is
    // no longer registered, with a fleet total that still counts them — and
    // under a latched build mismatch, where the feed and the fallback are
    // both withdrawn, "leaves" means for the rest of the page's life.
    let host_mutation_listing = request_listing.clone();
    let refresh_hosts = move |_| {
        request_hosts(Trigger::Explicit);
        host_mutation_listing(Trigger::Explicit);
    };

    // Filtering writes the query the listing reader carries immediately.
    // A read already walking under the old query is allowed to finish, then
    // refused by `commit_listing`; recording a fresh demand is what makes the
    // reader follow it with the query now visible in the popover.
    let filter_read = request_listing.clone();
    let request_text_filter = {
        let filter_read = filter_read.clone();
        move || {
            let mut generation = filter_text_generation;
            generation += 1;
            let captured_generation = generation();
            let generation_at_delay = generation;
            let filter_read = filter_read.clone();
            spawn(async move {
                sleep_ms(150).await;
                if *generation_at_delay.peek() == captured_generation {
                    filter_read(Trigger::Explicit);
                }
            });
        }
    };
    // A discrete choice is already a complete filter. It also retires a
    // pending text delay, so changing a select while typing cannot request
    // the same final filter twice.
    let request_immediate_filter = {
        let filter_read = filter_read.clone();
        move || {
            let mut generation = filter_text_generation;
            generation += 1;
            filter_read(Trigger::Explicit);
        }
    };

    // This is the filter-toggle counterpart of a row menu's measurement
    // task. Capturing the generation before the await prevents a stale rect
    // from a prior open from repainting a fast close-and-reopen.
    let measure_filter_popover = move || {
        let handle = filter_toggle_handle;
        let mut placement = filter_placement;
        let generation = filter_open_generation();
        spawn(async move {
            let measured = match handle.peek().clone() {
                Some(handle) => handle.get_client_rect().await.ok(),
                None => None,
            };
            if let Some(outcome) =
                measurement_outcome(generation, *filter_open_generation.peek(), measured)
            {
                placement.set(outcome);
            }
        });
    };
    // A count line can mount or change text above the toggle while someone is
    // typing. Start a fresh measurement generation so an older rect cannot
    // overwrite the post-listing position, but leave the live controls open.
    let remeasure_filter_popover = measure_filter_popover;
    use_effect(move || {
        listing_header_shape();
        if *filter_open.peek() {
            let mut generation = filter_open_generation;
            let mut placement = filter_placement;
            generation += 1;
            placement.set(PanelPlacement::Unmeasured);
            remeasure_filter_popover();
        }
    });

    // Change the order the list is read in, and remember it for this client.
    //
    // Three things happen and each is deliberate. The signal moves, so every
    // read from here on asks for the new sequence. The preference is written
    // — on CHANGE only, which is what keeps a client that never touches the
    // control from writing storage at all. And a read is asked for, for
    // live filter edit's reason: nothing else is coming, since the fleet did
    // not change and no revision will be published for a decision this client
    // made about itself.
    //
    // The walk simply restarts, which is the only correct way to re-sort a
    // cursor-paginated list: a cursor names a position in ONE order, and the
    // helm refuses one replayed under another (`api::fetch_sessions`).
    //
    // What deliberately does NOT happen is any change to the selection. A
    // sort reorders rows; it does not decide which session the other pane is
    // showing — the same principle SPEC.md states for filtering, where the
    // list may stop showing the selected session's row entirely and the pane
    // still stays put. Here even the row survives: it is still listed, just
    // elsewhere, so it goes on rendering as the selected one.
    let sort_read = request_listing.clone();
    let sort_base = base.clone();
    let mut apply_sort = move |next: ListSort| {
        // Re-selecting the option already in force is not a change: acting on
        // it would restart the walk and write the helm's row for nothing.
        if *sort.peek() == next {
            return;
        }
        sort.set(next);
        remember_sort(&sort_base, preferences, next);
        sort_read(Trigger::Explicit);
    };

    let stop_base = base.clone();
    // The surface reader, for the one path that reads outside it: a stop's
    // own refetch fails on its own and has nobody to retry it (see below).
    let stop_recovery = request_listing.clone();
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
        if confirming.read().contains(&id)
            || confirming_archive.read().contains(&id)
            || confirming_replace.read().contains(&id)
            || renaming.read().as_deref() == Some(id.as_str())
        {
            return;
        }
        // Re-entry guard for the per-session in-flight set: a disabled
        // button should already stop this, but the click and the
        // re-render that disables it are not synchronous, so the handler
        // checks for itself too. Refused both when an op for this id is
        // already running and while the shared token is held (see
        // `begin_row_op`).
        if !begin_row_op(&id) {
            return;
        }
        let base = stop_base.clone();
        // Cloned per invocation: the spawned task takes ownership of what it
        // captures, and this handler runs once per stop click.
        let stop_recovery = stop_recovery.clone();
        spawn(async move {
            // No optimistic flip (PLAN_M2.md design note): the row's
            // badge only ever reflects what the NEXT poll observes, so a
            // stop that silently failed can never leave the UI claiming a
            // session is exited when tmux still disagrees.
            let outcome = stop_session(&base, &id).await;
            match outcome.err() {
                Some(e) => {
                    errors.write().insert(id.clone(), format!("stop: {e}"));
                    end_row_op(&id);
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
                    // Through the SAME gate every other read uses, which is
                    // the whole reason the gate is per-request rather than
                    // per-loop: this read exists to show the stop at once,
                    // and a listing walk that started before the stop — a
                    // walk is several round trips, so one easily spans it —
                    // completing afterwards would put the pre-stop status
                    // back and undo exactly what this call is for. The race
                    // did not go away when the poll did: the stop itself
                    // bumps the helm's revision, so the feed answers with a
                    // re-read of its own, and the gate is what orders the
                    // two.
                    //
                    // Deliberately NOT routed through the surface reader,
                    // despite reading the same endpoint: this read exists to
                    // show ONE session's new status immediately, and a
                    // reader that happened to be mid-walk would turn it into
                    // a follow-up read that lands whenever the current one
                    // finishes. It is bounded by the operation that issued
                    // it — one stop, one refetch — rather than by the
                    // reader, and ordered against everything else by the
                    // same generation gate.
                    let generation = listing_reads.write().start();
                    // Cloned out of the signal before the walk, never read
                    // through it — see `listing_read` for why a guard must
                    // not survive into an await.
                    let snapshot: SessionFilter = filter.peek().clone();
                    // The order this refetch walks, snapshotted on the same
                    // terms — a stop's own read is still a read, and it must
                    // be refused like any other if the user re-sorts while it
                    // is in flight.
                    let ordered_by: ListSort = *sort.peek();
                    // Claimed like any other read, so the read order stays
                    // one sequence: an optimistic rename asks whether a
                    // reply STARTED after it, and a read outside the
                    // numbering could not answer.
                    let index = poll_sequence.peek().to_owned();
                    poll_sequence += 1;
                    let fetched = fetch_sessions(&base, &snapshot, ordered_by).await;
                    let failed = fetched.is_err();
                    // Not authoritative: this read speaks for one session's
                    // status, so a session missing from it is not a session
                    // that left — see `commit_listing`. It still settles the
                    // renames its own rows agree or disagree with.
                    commit_listing(generation, snapshot, ordered_by, fetched, index, false);
                    // A failure here replaces a perfectly good list with an
                    // error line, and nothing outside the reader ever retries
                    // — this walk is the operation's own, so a lost request
                    // would leave the page reading "failed to load sessions"
                    // until the fleet next changed. Handing the demand to the
                    // surface reader is what turns that into a blip: it
                    // retries on the ladder and the rows come back.
                    if failed {
                        stop_recovery(Trigger::Explicit);
                    }
                    end_row_op(&id);
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
    let delete_refresh = request_listing.clone();
    let mut do_delete = move |id: String| {
        if !begin_row_op(&id) {
            return;
        }
        let base = delete_base.clone();
        let refresh = delete_refresh.clone();
        spawn(async move {
            let outcome = delete_session(&base, &id).await;
            match outcome.err() {
                Some(e) => {
                    errors.write().insert(id.clone(), format!("delete: {e}"));
                    end_row_op(&id);
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
                    end_row_op(&id);
                    // AFTER the local bookkeeping, so a selection change
                    // this triggers repaints against the already-updated
                    // listing: the owner clears the right pane if the
                    // session it shows is the one that just went away.
                    on_removed.call(id.clone());
                    // The optimistic removal takes the ROW and nothing else:
                    // the fleet total, the matching count and the truncation
                    // flag all still describe a list that included it, and
                    // this client cannot recompute any of them (a truncated
                    // walk does not even know whether the row it dropped was
                    // one of the ones it was counting). The feed's own
                    // notification would settle that — except under a latched
                    // build mismatch, where the feed and the fallback are both
                    // withdrawn and this explicit read is the ONLY thing that
                    // will ever correct the banner.
                    refresh(Trigger::Explicit);
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
            || confirming_archive.read().contains(&target.id)
            || confirming_replace.read().contains(&target.id)
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
        // Refused OUTRIGHT while the shared token is held, BEFORE the
        // confirming flag is touched: `do_delete`'s own `begin_row_op`
        // would refuse anyway, but by then the flag is gone and the
        // prompt has silently dismissed itself with nothing deleted —
        // the confirmation must survive a refusal so the user can answer
        // it again once the other pane's operation finishes.
        if ops.busy_now() {
            return;
        }
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

    // Archive shares the per-row operation gate with stop, rename, and
    // delete, but it owns a separate confirmation. A successful response is
    // followed by a list read because the applied archive switch decides
    // whether the retained row disappears or changes in place; the client
    // cannot repair the fleet-wide counts by editing one row itself.
    let archive_base = base.clone();
    let archive_refresh = request_listing.clone();
    let mut do_archive = move |id: String| {
        if !begin_row_op(&id) {
            return;
        }
        errors.write().remove(&id);
        let base = archive_base.clone();
        let refresh = archive_refresh.clone();
        spawn(async move {
            match archive_session(&base, &id).await {
                Ok(_) => {
                    // The row leaves the LOCAL listing before the selection
                    // owner hears about the removal: the auto-select that
                    // runs the moment the selection clears reads this very
                    // listing, and the archived row still sitting in it —
                    // as the remembered id, no less — would be immediately
                    // re-selected, defeating the reconciliation.
                    if !filter.peek().include_archived
                        && let Some(Ok(current)) = listing.write().as_mut()
                    {
                        current.sessions.retain(|s| s.id != id);
                    }
                    refresh(Trigger::Explicit);
                    // An archived session leaves the DEFAULT filter, so for
                    // the selection's owner it has been removed just as a
                    // delete removes. Under an include-archived filter the
                    // row stays listed, so the selection legitimately
                    // stays too.
                    if !filter.peek().include_archived {
                        on_removed.call(id.clone());
                    }
                }
                Err(e) => {
                    errors.write().insert(id.clone(), format!("archive: {e}"));
                }
            }
            end_row_op(&id);
        });
    };
    let mut do_archive_on_confirm = do_archive.clone();
    let on_archive = move |session: Session| {
        if session.archived
            || pending.read().contains(&session.id)
            || confirming.read().contains(&session.id)
            || confirming_replace.read().contains(&session.id)
            || renaming.read().as_deref() == Some(session.id.as_str())
        {
            return;
        }
        if archive_confirmation(&session, session.tabs.len()).is_some() {
            confirming_archive.write().insert(session.id);
        } else {
            do_archive_on_confirm(session.id);
        }
    };
    let confirm_archive = move |id: String| {
        // Same shared-token refusal as `confirm_delete`, for the same
        // keep-the-prompt reason.
        if ops.busy_now() {
            return;
        }
        if confirming_archive.write().remove(&id) {
            do_archive(id);
        }
    };
    let cancel_archive = move |id: String| {
        confirming_archive.write().remove(&id);
    };

    // Replace shares the per-row operation gate with stop, rename,
    // archive, and delete (`begin_row_op`, which is also what enforces the
    // page-wide nav lock here — see that helper's own doc), but its
    // success path does something none of the others do: it changes which
    // session is SELECTED, not merely how the clicked row itself now reads.
    //
    // The selection write happens BEFORE `refresh` requests the next
    // listing read, and that ordering is load-bearing, not incidental.
    // `ListView`'s own auto-select effect only runs while `selected` reads
    // `None` — see that effect's guard — so writing the new session as the
    // selection FIRST is what keeps the reconciliation path from ever
    // getting a chance to run at all: there is no window where the old
    // row's disappearance leaves the pane momentarily unselected for that
    // effect to fill with whatever the fallback would have picked.
    // `remember_selection` is the same "a user-initiated choice" write
    // `guarded_open` and the create form's `on_created` make; `on_open` is
    // what actually swaps the right pane, which a stored preference alone
    // does not do for a client that already has a page open.
    let replace_base = base.clone();
    let replace_refresh = request_listing.clone();
    let mut do_replace = move |id: String| {
        if !begin_row_op(&id) {
            return;
        }
        errors.write().remove(&id);
        let base = replace_base.clone();
        let refresh = replace_refresh.clone();
        spawn(async move {
            match replace_session(&base, &id).await {
                Ok(session) => {
                    remember_selection(&base, preferences, &session.id);
                    on_open.call(session);
                    refresh(Trigger::Explicit);
                }
                Err(e) => {
                    // Keyed by the SOURCE id: the row this error belongs
                    // beside is the one the user clicked "replace" on,
                    // which — on a delete-after-create failure — is also
                    // the row that is still there to show it next to. On
                    // that same failure the message already names the new
                    // session's id too (`api::replace_session`'s own doc),
                    // so nothing here needs to remember it separately.
                    errors.write().insert(id.clone(), format!("replace: {e}"));
                }
            }
            end_row_op(&id);
        });
    };
    // The "replace" menu item's click: guarded by the SAME predicate
    // `on_clone` uses (`clone_is_refused`'s own doc explains why the two
    // share it — the difference between clone and replace is never in
    // whether either is refused, only in what happens once accepted), then
    // opens `confirming_replace` unconditionally, unlike `on_delete`'s
    // `has_ended()` split. Delete's split exists because an already-finished
    // session has nothing left for the CONFIRMATION to be honest about
    // beyond "delete anyway"; replace always has something worth confirming
    // regardless of liveness — a fresh session is about to take this row's
    // place — so `status::replace_consequence` is total over every status
    // (no `Option` to skip on) and this handler never bypasses the prompt
    // once the guard above lets it through.
    let on_replace = move |session: Session| {
        if clone_is_refused(
            ops.busy_now(),
            &session.id,
            &pending.read(),
            &confirming.read(),
            &confirming_archive.read(),
            &confirming_replace.read(),
            renaming.read().as_deref(),
        ) {
            return;
        }
        confirming_replace.write().insert(session.id);
    };
    let confirm_replace = move |id: String| {
        // Same shared-token refusal as `confirm_delete`/`confirm_archive`,
        // for the same keep-the-prompt reason.
        if ops.busy_now() {
            return;
        }
        if confirming_replace.write().remove(&id) {
            do_replace(id);
        }
    };
    let cancel_replace = move |id: String| {
        confirming_replace.write().remove(&id);
    };

    // The "clone" menu item's click. It never calls the API itself — it
    // opens the create form pre-filled from the clicked row, and the
    // ordinary submit path is what actually launches anything (see
    // `create_form::CreatePrefill` for what gets carried across, and
    // `CreateSessionForm`'s own doc for how a new generation reseeds an
    // already-open form).
    //
    // The generation is minted here, not by the form: it has to keep
    // climbing across clone clicks the form never even reopens for (the
    // form stays mounted while the user clones a second row without
    // closing it first), so the ONE place that has seen every click this
    // page has ever handled is the only place that can hand out a number
    // guaranteed to be new. `menu_open` is closed explicitly rather than
    // left to the general "close on anything that could move the layout"
    // effect near `show_create`'s declaration: that effect WILL also close
    // it once `show_create` flips, but the row whose menu this was is about
    // to be covered by the form regardless, and there is no reason to make
    // the close wait for that effect's next pass.
    //
    // Guarded FIRST, before any signal is touched — the same discipline
    // `on_rename_start` keeps and for the same reason: the row's own
    // `disabled`/`aria-disabled` attributes lag one render behind the
    // click that set them, so a clone queued just ahead of that render
    // (the shared token being claimed, or this very row entering a
    // pending or confirming state) would otherwise still reach here and
    // replace the form's prefill, or open it, out from under whatever the
    // other operation is doing. `ops.busy_now()` covers the page-wide
    // lock (a create submit already minting a key, a host mutation in
    // flight); the three row-local sets cover this SPECIFIC row having a
    // stop/delete in flight, a destructive confirmation open, or its
    // rename field open — any of which means this row's own `Session` is
    // about to change or is mid-decision, not a stable thing to snapshot
    // into a fresh clone right now.
    let on_clone = move |session: Session| {
        if clone_is_refused(
            ops.busy_now(),
            &session.id,
            &pending.read(),
            &confirming.read(),
            &confirming_archive.read(),
            &confirming_replace.read(),
            renaming.read().as_deref(),
        ) {
            return;
        }
        menu_open.set(None);
        let generation = clone_prefill
            .peek()
            .as_ref()
            .map_or(0, |prefill| prefill.generation + 1);
        clone_prefill.set(Some(prefill_from(&session, generation)));
        show_create.set(true);
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
        // The shared token counts too: the disabled attribute on the menu's
        // rename control is cosmetic (a dispatched synthetic click still
        // reaches this handler), and opening the field while the other
        // pane's operation owns the gate would present an editor whose
        // submit is guaranteed to be refused.
        if ops.busy_now()
            || pending.read().contains(&id)
            || confirming.read().contains(&id)
            || confirming_archive.read().contains(&id)
            || confirming_replace.read().contains(&id)
        {
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
    let rename_refresh = request_listing.clone();
    let on_rename_submit = move |(id, title): (String, String)| {
        if !begin_row_op(&id) {
            return;
        }
        // This row's own previous failure, cleared by the retry that
        // supersedes it and by nothing else (see `errors`).
        errors.write().remove(&id);
        let base = rename_base.clone();
        let refresh = rename_refresh.clone();
        spawn(async move {
            match rename_session(&base, &id, &title).await {
                Ok(session) => {
                    on_renamed.call((id.clone(), session.title.clone()));
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
                    // The overlay paints the new title and can do nothing
                    // else — and a title is exactly what a filter can be ON,
                    // and now also what the list can be ORDERED by. A row
                    // renamed OUT of an active title search stays on screen
                    // under a query it no longer matches, and the counts
                    // beside it still describe the old name; a row renamed
                    // under title order paints its new name in its OLD
                    // position, so the sidebar is briefly not alphabetical.
                    // Both are the same staleness and both end the same way,
                    // on the next read. That is acceptable rather than
                    // merely tolerated: the position is cosmetic where the
                    // title is the thing the user just typed, and the
                    // correction costs nothing extra — the read below is the
                    // one every rename already triggers. Normally the feed
                    // would supply it; under a latched build mismatch nothing
                    // does, so this explicit read is the correction.
                    refresh(Trigger::Explicit);
                }
                Err(e) => {
                    errors.write().insert(id.clone(), format!("rename: {e}"));
                }
            }
            end_row_op(&id);
        });
    };

    // Opening a row swaps which session the keyed `SessionView` beside this
    // list shows. The sidebar itself stays mounted now, but the selection
    // change still tears down the previous keyed view — and an operation
    // this list has in flight is about to repaint rows whose identity a
    // same-frame selection change would pull out from under it. So the open
    // click consults the page token AND this view's own per-session set,
    // and it does so INSIDE the handler — the `nav_locked` value below is
    // what the button renders with, and a render-time value is exactly what
    // a click landing in the same frame as the operation it should have
    // seen would read as idle.
    // Closures below need OWNED copies of the API base (`open_base` here,
    // `sort_base` and `created_base` likewise): each is `move`d into a
    // handler that outlives this render, so a borrow cannot serve.
    let open_base = base.clone();
    let guarded_open = move |session: Session| {
        if ops.busy_now() || !pending.peek().is_empty() {
            return;
        }
        // A USER-initiated selection is what gets remembered — the
        // auto-select fallback deliberately never writes (see
        // `SharedPreferences`).
        remember_selection(&open_base, preferences, &session.id);
        on_open.call(session);
    };

    // Stable callback identities are created once, outside the row loop.
    // The row-specific session or id travels as the callback argument, so
    // no hook depends on fleet size and unchanged rows remain memoized when
    // their parent refreshes.
    let guarded_open = use_callback(guarded_open);
    // Auto-select (BUGS_BURNDOWN.md issue 5, interviewed): an empty right
    // pane is a state to END, not to show — the remembered selection if
    // its row is still listed, else the newest-created non-archived row.
    // That fallback is picked by `created_at` rather than by position (see
    // `newest_created_fallback`), and has to be: the list is no longer
    // necessarily read in creation order, so the first row is whatever the
    // client's chosen sort put there while SPEC.md's fallback is
    // specifically the newest-created session.
    // Everything consulted is a TRACKED read, deliberately: the
    // effect must rerun when the listing commits, when the selection
    // clears, and when the write gates drain — an auto-select refused
    // during a busy operation would otherwise be lost until unrelated
    // fleet activity. The remembered id itself needs no waiting on: the
    // helm's preference was read before this component mounted.
    //
    // A remembered id ABSENT from a TRUNCATED listing is not evidence it
    // is gone (the helm's cap cut the view); it is resolved directly
    // instead, and only a definite not-found retires it in favor of the
    // fallback. The fallback itself deliberately does not re-persist:
    // overwriting the user's real choice with whichever row a cut listing
    // happened to hold would erase it permanently (see `SharedPreferences`).
    //
    // The fallback picks from the rows in hand even when the listing is
    // incomplete. Under a non-creation order the newest session may sit
    // past the cut, so the pick can be merely the newest row the reply
    // reached — accepted, because a listing cut at the helm's cap is a
    // fleet of hundreds of sessions, the auto-select exists to keep the
    // pane from sitting empty rather than to be exact, and the alternative
    // (a second request for one row in creation order) was a whole extra
    // API shape for that corner.
    let resolve_base = base.clone();
    let mut resolving_remembered = use_signal(|| false);
    let mut remembered_dead = use_signal(|| None::<String>);
    use_effect(move || {
        if selected.read().is_some() {
            return;
        }
        let page_busy = ops.busy();
        let rows_busy = *row_ops.read() > 0;
        let listing_read = listing.read();
        let Some(Ok(listing_ok)) = listing_read.as_ref() else {
            return;
        };
        if page_busy || rows_busy {
            return;
        }
        // A TRACKED read: retiring the remembered id (the 404 arm below)
        // must rerun this effect so the fallback can proceed.
        let remembered =
            stored_selection(preferences).filter(|id| remembered_dead.read().as_ref() != Some(id));
        let in_page = listing_ok
            .sessions
            .iter()
            .find(|s| remembered.as_deref() == Some(s.id.as_str()));
        if in_page.is_none()
            && let Some(id) = remembered.clone()
            && !listing_is_complete(listing_ok)
        {
            if !*resolving_remembered.peek() {
                resolving_remembered.set(true);
                let base = resolve_base.clone();
                spawn(async move {
                    match fetch_session(&base, &id).await {
                        Ok(Some(session)) => on_open.call(session),
                        // Definitely gone (or unreadable): the fallback may
                        // proceed on the next effect run.
                        _ => remembered_dead.set(Some(id)),
                    }
                    resolving_remembered.set(false);
                });
            }
            return;
        }
        // The newest-created row among the ones in hand (see
        // `newest_created_fallback` for why that is not simply the first).
        let candidate = in_page.or_else(|| newest_created_fallback(&listing_ok.sessions));
        if let Some(session) = candidate {
            // The same synchronous handler-time guard a click gets.
            if ops.busy_now() || !pending.peek().is_empty() {
                return;
            }
            on_open.call(session.clone());
        }
    });
    let toggle_menu = use_callback(move |id: String| {
        let currently = menu_open.peek().as_deref() == Some(id.as_str());
        menu_open.set(if currently { None } else { Some(id) });
        // Opening a session row's menu must close whichever host row's menu
        // is open — see `HostsPanel`'s own "one row menu open, across BOTH
        // panels" doc.
        if !currently {
            host_menu_open.set(None);
        }
    });
    let mark_seen_base = base.clone();
    // The row's read/unread toggle — the menu item and the dot both funnel
    // here (SPEC.md, Status). Routed through `queue_seen_write` rather than
    // a bare `mark_seen` call, unlike this handler's own earlier shape: the
    // automatic mark-on-open effect (`session_view.rs`) writes the SAME
    // endpoint for the SAME session, and only the shared queue guarantees
    // this manual write can never be overtaken by an older automatic one
    // still in flight — see `queue_seen_write`'s own doc. No `pending`/
    // `begin_row_op` guard either, since a toggle is not a destructive
    // lifecycle op and `SessionRow` already refuses the click while its own
    // `busy` flag is set — this handler only ever needs to queue the write.
    //
    // Unlike every OTHER handler's silent, log-only failure path
    // (`api::mark_seen`'s own doc explains why the automatic mark stays
    // silent), a MANUAL toggle's failure surfaces to the row's error line
    // exactly like `on_stop`/`on_delete`/`on_archive`/`on_rename_submit`
    // above (SPEC.md, Errors and diagnostics) — the user asked for this one
    // directly, so losing it silently would be exactly the kind of
    // succeeded-when-it-failed illusion that section forbids.
    let on_mark_seen = move |(id, seen_activity_at): (String, Option<i64>)| {
        let report_id = id.clone();
        queue_seen_write(
            &mark_seen_base,
            &id,
            seen_activity_at,
            move |result| match result {
                Ok(()) => {
                    errors.write().remove(&report_id);
                }
                Err(error) => {
                    errors
                        .write()
                        .insert(report_id.clone(), format!("seen: {error}"));
                }
            },
        );
    };
    let on_mark_seen = use_callback(on_mark_seen);
    let on_stop = use_callback(on_stop);
    let on_delete = use_callback(on_delete);
    let confirm_delete = use_callback(confirm_delete);
    let cancel_delete = use_callback(cancel_delete);
    let on_archive = use_callback(on_archive);
    let on_clone = use_callback(on_clone);
    let on_replace = use_callback(on_replace);
    let confirm_archive = use_callback(confirm_archive);
    let cancel_archive = use_callback(cancel_archive);
    let confirm_replace = use_callback(confirm_replace);
    let cancel_replace = use_callback(cancel_replace);
    let on_rename_start = use_callback(on_rename_start);
    let on_rename_submit = use_callback(on_rename_submit);
    let on_rename_cancel = use_callback(move |_| renaming.set(None));
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
    let host_options: Vec<HostOption> = host_options(hosts.read().hosts().unwrap_or_default());
    // Which registry row is the helm's own machine, derived once for the
    // whole render: it is what lets each row decide whether it is local,
    // remote, or unplaceable, and therefore which glyph (if any) and which
    // host name to draw (see `shared::session_locality`, and `SessionRow`'s
    // doc for the density argument). Taken from the
    // ALREADY-NORMALIZED `host_options` above rather than a second scan of
    // the raw registry — `HostOption::local` is the same
    // `HostKind::Local` comparison a dedicated rescan would make, so a
    // second helper computing it a second way was a second place for that
    // rule to drift. No new subscription either: this reads the `Vec` this
    // render already built, not `hosts` again.
    let local_host = host_options
        .iter()
        .find(|host| host.local)
        .map(|host| host.id);
    // The shared coarse clock every row's activity age is measured against,
    // read ONCE for the whole list rather than per row. Reading it here is
    // also what subscribes this view to the 30-second tick, which is the
    // only thing that makes an age on screen advance on a fleet where
    // nothing else is happening (see `activity`).
    let now_secs = *ACTIVITY_NOW.read();

    // What clear has to undo: any departure from the public default, the
    // archive switch included. Flipping that switch is something the user did
    // and clear is the way back from it, even though it does not narrow the
    // count banner's view.
    let filter_changed = *filter.read() != SessionFilter::default();
    // The selector's own copy: the create form takes ownership of
    // `host_options` further down, and both surfaces want the same list —
    // the same hosts, called the same things, with the same phase labels.
    let filter_hosts = host_options.clone();
    // The host this filter names that the registry no longer carries, if
    // any — a host removed from another client while a filter on it remains
    // applied. Only derive it once a registry read has landed: an empty list
    // before the first read would make every id look removed.
    let removed_filter_host = filter.read().host.filter(|id| {
        hosts.read().hosts().is_some() && !filter_hosts.iter().any(|host| host.id == *id)
    });
    // Event handlers own their captures. Give each field its own handle to
    // the shared request operation so no handler consumes the operation the
    // next field needs to invoke.
    let request_host_filter = request_immediate_filter.clone();
    let request_status_filter = request_immediate_filter.clone();
    let request_archive_filter = request_immediate_filter.clone();
    let request_parent_filter = request_text_filter.clone();
    let request_directory_filter = request_text_filter.clone();
    let request_profile_filter = request_text_filter.clone();
    let request_title_filter = request_text_filter;
    let request_clear_filter = request_immediate_filter.clone();
    // The create form's own copy of the API base, for recording a created
    // session as the selection (see `remember_selection`).
    let created_base = base.clone();

    rsx! {
        AppBar {
            profiles_open,
            profiles,
            ops,
            layout_epoch,
        }
        // The host list is one permanent surface. Keeping the component
        // mounted is a lifecycle requirement, not only a layout choice:
        // discovery, planning, and mutation replies must retain their owner.
        HostsPanel {
            hosts,
            ops,
            mutation_busy_hosts,
            provisioning_busy_hosts,
            provisioning_trace_shapes,
            host_menu_open,
            session_menu_open: menu_open,
            filter_open,
            on_changed: refresh_hosts,
        }
        // The filter is a viewport-fixed popover rather than an in-flow bar:
        // it follows the same overflow-escaping geometry as row menus without
        // making the sidebar reflow every time the controls open.
        if filter_open() {
        div {
            class: "filter-popover",
            style: filter_popover_placement_style(filter_placement()),
            onkeydown: move |evt| {
                if evt.key() == Key::Escape {
                    evt.prevent_default();
                    // Escape is the one close path that restores focus: the
                    // user asked to leave this transient surface, rather than
                    // choosing an outside destination with the pointer or Tab.
                    filter_open.set(false);
                    document::eval("document.querySelector('.filter-toggle')?.focus({ preventScroll: true });");
                }
            },
            onfocusout: move |_| {
                // Dioxus does not preserve `relatedTarget` through the
                // desktop event bridge. Once this bubbling event has finished
                // moving focus, the document's active element answers the
                // same question on both renderers — and it has to be a
                // four-way answer, not "inside or not":
                //
                // - inside the popover: focus moved between its own controls;
                // - the toggle: only reachable by keyboard (Tab from the last
                //   control), since the toggle's own `onmousedown` refuses
                //   focus on a pointer press; that is a genuine leave;
                // - `body` or nothing: EITHER focus is in transit — the
                //   control that had it was just unmounted by a re-render (a
                //   keystroke in the live filter, say) and the render that
                //   re-mounts it has not landed — OR the user clicked inert
                //   chrome outside (the count line), which focuses nothing
                //   at all. The two look identical at this instant and mean
                //   opposite things, so this case is asked AGAIN a moment
                //   later: a transit has resolved inside by then, a click on
                //   nothing has not, and only the latter closes;
                // - anything else: the user really moved on. Close, and leave
                //   focus where they put it.
                let mut filter_open = filter_open;
                let generation = filter_open_generation();
                spawn(async move {
                    const WHERE_IS_FOCUS: &str = "const active = document.activeElement; \
                         if (!active || active === document.body) { return 'transit'; } \
                         if (document.querySelector('.filter-popover')?.contains(active)) { return 'inside'; } \
                         if (active.classList.contains('filter-toggle')) { return 'toggle'; } \
                         return 'outside';";
                    let mut destination = document::eval(WHERE_IS_FOCUS).join::<String>().await;
                    if matches!(destination.as_deref(), Ok("transit")) {
                        // Long enough for a re-render to re-mount and refocus
                        // a replaced control; short enough that a click on
                        // inert chrome still reads as an immediate close.
                        sleep_ms(120).await;
                        destination = document::eval(WHERE_IS_FOCUS).join::<String>().await;
                    }
                    // A bridge that cannot answer is UNKNOWN, not evidence
                    // that focus escaped. A stale focus-out task also must
                    // not close a newer open after a quick toggle cycle.
                    if matches!(destination.as_deref(), Ok("outside") | Ok("toggle") | Ok("transit"))
                        && generation == *filter_open_generation.peek()
                    {
                        filter_open.set(false);
                    }
                });
            },
            label {
                "host"
                select {
                    class: "filter-host",
                    // Focused explicitly on mount rather than through
                    // `autofocus`: a browser honors autofocus on a control
                    // inserted after load only while NOTHING else holds
                    // focus, and whatever opened this popover usually does
                    // (the toggle after a keyboard activation; the last
                    // control the user touched after a pointer one, since
                    // the toggle refuses pointer focus). A popover that opens
                    // without focus inside it never sees the focus-out that
                    // is its only pointer-driven close path, so a click on
                    // inert chrome would leave it standing.
                    onmounted: move |evt| {
                        spawn(async move {
                            let _ = evt.data().set_focus(true).await;
                        });
                    },
                    // The empty value is "any host", not "no host": absence
                    // is what an unfiltered dimension looks like on the wire,
                    // so it is what the blank option has to produce.
                    value: filter.read().host.map(|id| id.to_string()).unwrap_or_default(),
                    onchange: move |evt| {
                        filter.write().host = evt.value().parse::<HostId>().ok();
                        request_host_filter();
                    },
                    // Selection is stated on each option, not only via the
                    // select's `value`: the option LIST mutates under an
                    // applied value (a removed host's option gives way to
                    // the tombstone below), and a re-rendered list resets
                    // the browser's selection to nothing while Dioxus —
                    // whose Rust-side `value` did not change — never
                    // re-applies it. The same defect class the create and
                    // profile selects fixed; this select joined it the day
                    // the tombstone made its options mutable.
                    option {
                        value: "",
                        selected: filter.read().host.is_none(),
                        "any host"
                    }
                    for host in filter_hosts.iter() {
                        option {
                            key: "{host.id}",
                            value: "{host.id}",
                            selected: filter.read().host == Some(host.id),
                            "{host.label()}"
                        }
                    }
                    // A host the filter names but the registry no longer
                    // carries gets a tombstone rather than vanishing.
                    //
                    // Without one the select falls back to showing its first
                    // option — "any host" — while the applied filter goes on
                    // sending the dead id with every read: the control says
                    // one thing, the request says another, and the rows
                    // agree with neither. The alternative (clearing the
                    // filter for them) was rejected because it silently
                    // widens a query the user chose; a disabled option
                    // states the situation and leaves the fix theirs, which
                    // is one click on any other option.
                    //
                    // Only ever rendered once the registry has actually been
                    // read: before that every id looks unregistered, and a
                    // tombstone during loading would be a lie that flickers.
                    if let Some(missing) = removed_filter_host {
                        option {
                            value: "{missing}",
                            disabled: true,
                            // Selected by construction: the tombstone only
                            // renders while the applied filter names this host, and
                            // it must claim the selection the moment it
                            // replaces the ordinary option.
                            selected: true,
                            "host {missing} (no longer registered)"
                        }
                    }
                }
            }
            label {
                "status"
                select {
                    class: "filter-status",
                    value: "{filter.read().status}",
                    onchange: move |evt| {
                        filter.write().status = evt.value();
                        request_status_filter();
                    },
                    option { value: "", "any status" }
                    for status in FILTERABLE_STATUSES {
                        option { key: "{status}", value: "{status}", "{status}" }
                    }
                }
            }
            label {
                "parent"
                input {
                    r#type: "text",
                    class: "filter-parent",
                    autocomplete: "off",
                    autocorrect: "off",
                    autocapitalize: "none",
                    spellcheck: "false",
                    value: "{filter.read().parent}",
                    oninput: move |evt| {
                        filter.write().parent = evt.value();
                        request_parent_filter();
                    },
                }
            }
            // The four free-text dimensions opt out of every form of
            // browser text mangling for the same reason the create form's
            // fields do: a directory is a literal path, a profile is a name
            // the helm matches exactly, and an autocorrected search term
            // finds the wrong thing while looking like it found nothing.
            label {
                "directory"
                input {
                    r#type: "text",
                    class: "filter-directory",
                    autocomplete: "off",
                    autocorrect: "off",
                    autocapitalize: "none",
                    spellcheck: "false",
                    value: "{filter.read().directory}",
                    oninput: move |evt| {
                        filter.write().directory = evt.value();
                        request_directory_filter();
                    },
                }
            }
            label {
                "profile"
                input {
                    r#type: "text",
                    class: "filter-profile",
                    autocomplete: "off",
                    autocorrect: "off",
                    autocapitalize: "none",
                    spellcheck: "false",
                    // Free text rather than a picker over the catalog, and
                    // deliberately: the helm matches a profile by id OR by
                    // the name a session snapshotted at creation, which is
                    // what keeps a DELETED profile's sessions findable — and
                    // a picker built from the catalog could not offer a
                    // profile that no longer exists.
                    value: "{filter.read().profile}",
                    oninput: move |evt| {
                        filter.write().profile = evt.value();
                        request_profile_filter();
                    },
                }
            }
            label {
                "search titles"
                input {
                    r#type: "text",
                    class: "filter-title",
                    autocomplete: "off",
                    autocorrect: "off",
                    autocapitalize: "none",
                    spellcheck: "false",
                    value: "{filter.read().title}",
                    oninput: move |evt| {
                        filter.write().title = evt.value();
                        request_title_filter();
                    },
                }
            }
            label { class: "filter-archived",
                input {
                    r#type: "checkbox",
                    class: "filter-include-archived",
                    checked: filter.read().include_archived,
                    onchange: move |evt| {
                        filter.write().include_archived = evt.checked();
                        request_archive_filter();
                    },
                }
                "include archived"
            }
            button {
                r#type: "button",
                class: "btn filter-clear",
                // Inert while nothing is applied, so the control cannot
                // offer to undo something that never happened. Cosmetic
                // only, like every other `disabled` on this page — the
                // handler clears an already-empty filter harmlessly.
                //
                disabled: !filter_changed,
                onclick: move |_| {
                    filter.set(SessionFilter::default());
                    request_clear_filter();
                    // Clearing disables this very button while it holds
                    // focus, and a focused control that becomes disabled
                    // drops focus onto `body`. The popover's focus-out probe
                    // would read that as a click on inert chrome and close
                    // the popover the user is still working in. The title
                    // field is where the next query goes, so focus moves
                    // there deliberately instead of falling out.
                    document::eval(
                        "document.querySelector('.filter-title')?.focus({ preventScroll: true });",
                    );
                },
                "clear"
            }
        }
        }
        // The session list's own header: count, compact preference, and the
        // creation action share one line so neither former control row costs
        // vertical space in the fixed sidebar.
        div { class: "list-header",
            div { class: "session-heading",
                if let Some(Ok(listing)) = &*listing.read() {
                    {
                        let banner = count_banner(listing);
                        rsx! { div { class: "{banner.class}", "{banner.text}" } }
                    }
                }
                label { class: "compact-toggle",
                    input {
                        r#type: "checkbox",
                        checked: compact,
                        onchange: move |event| {
                            let next = event.checked();
                            // Removing metadata moves every row below it; an
                            // open menu still holds its old anchor coordinates.
                            menu_open.set(None);
                            remember_compact(&base, preferences, next);
                        },
                    }
                    "compact"
                }
                button {
                r#type: "button",
                class: "btn btn-primary new-session-button",
                // The heading keeps the short visible word "new" while the
                // accessible name preserves the object named by the former
                // full-width label.
                aria_label: "new session",
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
                    let opening = !show_create();
                    if !opening {
                        // Closing the dialog discards its host choice with
                        // every other draft it holds. The signal lives up here
                        // because target identity and idempotency are shared
                        // with the list's create wiring; it would otherwise be
                        // the one piece of form state that survived a cancel.
                        // SPEC.md's
                        // creation default is about a FRESH dialog, not about
                        // where the last one was pointed.
                        chosen_host.set(None);
                        // Same reasoning as `chosen_host` right above:
                        // without this, the next "new session" open would
                        // silently reopen pre-filled from whatever row was
                        // last cloned.
                        clone_prefill.set(None);
                    }
                    show_create.set(opening);
                },
                    "new"
                }
            }
            // Keep the form immediately after its opener in DOM order:
            // forward Tab from "new" must enter it, not skip past a form
            // inserted above the heading. The opener remains in view above
            // the draft and still provides its existing cancellation action.
            if show_create() {
                CreateSessionForm {
                    hosts: host_options,
                    open_host,
                    hosts_loaded: hosts.read().hosts().is_some(),
                    chosen_host,
                    create_target,
                    catalog: profiles,
                    ops,
                    prefill: clone_prefill(),
                    on_created: move |session: Session| {
                        // Creation is a user-initiated selection too.
                        remember_selection(&created_base, preferences, &session.id);
                        show_create.set(false);
                        // This component stays mounted after creation, so
                        // clear the draft's host and clone seed just as the
                        // explicit cancellation path does.
                        chosen_host.set(None);
                        clone_prefill.set(None);
                        on_open.call(session);
                    },
                }
            }
        // The session list's remaining header: filter toggle and sort.
        // Rendered in EVERY listing state, not only after
        // a successful read — these controls are the only way to change or
        // clear a filter, and a failed read is exactly when someone may need
        // to (a filter naming a host that has since been removed, say). The
        // count already lives beside compact and new, so this row contains
        // only the controls that need a separate line.
            div { class: "list-header-controls",
                button {
                    r#type: "button",
                    class: "btn filter-toggle",
                    aria_expanded: filter_open(),
                    // A pointer press must not move focus onto this button:
                    // the popover closes on focus-out, and a click that first
                    // took focus would close it from the focus-out and then
                    // reopen it from the click. Keyboard activation never
                    // goes through mousedown, so Enter/Space are unaffected,
                    // and a Tab that lands here still counts as leaving.
                    onmousedown: move |evt| evt.prevent_default(),
                    onmounted: move |element| {
                        filter_toggle_handle.set(Some(element.data()));
                    },
                    onclick: move |_| {
                        if filter_open() {
                            filter_open.set(false);
                        } else {
                            let mut generation = filter_open_generation;
                            let mut placement = filter_placement;
                            generation += 1;
                            placement.set(PanelPlacement::Unmeasured);
                            filter_open.set(true);
                            measure_filter_popover();
                        }
                    },
                    "filter"
                }
                // A native select keeps the platform's keyboard and assistive
                // behavior. Its accessible name survives without a visible
                // word that would only duplicate the selected order's meaning.
                select {
                    class: "sort-select",
                    aria_label: "sort",
                    value: sort().key(),
                    onchange: move |evt| {
                        // A word this build does not know is ignored rather
                        // than defaulted: every option here is ours, so the
                        // only way to reach that arm is a value nobody
                        // offered, and silently re-sorting to the default
                        // would be a worse answer than doing nothing.
                        if let Some(next) = ListSort::from_key(&evt.value()) {
                            apply_sort(next);
                        }
                    },
                    for (option_sort, option_label) in SORT_OPTIONS {
                        option {
                            key: "{option_sort.key()}",
                            value: "{option_sort.key()}",
                            selected: sort() == option_sort,
                            "{option_label}"
                        }
                    }
                }
            }
        }
        match &*listing.read() {
            None => rsx! { div { class: "status", "loading sessions…" } },
            Some(Err(e)) => rsx! {
                div { class: "status error", "failed to load sessions: {e}" }
            },
            Some(Ok(listing)) => rsx! {
                // The plain empty-fleet line, which is deliberately NOT the
                // same thing as a filter matching nothing — see
                // `rows::is_empty_fleet` for why the request has to be
                // consulted and what taking this branch would suppress.
                if rows::is_empty_fleet(listing) {
                    div { class: "status", "no sessions" }
                } else {
                    // A filter that matched nothing says so in words, beside
                    // the banner's numbers. Without it the page is a count
                    // over an empty box, which reads as a list that failed
                    // to load rather than as a search that found nothing —
                    // and the two call for opposite reactions. The wording
                    // is `rows::no_match_line`'s: categorical only for a
                    // complete listing, scoped to "the sessions that could
                    // be read" under a cut one.
                    if let Some(line) = rows::no_match_line(listing) {
                        div { class: "status filter-empty", "{line}" }
                    }
                    div { class: "session-list",
                        // The rows are the server's listing with this
                        // view's own just-landed renames painted over it,
                        // so a renamed session reads correctly EVERYWHERE
                        // the row shows its title — the row itself, the
                        // delete prompt that quotes it, the rename field
                        // if it is reopened, and the `Session` that
                        // `on_open` carries into the session view.
                        //
                        // NOTE: this div does not itself scroll (it has no
                        // height constraint of its own — `.app-sidebar` is
                        // the real vertical scroller, see `app.css`), so an
                        // `onscroll` handler here would never fire; closing
                        // an open menu on scroll (or resize, or an internal
                        // layout change) is `layout_epoch`'s job instead —
                        // see the `use_effect` near `show_create`'s
                        // declaration.
                        for session in apply_optimistic_renames(&listing.sessions, &renamed.read()) {
                            SessionRow {
                                key: "{session.id}",
                                compact,
                                state: RowState {
                                    error: errors.read().get(&session.id).cloned(),
                                    // The shared token counts as busy for
                                    // every row's action buttons, not just
                                    // this row's own in-flight op: while
                                    // the other pane (or a page operation)
                                    // holds it, `begin_row_op` refuses row
                                    // ops anyway, and the disabled state is
                                    // that refusal made visible.
                                    busy: busy || pending.read().contains(&session.id),
                                    confirming: confirming.read().contains(&session.id),
                                    confirming_archive: confirming_archive
                                        .read()
                                        .contains(&session.id),
                                    confirming_replace: confirming_replace
                                        .read()
                                        .contains(&session.id),
                                    renaming: renaming.read().as_deref()
                                        == Some(session.id.as_str()),
                                    nav_disabled: nav_locked,
                                    menu_open: menu_open.read().as_deref()
                                        == Some(session.id.as_str()),
                                    selected: selected.read().as_deref()
                                        == Some(session.id.as_str()),
                                    locality: session_locality(session.host, local_host),
                                    // Formatted HERE, not in the row: see
                                    // `RowState::activity` for why the
                                    // clock's tick must not reach a row
                                    // whose displayed age has not moved.
                                    activity: ActivityStamp::new(
                                        now_secs,
                                        session.effective_activity(),
                                    ),
                                },
                                rename_draft,
                                on_open: guarded_open,
                                on_clone,
                                on_mark_seen,
                                on_replace,
                                on_confirm_replace: confirm_replace,
                                on_cancel_replace: cancel_replace,
                                on_stop,
                                on_delete,
                                on_confirm_delete: confirm_delete,
                                on_cancel_delete: cancel_delete,
                                on_archive,
                                on_confirm_archive: confirm_archive,
                                on_cancel_archive: cancel_archive,
                                on_rename_start,
                                on_menu_toggle: toggle_menu,
                                on_rename_submit,
                                // The draft is deliberately left alone: the
                                // next open reseeds it from the current row.
                                on_rename_cancel,
                                session,
                            }
                        }
                    }
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A clone (or replace — the two share this predicate, see its own doc)
    /// click is refused by EACH of its five guards independently, and
    /// accepted only when every one of them is clear.
    ///
    /// This pins the regression the guard exists to prevent: the row's own
    /// `disabled`/`aria-disabled` attributes are a render behind the click
    /// that sets them, so a clone queued just ahead of a create submit
    /// claiming the shared token, or just ahead of this row itself entering
    /// a pending stop/delete, a destructive confirmation, or a rename, must
    /// still be caught HERE rather than reaching `on_clone`'s signal writes.
    /// Each case below flips exactly one guard so a future edit that
    /// dropped any single one of them would fail here rather than only
    /// under a real browser.
    #[farhelm_testtrace::test]
    fn a_clone_is_refused_by_any_one_of_its_five_guards() {
        let id = "session-1";
        let empty = HashSet::new();
        let holding = HashSet::from([id.to_string()]);

        assert!(
            !clone_is_refused(false, id, &empty, &empty, &empty, &empty, None),
            "nothing is holding this row, so the clone must proceed"
        );
        assert!(
            clone_is_refused(true, id, &empty, &empty, &empty, &empty, None),
            "the shared page-wide lock alone must refuse it"
        );
        assert!(
            clone_is_refused(false, id, &holding, &empty, &empty, &empty, None),
            "a stop or delete already in flight for THIS row must refuse it"
        );
        assert!(
            clone_is_refused(false, id, &empty, &holding, &empty, &empty, None),
            "an open delete confirmation on THIS row must refuse it"
        );
        assert!(
            clone_is_refused(false, id, &empty, &empty, &holding, &empty, None),
            "an open archive confirmation on THIS row must refuse it"
        );
        assert!(
            clone_is_refused(false, id, &empty, &empty, &empty, &holding, None),
            "an open replace confirmation on THIS row must refuse it"
        );
        assert!(
            clone_is_refused(false, id, &empty, &empty, &empty, &empty, Some(id)),
            "this row's own open rename field must refuse it"
        );
        // A guard keyed to a DIFFERENT row must never refuse this one —
        // the per-row sets are per-row for exactly this reason, and a
        // guard that read them as a single shared flag would block every
        // clone in the list the instant any one row was mid-operation.
        assert!(
            !clone_is_refused(false, id, &empty, &empty, &empty, &empty, Some("other-row")),
            "another row's rename must not block this row's clone"
        );
    }

    /// A reply is refused when it answers a filter that is no longer
    /// applied, however new it is.
    ///
    /// The interleaving is ordinary: a filter is submitted while a read for
    /// the previous one is still walking (a walk is several round trips, and
    /// the previous filter may well have been "none" over a whole fleet).
    /// Ordering alone accepts it — it IS the newest reply — and the page
    /// then shows rows matching A under controls describing B. The failure
    /// is not transient either: if B's own read fails, nothing further is
    /// owed and the mismatch stands until the user acts again.
    #[farhelm_testtrace::test]
    fn a_reply_for_a_filter_that_is_no_longer_applied_is_refused() {
        let mut reads = ReadGate::default();
        // ORDER MATTERS in this test, and it is the half that was wrong
        // first time round: the read that answers the applied filter is
        // started FIRST, so the wrong-filter reply is genuinely newer. A
        // rejection that advanced the gate anyway would then lock the right
        // reply out — and a test that started the right read afterwards
        // could not tell the two implementations apart, because a newer
        // generation wins either way.
        let for_applied_filter = reads.start();
        let for_old_filter = reads.start();

        assert!(
            !accepts_listing(&mut reads, for_old_filter, true, false),
            "the newest reply is still the wrong question's answer"
        );
        assert!(
            accepts_listing(&mut reads, for_applied_filter, true, true),
            "and refusing it left the ordering alone, so the older reply that DOES answer the \
             applied filter still commits"
        );
    }

    /// The ordering half, unchanged: an older success loses, and a failure
    /// newer than what is displayed is still reported.
    ///
    /// Kept here as well as in `ops` because this is the call site that has
    /// to get BOTH admissions right at once — a refactor that folded the
    /// filter check into the success arm alone would leave failures
    /// reporting a read nobody asked for.
    #[farhelm_testtrace::test]
    fn listing_replies_are_ordered_by_generation_within_the_applied_filter() {
        let mut reads = ReadGate::default();
        let older = reads.start();
        let newer = reads.start();

        assert!(accepts_listing(&mut reads, newer, true, true));
        assert!(
            !accepts_listing(&mut reads, older, true, true),
            "an older walk describes a list that has since changed"
        );
        let latest = reads.start();
        assert!(
            accepts_listing(&mut reads, latest, false, true),
            "a failure newer than what is on screen is worth saying"
        );
        assert!(
            !accepts_listing(&mut reads, older, false, true),
            "while one older than it says nothing about the rows now displayed"
        );
    }

    /// A remembered sort preference is honored when this build knows it, and
    /// every other case is the default rather than a failure.
    ///
    /// The three cases are the three ways this value actually arrives. VALID
    /// is the whole point of persisting it. ABSENT is every first visit, and
    /// is why the default has to be a real answer rather than an empty one.
    /// INVALID is the case worth a test of its own: the helm-owned row
    /// outlives the build that validated it — current writes refuse unknown
    /// words, but a build with a different sort vocabulary can have written
    /// the row this one reads — and the helm answers an unrecognized `sort=`
    /// with a 400, so a value passed through unchecked would not sort the
    /// list differently, it would leave the sidebar reading "failed to load
    /// sessions" until someone fixed the row by hand.
    #[farhelm_testtrace::test]
    fn an_unrecognized_stored_sort_falls_back_to_the_default() {
        assert_eq!(decoded_sort(Some("title")), ListSort::Title);
        assert_eq!(decoded_sort(Some("created")), ListSort::Created);
        assert_eq!(decoded_sort(Some("activity")), ListSort::Activity);

        assert_eq!(
            decoded_sort(None),
            ListSort::default(),
            "a client that never chose gets the default order"
        );
        assert_eq!(
            decoded_sort(Some("")),
            ListSort::default(),
            "and so does one whose stored value is empty"
        );
        assert_eq!(
            decoded_sort(Some("most-recent")),
            ListSort::default(),
            "a word from another build must not reach the helm, which would refuse it with a 400"
        );
        assert_eq!(
            decoded_sort(Some("Activity")),
            ListSort::default(),
            "the wire's vocabulary is exact; a near-miss is not a spelling to be forgiving about"
        );
    }

    /// The sidebar offers exactly the three wire orders, spelled the way the
    /// wire spells them, with the default first.
    ///
    /// The words are asserted LITERALLY rather than round-tripped through
    /// `ListSort::from_key`, because a round trip agrees with itself no
    /// matter which words this build invented, and an invented word is a 400
    /// at the helm rather than a list that sorts oddly. `api`'s
    /// `every_listing_request_names_its_order` pins the same three literals
    /// on the request side; between them, an option and the query it produces
    /// cannot drift apart without one of the two failing.
    ///
    /// The default-first half is a presentation contract rather than a
    /// functional one — the select is controlled from `sort`, so the right
    /// option shows selected whatever position it sits in (see
    /// `SORT_OPTIONS`). What the assertion protects is the fallback reading:
    /// a render that lost the binding shows the head of the list, and the
    /// head being the default is what makes that degrade to a control
    /// agreeing with the rows instead of one contradicting them.
    #[farhelm_testtrace::test]
    fn the_offered_orders_are_the_wire_vocabulary_with_the_default_first() {
        assert_eq!(
            SORT_OPTIONS.map(|(sort, _)| sort.key()),
            ["activity", "created", "title"],
            "the offered words are the helm's own, and the default leads them"
        );
        assert_eq!(
            SORT_OPTIONS[0].0,
            ListSort::default(),
            "the head of the list is the order an unbound render would show"
        );
        for (_, label) in SORT_OPTIONS {
            assert!(
                !label.is_empty(),
                "every order must be labelled for the person reading it"
            );
        }
    }

    /// The auto-select fallback picks the newest-created row, which is not
    /// the same as the first row and not the same as the largest `created_at`
    /// naively taken.
    ///
    /// Every case here is one the sidebar can actually be in. DISPLAY ORDER ≠
    /// CREATION ORDER is the whole reason this function exists: under `title`
    /// or `activity` the newest session is wherever its name or its last
    /// output put it. EQUAL STAMPS are ordinary rather than exotic, since
    /// `created_at` has one-second granularity, and the tie has to resolve to
    /// the listing's own head rather than to whichever row an iterator
    /// happened to visit last. ARCHIVED rows are excluded because SPEC.md's
    /// fallback says non-archived and an archived session has no terminal to
    /// open. MISSING `created_at` is an older helm (`Session::created_at`),
    /// where the honest answer is the one this fallback gave before the field
    /// was decoded at all.
    #[farhelm_testtrace::test]
    fn the_fallback_picks_the_newest_created_row_not_the_first_one() {
        // Ids are spelled as words rather than as UUIDs so each assertion
        // reads as "which session won" — the row's position and its stamp
        // are what every case here is about, and a real id would hide both.
        fn row(id: &str, created_at: i64, archived: bool) -> Session {
            Session {
                created_at,
                archived,
                ..crate::list::row::row_specimen(id)
            }
        }

        // Title order: the newest session sits last, exactly where a
        // first-row fallback would miss it.
        let by_title = [
            row("aaa", 300, false),
            row("mmm", 100, false),
            row("zzz", 500, false),
        ];
        assert_eq!(
            newest_created_fallback(&by_title).map(|s| s.id.as_str()),
            Some("zzz")
        );

        // Ties keep the listing's own order. `Iterator::max_by` returns the
        // LAST maximal element, so a naive `max_by` here would answer "third"
        // — and would change its answer as unrelated rows were appended.
        let tied = [
            row("first", 500, false),
            row("second", 100, false),
            row("third", 500, false),
        ];
        assert_eq!(
            newest_created_fallback(&tied).map(|s| s.id.as_str()),
            Some("first"),
            "equal creation stamps must resolve to the listing's own head"
        );

        // Archived rows are not candidates even when they are the newest.
        let with_archived = [row("archived-newest", 900, true), row("live", 100, false)];
        assert_eq!(
            newest_created_fallback(&with_archived).map(|s| s.id.as_str()),
            Some("live")
        );

        // A helm too old to send the field leaves every stamp at zero, which
        // means UNKNOWN. The answer is the first non-archived row, not the
        // one an ordering over zeroes would single out.
        let stampless = [
            row("archived", 0, true),
            row("head", 0, false),
            row("tail", 0, false),
        ];
        assert_eq!(
            newest_created_fallback(&stampless).map(|s| s.id.as_str()),
            Some("head")
        );

        // Mixed: one row does carry a stamp, so the unknowns must not be
        // allowed to beat it by being earlier in the list.
        let mixed = [row("unknown", 0, false), row("stamped", 1, false)];
        assert_eq!(
            newest_created_fallback(&mixed).map(|s| s.id.as_str()),
            Some("stamped"),
            "a real timestamp outranks a missing one wherever the row sits"
        );

        assert_eq!(newest_created_fallback(&[]), None);
        assert_eq!(
            newest_created_fallback(&[row("only-archived", 900, true)]).map(|s| s.id.as_str()),
            None,
            "an all-archived listing offers nothing to auto-select"
        );
    }

    /// The filter's measurement states keep a current open hidden only while
    /// it lacks a rect, then expose either a clamped placement or the usable
    /// fallback. This pins the race contract independently of a renderer.
    #[farhelm_testtrace::test]
    fn filter_popover_placement_keeps_unmeasured_and_fallback_states_distinct() {
        assert_eq!(
            filter_popover_placement_style(PanelPlacement::Unmeasured),
            "opacity: 0; pointer-events: none;"
        );
        let fallback = filter_popover_placement_style(PanelPlacement::Fallback);
        assert!(fallback.contains("opacity: 1; pointer-events: auto"));
        assert!(fallback.contains("max-width: min(288px"));
        assert!(fallback.contains("max-height: calc(100vh - 16px)"));
    }
}

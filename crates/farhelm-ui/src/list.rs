//! The session list: `ListView` (the flat listing, its filter and search
//! surface, and its stop/delete/archive/create/rename actions), `SessionRow`
//! (one row, including the inline lifecycle confirmations and the inline
//! rename field), and
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
//! because two consumers need one read: the panel, and the create dialog's
//! host selector.
//!
//! ## Profiles reach this file in two places (PLAN_M6_75.md item 8)
//!
//! The create dialog gains an agent picker over the target host's catalog,
//! and a row created from a profile names the profile it SNAPSHOTTED. Both
//! rules — what a fresh dialog preselects, and how a snapshot reads once the
//! catalog has moved on — live in `profiles` rather than here, so this file
//! keeps the component, the state and the handlers, exactly as it does for
//! the rename overlay and the count banner.
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
    self, CreateAgent, SessionFilter, SessionListing, archive_session, create_session,
    delete_session, fetch_hosts, fetch_session, fetch_sessions, mint_intent_key, rename_session,
    stop_session,
};
use crate::archive::confirmation as archive_confirmation;
use crate::feed::{fallback_polls_now, fallback_sleep, use_feed_reader};
use crate::hosts::{
    HostsPanel, HostsRead, host_incarnation, is_connected, phase_class, phase_label,
};
use crate::ops::{OpLock, ReadGate};
use crate::peer::{DetailPart, PeerLine, display_peer};
use crate::profiles::{
    AgentChoice, CatalogLookup, CatalogSurface, HostTarget, UNRESOLVED_VALUE, existence_word,
    resolve_agent, seeded_choice, source_profile_label, use_catalog_surface,
};
use crate::reader::{SurfaceReader, Trigger, request_read};
use crate::rename::RenameForm;
use crate::rows::{
    self, absence_is_evidence, apply_optimistic_renames, count_banner, retire_vanished_renames,
    settle_optimistic_renames,
};
use crate::status::{confirm_consequence, status_badge};
use crate::{ApiBase, Host, HostId, HostKind, Session, SessionStatus};

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

/// The display state `ListView` derives for one `SessionRow` render.
///
/// None of these values owns list behavior. Keeping them together makes
/// that boundary explicit: the row decides how to present the current
/// snapshot, while `ListView` remains the owner of every signal and lock
/// the snapshot came from.
#[derive(Clone, PartialEq)]
struct RowState {
    error: Option<String>,
    busy: bool,
    confirming: bool,
    confirming_archive: bool,
    renaming: bool,
    nav_disabled: bool,
    /// Whether this row's actions menu is the (at most one) open one —
    /// the hosts panel's `profiles_open` shape, derived from `ListView`'s
    /// `menu_open` signal.
    menu_open: bool,
}

/// Which ordinary row controls exist for the current retention state.
///
/// Archive removes terminal lifecycle actions, not metadata management:
/// an archived row can still be opened, renamed, or deleted, but cannot be
/// stopped or archived a second time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RowControlVisibility {
    rename: bool,
    stop: bool,
    archive: bool,
    delete: bool,
}

fn row_control_visibility(archived: bool) -> RowControlVisibility {
    RowControlVisibility {
        rename: true,
        stop: !archived,
        archive: !archived,
        delete: true,
    }
}

/// The client's remembered selection, for auto-select on load.
///
/// The interviewed decision (BUGS_BURNDOWN.md issue 5): an empty right
/// pane auto-selects the client's own last-selected session, falling back
/// to the newest-created non-archived one; a placeholder appears only for
/// an empty fleet. Browser builds persist the pair in localStorage —
/// origin-scoped, and additionally keyed by HELM IDENTITY (the local host
/// row's install identity), per the recorded decision: two helms behind
/// one origin (a state-dir swap, a restored backup) must not inherit each
/// other's selection, where an id collision would silently auto-attach an
/// unrelated session.
///
/// Only USER-initiated selections are remembered (row clicks, creation).
/// The auto-select fallback deliberately never writes: a remembered row
/// that happens to sit beyond a TRUNCATED listing's bound must survive
/// that listing, not be overwritten by whichever row the fallback picked.
///
/// Desktop builds remember within the process only: the webview's
/// localStorage is not synchronously reachable from native Rust, and an
/// eval round trip for a best-effort nicety is not worth its failure
/// modes. A fresh desktop launch therefore lands on the newest-created
/// session — the decision's own fallback (SPEC.md says so in as many
/// words; revisit if real desktop use wants more).
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct RememberedSelection {
    helm: String,
    id: String,
}

#[cfg(target_arch = "wasm32")]
fn stored_selection(helm: &str) -> Option<String> {
    let raw = web_sys::window()?
        .local_storage()
        .ok()
        .flatten()?
        .get_item("farhelm.last-selected")
        .ok()
        .flatten()?;
    let stored: RememberedSelection = serde_json::from_str(&raw).ok()?;
    (stored.helm == helm).then_some(stored.id)
}

#[cfg(target_arch = "wasm32")]
fn remember_selection(helm: &str, id: &str) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let record = RememberedSelection {
            helm: helm.to_string(),
            id: id.to_string(),
        };
        if let Ok(raw) = serde_json::to_string(&record) {
            // Best-effort: a full or disabled storage must never break
            // selection itself.
            let _ = storage.set_item("farhelm.last-selected", &raw);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
static LAST_SELECTED: std::sync::OnceLock<std::sync::RwLock<Option<RememberedSelection>>> =
    std::sync::OnceLock::new();

#[cfg(not(target_arch = "wasm32"))]
fn stored_selection(helm: &str) -> Option<String> {
    let slot = LAST_SELECTED.get_or_init(Default::default).read().ok()?;
    slot.as_ref()
        .filter(|record| record.helm == helm)
        .map(|record| record.id.clone())
}

#[cfg(not(target_arch = "wasm32"))]
fn remember_selection(helm: &str, id: &str) {
    if let Ok(mut slot) = LAST_SELECTED.get_or_init(Default::default).write() {
        *slot = Some(RememberedSelection {
            helm: helm.to_string(),
            id: id.to_string(),
        });
    }
}

/// The row menu toggle's accessible name: the session's identity, clamped.
///
/// Every row renders an identical "⋯" toggle, so the NAME is the only
/// thing assistive technology can distinguish the buttons by — an
/// unnamed toggle invites renaming or deleting the wrong session. The
/// clamp exists because a title has no length bound (tens of KB is
/// legal) and an accessible name is read aloud in full; 64 characters
/// is plenty to tell sessions apart, and the ellipsis says something
/// was cut. Char-based, not byte-based, so a multi-byte title can never
/// split a codepoint.
fn menu_label(title: &str) -> String {
    const MAX_CHARS: usize = 64;
    let mut clamped: String = title.chars().take(MAX_CHARS + 1).collect();
    if clamped.chars().count() > MAX_CHARS {
        clamped.truncate(
            clamped
                .char_indices()
                .nth(MAX_CHARS)
                .map_or(clamped.len(), |(i, _)| i),
        );
        format!("session actions for {clamped}…")
    } else {
        format!("session actions for {clamped}")
    }
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
    /// The helm's CONNECTION token for this host (`Host::incarnation`), which
    /// a create hands back as its `expected_incarnation` so the helm can
    /// refuse one prepared against an install that has since been replaced.
    /// Distinct from the fingerprint above: that one describes the registry
    /// ROW, this one the live connection behind it.
    connection: u64,
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

/// What one create would actually LAUNCH.
///
/// The two creation modes are mutually exclusive on the wire (PLAN_M6_75.md
/// item 3) and they are mutually exclusive here for a second reason: they are
/// part of the intent an idempotency key stands for. Keeping the typed
/// command inside the `Command` arm rather than beside a nullable profile is
/// what makes "a profile-backed create does not care what is in the command
/// box" structural — a form field the user cannot reach in that mode can no
/// longer change what the key is bound to.
#[derive(Debug, Clone, PartialEq, Eq)]
enum LaunchIntent {
    /// The invocation as typed into the form.
    Command(String),
    /// A profile from the target host's catalog, by id.
    Profile(String),
}

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
///   `disabled` attributes that make the form inert land one render after the
///   submit, so a keystroke queued at submit time can change a field while the
///   key is being made. The binding is re-read after minting and compared
///   against this, which turns that gap into another mint rather than a key
///   that describes something the user did not submit — the attributes are
///   honesty about a create being in flight, not the guard.
#[derive(Debug, Clone, PartialEq, Eq)]
struct IntentBinding {
    host: HostId,
    /// The target host's incarnation at submit time — see the type docs.
    incarnation: String,
    cwd: String,
    /// What this create launches — see [`LaunchIntent`]. Switching between
    /// the two modes is a different intended create, and so is switching
    /// profiles, which is why the mode lives inside the binding rather than
    /// beside it.
    agent: LaunchIntent,
    title: String,
}

impl IntentBinding {
    /// The binding for a submit, or `None` when there is no host to create
    /// on — the one case a submit is refused locally rather than sent.
    fn of(
        selected: Option<HostId>,
        hosts: &[HostOption],
        cwd: String,
        agent: LaunchIntent,
        title: String,
    ) -> Option<IntentBinding> {
        let host = hosts.iter().find(|host| Some(host.id) == selected)?;
        Some(IntentBinding {
            host: host.id,
            incarnation: host.incarnation.clone(),
            cwd,
            agent,
            title,
        })
    }
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

/// Which host a fresh create dialog should target: the LOCAL row, whatever
/// phase it is in.
///
/// ## Why SPEC.md's first clause is not implemented here
///
/// SPEC.md's default is "the host of the currently open session, else the
/// helm's own host". Both clauses are live: the sidebar layout
/// (BUGS_BURNDOWN.md issue 5) put the create form beside an open session,
/// which is exactly the coexistence an earlier version of this function
/// documented as the condition for restoring the first clause. `open_host`
/// is the SELECTED session's host — the one open right now, never a
/// remembered last-viewed one, because a session the user deselected is
/// not open — and it only wins while that host is still in the registry
/// (an open session whose host was removed must not aim a create at a
/// selector entry that no longer exists).
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
fn default_create_host(hosts: &[HostOption], open_host: Option<HostId>) -> Option<HostId> {
    open_host
        .filter(|open| hosts.iter().any(|host| host.id == *open))
        .or_else(|| hosts.iter().find(|host| host.local).map(|host| host.id))
}

/// The host a create would ACTUALLY go to: the user's choice while it still
/// exists, and [`default_create_host`]'s answer otherwise.
///
/// Extracted because two places need the same answer and a second copy would
/// be the one that drifted: the create dialog renders it (a selector must
/// show the target that would be used, never one that has since left the
/// registry) and `ListView` points the profile catalog's reader at it. A
/// disagreement between those two would put one host's profiles under another
/// host's create — the exact failure `profiles::CatalogRead` refuses to make
/// possible from its side.
fn effective_create_host(
    hosts: &[HostOption],
    chosen: Option<HostId>,
    open_host: Option<HostId>,
) -> Option<HostId> {
    chosen
        .filter(|chosen| hosts.iter().any(|host| host.id == *chosen))
        .or_else(|| default_create_host(hosts, open_host))
}

/// Every registered host as the create dialog and the filter surface offer
/// it.
///
/// A free function over the snapshot rather than an inline `map` in the
/// render, because `ListView`'s target effect needs exactly the same reduction
/// from inside an effect, where the render's local is not in scope. Every host
/// is offered whatever phase it is in — see `ListView` for why filtering to
/// connected ones would quietly rewrite SPEC.md's creation default.
fn host_options(hosts: &[Host]) -> Vec<HostOption> {
    hosts
        .iter()
        .map(|host| HostOption {
            id: host.id,
            name: host.name.clone(),
            local: host.kind == HostKind::Local,
            // Non-connected hosts are labelled with their phase, so choosing
            // one is an informed choice rather than a surprise refusal.
            phase: (!is_connected(&host.state)).then(|| phase_label(&host.state).to_string()),
            incarnation: host_incarnation(host),
            connection: host.incarnation,
        })
        .collect()
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
///   a demand on the surface and the reader will ask again under B.
/// - **Is it the newest word?** The generation gate's ordinary split, with
///   successes and failures gated differently for the reasons `ops` gives.
fn accepts_listing(
    reads: &mut ReadGate,
    generation: u64,
    succeeded: bool,
    answers_applied_filter: bool,
) -> bool {
    if !answers_applied_filter {
        return false;
    }
    if succeeded {
        reads.accept_success(generation)
    } else {
        reads.accept_failure(generation)
    }
}

#[cfg(test)]
std::thread_local! {
    // How often the real row component ran in the callback-memoization
    // regression below. Thread-local because each Dioxus virtual DOM is
    // single-threaded while the Rust test harness runs tests concurrently.
    static SESSION_ROW_RENDERS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
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
/// answering accumulates one walk per notification and per fallback tick for
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
/// ## Two more reads, on demand (PLAN_M6_75.md item 8)
///
/// Profiles add two further surfaces, each with its own reader under exactly
/// the same discipline (`profiles::use_catalog_surface`): the catalog behind
/// the hosts panel's expanded profiles section, and the catalog behind the
/// create dialog's agent picker. They are separate because they answer
/// different questions at the same time — one is about the host whose row is
/// expanded, the other about the host the dialog would create on — and each
/// performs NO PROFILE REQUEST while its target is `None`, which is the state
/// a collapsed section and a closed dialog are in. The surfaces themselves
/// live as long as this page does (their hooks are here); what makes them free
/// is the target, not the mounting.
///
/// Both targets are decided HERE and are `profiles::HostTarget`s — a registry
/// row AND the install behind it — derived against the registry as it
/// currently stands rather than taken from what the user last clicked. That is
/// what makes a mutable host id safe to build on: a retarget or an adopt
/// changes the incarnation, so the target changes, so every surface pointed at
/// it is re-activated instead of continuing to act on a catalog that now
/// belongs to a different supervisor. A removed row clears the target (and
/// folds its section away) rather than leaving a reader retrying a dead id.
///
/// ## Filtering is a query, not a render pass (PLAN_M6_75.md item 7)
///
/// The filter surface builds `api::SessionFilter` and the helm answers with
/// the matching rows plus their count; nothing here narrows a list it was
/// handed. That is the only arrangement coherent with pagination — a
/// client-side filter over one page hides matches beyond the cut while the
/// banner reports a count that includes them.
///
/// The filter is applied on SUBMIT rather than per keystroke, which is a
/// deliberate trade: a live filter over a server-side query is one whole
/// cursor walk per character typed, and the debounce that would make that
/// tolerable is a second cadence to tune in a UI whose entire point this
/// milestone is that it has none left.
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
    /// helm's own host.
    open_host: Option<HostId>,
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
) -> Element {
    let base = use_context::<ApiBase>().0;
    let mut row_ops = row_ops;
    let mut listing = use_signal(|| None::<Result<SessionListing, String>>);
    // The same generation discipline the hosts read has, for the same
    // reason and against a slower race: a listing walk is several round
    // trips, so a read that started before a delete can easily still be
    // walking when the delete's own refresh has already landed — and
    // committing it would put the deleted row back until the next one.
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
    // The two on-demand sidebar sections (BUGS_BURNDOWN.md issue 5): the
    // hosts panel and the filter bar render only while toggled open, so
    // the sidebar's resting state is rows plus the create button. Both
    // default closed on every load, deliberately unpersisted — the compact
    // host strip and the "filtered" note carry the always-visible facts.
    let mut hosts_open = use_signal(|| false);
    let mut filter_open = use_signal(|| false);
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
    // At most one row's actions menu is open, and this parent owns which —
    // the hosts panel's `profiles_open` shape (see that component), copied
    // for the same reason: a per-row boolean would let two menus fight,
    // and the parent is the only place "opening yours closes mine" can
    // live. Defined up here with the other row-scoped UI state because
    // `commit_listing` reconciles it (a menu whose row left the listing
    // must not reappear, already open, when the row comes back).
    let mut menu_open = use_signal(|| None::<String>);
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
    // The create dialog's explicit host choice, and which host row has its
    // profiles section expanded (PLAN_M6_75.md item 8).
    //
    // Both live HERE rather than in the surfaces that show them, and for the
    // same reason: each one decides which host a profile CATALOG is read for,
    // and the reads on this page go through readers this component owns. A
    // choice held inside the dialog would be invisible to the reader that has
    // to follow it, and the two would drift into a picker offering one host's
    // profiles for a create aimed at another.
    let mut chosen_host = use_signal(|| None::<HostId>);
    let mut profiles_open = use_signal(|| None::<HostId>);
    // The filter the reads are currently CARRYING, and the one the surface
    // is being edited into. Two signals rather than one because a filter is
    // applied on submit (see this component's docs): the draft changes with
    // every keystroke, and the applied one changes only when a read is
    // actually asked for — so a re-read triggered by the feed mid-edit uses
    // the filter whose results are on screen rather than a half-typed one.
    let mut filter = use_signal(SessionFilter::default);
    let mut filter_draft = use_signal(SessionFilter::default);

    // The two surfaces' readers (`reader::SurfaceReader`): one reader each,
    // coalescing every trigger into a single live read and retrying one that
    // failed to answer. Separate, because the two reads are independent —
    // a hanging `/api/hosts` must not hold the session list off the screen,
    // which is the same independence the mount reads keep below.
    let listing_surface = use_signal(SurfaceReader::default);
    let hosts_surface = use_signal(SurfaceReader::default);

    // The two PROFILE catalogs this page can be showing, each with its own
    // one-door reader (`profiles::use_catalog_surface`, which wires the same
    // mount / feed / fallback / retarget triggers the reads above run under).
    //
    // Two surfaces rather than one, because they answer different questions
    // at the same time: the hosts panel's section is about the host whose row
    // is expanded, and the create dialog's picker is about the host that
    // dialog would create on. Collapsing them would make opening a profiles
    // section silently re-point the picker beside it. Neither performs any
    // profile request while its target is `None`, which is the state a
    // collapsed section and a closed dialog are in.
    //
    // Both targets are `profiles::HostTarget`s — a registry row AND the
    // install behind it — and both are DERIVED here, against the registry as
    // it currently stands, rather than taken from what the user last clicked.
    // That is the whole answer to a host id being mutable: a retarget or an
    // adopt changes the incarnation, the target changes with it, and every
    // surface pointed at it is re-activated instead of continuing to act on a
    // catalog that belongs to another machine.
    let mut panel_target = use_signal(|| None::<HostTarget>);
    use_effect(move || {
        let open = profiles_open();
        let read = hosts.read();
        let registry = read.hosts();
        let wanted = open.and_then(|id| {
            registry
                .unwrap_or_default()
                .iter()
                .find(|host| host.id == id)
                .map(HostTarget::of)
        });
        // A row that has LEFT the registry takes its section with it. Nothing
        // else would: the id is dead, so every read against it 404s forever
        // and the section would sit under a row that is not there, retrying.
        // Only acted on once a registry is actually in hand — a first read
        // that has not landed is not evidence that anything was removed.
        if open.is_some() && wanted.is_none() && registry.is_some() {
            profiles_open.set(None);
        }
        if *panel_target.peek() != wanted {
            panel_target.set(wanted);
        }
    });
    let host_profiles = use_catalog_surface(panel_target);

    // Which install the create dialog's picker is about. Derived rather than
    // chosen: it follows `effective_create_host`, so a chosen host leaving
    // the registry re-points this too — and it is `None` whenever the dialog
    // is closed, which is what stops it reading.
    let mut create_target = use_signal(|| None::<HostTarget>);
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
                let effective = effective_create_host(&options, chosen_host(), open_host);
                options
                    .into_iter()
                    .find(|host| Some(host.id) == effective)
                    // Both halves come off the OPTION: the fingerprint the
                    // create's idempotency key is bound to, and the connection
                    // its precondition names — one derivation each, so the
                    // catalog the picker offers, the machine the key names and
                    // the connection the request asserts cannot disagree.
                    .map(|host| HostTarget::new(host.id, host.connection, host.incarnation))
            })
            .flatten();
        // Compared before writing so an unrelated hosts refresh — the common
        // case, several times a minute on a live fleet — does not restart the
        // catalog read for a target that has not moved.
        if *create_target.peek() != wanted {
            create_target.set(wanted);
        }
    }));
    let create_catalog = use_catalog_surface(create_target);

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
    // `requested` is the filter this reply ANSWERS, sampled where the
    // request was issued. Ordering alone is not enough to make a reply
    // usable: the gate knows that read A started before read B, but not that
    // B asked a different question. A read for filter A completing after
    // filter B was applied would paint A's rows under controls describing B
    // — indefinitely, if B's own read failed — so a reply whose filter is no
    // longer the applied one is refused outright. The user's next move
    // (submitting, clearing) is a read of its own, and the surface reader
    // has already recorded the submit's own demand, so nothing is lost by
    // dropping this one.
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
            *filter.peek() == requested,
        );
        if !accepted {
            return;
        }
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
            // The right pane's placeholder may only claim "no active
            // sessions" on a committed result that actually proves one:
            // the DEFAULT view's own reply, complete and coherent, with no
            // rows. Not `rows::is_empty_fleet` — that helper answers a
            // different question (is the WHOLE fleet empty), and since the
            // ordinary exclude-archived view is itself an active
            // server-side predicate it would never say yes here. An
            // archived-only fleet lists nothing in the default view, and
            // "no ACTIVE sessions" is exactly right for it. A user filter
            // proves nothing about the pane and leaves the verdict as it
            // was.
            if requested == SessionFilter::default() && !listing.truncated && !listing.incoherent {
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
        // Read before incrementing, so `index` is this read's own position
        // in the view's read order — what tells an optimistic rename whether
        // this reply is late enough to be evidence about it. Claimed even
        // for a filtered read, so the order stays a single sequence; what a
        // filtered read does not get is AUTHORITY over absence (see
        // `commit_listing`).
        let index = poll_sequence.peek().to_owned();
        poll_sequence += 1;
        let authoritative = !requested.is_active();
        let generation = listing_reads.write().start();
        async move {
            let fetched = fetch_sessions(&base, &requested).await;
            let answered = fetched.is_ok();
            commit_listing(generation, requested, fetched, index, authoritative);
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
    // filter submit or a mutation's refresh is answered, because the page
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
    // cannot predict — an add's chip is whatever the connection finds, a
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

    // Apply the filter surface's draft: swap it in, then read with it.
    //
    // The read is asked for HERE rather than left to the feed or the
    // fallback, and that is the whole reason a filter feels like a filter:
    // nothing else is going to happen — the fleet did not change, so no
    // revision is coming, and the fallback is not running on a healthy feed.
    // A submit that only updated state would leave the list exactly as it
    // was until something unrelated changed.
    //
    // A read already walking under the OLD filter is left to finish and be
    // refused by `commit_listing`; the demand recorded here is what makes
    // the follow-up read carry the new one.
    let filter_read = request_listing.clone();
    let apply_filter = move |next: SessionFilter| {
        filter.set(next.clone());
        filter_draft.set(next);
        filter_read(Trigger::Explicit);
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
                    // Claimed like any other read, so the read order stays
                    // one sequence: an optimistic rename asks whether a
                    // reply STARTED after it, and a read outside the
                    // numbering could not answer.
                    let index = poll_sequence.peek().to_owned();
                    poll_sequence += 1;
                    let fetched = fetch_sessions(&base, &snapshot).await;
                    let failed = fetched.is_err();
                    // Not authoritative: this read speaks for one session's
                    // status, so a session missing from it is not a session
                    // that left — see `commit_listing`. It still settles the
                    // renames its own rows agree or disagree with.
                    commit_listing(generation, snapshot, fetched, index, false);
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
                    // else — and a title is exactly what a filter can be ON.
                    // A row renamed OUT of an active title search stays on
                    // screen under a query it no longer matches, and the
                    // counts beside it still describe the old name, until
                    // something re-reads. Normally the feed does; under a
                    // latched build mismatch nothing does, so this explicit
                    // read is the correction.
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
    let guarded_open = move |session: Session| {
        if ops.busy_now() || !pending.peek().is_empty() {
            return;
        }
        // A USER-initiated selection is what gets remembered — the
        // auto-select fallback deliberately never writes (see
        // `stored_selection`). Best-effort: no identity yet (hosts read
        // still out) just means this click is not recorded.
        if let Some(helm) = hosts
            .peek()
            .hosts()
            .unwrap_or_default()
            .iter()
            .find(|host| host.kind == HostKind::Local)
            .and_then(|host| host.identity.clone())
        {
            remember_selection(&helm, &session.id);
        }
        on_open.call(session);
    };

    // Stable callback identities are created once, outside the row loop.
    // The row-specific session or id travels as the callback argument, so
    // no hook depends on fleet size and unchanged rows remain memoized when
    // their parent refreshes.
    let guarded_open = use_callback(guarded_open);
    // Auto-select (BUGS_BURNDOWN.md issue 5, interviewed): an empty right
    // pane is a state to END, not to show — the remembered selection if
    // its row is still listed, else the newest-created non-archived row
    // (the merged listing is created_at DESCENDING, so that is the first
    // match). Everything consulted is a TRACKED read, deliberately: the
    // effect must rerun when the listing commits, when the selection
    // clears, when the hosts read lands (the remembered id is keyed by
    // helm identity, which comes from the local host row), and when the
    // write gates drain — an auto-select refused during a busy operation
    // would otherwise be lost until unrelated fleet activity.
    //
    // A remembered id ABSENT from a TRUNCATED listing is not evidence it
    // is gone (the walk stopped early); it is resolved directly instead,
    // and only a definite not-found retires it in favor of the fallback.
    // The fallback itself deliberately does not re-persist: overwriting
    // the user's real choice with whichever row a bounded walk happened
    // to return first would erase it permanently (see `stored_selection`).
    let resolve_base = base.clone();
    let mut resolving_remembered = use_signal(|| false);
    let mut remembered_dead = use_signal(|| None::<String>);
    use_effect(move || {
        if selected.read().is_some() {
            return;
        }
        let page_busy = ops.busy();
        let rows_busy = *row_ops.read() > 0;
        let hosts_read = hosts.read();
        let listing_read = listing.read();
        let Some(Ok(listing_ok)) = listing_read.as_ref() else {
            return;
        };
        let Some(helm) = hosts_read
            .hosts()
            .unwrap_or_default()
            .iter()
            .find(|host| host.kind == HostKind::Local)
            .and_then(|host| host.identity.clone())
        else {
            return;
        };
        if page_busy || rows_busy {
            return;
        }
        // A TRACKED read: retiring the remembered id (the 404 arm below)
        // must rerun this effect so the fallback can proceed.
        let remembered =
            stored_selection(&helm).filter(|id| remembered_dead.read().as_ref() != Some(id));
        let in_page = listing_ok
            .sessions
            .iter()
            .find(|s| remembered.as_deref() == Some(s.id.as_str()));
        if in_page.is_none()
            && let Some(id) = remembered.clone()
            && listing_ok.truncated
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
        let candidate = in_page.or_else(|| listing_ok.sessions.iter().find(|s| !s.archived));
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
    });
    let on_stop = use_callback(on_stop);
    let on_delete = use_callback(on_delete);
    let confirm_delete = use_callback(confirm_delete);
    let cancel_delete = use_callback(cancel_delete);
    let on_archive = use_callback(on_archive);
    let confirm_archive = use_callback(confirm_archive);
    let cancel_archive = use_callback(cancel_archive);
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

    // Whether the applied filter differs from the public default, for the
    // clear control and empty-result line. The default still excludes
    // archived rows, so `false` means "the documented default view", not
    // "no predicate is active". Read from the applied filter rather than
    // the draft because typed-but-unsubmitted text has changed no results.
    let filter_changed = *filter.read() != SessionFilter::default();
    // The selector's own copy: the create form takes ownership of
    // `host_options` further down, and both surfaces want the same list —
    // the same hosts, called the same things, with the same phase labels.
    let filter_hosts = host_options.clone();
    // The host this filter names that the registry no longer carries, if
    // any — a host removed from another client while a filter on it was
    // applied. Derived from the DRAFT because that is what the select's
    // value is bound to, and only once a registry read has landed: an empty
    // list before the first read would make every id look removed.
    let removed_filter_host = filter_draft.read().host.filter(|id| {
        hosts.read().hosts().is_some() && !filter_hosts.iter().any(|host| host.id == *id)
    });
    // Two handles on one closure, because a closure is not `Copy` once it
    // captures the API base: submit and clear are the same operation with
    // different arguments, not two operations.
    let mut submit_filter = apply_filter.clone();
    let mut clear_filter = apply_filter;

    rsx! {
        // The sidebar leads with two on-demand toggles (BUGS_BURNDOWN.md
        // issue 5, interviewed): the sidebar's permanent contents are the
        // session rows and the create button ONLY, with host management
        // and filtering opened when needed instead of permanently stacked
        // above the list. The compact host strip below the toggles is what
        // keeps SPEC.md's per-host visibility promise while the full panel
        // is closed.
        div { class: "sidebar-controls",
            // Closing NEVER unmounts the panel (it collapses via CSS, see
            // the holder below), so the toggle needs no busy guard: every
            // in-flight discovery, plan, or host mutation keeps its
            // component — and therefore its reply handling — alive behind
            // the collapsed surface, and provisioning-busy state survives
            // a close/reopen instead of flashing controls re-enabled.
            button {
                r#type: "button",
                class: "btn hosts-toggle",
                aria_expanded: hosts_open(),
                onclick: move |_| {
                    let opening = !hosts_open();
                    hosts_open.set(opening);
                    // A closed panel takes its profiles section with it:
                    // leaving `profiles_open` set would keep that host's
                    // catalog surface reading behind an invisible panel.
                    if !opening {
                        profiles_open.set(None);
                    }
                },
                "hosts"
            }
            button {
                r#type: "button",
                class: "btn filter-toggle",
                aria_expanded: filter_open(),
                onclick: move |_| filter_open.set(!filter_open()),
                "filter"
            }
            // An applied non-default filter narrows the list even while
            // the bar is closed; saying so beside the toggle is what keeps
            // a hidden filter from reading as a shrunken fleet.
            if filter_changed {
                span { class: "filter-active-note", "filtered" }
            }
        }
        // The always-visible compact form of the hosts panel: SPEC.md
        // requires per-host connection state (and the retry phase behind
        // it) to stay visible, and the interviewed redesign relaxed that
        // from the full management panel to this strip — one entry per
        // host, named, with the SAME phase word and color category the
        // full panel's chip uses (`hosts::phase_label`/`phase_class`), so
        // the two surfaces can never call one state two things.
        // Management (retry, edit, remove, profiles) stays in the full
        // panel behind the toggle.
        div { class: "hosts-compact",
            // The strip mirrors `HostsRead`'s four states, not just its
            // happy path: this is the surface SPEC.md's always-visible
            // promise now rests on, and a strip that renders only
            // `hosts()` would show a failed first read as an empty fleet
            // and a failed REFRESH as confidently current chips. The
            // wording mirrors the full panel's own loading/error lines.
            if hosts.read().is_loading() {
                span { class: "hosts-compact-note", "loading hosts…" }
            }
            for host in hosts.read().hosts().unwrap_or_default() {
                span {
                    key: "{host.id}",
                    class: "hosts-compact-entry",
                    span { class: "hosts-compact-name peer-value", dir: "ltr",
                        "{display_peer(&host.name)}"
                    }
                    span {
                        class: "host-chip {phase_class(&host.state)}",
                        "{phase_label(&host.state)}"
                    }
                }
            }
            // A failed refresh demotes everything above to last-known —
            // said HERE, where the chips are, not only inside the closed
            // panel. The helm's own words ride along like every other
            // peer string.
            if let Some(err) = hosts.read().refresh_error().map(str::to_string) {
                PeerLine {
                    class: "hosts-compact-error".to_string(),
                    parts: vec![
                        DetailPart::Text(
                            if hosts.read().hosts().is_some() {
                                "shown as of last successful read; refresh failed: ".to_string()
                            } else {
                                "hosts could not be read: ".to_string()
                            },
                        ),
                        DetailPart::Peer(err),
                    ],
                }
            }
        }
        // Mounted permanently, collapsed via CSS: an unmounted panel
        // would take every in-flight add-host discovery, provisioning
        // plan, and host mutation task down with it (their replies
        // silently discarded, a claimed token stranded), and would
        // forget `provisioning_busy_hosts` so a reopen briefly offered
        // mutations on a host mid-provisioning.
        div {
            class: if hosts_open() { "hosts-panel-holder" } else { "hosts-panel-holder collapsed" },
            HostsPanel {
                hosts,
                ops,
                mutation_busy_hosts,
                provisioning_busy_hosts,
                profiles_open,
                profiles: host_profiles,
                on_changed: refresh_hosts,
            }
        }
        // Filtering and search (PLAN_M6_75.md item 7): this builds the
        // QUERY, the helm answers it, and `rows::count_banner` says how many
        // matched. Nothing here narrows a list that was already fetched —
        // see this component's docs for why a client-side filter cannot be
        // coherent with pagination.
        //
        // A `form` rather than loose inputs, and that is what makes Enter
        // work from any field: applying a filter is a submit, so the keyboard
        // path and the button are the same path rather than two.
        if filter_open() {
        form {
            class: "session-filter",
            onsubmit: move |evt| {
                evt.prevent_default();
                // Bind before calling: `submit_filter` writes `filter_draft`
                // (apply swaps the draft in as the applied filter), and the
                // temporary read guard from an inline `peek()` would live to
                // the end of the whole call statement — a guaranteed
                // AlreadyBorrowed panic, not a race. The `let` ends the
                // guard's life before the call starts.
                let next = filter_draft.peek().clone();
                submit_filter(next);
            },
            label {
                "host"
                select {
                    class: "filter-host",
                    // The empty value is "any host", not "no host": absence
                    // is what an unfiltered dimension looks like on the wire,
                    // so it is what the blank option has to produce.
                    value: filter_draft.read().host.map(|id| id.to_string()).unwrap_or_default(),
                    onchange: move |evt| {
                        filter_draft.write().host = evt.value().parse::<HostId>().ok();
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
                        selected: filter_draft.read().host.is_none(),
                        "any host"
                    }
                    for host in filter_hosts.iter() {
                        option {
                            key: "{host.id}",
                            value: "{host.id}",
                            selected: filter_draft.read().host == Some(host.id),
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
                            // renders while the draft names this host, and
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
                    value: "{filter_draft.read().status}",
                    onchange: move |evt| filter_draft.write().status = evt.value(),
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
                    value: "{filter_draft.read().parent}",
                    oninput: move |evt| filter_draft.write().parent = evt.value(),
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
                    value: "{filter_draft.read().directory}",
                    oninput: move |evt| filter_draft.write().directory = evt.value(),
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
                    value: "{filter_draft.read().profile}",
                    oninput: move |evt| filter_draft.write().profile = evt.value(),
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
                    value: "{filter_draft.read().title}",
                    oninput: move |evt| filter_draft.write().title = evt.value(),
                }
            }
            label { class: "filter-archived",
                input {
                    r#type: "checkbox",
                    class: "filter-include-archived",
                    checked: filter_draft.read().include_archived,
                    onchange: move |evt| {
                        filter_draft.write().include_archived = evt.checked();
                    },
                }
                "include archived"
            }
            button { r#type: "submit", class: "btn filter-apply", "filter" }
            button {
                r#type: "button",
                class: "btn filter-clear",
                // Inert while nothing is applied, so the control cannot
                // offer to undo something that never happened. Cosmetic
                // only, like every other `disabled` on this page — the
                // handler clears an already-empty filter harmlessly.
                disabled: !filter_changed,
                onclick: move |_| clear_filter(SessionFilter::default()),
                "clear"
            }
        }
        }
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
                    let opening = !show_create();
                    if !opening {
                        // Closing the dialog discards its host choice with
                        // every other draft it holds. The signal lives up here
                        // (the catalog reader has to follow it), which is
                        // exactly why it would otherwise be the one piece of
                        // form state that survived a cancel — and SPEC.md's
                        // creation default is about a FRESH dialog, not about
                        // where the last one was pointed.
                        chosen_host.set(None);
                    }
                    show_create.set(opening);
                },
                "new session"
            }
        }
        if show_create() {
            CreateSessionForm {
                hosts: host_options,
                open_host,
                hosts_loaded: hosts.read().hosts().is_some(),
                chosen_host,
                catalog: create_catalog,
                ops,
                on_created: move |session: Session| {
                    // Creation is a user-initiated selection too.
                    if let Some(helm) = hosts
                        .peek()
                        .hosts()
                        .unwrap_or_default()
                        .iter()
                        .find(|host| host.kind == HostKind::Local)
                        .and_then(|host| host.identity.clone())
                    {
                        remember_selection(&helm, &session.id);
                    }
                    show_create.set(false);
                    // The other close path. This component STAYS mounted
                    // under the sidebar layout, so without this clear the
                    // next open of the dialog would silently reopen on the
                    // last create's host.
                    chosen_host.set(None);
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
                // The plain empty-fleet line, which is deliberately NOT the
                // same thing as a filter matching nothing — see
                // `rows::is_empty_fleet` for why the request has to be
                // consulted and what taking this branch would suppress.
                if rows::is_empty_fleet(listing) {
                    div { class: "status", "no sessions" }
                } else {
                    // The count ALWAYS renders, and which of its four
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
                    // A filter that matched nothing says so in words, beside
                    // the banner's numbers. Without it the page is a count
                    // over an empty box, which reads as a list that failed
                    // to load rather than as a search that found nothing —
                    // and the two call for opposite reactions. What counts
                    // as "nothing" is the helm's own matching count rather
                    // than the emptiness of this page's rows; see
                    // `rows::no_matches` for the two cases that distinction
                    // keeps the UI honest about.
                    if rows::no_matches(listing) {
                        div { class: "status filter-empty",
                            "no sessions match this filter"
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
                                    renaming: renaming.read().as_deref()
                                        == Some(session.id.as_str()),
                                    nav_disabled: nav_locked,
                                    menu_open: menu_open.read().as_deref()
                                        == Some(session.id.as_str()),
                                },
                                rename_draft,
                                on_open: guarded_open,
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
/// `Session` from the response body; `ListView` uses that to close the
/// form and select the new session in the adjacent pane, whose terminal
/// mounts immediately (SPEC.md: "creation launches the agent; you type
/// your first prompt into its terminal") — the sidebar itself stays
/// mounted throughout. On failure the form stays mounted with its values
/// untouched and the error text rendered next to it — the fields are
/// plain `use_signal<String>`s rather than being reset or lifted into
/// `ListView`, so "form contents preserved" falls out of simply not
/// clearing them rather than needing a restore step. On success the
/// fields are left as-is too: `on_created` closes this form (unmounting
/// it and its field signals) in the same frame, so there is no one left
/// to observe a reset — only the failure path needs to leave the control
/// usable again.
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
/// What makes that lifecycle a rule rather than a race is the SUBMIT PATH's
/// own discipline, not the disabled attributes: the agent, the host and the
/// text fields are resolved synchronously when the button is pressed and
/// frozen across the minting await, and the binding is re-read afterwards so
/// a keystroke that landed during it produces another mint rather than a key
/// describing values nobody submitted. The inputs are disabled too, and that
/// is worth having — an inert form is honest about a create being in flight —
/// but it is cosmetic in the way every `disabled` on this page is: the
/// attribute lands one render after the event that set it, so anything queued
/// in that gap still reaches the handler.
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
///
/// ## The agent picker (PLAN_M6_75.md item 8)
///
/// The dialog offers the TARGET host's profiles and defaults to the one a
/// session was last created from there, asking rather than guessing when that
/// profile is gone — `profiles::resolve_agent` owns both halves of that rule
/// and is where the reasoning lives. Two consequences show up here:
///
/// - Changing the host CLEARS the profile choice. A profile id is minted per
///   supervisor and every fresh supervisor seeds the same starters, so an id
///   carried across would not merely go stale — it would resolve over there,
///   against a profile nobody chose.
/// - The command field is disabled while a profile is selected, because the
///   two creation modes are mutually exclusive on the wire and a body naming
///   both is refused. Disabling it is also what keeps the intent binding
///   honest: a field the user cannot reach cannot change what the key stands
///   for.
///
/// The raw command path stays on the dialog rather than being replaced. It is
/// what runs anything no profile describes, and the e2e harness's own creates
/// go through it — a dialog that only offered profiles would make an ad-hoc
/// command a trip to `curl`.
#[component]
fn CreateSessionForm(
    hosts: Vec<HostOption>,
    /// See `ListView`'s parameter of the same name: the selected session's
    /// host, SPEC.md's first create-default clause.
    open_host: Option<HostId>,
    /// Whether the hosts read has EVER succeeded. Distinguishes "there are
    /// no hosts" (impossible for a live helm, which always has its local
    /// row) from "nothing has come back yet", which is what a submit has to
    /// be refused for.
    hosts_loaded: bool,
    /// The user's explicit host choice, if they have made one. `None` means
    /// "no choice yet", not "no host" — the effective target is
    /// [`effective_create_host`]'s answer, recomputed per render against the
    /// hosts that exist right now.
    ///
    /// `ListView`'s signal rather than this form's, because the profile
    /// catalog is read for whatever this names and the reads on that page go
    /// through readers it owns (see `ListView`).
    mut chosen_host: Signal<Option<HostId>>,
    /// The target host's profile catalog, read by `ListView` and rendered
    /// here. Pointed at [`effective_create_host`]'s answer, so the picker and
    /// the create can never describe different machines.
    catalog: CatalogSurface,
    /// The page's live-operation token. Claimed at submit, released when the
    /// request completes — the exclusion against every host mutation, and
    /// against a second submit of this form (see `ops`).
    mut ops: OpLock,
    on_created: EventHandler<Session>,
) -> Element {
    let base = use_context::<ApiBase>().0;
    // Prefilled rather than empty-with-a-placeholder, deliberately: what
    // gets sent is always exactly what the field shows, and the common
    // "just give me a session in my home directory" create needs no typing
    // at all. `~` resolves against the TARGET host's home — the supervisor
    // expands it at create time (SPEC.md's working-directory rule), which
    // is what makes a host-independent default possible here at all: this
    // form cannot know a remote host's home path.
    let mut cwd = use_signal(|| "~".to_string());
    let mut invocation = use_signal(String::new);
    let mut title = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    // The agent this dialog is going to use, once anything has decided it —
    // the user picking, or the host's remembered default being CONSUMED (see
    // the effect below). `None` means nobody has decided yet.
    let mut chosen_profile = use_signal(|| None::<AgentChoice>);
    // Whether an explicit choice has been overtaken by reality. Derived per
    // render rather than written back into `chosen_host`, so it cannot
    // outlive the condition that produced it — and so a host that comes back
    // (a re-added destination) silently reinstates the user's choice.
    let choice_vanished =
        chosen_host().is_some_and(|chosen| !hosts.iter().any(|host| host.id == chosen));
    let selected = effective_create_host(&hosts, chosen_host(), open_host);
    // This form's current intended create, if one has been submitted yet
    // (PLAN_M3.md item 6), together with the BINDING it was minted for.
    // Minted at first submit, reused by every later submit of the same
    // intent, and superseded the moment any part of that binding changes.
    let mut intent_key = use_signal(|| None::<(String, IntentBinding)>);
    let busy = ops.busy();

    // The agent choice is bound to an INSTALL, and this is what binds it.
    //
    // Two rules, and both are about not letting a decision outlive the thing
    // it was made about:
    //
    // - A target change — the user picking another host, a chosen host
    //   leaving the registry and the default taking over, or the SAME row
    //   being retargeted, adopted or reconnected — discards the choice and the
    //   intent key. A profile id means nothing on another supervisor, and
    //   because every fresh supervisor seeds the same starters, carrying one
    //   across does not fail loudly: it resolves, to a profile nobody picked.
    // - The remembered default is CONSUMED ONCE per target, and `seeded_for`
    //   is what records that it has been. Tracking consumption by "a choice
    //   exists" is not enough and the gap is reachable: a first catalog with
    //   NO remembered default, or one whose default was already deleted,
    //   leaves no choice behind — so a later refresh would seed from whatever
    //   the helm remembers BY THEN, and another client's create would move the
    //   selection under an open dialog. Latching the first answer, whatever it
    //   was, is what makes the default a decision this dialog made rather than
    //   a value it follows.
    let mut bound_target = use_signal(|| None::<HostTarget>);
    let mut seeded_for = use_signal(|| None::<HostTarget>);
    use_effect(move || {
        let target = catalog.watch_target();
        let read = catalog.catalog.read();
        let previous = bound_target.peek().clone();
        if previous != target {
            bound_target.set(target.clone());
            // The catalog must be re-seeded for ANY change, including a mere
            // reconnection: it was read on a connection that is gone, and the
            // helm now refuses reads that name it.
            seeded_for.set(None);
            // The CHOICE and the KEY, however, rotate only when the INSTALL
            // changes. A reconnection to the same machine is precisely when a
            // reply gets lost, and that is the case the idempotency key exists
            // for — rotating it there would turn the user's retry into a
            // second intended create and hand them two sessions for one press.
            // A retarget or an adoption is the opposite and must rotate: the
            // id now means another machine, where the key dedups against
            // nothing and the profile id resolves to something else.
            let same_install = match (&previous, &target) {
                (Some(before), Some(now)) => before.same_install(now),
                // Opening the dialog (None -> Some) and closing it are not
                // reconnections; a fresh dialog starts fresh.
                _ => false,
            };
            if !same_install {
                chosen_profile.set(None);
                intent_key.set(None);
                // A refusal belongs to the host it came from. Left standing,
                // host A's "directory does not exist" would sit under a form
                // now aimed at host B, where it may not even be true.
                error.set(None);
            }
        }
        // Consumption is only meaningful once there IS a target: on the first
        // render both are `None`, and reading that equality as "already
        // seeded" would make an unread catalog indistinguishable from a
        // deleted remembered profile — the dialog would open claiming
        // something was gone before it had asked anything.
        if target.is_some() && *seeded_for.peek() == target {
            return;
        }
        // Only a catalog that answers the CURRENT question may seed a choice
        // — `lookup` is what refuses one belonging to a previous activation
        // or another install.
        let CatalogLookup::Known { catalog: held, .. } = catalog.lookup(&read) else {
            return;
        };
        seeded_for.set(target);
        // The user may have answered while the read was in flight (picking a
        // profile, or typing a command); an answer outranks a default.
        if chosen_profile.peek().is_some() {
            return;
        }
        // Three outcomes, decided in one place (`profiles::seeded_choice`):
        // the remembered profile, the command path where nothing was ever
        // remembered, or NO choice where the remembered one is gone — which is
        // what leaves the dialog blocked and asking, told apart from "not read
        // yet" by the latch this effect just set.
        if let Some(choice) = seeded_choice(held) {
            chosen_profile.set(Some(choice));
        }
    });

    // What the picker may offer: this surface's catalog, and nothing at all
    // until one has been read for the question it is asking right now
    // (`CatalogSurface::lookup` refuses anything else, which is what stops
    // one install's profiles being offered for a create aimed at another).
    let catalog_read = catalog.catalog.read();
    let held = catalog.lookup(&catalog_read);
    let offered = match &held {
        CatalogLookup::Known { catalog, .. } => Some(*catalog),
        _ => None,
    };
    // Seeded only once a concrete target has been answered — see the effect
    // above for why `None == None` must not read as consumption.
    let seeded = bound_target.read().is_some() && *seeded_for.read() == *bound_target.read();
    let agent = resolve_agent(chosen_profile.read().as_ref(), offered, seeded);
    let by_profile = matches!(agent.choice, Some(AgentChoice::Profile(_)));
    // Owned, because the picker's options compare against it inside a loop
    // that also borrows the catalog guard this selection was derived from.
    // The placeholder's value stands in for "nothing is selected", which is a
    // state this dialog can genuinely be in — see `profiles::resolve_agent`.
    let chosen_agent = agent
        .choice
        .as_ref()
        .map(|choice| choice.value().to_string())
        .unwrap_or_else(|| UNRESOLVED_VALUE.to_string());

    // What a submit would launch, resolved SYNCHRONOUSLY inside the handler
    // from the live signals and the catalog as it stands at that instant —
    // never from a value the last render happened to compute.
    //
    // The distinction is one JavaScript turn wide and it decides what runs: a
    // change to the picker followed by a submit in the same turn reaches the
    // handler before any re-render, so a captured render-time value would send
    // the PREVIOUS selection under a freshly minted key — a key that faithfully
    // describes an intent nobody had. What is frozen is this resolution's
    // result, held across the minting await (see the submit path).
    let resolve_now = move || {
        let read = catalog.catalog.peek();
        let offered = match catalog.lookup(&read) {
            CatalogLookup::Known { catalog, .. } => Some(catalog),
            _ => None,
        };
        let seeded = bound_target.peek().is_some() && *seeded_for.peek() == *bound_target.peek();
        resolve_agent(chosen_profile.peek().as_ref(), offered, seeded).choice
    };

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
                // No agent, no create. "Nothing is selected" is a real state
                // rather than a gap to be filled — a profile that was chosen
                // or remembered and has since been deleted leaves the dialog
                // waiting for an answer, and the command field it would
                // otherwise fall back to still holds whatever was typed into
                // it earlier. Launching that would run something nobody
                // picked while the note beside it said nothing was selected.
                let Some(choice) = resolve_now() else {
                    error.set(Some(
                        "no agent is selected for this create — choose a profile, or choose \
                         \"custom command\" to run the command below"
                            .to_string(),
                    ));
                    ops.release();
                    return;
                };
                // Frozen HERE, from what was just resolved, and not touched
                // again: the minting await below can span a deletion or
                // another client's remembered-default write, and re-resolving
                // across it would let the request's MODE differ from the one
                // the button was pressed on. A profile that goes away in that
                // window is refused by the supervisor, by name.
                let launch = match choice {
                    AgentChoice::Command => LaunchIntent::Command(invocation.peek().clone()),
                    AgentChoice::Profile(id) => LaunchIntent::Profile(id),
                };
                // The HOST is derived here too, from the live signal — never
                // from what the last render computed. The same one-turn window
                // the agent has: changing the selector and pressing create in
                // one turn reaches this handler before any re-render, and a
                // captured host would send the create to the PREVIOUS machine
                // while the selector on screen names another.
                let target_now = catalog.target();
                let selected_now = effective_create_host(&hosts, chosen_host.peek().to_owned(), open_host);
                // And the catalog the agent was just resolved against has to
                // be the catalog OF that host. When they disagree the target
                // effect has not caught up with the selector yet — a window of
                // one render — and a profile id resolved against the old
                // host's catalog would be sent to the new one, where it means
                // something else or nothing.
                if selected_now != target_now.as_ref().map(|target| target.host) {
                    error.set(Some(
                        "the target host changed while this create was being submitted, so                          nothing was sent — check the agent and press create again"
                            .to_string(),
                    ));
                    ops.release();
                    return;
                }
                // No host, no create. The helm would default a hostless body
                // to its local row — usually the right answer, and not one
                // this form may reach by omission while its own selector is
                // still blank. Saying so beats creating on a machine the
                // user was never shown.
                let Some(binding) =
                    IntentBinding::of(selected_now, &hosts, cwd(), launch, title())
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
                    // browser for a UUID), and the `disabled` attributes that
                    // make this form inert land one render AFTER the submit —
                    // so a keystroke already queued when it fired can still
                    // change a field while the key is being made. The
                    // re-read below is what closes that, not the attribute. Publishing that key
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
                        // What the form's TEXT says now. Identical on the
                        // ordinary path; different exactly when a queued edit
                        // landed during the mint.
                        //
                        // The agent is deliberately NOT re-read here. It was
                        // frozen when the button was pressed, and re-resolving
                        // it would let a deletion or another client's
                        // remembered-default write — either of which can land
                        // during this await — change which creation MODE the
                        // request carries. A key that names one intent and a
                        // body that carries another is the exact failure the
                        // key exists to prevent, so the press wins and a
                        // profile that has since gone is refused by the
                        // supervisor, by name.
                        binding = IntentBinding {
                            cwd: cwd.peek().clone(),
                            title: title.peek().clone(),
                            ..binding
                        };
                    };
                    // Key, fields, host AND creation mode all travel from ONE
                    // value, so there is no arrangement of edits or reads in
                    // which the body describes a different intent than the
                    // key claims — including the mode itself, which the
                    // supervisor folds into its own idempotency fingerprint
                    // precisely so a retried create cannot flip it.
                    let agent = match &bound.agent {
                        LaunchIntent::Command(invocation) => CreateAgent::Command(invocation),
                        LaunchIntent::Profile(id) => CreateAgent::Profile(id),
                    };
                    // The connection this create was prepared against. Read
                    // from the surface's target rather than from the hosts
                    // snapshot, because the target is what the picker's
                    // catalog was read on — so the profile id in the body and
                    // the connection in the precondition describe one moment.
                    let expectation = target_now
                        .as_ref()
                        .map(|target| target.expectation())
                        .unwrap_or_default();
                    match create_session(
                            &base,
                            &bound.cwd,
                            agent,
                            &bound.title,
                            &key,
                            Some(bound.host),
                            expectation,
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
                            // Gated on the target this request was DISPATCHED
                            // for still being the one on screen: a refusal
                            // naming host A must not land under a form that
                            // has since been re-pointed at host B, where it
                            // would describe a machine the user is not looking
                            // at and may not even be true.
                            if catalog.target() == target_now {
                                let (stale, prose) = api::precondition_of(&e);
                                if stale {
                                    // The world moved between preparing this
                                    // create and routing it — the id now
                                    // reaches another install, where the
                                    // profile id would have resolved to
                                    // something else. The binding is stale in
                                    // every part, so the key goes with it (a
                                    // retry must be a NEW intent, not a replay
                                    // aimed at a machine that never saw the
                                    // first) and the catalog is re-read, which
                                    // is what supersedes this message.
                                    intent_key.set(None);
                                    chosen_profile.set(None);
                                    catalog.request(Trigger::Explicit);
                                }
                                // The key otherwise deliberately SURVIVES a
                                // failure: this is exactly the case it exists
                                // for. A failure whose cause was an ambiguous
                                // transport error may have created a session
                                // the user cannot see, and resubmitting
                                // unchanged must reach that same session
                                // rather than launch a second agent. A user
                                // who instead fixes the form gets a new key,
                                // because the binding no longer matches.
                                error.set(Some(prose));
                            }
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
                        // A profile id belongs to ONE supervisor, so a choice
                        // cannot follow the user to another host: carrying it
                        // over would either name nothing there or — because
                        // every fresh supervisor seeds the same starters —
                        // resolve to a profile they never chose. Cleared
                        // rather than remembered, so the new host's own
                        // remembered default takes over.
                        chosen_profile.set(None);
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
                            // Marked on the OPTION as well as through the
                            // select's `value` above, and that redundancy is
                            // load-bearing rather than belt-and-braces — see
                            // the agent picker below, where the same
                            // arrangement is what makes a preselection appear
                            // at all.
                            selected: selected == Some(host.id),
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
            // The agent, offered from the TARGET host's catalog and defaulting
            // to what a session was last created from there (SPEC.md's
            // creation rule; `profiles::resolve_agent`). The empty option is
            // the raw command path below rather than "no agent" — a create
            // always launches something, and this select is which of the two
            // mutually exclusive modes it uses.
            label {
                "agent"
                select {
                    class: "create-session-profile",
                    // Inert for the whole round trip, exactly like the host
                    // selector and for the same reason: the idempotency key
                    // is bound to what is launched, so a selection that moved
                    // between minting and sending would publish a key
                    // belonging to a different create.
                    disabled: busy,
                    value: "{chosen_agent}",
                    onchange: move |evt| {
                        chosen_profile.set(AgentChoice::from_value(&evt.value()));
                        // A different agent is a different intended create,
                        // exactly as a different directory is.
                        intent_key.set(None);
                    },
                    // The placeholder exists only while nothing is selected,
                    // and it is what a blocked dialog SHOWS: a `size=1` select
                    // always has one option selected, so "no answer yet" needs
                    // an option of its own rather than borrowing the command
                    // path's — borrowing it is exactly how a vanished profile
                    // used to turn into a silent command launch.
                    if agent.choice.is_none() {
                        option {
                            value: UNRESOLVED_VALUE,
                            selected: true,
                            "— choose an agent —"
                        }
                    }
                    // Which option is CHOSEN is stated on the options
                    // themselves, not only through the select's `value`
                    // above, and that is a correctness fix rather than
                    // belt-and-braces. A select's `value` is applied as a DOM
                    // PROPERTY, which a browser silently ignores when no
                    // option matches it yet — and this picker's options
                    // arrive later than its value by construction, since the
                    // catalog is read after the dialog opens. The property
                    // set is then never retried (the framework only re-emits
                    // an attribute whose value CHANGED), so the picker would
                    // sit on "custom command" forever while this component
                    // believed a profile was selected: the invisible
                    // mismatch — a control showing one thing while the submit
                    // sends another — that the host selector's own note calls
                    // the failure worth preventing. An option's `selected` is
                    // applied when the option itself is created, so it cannot
                    // race its own list.
                    option {
                        value: "",
                        // Selected only when the command path is what a
                        // submit would actually use — never merely because
                        // nothing else is, which is what the placeholder
                        // above is for.
                        selected: agent.choice == Some(AgentChoice::Command),
                        "custom command (below)"
                    }
                    for profile in offered.map(|catalog| catalog.profiles.as_slice()).unwrap_or_default() {
                        option {
                            key: "{profile.id}",
                            value: "{profile.id}",
                            selected: chosen_agent == profile.id,
                            // Escaped like every other rendering of
                            // peer-supplied text: an option label is exactly
                            // where a directional override could make one
                            // profile read as another, and what is chosen
                            // here decides what runs.
                            "{display_peer(&profile.name)}"
                        }
                    }
                }
            }
            // SPEC.md's ask-don't-guess fallback, said out loud. It appears
            // only when a profile that WAS available is not anymore — a first
            // create on a host has nothing to explain — and the thing it
            // rules out is the silent substitution: another profile quietly
            // preselected under the label of a remembered preference.
            if let Some(note) = agent.note {
                div { class: "create-session-profile-note", "{note.text()}" }
            }
            // A catalog that could not be READ is a third state, and it must
            // not look like a host with no profiles: the usual cause is a
            // host that is not connected, and the picker offering only the
            // command path with nothing said would leave a user wondering
            // where their profiles went — and then hitting the same refusal
            // from the create itself. The helm's sentence names the phase,
            // so it is printed as written.
            if let CatalogLookup::Failed(error) = &held {
                PeerLine {
                    class: "create-session-profile-error".to_string(),
                    parts: vec![
                        DetailPart::text("this host's profiles could not be read: "),
                        DetailPart::peer(*error),
                    ],
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
            // Present in both modes and INERT in one: a profile already says
            // what to run, the wire refuses a create naming both, and a field
            // that stayed live would invite a user to type a command that is
            // not what launches. `required` follows the mode for the same
            // reason — an empty command is exactly right when a profile
            // supplies it.
            label {
                if by_profile {
                    "agent command (unused: the selected profile supplies it)"
                } else {
                    "agent command"
                }
                input {
                    r#type: "text",
                    required: !by_profile,
                    autocomplete: "off",
                    autocorrect: "off",
                    autocapitalize: "none",
                    spellcheck: "false",
                    value: "{invocation}",
                    disabled: busy || by_profile,
                    oninput: move |evt| {
                        invocation.set(evt.value());
                        // Typing a command IS choosing the command path, and
                        // recording it here is what keeps a late arrival from
                        // taking the choice away: without it, a catalog
                        // landing a moment later could seed a remembered
                        // profile, disable this very field, and leave what was
                        // just typed on screen but unused. The user said what
                        // they want by typing it.
                        chosen_profile.set(Some(AgentChoice::Command));
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
                //
                // Inert with no agent selected for a different reason: there
                // is nothing to launch, and the handler refuses in words
                // anyway (a `disabled` attribute is one render behind, so it
                // is the visible half of that rule rather than the guard).
                disabled: busy || agent.choice.is_none(),
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

/// One row, in two layers. The row itself is a plain `<div>` wrapper
/// around two real `<button>`s — the open button (the session's stacked
/// title/host/cwd/invocation lines) and the small "⋯" actions-menu
/// toggle beside it. Everything else — rename, stop, archive, delete,
/// and their confirm prompts — mounts inside the floating panel the
/// toggle anchors, and only while that panel is open. Real buttons
/// rather than a `div` with `role`/`tabindex`/a hand-rolled `onkeydown`:
/// every action gets Enter- and Space-activation, focus styling, and
/// screen-reader semantics for free (a hand-rolled div also had a latent
/// bug: Space on a focused element scrolls the page unless the handler
/// prevents default, which native button activation never triggers).
///
/// The wrapper is a `div`, not a `button`, because HTML forbids
/// interactive content nested inside a `<button>` — a whole-row button
/// could not legally host the toggle. Tab order follows the layers:
/// closed, it walks open → toggle and on to the next row; open, the
/// panel's controls follow the toggle (rename → stop → archive →
/// delete, as visible per `RowControlVisibility`); confirming, the
/// panel holds consequence text plus confirm → cancel (with initial
/// FOCUS on cancel — see "Focus-on-open" below); renaming, the panel
/// holds the current title plus the field's input → save → cancel.
///
/// ## Host and staleness (PLAN_M6.md item 6)
///
/// The row stacks its fields: the title line leads (with the stale/
/// archived/status badges beside it), then the host, directory, and
/// invocation lines, each ellipsizing alone in the fixed-width sidebar.
/// A row the helm marked stale
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
/// open button. Opening ANY row swaps which session the keyed
/// `SessionView` shows, tearing the previous one down mid-operation, and
/// repaints this list under whatever operation is still in flight — the
/// point of disabling it is to keep the selection still until every
/// in-flight create/stop/delete has delivered its result.
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
/// The `.session-row-open` button (title/cwd/invocation/badge) stays
/// VISIBLE while a prompt is pending, merely `disabled`: the prompt lives
/// in the floating actions panel now, which overlays the rows below
/// instead of competing with the button for the row's own space, so the
/// MT-8 overflow that once forced the button out of layout entirely
/// (`display: none` behind a `prompting` class) cannot recur by
/// construction. Disabling it is still required — cancel must be the only
/// way back to normal, never an implicit click on open (see `confirming`
/// in `ListView`).
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
/// `renaming` swaps the actions panel's rename/stop/archive/delete set
/// for the session's own title plus `rename::RenameForm`, disabling (not
/// hiding) the open button exactly as the confirm prompt does. The two
/// states are mutually exclusive by construction — `ListView` refuses to
/// open either while the other is showing — so the branches below can be a
/// plain if/else chain rather than a composition of overlays.
///
/// Repeating the title inside the panel is load-bearing, not decoration:
/// a REFUSED rename must show the rejected draft NEXT TO the name that
/// still stands (SPEC.md requires the old title to stay while the
/// supervisor's refusal is shown), and the row's own title line may be
/// ellipsized past recognition in the narrow sidebar.
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
/// async call in the UI's own control to get wrong or ignore. It fires
/// whenever the cancel button is freshly created inside the `if
/// confirming` branch — which, since the prompt lives in the on-demand
/// panel, can be MORE than once per confirmation: the confirming flag
/// survives the panel closing (see `confirming` in `ListView`), so
/// closing and reopening the panel remounts the prompt and lands focus
/// on cancel again. That repeat is the safe direction — every fresh
/// appearance of the prompt starts with the escape hatch focused.
#[component]
#[allow(clippy::too_many_arguments)]
fn SessionRow(
    session: Session,
    state: RowState,
    rename_draft: Signal<String>,
    on_open: EventHandler<Session>,
    on_stop: EventHandler<String>,
    on_delete: EventHandler<DeleteTarget>,
    on_confirm_delete: EventHandler<String>,
    on_cancel_delete: EventHandler<String>,
    on_archive: EventHandler<Session>,
    on_confirm_archive: EventHandler<String>,
    on_cancel_archive: EventHandler<String>,
    on_rename_start: EventHandler<(String, String)>,
    on_rename_submit: EventHandler<(String, String)>,
    on_rename_cancel: EventHandler<()>,
    on_menu_toggle: EventHandler<String>,
) -> Element {
    let RowState {
        error,
        busy,
        confirming,
        confirming_archive,
        renaming,
        nav_disabled,
        menu_open,
    } = state;
    #[cfg(test)]
    SESSION_ROW_RENDERS.with(|renders| renders.set(renders.get() + 1));
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
    let archive_target = session.clone();
    let confirm_id = session.id.clone();
    let cancel_id = session.id.clone();
    let confirm_archive_id = session.id.clone();
    let cancel_archive_id = session.id.clone();
    let rename_start = (session.id.clone(), session.title.clone());
    let rename_submit_id = session.id.clone();
    let controls = row_control_visibility(session.archived);
    let menu_id = session.id.clone();
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
            "data-session-archived": "{session.archived}",
            // Two stacked rows, not one: the buttons need a plain flex
            // ROW (see `.session-row-main` in app.css), but a per-session
            // error line needs its own full-width row underneath rather
            // than squeezing in as a fourth flex item next to the
            // buttons — hence the extra wrapper rather than putting
            // everything directly under `.session-row`.
            div { class: "session-row-main",
                button {
                    r#type: "button",
                    class: "session-row-open",
                    // Disabled by ANY of the three locks: the global nav
                    // lock (any in-flight op anywhere), or this row's own
                    // confirmation or rename field being open — the
                    // simplest way to satisfy "cancel is the only way back
                    // to normal" (see the component doc above) is to make
                    // the open button inert for the whole time a prompt is
                    // showing, rather than giving it a second, competing
                    // meaning as an implicit cancel.
                    disabled: nav_disabled || confirming || confirming_archive || renaming,
                    onclick: move |_| on_open.call(open_session.clone()),
                    // STACKED lines rather than one squeezed flex row: the
                    // sidebar column (BUGS_BURNDOWN.md issue 5, interviewed
                    // row contents) is far too narrow for the old
                    // everything-on-one-line layout, whose min-width floors
                    // produced the MT-8 overflow class the moment space ran
                    // short. The title line leads with identity and its
                    // qualifiers; host, cwd, and invocation each get a line
                    // they can ellipsize alone.
                    // `span` wrappers, not `div`: this all sits inside the
                    // native `.session-row-open` <button>, whose content
                    // model only permits phrasing content — a flow-content
                    // div inside a button is invalid HTML that engines and
                    // accessibility tooling may interpret inconsistently.
                    // The stacked-line layout comes from the class's CSS,
                    // not the element kind.
                    span { class: "session-row-line",
                        span { class: "session-title", "{session.title}" }
                        // Beside the status badge rather than replacing it:
                        // the last-known status is still what the helm
                        // knows, and this says how old that knowledge is —
                        // two facts, not one, exactly as the stop annotation
                        // qualifies rather than replaces `exited`.
                        if session.stale {
                            span { class: "stale-badge", "stale" }
                        }
                        if session.archived {
                            span { class: "archived-badge", "archived" }
                        }
                        if let Some((badge_class, badge_text)) = badge {
                            span { class: "status-badge {badge_class}", "{badge_text}" }
                        }
                    }
                    // The host gets the second line: with more than one
                    // machine in play it is what disambiguates two otherwise
                    // identical sessions. The name is the helm's own
                    // rendering (`host_name`), denormalized onto the row so
                    // the list needs no second request — a row from a helm
                    // that sends none simply shows nothing here rather than
                    // inventing a label. Escaped and direction-isolated like
                    // every other rendering of a destination: this one names
                    // the machine a row's stop and delete will reach, so a
                    // name able to reorder the line around it could make one
                    // host's session read as another's.
                    if let Some(host_name) = &session.host_name {
                        span { class: "session-row-line",
                            span { class: "session-host peer-value", dir: "ltr",
                                "{display_peer(host_name)}"
                            }
                        }
                    }
                    span { class: "session-row-line",
                        // Two spans, not one: `.session-cwd` is the rtl
                        // clipping container that puts the ellipsis on the
                        // LEFT, and the inner `dir="ltr"` child is the bidi
                        // isolate that keeps the path's characters in
                        // logical order under it — rtl applied directly to
                        // the text would move a leading "/" to the visual
                        // right (see `.session-cwd` in app.css).
                        span { class: "session-cwd",
                            span { class: "session-cwd-text", dir: "ltr", "{session.cwd}" }
                        }
                    }
                    // No profile chip on the line (the interviewed row
                    // contents): the source-profile surface lives in the
                    // actions menu panel below.
                    span { class: "session-row-line",
                        span { class: "session-invocation", "{session.invocation}" }
                    }
                }
                // The actions menu: one small toggle beside the open
                // button, everything else in a floating panel it anchors
                // (BUGS_BURNDOWN.md issue 5's interviewed design — the row
                // itself carries no action buttons). The panel is also
                // where a destructive action CONFIRMS: clicking delete or
                // archive swaps the panel's contents for the consequence
                // line and confirm/cancel pair, keeping the whole exchange
                // on one small surface instead of bouncing the user
                // somewhere else. Rename lives here too — the ONLY
                // rename surface, by decision, which is what retires the
                // old dual-optimistic-overlay disagreement with the
                // titlebar (whose affordance the redesign removed).
                // Deliberately NOT disabled under the nav lock: opening a
                // panel mutates nothing — every action inside it carries
                // its own disabled state — and locking the toggle would
                // hide the very buttons whose disabled state tells the
                // user WHY the page is briefly inert.
                button {
                    r#type: "button",
                    class: "btn session-row-menu",
                    // The session's identity is part of the accessible
                    // name: a list renders one of these per row, and
                    // "session actions" alone leaves a screen-reader user
                    // no way to tell which session a toggle controls
                    // before activating it. Clamped, because a title can
                    // legally run to tens of KB and an accessible name is
                    // read aloud in full.
                    aria_label: menu_label(&session.title),
                    aria_expanded: menu_open,
                    onclick: move |_| on_menu_toggle.call(menu_id.clone()),
                    "⋯"
                }
                if menu_open {
                    div { class: "session-row-menu-panel",
                        if confirming {
                            // Two elements, consequence first: an
                            // untruncatable consequence and a separately
                            // truncatable title, in THIS order, so a long
                            // title can never clip the safety-critical
                            // half (see the component doc above).
                            span {
                                class: "confirm-consequence",
                                "{confirm_consequence(&session.status)}"
                            }
                            span { class: "confirm-title", "\"{session.title}\"" }
                            button {
                                r#type: "button",
                                class: "btn confirm-delete",
                                // Disabled while the shared token is held:
                                // the handler refuses then anyway (keeping
                                // the prompt), and the attribute is that
                                // refusal made visible. Cancel stays
                                // enabled — backing out is always safe.
                                disabled: busy,
                                onclick: move |_| on_confirm_delete.call(confirm_id.clone()),
                                "confirm delete"
                            }
                            button {
                                r#type: "button",
                                class: "btn confirm-cancel",
                                // Safe default: land keyboard focus on
                                // cancel, not confirm, the instant this
                                // prompt appears — a stray Enter/Space
                                // right after the menu's delete click backs
                                // OUT of the destructive action instead of
                                // into it (see the component doc for why
                                // declarative `autofocus` over the async
                                // focus API).
                                autofocus: true,
                                onclick: move |_| on_cancel_delete.call(cancel_id.clone()),
                                "cancel"
                            }
                        } else if confirming_archive {
                            if let Some(consequence) = archive_confirmation(&session, session.tabs.len()) {
                                span { class: "confirm-consequence", "{consequence}:" }
                            } else {
                                span { class: "confirm-consequence", "archiving removes the terminal:" }
                            }
                            span { class: "confirm-title", "\"{session.title}\"" }
                            button {
                                r#type: "button",
                                class: "btn confirm-archive",
                                // See confirm-delete: refusal made visible.
                                disabled: busy,
                                onclick: move |_| on_confirm_archive.call(confirm_archive_id.clone()),
                                "confirm archive"
                            }
                            button {
                                r#type: "button",
                                class: "btn archive-cancel",
                                autofocus: true,
                                onclick: move |_| on_cancel_archive.call(cancel_archive_id.clone()),
                                "cancel"
                            }
                        } else if renaming {
                            // The AUTHORITATIVE title stays beside the
                            // field: a refused rename must never leave the
                            // rejected DRAFT as the only name on this
                            // surface (SPEC.md's "the old title stays"
                            // while the refusal is shown).
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
                            if controls.rename {
                                button {
                                    r#type: "button",
                                    class: "btn session-row-rename",
                                    disabled: busy,
                                    onclick: move |_| on_rename_start.call(rename_start.clone()),
                                    "rename"
                                }
                            }
                            if controls.stop {
                                button {
                                    r#type: "button",
                                    class: "btn session-row-stop",
                                    disabled: busy,
                                    onclick: move |_| on_stop.call(stop_id.clone()),
                                    "stop"
                                }
                            }
                            if controls.archive {
                                button {
                                    r#type: "button",
                                    class: "btn session-row-archive",
                                    disabled: busy,
                                    onclick: move |_| on_archive.call(archive_target.clone()),
                                    "archive"
                                }
                            }
                            if controls.delete {
                                button {
                                    r#type: "button",
                                    class: "btn session-row-delete",
                                    disabled: busy,
                                    onclick: move |_| on_delete.call(delete_target.clone()),
                                    "delete"
                                }
                            }
                            // The profile this session was CREATED from, as
                            // it snapshotted the name — moved here from the
                            // row proper (the interviewed row contents drop
                            // the chip) so SPEC.md's snapshot rule keeps a
                            // visible surface: the name never moves under
                            // an existing session, and the qualifier
                            // (`profiles::source_profile_label`) keeps that
                            // from reading as a claim about today's
                            // catalog. `data-profile-existence` remains the
                            // browser suite's handle on the half that does
                            // change.
                            if let Some(source) = &session.source_profile {
                                span {
                                    class: "session-profile peer-value",
                                    dir: "ltr",
                                    "data-profile-existence": "{existence_word(source.existence)}",
                                    "{source_profile_label(source)}"
                                }
                            }
                        }
                    }
                    // The refusal a panel action produced renders INSIDE
                    // the open panel, under its controls: the panel floats
                    // over the row's own error line, so an error rendered
                    // only down there could sit hidden behind the very
                    // surface whose click caused it.
                    if let Some(err) = error.clone() {
                        PeerLine {
                            class: "action-error".to_string(),
                            parts: vec![DetailPart::Peer(err)],
                        }
                    }
                }
            }
            // The helm's own refusal, and on a stale row it names the host's
            // state and can quote peer-supplied text — so it renders through
            // the same escaping and isolation as every other peer string.
            // Only while the panel is CLOSED: the open panel carries the
            // error itself (above), and rendering both would say one
            // refusal twice.
            if let Some(err) = error {
                if !menu_open {
                    PeerLine {
                        class: "action-error".to_string(),
                        parts: vec![DetailPart::Peer(err)],
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Revealing an archived row must keep its metadata actions while
    /// withholding lifecycle controls that no longer have a terminal to act
    /// on.
    #[test]
    fn archived_rows_keep_metadata_controls_without_lifecycle_controls() {
        assert_eq!(
            row_control_visibility(true),
            RowControlVisibility {
                rename: true,
                stop: false,
                archive: false,
                delete: true,
            }
        );
    }

    /// Repeated parent refreshes must update direct callback props in place
    /// without rerendering an otherwise unchanged session row.
    ///
    /// Dioxus gives `EventHandler` component props special ownership and
    /// memoization. Hiding them inside an ordinary props struct bypasses
    /// that path: every fleet refresh then retains another callback set and
    /// rerenders every row. This drives the real `SessionRow` through many
    /// parent renders, so either regression changes the count from one.
    #[test]
    fn repeated_parent_refreshes_do_not_rerender_an_unchanged_row() {
        fn app() -> Element {
            let rename_draft = use_signal(String::new);
            let on_open = use_callback(|_: Session| {});
            let on_stop = use_callback(|_: String| {});
            let on_delete = use_callback(|_: DeleteTarget| {});
            let on_confirm_delete = use_callback(|_: String| {});
            let on_cancel_delete = use_callback(|_: String| {});
            let on_archive = use_callback(|_: Session| {});
            let on_confirm_archive = use_callback(|_: String| {});
            let on_cancel_archive = use_callback(|_: String| {});
            let on_rename_start = use_callback(|_: (String, String)| {});
            let on_rename_submit = use_callback(|_: (String, String)| {});
            let on_rename_cancel = use_callback(|_: ()| {});
            let on_menu_toggle = use_callback(|_: String| {});
            let session = Session {
                id: "session-1".to_string(),
                title: "stable".to_string(),
                cwd: "/tmp".to_string(),
                invocation: "agent".to_string(),
                status: SessionStatus::Exited { exit_code: Some(0) },
                annotation: None,
                restart_offer: crate::RestartOffer::FreshOnly,
                archived: false,
                tabs: Vec::new(),
                host: None,
                host_name: None,
                stale: false,
                source_profile: None,
            };
            rsx! {
                SessionRow {
                    session,
                    state: RowState {
                        error: None,
                        busy: false,
                        confirming: false,
                        confirming_archive: false,
                        renaming: false,
                        nav_disabled: false,
                        menu_open: false,
                    },
                    rename_draft,
                    on_open,
                    on_stop,
                    on_delete,
                    on_confirm_delete,
                    on_cancel_delete,
                    on_archive,
                    on_confirm_archive,
                    on_cancel_archive,
                    on_rename_start,
                    on_rename_submit,
                    on_rename_cancel,
                    on_menu_toggle,
                }
            }
        }

        SESSION_ROW_RENDERS.with(|renders| renders.set(0));
        let mut dom = VirtualDom::new(app);
        dom.rebuild_to_vec();
        for _ in 0..64 {
            dom.mark_dirty(dioxus::core::ScopeId::APP);
            dom.render_immediate(&mut dioxus::core::NoOpMutations);
        }
        SESSION_ROW_RENDERS.with(|renders| {
            assert_eq!(
                renders.get(),
                1,
                "unchanged rows must stay memoized across fleet refreshes"
            );
        });
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
    #[test]
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
    #[test]
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

    /// A CONNECTED host as the create dialog would be offered it —
    /// `phase: None` is what "connected" means to an option.
    fn option(id: HostId, name: &str, local: bool) -> HostOption {
        HostOption {
            id,
            name: name.into(),
            local,
            phase: None,
            incarnation: format!("incarnation-{id}"),
            connection: 1,
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

    /// With nothing selected, the default is the LOCAL row — SPEC.md's
    /// second clause, the one that fires whenever the first cannot.
    #[test]
    fn the_create_default_is_the_local_row() {
        let hosts = vec![
            option(1, "this machine", true),
            option(2, "user@box", false),
        ];
        assert_eq!(default_create_host(&hosts, None), Some(1));
    }

    /// The SELECTED session's host wins over the local row while it is in
    /// the registry, and falls back the moment it is not — SPEC.md's first
    /// create-default clause, live again now that the sidebar layout lets
    /// the create form and an open session coexist (BUGS_BURNDOWN.md
    /// issue 5).
    #[test]
    fn the_open_sessions_host_wins_while_it_exists() {
        let hosts = vec![
            option(1, "this machine", true),
            option(2, "user@box", false),
        ];
        assert_eq!(default_create_host(&hosts, Some(2)), Some(2));
        assert_eq!(
            default_create_host(&hosts, Some(99)),
            Some(1),
            "a selected session whose host left the registry falls back to the helm's own host"
        );
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
            default_create_host(&hosts, None),
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
        assert_eq!(default_create_host(&[], None), None);

        let remote_only = vec![option(2, "user@box", false), option(3, "user@other", false)];
        assert_eq!(
            default_create_host(&remote_only, None),
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
    ///
    /// The CREATION MODE joins that list at M6.75, and it is the sharpest
    /// case of the same rule: the same command line run from a profile and
    /// typed by hand are two different intended creates, and the supervisor
    /// folds the mode into its own idempotency fingerprint precisely so a
    /// retry cannot flip between them.
    #[test]
    fn an_intent_binding_changes_with_the_host_incarnation_and_with_the_fields() {
        let hosts = vec![option(1, "this machine", true)];
        let command = || LaunchIntent::Command("agent".to_string());
        let base = IntentBinding::of(
            Some(1),
            &hosts,
            "/tmp".to_string(),
            command(),
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
            command(),
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
                agent: LaunchIntent::Command("other-agent".to_string()),
                ..base.clone()
            },
            // A profile-backed create of the "same" thing is a DIFFERENT
            // intent: what runs is the profile's definition, which nothing on
            // this side can compare against a typed command.
            IntentBinding {
                agent: LaunchIntent::Profile("p-1".to_string()),
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
                command(),
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
        let nothing = || LaunchIntent::Command(String::new());
        assert!(IntentBinding::of(None, &hosts, String::new(), nothing(), String::new()).is_none());
        assert!(
            IntentBinding::of(Some(99), &hosts, String::new(), nothing(), String::new()).is_none(),
            "a selection the option list no longer contains is not a target either"
        );
    }

    /// The effective create target is the user's choice while it exists and
    /// the local row otherwise — one answer, used by both the dialog that
    /// renders it and the reader that follows it.
    ///
    /// The middle case is why this is a function rather than two expressions:
    /// a chosen host leaving the registry moves the target, and if the picker
    /// and the catalog reader disagreed about when, the dialog would offer
    /// one host's profiles for a create aimed at another — an id that names
    /// nothing over there, or worse, a starter profile that resolves.
    #[test]
    fn the_effective_create_target_follows_a_choice_until_it_is_gone() {
        let hosts = vec![
            option(1, "this machine", true),
            option(2, "user@box", false),
        ];
        assert_eq!(effective_create_host(&hosts, Some(2), None), Some(2));
        assert_eq!(
            effective_create_host(&hosts, None, None),
            Some(1),
            "with no choice made, the target is SPEC.md's default"
        );
        assert_eq!(
            effective_create_host(&hosts, Some(99), None),
            Some(1),
            "a choice the registry no longer holds falls back to the default rather than staying \
             on a host nothing can reach"
        );
        assert_eq!(effective_create_host(&[], Some(2), None), None);
    }

    /// Full three-candidate precedence: explicit choice over the open
    /// session's host over the local row — and a VANISHED choice falls back
    /// to the open session's host, not past it to local.
    ///
    /// The earlier tests each exercise one clause with the others absent,
    /// which a reversed precedence or a fallback that skips the middle
    /// clause would pass; this is the arrangement where every wrong order
    /// gives a different answer. The middle assertion is the subtle one:
    /// SPEC.md's first clause is "the host of the currently open session",
    /// so a dead explicit choice lands there, and skipping to the local
    /// row would silently move the create off the machine whose session
    /// the user is looking at.
    #[test]
    fn precedence_holds_with_all_three_candidates_present() {
        let hosts = vec![
            option(1, "this machine", true),
            option(2, "user@box", false),
            option(3, "user@other", false),
        ];
        assert_eq!(
            effective_create_host(&hosts, Some(3), Some(2)),
            Some(3),
            "a valid explicit choice beats the open session's host"
        );
        assert_eq!(
            effective_create_host(&hosts, Some(99), Some(2)),
            Some(2),
            "a vanished choice falls back to the open session's host, not to local"
        );
        assert_eq!(
            effective_create_host(&hosts, Some(99), Some(98)),
            Some(1),
            "and only when BOTH are gone does the local row answer"
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

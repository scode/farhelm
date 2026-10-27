//! The session listing and the reads that keep it current.
//!
//! `ListView` owns the fleet-wide state; row rendering and creation stay in
//! child modules so their narrower contracts remain visible.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use dioxus::prelude::*;

use crate::activity::{ACTIVITY_NOW, ActivityStamp};
use crate::anchored_popovers::{
    AnchorRegion, Interest, MovedRows, MovementCapture, MovementOwner, use_registration,
    use_rendered_movement_between,
};
use crate::api::{
    self, ListSort, Preferences, SessionFilter, SessionListing, delete_session, fetch_hosts,
    fetch_sessions, queue_seen_write, rename_session, stop_session,
};
use crate::app_bar::AppBar;
use crate::feed::{fallback_polls_now, fallback_sleep, use_feed_reader};
use crate::hosts::{HostFlow, HostsPanel, HostsRead, host_rows_moved};
use crate::modal_dialog::FocusTarget;
use crate::ops::{ConfirmToken, OpLock, ReadGate, use_confirm_slot, use_confirm_slots};
use crate::peer::PeerText;
use crate::provisioning::ProvisioningTraceShape;
use crate::reader::{SurfaceReader, Trigger, request_read};
use crate::rows::{
    self, absence_is_evidence, apply_optimistic_renames, count_banner,
    remove_row_reporting_menu_move, retire_vanished_renames, settle_optimistic_renames,
};
use crate::{ApiBase, HostId, Session, SessionStatus};

use super::create_form::{
    COMPOSER_DIALOG, CreatePrefill, CreateSessionForm, CreateTarget, CreatedSession,
    mark_replace_with, mark_restart_with, prefill_from,
};
use super::row::{IdentityFlow, SessionRow, session_identity_flow};
use super::shared::{
    DeleteClick, DeleteTarget, HostOption, OpenDestination, RowQuestion, RowState,
    effective_create_host, host_options, matching_host_option, session_locality,
};
use crate::rename::RenameDialog;
use crate::replace::{PendingReplace, ReplaceAnswer, ReplaceController, ReplaceResult};

/// Name the shortcut only where the app installs it; browsers keep Cmd+N for windows.
const NEW_SESSION_TOOLTIP: &str = if crate::HAS_NEW_SESSION_SHORTCUT {
    "new session: start an agent or a command on any host (⌘N)"
} else {
    "new session: start an agent or a command on any host"
};

/// Whether a fleet-wide switcher pick needs the sidebar widened to reveal it.
///
/// This is a navigation decision, not a second list filter: the helm still
/// owns membership. Only the three offered live states match a nonempty
/// status choice; ended or unclassified picks require All. An admitted sidebar
/// reply answering this exact filter takes precedence over the switcher's
/// older status snapshot. Presence proves visibility even in a cut reply;
/// absence does so only in a complete one, never proving deletion.
/// Callers pass None for failed or superseded replies
/// and clear neither dimension until the ordinary open guard accepts the pick.
fn switcher_pick_hidden(
    filter: &SessionFilter,
    session: &Session,
    current_listing: Option<&SessionListing>,
) -> bool {
    if let Some(current) = current_listing {
        if current.sessions.iter().any(|row| row.id == session.id) {
            return false;
        }
        if !current.truncated {
            return filter.narrows();
        }
    }
    let status_matches = match filter.status.as_str() {
        "" => true,
        "running" => matches!(session.status, SessionStatus::Running),
        "waiting" => matches!(session.status, SessionStatus::Waiting),
        "idle" => matches!(session.status, SessionStatus::Idle),
        _ => false,
    };
    filter.host.is_some_and(|host| session.host != Some(host)) || !status_matches
}

/// A successful render's held-order identities and their actual copy flow.
///
/// None is an absent/failed listing, not a successful empty list. The rendered
/// movement hook owns the only previous snapshot; this value has no independent
/// cache, clock subscription or publication lifecycle.
type IdentitySnapshot = Option<Vec<(String, Option<IdentityFlow>)>>;

/// The row's accepted session plus its already-derived local rendering values.
type RenderedSession = (Session, bool, Option<ActivityStamp>);

/// Use the same held/renamed session and local values passed into SessionRow.
fn identity_snapshot(rows: Option<&[RenderedSession]>, compact: bool) -> IdentitySnapshot {
    rows.map(|rows| {
        rows.iter()
            .map(|(session, deleting, activity)| {
                (
                    session.id.raw().to_string(),
                    session_identity_flow(session, *deleting, compact, activity.as_ref()),
                )
            })
            .collect()
    })
}

/// Preserve complete identity contributions and duplicates within one prefix.
fn identity_prefix(rows: &[(String, Option<IdentityFlow>)], end: usize) -> Vec<&IdentityFlow> {
    let mut values: Vec<_> = rows[..end]
        .iter()
        .filter_map(|(_, value)| value.as_ref())
        .collect();
    values.sort_unstable();
    values
}

/// Select surviving anchors whose strictly earlier identity contents changed.
///
/// Compare whole contributions as multisets: one accepted listing can exchange
/// two complete copy values wholly above an anchor without moving its prefix.
/// Multiplicity and text/activity pairing still matter. Routing identities are
/// deliberately excluded from contribution equality. Index/separator/detail and
/// refusal movement keep their existing publication rules.
fn identity_rows_moved(before: &IdentitySnapshot, after: &IdentitySnapshot) -> Vec<String> {
    let (Some(before), Some(after)) = (before, after) else {
        return Vec::new();
    };
    after
        .iter()
        .enumerate()
        .filter_map(|(index, (id, _))| {
            let previous_index = before.iter().position(|(previous, _)| previous == id)?;
            (identity_prefix(before, previous_index) != identity_prefix(after, index))
                .then(|| id.clone())
        })
        .collect()
}

/// Retain admitted causes across successful renders with different audiences.
///
/// A below-anchor change can select nobody while an earlier committed change
/// still requires closing that same opening. Only loss/recovery of a comparable
/// listing baseline resets pending; an unaffected successful capture does not.
/// The hook and owner retain the existing deferred exact-opening admission.
fn capture_identity_movement(
    owner: &MovementOwner,
    before: &IdentitySnapshot,
    after: &IdentitySnapshot,
) -> MovementCapture {
    let ticket = owner.capture_affected(&MovedRows::new(
        AnchorRegion::Sessions,
        identity_rows_moved(before, after),
    ));
    if before.is_some() && after.is_some() {
        MovementCapture::Merge(ticket)
    } else {
        MovementCapture::Replace(ticket)
    }
}

/// One-shot request sent by a session header to the list's existing composer.
/// AppBody owns the signal so the request survives the keyed session view and
/// ListView can consume it exactly once.
#[derive(Clone, PartialEq)]
pub(crate) enum HeaderPrefillRequest {
    Clone(Session),
    ReplaceWith(Session),
    RestartWith(Session, RestartBridge),
}

/// Keep terminal lifecycle work in its keyed view while the launcher owns
/// editing and dispatch. A disposed view needs no remount; fallible reads
/// prevent a late reply from invoking a callback whose scope was removed.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct RestartBridge {
    pub(crate) current: Signal<Session>,
    pub(crate) before: Callback<api::RestartEdit, Result<(), String>>,
    pub(crate) after: EventHandler<bool>,
}

impl RestartBridge {
    /// Admission immediately precedes the POST, after any host-setting await.
    pub(crate) fn begin(self, edit: api::RestartEdit) -> Result<(), String> {
        let alive = self.current.try_read().is_ok();
        if !alive {
            return Err("the session view closed; reopen restart with".into());
        }
        self.before.call(edit)
    }

    /// Every actual POST result reattaches, including refusals and lost replies.
    pub(crate) fn finish(self, succeeded: bool) {
        if self.current.try_read().is_ok() {
            self.after.call(succeeded);
        }
    }
}

/// One-shot request from a session header to delete the session it shows,
/// sent after its confirmation was answered or skipped by the session preference.
///
/// The delete itself runs through the list's ordinary delete path, not a
/// second implementation in the header: that path owns the per-row
/// operation gate, the optimistic row removal once the reply lands, the read
/// fence that keeps an older listing from resurrecting the row, and the
/// selection clearing, and the header would otherwise have to duplicate all
/// of it. Like [`HeaderPrefillRequest`], AppBody owns the signal so the
/// request outlives the keyed session view, and ListView consumes it once.
#[derive(Clone, PartialEq)]
pub(crate) struct HeaderDeleteRequest {
    pub(crate) id: String,
    /// The precondition the header's render, as the user saw it, covered
    /// (see `status::delete_guard`).
    pub(crate) guard: crate::DeleteGuard,
}

/// How long the pointer must rest completely still over the session list
/// before a held order is released anyway. Chosen by the user: long enough
/// to finish aiming at a row, short enough that a pointer left parked over
/// the list does not freeze its order indefinitely.
const ORDER_HOLD_STILL_MS: u128 = 5_000;

/// How often the hold's watcher checks whether the pointer has been still
/// for [`ORDER_HOLD_STILL_MS`]. A coarse poll rather than a timer reset on
/// every `pointermove`, because pointer moves arrive far faster than the
/// release needs to be precise, and a task per move would be wasted work.
const ORDER_HOLD_POLL_MS: u64 = 250;

/// One independently mounted rename editor and the source it must never lose.
///
/// The listing may reorder, filter, fail, or later prove this source gone;
/// none of those events may retarget the draft by row position. The generation
/// makes a later editor distinct from deferred work for the same id, even
/// though the ordinary pending-operation guard prevents that overlap today.
#[derive(Clone, PartialEq, Eq)]
struct RenameEditor {
    id: String,
    current_title: String,
    generation: u64,
    unavailable: bool,
    /// Keyboard opening records its original opener and returns to the terminal.
    shortcut: bool,
    /// Naming follows an accepted hidden quick launch: select its whole seed,
    /// allow unchanged submission without a request, and return to its terminal.
    naming: bool,
}

/// Name an accepted quick launch only when New never offered its title field.
/// Unset is enabled so fresh and upgraded helms share the product default.
fn name_created_session(new_was_never_shown: bool, preference: Option<bool>) -> bool {
    new_was_never_shown && preference != Some(false)
}

/// Naming can accept its exact current title without a rename request.
/// Compare raw bytes, not the escaped display; ordinary Rename still submits
/// unchanged titles to the supervisor and keeps its existing validation contract.
fn naming_keeps_current_title(editor: &RenameEditor, submitted: &str) -> bool {
    editor.naming && submitted == editor.current_title
}

/// Whether a request result still owns the editor generation it started.
///
/// Session ids are not enough: the same session can be edited again after a
/// request lifetime ends. Every delayed completion path uses this predicate
/// so an old result cannot close or annotate a newer draft.
fn rename_result_owns_editor(editor: Option<&RenameEditor>, generation: u64) -> bool {
    editor.is_some_and(|editor| editor.generation == generation)
}

/// Decide what a listing is allowed to say about the editor's source.
///
/// `None` means the listing has no authority to change availability. A
/// filtered, truncated, or mutation-local read may hide the
/// source without proving that it left the fleet.
fn listed_rename_unavailable(
    editor: &RenameEditor,
    authoritative: bool,
    listing: &SessionListing,
) -> Option<bool> {
    (authoritative && absence_is_evidence(listing)).then(|| {
        !listing
            .sessions
            .iter()
            .any(|session| session.id.raw() == editor.id)
    })
}

/// Whether a queued focus return may run against the rendered editor state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RenameFocusReturn {
    /// The originating dialog is still mounted; wait for its unmount.
    Wait,
    /// A newer dialog superseded the intent; discard it without focusing.
    Discard,
    /// No dialog remains, so the opener may be considered after render.
    Ready,
}

/// Fence post-unmount focus work against a replacement editor generation.
fn rename_focus_return_state(generation: u64, editor: Option<&RenameEditor>) -> RenameFocusReturn {
    match editor {
        Some(editor) if editor.generation == generation => RenameFocusReturn::Wait,
        Some(_) => RenameFocusReturn::Discard,
        None => RenameFocusReturn::Ready,
    }
}

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
/// Every accepted selection is remembered, including an automatic pick.
/// Before that pick the list rereads only selection inputs from the helm;
/// unrelated settings retain their existing reload behavior. The cached
/// visits also mirror local choices so a failed read can still choose.
///
/// Because the row is the helm's, a "second client" attaches to whatever
/// was most recently selected ANYWHERE, taking it over per the
/// one-attached-client rule — the consequence SPEC.md's Terminal
/// experience section names.
///
/// One signal for the preference fields, so a write to one dirties every reader
/// of the struct: a sort change with an empty right pane re-runs the
/// auto-select effect. That effect is idempotent (its `resolving_*`
/// guards), so this is a wasted run rather than a wrong one, and one
/// signal keeps the seed and writers consistent.
#[derive(Clone, Copy)]
pub(crate) struct SharedPreferences(pub(crate) Signal<Preferences>);

/// The newest notice a completed Delete carried: today a checkout its host could not archive and
/// left in place, unmanaged (SPEC.md "Managed checkouts": that outcome is never silent). The
/// session list shows it until dismissed or replaced by a newer one.
///
/// Shared app-wide, not local to the list, because a Replace deletes its
/// source too, and a Replace can start from the session view or the create
/// form as well as from the list; each sets this, and the list is where it
/// is shown. Its shared setter reports changes to the in-flow band before
/// publication, without clearing retained slots whose row DOM is absent.
#[derive(Clone, Copy)]
pub(crate) struct DeleteNotice(pub(crate) Signal<Option<String>>, Callback<Option<String>>);

/// One unresolved plain Replace, shared by every control on this page.
/// The source is reserved before key minting so a late reply from an older
/// row cannot overwrite a newer intent. Selecting a session does not discard
/// it; cancelling Replace, choosing Replace with, or replacing another source does.
#[derive(Clone, Copy)]
pub(crate) struct ReplaceIntent(pub(crate) Signal<Option<(String, Option<String>)>>);

/// Only one pane may own a YOLO question per source, independently of retry keys.
/// Each local question releases its source on consumption or unmount; other rows stay usable.
/// Retry identity may change A → B → A while the original question remains open.
#[derive(Clone, Copy)]
pub(crate) struct ReplaceYoloQuestions(pub(crate) Signal<HashSet<String>>);

impl DeleteNotice {
    /// Keep every Delete/Replace surface on the same synchronous publication
    /// boundary. Initial retained state is a mount baseline, not an event.
    pub(crate) fn new(mut value: Signal<Option<String>>, popovers: MovementOwner) -> Self {
        let replace = Callback::new(move |next: Option<String>| {
            let changed = {
                let previous = value.peek();
                previous.as_deref().map(crate::peer::display_peer)
                    != next.as_deref().map(crate::peer::display_peer)
            };
            if changed {
                popovers
                    .capture_region(AnchorRegion::Sessions)
                    .close_if_current();
            }
            // Same-value publications retain the original signal write, but
            // introduce no new movement report. None still differs from Some
            // empty text: the latter renders the band's dismiss control.
            value.set(next);
        });
        Self(value, replace)
    }

    /// Show `notice` when there is one; `None` leaves any earlier notice.
    pub(crate) fn publish(self, notice: Option<String>) {
        if let Some(notice) = notice {
            self.1.call(Some(notice));
        }
    }

    /// Remove this in-flow band before its old coordinates can survive in a
    /// session popover; host rows above the band are outside this report.
    pub(crate) fn dismiss(self) {
        self.1.call(None);
    }
}

/// Record an accepted selection (manual or automatic): the signal
/// first, so the choice is in force in this client whatever the helm says,
/// then a one-field patch through `api::store_preference`'s serialized
/// queue. The helm updates its capped visit history in that same write;
/// the local mirror supplies fallback input if a later preference read fails.
pub(crate) fn remember_selection(base: &str, mut preferences: SharedPreferences, id: &str) {
    preferences.0.write().record_selection(id);
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
    compact_choice(preferences.0.read().compact)
}

/// The compact-row default, shared by the render's subscribed read
/// (`stored_compact`) and `commit_listing`'s unsubscribed one so the two
/// cannot disagree about what an unset preference means.
fn compact_choice(stored: Option<bool>) -> bool {
    stored.unwrap_or(false)
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
/// The title and menu use these words; selection comes from the shared
/// preference rather than the position of the default in this list.
pub(super) const SORT_OPTIONS: [(ListSort, &str); 3] = [
    (ListSort::Activity, "Recently active"),
    (ListSort::Created, "Newest created"),
    (ListSort::Title, "Title A–Z"),
];

/// SPEC.md's auto-select fallback — the newest-created session from the rows
/// one listing actually carries.
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
///   first row in the listing's own order, which is what this
///   fallback did before `created_at` existed here.
/// - **Equal stamps keep the listing's order.** `created_at` has one-second
///   granularity, so ties are ordinary rather than exotic. `min_by` over a
///   REVERSED comparator is what implements "first of the maxima":
///   `Iterator::max_by` returns the LAST maximal element, which would make
///   the winner depend on the listing's length rather than on its head.
fn newest_created_fallback<'a>(
    sessions: &'a [Session],
    excluded: Option<&str>,
) -> Option<&'a Session> {
    let mut eligible = sessions
        .iter()
        .filter(|session| Some(session.id.raw()) != excluded);
    eligible
        .clone()
        .filter(|session| session.created_at > 0)
        .min_by(|a, b| b.created_at.cmp(&a.created_at))
        .or_else(|| eligible.next())
}

/// Choose within the current listing, for both startup and a cleared pane.
/// History outranks creation time; missing or filtered-out visits are skipped.
/// An empty history uses the retained last selection as one known visit, so
/// upgrading does not replace it merely because the new column starts empty.
/// Once history exists it is authoritative, even if the legacy value differs.
/// Exited and disconnected rows count normally. Excluding the last open ID
/// prevents a just-deleted row in a not-yet-refreshed listing from reopening.
/// Visits are full raw references, used only for identity lookup here.
fn automatic_selection<'a>(
    sessions: &'a [Session],
    history: &[PeerText],
    last_selected: Option<&str>,
    excluded: Option<&str>,
) -> Option<&'a Session> {
    history
        .iter()
        .map(PeerText::raw)
        .chain(last_selected.filter(|_| history.is_empty()))
        .filter(|id| Some(*id) != excluded)
        .find_map(|id| sessions.iter().find(|session| session.id.raw() == id))
        .or_else(|| newest_created_fallback(sessions, excluded))
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
/// the create form pre-filled) or replace (opens the row's Replace question) —
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
/// question.
///
/// `pending` is the row operation exclusion (`ListView`'s `row_pending`);
/// `asking` is THIS row's open inline question, if any, as `ListView`'s row
/// question slots report it (`ConfirmSlots::peek_key`).
fn clone_is_refused(
    busy: bool,
    session_id: &str,
    pending: &HashSet<String>,
    asking: Option<RowQuestion>,
    renaming: Option<&str>,
) -> bool {
    busy || pending.contains(session_id) || asking.is_some() || renaming == Some(session_id)
}

/// Admit a new operation on row `id` into the row operation exclusion:
/// `true` (and `id` now pending) only if the row has no operation in flight
/// and no inline question open.
///
/// A row's engagement has two parts that used to share one per-row phase,
/// and are now held apart because they guarantee different things:
///
/// - The operation exclusion (`pending`, `ListView`'s `row_pending`) keeps a
///   second stop, delete, replace or rename from starting on a row while one
///   runs. It is a plain set, entered here and left by `ListView`'s
///   `end_row_op`, and it says nothing about what the user saw.
/// - The inline question (`ListView`'s `row_questions`, one
///   `ops::ConfirmSlots` lane per row) carries opening identity: an answer
///   acts only for the opening its render drew.
///
/// They still exclude each other, and this is one half of that rule: a row
/// whose question is open admits no operation (its stop and delete buttons
/// are gone, and a click queued before that render must not slip one in
/// under the question), and the openers refuse a question while the row is
/// pending. Pure so the rule is testable without mounting `ListView`.
fn admit_row_op(pending: &mut HashSet<String>, asking: bool, id: &str) -> bool {
    if asking || pending.contains(id) {
        return false;
    }
    pending.insert(id.to_owned());
    true
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
    /// The currently selected session's destination — SPEC.md's first create-
    /// default clause, supplied by `App` because only it knows the
    /// selection. `None` when nothing is selected (or the selected
    /// session predates per-host metadata), which falls back to the
    /// helm's own host. Carries the install identity the session row
    /// reported beside the row id, so the default can notice a retarget
    /// or adopt landing between selection and create. The folder comes from
    /// that same snapshot even when sidebar filtering hides the selected row.
    open_destination: Option<OpenDestination>,
    /// The SHARED live-operation token (see `ops`'s module docs): owned by
    /// `AppBody` rather than created here, because the selected session's
    /// view claims the same token for its own restart — a private
    /// token per pane would let the two panes mutate the fleet under each
    /// other.
    ops: OpLock,
    /// Live count of this list's per-row operations, maintained here and
    /// read by the session view's `PaneGate`: row operations never hold
    /// the token (rows must stay concurrent with each other), so this
    /// count is the only way the other pane can refuse to start a write
    /// while one is in flight.
    row_ops: Signal<u32>,
    /// A session this list removed through a successful delete. `AppBody` reconciles the
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
    /// `automatic_selection`): whenever a committed listing has rows and
    /// nothing is selected, this list opens one. A signal rather than a
    /// plain prop because `commit_listing` consults it at REPLY time (a
    /// captured prop value would be as old as the render that made the
    /// closure) and the auto-select effect tracks it.
    selected: ReadSignal<Option<String>>,
    /// The open view publishes its raw title independently of sidebar filters.
    /// The selection's original snapshot may predate another client's rename.
    open_session_title: ReadSignal<Option<(String, String)>>,
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
    prefill_request: Signal<Option<HeaderPrefillRequest>>,
    /// The session header's confirmed deletes, run through `do_delete`.
    header_delete: Signal<Option<HeaderDeleteRequest>>,
    /// One-shot attended listing demand from a successful header mutation.
    header_refresh: Signal<bool>,
    /// Sessions with a delete in flight, written only by `do_delete` and
    /// read by the rows here and by the session header. AppBody owns it
    /// because the header is this view's sibling.
    deleting: Signal<HashSet<String>>,
) -> Element {
    let open_host = open_destination
        .as_ref()
        .map(|destination| destination.host.clone());
    let open_cwd = open_destination
        .as_ref()
        .map(|destination| destination.cwd.clone());
    let base = use_context::<ApiBase>().0;
    // The helm's shared preference, already read by `PreferencesGate` —
    // this component never mounts before it has (see `SharedPreferences`).
    let mut preferences = use_context::<SharedPreferences>();
    // Capture the preference with this render's action handlers, alongside
    // the row snapshots they act on. A skipped question still carries consent.
    let skip_session_confirmations = preferences.0.read().skip_session_confirmations == Some(true);
    let mut row_ops = row_ops;
    let mut deleting = deleting;
    let mut listing = use_signal(|| None::<Result<SessionListing, String>>);
    // A confirmed local archive and an observed managed row disappearing both
    // invalidate the count. The trash reads only on these edges, never a timer.
    let mut trash_revision = use_signal(|| 0_u64);
    // The same generation discipline the hosts read has, for the same
    // reason and against a slower race: a listing read that started before
    // a delete can easily still be in flight when the delete's own refresh
    // has already landed — and committing it would put the deleted row
    // back until the next one.
    let mut listing_reads = use_signal(ReadGate::default);
    // AppBody keeps the snapshot above both panes. This view still owns every
    // refresh, so showing host identity in the session adds no request path.
    let mut hosts = use_context::<Signal<HostsRead>>();
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
    // The seen writer outlives this component. It reports success through
    // this list-owned counter; the effect below owns the actual refresh task.
    let seen_toggle_successes = use_signal(|| 0_u64);
    // Not a row error: the row is gone. See `DeleteNotice`.
    let delete_notice = use_context::<DeleteNotice>();
    // The rows' operation exclusion: every row with a stop, delete, replace
    // or rename in flight. It disables that row's buttons and is the
    // re-entry guard the click handlers check, since the DOM update
    // disabling a button is not synchronous with its click handler. Entered
    // only through `begin_row_op` and left only through `end_row_op`; see
    // `admit_row_op` for why it is held apart from the questions below.
    let mut row_pending = use_signal(HashSet::<String>::new);
    // Each row's open inline question (Delete or Replace), one lane per row,
    // so several rows can hold a question at once and each answer is bound
    // to the opening its render drew. Opening identity is all this holds:
    // whether the row may start an operation is `row_pending`'s job.
    //
    // The questions are deliberately client-side with no timeout and no
    // refresh-driven reset: a listing refresh must leave an in-progress
    // confirmation alone (the user is mid-decision, not mid-refresh), and so
    // must a filter or a failed read that only stops drawing the row. The
    // one reconciliation is in `commit_listing`, which closes a question
    // once its session is no longer in the listing at all.
    let mut row_questions = use_confirm_slots::<String, RowQuestion, ()>();
    // `row_pending`'s entry and exit, with the cross-pane bookkeeping attached:
    // every row operation must (a) refuse to start while the SHARED token
    // is held — the session view or a page operation is mid-write, and a
    // row op landing under it is exactly the cross-pane race the shared
    // gate exists to close — and (b) keep `row_ops` equal to
    // `row_pending`'s size, because that count is what the session view's
    // `PaneGate` consults to refuse ITS claims while a row op runs. Paired
    // helpers rather than open-coded at each site so no exit path can forget
    // the decrement (a stuck count would leave the session view inert).
    // Borrow raw identity so decoded references and local action snapshots
    // enter the same gate without changing the set's owned keys. A row with
    // its question open is refused too (`admit_row_op`).
    let mut begin_row_op = move |id: &str| -> bool {
        if ops.busy_now() {
            return false;
        }
        let asking = row_questions.peek_key(id).is_some();
        if !admit_row_op(&mut row_pending.write(), asking, id) {
            return false;
        }
        row_ops += 1;
        true
    };
    let mut end_row_op = move |id: &String| {
        if row_pending.write().remove(id) {
            row_ops -= 1;
        }
    };
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
    // The one open notification list, if any (SPEC.md, Status): the
    // session's id, and its read mark as it stood when the list opened,
    // which keeps the entries that were new at that moment marked new while
    // the list stays open. Owned here for `menu_open`'s reasons: every
    // layout change that detaches a row's floating menu detaches this list
    // too, and closing is where its entries become read, which the
    // read-on-close effect below does for every way it can close.
    let mut bell_open = use_signal(|| None::<OpenBell>);
    // Register the existing slots once, in their original close order. Live
    // peek readers preserve effect dependencies; no second open-state model
    // can lag a toggle or replay an earlier movement against a new opening.
    let popovers = use_context::<MovementOwner>();
    let menu_registration = use_registration(
        popovers.clone(),
        Interest {
            direct_host_disclosure: true,
            ..Interest::new(AnchorRegion::Sessions)
        },
        move || menu_open.peek().clone(),
        move || {
            let mut slot = menu_open;
            slot.set(None);
        },
    );
    let host_registration = use_registration(
        popovers.clone(),
        Interest {
            direct_host_disclosure: true,
            ..Interest::new(AnchorRegion::Hosts)
        },
        move || (*host_menu_open.peek()).map(|id| id.to_string()),
        move || {
            let mut slot = host_menu_open;
            slot.set(None);
        },
    );
    let bell_registration = use_registration(
        popovers.clone(),
        Interest::new(AnchorRegion::Sessions),
        move || bell_open.peek().as_ref().map(|open| open.id.clone()),
        move || {
            let mut slot = bell_open;
            slot.set(None);
        },
    );
    // Root open slots survive failed listings. New content reports also need
    // the live success check and the row's scoped measurement binding.
    let listing_ready = use_callback(move |()| matches!(listing.peek().as_ref(), Some(Ok(_))));
    // The read mark each list this page closed sent, by session id, so a
    // list reopened before the next listing arrives does not show the same
    // entries as new again. Failed writes release only their own current mark,
    // allowing the next close to repair it without erasing a later close.
    let bell_read_sent = use_hook(|| Rc::new(RefCell::new(HashMap::<String, u64>::new())));
    // Both menus close on a pointer-down outside them; the relay button that
    // listener clicks is rendered at the top of this view (see there).
    use_hook(crate::menu_panel::install_row_menu_outside_dismiss);
    // The one stable rename editor, if any, and its text draft.
    //
    // One at a time, unlike the rows' questions: rename is a focused edit the
    // user finishes or abandons. The dialog is ListView-owned rather than
    // nested in a row menu, so a listing can move or temporarily remove rows
    // without reparenting the textarea. The draft remains separate from the
    // editor record so an authoritative disappearance can preserve it for
    // copying and cancellation.
    let mut rename_editor = use_signal(|| None::<RenameEditor>);
    let mut rename_draft = use_signal(String::new);
    let mut rename_generation = use_signal(|| 0_u64);
    let mut rename_error = use_signal(|| None::<(u64, String)>);
    // Focus ownership is recorded while the exact dialog generation is still
    // mounted. Busy controls remain focusable, so only a real move outside
    // ends ownership. The return request is consumed after unmount.
    let mut rename_focus_owned = use_signal(|| false);
    let mut rename_focus_return = use_signal(|| None::<(String, u64, bool)>);
    use_effect(move || {
        let intent = rename_focus_return();
        let editor = rename_editor();
        let Some((id, generation, terminal)) = intent else {
            return;
        };
        match rename_focus_return_state(generation, editor.as_ref()) {
            RenameFocusReturn::Wait => return,
            RenameFocusReturn::Discard => {
                rename_focus_return.set(None);
                return;
            }
            RenameFocusReturn::Ready => {}
        }
        rename_focus_return.set(None);
        // The parent owns admission and post-render timing. The shared close
        // keeps a newer active control focused; only the destination depends
        // on whether this opening came from a row, shortcut, or quick launch.
        let id = serde_json::to_string(&id).expect("session ids serialize as JSON strings");
        crate::rename::dialog_handle(generation).close_immediately_if_document_focused(&format!(
            r#"if ({terminal}) {{ window.farhelmTerm?.focusSelected(); return; }}
            const id = {id};
            document.querySelector(`[data-session-id="${{CSS.escape(id)}}"] .session-row-menu`)
                ?.focus({{ preventScroll: true }});"#,
        ));
    });
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
    // A switcher choice is an accepted search action, not a clone seed.
    let mut initial_create_action =
        use_signal(|| None::<crate::launch_composer::ComposerSearchResult>);
    // A quick launch keeps the definitions from its one fresh read. Replaying
    // names against another fetch could compose a different setup mid-opening.
    let mut initial_quick_templates =
        use_signal(Vec::<farhelm_proto::launcher::LaunchTemplate>::new);
    let mut initial_partial_template =
        use_signal(|| None::<crate::launch_sequence::PartialTemplateBanner>);
    // Quick launch owns one fetch/mount/submit handoff. New still composes the
    // request; this flag only keeps its fields hidden until a fallback is needed.
    let mut quick_launch_pending = use_signal(|| false);
    // A visible launcher accepts presses faster than a fresh read completes.
    // Serialize the reads so their captured definitions reach the form in
    // physical press order, then let the form apply each as a separate batch.
    let mut shortcut_dispatch_queue = use_signal(
        std::collections::VecDeque::<(
            u64,
            Vec<crate::launch_sequence::TemplateReference>,
            crate::launch_sequence::QuickLaunchOrigin,
            bool,
        )>::new,
    );
    let mut shortcut_dispatch_running = use_signal(|| false);
    // The page's press snapshots pair with this queue's keyboard entries
    // purely by order, but the page's listener outlives this component. When
    // the list unmounts (the token prompt), the queue and its worker task die
    // with this scope while the page could still hold snapshots they owed.
    // Signing in reloads the page, but a build-mismatch notice over the token
    // prompt remounts the list in place; left there, each snapshot would be
    // taken by that mount's first keyboard press, shifting every later pair
    // and launching the wrong template. Clearing the page here empties both
    // sides at the same boundary: the remount starts a fresh queue, and no
    // snapshot owed to the next mount can exist before this clear. The page
    // captures only when it finds a relay, and on the web renderer this eval
    // runs in the same browser task that then removes the relays, so no
    // keydown falls between the two. (Only the web renderer raises the token
    // prompt; the desktop keeps this list mounted.) Clearing on mount instead
    // could run after the new relays reach the page and drop a valid press.
    use_drop(crate::launch_sequence::clear_shortcut_presses);
    let mut shortcut_apply_requests =
        use_signal(std::collections::VecDeque::<super::create_form::ShortcutApplyRequest>::new);
    let mut launch_sequence = use_context::<crate::launch_sequence::LaunchSequence>().0;
    let mut launch_sequence_in_launcher =
        use_context::<crate::launch_sequence::LaunchSequenceInLauncher>().0;
    // A cancelled sequence must not let its late preference read rewrite the
    // indicator of a later one. Enter still captures only what was displayed.
    let mut sequence_generation = use_signal(|| 0_u64);
    let mut auto_launch = use_signal(|| false);
    // One opening generation fences the fresh read and any queued DOM submit.
    // Ordinary New, Clone/Replace and Templates revoke it synchronously.
    // It names an opening of the launcher, which answers and deferred work
    // check before acting; it is not itself an answer to a prompt, so it
    // deliberately stays outside `ops::ConfirmSlot` (SPEC_impl.md,
    // "GUI: Dioxus", the paragraph "Answers bind to what was drawn").
    let mut opening_generation = use_signal(|| 0_u64);
    // Diagnostic settlement only, so held-read tests can observe a revoked
    // continuation actually finishing instead of using a delay as proof.
    let mut quick_read_completed = use_signal(|| 0_u64);
    let mut initial_create_error = use_signal(|| None::<String>);
    // The Templates dialog, opened from the button beside New.
    let mut templates_open = use_signal(|| false);
    let templates_dialog =
        crate::modal_dialog::DialogHandle::new(super::templates::DIALOG_SELECTOR);
    let mut quick_launch_hint = use_signal(|| false);
    let mut number_shortcut_hint = use_signal(|| false);
    // A launcher save hands the actual stored snapshot to the next dialog;
    // loading the list must not decide which template the person just opened.
    let mut initial_template = use_signal(|| None::<farhelm_proto::launcher::LaunchTemplate>);
    // The switcher stays list-owned so picks use the ordinary navigation gate.
    let mut quick_switcher_open = use_signal(|| false);
    let mut quick_switcher_mode = use_signal(|| super::quick_switcher::SwitcherMode::Sessions);
    // A close owns one focus/navigation handoff until the DOM removal is
    // acknowledged. Reopening in that gap would give the old task a new
    // dialog and a new focus target to wait for.
    let mut quick_switcher_closing = use_signal(|| false);
    // Bumped when the dialog closes; the launcher re-reads templates on it.
    let mut templates_revision = use_signal(|| 0_u64);
    use_context_provider(|| super::templates::TemplatesRevision(templates_revision));
    let provisioning_trace_shapes = use_signal(HashMap::<HostId, ProvisioningTraceShape>::new);
    let provisioning_auto_details = use_signal(HashSet::<HostId>::new);
    // Presentation never owns the list reader, selection, or disclosure.
    // Both surfaces stay mounted so hidden host work can finish normally.
    let mut hosts_panel_open = use_signal(|| false);
    // One client-local details choice for every host row, reset on page mount.
    // HostsPanel remains permanently mounted below and owns every write;
    // this shared handle lets accepted reads compare its actual visible
    // lines synchronously. Automatic per-row disclosure never writes it.
    let host_details_open = use_signal(|| false);
    // Visible management growth can move a lower host's fixed menu. Its
    // resize callback is gated at the presentation boundary; hidden progress
    // cannot move session or rail anchors. Actual scrolls remain invalidators.
    let mut hosts_panel_layout_epoch = use_signal(|| 0_u64);
    // A shape signature for the always-mounted host list. A memo prevents
    // ordinary phase-only polls and progress-step churn from closing fixed
    // surfaces while still catching every rendered trace transition, every
    // automatic-disclosure transition (an update expanding or collapsing
    // its own row moves the rows below it exactly like a trace does), and
    // the count or read-state changes that move visible management rows.
    let hosts_list_shape = use_memo(move || {
        let read = hosts.read();
        let mut traces = provisioning_trace_shapes
            .read()
            .iter()
            .map(|(host, shape)| (*host, shape.operation, shape.status))
            .collect::<Vec<_>>();
        traces.sort_by_key(|(host, _, _)| *host);
        let mut auto = provisioning_auto_details
            .read()
            .iter()
            .copied()
            .collect::<Vec<_>>();
        auto.sort();
        (
            read.hosts().map(<[_]>::len).unwrap_or(0),
            read.is_loading(),
            read.refresh_error().is_some(),
            traces,
            auto,
        )
    });
    // A fixed menu uses one coordinate snapshot. External layout changes and
    // real content/rail scrolling still close existing openings. Management
    // shape participates only while Hosts is visible: retained progress and
    // automatic disclosure continue while concealed but move no visible row.
    // The panel transition itself closes menus before changing presentation.
    //
    // Registration reads remain untracked, so a fresh opening cannot receive
    // a past invalidation. Per-row text and prefix changes retain their causal
    // MovementOwner reports at the accepted listing/host publication boundary;
    // this aggregate path supplements those reports rather than replacing them.
    let layout_popovers = popovers.clone();
    use_effect(move || {
        layout_epoch();
        show_create();
        if hosts_panel_open() {
            hosts_list_shape();
        }
        hosts_panel_layout_epoch();
        layout_popovers.close_open_in(&[AnchorRegion::Hosts, AnchorRegion::Sessions]);
    });
    // The create dialog's explicit host choice names the installation used
    // for creation idempotency.
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
    let mut restart_bridge = use_signal(|| None::<RestartBridge>);
    // An ordinary New inherits only the selected session's directory. It is
    // intentionally separate from clone prefill: no title, host, command, or
    // structured launch choice is implied by this convenience.
    let mut ordinary_new_cwd = use_signal(|| None::<String>);
    // Host and status choices are page-local queries, not preferences. Each
    // control changes only its own field; the helm combines their membership.
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
    // The session list's order hold: while the pointer is over the list,
    // the ids in the order they are displayed, so no row moves out from
    // under the pointer on its way to a click (SPEC.md, Session list). A
    // display-only exception to rendering the helm's order as served:
    // nothing here is remembered or sent anywhere, and
    // `rows::held_display_order` is the whole rule for what a hold shows.
    //
    // A hold always describes the committed `listing`, never the controls:
    // a reply to a DIFFERENT query than the one on screen ends it (see
    // `commit_listing`), because changing the host, filter, or sort is
    // asking for a different list. That also covers a hold taken while a
    // changed query's reply is still in flight, which froze the old rows.
    let mut order_hold = use_signal(|| None::<Vec<String>>);
    // The query the committed `listing` answers, which is what a new reply
    // is compared against to decide whether the hold still applies.
    let mut listing_query = use_signal(|| None::<(SessionFilter, ListSort)>);
    // A sounds read can outlive several accepted views. Keep their identity
    // even across A -> B -> A, so the returning view gets a silent baseline
    // rather than comparing against attention observed before B was shown.
    let mut listing_scope = use_signal(crate::sounds::StatusScope::default);
    // A row's refusal line (`action-error`, drawn under the row while its
    // own menu is closed) makes the row taller, so one appearing, clearing
    // or changing its text on a row ABOVE the open session menu can move
    // that menu's row without changing any listing. The listing-side check
    // in `commit_listing` cannot see it, because the lines come from
    // `errors`, so this effect reports affected displayed rows to the owner
    // (SPEC_impl.md: close on layout changes that could have moved the row).
    // `error_rows` is `errors` as of the previous run, kept in a plain cell
    // because only this effect reads it and nothing renders from it.
    let error_rows = use_hook(|| Rc::new(Cell::new(HashMap::<String, String>::new())));
    let error_popovers = popovers.clone();
    use_effect(move || {
        let now = errors.read().clone();
        let before = error_rows.replace(now.clone());
        // The open notification list floats beside its row exactly like a
        // menu, so a refusal line appearing above it moves it the same way.
        let changed: HashSet<&str> = before
            .iter()
            .filter(|(id, message)| now.get(*id) != Some(*message))
            .chain(now.iter().filter(|(id, _)| !before.contains_key(*id)))
            .map(|(id, _)| id.as_str())
            .collect();
        let moved = {
            let listing = listing.peek();
            let Some(Ok(listing)) = listing.as_ref() else {
                return;
            };
            let held = order_hold.peek().clone().unwrap_or_default();
            rows::rows_below_changed(
                &rows::held_display_order(&held, &listing.sessions),
                &changed,
            )
        };
        error_popovers.close_affected(&MovedRows::new(AnchorRegion::Sessions, moved));
    });
    // The hold's pointer facts, read by `rows::hold_should_release`: when the
    // pointer last moved over the list, and whether it is over it now. Plain
    // cells rather than signals on purpose: they change on every pointer
    // move and nothing renders from them, so making them reactive would only
    // schedule re-renders nobody needs. The generation retires the watcher
    // of an ended hold when a new one starts.
    let hold_clock = use_hook(|| Rc::new(Cell::new(web_time::Instant::now())));
    let hold_generation = use_hook(|| Rc::new(Cell::new(0_u64)));
    let pointer_inside = use_hook(|| Rc::new(Cell::new(false)));
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

    // Which installation the create request is about. This follows
    // `effective_create_host`.
    let mut create_target = use_signal(|| None::<CreateTarget>);
    // `use_reactive` because `open_host` is a plain prop, not a signal:
    // without it the effect would capture the value it saw on first run
    // and an open form would keep offering the OLD session's host after the
    // user selected a session on another machine —
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
    // Whether a reply for `requested`/`ordered_by` answers the same query as
    // the listing on screen, which is what lets an order hold carry over.
    let answers_listing_query = move |requested: &SessionFilter, ordered_by: ListSort| {
        listing_query
            .peek()
            .as_ref()
            .is_some_and(|(filter, sort)| filter == requested && *sort == ordered_by)
    };
    let occurrences = use_context::<crate::claude_trust::OccurrenceDismissals>();
    let listing_popovers = popovers.clone();
    let commit_listing = move |generation: u64,
                               requested: SessionFilter,
                               ordered_by: ListSort,
                               mut fetched: Result<SessionListing, String>,
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
        // Partition only this sidebar's copy, before any hold reconciliation.
        // The helm order and quick switcher's independent read stay intact.
        if let Ok(listing) = &mut fetched {
            rows::pinned_first(&mut listing.sessions);
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
            for session in &listing.sessions {
                occurrences.observe(session);
            }
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
                .is_some_and(|id| !listing.sessions.iter().any(|s| s.id.raw() == *id));
            if menu_vanished {
                menu_open.set(None);
            }
            // The notification list closes on the same grounds, and also
            // when its row is still listed but has no bell any more (another
            // window cleared it): left open, the list would reappear by
            // itself with the next notification.
            let bell_vanished = bell_open.read().as_ref().is_some_and(|open| {
                !listing
                    .sessions
                    .iter()
                    .any(|s| s.id.raw() == open.id && !s.notifications.is_empty())
            });
            if bell_vanished {
                bell_open.set(None);
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
            // Report displayed rows once, independently of popup kinds. The
            // outgoing listing borrow must end before closers or the final
            // listing write can run, preserving the old commit boundary.
            let (moved, banner_changed, recovered) = {
                let previous = listing_signal.peek();
                let recovered = matches!(previous.as_ref(), Some(Err(_)));
                let banner_changed = match previous.as_ref() {
                    Some(Ok(previous)) => count_banner(previous) != count_banner(listing),
                    _ => false,
                };
                let previous_sessions = match previous.as_ref() {
                    Some(Ok(prev)) => Some(prev.sessions.as_slice()),
                    _ => None,
                };
                // Compared as DISPLAYED, not as served: while the order
                // is held, a served reorder moves nothing on screen and
                // must not close a menu whose row stayed put. The two
                // sides are shown under different holds when this reply
                // answers a different query, which ends the hold below.
                let previous_held = order_hold.peek().clone().unwrap_or_default();
                let incoming_held = if answers_listing_query(&requested, ordered_by) {
                    previous_held.clone()
                } else {
                    Vec::new()
                };
                let previous_shown = previous_sessions
                    .map(|previous| rows::held_display_order(&previous_held, previous));
                // Peeked live rather than taken from this render's
                // `compact`: the reply may land renders after the
                // closure was built, and a toggle in between decides
                // whether detail lines are on screen now.
                let detail_lines_shown = !compact_choice(preferences.0.peek().compact);
                let incoming_shown = rows::held_display_order(&incoming_held, &listing.sessions);
                let mut moved = rows::menu_rows_reordered(
                    previous_shown.as_deref(),
                    &incoming_shown,
                    detail_lines_shown,
                );
                // Use the same live host/error inputs on both sides: this
                // commit changes listing contents/order, while the existing
                // errors effect still owns error-map edits and their timing.
                let current_hosts = hosts.peek();
                let current_errors = errors.peek();
                moved.extend(rows::session_content_rows_moved(
                    previous_shown.as_deref().map(|shown| rows::SessionFlow {
                        shown,
                        hosts: &current_hosts,
                        errors: &current_errors,
                        compact: !detail_lines_shown,
                    }),
                    rows::SessionFlow {
                        shown: &incoming_shown,
                        hosts: &current_hosts,
                        errors: &current_errors,
                        compact: !detail_lines_shown,
                    },
                ));
                (moved, banner_changed, recovered)
            };
            listing_popovers.close_affected(&MovedRows::new(AnchorRegion::Sessions, moved));
            // A reply replacing a failed read must not bring back a session
            // menu or notification list left open across the failure. When
            // the error line rendered, the rows remount and the popup
            // reopens unasked; when the recovery lands before it rendered,
            // the popup keeps coordinates measured against the old list. The
            // reorder check above deliberately reads a recovery as no
            // movement, so this closes them instead.
            if recovered {
                listing_popovers.close_open_in(&[AnchorRegion::Sessions]);
            }
            // The count sentence and header actions can rewrap above every
            // session anchor. Admission excludes retained Err/remount slots;
            // this report stays synchronous before the original publication.
            if banner_changed {
                listing_popovers
                    .capture_affected(&MovedRows::new(
                        AnchorRegion::Sessions,
                        listing
                            .sessions
                            .iter()
                            .map(|session| session.id.raw().to_owned()),
                    ))
                    .close_if_current();
            }
            // Still open: this listing's entries are what the list shows next.
            let still_shown = bell_open.peek().as_ref().and_then(|open| {
                let newest = listing
                    .sessions
                    .iter()
                    .find(|session| session.id.raw() == open.id)
                    .and_then(Session::newest_notification_seq)?;
                (newest > open.shown).then(|| OpenBell {
                    shown: newest,
                    ..open.clone()
                })
            });
            if let Some(raised) = still_shown {
                bell_open.set(Some(raised));
            }
            // The right pane's placeholder may only claim an empty fleet on
            // a committed, uncut default listing. A user filter proves
            // nothing about the pane and leaves the verdict as it was.
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
            // Filtered and truncated omissions cannot prove an archive was
            // created. Compare only complete fleet replies before refreshing.
            let managed_disappeared = listing_signal
                .peek()
                .as_ref()
                .and_then(|value| value.as_ref().ok())
                .filter(|previous| absence_is_evidence(previous))
                .is_some_and(|previous| {
                    previous.sessions.iter().any(|session| {
                        session.working_copy.is_some()
                            && !listing
                                .sessions
                                .iter()
                                .any(|next| next.id.raw() == session.id.raw())
                    })
                });
            if managed_disappeared {
                trash_revision += 1;
            }
            // Close any row question whose session is gone from
            // this fetch entirely — the counterpart to the "a poll
            // refresh must not clear an in-progress confirmation"
            // rule just above: that rule protects a row that is
            // still LISTED, not one that has vanished (deleted from
            // another client while this one sat mid-confirmation, an
            // externally-imposed departure the `retain` below cannot
            // distinguish from the id simply never having existed).
            // Reconciliation, not an answer, so it closes lanes by row
            // and never needs a token. `row_pending` is left alone: an
            // operation in flight ends only through its own completion.
            // These keys reconcile local row state against routing identities,
            // never against the escaped spelling a user sees in a label.
            let live_ids: HashSet<&str> = listing.sessions.iter().map(|s| s.id.raw()).collect();
            row_questions.retain(|id| live_ids.contains(id.as_str()));
            // A complete committed fleet read is the only absence proof
            // strong enough to change an editor. It does not close one: a
            // user may still need its draft. If the exact source returns on
            // a later authoritative read, only the unavailable condition is
            // cleared; the source snapshot and typed text remain untouched.
            let current_editor = rename_editor.peek().clone();
            if let Some(mut editor) = current_editor
                && let Some(unavailable) =
                    listed_rename_unavailable(&editor, authoritative, listing)
                && editor.unavailable != unavailable
            {
                editor.unavailable = unavailable;
                rename_editor.set(Some(editor));
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
        if let Ok(listing) = &fetched {
            // A reply to the same query as the rows on screen keeps the hold
            // and makes it adopt what it now shows, including rows the reply
            // appended at the bottom, so a later reply cannot re-sort those
            // among themselves while the pointer is still over them. A reply
            // to a different query ends the hold (see `order_hold`).
            let reconciled = match order_hold.peek().as_ref() {
                Some(held) if answers_listing_query(&requested, ordered_by) => Some(
                    rows::held_display_order(held, &listing.sessions)
                        .into_iter()
                        .map(|session| session.id.raw().to_owned())
                        .collect::<Vec<_>>(),
                ),
                _ => None,
            };
            if reconciled.is_some() || order_hold.peek().is_some() {
                order_hold.set(reconciled);
            }
            if listing_query
                .peek()
                .as_ref()
                .is_none_or(|(filter, _)| *filter != requested)
            {
                listing_scope.write().advance(listing);
            }
            listing_query.set(Some((requested.clone(), ordered_by)));
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
    let read_commit_listing = commit_listing.clone();
    let listing_read = move || {
        let base = read_listing_base.clone();
        let mut commit_listing = read_commit_listing.clone();
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
        // The evidence predicate, never the banner's: a narrowed request
        // cannot treat an omitted row as a departure.
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
    let host_content_popovers = popovers.clone();
    let hosts_read = move || {
        let base = read_hosts_base.clone();
        let popovers = host_content_popovers.clone();
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
                // Preview one real HostsRead fold for both consumers. Session
                // wording uses effective lookup state; host rows keep drawing
                // a retained snapshot after a refresh failure. A first host
                // snapshot has no prior anchor baseline, while a failed
                // session listing heals by remounting rather than comparing.
                let (session_moved, host_moved) = {
                    let before = hosts.peek();
                    let mut after = before.clone();
                    after.record(outcome.clone());
                    // Hidden management lines cannot move a rail anchor.
                    // Membership/order still matters in either presentation.
                    let panel_visible = *hosts_panel_open.peek();
                    let details_open = panel_visible && *host_details_open.peek();
                    let auto_details = provisioning_auto_details.peek();
                    let no_auto_details = HashSet::new();
                    let visible_auto_details = if panel_visible {
                        &*auto_details
                    } else {
                        &no_auto_details
                    };
                    let host_moved = host_rows_moved(
                        before.hosts().map(|shown| HostFlow {
                            shown,
                            details_open,
                            auto_details: visible_auto_details,
                        }),
                        HostFlow {
                            shown: after.hosts().unwrap_or_default(),
                            details_open,
                            auto_details: visible_auto_details,
                        },
                    );
                    let current_listing = listing.peek();
                    let compact = compact_choice(preferences.0.peek().compact);
                    let session_moved = if let Some(Ok(current)) = current_listing.as_ref()
                        && !compact
                    {
                        let held = order_hold.peek().clone().unwrap_or_default();
                        let shown = rows::held_display_order(&held, &current.sessions);
                        let current_errors = errors.peek();
                        rows::session_content_rows_moved(
                            Some(rows::SessionFlow {
                                shown: &shown,
                                hosts: &before,
                                errors: &current_errors,
                                compact,
                            }),
                            rows::SessionFlow {
                                shown: &shown,
                                hosts: &after,
                                errors: &current_errors,
                                compact,
                            },
                        )
                    } else {
                        Vec::new()
                    };
                    (session_moved, host_moved)
                };
                // No read guards survive dispatch, and there is no await or
                // late effect that can replay this change at a fresh opening.
                popovers.close_affected(&MovedRows::new(AnchorRegion::Sessions, session_moved));
                popovers.close_affected(&MovedRows::new(
                    AnchorRegion::Hosts,
                    host_moved.into_iter().map(|id| id.to_string()),
                ));
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
    // Read-on-close, Clear and manual seen toggles wait for this refresh to
    // update the row, rather than editing it optimistically. They need no
    // rename overlay or mutation fence: the reader serializes its requests,
    // and ReadGate rejects older generations (including a stop's direct
    // refetch) after the refresh commits. An older reply can precede the
    // refreshed row, but cannot bring its stale marks back afterwards.
    //
    // Cloned per call site rather than made `Copy`: the read closures own
    // their copy of the API base, which is a `String`.
    let request_listing =
        move |trigger: Trigger| request_read(listing_surface, trigger, listing_read.clone());
    let request_hosts =
        move |trigger: Trigger| request_read(hosts_surface, trigger, hosts_read.clone());

    // AppBody owns the demand so replacing the keyed detail view cannot
    // drop it. The existing reader coalesces it with any read already out.
    let mut header_refresh = header_refresh;
    let header_listing = request_listing.clone();
    use_effect(move || {
        if header_refresh() {
            header_refresh.set(false);
            header_listing(Trigger::Explicit);
        }
    });

    // This effect runs in the list's scope, unlike the root-owned seen
    // writer. Zero means no manual save has settled, so mount keeps its one
    // initial read; every later success asks for an attended refresh.
    let seen_toggle_refresh = request_listing.clone();
    use_effect(move || {
        if seen_toggle_successes() != 0 {
            seen_toggle_refresh(Trigger::Explicit);
        }
    });

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
    let feed_acted_on = use_feed_reader(move || {
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

    // A host selection is already a complete server-side query. The reader
    // still serializes and coalesces demand; `Explicit` merely records this
    // user action immediately rather than waiting for a text debounce.
    let request_host_filter = request_listing.clone();
    let request_status_filter = request_listing.clone();
    let apply_status = use_callback(move |value: &'static str| {
        if filter.peek().status != value {
            filter.write().status = value.to_string();
            request_status_filter(Trigger::Explicit);
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
    let sort_popovers = popovers.clone();
    let apply_sort = move |next: ListSort| {
        // Re-selecting the option already in force is not a change: acting on
        // it would restart the walk and write the helm's row for nothing.
        if *sort.peek() == next {
            return;
        }
        // A different title can wrap the header even before the new rows
        // arrive. Existing fixed popovers must not retain that old anchor.
        sort_popovers.close_registered(AnchorRegion::Sessions);
        sort.set(next);
        remember_sort(&sort_base, preferences, next);
        sort_read(Trigger::Explicit);
    };

    let stop_base = base.clone();
    let stop_commit_listing = commit_listing.clone();
    // The surface reader, for the one path that reads outside it: a stop's
    // own refetch fails on its own and has nobody to retry it (see below).
    let stop_recovery = request_listing.clone();
    // Takes the id directly, not the whole `Session`: nothing past the
    // insert-into-`pending` check below reads any other field, so a
    // `Session` clone (and a second, redundant id clone off of it) would
    // only be dead weight — see `SessionRow`'s call site for the mirrored
    // simplification on the caller's side.
    let on_stop = move |id: String| {
        // Cross-guard against an open row question, not just
        // `row_pending`: the stop/delete buttons are only ABSENT from the
        // DOM once a rerender following the question's opening has landed
        // (see `SessionRow`'s doc), so a stop click queued just ahead of
        // that rerender — a rapid synthetic double-click, say — could
        // otherwise still reach this handler for a row that is, or is
        // about to be, showing the confirm prompt. Refusing here keeps
        // the row's two lifecycle handlers from ever racing each other
        // for the same id: without it, a stop could slip `id` into
        // `row_pending` WHILE a delete confirmation is open, and the
        // eventual "confirm delete" click would then silently no-op — NOT
        // because of anything in `confirm_delete` itself, but because
        // `do_delete` (which it calls after taking the question) has its
        // OWN `row_pending` re-entry guard, which would find the id
        // already occupied by that stop and bail with no error at all.
        // (`begin_row_op` below also refuses a row whose question is open;
        // this check keeps the refusal explicit at the handler.)
        //
        // The same argument covers an open RENAME field, which replaces
        // the same buttons for the same reason.
        if row_questions.peek_key(&id).is_some()
            || rename_editor
                .read()
                .as_ref()
                .map(|editor| editor.id.as_str())
                == Some(id.as_str())
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
        // Acceptance completes the menu interaction; a later stop failure
        // belongs in the row's error line. Close here so async completion
        // cannot dismiss a menu the user opens while the request is pending.
        if menu_open.peek().as_deref() == Some(id.as_str()) {
            menu_open.set(None);
        }
        let base = stop_base.clone();
        // Cloned per invocation: the spawned task takes ownership of what it
        // captures, and this handler runs once per stop click.
        let stop_recovery = stop_recovery.clone();
        let mut commit_listing = stop_commit_listing.clone();
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
    let delete_popovers = popovers.clone();
    // `guard` is what the prompt the user answered covered, or
    // `NothingAlive` for the one caller that deletes without asking (see
    // `on_delete`): the row it decided from can be stale, and the supervisor
    // then refuses rather than kills what nobody confirmed.
    //
    // Returns whether the delete started. Only the header path reads it: the
    // row's own callers refuse busy rows before they get here.
    let mut do_delete = move |id: String, guard: crate::DeleteGuard| -> bool {
        if !begin_row_op(&id) {
            return false;
        }
        // Shown from here until the reply: the row and, if this session is
        // open, the header switch to their in-progress state. Cleared beside
        // each `end_row_op` below, on success and on failure alike.
        deleting.write().insert(id.clone());
        super::trash::capture_row(&id);
        let base = delete_base.clone();
        let refresh = delete_refresh.clone();
        let popovers = delete_popovers.clone();
        spawn(async move {
            let outcome = delete_session(&base, &id, guard).await;
            let archived = outcome.as_ref().is_ok_and(|reply| reply.archived);
            super::trash::finish_row_cue(&id, archived);
            if let Ok(reply) = &outcome {
                delete_notice.publish(reply.notice.clone());
                if reply.archived {
                    trash_revision += 1;
                }
            }
            match outcome.err() {
                Some(e) => {
                    errors.write().insert(id.clone(), format!("delete: {e}"));
                    deleting.write().remove(&id);
                    end_row_op(&id);
                    // A refusal changed nothing on the host, so no fleet
                    // event will correct a stale row; read the listing now,
                    // so a row the supervisor found alive shows that and
                    // the next Delete decides from current state under the
                    // same session-confirmation preference.
                    refresh(Trigger::Explicit);
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
                        let open_anchor = popovers.first_open_row(AnchorRegion::Sessions);
                        // Judged on the displayed order, which an order hold
                        // can make differ from the served one. The menu and
                        // bell exclude each other; check their sole anchor
                        // before removal discards the old layout baseline.
                        let held = order_hold.peek().clone().unwrap_or_default();
                        if remove_row_reporting_menu_move(
                            &mut current.sessions,
                            &id,
                            open_anchor.as_deref(),
                            &held,
                        ) {
                            // The old paired clear also wrote the closed slot.
                            // Keep those effect notifications, including when
                            // removal took the anchor itself out of the list.
                            popovers.close_registered(AnchorRegion::Sessions);
                        }
                    }
                    // A listing read already in flight started before the
                    // helm finished this delete, and it would otherwise
                    // commit as the newest read (the refresh below only
                    // queues behind it), putting the deleted row back with
                    // its Delete button live until the refresh lands. The
                    // fence supersedes every read that started before this
                    // confirmed delete (see `ReadGate::fence`).
                    listing_reads.write().fence();
                    deleting.write().remove(&id);
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
        true
    };

    // The delete button's initial click: decides whether this id needs
    // confirming at all. A skipped question uses the same request and guard
    // as its confirm button, from the snapshot the clicked row supplied.
    //
    // SPEC.md's "Lifecycle operations": delete confirms first only when
    // the agent might still be alive, unless session questions are skipped;
    // an ended, tabless session deletes immediately (see the residual risk
    // below). With the preference off, live and Unknown statuses open the
    // row's Delete question (`row_questions`), whose opening `SessionRow`
    // reads to swap its action area; `confirm_delete` answers that prompt.
    //
    // Refuses an id already in `row_pending` (the cross-guard mirroring
    // `on_stop`'s question check above): the delete button is only
    // disabled once a rerender following a `row_pending` insert lands, so a
    // rapid click queued just ahead of that rerender could otherwise
    // still reach this handler while, say, a stop is already in flight
    // for the same session. Refusing here is what keeps the question from
    // ever opening in that window — closing the door on the
    // opposite race `on_stop`'s guard closes. `do_delete`'s OWN
    // `row_pending` guard (not `confirm_delete`'s opening check, which
    // exists for a different race entirely — see its own doc below) would
    // eventually refuse this same id too, but only AFTER a confirm prompt
    // had already opened for nothing; catching it here means the prompt
    // never opens in the first place. A row asking its Replace question is
    // refused for the same reason: one question per row.
    let mut do_delete_on_confirm = do_delete.clone();
    let on_delete = move |target: DeleteTarget| {
        if row_pending.read().contains(&target.id)
            || row_questions.peek_key(&target.id) == Some(RowQuestion::Replace)
            || rename_editor
                .read()
                .as_ref()
                .map(|editor| editor.id.as_str())
                == Some(target.id.as_str())
        {
            return;
        }
        // With session questions enabled, the agent half of the split is `has_ended()` vs.
        // everything else, which is to say: a status the UI knows to be
        // finished can delete straight away, and the rest — the three live
        // statuses and `Unknown` — go through a confirmation. Asking the status rather than
        // listing the variants is what kept this correct THROUGH M6.75's
        // liveness split, which added two live variants and needed no edit
        // here (see `SessionStatus::has_ended`).
        if let DeleteClick::DeleteGuarded(guard) = target.click(skip_session_confirmations) {
            // With the preference off, only an ended agent with no tabs
            // reaches this path. With it on, the rendered guard limits the
            // direct click's consent: AgentEnded still protects a new agent.
            //
            // The default unconfirmed path accepts a known residual: the AGENT
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
            do_delete_on_confirm(target.id, guard);
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
            //
            // A Delete question already open on this row stays as it is
            // rather than reopening: a click queued just ahead of the
            // render that swapped the menu for the prompt must not retire
            // the opening that render's answers carry. (The Replace
            // question was refused above, so an open lane here is Delete.)
            if row_questions.peek_key(&target.id).is_none() {
                row_questions.open(target.id, RowQuestion::Delete, ());
            }
        }
    };

    // The confirm-delete button's click, inside the inline prompt: the
    // exact same DELETE call an accepted `window.confirm()` used to
    // trigger before this rewrite, just reached from a different UI
    // widget. Takes the question first so the row falls back to its
    // normal (busy/disabled) button layout the instant `do_delete`'s own
    // `row_pending` insert takes effect, rather than momentarily showing
    // both the prompt and a busy state.
    //
    // Proceeds ONLY when `take` hands back the opening the row drew: it
    // refuses whenever this confirmation was already resolved by
    // something else — `cancel_delete` running first (a queued confirm
    // click landing just after a cancel click, both fired in the same
    // burst), a second confirm click racing the first's own take, or a
    // cancel and a reopen in the same burst, which leave this click
    // holding an earlier showing's token. Without this check, that click
    // would fall through to `do_delete` regardless, which for the
    // cancel-then-confirm race would delete a session the user just told
    // the UI to leave alone.
    let mut header_do_delete = do_delete.clone();
    // `guard` and `token` come from the row render that drew the prompt the
    // user answered (`status::delete_guard`), not from the row as it reads now.
    let confirm_delete =
        move |(id, token, guard): (String, ConfirmToken<RowQuestion>, crate::DeleteGuard)| {
            // Refused OUTRIGHT while the shared token is held, BEFORE the
            // question is touched: `do_delete`'s own `begin_row_op`
            // would refuse anyway, but by then the question is gone and the
            // prompt has silently dismissed itself with nothing deleted —
            // the confirmation must survive a refusal so the user can answer
            // it again once the other pane's operation finishes.
            if ops.busy_now() {
                return;
            }
            if *token.key() != RowQuestion::Delete || row_questions.take(&id, &token).is_none() {
                return;
            }
            do_delete(id, guard);
        };

    // The inline prompt's cancel button: closes the opening it was drawn
    // for, and nothing later. No API call, no `row_pending` involvement —
    // cancelling was never in flight to begin with.
    let cancel_delete = move |(id, token): (String, ConfirmToken<RowQuestion>)| {
        if *token.key() == RowQuestion::Delete {
            row_questions.cancel_for(&id, &token);
        }
    };

    // A delete the session header already confirmed. The header released its
    // lifecycle claim just before sending this, because `begin_row_op`
    // refuses while the shared token is held, so a request that still finds
    // the page busy lost a race in the frame between the two: another
    // operation claimed the token, or this row is engaged on its own (an
    // operation in flight, or its sidebar question open). Nothing was deleted
    // then, and the refusal is shown on the session's row, where every other
    // delete failure is reported, rather than dropped silently.
    use_effect(move || {
        let Some(request) = header_delete.read().clone() else {
            return;
        };
        header_delete.set(None);
        if !header_do_delete(request.id.clone(), request.guard) {
            errors.write().insert(
                request.id,
                "delete: not started, another operation is in progress; try again".to_string(),
            );
        }
    });

    // Replace shares the per-row operation gate with stop, rename, and
    // delete (`begin_row_op`, which is also what enforces the
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
    // Local question state, over the same page-owned retry and source exclusion
    // used by the header. Admission and successful selection remain list-owned.
    let yolo_replace = use_confirm_slot::<PendingReplace, ()>();
    let yolo_replace_error = use_signal(|| None::<String>);
    let replace_controller = ReplaceController::sidebar(
        use_context::<ReplaceIntent>().0,
        use_context::<ReplaceYoloQuestions>().0,
        yolo_replace,
        yolo_replace_error,
    );
    use_drop(move || replace_controller.release_question());
    // The caller has already begun the source row's operation. Reserve before
    // spawning; keep the row admitted until success callbacks and refresh finish.
    // YOLO answers carry the originally confirmed source and delete precondition.
    let mut do_replace =
        move |source: Session, guard: crate::DeleteGuard, answered: Option<ReplaceAnswer>| {
            let id = source.id.raw().to_owned();
            errors.write().remove(&id);
            let base = replace_base.clone();
            let refresh = replace_refresh.clone();
            let attempt = replace_controller.prepare(source.clone(), guard, answered);
            spawn(async move {
                match replace_controller.run(&base, attempt).await {
                    ReplaceResult::Replaced(session, notice) => {
                        delete_notice.publish(notice);
                        // A pre-replace listing cannot establish the new session's
                        // absence. Fence it before selecting the bare reply.
                        listing_reads.write().fence();
                        let session = super::with_source_host(*session, &source);
                        remember_selection(&base, preferences, session.id.raw());
                        on_open.call(session);
                        refresh(Trigger::Explicit);
                    }
                    ReplaceResult::Refused(reason) => {
                        // Partial failure belongs beside the source row; its
                        // endpoint text already identifies the replacement too.
                        errors
                            .write()
                            .insert(id.clone(), format!("replace: {reason}"));
                    }
                    ReplaceResult::QuestionHandled => {}
                }
                end_row_op(&id);
            });
        };
    // The "replace" menu item's click: guarded by the SAME predicate
    // `on_clone` uses (`clone_is_refused`'s own doc explains why the two
    // share it — the difference between clone and replace is never in
    // whether either is refused, only in what happens once accepted), then
    // opens the row's Replace question when session questions are enabled,
    // unlike `on_delete`'s
    // `has_ended()` split. Delete's split exists because an already-finished
    // session has nothing left for the CONFIRMATION to be honest about
    // beyond "delete anyway"; replace always has something worth confirming
    // regardless of liveness — a fresh session is about to take this row's
    // place — so `status::replace_consequence` is total over every status
    // (no `Option` to skip on) and this handler never bypasses the prompt
    // once the guard above lets it through, unless the user opted out.
    // The direct route acquires the same row operation and uses the same
    // rendered source and guard, including the existing YOLO refusal path.
    let mut direct_do_replace = do_replace.clone();
    let on_replace = move |session: Session| {
        if clone_is_refused(
            ops.busy_now(),
            session.id.raw(),
            &row_pending.read(),
            row_questions.peek_key(session.id.raw()),
            rename_editor
                .read()
                .as_ref()
                .map(|editor| editor.id.as_str()),
        ) {
            return;
        }
        replace_controller.select_source(session.id.raw());
        if skip_session_confirmations {
            let guard = crate::status::delete_guard(&session.status, session.tabs.len());
            if begin_row_op(session.id.raw()) {
                direct_do_replace(session, guard, None);
            }
        } else {
            // `clone_is_refused` above refused a row already asking, so
            // this opening never replaces another question of this row.
            row_questions.open(session.id.raw().to_owned(), RowQuestion::Replace, ());
        }
    };
    // The loud YOLO confirmation's own way back into the same replace.
    let yolo_do_replace = do_replace.clone();
    // The row hands up the session its prompt was drawn from, so the
    // nothing-alive answer is the one the user read, and a row that has since
    // left the listing is still replaced with its precondition rather than
    // looked up and missed. The token is that same render's opening: an
    // answer drawn for an earlier showing of the question takes nothing.
    let confirm_replace = move |(source, token): (Session, ConfirmToken<RowQuestion>)| {
        // Same shared-token refusal as `confirm_delete`, for the same
        // keep-the-prompt reason.
        if ops.busy_now() {
            return;
        }
        if *token.key() == RowQuestion::Replace
            && row_questions.take(source.id.raw(), &token).is_some()
            && begin_row_op(source.id.raw())
        {
            let guard = crate::status::delete_guard(&source.status, source.tabs.len());
            do_replace(source, guard, None);
        }
    };
    // Releases the Replace source only when this cancel closed the opening
    // it was drawn for; a stale cancel leaves the newer question and its
    // source selection alone.
    let cancel_replace = move |(id, token): (String, ConfirmToken<RowQuestion>)| {
        if *token.key() == RowQuestion::Replace && row_questions.take(&id, &token).is_some() {
            replace_controller.cancel_source(&id);
        }
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
    // pending operation or opening a question) would otherwise still reach
    // here and replace the form's prefill, or open it, out from under
    // whatever the other operation is doing. `ops.busy_now()` covers the
    // page-wide lock (a create submit already minting a key, a host mutation
    // in flight); the three row-local states cover this SPECIFIC row having a
    // stop/delete in flight, a destructive confirmation open, or its
    // rename editor open — any of which means this row's own `Session` is
    // about to change or is mid-decision, not a stable thing to snapshot
    // into a fresh clone right now.
    let on_clone = move |session: Session| {
        if clone_is_refused(
            ops.busy_now(),
            session.id.raw(),
            &row_pending.read(),
            row_questions.peek_key(session.id.raw()),
            rename_editor
                .read()
                .as_ref()
                .map(|editor| editor.id.as_str()),
        ) {
            return;
        }
        menu_open.set(None);
        let generation = clone_prefill
            .peek()
            .as_ref()
            .map_or(0, |prefill| prefill.generation + 1);
        opening_generation += 1;
        quick_launch_pending.set(false);
        auto_launch.set(false);
        initial_create_action.set(None);
        clone_prefill.set(Some(prefill_from(&session, generation)));
        show_create.set(true);
    };

    // The last (status, tab count) a successful listing showed for the open
    // "replace with" launcher's source, keyed by its id. The launcher falls
    // back to it while the current listing lacks the row, so a failed refresh
    // or a filter hiding the source keeps the warning at the newest state the
    // page has seen rather than reverting it to the one from when the
    // launcher opened (see the `replace_source_state` prop below). It belongs
    // to one opening: both "replace with" paths clear it before publishing a
    // new prefill, or a launcher reopened on the same source while the
    // listing lacks it would show what an earlier opening saw instead of its
    // own, newer snapshot.
    let mut replace_source_seen = use_signal(|| None::<(String, (SessionStatus, usize))>);
    use_effect(move || {
        let Some(source) = clone_prefill
            .read()
            .as_ref()
            .and_then(|prefill| prefill.mode.replace_source().map(str::to_owned))
        else {
            return;
        };
        if let Some(state) = listed_state(&listing.read(), &source) {
            let seen = Some((source, state));
            if *replace_source_seen.peek() != seen {
                replace_source_seen.set(seen);
            }
        }
    });

    // The "replace with" menu item's click — `on_clone`'s body verbatim
    // (same guard, same shared `clone_prefill` signal, same
    // generation-minting scheme, same reasons for both) except that the
    // prefill it builds is marked with THIS row as its replace-with source
    // (`CreatePrefill::mode`). Replace mode makes the shared submit path call
    // `api::replace_session_with` instead of `api::create_session`, and
    // makes this closing form's `on_created` request an explicit
    // listing refresh so the source row leaves the list promptly (see that
    // handler's own doc). See `on_clone`'s doc above for why the guard runs
    // before any signal is touched and why the generation is minted here
    // rather than by the form.
    let on_replace_with = move |session: Session| {
        if clone_is_refused(
            ops.busy_now(),
            session.id.raw(),
            &row_pending.read(),
            row_questions.peek_key(session.id.raw()),
            rename_editor
                .read()
                .as_ref()
                .map(|editor| editor.id.as_str()),
        ) {
            return;
        }
        replace_controller.cancel_all();
        menu_open.set(None);
        let generation = clone_prefill
            .peek()
            .as_ref()
            .map_or(0, |prefill| prefill.generation + 1);
        let mut prefill = prefill_from(&session, generation);
        mark_replace_with(&mut prefill, &session);
        // A new opening starts from its own snapshot, never from what an
        // earlier launcher on the same source last saw.
        replace_source_seen.set(None);
        opening_generation += 1;
        quick_launch_pending.set(false);
        auto_launch.set(false);
        initial_create_action.set(None);
        clone_prefill.set(Some(prefill));
        show_create.set(true);
    };

    // Header actions arrive through the single AppBody-owned request signal;
    // consuming and clearing it here prevents a keyed view remount from
    // replaying an old clone, replace-with or restart-with request.
    use_effect(move || {
        let Some(request) = prefill_request.read().clone() else {
            return;
        };
        let (session, bridge) = match request {
            HeaderPrefillRequest::Clone(session) | HeaderPrefillRequest::ReplaceWith(session) => {
                (session, None)
            }
            HeaderPrefillRequest::RestartWith(session, bridge) => (session, Some(bridge)),
        };
        if clone_is_refused(
            ops.busy_now(),
            session.id.raw(),
            &row_pending.read(),
            row_questions.peek_key(session.id.raw()),
            rename_editor
                .read()
                .as_ref()
                .map(|editor| editor.id.as_str()),
        ) {
            prefill_request.set(None);
            return;
        }
        let generation = clone_prefill
            .peek()
            .as_ref()
            .map_or(0, |prefill| prefill.generation + 1);
        let replace_with = matches!(
            prefill_request.peek().as_ref(),
            Some(HeaderPrefillRequest::ReplaceWith(_))
        );
        let mut prefill = prefill_from(&session, generation);
        if replace_with {
            replace_controller.cancel_all();
            mark_replace_with(&mut prefill, &session);
            replace_source_seen.set(None);
        }
        if bridge.is_some() && !mark_restart_with(&mut prefill, &session) {
            prefill_request.set(None);
            return;
        }
        restart_bridge.set(bridge);
        opening_generation += 1;
        quick_launch_pending.set(false);
        auto_launch.set(false);
        initial_create_action.set(None);
        clone_prefill.set(Some(prefill));
        menu_open.set(None);
        // Main-pane actions can enter while Hosts is visible. Reveal the
        // launcher's ordinary return control only after admission succeeds.
        host_menu_open.set(None);
        hosts_panel_open.set(false);
        show_create.set(true);
        prefill_request.set(None);
    });

    // Opening Rename owns one editor above the keyed rows. The source menu
    // closes before the dialog mounts, so no fixed-position popup remains to
    // reappear after completion or detach while the list reorders.
    let on_rename_start = move |(id, title, shortcut, naming): (String, String, bool, bool)| {
        // The shared token counts too: the disabled attribute on the menu's
        // rename control is cosmetic (a dispatched synthetic click still
        // reaches this handler), and opening the field while the other
        // pane's operation owns the gate would present an editor whose
        // submit is guaranteed to be refused.
        if ops.busy_now()
            || row_pending.read().contains(&id)
            || row_questions.peek_key(&id).is_some()
            || rename_editor.peek().is_some()
        {
            return;
        }
        // Retire a held quick read here so cancelling Rename cannot revive it.
        opening_generation += 1;
        quick_launch_pending.set(false);
        auto_launch.set(false);
        rename_draft.set(crate::peer::editable_peer(&title));
        rename_error.set(None);
        rename_focus_owned.set(false);
        // A new editor is a distinct focus lifetime, even if the previous
        // one named the same session. It must cancel any queued old return.
        rename_focus_return.set(None);
        let generation = rename_generation.peek().wrapping_add(1);
        rename_generation.set(generation);
        rename_editor.set(Some(RenameEditor {
            id: id.clone(),
            current_title: title,
            generation,
            unavailable: false,
            shortcut,
            naming,
        }));
        if menu_open.peek().as_deref() == Some(id.as_str()) {
            menu_open.set(None);
        }
    };

    // The rename editor's submit. The title goes to the supervisor exactly
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
    let on_rename_submit = move |(generation, title): (u64, String)| {
        let Some(editor) = rename_editor.peek().clone() else {
            return;
        };
        if editor.generation != generation || editor.unavailable {
            return;
        }
        let terminal_return = editor.shortcut || editor.naming;
        let keep_current_title = naming_keeps_current_title(&editor, &title);
        let id = editor.id;
        if !begin_row_op(&id) {
            return;
        }
        if keep_current_title {
            // Use the normal operation admission even though no request follows:
            // an event racing another pane's write must retain the busy guard.
            end_row_op(&id);
            rename_focus_return.set(Some((id, generation, terminal_return)));
            crate::rename::dialog_handle(generation).release();
            rename_editor.set(None);
            rename_error.set(None);
            return;
        }
        // This row's own previous failure, cleared by the retry that
        // supersedes it and by nothing else (see `errors`).
        errors.write().remove(&id);
        if rename_error
            .peek()
            .as_ref()
            .is_some_and(|(error_generation, _)| *error_generation == generation)
        {
            rename_error.set(None);
        }
        let base = rename_base.clone();
        let refresh = rename_refresh.clone();
        spawn(async move {
            match rename_session(&base, &id, &title).await {
                Ok(session) => {
                    // Accepted titles enter the existing local rename state as
                    // raw bytes, never as the editor's escaped spelling.
                    on_renamed.call((id.clone(), session.title.raw().to_owned()));
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
                        .insert(id.clone(), (session.title.raw().to_owned(), observed_from));
                    // The response owns only the exact editor it started.
                    // Any later editor for the same id is a new generation,
                    // so old deferred work cannot close its fresh draft or
                    // return focus over a control the user has since reached.
                    if rename_result_owns_editor(rename_editor.peek().as_ref(), generation) {
                        if rename_focus_owned() {
                            rename_focus_return.set(Some((
                                id.clone(),
                                generation,
                                terminal_return,
                            )));
                        }
                        // Even a close that owes no return must release its own
                        // isolation before removing the generation it belongs to.
                        crate::rename::dialog_handle(generation).release();
                        rename_editor.set(None);
                        rename_error.set(None);
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
                    if rename_result_owns_editor(rename_editor.peek().as_ref(), generation) {
                        rename_error.set(Some((generation, e)));
                    }
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
        if ops.busy_now() || !row_pending.peek().is_empty() {
            return false;
        }
        remember_selection(&open_base, preferences, session.id.raw());
        on_open.call(session);
        true
    };

    // Stable callback identities are created once, outside the row loop.
    // The row-specific session or id travels as the callback argument, so
    // no hook depends on fleet size and unchanged rows remain memoized when
    // their parent refreshes.
    let guarded_open = use_callback(guarded_open);
    // Sidebar rows do not consume the acceptance result; the switcher does
    // so a busy refusal preserves both the filter and the opener's focus.
    let row_open = use_callback(move |session| {
        guarded_open.call(session);
    });
    // An empty pane uses one rule on startup and after a noticed removal:
    // first listed visit, else newest-created among the same eligible rows.
    // A filter or cutoff narrows candidates; it does not prove that an
    // already-open session disappeared (that separate gate remains above).
    // Re-read visits just before choosing so another window's choices count,
    // then remember the pick through the same sparse writer as a row click.
    let resolve_base = base.clone();
    let mut resolving_selection = use_signal(|| false);
    // Keep one transient exclusion across Some -> None. A local delete can
    // clear the pane before the refreshed listing drops its old row.
    let mut last_opened = use_signal(|| None::<String>);
    use_effect(move || {
        if let Some(id) = selected.read().as_ref() {
            if last_opened.peek().as_ref() != Some(id) {
                last_opened.set(Some(id.clone()));
            }
            return;
        }
        // Tracked admission reads matter: a pick deferred during an operation
        // must run when it ends, even if no new fleet event arrives.
        let page_busy = ops.busy();
        let rows_busy = *row_ops.read() > 0;
        let listing_read = listing.read();
        let Some(Ok(listing_ok)) = listing_read.as_ref() else {
            return;
        };
        if page_busy || rows_busy {
            return;
        }
        if !listing_ok
            .sessions
            .iter()
            .any(|session| last_opened.peek().as_deref() != Some(session.id.raw()))
            || *resolving_selection.read()
        {
            return;
        }
        resolving_selection.set(true);
        let base = resolve_base.clone();
        spawn(async move {
            let fresh = api::fetch_preferences(&base).await;
            // A manual selection or operation admitted during the read wins.
            // Choose from the latest committed rows, not a pre-await snapshot.
            if selected.peek().is_none()
                && !ops.busy_now()
                && *row_ops.peek() == 0
                && row_pending.peek().is_empty()
            {
                let mut preferences = preferences;
                if let Ok(fresh) = fresh {
                    // An incidental read must not refresh unrelated settings
                    // or replace optimistic choices in their open controls.
                    let mut local = preferences.0.write();
                    local.last_selected = fresh.last_selected;
                    local.visit_history = fresh.visit_history;
                }
                let candidate = listing
                    .peek()
                    .as_ref()
                    .and_then(|result| result.as_ref().ok())
                    .and_then(|rows| {
                        automatic_selection(
                            &rows.sessions,
                            &preferences.0.peek().visit_history,
                            preferences
                                .0
                                .peek()
                                .last_selected
                                .as_ref()
                                .map(PeerText::raw),
                            last_opened.peek().as_deref(),
                        )
                        .cloned()
                    });
                if let Some(session) = candidate {
                    remember_selection(&base, preferences, session.id.raw());
                    on_open.call(session);
                }
            }
            resolving_selection.set(false);
        });
    });
    // Snapshot concrete signals, without adding reactive subscriptions or
    // another scheduled effect. Selection agreement keeps a not-yet-mounted
    // session view from looking like an idle listing-only page to feed tests.
    let snapshot_feed = crate::feed::FLEET_FEED.signal();
    let snapshot_skew = crate::skew::HELM_BUILD_SKEW.signal();
    crate::reader::use_test_reader_snapshot(move || {
        let feed = snapshot_feed.peek();
        let rows = listing.peek();
        serde_json::json!({
            "role": "list",
            "notices": feed.notices.to_string(),
            "healthy": feed.healthy,
            "skew": snapshot_skew.peek().is_some(),
            "acted_on": feed_acted_on.peek().to_string(),
            "selected": selected.peek().clone(),
            "listing_answered": matches!(rows.as_ref(), Some(Ok(_))),
            "has_rows": rows.as_ref().is_some_and(|result| result.as_ref().is_ok_and(|rows| !rows.sessions.is_empty())),
            "resolving_selection": *resolving_selection.peek(),
            "readers": [listing_surface.peek().test_snapshot(), hosts_surface.peek().test_snapshot()],
        })
    });
    // End the order hold if `rows::hold_should_release` says so now: the
    // pointer has left the list or been still long enough, and no session
    // row's menu is open. Nothing else ends a hold except a reply to a
    // different query (see `order_hold`), and because this checks the current
    // facts rather than remembering events, a leave or a stillness timeout
    // that happened while a menu was open simply takes effect on the next
    // check after the menu closes.
    let try_release_order_hold = {
        let hold_clock = hold_clock.clone();
        let pointer_inside = pointer_inside.clone();
        move || {
            if order_hold.peek().is_none() {
                return;
            }
            // An open notification list holds the order like a menu does:
            // its rows must not move out from under it.
            if rows::hold_should_release(
                menu_open.peek().is_some() || bell_open.peek().is_some(),
                pointer_inside.get(),
                hold_clock.get().elapsed().as_millis(),
                ORDER_HOLD_STILL_MS,
            ) {
                order_hold.set(None);
            }
        }
    };
    // Any pointer activity over the list: movement, a press, or a wheel
    // scroll (which moves the rows under a still pointer, so it counts as
    // activity too). It restarts the stillness clock, and the first one
    // takes the hold, snapshotting the order on screen and starting the
    // watcher that applies the release rule every `ORDER_HOLD_POLL_MS` until
    // the hold ends.
    let note_list_pointer = {
        let hold_clock = hold_clock.clone();
        let hold_generation = hold_generation.clone();
        let pointer_inside = pointer_inside.clone();
        let try_release_order_hold = try_release_order_hold.clone();
        move || {
            pointer_inside.set(true);
            hold_clock.set(web_time::Instant::now());
            if order_hold.peek().is_some() {
                return;
            }
            let ids: Vec<String> = match listing.peek().as_ref() {
                Some(Ok(current)) => current
                    .sessions
                    .iter()
                    .map(|s| s.id.raw().to_owned())
                    .collect(),
                _ => return,
            };
            order_hold.set(Some(ids));
            let generation = hold_generation.get() + 1;
            hold_generation.set(generation);
            let hold_generation = hold_generation.clone();
            let mut try_release_order_hold = try_release_order_hold.clone();
            spawn(async move {
                loop {
                    crate::reader::sleep_ms(ORDER_HOLD_POLL_MS).await;
                    if hold_generation.get() != generation || order_hold.peek().is_none() {
                        return;
                    }
                    try_release_order_hold();
                }
            });
        }
    };
    // The pointer left the list: release now unless a menu is open, in which
    // case the watcher releases once it closes.
    let leave_list = {
        let pointer_inside = pointer_inside.clone();
        let mut try_release_order_hold = try_release_order_hold.clone();
        move || {
            pointer_inside.set(false);
            try_release_order_hold();
        }
    };
    let menu_opening_registration = menu_registration.clone();
    let toggle_menu = use_callback(move |id: String| {
        let currently = menu_open.peek().as_deref() == Some(id.as_str());
        if !currently {
            menu_opening_registration.forget_anchor();
        }
        menu_open.set(if currently { None } else { Some(id) });
        // Opening a session row's menu must close whichever host row's menu
        // is open — see `HostsPanel`'s own "one row menu open, across BOTH
        // panels" doc.
        if !currently {
            host_menu_open.set(None);
            bell_open.set(None);
        }
    });
    // The notification bell's click opens its session's list, closing any
    // other floating surface in the sidebar, or closes it when it is already
    // the open one (SPEC.md, Status).
    let toggle_read_sent = bell_read_sent.clone();
    let bell_opening_registration = bell_registration.clone();
    let toggle_bell = use_callback(move |(id, read_through, newest): (String, u64, u64)| {
        let currently = bell_open.peek().as_ref().is_some_and(|open| open.id == id);
        if currently {
            bell_open.set(None);
            return;
        }
        menu_open.set(None);
        host_menu_open.set(None);
        let sent = toggle_read_sent.borrow().get(&id).copied().unwrap_or(0);
        bell_opening_registration.forget_anchor();
        bell_open.set(Some(OpenBell {
            id,
            read_through: read_through.max(sent),
            shown: newest,
        }));
    });
    // A host row's menu opening closes the list, as a session menu opening
    // does (`toggle_menu`). The hosts panel owns that transition and knows
    // only the session menu, so this watches for it instead; a keyboard
    // opening sends no pointer-down for the outside-click dismissal to see.
    use_effect(move || {
        if host_menu_open().is_some() && bell_open.peek().is_some() {
            bell_open.set(None);
        }
    });
    let close_bell = use_callback(move |id: String| {
        if bell_open.peek().as_ref().is_some_and(|open| open.id == id) {
            bell_open.set(None);
        }
    });
    // Closing the list marks everything it showed read, whatever closed it:
    // the bell, Escape, a click elsewhere, the row moving or leaving, or a
    // clear. Watching the open state rather than writing at each close site
    // is what keeps a newly added close path from forgetting the mark.
    //
    // "Everything it showed" is the closing list's own `OpenBell::shown`,
    // from the last state this effect saw before the close, never the
    // listing that closed it, which may have removed the row (a filter) or
    // carry an entry the list never displayed. Nothing is sent when the list
    // showed nothing above the read mark; resolved entries above it are covered
    // too. Best effort and silent on failure (SPEC.md,
    // Errors and diagnostics): a lost mark leaves the bell loud until the
    // list is next closed, which the user would only notice as a bell that
    // stayed loud.
    // A successful mark refreshes explicitly: the bell must quiet even
    // while a build mismatch withdraws feed and fallback reads.
    let read_base = base.clone();
    let read_refresh = request_listing.clone();
    let previous_bell = use_hook(|| Rc::new(RefCell::new(None::<OpenBell>)));
    use_effect(move || {
        let now_open = bell_open.read().clone();
        let closed: Option<OpenBell> = previous_bell.replace(now_open.clone());
        let Some(closed) =
            closed.filter(|closed| now_open.as_ref().map(|open| &open.id) != Some(&closed.id))
        else {
            return;
        };
        if closed.shown <= closed.read_through {
            return;
        }
        let (closed, through) = (closed.id, closed.shown);
        bell_read_sent.borrow_mut().insert(closed.clone(), through);
        let sent = bell_read_sent.clone();
        let base = read_base.clone();
        let refresh = read_refresh.clone();
        spawn(async move {
            if let Err(error) = crate::api::mark_notifications(&base, &closed, through, false).await
            {
                // An older failure must not erase a newer close's mark. Only
                // release our still-current mark so the next close can retry.
                let mut sent = sent.borrow_mut();
                if sent.get(&closed) == Some(&through) {
                    sent.remove(&closed);
                }
                dioxus::logger::tracing::warn!(
                    session_id = closed.as_str(),
                    error = error.as_str(),
                    "could not mark the session's notifications read"
                );
            } else {
                refresh(Trigger::Explicit);
            }
        });
    });
    // The list's clear button: closes the list (which also marks it read,
    // above) and clears through the newest entry it showed. Unlike the read
    // mark, a failed clear is something the user asked for directly, so it
    // surfaces on the row's error line like any other operation (SPEC.md,
    // Errors and diagnostics).
    // Like read-on-close, success waits for the gated listing refresh to
    // remove the bell; no optimistic edit needs a rename-style guard.
    let clear_base = base.clone();
    let clear_refresh = request_listing.clone();
    let clear_bell = use_callback(move |(id, through): (String, u64)| {
        if bell_open.peek().as_ref().is_some_and(|open| open.id == id) {
            bell_open.set(None);
        }
        let base = clear_base.clone();
        let refresh = clear_refresh.clone();
        spawn(async move {
            let result = crate::api::mark_notifications(&base, &id, through, true).await;
            let Ok(mut errors) = errors.try_write() else {
                return;
            };
            match result {
                Ok(()) => {
                    errors.remove(&id);
                    drop(errors);
                    refresh(Trigger::Explicit);
                }
                Err(error) => {
                    errors.insert(id, format!("clear notifications: {error}"));
                }
            }
        });
    });
    let pin_base = base.clone();
    let pin_refresh = request_listing.clone();
    // A manual metadata edit reports failure on its row and reads success
    // back from the helm. It closes the menu at acceptance like Mark read;
    // the pointer hold still decides when that new membership may move it.
    let on_pin = use_callback(move |(id, pinned): (String, bool)| {
        menu_open.set(None);
        let base = pin_base.clone();
        let refresh = pin_refresh.clone();
        spawn(async move {
            let result = api::set_pin(&base, &id, pinned).await;
            let Ok(mut errors) = errors.try_write() else {
                return;
            };
            match result {
                Ok(()) => {
                    errors.remove(&id);
                    drop(errors);
                    refresh(Trigger::Explicit);
                }
                Err(error) => {
                    errors.insert(id, format!("pin: {error}"));
                }
            }
        });
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
    // exactly like `on_stop`/`on_delete`/`on_rename_submit`
    // above (SPEC.md, Errors and diagnostics) — the user asked for this one
    // directly, so losing it silently would be exactly the kind of
    // succeeded-when-it-failed illusion that section forbids.
    // Its root-owned writer must not call the list's reader directly: it
    // may finish after unmount, and any task it spawned would be root-owned
    // too. Success bumps list state through try_write; the list's effect
    // above turns that into the same explicit refresh as the bell actions.
    let on_mark_seen = move |(id, seen_activity_at): (String, Option<i64>)| {
        // The status dot shares this handler. Only dismiss this row's menu,
        // and do it at acceptance rather than when the queued write returns.
        if menu_open.peek().as_deref() == Some(id.as_str()) {
            menu_open.set(None);
        }
        queue_seen_write(
            &mark_seen_base,
            &id,
            seen_activity_at,
            seen_toggle_report(errors, seen_toggle_successes, id.clone()),
        );
    };
    let on_mark_seen = use_callback(on_mark_seen);
    let on_stop = use_callback(on_stop);
    let on_delete = use_callback(on_delete);
    let confirm_delete = use_callback(confirm_delete);
    let cancel_delete = use_callback(cancel_delete);
    let on_clone = use_callback(on_clone);
    let on_replace = use_callback(on_replace);
    let confirm_replace = use_callback(confirm_replace);
    let cancel_replace = use_callback(cancel_replace);
    let on_rename_start = use_callback(on_rename_start);
    let on_rename_row =
        use_callback(move |(id, title)| on_rename_start.call((id, title, false, false)));
    let on_rename_submit = use_callback(on_rename_submit);
    let on_rename_cancel = use_callback(move |generation| {
        let current_editor = rename_editor.peek().clone();
        if let Some(editor) = current_editor
            && editor.generation == generation
            && !row_pending.read().contains(&editor.id)
        {
            // Both cancellation paths originate inside the dialog: a click
            // on Cancel or an ordinary Escape from one of its controls.
            rename_focus_return.set(Some((
                editor.id,
                generation,
                editor.shortcut || editor.naming,
            )));
            crate::rename::dialog_handle(generation).release();
            rename_editor.set(None);
            rename_error.set(None);
        }
    });
    // Cosmetic reflection of the same conditions, for the disabled
    // attributes. Not the guard — see `ops`.
    let busy = ops.busy();
    let nav_locked = busy || !row_pending.read().is_empty();
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

    let request_host_filter = request_host_filter.clone();
    // The create form's own copy of the API base, for recording a created
    // session as the selection (see `remember_selection`).
    let created_base = base.clone();
    // The create form's own copy of the listing requester, for the explicit
    // refresh a successful replace-with issues (see `on_created` below) —
    // the same "do not wait for the feed" nudge `do_replace` gives an
    // ordinary replace's own success path.
    let created_listing = request_listing.clone();
    // The order hold that applies to the list being shown, if any, as the
    // ids `rows::held_display_order` keeps in place. Read (not peeked) so a
    // hold starting or ending re-renders the rows.
    let held_order: Vec<String> = order_hold.read().clone().unwrap_or_default();
    let displayed = listing
        .read()
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .map(|listing| {
            apply_optimistic_renames(
                &rows::held_display_order(&held_order, &listing.sessions),
                &renamed.read(),
            )
        });
    let pin_separator = displayed
        .as_deref()
        .and_then(rows::pin_separator_after)
        .map(str::to_string);
    // Prepare the exact local delete/activity values once for both the copy
    // comparison and the row props. The existing clock read above remains the
    // sole time source, and held rows retain their current accepted fields.
    let displayed_rows = displayed.map(|sessions| {
        let deleting = deleting.read();
        sessions
            .into_iter()
            .map(|session| {
                let is_deleting = deleting.contains(session.id.raw());
                let activity = ActivityStamp::new(now_secs, session.effective_activity());
                (session, is_deleting, activity)
            })
            .collect::<Vec<_>>()
    });
    let identity_snapshot = identity_snapshot(displayed_rows.as_deref(), compact);
    // Capture admitted openings before changed DOM is published. The common
    // hook merges successful transitions until delivery, so a later unrelated
    // change cannot erase an earlier cause. Unavailable/recovery baselines
    // replace pending work with an empty capture rather than replaying it.
    use_rendered_movement_between(&identity_snapshot, |before, after| {
        capture_identity_movement(&popovers, before, after)
    });
    // Every session whose menu an outside click may have to dismiss: the one
    // whose menu is open, plus any row still holding a delete or replace
    // confirmation. Sorted so the relays render in a stable order. Each
    // carries the opening of its row's question that THIS render read, so
    // the relay's cancel closes that opening only (see the relays below).
    let outside_relay_sessions: Vec<(String, Option<ConfirmToken<RowQuestion>>)> = {
        let mut ids: Vec<String> = row_questions.open_lanes();
        if let Some(open) = menu_open() {
            ids.push(open);
        }
        ids.sort();
        ids.dedup();
        ids.into_iter()
            .map(|id| {
                let shown = row_questions.current(&id);
                (id, shown)
            })
            .collect()
    };

    // Both entry points open the ordinary New draft. Keep destination and
    // permission seeding here rather than manufacturing a clone prefill,
    // which would suppress remembered permissions inside the launcher.
    let new_hosts = host_options.clone();
    let new_open_host = open_host.clone();
    let new_open_destination = open_destination.clone();
    let open_new = use_callback(
        move |action: Option<crate::launch_composer::ComposerSearchResult>| {
            if ops.busy_now() {
                return false;
            }
            // An admitted launcher handoff exposes its ordinary return
            // controls too. A refused shortcut leaves presentation alone.
            host_menu_open.set(None);
            hosts_panel_open.set(false);
            opening_generation += 1;
            quick_launch_pending.set(false);
            let current_hosts = new_hosts.clone();
            let current_target =
                effective_create_host(&current_hosts, *chosen_host.peek(), new_open_host.as_ref());
            ordinary_new_cwd.set(new_open_destination.as_ref().and_then(|destination| {
                // Confirm the entire inherited destination;
                // a remote path cannot follow the host fallback
                // onto this machine after an installation change.
                matching_host_option(&destination.host, &current_hosts)
                    .filter(|host| Some(host.id) == current_target)
                    .map(|_| destination.cwd.clone())
            }));
            clone_prefill.set(None);
            auto_launch.set(false);
            initial_create_error.set(None);
            initial_create_action.set(action);
            initial_quick_templates.set(Vec::new());
            initial_partial_template.set(None);
            show_create.set(true);
            true
        },
    );
    let quick_base = base.clone();
    // One worker serializes fresh reads for every shortcut source. The page
    // still opens New once; a visible launcher receives captured batches in
    // press order without a second resolver or a second template apply path.
    let quick_launch = use_callback(
        move |(references, origin): (
            Vec<crate::launch_sequence::TemplateReference>,
            crate::launch_sequence::QuickLaunchOrigin,
        )| {
            let refused = (*quick_launch_pending.peek() && !*show_create.peek())
                || *templates_open.peek()
                || ops.busy_now();
            // Keyboard clicks always enqueue, even refused or empty ones, so
            // the worker drains the JS snapshot each one owns in press order.
            let blocked =
                match crate::launch_sequence::shortcut_admission(origin, &references, refused) {
                    crate::launch_sequence::ShortcutAdmission::Ignore => return,
                    crate::launch_sequence::ShortcutAdmission::Drain => true,
                    crate::launch_sequence::ShortcutAdmission::Dispatch => false,
                };
            let opening = if blocked || *show_create.peek() {
                *opening_generation.peek()
            } else {
                opening_generation += 1;
                *opening_generation.peek()
            };
            shortcut_dispatch_queue
                .write()
                .push_back((opening, references, origin, blocked));
            // Only an entry that can launch refuses the next press. A drained
            // one launches nothing, so it must not disable Begin and the
            // direct shortcuts while the worker takes its snapshot. Raise the
            // flag even when joining a running worker: something may have
            // cleared it since the worker started (its own open_new, or a
            // drain started the worker), and this launch is still in flight.
            // The worker's exit clears it.
            if !blocked {
                quick_launch_pending.set(true);
            }
            if *shortcut_dispatch_running.peek() {
                return;
            }
            shortcut_dispatch_running.set(true);
            let base = quick_base.clone();
            spawn(async move {
                loop {
                    // Drop the signal's write guard before any await: a second
                    // shortcut must be able to join the queue during this read.
                    let next = { shortcut_dispatch_queue.write().pop_front() };
                    let Some((opening, references, origin, blocked)) = next else {
                        break;
                    };
                    // The capture listener and dispatch ask the same DOM rule.
                    // Its epoch revokes a press when a different dialog opened
                    // during the awaited read, even if that dialog then closed.
                    let entry = if origin.keyboard() {
                        crate::launch_sequence::take_shortcut_press_snapshot().await
                    } else {
                        crate::launch_sequence::shortcut_context_snapshot().await
                    };
                    let Some(entry) = entry else {
                        continue;
                    };
                    // A relay can race a disabled render, and a sequence's
                    // Enter can arrive after the app refused its Begin. Drain
                    // its JS snapshot in order, but never apply a press refused
                    // at admission after the operation becomes idle again.
                    if blocked {
                        continue;
                    }
                    if !matches!(entry.context.as_str(), "page" | "launcher") {
                        continue;
                    }
                    let references =
                        if origin == crate::launch_sequence::QuickLaunchOrigin::Sequence {
                            let Some(captured) = entry.capture_sequence(&references) else {
                                continue;
                            };
                            captured
                        } else {
                            references
                        };
                    let result = async {
                        let fresh = crate::api::fetch_preferences(&base).await?;
                        let templates = crate::api::fetch_templates(&base).await?;
                        Ok::<_, String>((
                            crate::api::template_bindings_after_read(fresh),
                            templates,
                        ))
                    }
                    .await;
                    let context = crate::launch_sequence::shortcut_context_snapshot().await;
                    quick_read_completed.set(opening);
                    if *opening_generation.peek() != opening
                        || !context.is_some_and(|snapshot| {
                            snapshot.context == entry.context
                                && snapshot.modal_opening == entry.modal_opening
                        })
                        || *templates_open.peek()
                        || ops.busy_now()
                    {
                        continue;
                    }
                    match result {
                        Ok((fresh, templates)) => {
                            let resolved = crate::launch_sequence::resolve_template_references(
                                &references,
                                &fresh,
                                &templates,
                            );
                            preferences.0.write().quick_launch_template =
                                fresh.quick_launch_template;
                            preferences.0.write().template_shortcuts = fresh.template_shortcuts;
                            if entry.context == "launcher" {
                                if !*show_create.peek() {
                                    continue;
                                }
                                let refusal = if resolved.is_none() {
                                    Some(match references.as_slice() {
                                        [
                                            crate::launch_sequence::TemplateReference::Bolt
                                            | crate::launch_sequence::TemplateReference::Unbound(
                                                None,
                                            ),
                                        ] => {
                                            "no template is marked ⚡ for quick launch".to_string()
                                        }
                                        [
                                            crate::launch_sequence::TemplateReference::Number(key)
                                            | crate::launch_sequence::TemplateReference::Unbound(
                                                Some(key),
                                            ),
                                        ] => format!("no template is bound to key {key}"),
                                        _ => "a template in this sequence is unbound or missing"
                                            .to_string(),
                                    })
                                } else {
                                    None
                                };
                                shortcut_apply_requests.write().push_back(
                                    super::create_form::ShortcutApplyRequest {
                                        opening,
                                        context: entry.clone(),
                                        templates: resolved.unwrap_or_default().into(),
                                        refusal,
                                    },
                                );
                            } else if !*show_create.peek() {
                                if let Some(templates) = resolved {
                                    if open_new.call(None) {
                                        let launchable =
                                            farhelm_proto::launcher::stacked_quick_launch_eligible(
                                                templates.iter().map(|template| &template.fields),
                                            );
                                        auto_launch.set(launchable);
                                        if !launchable {
                                            initial_partial_template
                                                .set(Some(origin.incomplete_banner(&templates)));
                                        }
                                        initial_quick_templates.set(templates);
                                    }
                                } else if let Some(name) =
                                    references.iter().find_map(|reference| match reference {
                                        crate::launch_sequence::TemplateReference::Name(name)
                                            if !templates
                                                .iter()
                                                .any(|template| &template.name == name) =>
                                        {
                                            Some(name)
                                        }
                                        _ => None,
                                    })
                                {
                                    // Pickers and sequences retain names from an earlier
                                    // snapshot. A missing name belongs in New's refusal,
                                    // rather than an unrelated binding hint in Templates.
                                    if open_new.call(None) {
                                        initial_create_error.set(Some(format!(
                                            "template {name} is no longer available"
                                        )));
                                    }
                                } else {
                                    initial_template.set(None);
                                    quick_launch_hint.set(references.iter().any(|reference| {
                                        matches!(
                                            reference,
                                            crate::launch_sequence::TemplateReference::Bolt | crate::launch_sequence::TemplateReference::Unbound(None)
                                        )
                                    }));
                                    number_shortcut_hint.set(references.iter().any(|reference| {
                                        matches!(
                                            reference,
                                            crate::launch_sequence::TemplateReference::Number(_) | crate::launch_sequence::TemplateReference::Unbound(Some(_))
                                        )
                                    }));
                                    templates_open.set(true);
                                }
                            }
                        }
                        Err(message) => {
                            if entry.context == "launcher" && *show_create.peek() {
                                shortcut_apply_requests.write().push_back(
                                    super::create_form::ShortcutApplyRequest {
                                        opening,
                                        context: entry.clone(),
                                        templates: Default::default(),
                                        refusal: Some(format!(
                                            "couldn't load templates: {message}"
                                        )),
                                    },
                                );
                            } else if entry.context == "page"
                                && !*show_create.peek()
                                && open_new.call(None)
                            {
                                initial_create_error
                                    .set(Some(format!("couldn't prepare quick launch: {message}")));
                            }
                        }
                    }
                }
                shortcut_dispatch_running.set(false);
                quick_launch_pending.set(false);
            });
        },
    );
    let switcher_listing = request_listing.clone();
    let finish_switcher = use_callback(move |pick: Option<super::quick_switcher::SwitcherPick>| {
        if *quick_switcher_closing.peek() {
            return;
        }
        quick_switcher_closing.set(true);
        // The surviving task waits for removal, but its first poll can follow
        // that render. Release while this exact switcher is still mounted.
        super::quick_switcher::DIALOG.release();
        quick_switcher_open.set(false);
        let switcher_listing = switcher_listing.clone();
        // Spawn in this parent's scope: a task owned by the closing dialog
        // would be cancelled by the very unmount it must wait for.
        spawn(async move {
            super::quick_switcher::DIALOG.close_and_wait().await;
            let mut restore = true;
            match pick {
                Some(super::quick_switcher::SwitcherPick::Session(session)) => {
                    let same = selected.peek().as_deref() == Some(session.id.raw());
                    let hidden = {
                        let current_filter = filter.peek();
                        let current_listing = listing.peek();
                        let answers_current = listing_query
                            .peek()
                            .as_ref()
                            .is_some_and(|(answered, _)| answered == &*current_filter);
                        let current = current_listing
                            .as_ref()
                            .and_then(|result| result.as_ref().ok())
                            .filter(|_| answers_current);
                        switcher_pick_hidden(&current_filter, &session, current)
                    };
                    if guarded_open.call(*session) {
                        hosts_panel_open.set(false);
                        restore = same;
                        if hidden {
                            // Reveal a hidden fleet pick with one widening read.
                            // A refused navigation must retain both choices.
                            {
                                let mut current = filter.write();
                                current.host = None;
                                current.status.clear();
                            }
                            switcher_listing(Trigger::Explicit);
                        }
                    }
                }
                Some(super::quick_switcher::SwitcherPick::New(action)) => {
                    restore = !open_new.call(Some(action));
                }
                Some(super::quick_switcher::SwitcherPick::QuickLaunch) => {
                    quick_launch.call((
                        vec![crate::launch_sequence::TemplateReference::Bolt],
                        crate::launch_sequence::QuickLaunchOrigin::QuickSwitcher,
                    ));
                    restore = false;
                }
                Some(super::quick_switcher::SwitcherPick::TemplateLaunch(name)) => {
                    quick_launch.call((
                        vec![crate::launch_sequence::TemplateReference::Name(name)],
                        crate::launch_sequence::QuickLaunchOrigin::TemplateList,
                    ));
                    restore = false;
                }
                None => {}
            }
            // Cancellation, refusal and the current session owe the opener
            // focus. An accepted handoff owns its next focus instead, but
            // still consumes the capture before either trigger can reopen.
            super::quick_switcher::OPENER.restore(restore);
            quick_switcher_closing.set(false);
        });
    });
    let open_switcher = use_callback(move |mode: super::quick_switcher::SwitcherMode| {
        if *quick_switcher_closing.peek() {
            return;
        }
        opening_generation += 1;
        quick_launch_pending.set(false);
        quick_switcher_mode.set(mode);
        quick_switcher_open.set(true);
    });

    // A navigation can precede the keyed view's first publishing effect. An
    // old title must never let this shortcut rename the previous selection.
    let rename_target =
        use_memo(move || open_session_title().filter(|(id, _)| selected().as_ref() == Some(id)));
    let sequence_base = base.clone();
    rsx! {
        // Relays remain enabled while armed state catches up in the DOM.
        // Their live signal guards preserve prefix/digit/Enter event order
        // even when all three arrive before the first indicator render.
        button {
            class: "launch-sequence-trigger", "data-sequence-action": "begin",
            r#type: "button", hidden: true, tabindex: "-1",
            disabled: (quick_launch_pending() && !show_create()) || templates_open() || busy,
            onclick: move |_| {
                if (!*quick_launch_pending.peek() || *show_create.peek())
                    && !*templates_open.peek() && !ops.busy_now() {
                    launch_sequence_in_launcher.set(*show_create.peek());
                    launch_sequence.set(Some(Vec::new()));
                    sequence_generation += 1;
                    let generation = *sequence_generation.peek();
                    let base = sequence_base.clone();
                    spawn(async move {
                        // Refresh on the prefix, never at Enter: a binding that
                        // the pill showed as unbound must not acquire authority
                        // from dispatch's later read.
                        if let Ok(fresh) = crate::api::fetch_preferences(&base).await
                            && *sequence_generation.peek() == generation
                            && launch_sequence.peek().is_some()
                        {
                            let fresh = crate::api::template_bindings_after_read(fresh);
                            preferences.0.write().quick_launch_template = fresh.quick_launch_template;
                            preferences.0.write().template_shortcuts = fresh.template_shortcuts;
                        }
                    });
                }
            },
        }
        for key in crate::launch_sequence::TEMPLATE_SHORTCUTS.number.keys.iter().map(|key| key.value) {
            button {
                key: "sequence-{key}", class: "launch-sequence-trigger",
                "data-sequence-action": "digit", "data-number-key": "{key}",
                r#type: "button", hidden: true, tabindex: "-1",
                onclick: move |_| {
                    if let Some(queue) = launch_sequence.write().as_mut() {
                        queue.push(crate::launch_sequence::TemplateReference::Number(key.into()));
                    }
                },
            }
        }
        button {
            class: "launch-sequence-trigger", "data-sequence-action": "bolt",
            r#type: "button", hidden: true, tabindex: "-1",
            onclick: move |_| {
                if let Some(queue) = launch_sequence.write().as_mut() { queue.push(crate::launch_sequence::TemplateReference::Bolt); }
            },
        }
        button {
            class: "launch-sequence-trigger", "data-sequence-action": "launch",
            r#type: "button", hidden: true, tabindex: "-1",
            onclick: move |_| {
                // JS has already copied the indicator's published bindings at
                // physical Enter, and clicks this relay only with that copy
                // queued. The worker pairs them with this raw queue before its
                // fresh reads can observe another binding. A queue the app
                // refused at Begin still goes through, empty, so the worker
                // discards this Enter's copy instead of handing it to the next
                // press (`shortcut_admission`).
                let queued = launch_sequence.write().take();
                quick_launch.call((queued.unwrap_or_default(), crate::launch_sequence::QuickLaunchOrigin::Sequence));
            },
        }
        button {
            class: "launch-sequence-trigger", "data-sequence-action": "cancel",
            r#type: "button", hidden: true, tabindex: "-1",
            onclick: move |_| launch_sequence.set(None),
        }
        // The current session, rather than a visible row, owns this target.
        // The common opener still enforces admission at actual click time.
        button {
            class: "session-rename-trigger", r#type: "button", hidden: true, tabindex: "-1",
            disabled: busy || rename_target.read().is_none() || rename_editor.read().is_some(),
            onclick: move |_| {
                if let Some((id, title)) = rename_target.peek().as_ref() {
                    on_rename_start.call((id.clone(), title.clone(), true, false));
                }
            },
        }
        button {
            class: "quick-launch-trigger", r#type: "button", hidden: true, tabindex: "-1",
            "data-opening-generation": "{opening_generation()}",
            "data-read-completed-generation": "{quick_read_completed()}",
            disabled: (quick_launch_pending() && !show_create()) || templates_open() || busy,
            onclick: move |_| quick_launch.call((vec![crate::launch_sequence::TemplateReference::Bolt], crate::launch_sequence::QuickLaunchOrigin::Bolt)),
        }
        for key in crate::launch_sequence::TEMPLATE_SHORTCUTS.number.keys.iter().map(|key| key.value) {
            button {
                key: "number-{key}", class: "template-number-trigger", r#type: "button",
                hidden: true, tabindex: "-1", "data-number-key": "{key}",
                disabled: (quick_launch_pending() && !show_create()) || templates_open() || busy,
                onclick: move |_| quick_launch.call((vec![crate::launch_sequence::TemplateReference::Number(key.into())], crate::launch_sequence::QuickLaunchOrigin::Number)),
            }
        }
        // Status sounds follow the very replies these rows display. Keeping
        // the observer here avoids another session read or a mirrored listing.
        crate::sounds::SessionSounds { selected, listing, listing_scope }
        // Both keyboard relays enter one opener; only the mode and the
        // resulting rows differ. Closing still waits for dialog DOM removal.
        button {
            class: "quick-switcher-trigger", r#type: "button", hidden: true,
            tabindex: "-1", disabled: quick_switcher_closing(),
            onclick: move |_| open_switcher.call(super::quick_switcher::SwitcherMode::Sessions),
        }
        button {
            class: "quick-switcher-template-trigger", r#type: "button", hidden: true,
            tabindex: "-1", disabled: quick_switcher_closing(),
            onclick: move |_| open_switcher.call(super::quick_switcher::SwitcherMode::Templates),
        }
        if quick_switcher_open() {
            super::quick_switcher::QuickSwitcher {
                mode: quick_switcher_mode(),
                hosts,
                on_close: move |_| finish_switcher.call(None),
                on_pick: move |pick| finish_switcher.call(Some(pick)),
            }
        }
        // Targets of `menu_panel::install_row_menu_outside_dismiss`: a pointer
        // going down outside the open row menu clicks the relay carrying that
        // menu's identity, and the menu closes the way Escape closes it.
        //
        // One relay per identity rather than one shared button: the click
        // arrives a task after the pointer-down, and by then the same
        // gesture's click on another row's toggle may already have opened a
        // different menu. Each relay is keyed by its identity, so an event
        // for a menu that has since gone reaches no handler, and each handler
        // closes its own menu only if it is still the open one.
        //
        // A session menu showing its delete or replace confirmation is the one
        // place this does more than Escape: Escape is deliberately unbound in
        // those prompts, because closing the panel leaves the row's question
        // open and the row would reopen straight into the same prompt. A
        // click elsewhere is taken as the prompt's safe answer, cancel, so the
        // relay closes that question too. Rows with an open question keep a
        // relay even when another menu replaced their panel, so that answer is
        // not lost to the race above. Cancelling holds no claim to release;
        // it only closes the question, exactly as the prompt's cancel does,
        // and like that cancel it carries the opening this render drew. So a
        // relay click drawn before the row's current opening closes only the
        // menu and leaves the question open. That covers a click left over
        // from before a reopening, and also one drawn when the row had no
        // question open at all, whose click lands after the question first
        // opened: the menu closes and the question stays open behind it.
        // Both follow the plan's decision that a stale answer does nothing
        // and the current prompt stays; the row's menu reopens into it.
        for (id, shown_question) in outside_relay_sessions {
            button {
                key: "session:{id}",
                r#type: "button",
                class: crate::menu_panel::ROW_MENU_OUTSIDE_RELAY,
                "data-row-menu": crate::menu_panel::row_menu_relay_key("session", &id),
                hidden: true,
                tabindex: "-1",
                onclick: move |_| {
                    if let Some(token) = &shown_question
                        && row_questions.take(&id, token).is_some()
                        && *token.key() == RowQuestion::Replace
                    {
                        replace_controller.cancel_source(&id);
                    }
                    if menu_open.peek().as_deref() == Some(id.as_str()) {
                        menu_open.set(None);
                    }
                },
            }
        }
        // The open notification list's relay, keyed `bell:<id>` so a
        // dismissal aimed at one list never closes another that a later
        // click opened.
        if let Some(OpenBell { id: bell_id, .. }) = bell_open() {
            button {
                key: "bell:{bell_id}",
                r#type: "button",
                class: crate::menu_panel::ROW_MENU_OUTSIDE_RELAY,
                "data-row-menu": crate::menu_panel::row_menu_relay_key("bell", &bell_id),
                hidden: true,
                tabindex: "-1",
                onclick: move |_| close_bell.call(bell_id.clone()),
            }
        }
        if let Some(host_id) = host_menu_open() {
            button {
                key: "host:{host_id}",
                r#type: "button",
                class: crate::menu_panel::ROW_MENU_OUTSIDE_RELAY,
                "data-row-menu": crate::menu_panel::row_menu_relay_key("host", &host_id.to_string()),
                hidden: true,
                tabindex: "-1",
                onclick: move |_| {
                    if *host_menu_open.peek() == Some(host_id) {
                        host_menu_open.set(None);
                    }
                },
            }
        }
        AppBar { layout_epoch,
        hosts_toggle: rsx! {
            button {
                r#type: "button", class: "btn btn-neutral hosts-panel-toggle",
                aria_label: "Hosts", "data-tooltip": "Hosts: show host management",
                aria_pressed: hosts_panel_open(), aria_expanded: hosts_panel_open(),
                aria_controls: "host-management-panel",
                onclick: {
                    let popovers = popovers.clone();
                    move |_| {
                        popovers.close_open_in(&[AnchorRegion::Hosts, AnchorRegion::Sessions]);
                        host_menu_open.set(None);
                        menu_open.set(None);
                        hosts_panel_open.toggle();
                        FocusTarget::new(".hosts-panel-toggle", None, false).focus();
                    }
                },
                crate::icons::ActionIcon { glyph: crate::icons::ActionGlyph::Hosts }
            }
        },
        // The rail and footer remain reachable while this content scrolls.
        // Owners below stay mounted; filtering never deselects a session.
        div { class: "sidebar-content",
        onscroll: move |_| hosts_panel_layout_epoch += 1,
        // The host list is one permanent surface. Keeping the component
        // mounted is a lifecycle requirement, not only a layout choice:
        // discovery, planning, and mutation replies must retain their owner.
        HostsPanel {
            hosts,
            panel_open: hosts_panel_open,
            selected_host: filter.read().host,
            on_host_choice: move |host| {
                host_menu_open.set(None);
                menu_open.set(None);
                hosts_panel_open.set(false);
                if filter.peek().host != host {
                    filter.write().host = host;
                    request_host_filter(Trigger::Explicit);
                }
            },
            host_registration,
            details_open: host_details_open,
            ops,
            mutation_busy_hosts,
            provisioning_busy_hosts,
            provisioning_trace_shapes,
            provisioning_auto_details,
            host_menu_open,
            session_menu_open: menu_open,
            on_resize: move |_| hosts_panel_layout_epoch += 1,
            on_changed: refresh_hosts,
            on_appearance_committed: move |(id, icon, color)| {
                // This successful write is newer than every GET already in
                // flight. Publish its known pair before controls re-enable;
                // the normal refresh still supplies all connection facts.
                let generation = hosts_reads.write().start();
                hosts_reads.write().accept_success(generation);
                hosts.write().record_appearance(id, icon, color);
            },
        }
        // The ordering is the list's title; count and icon actions share its
        // band. At minimum width the action group wraps as a whole.
        div { class: "list-header",
            div { class: "session-heading", hidden: hosts_panel_open(),
                super::sort_menu::SortMenu {
                    sort: sort(), compact, layout_epoch,
                    on_sort: apply_sort,
                    on_compact: move |next| {
                        popovers.close_registered(AnchorRegion::Sessions);
                        remember_compact(&base, preferences, next);
                    },
                }
                if let Some(Ok(listing)) = &*listing.read() {
                    {
                        let banner = count_banner(listing);
                        rsx! {
                            div { class: "{banner.class}",
                                "data-warning": banner.warning,
                                aria_label: format!("Sessions: {}", banner.text),
                                "data-tooltip": format!("Sessions: {}", banner.text),
                                "{banner.text}"
                            }
                        }
                    }
                }
                div { class: "session-heading-actions",
                    super::trash::TrashControl {
                        hosts, revision: trash_revision,
                        gate: crate::ops::PaneGate::new(ops, row_ops),
                    }
                    // Beside New, the action it feeds (SPEC.md: "The Templates
                    // control sits beside New in the session list header").
                    button {
                        r#type: "button",
                        class: "btn btn-neutral templates-button",
                        aria_label: "Templates",
                        "data-tooltip": preferences.0.read().quick_launch_template.as_ref().map_or_else(|| format!("templates: mark one with ⚡ for quick launch ({})", crate::launch_sequence::TEMPLATE_SHORTCUTS.bolt.label(crate::launch_sequence::TEMPLATE_SHORTCUTS.mac_desktop)), |name| format!("templates: quick launch {} ({})", name.escaped(), crate::launch_sequence::TEMPLATE_SHORTCUTS.bolt.label(crate::launch_sequence::TEMPLATE_SHORTCUTS.mac_desktop))),
                        aria_haspopup: "dialog",
                        disabled: busy,
                        onclick: move |_| { opening_generation += 1; quick_launch_pending.set(false); auto_launch.set(false); initial_template.set(None); quick_launch_hint.set(false); number_shortcut_hint.set(false); templates_open.set(true); },
                        crate::icons::ActionIcon { glyph: crate::icons::ActionGlyph::Templates }
                    }
                    button {
                    r#type: "button",
                    // Focus restoration and the Mac Cmd+N shortcut select this
                    // live control by class, so both retain its ordinary guards.
                    class: "btn btn-primary new-session-button",
                    "data-tooltip": NEW_SESSION_TOOLTIP,
                    aria_label: "New session",
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
                        if opening {
                            open_new.call(None);
                            return;
                        }
                        // A queued toggle-close still owns the visible form's
                        // isolation even though New is inert while it is open.
                        COMPOSER_DIALOG.release();
                        {
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
                            initial_create_action.set(None);
                            ordinary_new_cwd.set(None);
                        }
                        show_create.set(false);
                    },
                        crate::icons::ActionIcon { glyph: crate::icons::ActionGlyph::Add }
                    }

                }
            }
            if templates_open() {
                super::templates::TemplatesDialog {
                    hosts: host_options.clone(),
                    layout_epoch,
                    initial_template: initial_template(),
                    quick_launch_hint: quick_launch_hint(),
                    number_shortcut_hint: number_shortcut_hint(),
                    on_close: move |_| {
                        templates_dialog.close(super::templates::RETURN_FOCUS);
                        templates_open.set(false);
                        initial_template.set(None);
                        templates_revision += 1;
                    },
                }
            }
            // The form follows New in DOM order, so New stays above the draft
            // with its existing cancellation action.
            if show_create() {
                CreateSessionForm {
                    hosts: host_options.clone(),
                    open_host: open_host.clone(),
                    open_cwd: open_cwd.clone(),
                    hosts_loaded: hosts.read().hosts().is_some(),
                    host_read_error: hosts.read().refresh_error().map(str::to_owned),
                    chosen_host,
                    create_target,
                    ops,
                    initial_cwd: ordinary_new_cwd(),
                    auto_launch,
                    opening_generation,
                    initial_error: initial_create_error(),
                    prefill: clone_prefill(),
                    restart_bridge: restart_bridge(),
                    on_restarted: move |_| {
                        // The view already refreshed and reattached. Closing
                        // must not select a new session or rewrite defaults.
                        COMPOSER_DIALOG.release();
                        opening_generation += 1;
                        chosen_host.set(None);
                        clone_prefill.set(None);
                        restart_bridge.set(None);
                        initial_create_action.set(None);
                        ordinary_new_cwd.set(None);
                        show_create.set(false);
                    },
                    // Clone/Replace owns its seed even if it supersedes a
                    // switcher opening before the launcher unmounts.
                    initial_action: if clone_prefill().is_none() { initial_create_action() } else { None },
                    initial_templates: if clone_prefill().is_none() { initial_quick_templates() } else { Vec::new() },
                    shortcut_requests: shortcut_apply_requests,
                    partial_template_banner: if clone_prefill().is_none() { initial_partial_template() } else { None },
                    // The "replace with" source as the sidebar currently
                    // lists it, so the launcher's warning follows a source
                    // that starts or stops while it is open. While the
                    // listing lacks the row (a failed read, a filter that
                    // hides it), the last state a listing showed stands in,
                    // and before any listing has shown it, the snapshot
                    // taken when the launcher opened.
                    replace_source_state: clone_prefill.read().as_ref().and_then(|prefill| {
                        let source = prefill.mode.replace_source()?;
                        listed_state(&listing.read(), source)
                            .or_else(|| {
                                replace_source_seen
                                    .read()
                                    .as_ref()
                                    .filter(|(id, _)| id == source)
                                    .map(|(_, state)| state.clone())
                            })
                            .or_else(|| prefill.mode.source_opened().cloned())
                    }),
                    on_cancel: move |_| {
                        let was_restart = clone_prefill.peek().as_ref().is_some_and(|prefill| prefill.mode.is_restart());
                        COMPOSER_DIALOG.release();
                        opening_generation += 1;
                        auto_launch.set(false);
                        // A cancelled composer must not carry its destination
                        // or clone provenance into the next fresh launch.
                        chosen_host.set(None);
                        clone_prefill.set(None);
                        initial_create_action.set(None);
                        ordinary_new_cwd.set(None);
                        show_create.set(false);
                        // Release above must precede this immediate handback;
                        // the removed field would otherwise leave focus on body.
                        restart_bridge.set(None);
                        FocusTarget::new(if was_restart { ".restart-with-trigger" } else { ".new-session-button" }, None, false).focus();
                    },
                    on_template_saved: move |template| {
                        // Templates owns the next focus; release the launcher
                        // before removing it without queuing a New-button return.
                        COMPOSER_DIALOG.release();
                        chosen_host.set(None);
                        clone_prefill.set(None);
                        initial_create_action.set(None);
                        ordinary_new_cwd.set(None);
                        show_create.set(false);
                        initial_template.set(Some(template));
                        templates_open.set(true);
                    },
                    on_created: move |created: CreatedSession| {
                        hosts_panel_open.set(false);
                        let CreatedSession {
                            session,
                            submitted_launch,
                            new_was_never_shown,
                        } = created;
                        // Reads begun before creation cannot prove this new
                        // session absent. Invalidate them before its selection
                        // can trigger reconciliation against a late old reply.
                        listing_reads.write().fence();
                        // Creation is a user-initiated selection too.
                        remember_selection(&created_base, preferences, session.id.raw());
                        // Mirror the structured launch THIS client submitted
                        // (never the reply's `session.launch`, which the
                        // remote host writes: SPEC.md allows only explicit
                        // GUI selections to shape GUI defaults) into its
                        // held copy of the preferences, so the next
                        // "New" open in this client is right without
                        // waiting on a refetch — the durable copy is
                        // already written by the helm itself as part of
                        // this same create, so there is deliberately no
                        // PUT here, only a local mirror (compare
                        // `remember_selection`/`remember_compact`, which DO
                        // write through). A command launch carries no
                        // `launch` at all and must leave these fields alone,
                        // matching the helm's own write condition. Not
                        // exactly, though: the helm also declines to write
                        // when the create's history admission is refused (a
                        // host with no install identity, an out-of-order
                        // replay), cases this client cannot see. Until the
                        // next reload this client may then preselect a value
                        // the helm never stored — the rarer, reversible
                        // direction, and accepted rather than plumbed back.
                        mirror_submitted_launch(
                            &mut preferences.0.write(),
                            submitted_launch.as_ref(),
                        );
                        show_create.set(false);
                        // This component stays mounted after creation, so
                        // clear the draft's host and clone seed just as the
                        // explicit cancellation path does.
                        chosen_host.set(None);
                        clone_prefill.set(None);
                        initial_create_action.set(None);
                        ordinary_new_cwd.set(None);
                        let naming = name_created_session(
                            new_was_never_shown,
                            preferences.0.peek().name_session_on_quick_launch,
                        ).then(|| (session.id.raw().to_owned(), session.title.raw().to_owned()));
                        on_open.call(session);
                        if let Some((id, title)) = naming {
                            // Selection and the modal mount share this completion
                            // event. The accepted create has already released its
                            // token, and terminal reveal must see the modal before
                            // it can take focus or keystrokes.
                            on_rename_start.call((id, title, false, true));
                        } else {
                            FocusTarget::new(".new-session-button", None, false).focus();
                        }
                        // The fence needs a fresh authoritative successor even
                        // when a healthy feed has no further notice to deliver.
                        // Replace-with also needs this read to retire its source.
                        created_listing(Trigger::Explicit);
                    },
                }
            }
        // Status remains available while a listing is loading or failed;
        // the rail independently keeps the current host choice reachable.
            div { class: "session-status-filter", hidden: hosts_panel_open(),
                div { class: "segmented-control", role: "radiogroup", aria_label: "Session status",
                    for (value, label, help) in [
                        ("", "All", "Show every session, including ended sessions"),
                        ("running", "Running", "Show sessions whose agents are working"),
                        ("waiting", "Waiting", "Show sessions that need your attention"),
                        ("idle", "Idle", "Show live sessions whose agents are at rest"),
                    ] {
                        label { class: "segment-radio", "data-tooltip": help,
                            input {
                                r#type: "radio", name: "session-status", value,
                                checked: filter.read().status == value,
                                onchange: move |_| apply_status.call(value),
                            }
                            span { "{label}" }
                        }
                    }
                }
            }
        }
        if let Some(notice) = delete_notice.0() {
            div { class: "delete-notice", hidden: hosts_panel_open(),
                crate::peer::PeerLine {
                    class: "status".to_string(),
                    parts: vec![crate::peer::DetailPart::peer(notice)],
                }
                button {
                    class: "btn",
                    r#type: "button",
                    "data-tooltip": "dismiss: hide this notice",
                    onclick: move |_| {
                        delete_notice.dismiss();
                    },
                    "dismiss"
                }
            }
        }
        match &*listing.read() {
            None => rsx! { div { class: "status", hidden: hosts_panel_open(), "loading sessions…" } },
            // The helm's refusal text, rendered as the peer text it may
            // quote (see `api::refusal_text`).
            Some(Err(e)) => rsx! {
                div { hidden: hosts_panel_open(),
                crate::peer::PeerLine {
                    class: "status error".to_string(),
                    parts: vec![
                        crate::peer::DetailPart::text("failed to load sessions: "),
                        crate::peer::DetailPart::peer(e.clone()),
                    ],
                }
                }
            },
            Some(Ok(listing)) => rsx! {
                // The plain empty-fleet line, which is deliberately NOT the
                // same thing as a filter matching nothing — see
                // `rows::is_empty_fleet` for why the request has to be
                // consulted and what taking this branch would suppress.
                if rows::is_empty_fleet(listing) {
                    div { class: "status", hidden: hosts_panel_open(), "no sessions" }
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
                        div { class: "status filter-empty", hidden: hosts_panel_open(), "{line}" }
                    }
                    if let Some(question) = replace_controller.question() {
                        // Keyed by the source row, so another row's question
                        // mounts fresh (scrolled into view, cancel focused)
                        // instead of reusing this one's element.
                        crate::yolo_confirm::YoloConfirmation {
                            key: "{question.key().source.id.raw()}",
                            ask: question.key().ask.clone(),
                            // This flow holds a row operation rather than
                            // the page's token (`begin_row_op`), so its own
                            // in-flight replace shows as the source row's
                            // pending phase, not as `ops.busy()`.
                            busy: ops.busy()
                                || row_pending.read().contains(question.key().source.id.raw()),
                            error: replace_controller.error(),
                            confirm_submits: false,
                            // Both answers continue the Replace the user
                            // confirmed, with the row snapshot and the
                            // nothing-alive answer that prompt carried, and
                            // only by taking this opening of the question
                            // (`ReplaceController::answer_sidebar`), once row
                            // admission succeeds. Taking it closes it, so the
                            // permanent answer leaves it down while the host
                            // is marked; the controller reopens it with the
                            // reason if that fails and no newer question exists.
                            on_confirm: {
                                let mut do_replace = yolo_do_replace.clone();
                                let question = question.clone();
                                move |_| {
                                    let id = question.key().source.id.raw().to_owned();
                                    if let Some(answer) = replace_controller.answer_sidebar(
                                        &question,
                                        false,
                                        || begin_row_op(&id),
                                        || end_row_op(&id),
                                    ) {
                                        let pending = answer.pending.clone();
                                        do_replace(pending.source, pending.guard, Some(answer));
                                    }
                                }
                            },
                            on_confirm_and_stop_asking: {
                                let mut do_replace = yolo_do_replace.clone();
                                let question = question.clone();
                                move |_| {
                                    let id = question.key().source.id.raw().to_owned();
                                    if let Some(answer) = replace_controller.answer_sidebar(
                                        &question,
                                        true,
                                        || begin_row_op(&id),
                                        || end_row_op(&id),
                                    ) {
                                        let pending = answer.pending.clone();
                                        do_replace(pending.source, pending.guard, Some(answer));
                                    }
                                }
                            },
                            on_cancel: {
                                let question = question.clone();
                                move |_| {
                                    replace_controller.cancel_question(&question);
                                }
                            },
                        }
                    }
                    div { class: "session-list", hidden: hosts_panel_open(),
                        onpointerenter: {
                            let mut note = note_list_pointer.clone();
                            move |_| note()
                        },
                        onpointermove: {
                            let mut note = note_list_pointer.clone();
                            move |_| note()
                        },
                        onpointerdown: {
                            let mut note = note_list_pointer.clone();
                            move |_| note()
                        },
                        onwheel: {
                            let mut note = note_list_pointer.clone();
                            move |_| note()
                        },
                        onpointerleave: {
                            let mut leave = leave_list.clone();
                            move |_| leave()
                        },
                        // The rows are the server's listing with this
                        // view's own just-landed renames painted over it,
                        // so a renamed session reads correctly EVERYWHERE
                        // the row shows its title — the row itself, the
                        // delete prompt that quotes it, the rename editor
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
                        // While the pointer holds the order, rows keep the
                        // positions they had when it arrived (see
                        // `rows::held_display_order` for new and removed rows).
                        for (session, row_deleting, row_activity) in displayed_rows.clone().unwrap_or_default() {
                            SessionRow {
                                key: "{session.id.raw()}",
                                menu_registration: menu_registration.clone(),
                                bell_registration: bell_registration.clone(),
                                listing_ready,
                                compact,
                                pin_separator: pin_separator.as_deref() == Some(session.id.raw()),
                                state: RowState {
                                    error: errors.read().get(session.id.raw()).cloned(),
                                    // The shared token counts as busy for
                                    // every row's action buttons, not just
                                    // this row's own in-flight op: while
                                    // the other pane (or a page operation)
                                    // holds it, `begin_row_op` refuses row
                                    // ops anyway, and the disabled state is
                                    // that refusal made visible.
                                    busy: busy || row_pending.read().contains(session.id.raw()),
                                    // The opening this render draws; the
                                    // row's answers carry it back.
                                    question: row_questions.current(session.id.raw()),
                                    deleting: row_deleting,
                                    renaming: rename_editor
                                        .read()
                                        .as_ref()
                                        .is_some_and(|editor| editor.id == session.id.raw()),
                                    nav_disabled: nav_locked,
                                    menu_open: menu_open.read().as_deref()
                                        == Some(session.id.raw()),
                                    // A prefill WITHOUT its dialog (or a
                                    // blank New) is not a transfer: only
                                    // the two together mean this row's
                                    // menu close handed focus to a
                                    // composer that now owns it. See
                                    // `RowState::composer_transfer_open`.
                                    composer_transfer_open: *show_create.read()
                                        && clone_prefill.read().is_some(),
                                    selected: selected.read().as_deref()
                                        == Some(session.id.raw()),
                                    locality: session_locality(session.host, local_host),
                                    host_icon: hosts.read().hosts().unwrap_or_default().iter()
                                        .find(|host| Some(host.id) == session.host).map(|host| host.icon).unwrap_or_default(),
                                    host_color: hosts.read().hosts().unwrap_or_default().iter()
                                        .find(|host| Some(host.id) == session.host).map(|host| host.color).unwrap_or_default(),
                                    unavailable_cause: crate::hosts::session_unavailable_cause(hosts.read().lookup(session.host)),
                                    // Formatted HERE, not in the row: see
                                    // `RowState::activity` for why the
                                    // clock's tick must not reach a row
                                    // whose displayed age has not moved.
                                    activity: row_activity,
                                    bell_open: bell_open
                                        .read()
                                        .as_ref()
                                        .filter(|open| open.id == session.id.raw())
                                        .map(|open| open.read_through),
                                },
                                on_open: row_open,
                                on_clone,
                                on_replace_with,
                                on_mark_seen,
                                on_pin,
                                on_replace,
                                on_confirm_replace: confirm_replace,
                                on_cancel_replace: cancel_replace,
                                on_stop,
                                on_delete,
                                on_confirm_delete: confirm_delete,
                                on_cancel_delete: cancel_delete,
                                on_rename_start: on_rename_row,
                                on_menu_toggle: toggle_menu,
                                on_bell_toggle: toggle_bell,
                                on_bell_close: close_bell,
                                on_bell_clear: clear_bell,
                                session,
                            }
                        }
                    }
                }
            },
        }
        if let Some(editor) = rename_editor() {
            RenameDialog {
                draft: rename_draft,
                busy: row_pending.read().contains(&editor.id),
                busy_now: {
                    let id = editor.id.clone();
                    Callback::new(move |_| row_pending.read().contains(&id))
                },
                unavailable: editor.unavailable,
                current_title: editor.current_title,
                error: rename_error()
                    .and_then(|(error_generation, error)| {
                        (error_generation == editor.generation).then_some(error)
                    }),
                generation: editor.generation,
                shortcut: editor.shortcut,
                naming: editor.naming,
                focus_owned: rename_focus_owned,
                on_submit: on_rename_submit,
                on_cancel: on_rename_cancel,
            }
        }
        }
        }
    }
}

/// Mirror a successful create's SUBMITTED structured launch into this
/// client's held preferences, the way the helm records it durably.
///
/// Takes only the submitted selection, never the created `Session`: the
/// session's `launch` is the supervisor's reply, which a remote host writes,
/// and SPEC.md allows only explicit GUI selections to shape GUI defaults.
/// `None` (a command create) leaves the preferences alone, as the
/// helm does. Workspace trust comes from Settings and never affects GUI defaults.
/// Wrapper words enter the typed mirror unchanged; the next launcher owns their
/// shell-line spelling and any inherited-value presentation guard.
fn mirror_submitted_launch(
    preferences: &mut crate::api::Preferences,
    submitted: Option<&farhelm_proto::LaunchSelection>,
) {
    let Some(launch) = submitted else {
        return;
    };
    // The helm normalizes omitted permissions before returning the session.
    // A harness whose omitted mode is already YOLO must not seed that value
    // into the next dialog as an explicit choice; its default supplies the
    // same behavior without spilling into harnesses with a real default.
    preferences.remembered_permissions = launch
        .permissions
        .filter(|permission| Some(*permission) != launch.harness.omitted_permission())
        .filter(|permission| *permission != crate::LaunchPermission::Custom)
        .map(|permission| permission.wire_word().to_string());
    if let Some(prefix) = &launch.wrapper_prefix {
        preferences
            .remembered_wrapper_prefixes
            .insert(launch.harness, prefix.iter().map(PeerText::from).collect());
    } else {
        preferences
            .remembered_wrapper_prefixes
            .remove(&launch.harness);
    }
}

/// The one open notification list (SPEC.md, Status), as `ListView` tracks it.
#[derive(Debug, Clone, PartialEq)]
struct OpenBell {
    /// The session whose bell is open.
    id: String,
    /// The read mark as it stood when the list opened: entries above it stay
    /// marked new for as long as the list is open, even if another window
    /// moves the shared mark meanwhile.
    read_through: u64,
    /// The newest entry the list has displayed: what closing it marks read.
    /// Set at the click and raised by every listing the list stays open
    /// through.
    shown: u64,
}

/// The manual read/unread toggle's report: the settled save's outcome on
/// the row's error line, and a list-owned refresh request on success.
///
/// The report runs from a writer task that outlives this list (see
/// `api::SeenWriteReport`), and the list may be gone by the time the helm
/// answers, for example behind the browser's token prompt. Every access to
/// list state therefore goes through `try_write`: the panicking accessors
/// (`write`, `set`, `read` and the rest) would take the window down with
/// them on a dropped signal, and with no row left to show the result on,
/// dropping it is all there is to do. The counter lets the list's effect
/// start its own refresh; calling its reader here would both access dropped
/// state and give a refresh task the writer's root scope instead.
fn seen_toggle_report(
    mut errors: Signal<HashMap<String, String>>,
    mut successes: Signal<u64>,
    id: String,
) -> impl FnOnce(Result<(), String>) {
    move |result| {
        let Ok(mut errors) = errors.try_write() else {
            return;
        };
        match result {
            Ok(()) => {
                errors.remove(&id);
                drop(errors);
                if let Ok(mut successes) = successes.try_write() {
                    *successes += 1;
                }
            }
            Err(error) => {
                errors.insert(id, format!("seen: {error}"));
            }
        }
    }
}

/// One session's (status, terminal tab count) as a successfully read listing
/// shows it, or `None` when the listing failed, has not loaded, or lacks the
/// row. Feeds the "replace with" launcher's view of its source.
fn listed_state(
    listing: &Option<Result<SessionListing, String>>,
    id: &str,
) -> Option<(SessionStatus, usize)> {
    match listing {
        Some(Ok(listing)) => listing
            .sessions
            .iter()
            .find(|session| session.id.raw() == id)
            .map(|session| (session.status.clone(), session.tabs.len())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A switcher widens only when host or status hides the accepted pick.
    /// Ended and unclassified sessions cannot match a live-only segment.
    #[farhelm_testtrace::test]
    fn switcher_picks_respect_both_sidebar_dimensions() {
        let mut session = crate::list::row::row_specimen("pick");
        session.host = Some(7);
        for status in [
            SessionStatus::Running,
            SessionStatus::Waiting,
            SessionStatus::Idle,
            SessionStatus::Unknown,
            SessionStatus::Exited { exit_code: Some(0) },
            SessionStatus::Interrupted,
            SessionStatus::Error {
                detail: "refused".into(),
            },
        ] {
            session.status = status;
            assert!(!switcher_pick_hidden(
                &SessionFilter::default(),
                &session,
                None
            ));
            for (key, matches) in [
                ("running", matches!(session.status, SessionStatus::Running)),
                ("waiting", matches!(session.status, SessionStatus::Waiting)),
                ("idle", matches!(session.status, SessionStatus::Idle)),
            ] {
                let mut filter = SessionFilter {
                    host: Some(7),
                    status: key.into(),
                    ..Default::default()
                };
                assert_eq!(switcher_pick_hidden(&filter, &session, None), !matches);
                filter.host = Some(8);
                assert!(switcher_pick_hidden(&filter, &session, None));
            }
        }
    }

    /// An admitted current membership beats stale switcher statuses in both directions.
    /// Missing rows widen navigation only; they still carry no deletion evidence.
    #[farhelm_testtrace::test]
    fn switcher_picks_prefer_current_complete_membership() {
        let mut session = crate::list::row::row_specimen("pick");
        session.status = SessionStatus::Idle;
        let filter = SessionFilter {
            status: "idle".into(),
            ..Default::default()
        };
        let mut current = SessionListing {
            sessions: vec![],
            total: 1,
            matching: Some(0),
            filtered: true,
            omits_fleet_members: true,
            truncated: false,
        };
        assert!(switcher_pick_hidden(&filter, &session, Some(&current)));
        session.status = SessionStatus::Running;
        current.sessions.push(Session {
            status: SessionStatus::Idle,
            ..session.clone()
        });
        assert!(!switcher_pick_hidden(&filter, &session, Some(&current)));
        current.truncated = true;
        assert!(
            !switcher_pick_hidden(&filter, &session, Some(&current)),
            "presence still proves visibility in a cut reply"
        );
        current.sessions.clear();
        session.status = SessionStatus::Idle;
        assert!(
            !switcher_pick_hidden(&filter, &session, Some(&current)),
            "a cut omission falls back to the snapshot"
        );
        session.status = SessionStatus::Running;
        assert!(switcher_pick_hidden(&filter, &session, None));
        assert!(!switcher_pick_hidden(
            &SessionFilter::default(),
            &session,
            Some(&current)
        ));
    }

    /// Keep a real admitted B opening while independently publishing held row
    /// snapshots. Cells deliberately leave effect polling under test control;
    /// no geometry or replacement owner policy is fabricated by this fixture.
    struct IdentityRenderFixture {
        owner: MovementOwner,
        registration: crate::anchored_popovers::Registration,
        lease: RefCell<Option<crate::anchored_popovers::AnchorLease>>,
        open: Rc<RefCell<Option<String>>>,
        deliveries: Rc<Cell<usize>>,
        ready: Rc<Cell<bool>>,
        bindings: Cell<usize>,
        rows: RefCell<Option<Vec<RenderedSession>>>,
        scope: Cell<Option<dioxus::core::ScopeId>>,
    }

    impl PartialEq for IdentityRenderFixture {
        /// Mutable render inputs do not change the fixture's prop identity.
        fn eq(&self, other: &Self) -> bool {
            std::ptr::eq(self, other)
        }
    }

    impl IdentityRenderFixture {
        /// Establish held A/B/C despite reverse served order, then bind the
        /// existing root B slot before mount so an accidental initial close is
        /// observable. A and C can change copy flow without changing position.
        fn new() -> Rc<Self> {
            let owner = MovementOwner::default();
            let open = Rc::new(RefCell::new(Some("B".to_string())));
            let deliveries = Rc::new(Cell::new(0));
            let current = open.clone();
            let closing = open.clone();
            let delivered = deliveries.clone();
            let registration = owner.register(
                Interest::new(AnchorRegion::Sessions),
                move || current.borrow().clone(),
                move || {
                    delivered.set(delivered.get() + 1);
                    *closing.borrow_mut() = None;
                },
            );
            let ready = Rc::new(Cell::new(true));
            let admitted = ready.clone();
            let lease = registration
                .bind_anchor("B".to_string(), move || admitted.get())
                .expect("open B must admit its measurement lease");
            let served: Vec<_> = ["C", "B", "A"]
                .into_iter()
                .map(|id| {
                    let mut session = super::super::row::row_specimen(id);
                    session.pinned = id != "B";
                    session.title = format!("initial {id}").into();
                    session
                })
                .collect();
            let held = ["A".to_string(), "B".to_string(), "C".to_string()];
            let shown = rows::held_display_order(&held, &served);
            assert_eq!(
                shown.iter().map(|row| row.id.raw()).collect::<Vec<_>>(),
                ["A", "B", "C"]
            );
            Rc::new(Self {
                owner,
                registration,
                lease: RefCell::new(Some(lease)),
                open,
                deliveries,
                ready,
                bindings: Cell::new(1),
                rows: RefCell::new(Some(
                    shown.into_iter().map(|row| (row, false, None)).collect(),
                )),
                scope: Cell::new(None),
            })
        }

        /// Drain mount only; subsequent renders intentionally leave delivery
        /// unsettled until the test has published the competing transition.
        fn mount(self: &Rc<Self>) -> VirtualDom {
            let mut dom = VirtualDom::new_with_props(identity_render_probe, self.clone());
            dom.rebuild_to_vec();
            dom.process_events();
            assert_eq!(self.deliveries.get(), 0, "mount is a baseline");
            assert!(self.ready.get() && self.lease.borrow().is_some());
            dom
        }

        /// Read current inputs through the production snapshot resolver.
        fn snapshot(&self) -> IdentitySnapshot {
            identity_snapshot(self.rows.borrow().as_deref(), false)
        }

        /// Change an actual row input, rather than manufacturing a selected
        /// audience or returning a prebuilt ticket from the test component.
        fn title(&self, id: &str, title: &str) {
            let mut rows = self.rows.borrow_mut();
            let row = rows
                .as_mut()
                .expect("successful fixture listing")
                .iter_mut()
                .find(|row| row.0.id.raw() == id)
                .expect("held row exists");
            row.0.title = title.into();
        }

        /// Return committed witness edits without polling the queued effect.
        fn render(&self, dom: &mut VirtualDom) -> Vec<dioxus::core::Mutation> {
            dom.mark_dirty(self.scope.get().expect("probe mounted"));
            dom.render_immediate_to_vec().edits
        }

        /// Revoke the old opening before restoring Some(B), then bind the new
        /// measurement lifetime through the same production registration API.
        fn reopen(&self) {
            *self.open.borrow_mut() = None;
            self.registration.forget_anchor();
            *self.open.borrow_mut() = Some("B".to_string());
            let ready = self.ready.clone();
            *self.lease.borrow_mut() = Some(
                self.registration
                    .bind_anchor("B".to_string(), move || ready.get())
                    .expect("fresh B opening admits a distinct lease"),
            );
            self.bindings.set(self.bindings.get() + 1);
        }
    }

    /// Render a minimal witness of the exact production snapshot, while using
    /// the production capture helper and common deferred hook unchanged. The
    /// real SessionRow markup is covered separately; this test owns timing.
    fn identity_render_probe(fixture: Rc<IdentityRenderFixture>) -> Element {
        fixture.scope.set(Some(dioxus::core::current_scope_id()));
        let snapshot = fixture.snapshot();
        use_rendered_movement_between(&snapshot, |before, after| {
            capture_identity_movement(&fixture.owner, before, after)
        });
        let witness = format!("{snapshot:?}");
        rsx! { span { "{witness}" } }
    }

    /// A committed above-B cause must survive a below-B-only render before
    /// effects run. The second transition has a genuinely empty selected set,
    /// while B's readiness and exact lease remain unchanged throughout.
    #[farhelm_testtrace::test]
    fn identity_pending_close_survives_an_unrelated_successful_render() {
        let fixture = IdentityRenderFixture::new();
        let mut dom = fixture.mount();
        let before = fixture.snapshot();
        fixture.title("A", "A expanded title");
        let first = fixture.snapshot();
        assert_eq!(identity_rows_moved(&before, &first), ["B", "C"]);
        let edits = fixture.render(&mut dom);
        assert!(edits.iter().any(|edit| matches!(edit,
            dioxus::core::Mutation::SetText { value, .. } if value.contains("A expanded title"))));
        assert_eq!(fixture.deliveries.get(), 0);
        fixture.title("C", "C independently expanded");
        assert!(identity_rows_moved(&first, &fixture.snapshot()).is_empty());
        let edits = fixture.render(&mut dom);
        assert!(edits.iter().any(|edit| matches!(edit,
            dioxus::core::Mutation::SetText { value, .. } if value.contains("C independently expanded"))));
        assert_eq!(fixture.open.borrow().as_deref(), Some("B"));
        assert!(fixture.ready.get() && fixture.lease.borrow().is_some());
        assert_eq!(fixture.bindings.get(), 1, "both changes use the same lease");
        assert_eq!(
            fixture.deliveries.get(),
            0,
            "no delivery before effect polling"
        );
        dom.process_events();
        assert_eq!(fixture.deliveries.get(), 1);
        assert!(fixture.open.borrow().is_none());
        dom.process_events();
        assert_eq!(fixture.deliveries.get(), 1, "pending drains once");
    }

    /// None is an explicit comparable-baseline reset, even if logical
    /// readiness recovers before delivery and the old fixture lease survives.
    /// This isolates reset from teardown's independent weak-ticket rejection.
    #[farhelm_testtrace::test]
    fn identity_pending_close_resets_across_unavailable_and_recovery_renders() {
        let fixture = IdentityRenderFixture::new();
        let mut dom = fixture.mount();
        fixture.title("A", "A changed before Err");
        fixture.render(&mut dom);
        let recovery = fixture.rows.borrow().clone();
        fixture.ready.set(false);
        *fixture.rows.borrow_mut() = None;
        fixture.render(&mut dom);
        fixture.ready.set(true);
        *fixture.rows.borrow_mut() = recovery;
        fixture.render(&mut dom);
        assert_eq!(fixture.deliveries.get(), 0);
        assert_eq!(fixture.bindings.get(), 1);
        dom.process_events();
        assert_eq!(
            fixture.deliveries.get(),
            0,
            "recovery must not replay the old cause"
        );
        assert_eq!(fixture.open.borrow().as_deref(), Some("B"));
        fixture.title("A", "A genuinely changes after recovery");
        fixture.render(&mut dom);
        dom.process_events();
        assert_eq!(fixture.deliveries.get(), 1);
    }

    /// Retaining an earlier ticket must not reinterpret it as a fresh opening.
    /// A later genuine above-B cause, rather than a below-B render, is required
    /// to close the newly measured opening.
    #[farhelm_testtrace::test]
    fn identity_pending_close_rejects_reopening_until_a_new_cause() {
        let fixture = IdentityRenderFixture::new();
        let mut dom = fixture.mount();
        fixture.title("A", "A changed for the old opening");
        fixture.render(&mut dom);
        fixture.reopen();
        let before = fixture.snapshot();
        fixture.title("C", "C below the fresh opening");
        assert!(identity_rows_moved(&before, &fixture.snapshot()).is_empty());
        fixture.render(&mut dom);
        assert_eq!(fixture.deliveries.get(), 0);
        assert_eq!(fixture.bindings.get(), 2);
        dom.process_events();
        assert_eq!(fixture.deliveries.get(), 0);
        assert_eq!(fixture.open.borrow().as_deref(), Some("B"));
        fixture.title("A", "A genuinely changes for the fresh opening");
        fixture.render(&mut dom);
        dom.process_events();
        assert_eq!(fixture.deliveries.get(), 1);
    }

    /// A held source can change pin flow without changing index/separator.
    /// Select by the actual held vector, never numeric identity or served sort;
    /// own-row and below-row contributions cannot move an earlier top anchor.
    #[farhelm_testtrace::test]
    fn identity_selection_uses_current_held_rows_and_strict_prefixes() {
        let c = super::super::row::row_specimen("9303");
        let mut a = super::super::row::row_specimen("9302");
        let b = super::super::row::row_specimen("9301");
        let held = vec!["9303".to_string(), "9302".to_string(), "9301".to_string()];
        let renamed = HashMap::from([(
            "9302".to_string(),
            ("actual optimistic title".to_string(), 1),
        )]);
        let prepare = |served: &[Session]| {
            apply_optimistic_renames(&rows::held_display_order(&held, served), &renamed)
                .into_iter()
                .map(|session| (session, false, None))
                .collect::<Vec<RenderedSession>>()
        };
        let before_rows = prepare(&[b.clone(), a.clone(), c.clone()]);
        assert_eq!(
            before_rows
                .iter()
                .map(|row| row.0.id.raw())
                .collect::<Vec<_>>(),
            ["9303", "9302", "9301"]
        );
        assert_eq!(before_rows[1].0.title.raw(), "actual optimistic title");
        let before = identity_snapshot(Some(&before_rows), false);
        a.pinned = true;
        let after_rows = prepare(&[a.clone(), c.clone(), b.clone()]);
        let after = identity_snapshot(Some(&after_rows), false);
        assert_eq!(identity_rows_moved(&before, &after), ["9301"]);
        assert_eq!(
            rows::pin_separator_after(
                &before_rows
                    .iter()
                    .map(|row| row.0.clone())
                    .collect::<Vec<_>>()
            ),
            None
        );
        assert_eq!(
            rows::pin_separator_after(
                &after_rows
                    .iter()
                    .map(|row| row.0.clone())
                    .collect::<Vec<_>>()
            ),
            None
        );
        let mut below_rows = after_rows.clone();
        below_rows[2].0.pinned = true;
        assert!(
            identity_rows_moved(&after, &identity_snapshot(Some(&below_rows), false)).is_empty()
        );
        assert!(
            identity_rows_moved(
                &identity_snapshot(Some(&before_rows), true),
                &identity_snapshot(Some(&after_rows), true),
            )
            .is_empty()
        );
    }

    /// One accepted listing may exchange complete identities wholly above a
    /// target. Atomic cancellation must survive, while duplicate counts and
    /// text/activity pairing must not be flattened into independent bags.
    #[farhelm_testtrace::test]
    fn identity_prefix_preserves_atomic_exchanges_multiplicity_and_pairing() {
        let row = |id: &str, title: &str, age: &str| {
            let mut session = super::super::row::row_specimen(id);
            session.pinned = true;
            session.title = title.into();
            (
                session,
                false,
                Some(ActivityStamp {
                    age: age.to_string(),
                    absolute: "tooltip".to_string(),
                }),
            )
        };
        let before_rows = vec![
            row("a", "short", "1m"),
            row("b", "long title", "10m"),
            row("target", "target", "1m"),
        ];
        let exchanged = vec![
            row("a", "long title", "10m"),
            row("b", "short", "1m"),
            row("target", "target", "1m"),
        ];
        let before = identity_snapshot(Some(&before_rows), false);
        let exchanged = identity_snapshot(Some(&exchanged), false);
        assert_eq!(identity_rows_moved(&before, &exchanged), ["b"]);
        let re_paired = vec![
            row("a", "short", "10m"),
            row("b", "long title", "1m"),
            row("target", "target", "1m"),
        ];
        assert!(
            identity_rows_moved(&before, &identity_snapshot(Some(&re_paired), false))
                .contains(&"target".to_string())
        );
        let duplicates = vec![
            row("a", "same", "1m"),
            row("b", "same", "1m"),
            row("c", "other", "1m"),
            row("target", "target", "1m"),
        ];
        let changed_count = vec![
            row("a", "same", "1m"),
            row("b", "other", "1m"),
            row("c", "other", "1m"),
            row("target", "target", "1m"),
        ];
        assert!(
            identity_rows_moved(
                &identity_snapshot(Some(&duplicates), false),
                &identity_snapshot(Some(&changed_count), false),
            )
            .contains(&"target".to_string())
        );
    }

    /// Err/loading means no measured row baseline. Recovery and fresh rows get
    /// fresh measurements; local deleting state still compares in a surviving
    /// successful render, including compact rows whose copy cannot wrap.
    #[farhelm_testtrace::test]
    fn identity_selection_preserves_unavailable_baselines_and_local_delete_values() {
        let rows = vec![
            (super::super::row::row_specimen("a"), false, None),
            (super::super::row::row_specimen("target"), false, None),
        ];
        for compact in [false, true] {
            let before = identity_snapshot(Some(&rows), compact);
            assert!(identity_rows_moved(&None, &before).is_empty());
            assert!(identity_rows_moved(&before, &None).is_empty());
            assert!(identity_rows_moved(&Some(Vec::new()), &before).is_empty());
            let mut deleting = rows.clone();
            deleting[0].1 = true;
            let after = identity_snapshot(Some(&deleting), compact);
            assert_eq!(identity_rows_moved(&before, &after), ["target"]);
            let mut fresh = deleting.clone();
            fresh.push((super::super::row::row_specimen("fresh"), false, None));
            assert_eq!(
                identity_rows_moved(&before, &identity_snapshot(Some(&fresh), compact)),
                ["target"]
            );
            let mut same_label = deleting;
            same_label[0].0.title = "hidden title".into();
            same_label[0].0.status = SessionStatus::Exited { exit_code: Some(5) };
            assert_eq!(after, identity_snapshot(Some(&same_label), compact));
        }
    }

    /// Every Delete/Replace publisher uses this real context setter. It must
    /// report before replacing visible text, preserve None/same-display writes,
    /// and treat construction with a retained notice as a mount baseline.
    #[farhelm_testtrace::test]
    fn delete_notice_reports_actual_display_changes_before_publication() {
        use crate::anchored_popovers::Registration;
        thread_local! {
            static OWNER: RefCell<Option<MovementOwner>> = const { RefCell::new(None) };
            static NOTICE: Cell<Option<DeleteNotice>> = const { Cell::new(None) };
        }
        fn app() -> Element {
            let owner = OWNER.with(|owner| owner.borrow().clone().unwrap());
            let value = use_signal(|| Some("retained".to_string()));
            let notice = use_hook(move || DeleteNotice::new(value, owner));
            NOTICE.with(|slot| slot.set(Some(notice)));
            rsx! {}
        }
        /// Model a fresh root opening, followed by its existing measurement.
        fn open(
            registration: &Registration,
            row: &RefCell<Option<String>>,
            ready: Rc<Cell<bool>>,
        ) -> crate::anchored_popovers::AnchorLease {
            registration.forget_anchor();
            *row.borrow_mut() = Some("target".into());
            registration
                .bind_anchor("target".into(), move || ready.get())
                .unwrap()
        }
        let owner = MovementOwner::default();
        let row = Rc::new(RefCell::new(None::<String>));
        let observed = Rc::new(RefCell::new(Vec::<Option<String>>::new()));
        let read_row = row.clone();
        let close_row = row.clone();
        let closed = observed.clone();
        let registration = owner.register(
            Interest::new(AnchorRegion::Sessions),
            move || read_row.borrow().clone(),
            move || {
                let notice = NOTICE.with(|slot| slot.get().unwrap());
                closed.borrow_mut().push(notice.0.peek().clone());
                *close_row.borrow_mut() = None;
            },
        );
        let ready = Rc::new(Cell::new(true));
        let first = open(&registration, &row, ready.clone());
        OWNER.with(|slot| *slot.borrow_mut() = Some(owner));
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        assert!(
            observed.borrow().is_empty(),
            "retained initial text is no event"
        );
        dom.in_runtime(|| {
            let notice = NOTICE.with(|slot| slot.get().unwrap());
            notice.publish(None);
            notice.publish(Some("retained".into()));
            assert!(observed.borrow().is_empty());
            assert_eq!(notice.0.peek().as_deref(), Some("retained"));
            notice.publish(Some("notice\u{202e}".into()));
            assert_eq!(*observed.borrow(), [Some("retained".into())]);
            assert_eq!(notice.0.peek().as_deref(), Some("notice\u{202e}"));
            let second = open(&registration, &row, ready.clone());
            drop(first);
            notice.publish(Some("notice<U+202E>".into()));
            assert_eq!(
                observed.borrow().len(),
                1,
                "equal escaped display produces no report"
            );
            assert_eq!(
                notice.0.peek().as_deref(),
                Some("notice<U+202E>"),
                "the original same-display write is retained"
            );
            notice.dismiss();
            assert_eq!(
                observed.borrow().last(),
                Some(&Some("notice<U+202E>".into()))
            );
            assert!(notice.0.peek().is_none());
            notice.dismiss();
            assert_eq!(
                observed.borrow().len(),
                2,
                "stale repeated dismiss is no movement"
            );
            let _third = open(&registration, &row, ready.clone());
            drop(second);
            ready.set(false);
            notice.publish(Some("changed while listing failed".into()));
            assert_eq!(
                observed.borrow().len(),
                2,
                "Err root slot has no admitted DOM"
            );
        });
        drop(dom);
        NOTICE.with(|slot| slot.set(None));
        OWNER.with(|slot| slot.borrow_mut().take());
    }

    /// Upgraded helms must enable naming without a stored preference, while
    /// revealed New and an explicit opt-out each independently prevent it.
    #[farhelm_testtrace::test]
    fn naming_after_create_requires_hidden_new_and_an_enabled_preference() {
        assert!(
            name_created_session(true, None),
            "unset enables upgraded helms"
        );
        assert!(name_created_session(true, Some(true)));
        assert!(
            !name_created_session(true, Some(false)),
            "explicit opt-out wins"
        );
        assert!(!name_created_session(false, None));
        assert!(
            !name_created_session(false, Some(true)),
            "New already showed the field"
        );
        assert!(!name_created_session(false, Some(false)));
    }

    /// Accepting naming is a byte-exact no-op only in the post-launch mode.
    /// Ordinary Rename still submits unchanged text, and an edited display
    /// spelling or whitespace change must reach the supervisor verbatim.
    #[farhelm_testtrace::test]
    fn naming_accepts_only_its_unchanged_raw_title_without_a_request() {
        let editor = RenameEditor {
            id: "new-session".into(),
            current_title: "original\nname".into(),
            generation: 1,
            unavailable: false,
            shortcut: false,
            naming: true,
        };
        assert!(naming_keeps_current_title(&editor, "original\nname"));
        assert!(!naming_keeps_current_title(
            &editor,
            &crate::peer::editable_peer(&editor.current_title)
        ));
        assert!(!naming_keeps_current_title(&editor, "original\nname "));
        assert!(!naming_keeps_current_title(&editor, "different"));
        assert!(!naming_keeps_current_title(
            &RenameEditor {
                naming: false,
                ..editor.clone()
            },
            &editor.current_title
        ));
        let empty = RenameEditor {
            current_title: String::new(),
            ..editor
        };
        assert!(naming_keeps_current_title(&empty, ""));
    }

    /// A read/unread toggle's report must not panic when the list that
    /// queued it has unmounted. Its writer deliberately outlives the list,
    /// and the browser's token prompt replaces the list, before the page
    /// reloads, while a save may be in flight; a panicking report
    /// crashed the window (SPEC.md "Signing in again"). Spec: while the list
    /// is mounted, a failure lands on the row's error line and a success
    /// clears it and increments the refresh counter. Failure never requests
    /// a refresh; once the list is gone, either outcome is dropped quietly.
    #[farhelm_testtrace::test]
    fn seen_toggle_report_tolerates_an_unmounted_list() {
        use std::cell::Cell;
        std::thread_local! {
            static SHOW: Cell<bool> = const { Cell::new(true) };
            static ERRORS: Cell<Option<Signal<HashMap<String, String>>>> =
                const { Cell::new(None) };
            static SUCCESSES: Cell<Option<Signal<u64>>> = const { Cell::new(None) };
        }

        /// Own both report destinations in a disposable child scope, so
        /// unmount tests dropped signals rather than merely an absent row.
        #[component]
        fn List() -> Element {
            let errors = use_signal(HashMap::<String, String>::new);
            let successes = use_signal(|| 0_u64);
            ERRORS.with(|slot| slot.set(Some(errors)));
            SUCCESSES.with(|slot| slot.set(Some(successes)));
            rsx! {}
        }

        /// Keep the runtime alive after dropping the list, as sign-in does.
        fn app() -> Element {
            let show = SHOW.with(Cell::get);
            rsx! {
                if show {
                    List {}
                }
            }
        }

        // Render the parent explicitly because SHOW is an external test
        // control, not a signal that schedules its own invalidation.
        let rerender = |dom: &mut VirtualDom| {
            dom.mark_dirty(dioxus::core::ScopeId::APP);
            dom.render_immediate(&mut dioxus::core::NoOpMutations);
        };
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        let errors = ERRORS.with(Cell::get).expect("the list mounted");
        let successes = SUCCESSES.with(Cell::get).expect("the list mounted");

        dom.in_runtime(|| {
            seen_toggle_report(errors, successes, "s1".to_string())(Err("refused".into()))
        });
        dom.in_runtime(|| {
            assert_eq!(
                errors.peek().get("s1").map(String::as_str),
                Some("seen: refused"),
                "a mounted list shows the failure on the row"
            );
            assert_eq!(*successes.peek(), 0, "a failure requests no refresh");
        });
        dom.in_runtime(|| seen_toggle_report(errors, successes, "s1".to_string())(Ok(())));
        dom.in_runtime(|| {
            assert!(errors.peek().get("s1").is_none(), "a success clears it");
            assert_eq!(*successes.peek(), 1, "a success requests one refresh");
        });

        SHOW.with(|show| show.set(false));
        rerender(&mut dom);
        rerender(&mut dom);
        dom.in_runtime(|| {
            assert!(
                errors.try_peek().is_err(),
                "the list's signal is gone with it"
            );
            assert!(
                successes.try_peek().is_err(),
                "the refresh signal is gone too"
            );
            seen_toggle_report(errors, successes, "s1".to_string())(Err("refused".into()));
            seen_toggle_report(errors, successes, "s1".to_string())(Ok(()));
        });
    }

    /// A successful create mirrors only submitted permissions. Command launches
    /// leave memory alone, and historical trust memory remains inert even if
    /// the submitted selection carries a compatibility value.
    ///
    /// Why: the mirror used to copy the created session's `launch`, which is
    /// the supervisor's reply. A misbehaving remote host could make this
    /// page's next New dialog preselect "yolo" for every
    /// host until reload, even with the helm's own stored defaults untouched.
    /// The helper takes no `Session` at all, so the reply cannot reach it.
    #[farhelm_testtrace::test]
    fn the_preference_mirror_follows_only_the_submitted_launch() {
        let mut preferences = crate::api::Preferences {
            remembered_permissions: Some("smart_approve".to_string()),
            remembered_workspace_trust: Some(false),
            ..crate::api::Preferences::default()
        };
        mirror_submitted_launch(&mut preferences, None);
        assert_eq!(
            preferences.remembered_permissions.as_deref(),
            Some("smart_approve")
        );
        assert_eq!(preferences.remembered_workspace_trust, Some(false));

        let submitted = farhelm_proto::LaunchSelection {
            permission_flags: None,
            harness: farhelm_proto::LaunchHarness::Codex,
            model: None,
            effort: None,
            permissions: None,
            wrapper_prefix: None,
            workspace_trust: Some(true),
        };
        mirror_submitted_launch(&mut preferences, Some(&submitted));
        assert_eq!(
            preferences.remembered_permissions, None,
            "a default-permissions submission clears the remembered mode"
        );
        assert_eq!(
            preferences.remembered_workspace_trust,
            Some(false),
            "even a historical trust value must leave retired memory untouched"
        );

        let omitted_yolo_default = farhelm_proto::LaunchSelection {
            permission_flags: None,
            harness: farhelm_proto::LaunchHarness::Goose,
            model: None,
            effort: None,
            permissions: Some(farhelm_proto::LaunchPermission::Yolo),
            wrapper_prefix: None,
            workspace_trust: None,
        };
        mirror_submitted_launch(&mut preferences, Some(&omitted_yolo_default));
        assert_eq!(
            preferences.remembered_permissions, None,
            "a harness default that normalizes to YOLO must not become a remembered override"
        );
    }

    #[farhelm_testtrace::test]
    fn rename_availability_changes_only_on_authoritative_absence() {
        let editor = RenameEditor {
            id: "source".to_string(),
            current_title: "original title".to_string(),
            generation: 17,
            unavailable: false,
            shortcut: false,
            naming: false,
        };
        let listing = |sessions, omits_fleet_members| SessionListing {
            sessions,
            total: 0,
            matching: None,
            filtered: false,
            omits_fleet_members,
            truncated: false,
        };

        assert_eq!(
            listed_rename_unavailable(&editor, true, &listing(Vec::new(), true)),
            None,
            "a partial view cannot prove why the source is absent"
        );
        assert_eq!(
            listed_rename_unavailable(&editor, false, &listing(Vec::new(), false)),
            None,
            "a mutation-local read has no fleet-wide standing even when its reply is complete"
        );
        assert_eq!(
            listed_rename_unavailable(&editor, true, &listing(Vec::new(), false)),
            Some(true),
            "a complete authoritative absence makes submission unavailable"
        );

        let source = crate::list::row::row_specimen("source");
        let mut reconciled = RenameEditor {
            unavailable: true,
            ..editor.clone()
        };
        reconciled.unavailable =
            listed_rename_unavailable(&reconciled, true, &listing(vec![source], false))
                .expect("an authoritative listing supplies an availability answer");
        assert!(!reconciled.unavailable);
        assert_eq!(reconciled.id, editor.id);
        assert_eq!(reconciled.current_title, editor.current_title);
        assert_eq!(reconciled.generation, editor.generation);
    }

    /// Delayed results and focus intents belong to one editor generation.
    ///
    /// Pending rename cannot be cancelled through the product UI, so the
    /// otherwise unreachable same-id replacement race is pinned at the pure
    /// ownership gate instead of adding a bypass that would exist only for a
    /// browser test.
    #[farhelm_testtrace::test]
    fn delayed_rename_work_cannot_act_on_a_newer_editor_generation() {
        let editor = RenameEditor {
            id: "same-session".to_string(),
            current_title: "newer draft source".to_string(),
            generation: 9,
            unavailable: false,
            shortcut: false,
            naming: false,
        };

        assert!(rename_result_owns_editor(Some(&editor), 9));
        assert!(
            !rename_result_owns_editor(Some(&editor), 8),
            "the same session id does not let an old result close the reopened editor"
        );
        assert_eq!(
            rename_focus_return_state(9, Some(&editor)),
            RenameFocusReturn::Wait,
            "the originating dialog must unmount before focus can return"
        );
        assert_eq!(
            rename_focus_return_state(8, Some(&editor)),
            RenameFocusReturn::Discard,
            "an old intent must not focus through a newer mounted dialog"
        );
        assert_eq!(
            rename_focus_return_state(9, None),
            RenameFocusReturn::Ready,
            "the matching intent becomes eligible only after unmount"
        );
    }

    /// A clone (or replace — the two share this predicate, see its own doc)
    /// click is refused by each guard independently, and
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
    fn a_clone_is_refused_by_any_one_of_its_guards() {
        let id = "session-1";
        let empty = HashSet::new();
        let pending = HashSet::from([id.to_string()]);

        assert!(
            !clone_is_refused(false, id, &empty, None, None),
            "nothing is holding this row, so the clone must proceed"
        );
        assert!(
            clone_is_refused(true, id, &empty, None, None),
            "the shared page-wide lock alone must refuse it"
        );
        assert!(
            clone_is_refused(false, id, &pending, None, None),
            "a stop or delete already in flight for THIS row must refuse it"
        );
        assert!(
            clone_is_refused(false, id, &empty, Some(RowQuestion::Delete), None),
            "an open delete confirmation on THIS row must refuse it"
        );
        assert!(
            clone_is_refused(false, id, &empty, Some(RowQuestion::Replace), None),
            "an open replace confirmation on THIS row must refuse it"
        );
        assert!(
            clone_is_refused(false, id, &empty, None, Some(id)),
            "this row's own open rename editor must refuse it"
        );
        // A guard keyed to a DIFFERENT row must never refuse this one —
        // the per-row state is per-row for exactly this reason, and a
        // guard that read it as a single shared flag would block every
        // clone in the list the instant any one row was mid-operation.
        // (`asking` is already this row's own question, looked up by id.)
        let other = HashSet::from(["other-row".to_string()]);
        assert!(
            !clone_is_refused(false, id, &other, None, Some("other-row")),
            "another row's operation or rename must not block this row's clone"
        );
    }

    /// Why this matters: splitting the rows' questions out of their
    /// operation state must not loosen the exclusion between the two. A
    /// second stop or delete admitted on a row that already has one in
    /// flight would race the first, and an operation admitted under an open
    /// question would make that question's eventual confirm a silent no-op.
    ///
    /// Spec (`admit_row_op`): a row is admitted once and refused while it is
    /// pending; a row whose question is open is refused and left unchanged;
    /// another row's pending operation never refuses this one; ending the
    /// operation (removing it from the set) admits the row again.
    #[farhelm_testtrace::test]
    fn the_row_operation_exclusion_still_blocks_a_second_operation() {
        let mut pending = HashSet::new();
        assert!(
            admit_row_op(&mut pending, false, "row-a"),
            "a free row is admitted"
        );
        assert!(
            !admit_row_op(&mut pending, false, "row-a"),
            "a second operation on a pending row is refused"
        );
        assert!(
            admit_row_op(&mut pending, false, "row-b"),
            "another row's operation does not block this one"
        );
        assert!(
            !admit_row_op(&mut pending, true, "row-c"),
            "a row with its question open admits no operation"
        );
        assert!(
            !pending.contains("row-c"),
            "a refused admission leaves the row out of the exclusion"
        );
        pending.remove("row-a");
        assert!(
            admit_row_op(&mut pending, false, "row-a"),
            "an ended operation frees its row"
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
    /// Default first is presentation: the menu highlights the shared choice,
    /// while a new user meets the usual ordering before the alternatives.
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
            "the default order leads the offered choices"
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
    /// happened to visit last. MISSING `created_at` is an older helm (`Session::created_at`),
    /// where the honest answer is the one this fallback gave before the field
    /// was decoded at all.
    #[farhelm_testtrace::test]
    fn the_fallback_picks_the_newest_created_row_not_the_first_one() {
        // Ids are spelled as words rather than as UUIDs so each assertion
        // reads as "which session won" — the row's position and its stamp
        // are what every case here is about, and a real id would hide both.
        fn row(id: &str, created_at: i64) -> Session {
            Session {
                created_at,
                ..crate::list::row::row_specimen(id)
            }
        }

        // Title order: the newest session sits last, exactly where a
        // first-row fallback would miss it.
        let by_title = [row("aaa", 300), row("mmm", 100), row("zzz", 500)];
        assert_eq!(
            newest_created_fallback(&by_title, None).map(|s| s.id.raw()),
            Some("zzz")
        );

        // Ties keep the listing's own order. `Iterator::max_by` returns the
        // LAST maximal element, so a naive `max_by` here would answer "third"
        // — and would change its answer as unrelated rows were appended.
        let tied = [row("first", 500), row("second", 100), row("third", 500)];
        assert_eq!(
            newest_created_fallback(&tied, None).map(|s| s.id.raw()),
            Some("first"),
            "equal creation stamps must resolve to the listing's own head"
        );

        // A helm too old to send the field leaves every stamp at zero, which
        // means UNKNOWN. The answer is the first row, not the
        // one an ordering over zeroes would single out.
        let stampless = [row("head", 0), row("tail", 0)];
        assert_eq!(
            newest_created_fallback(&stampless, None).map(|s| s.id.raw()),
            Some("head")
        );

        // Mixed: one row does carry a stamp, so the unknowns must not be
        // allowed to beat it by being earlier in the list.
        let mixed = [row("unknown", 0), row("stamped", 1)];
        assert_eq!(
            newest_created_fallback(&mixed, None).map(|s| s.id.raw()),
            Some("stamped"),
            "a real timestamp outranks a missing one wherever the row sits"
        );

        assert_eq!(newest_created_fallback(&[], None), None);
    }

    /// A cleared pane returns to its first listed visit even when creation
    /// order disagrees. A filter narrows candidates, and a stale deleted row
    /// cannot win through either history or the creation-time fallback.
    #[farhelm_testtrace::test]
    fn automatic_selection_respects_visits_listing_and_removed_exclusion() {
        let rows = [
            Session {
                created_at: 100,
                ..crate::list::row::row_specimen("a")
            },
            Session {
                created_at: 200,
                ..crate::list::row::row_specimen("b")
            },
            Session {
                created_at: 300,
                ..crate::list::row::row_specimen("c")
            },
        ];
        let history = ["missing", "b", "a"].map(PeerText::from);
        let pick = |listed: &[Session], history: &[PeerText], removed| {
            automatic_selection(listed, history, None, removed)
                .map(|session| session.id.raw().to_owned())
        };
        assert_eq!(pick(&rows, &history, None).as_deref(), Some("b"));
        assert_eq!(pick(&rows, &history, Some("b")).as_deref(), Some("a"));
        assert_eq!(
            pick(&rows[2..], &history, None).as_deref(),
            Some("c"),
            "filtered-out visits are not candidates"
        );
        assert_eq!(pick(&rows, &[], None).as_deref(), Some("c"));
        assert_eq!(pick(&rows, &[], Some("c")).as_deref(), Some("b"));
        assert_eq!(pick(&rows[..1], &history, Some("a")), None);
        assert_eq!(pick(&[], &history, None), None);
        let unknown = [
            crate::list::row::row_specimen("gone"),
            crate::list::row::row_specimen("survivor"),
        ];
        assert_eq!(
            pick(&unknown, &[], Some("gone")).as_deref(),
            Some("survivor")
        );
    }

    /// Migration leaves the new history empty but retains a known selection.
    /// Treat that value as one visit until normal writes establish history;
    /// it must obey the same candidate/exclusion rules and never override or
    /// extend nonempty history.
    #[farhelm_testtrace::test]
    fn automatic_selection_preserves_a_known_visit_with_empty_history() {
        let rows = [
            Session {
                created_at: 100,
                ..crate::list::row::row_specimen("a")
            },
            Session {
                created_at: 300,
                ..crate::list::row::row_specimen("c")
            },
        ];
        let pick = |history: &[PeerText], legacy, removed| {
            automatic_selection(&rows, history, legacy, removed).map(|session| session.id.raw())
        };
        assert_eq!(pick(&[], Some("a"), None), Some("a"));
        assert_eq!(pick(&[], Some("a"), Some("a")), Some("c"));
        assert_eq!(pick(&[], Some("absent"), None), Some("c"));
        assert_eq!(pick(&["c".into()], Some("a"), None), Some("c"));
        assert_eq!(
            pick(&["missing".into()], Some("a"), None),
            Some("c"),
            "nonempty stale history must not append a differing legacy value"
        );
    }
}
